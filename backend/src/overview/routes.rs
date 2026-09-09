use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    routing::get,
};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    auth::{TokenType, decode_token},
    state::AppState,
};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct OverviewProject {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct OverviewPhase {
    pub id: Uuid,
    pub phase_type: String,
    pub position: i16,
    pub status: String,
    pub required_criteria: i64,
    pub completed_required_criteria: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LatestDecision {
    pub code: String,
    pub title: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct ProjectOverviewResponse {
    pub project: OverviewProject,
    pub phases: Vec<OverviewPhase>,
    pub requirement_count: i64,
    pub open_task_count: i64,
    pub decision_count: i64,
    pub latest_decision: Option<LatestDecision>,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/projects/{project_id}/overview", get(project_overview))
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

async fn project_overview(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ProjectOverviewResponse>, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;

    let project = sqlx::query_as::<_, OverviewProject>(
        "SELECT p.id, p.name, p.description, p.status::text AS status FROM projects p JOIN workspace_members wm ON wm.workspace_id = p.workspace_id WHERE p.id = $1 AND wm.user_id = $2 AND p.deleted_at IS NULL",
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let phases = sqlx::query_as::<_, OverviewPhase>(
        "SELECT p.id, p.phase_type::text AS phase_type, p.position, p.status::text AS status, COUNT(vc.id) FILTER (WHERE vc.required = true) AS required_criteria, COUNT(vc.id) FILTER (WHERE vc.required = true AND vc.completed = true) AS completed_required_criteria FROM phases p LEFT JOIN validation_criteria vc ON vc.phase_id = p.id WHERE p.project_id = $1 GROUP BY p.id ORDER BY p.position ASC",
    )
    .bind(project_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let requirement_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM requirements WHERE project_id = $1 AND deleted_at IS NULL",
    )
    .bind(project_id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let open_task_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM tasks WHERE project_id = $1 AND deleted_at IS NULL AND status NOT IN ('DONE', 'CANCELLED')",
    )
    .bind(project_id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let decision_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM decisions WHERE project_id = $1 AND deleted_at IS NULL",
    )
    .bind(project_id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let latest_decision = sqlx::query_as::<_, LatestDecision>(
        "SELECT code, title, status::text AS status FROM decisions WHERE project_id = $1 AND deleted_at IS NULL ORDER BY created_at DESC LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ProjectOverviewResponse {
        project,
        phases,
        requirement_count,
        open_task_count,
        decision_count,
        latest_decision,
    }))
}
