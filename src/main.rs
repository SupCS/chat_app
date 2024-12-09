mod auth;
mod db;
mod handlers;
mod models;

use db::connect_to_db;
use handlers::auth::{login_handler, register_handler};
use handlers::chat::{chat_handler, get_chat_history_handler, get_user_chats_handler, Clients};
use mongodb::Database;
use std::sync::Arc;
use warp::{http::Method, http::StatusCode, reject, Filter, Rejection, Reply};

// Власний тип помилки
#[derive(Debug)]
struct CustomError(String);

impl reject::Reject for CustomError {}

// Обробка помилок
async fn handle_rejection(err: Rejection) -> Result<impl Reply, std::convert::Infallible> {
    if let Some(custom_error) = err.find::<CustomError>() {
        let json = warp::reply::json(&serde_json::json!({ "error": custom_error.0 }));
        return Ok(warp::reply::with_status(json, StatusCode::BAD_REQUEST));
    }

    let json = warp::reply::json(&serde_json::json!({ "error": "Unhandled error" }));
    Ok(warp::reply::with_status(
        json,
        StatusCode::INTERNAL_SERVER_ERROR,
    ))
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    env_logger::init();

    // Підключення до бази даних
    let db = Arc::new(connect_to_db().await.expect("Failed to connect to DB"));
    let clients = Arc::new(Clients::new());

    // Налаштування CORS
    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec![Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers(vec!["Content-Type", "Authorization"])
        .build();

    // Маршрути
    let register = warp::path!("register")
        .and(warp::post())
        .and(warp::body::json())
        .and(warp::any().map({
            let db = db.clone();
            move || db.clone()
        }))
        .and_then(register_handler);

    let login = warp::path!("login")
        .and(warp::post())
        .and(warp::body::json())
        .and(warp::any().map({
            let db = db.clone();
            move || db.clone()
        }))
        .and_then(login_handler);

    let chat = warp::path!("chat")
        .and(warp::ws())
        .and(warp::query::<std::collections::HashMap<String, String>>())
        .and(warp::any().map({
            let clients = clients.clone();
            move || clients.clone()
        }))
        .and(warp::any().map({
            let db = db.clone();
            move || db.clone()
        }))
        .and_then(
            |ws: warp::ws::Ws,
             query: std::collections::HashMap<String, String>,
             clients: Arc<Clients>,
             db: Arc<Database>| async move {
                if let Some(token) = query.get("token").cloned() {
                    match crate::auth::validate_jwt(&token) {
                        Ok(_) => {
                            Ok(ws
                                .on_upgrade(move |socket| chat_handler(socket, clients, db, token)))
                        }
                        Err(_) => Err(reject::custom(CustomError("Invalid token".into()))),
                    }
                } else {
                    Err(reject::custom(CustomError("Missing token".into())))
                }
            },
        );

    let chat_list = warp::path!("chats")
        .and(warp::get())
        .and(warp::header::<String>("Authorization"))
        .and(warp::any().map({
            let db = db.clone();
            move || db.clone()
        }))
        .and_then(get_user_chats_handler);

    let chat_history = warp::path!("history")
        .and(warp::get())
        .and(warp::query::<std::collections::HashMap<String, String>>())
        .and(warp::header::<String>("Authorization"))
        .and(warp::any().map({
            let db = db.clone();
            move || db.clone()
        }))
        .and_then(get_chat_history_handler);

    let check_user = warp::path!("users" / "check")
        .and(warp::get())
        .and(warp::query::<std::collections::HashMap<String, String>>())
        .and(warp::any().map({
            let db = db.clone();
            move || db.clone()
        }))
        .and_then(handlers::chat::check_user_handler);

    // Об'єднання маршрутів із застосуванням CORS
    let routes = register
        .or(login)
        .or(chat)
        .or(chat_history)
        .or(chat_list)
        .or(check_user)
        .recover(handle_rejection)
        .with(cors);

    // Запуск сервера
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}
