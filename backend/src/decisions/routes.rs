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
pub struct DecisionResponse {
    pub id: Uuid,
    pub project_id: Uuid,
    pub phase_id: Option<Uuid>,
    pub code: String,
    pub title: String,
    pub context: Option<String>,
    pub decision: String,
    pub alternatives: Value,
    pub consequences: Option<String>,
    pub status: String,
    pub decided_by: Uuid,
    pub decided_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDecisionRequest {
    pub phase_id: Option<Uuid>,
    pub title: String,
    pub context: Option<String>,
    pub decision: String,
    pub alternatives: Option<Value>,
    pub consequences: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDecisionRequest {
    pub phase_id: Option<Uuid>,
    pub title: Option<String>,
    pub context: Option<String>,
    pub decision: Option<String>,
    pub alternatives: Option<Value>,
    pub consequences: Option<String>,
    pub status: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/projects/{project_id}/decisions",
            get(list_decisions).post(create_decision),
        )
        .route(
            "/decisions/{decision_id}",
            get(get_decision)
                .patch(update_decision)
                .delete(delete_decision),
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

async fn decision_access(
    state: &AppState,
    decision_id: Uuid,
    user_id: Uuid,
) -> Result<(Uuid, String), StatusCode> {
    sqlx::query_as::<_, (Uuid, String)>(
        "SELECT d.project_id, wm.role::text FROM decisions d JOIN projects p ON p.id = d.project_id JOIN workspace_members wm ON wm.workspace_id = p.workspace_id WHERE d.id = $1 AND d.deleted_at IS NULL AND p.deleted_at IS NULL AND wm.user_id = $2",
    )
    .bind(decision_id)
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

fn normalize_status(value: Option<String>) -> Result<String, StatusCode> {
    let value = value
        .unwrap_or_else(|| "PROPOSED".into())
        .trim()
        .to_uppercase();
    if matches!(
        value.as_str(),
        "PROPOSED" | "ACCEPTED" | "SUPERSEDED" | "REJECTED"
    ) {
        Ok(value)
    } else {
        Err(StatusCode::UNPROCESSABLE_ENTITY)
    }
}

fn validate_alternatives(value: &Value) -> Result<(), StatusCode> {
    if value.is_array() {
        Ok(())
    } else {
        Err(StatusCode::UNPROCESSABLE_ENTITY)
    }
}

async fn validate_phase(
    state: &AppState,
    project_id: Uuid,
    phase_id: Option<Uuid>,
) -> Result<(), StatusCode> {
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
    Ok(())
}

async fn list_decisions(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<DecisionResponse>>, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    project_role(&state, project_id, user_id).await?;
    let decisions = sqlx::query_as::<_, DecisionResponse>(
        "SELECT id, project_id, phase_id, code, title, context, decision, alternatives, consequences, status::text AS status, decided_by, decided_at FROM decisions WHERE project_id = $1 AND deleted_at IS NULL ORDER BY created_at ASC",
    )
    .bind(project_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(decisions))
}

async fn get_decision(
    State(state): State<AppState>,
    Path(decision_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<DecisionResponse>, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    decision_access(&state, decision_id, user_id).await?;
    let decision = sqlx::query_as::<_, DecisionResponse>(
        "SELECT id, project_id, phase_id, code, title, context, decision, alternatives, consequences, status::text AS status, decided_by, decided_at FROM decisions WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(decision_id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(decision))
}

async fn create_decision(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<CreateDecisionRequest>,
) -> Result<(StatusCode, Json<DecisionResponse>), StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    let role = project_role(&state, project_id, user_id).await?;
    ensure_writable(&role)?;

    let title = payload.title.trim();
    let decision_text = payload.decision.trim();
    if title.is_empty() || decision_text.is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let status = normalize_status(payload.status)?;
    let alternatives = payload.alternatives.unwrap_or_else(|| Value::Array(vec![]));
    validate_alternatives(&alternatives)?;
    validate_phase(&state, project_id, payload.phase_id).await?;

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
        "SELECT COALESCE(MAX(CASE WHEN code ~ '^DEC-[0-9]+$' THEN substring(code FROM 5)::int END), 0) FROM decisions WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let code = format!("DEC-{:03}", current + 1);
    let decided_at = if status == "ACCEPTED" {
        Some(chrono::Utc::now())
    } else {
        None
    };

    let decision = sqlx::query_as::<_, DecisionResponse>(
        "INSERT INTO decisions (project_id, phase_id, code, title, context, decision, alternatives, consequences, status, decided_by, decided_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9::decision_status, $10, $11) RETURNING id, project_id, phase_id, code, title, context, decision, alternatives, consequences, status::text AS status, decided_by, decided_at",
    )
    .bind(project_id)
    .bind(payload.phase_id)
    .bind(code)
    .bind(title)
    .bind(payload.context)
    .bind(decision_text)
    .bind(alternatives)
    .bind(payload.consequences)
    .bind(status)
    .bind(user_id)
    .bind(decided_at)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((StatusCode::CREATED, Json(decision)))
}

async fn update_decision(
    State(state): State<AppState>,
    Path(decision_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<UpdateDecisionRequest>,
) -> Result<Json<DecisionResponse>, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    let (project_id, role) = decision_access(&state, decision_id, user_id).await?;
    ensure_writable(&role)?;

    if payload
        .title
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
        || payload
            .decision
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
    {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let status = match payload.status {
        Some(value) => Some(normalize_status(Some(value))?),
        None => None,
    };
    if let Some(ref alternatives) = payload.alternatives {
        validate_alternatives(alternatives)?;
    }
    validate_phase(&state, project_id, payload.phase_id).await?;

    let decision = sqlx::query_as::<_, DecisionResponse>(
        "UPDATE decisions SET phase_id = COALESCE($2, phase_id), title = COALESCE($3, title), context = COALESCE($4, context), decision = COALESCE($5, decision), alternatives = COALESCE($6, alternatives), consequences = COALESCE($7, consequences), status = COALESCE($8::decision_status, status), decided_at = CASE WHEN $8::decision_status = 'ACCEPTED' AND status <> 'ACCEPTED' THEN now() WHEN $8::decision_status IS NOT NULL AND $8::decision_status <> 'ACCEPTED' THEN NULL ELSE decided_at END, updated_at = now() WHERE id = $1 AND deleted_at IS NULL RETURNING id, project_id, phase_id, code, title, context, decision, alternatives, consequences, status::text AS status, decided_by, decided_at",
    )
    .bind(decision_id)
    .bind(payload.phase_id)
    .bind(payload.title.map(|value| value.trim().to_string()))
    .bind(payload.context)
    .bind(payload.decision.map(|value| value.trim().to_string()))
    .bind(payload.alternatives)
    .bind(payload.consequences)
    .bind(status)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(decision))
}

async fn delete_decision(
    State(state): State<AppState>,
    Path(decision_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    let (_, role) = decision_access(&state, decision_id, user_id).await?;
    ensure_writable(&role)?;
    let result = sqlx::query(
        "UPDATE decisions SET deleted_at = now(), updated_at = now() WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(decision_id)
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
    use super::{normalize_status, validate_alternatives};
    use serde_json::json;

    #[test]
    fn decision_values_are_validated() {
        assert_eq!(normalize_status(None).unwrap(), "PROPOSED");
        assert_eq!(
            normalize_status(Some("accepted".into())).unwrap(),
            "ACCEPTED"
        );
        assert!(normalize_status(Some("unknown".into())).is_err());
        assert!(validate_alternatives(&json!([])).is_ok());
        assert!(validate_alternatives(&json!({})).is_err());
    }
}
