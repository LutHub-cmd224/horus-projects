use crate::{
    auth::{TokenType, decode_token},
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct DeployProfile {
    target_environment: String,
    production_url: String,
    provider: String,
    deployment_status: String,
    configuration_notes: String,
    configuration_keys: Value,
    migrations_required: bool,
    migrations_plan: String,
    backups_required: bool,
    backups_plan: String,
    monitoring_required: bool,
    monitoring_plan: String,
    healthcheck_required: bool,
    healthcheck: String,
    rollback_strategy: String,
    deployed_version: String,
    deployed_at: Option<DateTime<Utc>>,
    release_notes: String,
}
#[derive(Debug, Serialize)]
struct DeployWorkspace {
    test_report_ready: bool,
    profile: DeployProfile,
    artifacts: Vec<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/phases/{phase_id}/deploy", get(load).put(save))
        .route(
            "/phases/{phase_id}/deploy/artifacts/RELEASE_REPORT",
            post(report),
        )
}
fn user(h: &HeaderMap, s: &AppState) -> Result<Uuid, StatusCode> {
    let t = h
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let c = decode_token(t, &s.jwt_secret).map_err(|_| StatusCode::UNAUTHORIZED)?;
    if c.token_type != TokenType::Access {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(c.sub)
}
async fn access(s: &AppState, p: Uuid, u: Uuid) -> Result<(Uuid, String, String), StatusCode> {
    sqlx::query_as("SELECT ph.project_id,wm.role::text,ph.status::text FROM phases ph JOIN projects pr ON pr.id=ph.project_id AND pr.deleted_at IS NULL JOIN workspace_members wm ON wm.workspace_id=pr.workspace_id WHERE ph.id=$1 AND ph.phase_type='DEPLOY' AND wm.user_id=$2").bind(p).bind(u).fetch_optional(&s.db).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::FORBIDDEN)
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
fn empty() -> DeployProfile {
    DeployProfile {
        target_environment: "Production".into(),
        production_url: String::new(),
        provider: String::new(),
        deployment_status: "PREPARING".into(),
        configuration_notes: String::new(),
        configuration_keys: json!([]),
        migrations_required: false,
        migrations_plan: String::new(),
        backups_required: false,
        backups_plan: String::new(),
        monitoring_required: true,
        monitoring_plan: String::new(),
        healthcheck_required: true,
        healthcheck: String::new(),
        rollback_strategy: String::new(),
        deployed_version: String::new(),
        deployed_at: None,
        release_notes: String::new(),
    }
}
async fn criterion(
    tx: &mut Transaction<'_, Postgres>,
    p: Uuid,
    code: &str,
    done: bool,
    u: Uuid,
) -> Result<(), StatusCode> {
    sqlx::query("UPDATE validation_criteria SET completed=$3,completed_by=CASE WHEN $3 THEN $4 ELSE NULL END,completed_at=CASE WHEN $3 THEN now() ELSE NULL END WHERE phase_id=$1 AND code=$2").bind(p).bind(code).bind(done).bind(u).execute(&mut**tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}
async fn refresh(
    tx: &mut Transaction<'_, Postgres>,
    p: Uuid,
    project: Uuid,
    u: Uuid,
    profile: &DeployProfile,
) -> Result<(), StatusCode> {
    let test_report=sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM phases t JOIN deliverables d ON d.phase_id=t.id AND d.type='TEST_TEST_REPORT' AND d.status='READY' AND d.deleted_at IS NULL WHERE t.project_id=$1 AND t.phase_type='TEST' AND t.status='VALIDATED')").bind(project).fetch_one(&mut**tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    let configuration = !profile.target_environment.trim().is_empty()
        && !profile.provider.trim().is_empty()
        && !profile.configuration_notes.trim().is_empty()
        && profile.configuration_keys.is_array();
    let migrations = !profile.migrations_required || !profile.migrations_plan.trim().is_empty();
    let backups = !profile.backups_required || !profile.backups_plan.trim().is_empty();
    let health = !profile.healthcheck_required || !profile.healthcheck.trim().is_empty();
    let monitoring = !profile.monitoring_required || !profile.monitoring_plan.trim().is_empty();
    let rollback = !profile.rollback_strategy.trim().is_empty();
    let deployed = profile.deployment_status == "DEPLOYED"
        && !profile.production_url.trim().is_empty()
        && !profile.deployed_version.trim().is_empty()
        && profile.deployed_at.is_some();
    for (code, done) in [
        ("test_report_available", test_report),
        ("production_configured", configuration),
        ("migrations_prepared", migrations),
        ("backups_ready", backups),
        ("healthcheck_defined", health),
        ("monitoring_ready", monitoring),
        ("rollback_defined", rollback),
        ("deployment_confirmed", deployed),
    ] {
        criterion(tx, p, code, done, u).await?;
    }
    criterion(tx, p, "release_report_ready", false, u).await?;
    sqlx::query("UPDATE phases SET status='IN_PROGRESS',started_at=COALESCE(started_at,now()),updated_at=now() WHERE id=$1 AND status='AVAILABLE'").bind(p).execute(&mut**tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}
async fn workspace(s: &AppState, p: Uuid, project: Uuid) -> Result<DeployWorkspace, StatusCode> {
    let test_report_ready=sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM phases t JOIN deliverables d ON d.phase_id=t.id AND d.type='TEST_TEST_REPORT' AND d.status='READY' AND d.deleted_at IS NULL WHERE t.project_id=$1 AND t.phase_type='TEST' AND t.status='VALIDATED')").bind(project).fetch_one(&s.db).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    let profile=sqlx::query_as::<_,DeployProfile>("SELECT target_environment,production_url,provider,deployment_status,configuration_notes,configuration_keys,migrations_required,migrations_plan,backups_required,backups_plan,monitoring_required,monitoring_plan,healthcheck_required,healthcheck,rollback_strategy,deployed_version,deployed_at,release_notes FROM deploy_profiles WHERE phase_id=$1").bind(p).fetch_optional(&s.db).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?.unwrap_or_else(empty);
    let artifacts=sqlx::query_scalar::<_,String>("SELECT substring(type FROM 9) FROM deliverables WHERE phase_id=$1 AND type LIKE 'RELEASE_%' AND deleted_at IS NULL").bind(p).fetch_all(&s.db).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(DeployWorkspace {
        test_report_ready,
        profile,
        artifacts,
    })
}
async fn load(
    State(s): State<AppState>,
    Path(p): Path<Uuid>,
    h: HeaderMap,
) -> Result<Json<DeployWorkspace>, StatusCode> {
    let (project, _, _) = access(&s, p, user(&h, &s)?).await?;
    Ok(Json(workspace(&s, p, project).await?))
}
fn valid_status(v: &str) -> bool {
    matches!(
        v,
        "PREPARING" | "READY" | "DEPLOYED" | "FAILED" | "ROLLED_BACK"
    )
}
async fn save(
    State(s): State<AppState>,
    Path(p): Path<Uuid>,
    h: HeaderMap,
    Json(mut x): Json<DeployProfile>,
) -> Result<Json<DeployWorkspace>, StatusCode> {
    let u = user(&h, &s)?;
    let (project, role, status) = access(&s, p, u).await?;
    writable(&role, &status)?;
    x.deployment_status = x.deployment_status.to_uppercase();
    if !valid_status(&x.deployment_status) || !x.configuration_keys.is_array() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    if x.deployment_status == "DEPLOYED" && x.deployed_at.is_none() {
        x.deployed_at = Some(Utc::now())
    }
    let mut tx =
        s.db.begin()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("INSERT INTO deploy_profiles(phase_id,target_environment,production_url,provider,deployment_status,configuration_notes,configuration_keys,migrations_required,migrations_plan,backups_required,backups_plan,monitoring_required,monitoring_plan,healthcheck_required,healthcheck,rollback_strategy,deployed_version,deployed_at,release_notes)VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19)ON CONFLICT(phase_id)DO UPDATE SET target_environment=$2,production_url=$3,provider=$4,deployment_status=$5,configuration_notes=$6,configuration_keys=$7,migrations_required=$8,migrations_plan=$9,backups_required=$10,backups_plan=$11,monitoring_required=$12,monitoring_plan=$13,healthcheck_required=$14,healthcheck=$15,rollback_strategy=$16,deployed_version=$17,deployed_at=$18,release_notes=$19,updated_at=now()").bind(p).bind(x.target_environment.trim()).bind(x.production_url.trim()).bind(x.provider.trim()).bind(&x.deployment_status).bind(x.configuration_notes.trim()).bind(&x.configuration_keys).bind(x.migrations_required).bind(x.migrations_plan.trim()).bind(x.backups_required).bind(x.backups_plan.trim()).bind(x.monitoring_required).bind(x.monitoring_plan.trim()).bind(x.healthcheck_required).bind(x.healthcheck.trim()).bind(x.rollback_strategy.trim()).bind(x.deployed_version.trim()).bind(x.deployed_at).bind(x.release_notes.trim()).execute(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    refresh(&mut tx, p, project, u, &x).await?;
    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(workspace(&s, p, project).await?))
}
async fn report(
    State(s): State<AppState>,
    Path(p): Path<Uuid>,
    h: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    let u = user(&h, &s)?;
    let (project, role, status) = access(&s, p, u).await?;
    writable(&role, &status)?;
    let mut tx =
        s.db.begin()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let profile=sqlx::query_as::<_,DeployProfile>("SELECT target_environment,production_url,provider,deployment_status,configuration_notes,configuration_keys,migrations_required,migrations_plan,backups_required,backups_plan,monitoring_required,monitoring_plan,healthcheck_required,healthcheck,rollback_strategy,deployed_version,deployed_at,release_notes FROM deploy_profiles WHERE phase_id=$1 FOR UPDATE").bind(p).fetch_optional(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
    refresh(&mut tx, p, project, u, &profile).await?;
    let ready=sqlx::query_scalar::<_,bool>("SELECT COUNT(*)=8 FROM validation_criteria WHERE phase_id=$1 AND code IN ('test_report_available','production_configured','migrations_prepared','backups_ready','healthcheck_defined','monitoring_ready','rollback_defined','deployment_confirmed') AND completed").bind(p).fetch_one(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    if !ready {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let content = serde_json::to_value(&profile).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let changed=sqlx::query("UPDATE deliverables SET content=$2,status='READY',version=version+1,updated_at=now() WHERE phase_id=$1 AND type='RELEASE_REPORT' AND deleted_at IS NULL").bind(p).bind(&content).execute(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    if changed.rows_affected() == 0 {
        sqlx::query("INSERT INTO deliverables(phase_id,title,type,content,status,created_by)VALUES($1,'Release Report','RELEASE_REPORT',$2,'READY',$3)").bind(p).bind(&content).bind(u).execute(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    criterion(&mut tx, p, "release_report_ready", true, u).await?;
    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(content))
}
