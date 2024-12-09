use crate::auth::validate_jwt;
use crate::db::message::{get_all_messages, save_message};
use crate::models::message::Message as ChatMessage;
use crate::models::user::User;
use crate::CustomError;

use futures_util::{SinkExt, StreamExt, TryStreamExt};
use mongodb::bson::oid::ObjectId;
use mongodb::bson::{self, doc};
use mongodb::Database;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc;
use warp::ws::{Message, WebSocket};
use warp::{http::StatusCode, Rejection, Reply};

// Тип для каналу повідомлень
type Tx = mpsc::UnboundedSender<Message>;

// Структура для зберігання клієнтів
#[derive(Debug, Clone)]
pub struct Clients {
    pub senders: Arc<tokio::sync::Mutex<Vec<Tx>>>,
}

impl Clients {
    pub fn new() -> Self {
        Clients {
            senders: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }

    pub async fn broadcast(&self, message: String) {
        let mut clients = self.senders.lock().await;

        // Відправляємо повідомлення всім клієнтам
        clients.retain(|client| client.send(Message::text(message.clone())).is_ok());
    }
}

// Обробник WebSocket-з'єднання
pub async fn chat_handler(ws: WebSocket, clients: Arc<Clients>, db: Arc<Database>, token: String) {
    let (mut ws_tx, mut ws_rx) = ws.split();
    let (client_tx, mut client_rx) = mpsc::unbounded_channel();

    // Додаємо нового клієнта
    clients.senders.lock().await.push(client_tx.clone());
    println!(
        "Новий клієнт підключився. Всього клієнтів: {}",
        clients.senders.lock().await.len()
    );

    let clients_clone = clients.clone();
    let db_clone = db.clone();

    // Витягуємо користувача з токена
    let claims = match validate_jwt(&token) {
        Ok(claims) => claims,
        Err(_) => {
            eprintln!("Невалідний токен");
            return;
        }
    };

    let authenticated_user_id = claims.sub;

    // Отримуємо username користувача
    let collection = db.collection::<User>("users");
    let authenticated_user = match collection
        .find_one(
            doc! { "_id": ObjectId::parse_str(&authenticated_user_id).unwrap() },
            None,
        )
        .await
    {
        Ok(Some(user)) => user.username,
        _ => {
            eprintln!("Користувач з ID {} не знайдений", authenticated_user_id);
            return;
        }
    };

    println!("Користувач авторизований: {}", authenticated_user);

    // Обробка вхідних повідомлень від WebSocket
    tokio::spawn(async move {
        while let Some(result) = ws_rx.next().await {
            match result {
                Ok(message) => {
                    if let Ok(text) = message.to_str() {
                        println!("Отримано повідомлення: {}", text);

                        if let Ok(json_message) = serde_json::from_str::<Value>(text) {
                            if let (Some(sender), Some(receiver), Some(content)) = (
                                json_message.get("sender").and_then(|v| v.as_str()),
                                json_message.get("receiver").and_then(|v| v.as_str()),
                                json_message.get("content").and_then(|v| v.as_str()),
                            ) {
                                if sender != authenticated_user {
                                    eprintln!(
                                        "Помилка: Відправник {} не відповідає авторизованому користувачу {}",
                                        sender, authenticated_user
                                    );
                                    continue;
                                }

                                println!(
                                    "Відправник: {}, Отримувач: {}, Зміст: {}",
                                    sender, receiver, content
                                );

                                // Зберігаємо повідомлення в MongoDB
                                let chat_message = ChatMessage::new(sender, receiver, content);
                                if let Err(e) =
                                    save_message(&chat_message, &db_clone.collection("messages"))
                                        .await
                                {
                                    eprintln!("Помилка збереження повідомлення: {}", e);
                                    continue;
                                }

                                println!("Повідомлення збережено у базі даних.");

                                // Трансляція повідомлення всім клієнтам
                                if let Ok(serialized_message) = serde_json::to_string(&chat_message)
                                {
                                    clients_clone.broadcast(serialized_message).await;
                                    println!("Повідомлення транслюється клієнтам.");
                                } else {
                                    eprintln!("Помилка серіалізації повідомлення.");
                                }
                            } else {
                                eprintln!("Невірний формат повідомлення: {:?}", json_message);
                            }
                        } else {
                            eprintln!("Не вдалося розібрати повідомлення: {}", text);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Помилка при отриманні повідомлення: {}", e);
                    break;
                }
            }
        }

        println!(
            "WebSocket-з'єднання закрите для користувача: {}",
            authenticated_user
        );
    });

    // Обробка відправки повідомлень до WebSocket клієнта
    tokio::spawn(async move {
        while let Some(message) = client_rx.recv().await {
            println!("Відправка повідомлення клієнту: {:?}", message);
            if let Err(e) = ws_tx.send(message).await {
                eprintln!("Помилка при відправці повідомлення: {}", e);
                break;
            }
        }
    });
}

// Маршрут для отримання історії повідомлень між двома користувачами
pub async fn get_chat_history_handler(
    query_params: std::collections::HashMap<String, String>,
    token: String,
    db: Arc<Database>,
) -> Result<impl Reply, Rejection> {
    println!(
        "Отримано запит на історію з параметрами: {:?}",
        query_params
    );
    println!("Токен для перевірки: {}", token);

    // Валідація токена
    let claims = match validate_jwt(&token) {
        Ok(claims) => {
            println!("JWT успішно валідовано: {:?}", claims);
            claims
        }
        Err(_) => {
            println!("Невалідний токен");
            return Ok(warp::reply::json(
                &serde_json::json!({ "error": "Invalid token" }),
            ));
        }
    };

    // Отримання username з claims.sub
    let collection = db.collection::<User>("users");
    let authenticated_user = match collection
        .find_one(
            doc! { "_id": ObjectId::parse_str(&claims.sub).unwrap() },
            None,
        )
        .await
    {
        Ok(Some(user)) => {
            println!("Авторизований користувач: {}", user.username);
            user.username
        }
        _ => {
            println!("Користувача не знайдено для ID: {}", claims.sub);
            return Ok(warp::reply::json(&serde_json::json!({
                "error": "User not found"
            })));
        }
    };

    let sender = query_params.get("sender").cloned();
    let receiver = query_params.get("receiver").cloned();

    if sender.is_none() || receiver.is_none() {
        return Ok(warp::reply::json(&serde_json::json!({
            "error": "Both sender and receiver must be specified"
        })));
    }

    let sender = sender.unwrap();
    let receiver = receiver.unwrap();

    // Перевіряємо, чи авторизований користувач є учасником розмови
    if authenticated_user != sender && authenticated_user != receiver {
        println!(
            "Авторизований користувач ({}) не є учасником розмови між {} та {}",
            authenticated_user, sender, receiver
        );
        return Ok(warp::reply::json(&serde_json::json!({
            "error": "Unauthorized access to chat history"
        })));
    }

    let collection = db.collection::<ChatMessage>("messages");

    let filter = doc! {
        "$or": [
            { "sender": &sender, "receiver": &receiver },
            { "sender": &receiver, "receiver": &sender }
        ]
    };

    let mut cursor = match collection.find(filter, None).await {
        Ok(cursor) => cursor,
        Err(_) => {
            return Ok(warp::reply::json(&serde_json::json!({
                "error": "Failed to get chat history"
            })));
        }
    };

    let mut messages = Vec::new();

    while let Some(doc) = cursor.try_next().await.unwrap_or_else(|_| None) {
        messages.push(doc);
    }

    Ok(warp::reply::json(&messages))
}

pub async fn get_user_chats_handler(
    token: String,
    db: Arc<Database>,
) -> Result<impl Reply, Rejection> {
    // Валідація токена
    let claims = match validate_jwt(&token) {
        Ok(claims) => claims,
        Err(_) => {
            return Ok(warp::reply::json(&serde_json::json!({
                "error": "Invalid token"
            })));
        }
    };

    // Отримання username автентифікованого користувача
    let user_collection = db.collection::<User>("users");
    let authenticated_user = match user_collection
        .find_one(
            doc! { "_id": ObjectId::parse_str(&claims.sub).unwrap() },
            None,
        )
        .await
    {
        Ok(Some(user)) => user.username,
        _ => {
            return Ok(warp::reply::json(&serde_json::json!({
                "error": "User not found"
            })));
        }
    };

    // Отримання колекції повідомлень
    let messages_collection = db.collection::<ChatMessage>("messages");

    // Пошук унікальних користувачів, з якими був обмін повідомленнями
    let pipeline = vec![
        doc! {
            "$match": {
                "$or": [
                    { "sender": &authenticated_user },
                    { "receiver": &authenticated_user }
                ]
            }
        },
        doc! {
            "$group": {
                "_id": {
                    "$cond": [
                        { "$eq": ["$sender", &authenticated_user] },
                        "$receiver",
                        "$sender"
                    ]
                }
            }
        },
        doc! {
            "$lookup": {
                "from": "users",
                "localField": "_id",
                "foreignField": "username",
                "as": "user_info"
            }
        },
        doc! {
            "$unwind": "$user_info"
        },
        doc! {
            "$project": {
                "username": "$_id",
                "user_id": "$user_info._id"
            }
        },
    ];

    let mut cursor = match messages_collection.aggregate(pipeline, None).await {
        Ok(cursor) => cursor,
        Err(_) => {
            return Ok(warp::reply::json(&serde_json::json!({
                "error": "Failed to retrieve chat list"
            })));
        }
    };

    let mut chat_list = Vec::new();
    while let Some(result) = cursor.next().await {
        match result {
            Ok(document) => {
                if let Ok(chat) = bson::from_document::<ChatUser>(document) {
                    chat_list.push(chat);
                }
            }
            Err(_) => {
                return Ok(warp::reply::json(&serde_json::json!({
                    "error": "Error processing chat list"
                })));
            }
        }
    }

    Ok(warp::reply::json(&chat_list))
}

#[derive(Serialize, Deserialize)]
struct ChatUser {
    user_id: ObjectId,
    username: String,
}

pub async fn check_user_handler(
    query_params: std::collections::HashMap<String, String>,
    db: Arc<Database>,
) -> Result<impl Reply, Rejection> {
    if let Some(username) = query_params.get("username") {
        let collection = db.collection::<User>("users");

        match collection
            .find_one(doc! { "username": username }, None)
            .await
        {
            Ok(Some(_)) => Ok(warp::reply::json(&serde_json::json!({ "exists": true }))),
            Ok(None) => Ok(warp::reply::json(&serde_json::json!({ "exists": false }))),
            Err(e) => {
                eprintln!("Помилка при перевірці користувача: {}", e);
                Err(warp::reject::custom(CustomError("Database error".into())))
            }
        }
    } else {
        Ok(warp::reply::json(&serde_json::json!({
            "error": "Username is required"
        })))
    }
}
