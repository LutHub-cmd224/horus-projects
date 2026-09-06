pub mod routes;

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::{SaltString, rand_core::OsRng}};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims { pub sub: Uuid, pub exp: usize, pub iat: usize, pub token_type: TokenType }

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TokenType { Access, Refresh }

pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default().hash_password(password.as_bytes(), &salt)?.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash).ok().is_some_and(|parsed| Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
}

pub fn issue_token(user_id: Uuid, token_type: TokenType, secret: &[u8]) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let ttl = if token_type == TokenType::Access { Duration::minutes(15) } else { Duration::days(30) };
    let claims = Claims { sub: user_id, iat: now.timestamp() as usize, exp: (now + ttl).timestamp() as usize, token_type };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret))
}

pub fn decode_token(token: &str, secret: &[u8]) -> Result<Claims, jsonwebtoken::errors::Error> {
    Ok(decode::<Claims>(token, &DecodingKey::from_secret(secret), &Validation::default())?.claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn password_round_trip() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password("correct horse battery staple", &hash));
        assert!(!verify_password("wrong", &hash));
    }
    #[test]
    fn access_token_round_trip() {
        let id = Uuid::now_v7();
        let token = issue_token(id, TokenType::Access, b"test-secret").unwrap();
        let claims = decode_token(&token, b"test-secret").unwrap();
        assert_eq!(claims.sub, id);
        assert_eq!(claims.token_type, TokenType::Access);
    }
}
