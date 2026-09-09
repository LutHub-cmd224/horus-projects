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

#[derive(Debug, Deserialize, Serialize, sqlx::FromRow)]
pub struct AnalyzeProfile {
    pub problem: String,
    pub target_audiences: Value,
    pub target_details: String,
    pub value_proposition: String,
    pub success_objectives: Value,
    pub budget: String,
    pub deadline: String,
    pub platform: String,
    pub special_constraints: String,
    pub constraints_unknown: bool,
    pub mvp_features: Value,
}

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/phases/{phase_id}/analyze",
        get(load_analyze).put(save_analyze),
    )
}

fn user(headers: &HeaderMap, state: &AppState) -> Result<Uuid, StatusCode> {
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

async fn access(
    state: &AppState,
    phase_id: Uuid,
    user_id: Uuid,
) -> Result<(String, String), StatusCode> {
    sqlx::query_as::<_, (String, String)>("SELECT wm.role::text, ph.status::text FROM phases ph JOIN projects p ON p.id=ph.project_id JOIN workspace_members wm ON wm.workspace_id=p.workspace_id WHERE ph.id=$1 AND ph.phase_type='ANALYZE' AND wm.user_id=$2 AND p.deleted_at IS NULL")
        .bind(phase_id)
        .bind(user_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::FORBIDDEN)
}

async fn profile(state: &AppState, phase_id: Uuid) -> Result<AnalyzeProfile, StatusCode> {
    sqlx::query_as::<_, AnalyzeProfile>("SELECT problem,target_audiences,target_details,value_proposition,success_objectives,budget,deadline,platform,special_constraints,constraints_unknown,mvp_features FROM analyze_profiles WHERE phase_id=$1")
        .bind(phase_id)
        .fetch_one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn load_analyze(
    State(state): State<AppState>,
    Path(phase_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<AnalyzeProfile>, StatusCode> {
    access(&state, phase_id, user(&headers, &state)?).await?;
    Ok(Json(profile(&state, phase_id).await?))
}

fn non_empty_array(value: &Value) -> bool {
    value.as_array().is_some_and(|items| {
        items
            .iter()
            .any(|item| item.as_str().is_some_and(|text| !text.trim().is_empty()))
    })
}

async fn save_analyze(
    State(state): State<AppState>,
    Path(phase_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<AnalyzeProfile>,
) -> Result<Json<AnalyzeProfile>, StatusCode> {
    let user_id = user(&headers, &state)?;
    let (role, status) = access(&state, phase_id, user_id).await?;
    if role == "VIEWER" {
        return Err(StatusCode::FORBIDDEN);
    }
    if status == "LOCKED" || status == "VALIDATED" {
        return Err(StatusCode::CONFLICT);
    }
    if !payload.target_audiences.is_array()
        || !payload.success_objectives.is_array()
        || !payload.mvp_features.is_array()
    {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    let completed = [
        ("problem_defined", !payload.problem.trim().is_empty()),
        (
            "target_user_defined",
            non_empty_array(&payload.target_audiences) || !payload.target_details.trim().is_empty(),
        ),
        (
            "value_proposition_defined",
            !payload.value_proposition.trim().is_empty(),
        ),
        (
            "objectives_defined",
            non_empty_array(&payload.success_objectives),
        ),
        (
            "constraints_defined",
            payload.constraints_unknown
                || [
                    &payload.budget,
                    &payload.deadline,
                    &payload.platform,
                    &payload.special_constraints,
                ]
                .iter()
                .any(|value| !value.trim().is_empty()),
        ),
        ("mvp_defined", non_empty_array(&payload.mvp_features)),
    ];
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("INSERT INTO analyze_profiles(phase_id,problem,target_audiences,target_details,value_proposition,success_objectives,budget,deadline,platform,special_constraints,constraints_unknown,mvp_features) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) ON CONFLICT(phase_id) DO UPDATE SET problem=EXCLUDED.problem,target_audiences=EXCLUDED.target_audiences,target_details=EXCLUDED.target_details,value_proposition=EXCLUDED.value_proposition,success_objectives=EXCLUDED.success_objectives,budget=EXCLUDED.budget,deadline=EXCLUDED.deadline,platform=EXCLUDED.platform,special_constraints=EXCLUDED.special_constraints,constraints_unknown=EXCLUDED.constraints_unknown,mvp_features=EXCLUDED.mvp_features,updated_at=now()")
        .bind(phase_id).bind(payload.problem.trim()).bind(&payload.target_audiences).bind(payload.target_details.trim()).bind(payload.value_proposition.trim()).bind(&payload.success_objectives).bind(payload.budget.trim()).bind(payload.deadline.trim()).bind(payload.platform.trim()).bind(payload.special_constraints.trim()).bind(payload.constraints_unknown).bind(&payload.mvp_features)
        .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("UPDATE phases SET status='IN_PROGRESS',started_at=COALESCE(started_at,now()),updated_at=now() WHERE id=$1 AND status='AVAILABLE'")
        .bind(phase_id).execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    for (code, done) in completed {
        sqlx::query("UPDATE validation_criteria SET completed=$2,completed_by=CASE WHEN $2 THEN $3 ELSE NULL END,completed_at=CASE WHEN $2 THEN now() ELSE NULL END WHERE phase_id=$1 AND code=$4")
            .bind(phase_id).bind(done).bind(user_id).bind(code).execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(profile(&state, phase_id).await?))
}
