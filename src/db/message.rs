use crate::models::message::Message;
use futures_util::stream::TryStreamExt;
use mongodb::{bson::doc, error::Error, Collection};

pub async fn save_message(
    message: &Message,
    collection: &Collection<Message>,
) -> Result<(), Error> {
    collection.insert_one(message, None).await?;
    Ok(())
}

pub async fn get_all_messages(collection: &Collection<Message>) -> Result<Vec<Message>, Error> {
    let mut cursor = collection.find(None, None).await?;
    let mut messages = Vec::new();

    while let Some(doc) = cursor.try_next().await? {
        messages.push(doc);
    }

    Ok(messages)
}
