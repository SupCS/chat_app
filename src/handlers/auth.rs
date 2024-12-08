use crate::auth::{generate_jwt, hash_password, verify_password};
use crate::db::user::{find_user_by_username, save_user};
use crate::models::user::User;
use mongodb::Database;
use std::sync::Arc;
use warp::{http::StatusCode, reply, Rejection, Reply};

fn json_response(message: &str, status: StatusCode) -> warp::reply::Response {
    reply::with_status(
        reply::json(&serde_json::json!({ "message": message })),
        status,
    )
    .into_response()
}

pub async fn register_handler(
    user: User,
    db: Arc<Database>,
) -> Result<warp::reply::Response, Rejection> {
    let collection = db.collection::<User>("users");

    // Перевіряємо, чи існує користувач
    if let Some(_) = find_user_by_username(&user.username, &collection)
        .await
        .unwrap_or(None)
    {
        return Ok(json_response(
            "Username already taken",
            StatusCode::CONFLICT,
        ));
    }

    // Хешуємо пароль
    let hashed_password = hash_password(&user.password);

    // Створюємо нового користувача
    let new_user = User::new(&user.username, &hashed_password);

    // Зберігаємо користувача
    save_user(&new_user, &collection).await.unwrap();

    Ok(json_response(
        "User registered successfully",
        StatusCode::CREATED,
    ))
}

pub async fn login_handler(
    user: User,
    db: Arc<Database>,
) -> Result<warp::reply::Response, Rejection> {
    let collection = db.collection::<User>("users");

    // Знаходимо користувача за іменем
    if let Some(stored_user) = find_user_by_username(&user.username, &collection)
        .await
        .unwrap_or(None)
    {
        // Перевіряємо пароль
        if verify_password(&stored_user.password, &user.password) {
            // Генеруємо JWT
            let token = generate_jwt(&stored_user.id.unwrap().to_string());

            return Ok(reply::json(&serde_json::json!({
                "token": token
            }))
            .into_response());
        } else {
            return Ok(json_response("Invalid password", StatusCode::UNAUTHORIZED));
        }
    }

    // Якщо користувача не знайдено
    Ok(json_response("User not found", StatusCode::NOT_FOUND))
}
