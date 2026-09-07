use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    routing::get,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    auth::{TokenType, decode_token},
    state::AppState,
};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct RequirementResponse {
    pub id: Uuid,
    pub project_id: Uuid,
    pub code: String,
    pub title: String,
    pub description: Option<String>,
    pub requirement_type: String,
    pub priority: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateRequirementRequest {
    pub code: String,
    pub title: String,
    pub description: Option<String>,
    pub requirement_type: String,
    pub priority: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRequirementRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub requirement_type: Option<String>,
    pub priority: Option<String>,
    pub status: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/projects/{project_id}/requirements",
            get(list_requirements).post(create_requirement),
        )
        .route(
            "/requirements/{requirement_id}",
            get(get_requirement)
                .patch(update_requirement)
                .delete(delete_requirement),
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

async fn project_role(
    state: &AppState,
    project_id: Uuid,
    user_id: Uuid,
) -> Result<String, StatusCode> {
    sqlx::query_scalar::<_, String>(
        "SELECT wm.role::text FROM projects p JOIN workspace_members wm ON wm.workspace_id = p.workspace_id WHERE p.id = $1 AND p.deleted_at IS NULL AND wm.user_id = $2",
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::FORBIDDEN)
}

async fn requirement_access(
    state: &AppState,
    requirement_id: Uuid,
    user_id: Uuid,
) -> Result<(Uuid, String), StatusCode> {
    sqlx::query_as::<_, (Uuid, String)>(
        "SELECT r.project_id, wm.role::text FROM requirements r JOIN projects p ON p.id = r.project_id JOIN workspace_members wm ON wm.workspace_id = p.workspace_id WHERE r.id = $1 AND r.deleted_at IS NULL AND p.deleted_at IS NULL AND wm.user_id = $2",
    )
    .bind(requirement_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)
}

fn ensure_writable(role: &str) -> Result<(), StatusCode> {
    if role == "VIEWER" {
        Err(StatusCode::FORBIDDEN)
    } else {
        Ok(())
    }
}

fn normalize_type(value: String) -> Result<String, StatusCode> {
    let value = value.trim().to_uppercase();
    if matches!(
        value.as_str(),
        "FUNCTIONAL" | "TECHNICAL" | "SECURITY" | "PERFORMANCE"
    ) {
        Ok(value)
    } else {
        Err(StatusCode::UNPROCESSABLE_ENTITY)
    }
}

fn normalize_priority(value: Option<String>) -> Result<String, StatusCode> {
    let value = value.unwrap_or_else(|| "MEDIUM".into()).trim().to_uppercase();
    if matches!(value.as_str(), "LOW" | "MEDIUM" | "HIGH" | "CRITICAL") {
        Ok(value)
    } else {
        Err(StatusCode::UNPROCESSABLE_ENTITY)
    }
}

fn normalize_status(value: Option<String>) -> Result<String, StatusCode> {
    let value = value.unwrap_or_else(|| "DRAFT".into()).trim().to_uppercase();
    if matches!(
        value.as_str(),
        "DRAFT" | "APPROVED" | "IN_PROGRESS" | "DONE" | "REJECTED"
    ) {
        Ok(value)
    } else {
        Err(StatusCode::UNPROCESSABLE_ENTITY)
    }
}

async fn list_requirements(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<RequirementResponse>>, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    project_role(&state, project_id, user_id).await?;
    let requirements = sqlx::query_as::<_, RequirementResponse>(
        "SELECT id, project_id, code, title, description, type::text AS requirement_type, priority::text AS priority, status::text AS status FROM requirements WHERE project_id = $1 AND deleted_at IS NULL ORDER BY created_at ASC",
    )
    .bind(project_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(requirements))
}

async fn get_requirement(
    State(state): State<AppState>,
    Path(requirement_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<RequirementResponse>, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    requirement_access(&state, requirement_id, user_id).await?;
    let requirement = sqlx::query_as::<_, RequirementResponse>(
        "SELECT id, project_id, code, title, description, type::text AS requirement_type, priority::text AS priority, status::text AS status FROM requirements WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(requirement_id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(requirement))
}

async fn create_requirement(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<CreateRequirementRequest>,
) -> Result<(StatusCode, Json<RequirementResponse>), StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    let role = project_role(&state, project_id, user_id).await?;
    ensure_writable(&role)?;

    let code = payload.code.trim().to_uppercase();
    let title = payload.title.trim();
    if code.is_empty() || title.is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let requirement_type = normalize_type(payload.requirement_type)?;
    let priority = normalize_priority(payload.priority)?;
    let status = normalize_status(payload.status)?;

    let requirement = sqlx::query_as::<_, RequirementResponse>(
        "INSERT INTO requirements (project_id, code, title, description, type, priority, status, created_by) VALUES ($1, $2, $3, $4, $5::requirement_type, $6::priority_level, $7::requirement_status, $8) RETURNING id, project_id, code, title, description, type::text AS requirement_type, priority::text AS priority, status::text AS status",
    )
    .bind(project_id)
    .bind(code)
    .bind(title)
    .bind(payload.description)
    .bind(requirement_type)
    .bind(priority)
    .bind(status)
    .bind(user_id)
    .fetch_one(&state.db)
    .await
    .map_err(|error| {
        if error.as_database_error().is_some_and(|db| db.is_unique_violation()) {
            StatusCode::CONFLICT
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    })?;
    Ok((StatusCode::CREATED, Json(requirement)))
}

async fn update_requirement(
    State(state): State<AppState>,
    Path(requirement_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<UpdateRequirementRequest>,
) -> Result<Json<RequirementResponse>, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    let (_, role) = requirement_access(&state, requirement_id, user_id).await?;
    ensure_writable(&role)?;

    if payload.title.as_deref().is_some_and(|value| value.trim().is_empty()) {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let requirement_type = match payload.requirement_type {
        Some(value) => Some(normalize_type(value)?),
        None => None,
    };
    let priority = match payload.priority {
        Some(value) => Some(normalize_priority(Some(value))?),
        None => None,
    };
    let status = match payload.status {
        Some(value) => Some(normalize_status(Some(value))?),
        None => None,
    };

    let requirement = sqlx::query_as::<_, RequirementResponse>(
        "UPDATE requirements SET title = COALESCE($2, title), description = COALESCE($3, description), type = COALESCE($4::requirement_type, type), priority = COALESCE($5::priority_level, priority), status = COALESCE($6::requirement_status, status), updated_at = now() WHERE id = $1 AND deleted_at IS NULL RETURNING id, project_id, code, title, description, type::text AS requirement_type, priority::text AS priority, status::text AS status",
    )
    .bind(requirement_id)
    .bind(payload.title.map(|value| value.trim().to_string()))
    .bind(payload.description)
    .bind(requirement_type)
    .bind(priority)
    .bind(status)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(requirement))
}

async fn delete_requirement(
    State(state): State<AppState>,
    Path(requirement_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    let (_, role) = requirement_access(&state, requirement_id, user_id).await?;
    ensure_writable(&role)?;
    let result = sqlx::query(
        "UPDATE requirements SET deleted_at = now(), updated_at = now() WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(requirement_id)
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
    use super::{normalize_priority, normalize_status, normalize_type};

    #[test]
    fn requirement_enums_are_validated() {
        assert_eq!(normalize_type("functional".into()).unwrap(), "FUNCTIONAL");
        assert_eq!(normalize_priority(None).unwrap(), "MEDIUM");
        assert_eq!(normalize_status(None).unwrap(), "DRAFT");
        assert!(normalize_type("other".into()).is_err());
        assert!(normalize_priority(Some("urgent".into())).is_err());
        assert!(normalize_status(Some("unknown".into())).is_err());
    }
}
