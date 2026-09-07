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
pub struct TaskResponse {
    pub id: Uuid,
    pub project_id: Uuid,
    pub requirement_id: Option<Uuid>,
    pub phase_id: Option<Uuid>,
    pub code: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: String,
    pub assigned_to: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub requirement_id: Option<Uuid>,
    pub phase_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub assigned_to: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTaskRequest {
    pub requirement_id: Option<Uuid>,
    pub phase_id: Option<Uuid>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub assigned_to: Option<Uuid>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/projects/{project_id}/tasks",
            get(list_tasks).post(create_task),
        )
        .route(
            "/tasks/{task_id}",
            get(get_task).patch(update_task).delete(delete_task),
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

async fn task_access(
    state: &AppState,
    task_id: Uuid,
    user_id: Uuid,
) -> Result<(Uuid, String), StatusCode> {
    sqlx::query_as::<_, (Uuid, String)>(
        "SELECT t.project_id, wm.role::text FROM tasks t JOIN projects p ON p.id = t.project_id JOIN workspace_members wm ON wm.workspace_id = p.workspace_id WHERE t.id = $1 AND t.deleted_at IS NULL AND p.deleted_at IS NULL AND wm.user_id = $2",
    )
    .bind(task_id)
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

fn normalize_priority(value: Option<String>) -> Result<String, StatusCode> {
    let value = value
        .unwrap_or_else(|| "MEDIUM".into())
        .trim()
        .to_uppercase();
    if matches!(value.as_str(), "LOW" | "MEDIUM" | "HIGH" | "CRITICAL") {
        Ok(value)
    } else {
        Err(StatusCode::UNPROCESSABLE_ENTITY)
    }
}

fn normalize_status(value: Option<String>) -> Result<String, StatusCode> {
    let value = value
        .unwrap_or_else(|| "TODO".into())
        .trim()
        .to_uppercase();
    if matches!(
        value.as_str(),
        "TODO" | "IN_PROGRESS" | "BLOCKED" | "DONE" | "CANCELLED"
    ) {
        Ok(value)
    } else {
        Err(StatusCode::UNPROCESSABLE_ENTITY)
    }
}

async fn validate_links(
    state: &AppState,
    project_id: Uuid,
    requirement_id: Option<Uuid>,
    phase_id: Option<Uuid>,
    assigned_to: Option<Uuid>,
) -> Result<(), StatusCode> {
    if let Some(requirement_id) = requirement_id {
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM requirements WHERE id = $1 AND project_id = $2 AND deleted_at IS NULL)",
        )
        .bind(requirement_id)
        .bind(project_id)
        .fetch_one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if !exists {
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
    }

    if let Some(phase_id) = phase_id {
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM phases WHERE id = $1 AND project_id = $2)",
        )
        .bind(phase_id)
        .bind(project_id)
        .fetch_one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if !exists {
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
    }

    if let Some(assigned_to) = assigned_to {
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM projects p JOIN workspace_members wm ON wm.workspace_id = p.workspace_id WHERE p.id = $1 AND wm.user_id = $2)",
        )
        .bind(project_id)
        .bind(assigned_to)
        .fetch_one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if !exists {
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
    }

    Ok(())
}

async fn list_tasks(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<TaskResponse>>, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    project_role(&state, project_id, user_id).await?;
    let tasks = sqlx::query_as::<_, TaskResponse>(
        "SELECT id, project_id, requirement_id, phase_id, code, title, description, status::text AS status, priority::text AS priority, assigned_to FROM tasks WHERE project_id = $1 AND deleted_at IS NULL ORDER BY created_at ASC",
    )
    .bind(project_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(tasks))
}

async fn get_task(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<TaskResponse>, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    task_access(&state, task_id, user_id).await?;
    let task = sqlx::query_as::<_, TaskResponse>(
        "SELECT id, project_id, requirement_id, phase_id, code, title, description, status::text AS status, priority::text AS priority, assigned_to FROM tasks WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(task_id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(task))
}

async fn create_task(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<TaskResponse>), StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    let role = project_role(&state, project_id, user_id).await?;
    ensure_writable(&role)?;

    let title = payload.title.trim();
    if title.is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let priority = normalize_priority(payload.priority)?;
    let status = normalize_status(payload.status)?;
    validate_links(
        &state,
        project_id,
        payload.requirement_id,
        payload.phase_id,
        payload.assigned_to,
    )
    .await?;

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM projects WHERE id = $1 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let current = sqlx::query_scalar::<_, i32>(
        "SELECT COALESCE(MAX(CASE WHEN code ~ '^TASK-[0-9]+$' THEN substring(code FROM 6)::int END), 0) FROM tasks WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let code = format!("TASK-{:03}", current + 1);

    let task = sqlx::query_as::<_, TaskResponse>(
        "INSERT INTO tasks (project_id, requirement_id, phase_id, code, title, description, status, priority, assigned_to, created_by) VALUES ($1, $2, $3, $4, $5, $6, $7::task_status, $8::priority_level, $9, $10) RETURNING id, project_id, requirement_id, phase_id, code, title, description, status::text AS status, priority::text AS priority, assigned_to",
    )
    .bind(project_id)
    .bind(payload.requirement_id)
    .bind(payload.phase_id)
    .bind(code)
    .bind(title)
    .bind(payload.description)
    .bind(status)
    .bind(priority)
    .bind(payload.assigned_to)
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((StatusCode::CREATED, Json(task)))
}

async fn update_task(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<UpdateTaskRequest>,
) -> Result<Json<TaskResponse>, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    let (project_id, role) = task_access(&state, task_id, user_id).await?;
    ensure_writable(&role)?;

    if payload
        .title
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let priority = match payload.priority {
        Some(value) => Some(normalize_priority(Some(value))?),
        None => None,
    };
    let status = match payload.status {
        Some(value) => Some(normalize_status(Some(value))?),
        None => None,
    };
    validate_links(
        &state,
        project_id,
        payload.requirement_id,
        payload.phase_id,
        payload.assigned_to,
    )
    .await?;

    let task = sqlx::query_as::<_, TaskResponse>(
        "UPDATE tasks SET requirement_id = COALESCE($2, requirement_id), phase_id = COALESCE($3, phase_id), title = COALESCE($4, title), description = COALESCE($5, description), status = COALESCE($6::task_status, status), priority = COALESCE($7::priority_level, priority), assigned_to = COALESCE($8, assigned_to), updated_at = now() WHERE id = $1 AND deleted_at IS NULL RETURNING id, project_id, requirement_id, phase_id, code, title, description, status::text AS status, priority::text AS priority, assigned_to",
    )
    .bind(task_id)
    .bind(payload.requirement_id)
    .bind(payload.phase_id)
    .bind(payload.title.map(|value| value.trim().to_string()))
    .bind(payload.description)
    .bind(status)
    .bind(priority)
    .bind(payload.assigned_to)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(task))
}

async fn delete_task(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    let (_, role) = task_access(&state, task_id, user_id).await?;
    ensure_writable(&role)?;
    let result = sqlx::query(
        "UPDATE tasks SET deleted_at = now(), updated_at = now() WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(task_id)
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
    use super::{normalize_priority, normalize_status};

    #[test]
    fn task_enums_are_validated() {
        assert_eq!(normalize_priority(None).unwrap(), "MEDIUM");
        assert_eq!(normalize_status(None).unwrap(), "TODO");
        assert!(normalize_priority(Some("urgent".into())).is_err());
        assert!(normalize_status(Some("unknown".into())).is_err());
    }
}
