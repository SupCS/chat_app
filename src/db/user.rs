use crate::models::user::User;
use mongodb::{bson::doc, error::Error, Collection};

pub async fn save_user(user: &User, collection: &Collection<User>) -> Result<(), Error> {
    collection.insert_one(user, None).await?;
    Ok(())
}

pub async fn find_user_by_username(
    username: &str,
    collection: &Collection<User>,
) -> Result<Option<User>, Error> {
    let filter = doc! { "username": username };
    let user = collection.find_one(filter, None).await?;
    Ok(user)
}
