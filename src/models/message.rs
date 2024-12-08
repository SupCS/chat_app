use chrono::Utc;
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Message {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>, // MongoDB автоматично генерує _id
    pub sender: String,    // ID відправника
    pub receiver: String,  // ID отримувача
    pub content: String,   // Текст повідомлення
    pub timestamp: String, // Час відправлення
}

impl Message {
    pub fn new(sender: &str, receiver: &str, content: &str) -> Self {
        Message {
            id: None,
            sender: sender.to_string(),
            receiver: receiver.to_string(),
            content: content.to_string(),
            timestamp: Utc::now().to_rfc3339(),
        }
    }
}
