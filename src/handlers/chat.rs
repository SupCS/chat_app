use crate::db::message::{get_all_messages, save_message};
use crate::models::message::Message as ChatMessage;
use futures_util::{SinkExt, StreamExt};
use mongodb::Database;
use std::sync::Arc;
use tokio::sync::mpsc;
use warp::ws::{Message, WebSocket};
use warp::Error;
use warp::{http::StatusCode, Reply};

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
pub async fn chat_handler(ws: WebSocket, clients: Arc<Clients>, db: Arc<Database>) {
    let (mut ws_tx, mut ws_rx) = ws.split();

    let (client_tx, mut client_rx) = mpsc::unbounded_channel();

    // Додаємо нового клієнта
    clients.senders.lock().await.push(client_tx);

    let clients_clone = clients.clone();
    let db_clone = db.clone();

    // Завдання для читання повідомлень від клієнта
    tokio::spawn(async move {
        while let Some(result) = ws_rx.next().await {
            match result {
                Ok(message) => {
                    if let Ok(text) = message.to_str() {
                        println!("Отримано повідомлення: {}", text);

                        // Зберігаємо повідомлення в MongoDB
                        let chat_message = ChatMessage::new("anonymous", text); // "anonymous" може бути замінено на ID користувача
                        let collection = db_clone.collection::<ChatMessage>("messages");
                        if let Err(e) = save_message(&chat_message, &collection).await {
                            eprintln!("Помилка при збереженні повідомлення: {}", e);
                        }

                        clients_clone.broadcast(text.to_string()).await;
                    }
                }
                Err(e) => {
                    eprintln!("Помилка при отриманні повідомлення: {}", e);
                    break;
                }
            }
        }
    });

    // Завдання для відправки повідомлень клієнту
    tokio::spawn(async move {
        while let Some(message) = client_rx.recv().await {
            if let Err(e) = ws_tx.send(message).await {
                eprintln!("Помилка при відправці повідомлення: {}", e);
                break;
            }
        }
    });
}

// Маршрут для отримання історії повідомлень
pub async fn get_chat_history_handler(db: Arc<Database>) -> Result<impl Reply, warp::Rejection> {
    let collection = db.collection::<ChatMessage>("messages");
    match get_all_messages(&collection).await {
        Ok(messages) => Ok(warp::reply::json(&messages)), // Повертаємо JSON без статусу
        Err(_) => Ok(warp::reply::json(
            &serde_json::json!({ "error": "Failed to get chat history" }),
        )),
    }
}
