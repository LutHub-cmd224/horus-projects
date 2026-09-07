use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    routing::{get, patch},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    auth::{TokenType, decode_token},
    state::AppState,
};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct WorkspaceResponse {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub role: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct MemberResponse {
    pub user_id: Uuid,
    pub email: String,
    pub display_name: Option<String>,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRoleRequest {
    pub role: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_workspaces))
        .route("/{workspace_id}/members", get(list_members))
        .route(
            "/{workspace_id}/members/{user_id}",
            patch(update_member_role),
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

async fn membership_role(
    state: &AppState,
    workspace_id: Uuid,
    user_id: Uuid,
) -> Result<String, StatusCode> {
    sqlx::query_scalar::<_, String>(
        "SELECT role::text FROM workspace_members WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::FORBIDDEN)
}

async fn list_workspaces(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkspaceResponse>>, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    let workspaces = sqlx::query_as::<_, WorkspaceResponse>(
        "SELECT w.id, w.name, w.slug, wm.role::text AS role FROM workspaces w JOIN workspace_members wm ON wm.workspace_id = w.id WHERE wm.user_id = $1 ORDER BY w.created_at ASC",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(workspaces))
}

async fn list_members(
    State(state): State<AppState>,
    Path(workspace_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<MemberResponse>>, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    membership_role(&state, workspace_id, user_id).await?;
    let members = sqlx::query_as::<_, MemberResponse>(
        "SELECT u.id AS user_id, u.email, u.display_name, wm.role::text AS role FROM workspace_members wm JOIN users u ON u.id = wm.user_id WHERE wm.workspace_id = $1 ORDER BY wm.joined_at ASC",
    )
    .bind(workspace_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(members))
}

async fn update_member_role(
    State(state): State<AppState>,
    Path((workspace_id, target_user_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(payload): Json<UpdateRoleRequest>,
) -> Result<StatusCode, StatusCode> {
    let actor_id = authenticated_user(&headers, &state)?;
    let actor_role = membership_role(&state, workspace_id, actor_id).await?;
    if actor_role != "OWNER" && actor_role != "ADMIN" {
        return Err(StatusCode::FORBIDDEN);
    }
    if actor_id == target_user_id && actor_role == "OWNER" {
        return Err(StatusCode::CONFLICT);
    }
    let role = payload.role.to_uppercase();
    if !matches!(role.as_str(), "ADMIN" | "MEMBER" | "VIEWER") {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let result = sqlx::query(
        "UPDATE workspace_members SET role = $1::workspace_role WHERE workspace_id = $2 AND user_id = $3 AND role <> 'OWNER'",
    )
    .bind(role)
    .bind(workspace_id)
    .bind(target_user_id)
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if result.rows_affected() != 1 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}
