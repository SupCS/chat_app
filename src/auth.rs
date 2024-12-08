use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use password_hash::SaltString;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Serialize, Deserialize, Debug)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

pub fn hash_password(password: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .expect("Failed to hash password");
    password_hash.to_string()
}

pub fn verify_password(hash: &str, password: &str) -> bool {
    let parsed_hash = PasswordHash::new(hash).expect("Failed to parse hash");
    let argon2 = Argon2::default();
    argon2
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

pub fn generate_jwt(user_id: &str) -> String {
    let secret = env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    let claims = Claims {
        sub: user_id.to_string(),
        exp: (Utc::now() + chrono::Duration::days(1)).timestamp() as usize,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )
    .expect("Failed to generate JWT")
}

pub fn validate_jwt(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    // Розділення токена, якщо він містить "Bearer "
    let token = token.strip_prefix("Bearer ").unwrap_or(token);

    println!("Валідація токена: {}", token);

    let secret = match env::var("JWT_SECRET") {
        Ok(secret) => {
            println!("Знайдений секретний ключ: {}", secret);
            secret
        }
        Err(_) => {
            println!("Секретний ключ не знайдений у змінних оточення");
            return Err(jsonwebtoken::errors::Error::from(
                jsonwebtoken::errors::ErrorKind::MissingRequiredClaim("JWT_SECRET".to_string()),
            ));
        }
    };

    let validation = Validation::default();

    // Спроба розпарсити токен
    match decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &validation,
    ) {
        Ok(token_data) => {
            println!("Успішно валідовано токен: {:?}", token_data.claims);
            Ok(token_data.claims)
        }
        Err(e) => {
            println!("Помилка валідації токена: {:?}", e);
            Err(e)
        }
    }
}
