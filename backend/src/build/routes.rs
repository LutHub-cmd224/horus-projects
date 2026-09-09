use crate::{
    auth::{TokenType, decode_token},
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    routing::{get, patch, post, put},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Serialize, sqlx::FromRow)]
struct Requirement {
    id: Uuid,
    code: String,
    title: String,
    status: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct Task {
    id: Uuid,
    requirement_id: Option<Uuid>,
    code: String,
    title: String,
    description: Option<String>,
    status: String,
    priority: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct Decision {
    id: Uuid,
    code: String,
    title: String,
    decision: String,
    status: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct GithubProfile {
    repository_url: String,
    default_branch: String,
    integration_strategy: String,
    ci_required: bool,
    ci_configured: bool,
    definition_of_done: Value,
}

#[derive(Debug, Serialize)]
struct Progress {
    total: i64,
    done: i64,
    blocked: i64,
    percent: i64,
}

#[derive(Debug, Serialize)]
struct BuildWorkspace {
    design_pack_ready: bool,
    requirements: Vec<Requirement>,
    tasks: Vec<Task>,
    decisions: Vec<Decision>,
    github: GithubProfile,
    progress: Progress,
    artifacts: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CreateTask {
    requirement_id: Uuid,
    title: String,
    description: Option<String>,
    priority: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateTask {
    status: String,
}

#[derive(Debug, Deserialize)]
struct CreateDecision {
    title: String,
    decision: String,
    status: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/phases/{phase_id}/build", get(load))
        .route("/phases/{phase_id}/build/github", put(save_github))
        .route("/phases/{phase_id}/build/tasks", post(create_task))
        .route(
            "/phases/{phase_id}/build/tasks/{task_id}",
            patch(update_task),
        )
        .route("/phases/{phase_id}/build/decisions", post(create_decision))
        .route(
            "/phases/{phase_id}/build/artifacts/BUILD_PLAN",
            post(generate_build_plan),
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
) -> Result<(Uuid, String, String), StatusCode> {
    sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT ph.project_id,wm.role::text,ph.status::text FROM phases ph JOIN projects p ON p.id=ph.project_id AND p.deleted_at IS NULL JOIN workspace_members wm ON wm.workspace_id=p.workspace_id WHERE ph.id=$1 AND ph.phase_type='BUILD' AND wm.user_id=$2",
    )
    .bind(phase_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::FORBIDDEN)
}

fn writable(role: &str, status: &str) -> Result<(), StatusCode> {
    if role == "VIEWER" {
        return Err(StatusCode::FORBIDDEN);
    }
    if status == "LOCKED" || status == "VALIDATED" {
        return Err(StatusCode::CONFLICT);
    }
    Ok(())
}

async fn set_criterion(
    tx: &mut Transaction<'_, Postgres>,
    phase_id: Uuid,
    code: &str,
    completed: bool,
    user_id: Uuid,
) -> Result<(), StatusCode> {
    sqlx::query("UPDATE validation_criteria SET completed=$3,completed_by=CASE WHEN $3 THEN $4 ELSE NULL END,completed_at=CASE WHEN $3 THEN now() ELSE NULL END WHERE phase_id=$1 AND code=$2")
        .bind(phase_id).bind(code).bind(completed).bind(user_id).execute(&mut **tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}

async fn refresh_criteria(
    tx: &mut Transaction<'_, Postgres>,
    phase_id: Uuid,
    project_id: Uuid,
    user_id: Uuid,
) -> Result<(), StatusCode> {
    let design_pack = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM phases d JOIN deliverables x ON x.phase_id=d.id AND x.type='DESIGN_DESIGN_PACK' AND x.status='READY' AND x.deleted_at IS NULL WHERE d.project_id=$1 AND d.phase_type='DESIGN' AND d.status='VALIDATED')").bind(project_id).fetch_one(&mut **tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    let (total, linked) = sqlx::query_as::<_, (i64, i64)>(
        "SELECT COUNT(*),COUNT(requirement_id) FROM tasks WHERE phase_id=$1 AND deleted_at IS NULL",
    )
    .bind(phase_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let accepted = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM decisions WHERE phase_id=$1 AND status='ACCEPTED' AND deleted_at IS NULL)").bind(phase_id).fetch_one(&mut **tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    let github = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM build_profiles WHERE phase_id=$1 AND repository_url<>'' AND default_branch<>'' AND jsonb_array_length(definition_of_done)>0 AND (NOT ci_required OR ci_configured))").bind(phase_id).fetch_one(&mut **tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    for (code, completed) in [
        ("design_pack_available", design_pack),
        ("backlog_ready", total > 0),
        ("requirements_linked", total > 0 && total == linked),
        ("execution_decisions_recorded", accepted),
        ("github_ready", github),
    ] {
        set_criterion(tx, phase_id, code, completed, user_id).await?;
    }
    sqlx::query("UPDATE phases SET status='IN_PROGRESS',started_at=COALESCE(started_at,now()),updated_at=now() WHERE id=$1 AND status='AVAILABLE'").bind(phase_id).execute(&mut **tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}

async fn workspace(
    state: &AppState,
    phase_id: Uuid,
    project_id: Uuid,
) -> Result<BuildWorkspace, StatusCode> {
    let design_pack_ready=sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM phases d JOIN deliverables x ON x.phase_id=d.id AND x.type='DESIGN_DESIGN_PACK' AND x.status='READY' AND x.deleted_at IS NULL WHERE d.project_id=$1 AND d.phase_type='DESIGN' AND d.status='VALIDATED')").bind(project_id).fetch_one(&state.db).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    let requirements=sqlx::query_as::<_,Requirement>("SELECT id,code,title,status::text status FROM requirements WHERE project_id=$1 AND deleted_at IS NULL ORDER BY code").bind(project_id).fetch_all(&state.db).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    let tasks=sqlx::query_as::<_,Task>("SELECT id,requirement_id,code,title,description,status::text status,priority::text priority FROM tasks WHERE phase_id=$1 AND deleted_at IS NULL ORDER BY created_at").bind(phase_id).fetch_all(&state.db).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    let decisions=sqlx::query_as::<_,Decision>("SELECT id,code,title,decision,status::text status FROM decisions WHERE phase_id=$1 AND deleted_at IS NULL ORDER BY created_at").bind(phase_id).fetch_all(&state.db).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    let github=sqlx::query_as::<_,GithubProfile>("SELECT repository_url,default_branch,integration_strategy,ci_required,ci_configured,definition_of_done FROM build_profiles WHERE phase_id=$1").bind(phase_id).fetch_optional(&state.db).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?.unwrap_or(GithubProfile{repository_url:String::new(),default_branch:"main".into(),integration_strategy:"PULL_REQUEST".into(),ci_required:true,ci_configured:false,definition_of_done:json!([])});
    let total = tasks.len() as i64;
    let done = tasks.iter().filter(|task| task.status == "DONE").count() as i64;
    let blocked = tasks.iter().filter(|task| task.status == "BLOCKED").count() as i64;
    let artifacts=sqlx::query_scalar::<_,String>("SELECT substring(type FROM 7) FROM deliverables WHERE phase_id=$1 AND type LIKE 'BUILD_%' AND deleted_at IS NULL ORDER BY type").bind(phase_id).fetch_all(&state.db).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(BuildWorkspace {
        design_pack_ready,
        requirements,
        tasks,
        decisions,
        github,
        progress: Progress {
            total,
            done,
            blocked,
            percent: if total == 0 { 0 } else { done * 100 / total },
        },
        artifacts,
    })
}

async fn load(
    State(state): State<AppState>,
    Path(phase_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<BuildWorkspace>, StatusCode> {
    let (project_id, _, _) = access(&state, phase_id, user(&headers, &state)?).await?;
    Ok(Json(workspace(&state, phase_id, project_id).await?))
}

async fn save_github(
    State(state): State<AppState>,
    Path(phase_id): Path<Uuid>,
    headers: HeaderMap,
    Json(profile): Json<GithubProfile>,
) -> Result<Json<BuildWorkspace>, StatusCode> {
    let user_id = user(&headers, &state)?;
    let (project_id, role, status) = access(&state, phase_id, user_id).await?;
    writable(&role, &status)?;
    if !profile.repository_url.is_empty()
        && !(profile.repository_url.starts_with("https://github.com/")
            || profile.repository_url.starts_with("git@github.com:"))
    {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    if profile.default_branch.trim().is_empty()
        || !matches!(
            profile.integration_strategy.as_str(),
            "PULL_REQUEST" | "TRUNK_BASED" | "GIT_FLOW"
        )
        || !profile.definition_of_done.is_array()
    {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("INSERT INTO build_profiles(phase_id,repository_url,default_branch,integration_strategy,ci_required,ci_configured,definition_of_done)VALUES($1,$2,$3,$4,$5,$6,$7)ON CONFLICT(phase_id)DO UPDATE SET repository_url=$2,default_branch=$3,integration_strategy=$4,ci_required=$5,ci_configured=$6,definition_of_done=$7,updated_at=now()").bind(phase_id).bind(profile.repository_url.trim()).bind(profile.default_branch.trim()).bind(&profile.integration_strategy).bind(profile.ci_required).bind(profile.ci_configured).bind(&profile.definition_of_done).execute(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    refresh_criteria(&mut tx, phase_id, project_id, user_id).await?;
    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(workspace(&state, phase_id, project_id).await?))
}

fn priority(value: Option<String>) -> Result<String, StatusCode> {
    let value = value.unwrap_or_else(|| "MEDIUM".into()).to_uppercase();
    if matches!(value.as_str(), "LOW" | "MEDIUM" | "HIGH" | "CRITICAL") {
        Ok(value)
    } else {
        Err(StatusCode::UNPROCESSABLE_ENTITY)
    }
}

async fn create_task(
    State(state): State<AppState>,
    Path(phase_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateTask>,
) -> Result<(StatusCode, Json<BuildWorkspace>), StatusCode> {
    let user_id = user(&headers, &state)?;
    let (project_id, role, status) = access(&state, phase_id, user_id).await?;
    writable(&role, &status)?;
    if input.title.trim().is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let priority = priority(input.priority)?;
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let requirement=sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM requirements WHERE id=$1 AND project_id=$2 AND deleted_at IS NULL)").bind(input.requirement_id).bind(project_id).fetch_one(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    if !requirement {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    sqlx::query_scalar::<_, Uuid>("SELECT id FROM projects WHERE id=$1 FOR UPDATE")
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let current=sqlx::query_scalar::<_,i32>("SELECT COALESCE(MAX(CASE WHEN code~'^TASK-[0-9]+$' THEN substring(code FROM 6)::int END),0) FROM tasks WHERE project_id=$1").bind(project_id).fetch_one(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("INSERT INTO tasks(project_id,requirement_id,phase_id,code,title,description,status,priority,created_by)VALUES($1,$2,$3,$4,$5,$6,'TODO',$7::priority_level,$8)").bind(project_id).bind(input.requirement_id).bind(phase_id).bind(format!("TASK-{:03}",current+1)).bind(input.title.trim()).bind(input.description).bind(priority).bind(user_id).execute(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    refresh_criteria(&mut tx, phase_id, project_id, user_id).await?;
    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((
        StatusCode::CREATED,
        Json(workspace(&state, phase_id, project_id).await?),
    ))
}

async fn update_task(
    State(state): State<AppState>,
    Path((phase_id, task_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(input): Json<UpdateTask>,
) -> Result<Json<BuildWorkspace>, StatusCode> {
    let user_id = user(&headers, &state)?;
    let (project_id, role, phase_status) = access(&state, phase_id, user_id).await?;
    writable(&role, &phase_status)?;
    let status = input.status.to_uppercase();
    if !matches!(
        status.as_str(),
        "TODO" | "IN_PROGRESS" | "BLOCKED" | "DONE" | "CANCELLED"
    ) {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let changed=sqlx::query("UPDATE tasks SET status=$3::task_status,updated_at=now() WHERE id=$1 AND phase_id=$2 AND deleted_at IS NULL").bind(task_id).bind(phase_id).bind(status).execute(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    if changed.rows_affected() != 1 {
        return Err(StatusCode::NOT_FOUND);
    }
    refresh_criteria(&mut tx, phase_id, project_id, user_id).await?;
    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(workspace(&state, phase_id, project_id).await?))
}

async fn create_decision(
    State(state): State<AppState>,
    Path(phase_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateDecision>,
) -> Result<(StatusCode, Json<BuildWorkspace>), StatusCode> {
    let user_id = user(&headers, &state)?;
    let (project_id, role, phase_status) = access(&state, phase_id, user_id).await?;
    writable(&role, &phase_status)?;
    if input.title.trim().is_empty() || input.decision.trim().is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let status = input
        .status
        .unwrap_or_else(|| "PROPOSED".into())
        .to_uppercase();
    if !matches!(
        status.as_str(),
        "PROPOSED" | "ACCEPTED" | "SUPERSEDED" | "REJECTED"
    ) {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query_scalar::<_, Uuid>("SELECT id FROM projects WHERE id=$1 FOR UPDATE")
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let current=sqlx::query_scalar::<_,i32>("SELECT COALESCE(MAX(CASE WHEN code~'^DEC-[0-9]+$' THEN substring(code FROM 5)::int END),0) FROM decisions WHERE project_id=$1").bind(project_id).fetch_one(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("INSERT INTO decisions(project_id,phase_id,code,title,decision,status,decided_by,decided_at)VALUES($1,$2,$3,$4,$5,$6::decision_status,$7,CASE WHEN $6='ACCEPTED' THEN now() ELSE NULL END)").bind(project_id).bind(phase_id).bind(format!("DEC-{:03}",current+1)).bind(input.title.trim()).bind(input.decision.trim()).bind(status).bind(user_id).execute(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    refresh_criteria(&mut tx, phase_id, project_id, user_id).await?;
    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((
        StatusCode::CREATED,
        Json(workspace(&state, phase_id, project_id).await?),
    ))
}

async fn generate_build_plan(
    State(state): State<AppState>,
    Path(phase_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    let user_id = user(&headers, &state)?;
    let (project_id, role, status) = access(&state, phase_id, user_id).await?;
    writable(&role, &status)?;
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    refresh_criteria(&mut tx, phase_id, project_id, user_id).await?;
    let ready=sqlx::query_scalar::<_,bool>("SELECT COUNT(*)=5 FROM validation_criteria WHERE phase_id=$1 AND code IN ('design_pack_available','backlog_ready','requirements_linked','execution_decisions_recorded','github_ready') AND completed").bind(phase_id).fetch_one(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    if !ready {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let content = serde_json::to_value(workspace(&state, phase_id, project_id).await?)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let changed=sqlx::query("UPDATE deliverables SET content=$2,status='READY',version=version+1,updated_at=now() WHERE phase_id=$1 AND type='BUILD_BUILD_PLAN' AND deleted_at IS NULL").bind(phase_id).bind(&content).execute(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    if changed.rows_affected() == 0 {
        sqlx::query("INSERT INTO deliverables(phase_id,title,type,content,status,created_by)VALUES($1,'Build Plan','BUILD_BUILD_PLAN',$2,'READY',$3)").bind(phase_id).bind(&content).bind(user_id).execute(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    set_criterion(&mut tx, phase_id, "build_plan_ready", true, user_id).await?;
    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(content))
}
