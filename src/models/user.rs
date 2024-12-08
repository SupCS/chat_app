use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>, // MongoDB автоматично генерує _id
    pub username: String,
    pub password: String, // зберігатимемо хеш пароля
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub created_at: Option<String>,
}

impl User {
    pub fn new(username: &str, hashed_password: &str) -> Self {
        User {
            id: None,
            username: username.to_string(),
            password: hashed_password.to_string(),
            created_at: Some(chrono::Utc::now().to_rfc3339()),
        }
    }
}
