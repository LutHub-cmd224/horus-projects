use axum::{
    Json, Router,
    extract::Request,
    http::{StatusCode, header},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{TokenType, decode_token, hash_password, issue_token, verify_password};

const DEV_JWT_SECRET: &[u8] = b"horus-development-secret-change-in-production";

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: &'static str,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub user_id: Uuid,
}

pub fn router() -> Router {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/refresh", post(refresh))
        .route("/logout", post(logout))
        .route("/me", get(me))
}

fn tokens(user_id: Uuid) -> Result<AuthResponse, StatusCode> {
    let access_token = issue_token(user_id, TokenType::Access, DEV_JWT_SECRET)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let refresh_token = issue_token(user_id, TokenType::Refresh, DEV_JWT_SECRET)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(AuthResponse {
        access_token,
        refresh_token,
        token_type: "Bearer",
    })
}

async fn register(
    Json(payload): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<AuthResponse>), StatusCode> {
    if !payload.email.contains('@') || payload.password.len() < 12 {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let hash = hash_password(&payload.password).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !verify_password(&payload.password, &hash) {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    let _ = (payload.email, payload.display_name);
    Ok((StatusCode::CREATED, Json(tokens(Uuid::now_v7())?)))
}

async fn login(Json(payload): Json<LoginRequest>) -> Result<Json<AuthResponse>, StatusCode> {
    if payload.email.is_empty() || payload.password.is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    Ok(Json(tokens(Uuid::now_v7())?))
}

async fn refresh(Json(payload): Json<RefreshRequest>) -> Result<Json<AuthResponse>, StatusCode> {
    let claims = decode_token(&payload.refresh_token, DEV_JWT_SECRET)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    if claims.token_type != TokenType::Refresh {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(Json(tokens(claims.sub)?))
}

async fn logout() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn me(request: Request) -> Result<Json<MeResponse>, StatusCode> {
    let value = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let claims = decode_token(value, DEV_JWT_SECRET).map_err(|_| StatusCode::UNAUTHORIZED)?;
    if claims.token_type != TokenType::Access {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(Json(MeResponse { user_id: claims.sub }))
}
