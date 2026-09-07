use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    auth::{TokenType, decode_token},
    state::AppState,
};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DeliverableResponse {
    pub id: Uuid,
    pub phase_id: Uuid,
    pub title: String,
    pub deliverable_type: String,
    pub content: Value,
    pub version: i32,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateDeliverableRequest {
    pub title: String,
    pub deliverable_type: String,
    pub content: Option<Value>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDeliverableRequest {
    pub title: Option<String>,
    pub deliverable_type: Option<String>,
    pub content: Option<Value>,
    pub status: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/phases/{phase_id}/deliverables",
            get(list_deliverables).post(create_deliverable),
        )
        .route(
            "/deliverables/{deliverable_id}",
            get(get_deliverable)
                .patch(update_deliverable)
                .delete(delete_deliverable),
        )
}

fn authenticated_user(headers: &HeaderMap, state: &AppState) -> Result<Uuid, StatusCode> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let claims = decode_token(token, &state.jwt_secret).map_err(|_| StatusCode::UNAUTHORIZED)?;
    if claims.token_type != TokenType::Access {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(claims.sub)
}

async fn phase_access(
    state: &AppState,
    phase_id: Uuid,
    user_id: Uuid,
) -> Result<(Uuid, String, String), StatusCode> {
    sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT p.id, wm.role::text, ph.status::text FROM phases ph JOIN projects p ON p.id = ph.project_id JOIN workspace_members wm ON wm.workspace_id = p.workspace_id WHERE ph.id = $1 AND wm.user_id = $2 AND p.deleted_at IS NULL",
    )
    .bind(phase_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::FORBIDDEN)
}

async fn deliverable_access(
    state: &AppState,
    deliverable_id: Uuid,
    user_id: Uuid,
) -> Result<(Uuid, String, String), StatusCode> {
    sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT d.phase_id, wm.role::text, ph.status::text FROM deliverables d JOIN phases ph ON ph.id = d.phase_id JOIN projects p ON p.id = ph.project_id JOIN workspace_members wm ON wm.workspace_id = p.workspace_id WHERE d.id = $1 AND d.deleted_at IS NULL AND p.deleted_at IS NULL AND wm.user_id = $2",
    )
    .bind(deliverable_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)
}

fn ensure_writable(role: &str, phase_status: &str) -> Result<(), StatusCode> {
    if role == "VIEWER" {
        return Err(StatusCode::FORBIDDEN);
    }
    if phase_status == "LOCKED" || phase_status == "VALIDATED" {
        return Err(StatusCode::CONFLICT);
    }
    Ok(())
}

fn normalize_status(status: Option<String>) -> Result<String, StatusCode> {
    let status = status.unwrap_or_else(|| "DRAFT".to_string()).to_uppercase();
    if !matches!(status.as_str(), "DRAFT" | "READY" | "VALIDATED") {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    Ok(status)
}

async fn list_deliverables(
    State(state): State<AppState>,
    Path(phase_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<DeliverableResponse>>, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    phase_access(&state, phase_id, user_id).await?;

    let deliverables = sqlx::query_as::<_, DeliverableResponse>(
        "SELECT id, phase_id, title, type AS deliverable_type, content, version, status::text AS status FROM deliverables WHERE phase_id = $1 AND deleted_at IS NULL ORDER BY created_at ASC",
    )
    .bind(phase_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(deliverables))
}

async fn get_deliverable(
    State(state): State<AppState>,
    Path(deliverable_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<DeliverableResponse>, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    deliverable_access(&state, deliverable_id, user_id).await?;

    let deliverable = sqlx::query_as::<_, DeliverableResponse>(
        "SELECT id, phase_id, title, type AS deliverable_type, content, version, status::text AS status FROM deliverables WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(deliverable_id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(deliverable))
}

async fn create_deliverable(
    State(state): State<AppState>,
    Path(phase_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<CreateDeliverableRequest>,
) -> Result<(StatusCode, Json<DeliverableResponse>), StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    let (_, role, phase_status) = phase_access(&state, phase_id, user_id).await?;
    ensure_writable(&role, &phase_status)?;

    let title = payload.title.trim();
    let deliverable_type = payload.deliverable_type.trim();
    if title.is_empty() || deliverable_type.is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let status = normalize_status(payload.status)?;

    let deliverable = sqlx::query_as::<_, DeliverableResponse>(
        "INSERT INTO deliverables (phase_id, title, type, content, status, created_by) VALUES ($1, $2, $3, $4, $5::deliverable_status, $6) RETURNING id, phase_id, title, type AS deliverable_type, content, version, status::text AS status",
    )
    .bind(phase_id)
    .bind(title)
    .bind(deliverable_type)
    .bind(payload.content.unwrap_or_else(|| serde_json::json!({})))
    .bind(status)
    .bind(user_id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::CREATED, Json(deliverable)))
}

async fn update_deliverable(
    State(state): State<AppState>,
    Path(deliverable_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<UpdateDeliverableRequest>,
) -> Result<Json<DeliverableResponse>, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    let (_, role, phase_status) = deliverable_access(&state, deliverable_id, user_id).await?;
    ensure_writable(&role, &phase_status)?;

    let status = match payload.status {
        Some(value) => Some(normalize_status(Some(value))?),
        None => None,
    };
    if payload
        .title
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
        || payload
            .deliverable_type
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
    {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    let deliverable = sqlx::query_as::<_, DeliverableResponse>(
        "UPDATE deliverables SET title = COALESCE($2, title), type = COALESCE($3, type), content = COALESCE($4, content), status = COALESCE($5::deliverable_status, status), version = version + 1, updated_at = now() WHERE id = $1 AND deleted_at IS NULL RETURNING id, phase_id, title, type AS deliverable_type, content, version, status::text AS status",
    )
    .bind(deliverable_id)
    .bind(payload.title.map(|value| value.trim().to_string()))
    .bind(payload.deliverable_type.map(|value| value.trim().to_string()))
    .bind(payload.content)
    .bind(status)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(deliverable))
}

async fn delete_deliverable(
    State(state): State<AppState>,
    Path(deliverable_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    let (_, role, phase_status) = deliverable_access(&state, deliverable_id, user_id).await?;
    ensure_writable(&role, &phase_status)?;

    let result = sqlx::query(
        "UPDATE deliverables SET deleted_at = now(), updated_at = now() WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(deliverable_id)
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if result.rows_affected() != 1 {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::normalize_status;

    #[test]
    fn deliverable_status_is_validated() {
        assert_eq!(normalize_status(None).unwrap(), "DRAFT");
        assert_eq!(normalize_status(Some("ready".into())).unwrap(), "READY");
        assert!(normalize_status(Some("unknown".into())).is_err());
    }
}
