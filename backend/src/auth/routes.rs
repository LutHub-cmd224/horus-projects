use axum::{Json, Router, http::StatusCode, routing::{get, post}};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct RegisterRequest { pub email: String, pub password: String, pub display_name: Option<String> }
#[derive(Debug, Deserialize)]
pub struct LoginRequest { pub email: String, pub password: String }
#[derive(Debug, Deserialize)]
pub struct RefreshRequest { pub refresh_token: String }
#[derive(Debug, Serialize)]
pub struct MessageResponse { pub message: &'static str }

pub fn router() -> Router {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/refresh", post(refresh))
        .route("/logout", post(logout))
        .route("/me", get(me))
}

async fn register(Json(payload): Json<RegisterRequest>) -> Result<(StatusCode, Json<MessageResponse>), StatusCode> {
    if !payload.email.contains('@') || payload.password.len() < 12 { return Err(StatusCode::UNPROCESSABLE_ENTITY); }
    let _ = payload.display_name;
    Ok((StatusCode::CREATED, Json(MessageResponse { message: "registration endpoint ready" })))
}

async fn login(Json(payload): Json<LoginRequest>) -> Result<Json<MessageResponse>, StatusCode> {
    if payload.email.is_empty() || payload.password.is_empty() { return Err(StatusCode::UNPROCESSABLE_ENTITY); }
    Ok(Json(MessageResponse { message: "login endpoint ready" }))
}

async fn refresh(Json(payload): Json<RefreshRequest>) -> Result<Json<MessageResponse>, StatusCode> {
    if payload.refresh_token.is_empty() { return Err(StatusCode::UNAUTHORIZED); }
    Ok(Json(MessageResponse { message: "refresh endpoint ready" }))
}

async fn logout() -> StatusCode { StatusCode::NO_CONTENT }
async fn me() -> Result<Json<MessageResponse>, StatusCode> { Err(StatusCode::UNAUTHORIZED) }
