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

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub workspace_id: Uuid,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ProjectResponse {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub status: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_projects).post(create_project))
        .route("/{project_id}", get(get_project).delete(archive_project))
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

async fn workspace_role(
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

fn slugify(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

async fn list_projects(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ProjectResponse>>, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    let projects = sqlx::query_as::<_, ProjectResponse>(
        "SELECT p.id, p.workspace_id, p.name, p.slug, p.description, p.status::text AS status FROM projects p JOIN workspace_members wm ON wm.workspace_id = p.workspace_id WHERE wm.user_id = $1 AND p.deleted_at IS NULL ORDER BY p.created_at DESC",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(projects))
}

async fn get_project(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ProjectResponse>, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    let project = sqlx::query_as::<_, ProjectResponse>(
        "SELECT p.id, p.workspace_id, p.name, p.slug, p.description, p.status::text AS status FROM projects p JOIN workspace_members wm ON wm.workspace_id = p.workspace_id WHERE p.id = $1 AND wm.user_id = $2 AND p.deleted_at IS NULL",
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(project))
}

async fn create_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateProjectRequest>,
) -> Result<(StatusCode, Json<ProjectResponse>), StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    let role = workspace_role(&state, payload.workspace_id, user_id).await?;
    if role == "VIEWER" {
        return Err(StatusCode::FORBIDDEN);
    }

    let name = payload.name.trim();
    if name.is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    let base_slug = slugify(name);
    if base_slug.is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let slug = format!(
        "{}-{}",
        base_slug,
        &Uuid::now_v7().simple().to_string()[..8]
    );

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let project = sqlx::query_as::<_, ProjectResponse>(
        "INSERT INTO projects (workspace_id, name, slug, description, status, created_by) VALUES ($1, $2, $3, $4, 'DRAFT', $5) RETURNING id, workspace_id, name, slug, description, status::text AS status",
    )
    .bind(payload.workspace_id)
    .bind(name)
    .bind(slug)
    .bind(payload.description)
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let phases = [
        ("ANALYZE", 1_i16, "AVAILABLE"),
        ("MODEL", 2_i16, "LOCKED"),
        ("DESIGN", 3_i16, "LOCKED"),
        ("BUILD", 4_i16, "LOCKED"),
        ("TEST", 5_i16, "LOCKED"),
        ("DEPLOY", 6_i16, "LOCKED"),
    ];

    let mut analyze_phase_id = None;
    let mut model_phase_id = None;
    let mut design_phase_id = None;
    let mut build_phase_id = None;
    let mut test_phase_id = None;
    for (phase_type, position, status) in phases {
        let phase_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO phases (project_id, phase_type, position, status) VALUES ($1, $2::phase_type, $3, $4::phase_status) RETURNING id",
        )
        .bind(project.id)
        .bind(phase_type)
        .bind(position)
        .bind(status)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if position == 1 {
            analyze_phase_id = Some(phase_id);
        } else if position == 2 {
            model_phase_id = Some(phase_id);
        } else if position == 3 {
            design_phase_id = Some(phase_id);
        } else if position == 4 {
            build_phase_id = Some(phase_id);
        } else if position == 5 {
            test_phase_id = Some(phase_id);
        }
    }

    let analyze_phase_id = analyze_phase_id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let criteria = [
        ("problem_defined", "Problem defined"),
        ("target_user_defined", "Target user defined"),
        ("value_proposition_defined", "Value proposition defined"),
        ("objectives_defined", "Objectives defined"),
        ("constraints_defined", "Constraints defined"),
        ("mvp_defined", "MVP defined"),
    ];
    for (code, label) in criteria {
        sqlx::query(
            "INSERT INTO validation_criteria (phase_id, code, label, required, completed) VALUES ($1, $2, $3, true, false)",
        )
        .bind(analyze_phase_id)
        .bind(code)
        .bind(label)
        .execute(&mut *tx)
        .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    let model_phase_id = model_phase_id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    for (code, label) in [
        ("entities_defined", "Entities defined"),
        ("relations_defined", "Relationships defined"),
        ("business_rules_defined", "Business rules defined"),
        ("mcd_defined", "MCD ready"),
        ("mld_defined", "MLD ready"),
        ("mpd_defined", "MPD ready"),
    ] {
        sqlx::query("INSERT INTO validation_criteria (phase_id, code, label, required, completed) VALUES ($1, $2, $3, true, false)")
            .bind(model_phase_id)
            .bind(code)
            .bind(label)
            .execute(&mut *tx)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    let test_phase_id = test_phase_id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    for (code, label) in [
        ("build_plan_available", "Validated Build Plan available"),
        ("test_cases_defined", "Test cases defined"),
        ("test_traceability_complete", "Test traceability complete"),
        ("tests_executed", "All tests executed"),
        ("tests_passing", "All tests passing"),
        ("no_blocking_defects", "No unresolved blocking defects"),
        ("test_report_ready", "Test report ready"),
    ] {
        sqlx::query("INSERT INTO validation_criteria(phase_id,code,label,required,completed) VALUES($1,$2,$3,true,false)").bind(test_phase_id).bind(code).bind(label).execute(&mut *tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    let build_phase_id = build_phase_id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    for (code, label) in [
        ("design_pack_available", "Validated Design Pack available"),
        ("backlog_ready", "Implementation backlog ready"),
        ("requirements_linked", "Tasks linked to requirements"),
        (
            "execution_decisions_recorded",
            "Execution decisions recorded",
        ),
        ("github_ready", "GitHub preparation complete"),
        ("build_plan_ready", "Build plan ready"),
    ] {
        sqlx::query("INSERT INTO validation_criteria(phase_id,code,label,required,completed) VALUES($1,$2,$3,true,false)").bind(build_phase_id).bind(code).bind(label).execute(&mut *tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    let design_phase_id = design_phase_id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    for (code, label) in [
        ("architecture_defined", "Technical architecture defined"),
        ("ux_flows_defined", "UX flows defined"),
        ("features_specified", "Features specified"),
        ("api_contracts_defined", "API contracts defined"),
        ("security_reviewed", "Security reviewed"),
        (
            "technology_decisions_accepted",
            "Technology decisions accepted",
        ),
        ("design_pack_ready", "Design pack ready"),
    ] {
        sqlx::query("INSERT INTO validation_criteria(phase_id,code,label,required,completed) VALUES($1,$2,$3,true,false)").bind(design_phase_id).bind(code).bind(label).execute(&mut *tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::CREATED, Json(project)))
}

async fn archive_project(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let user_id = authenticated_user(&headers, &state)?;
    let project = sqlx::query_as::<_, ProjectResponse>(
        "SELECT p.id, p.workspace_id, p.name, p.slug, p.description, p.status::text AS status FROM projects p JOIN workspace_members wm ON wm.workspace_id = p.workspace_id WHERE p.id = $1 AND wm.user_id = $2 AND p.deleted_at IS NULL",
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let role = workspace_role(&state, project.workspace_id, user_id).await?;
    if role != "OWNER" && role != "ADMIN" {
        return Err(StatusCode::FORBIDDEN);
    }

    sqlx::query("UPDATE projects SET deleted_at = now(), status = 'ARCHIVED', updated_at = now() WHERE id = $1")
        .bind(project_id)
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}
