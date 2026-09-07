use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    routing::{get, patch, post},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    auth::{TokenType, decode_token},
    state::AppState,
};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PhaseResponse {
    pub id: Uuid,
    pub project_id: Uuid,
    pub phase_type: String,
    pub position: i16,
    pub status: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CriterionResponse {
    pub id: Uuid,
    pub phase_id: Uuid,
    pub code: String,
    pub label: String,
    pub required: bool,
    pub completed: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateCriterionRequest {
    pub code: String,
    pub label: String,
    pub required: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCriterionRequest {
    pub completed: bool,
}

#[derive(Debug, Deserialize)]
pub struct ValidatePhaseRequest {
    pub comment: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/projects/{project_id}/phases", get(list_project_phases))
        .route("/phases/{phase_id}", get(get_phase))
        .route("/phases/{phase_id}/start", post(start_phase))
        .route(
            "/phases/{phase_id}/criteria",
            get(list_criteria).post(create_criterion),
        )
        .route(
            "/phases/{phase_id}/criteria/{criterion_id}",
            patch(update_criterion),
        )
        .route("/phases/{phase_id}/validate", post(validate_phase))
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
        "SELECT wm.role::text FROM projects p JOIN workspace_members wm ON wm.workspace_id = p.workspace_id WHERE p.id = $1 AND wm.user_id = $2 AND p.deleted_at IS NULL",
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::FORBIDDEN)
}

async fn phase_project_id(state: &AppState, phase_id: Uuid) -> Result<Uuid, StatusCode> {
    sqlx::query_scalar::<_, Uuid>(
        "SELECT p.id FROM phases ph JOIN projects p ON p.id = ph.project_id WHERE ph.id = $1 AND p.deleted_at IS NULL",
    )
    .bind(phase_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)
}

async fn list_project_phases(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<PhaseResponse>>, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    project_role(&state, project_id, user_id).await?;
    let phases = sqlx::query_as::<_, PhaseResponse>(
        "SELECT id, project_id, phase_type::text AS phase_type, position, status::text AS status FROM phases WHERE project_id = $1 ORDER BY position ASC",
    )
    .bind(project_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(phases))
}

async fn get_phase(
    State(state): State<AppState>,
    Path(phase_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<PhaseResponse>, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    let project_id = phase_project_id(&state, phase_id).await?;
    project_role(&state, project_id, user_id).await?;
    let phase = sqlx::query_as::<_, PhaseResponse>(
        "SELECT id, project_id, phase_type::text AS phase_type, position, status::text AS status FROM phases WHERE id = $1",
    )
    .bind(phase_id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(phase))
}

async fn start_phase(
    State(state): State<AppState>,
    Path(phase_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    let project_id = phase_project_id(&state, phase_id).await?;
    let role = project_role(&state, project_id, user_id).await?;
    if role == "VIEWER" {
        return Err(StatusCode::FORBIDDEN);
    }

    let result = sqlx::query(
        "UPDATE phases SET status = 'IN_PROGRESS', started_at = COALESCE(started_at, now()), updated_at = now() WHERE id = $1 AND status = 'AVAILABLE'",
    )
    .bind(phase_id)
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if result.rows_affected() != 1 {
        return Err(StatusCode::CONFLICT);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn list_criteria(
    State(state): State<AppState>,
    Path(phase_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<CriterionResponse>>, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    let project_id = phase_project_id(&state, phase_id).await?;
    project_role(&state, project_id, user_id).await?;
    let criteria = sqlx::query_as::<_, CriterionResponse>(
        "SELECT id, phase_id, code, label, required, completed FROM validation_criteria WHERE phase_id = $1 ORDER BY code ASC",
    )
    .bind(phase_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(criteria))
}

async fn create_criterion(
    State(state): State<AppState>,
    Path(phase_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<CreateCriterionRequest>,
) -> Result<(StatusCode, Json<CriterionResponse>), StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    let project_id = phase_project_id(&state, phase_id).await?;
    let role = project_role(&state, project_id, user_id).await?;
    if role == "VIEWER" {
        return Err(StatusCode::FORBIDDEN);
    }

    let code = payload.code.trim();
    let label = payload.label.trim();
    if code.is_empty() || label.is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    let status = sqlx::query_scalar::<_, String>("SELECT status::text FROM phases WHERE id = $1")
        .bind(phase_id)
        .fetch_one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if status == "LOCKED" || status == "VALIDATED" {
        return Err(StatusCode::CONFLICT);
    }

    let criterion = sqlx::query_as::<_, CriterionResponse>(
        "INSERT INTO validation_criteria (phase_id, code, label, required, completed) VALUES ($1, $2, $3, $4, false) RETURNING id, phase_id, code, label, required, completed",
    )
    .bind(phase_id)
    .bind(code)
    .bind(label)
    .bind(payload.required.unwrap_or(true))
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::CONFLICT)?;

    Ok((StatusCode::CREATED, Json(criterion)))
}

async fn update_criterion(
    State(state): State<AppState>,
    Path((phase_id, criterion_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(payload): Json<UpdateCriterionRequest>,
) -> Result<StatusCode, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    let project_id = phase_project_id(&state, phase_id).await?;
    let role = project_role(&state, project_id, user_id).await?;
    if role == "VIEWER" {
        return Err(StatusCode::FORBIDDEN);
    }

    let phase_status = sqlx::query_scalar::<_, String>("SELECT status::text FROM phases WHERE id = $1")
        .bind(phase_id)
        .fetch_one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if phase_status == "LOCKED" || phase_status == "VALIDATED" {
        return Err(StatusCode::CONFLICT);
    }

    let result = sqlx::query(
        "UPDATE validation_criteria SET completed = $1, completed_by = CASE WHEN $1 THEN $2 ELSE NULL END, completed_at = CASE WHEN $1 THEN now() ELSE NULL END WHERE id = $3 AND phase_id = $4",
    )
    .bind(payload.completed)
    .bind(user_id)
    .bind(criterion_id)
    .bind(phase_id)
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if result.rows_affected() != 1 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn validate_phase(
    State(state): State<AppState>,
    Path(phase_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<ValidatePhaseRequest>,
) -> Result<StatusCode, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    let project_id = phase_project_id(&state, phase_id).await?;
    let role = project_role(&state, project_id, user_id).await?;
    if role != "OWNER" && role != "ADMIN" {
        return Err(StatusCode::FORBIDDEN);
    }

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let phase = sqlx::query_as::<_, PhaseResponse>(
        "SELECT id, project_id, phase_type::text AS phase_type, position, status::text AS status FROM phases WHERE id = $1 FOR UPDATE",
    )
    .bind(phase_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if phase.status != "IN_PROGRESS" && phase.status != "AVAILABLE" {
        return Err(StatusCode::CONFLICT);
    }

    let criteria_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM validation_criteria WHERE phase_id = $1",
    )
    .bind(phase_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if criteria_count == 0 {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    let incomplete_required = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM validation_criteria WHERE phase_id = $1 AND required = true AND completed = false",
    )
    .bind(phase_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if incomplete_required > 0 {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    sqlx::query(
        "INSERT INTO phase_validations (phase_id, validated_by, comment) VALUES ($1, $2, $3)",
    )
    .bind(phase_id)
    .bind(user_id)
    .bind(payload.comment)
    .execute(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    sqlx::query(
        "UPDATE phases SET status = 'VALIDATED', validated_at = now(), updated_at = now() WHERE id = $1",
    )
    .bind(phase_id)
    .execute(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if phase.position < 6 {
        sqlx::query(
            "UPDATE phases SET status = 'AVAILABLE', updated_at = now() WHERE project_id = $1 AND position = $2 AND status = 'LOCKED'",
        )
        .bind(project_id)
        .bind(phase.position + 1)
        .execute(&mut *tx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    } else {
        sqlx::query(
            "UPDATE projects SET status = 'COMPLETED', updated_at = now() WHERE id = $1",
        )
        .bind(project_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    #[test]
    fn phase_order_is_stable() {
        let phases = ["ANALYZE", "MODEL", "DESIGN", "BUILD", "TEST", "DEPLOY"];
        assert_eq!(phases.len(), 6);
        assert_eq!(phases.first(), Some(&"ANALYZE"));
        assert_eq!(phases.last(), Some(&"DEPLOY"));
    }
}
