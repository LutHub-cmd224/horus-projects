use axum::{
    Json, Router,
    extract::{Request, State},
    http::{StatusCode, header},
    routing::{get, post},
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{TokenType, decode_token, hash_password, issue_token, verify_password};
use crate::state::AppState;

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

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct MeResponse {
    pub id: Uuid,
    pub email: String,
    pub display_name: Option<String>,
}

#[derive(sqlx::FromRow)]
struct UserAuthRow {
    id: Uuid,
    password_hash: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/refresh", post(refresh))
        .route("/logout", post(logout))
        .route("/me", get(me))
}

fn token_hash(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

async fn issue_and_store(state: &AppState, user_id: Uuid) -> Result<AuthResponse, StatusCode> {
    let access_token = issue_token(user_id, TokenType::Access, &state.jwt_secret)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let refresh_token = issue_token(user_id, TokenType::Refresh, &state.jwt_secret)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("INSERT INTO auth_sessions (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(token_hash(&refresh_token))
        .bind(Utc::now() + Duration::days(30))
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(AuthResponse { access_token, refresh_token, token_type: "Bearer" })
}

async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<AuthResponse>), StatusCode> {
    let email = payload.email.trim().to_lowercase();
    if !email.contains('@') || payload.password.len() < 12 {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let password_hash = hash_password(&payload.password).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let user_id = Uuid::now_v7();
    let workspace_id = Uuid::now_v7();
    let mut tx = state.db.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let inserted = sqlx::query("INSERT INTO users (id, email, password_hash, display_name) VALUES ($1, $2, $3, $4)")
        .bind(user_id).bind(&email).bind(password_hash).bind(&payload.display_name)
        .execute(&mut *tx).await;
    if inserted.is_err() { return Err(StatusCode::CONFLICT); }
    sqlx::query("INSERT INTO workspaces (id, name, slug, created_by) VALUES ($1, $2, $3, $4)")
        .bind(workspace_id).bind("Personal Workspace").bind(format!("personal-{workspace_id}")) .bind(user_id)
        .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, 'OWNER')")
        .bind(workspace_id).bind(user_id).execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((StatusCode::CREATED, Json(issue_and_store(&state, user_id).await?)))
}

async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, StatusCode> {
    let email = payload.email.trim().to_lowercase();
    let user = sqlx::query_as::<_, UserAuthRow>("SELECT id, password_hash FROM users WHERE email = $1")
        .bind(email).fetch_optional(&state.db).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if !verify_password(&payload.password, &user.password_hash) { return Err(StatusCode::UNAUTHORIZED); }
    Ok(Json(issue_and_store(&state, user.id).await?))
}

async fn refresh(
    State(state): State<AppState>,
    Json(payload): Json<RefreshRequest>,
) -> Result<Json<AuthResponse>, StatusCode> {
    let claims = decode_token(&payload.refresh_token, &state.jwt_secret).map_err(|_| StatusCode::UNAUTHORIZED)?;
    if claims.token_type != TokenType::Refresh { return Err(StatusCode::UNAUTHORIZED); }
    let result = sqlx::query("UPDATE auth_sessions SET revoked_at = now() WHERE user_id = $1 AND token_hash = $2 AND revoked_at IS NULL AND expires_at > now()")
        .bind(claims.sub).bind(token_hash(&payload.refresh_token)).execute(&state.db).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if result.rows_affected() != 1 { return Err(StatusCode::UNAUTHORIZED); }
    Ok(Json(issue_and_store(&state, claims.sub).await?))
}

async fn logout(
    State(state): State<AppState>,
    Json(payload): Json<RefreshRequest>,
) -> Result<StatusCode, StatusCode> {
    sqlx::query("UPDATE auth_sessions SET revoked_at = now() WHERE token_hash = $1 AND revoked_at IS NULL")
        .bind(token_hash(&payload.refresh_token)).execute(&state.db).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn me(
    State(state): State<AppState>,
    request: Request,
) -> Result<Json<MeResponse>, StatusCode> {
    let token = request.headers().get(header::AUTHORIZATION).and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer ")).ok_or(StatusCode::UNAUTHORIZED)?;
    let claims = decode_token(token, &state.jwt_secret).map_err(|_| StatusCode::UNAUTHORIZED)?;
    if claims.token_type != TokenType::Access { return Err(StatusCode::UNAUTHORIZED); }
    let user = sqlx::query_as::<_, MeResponse>("SELECT id, email, display_name FROM users WHERE id = $1")
        .bind(claims.sub).fetch_optional(&state.db).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    Ok(Json(user))
}
