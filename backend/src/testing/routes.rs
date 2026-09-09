use crate::{
    auth::{TokenType, decode_token},
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    routing::{get, patch, post},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Serialize, sqlx::FromRow)]
struct TestCase {
    id: Uuid,
    requirement_id: Option<Uuid>,
    task_id: Option<Uuid>,
    code: String,
    title: String,
    test_type: String,
    expected_result: String,
    actual_result: Option<String>,
    status: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct Defect {
    id: Uuid,
    test_case_id: Uuid,
    title: String,
    description: String,
    severity: String,
    status: String,
    resolution: Option<String>,
}

#[derive(Debug, Serialize)]
struct Link {
    id: Uuid,
    code: String,
    title: String,
}

#[derive(Debug, Serialize)]
struct Progress {
    total: i64,
    executed: i64,
    passed: i64,
    failed: i64,
    blocked: i64,
    percent: i64,
    pass_rate: i64,
}

#[derive(Debug, Serialize)]
struct TestWorkspace {
    build_plan_ready: bool,
    requirements: Vec<Link>,
    tasks: Vec<Link>,
    test_cases: Vec<TestCase>,
    defects: Vec<Defect>,
    progress: Progress,
    artifacts: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CreateCase {
    requirement_id: Option<Uuid>,
    task_id: Option<Uuid>,
    title: String,
    test_type: String,
    expected_result: String,
}

#[derive(Debug, Deserialize)]
struct ExecuteCase {
    status: String,
    actual_result: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateDefect {
    test_case_id: Uuid,
    title: String,
    description: String,
    severity: String,
}

#[derive(Debug, Deserialize)]
struct ResolveDefect {
    status: String,
    resolution: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/phases/{phase_id}/test", get(load))
        .route("/phases/{phase_id}/test/cases", post(create_case))
        .route(
            "/phases/{phase_id}/test/cases/{case_id}",
            patch(execute_case),
        )
        .route("/phases/{phase_id}/test/defects", post(create_defect))
        .route(
            "/phases/{phase_id}/test/defects/{defect_id}",
            patch(resolve_defect),
        )
        .route(
            "/phases/{phase_id}/test/artifacts/TEST_REPORT",
            post(generate_report),
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
        "SELECT ph.project_id,wm.role::text,ph.status::text FROM phases ph JOIN projects p ON p.id=ph.project_id AND p.deleted_at IS NULL JOIN workspace_members wm ON wm.workspace_id=p.workspace_id WHERE ph.id=$1 AND ph.phase_type='TEST' AND wm.user_id=$2",
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

async fn criterion(
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

async fn refresh(
    tx: &mut Transaction<'_, Postgres>,
    phase_id: Uuid,
    project_id: Uuid,
    user_id: Uuid,
) -> Result<(), StatusCode> {
    let build_plan=sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM phases b JOIN deliverables d ON d.phase_id=b.id AND d.type='BUILD_BUILD_PLAN' AND d.status='READY' AND d.deleted_at IS NULL WHERE b.project_id=$1 AND b.phase_type='BUILD' AND b.status='VALIDATED')").bind(project_id).fetch_one(&mut **tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    let (total, traced, not_run, not_passed)=sqlx::query_as::<_,(i64,i64,i64,i64)>("SELECT COUNT(*),COUNT(*) FILTER(WHERE requirement_id IS NOT NULL OR task_id IS NOT NULL),COUNT(*) FILTER(WHERE status='NOT_RUN'),COUNT(*) FILTER(WHERE status<>'PASSED') FROM test_cases WHERE phase_id=$1 AND deleted_at IS NULL").bind(phase_id).fetch_one(&mut **tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    let blocking=sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM test_defects WHERE phase_id=$1 AND severity='BLOCKING' AND status='OPEN' AND deleted_at IS NULL)").bind(phase_id).fetch_one(&mut **tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    for (code, done) in [
        ("build_plan_available", build_plan),
        ("test_cases_defined", total > 0),
        ("test_traceability_complete", total > 0 && total == traced),
        ("tests_executed", total > 0 && not_run == 0),
        ("tests_passing", total > 0 && not_passed == 0),
        ("no_blocking_defects", !blocking),
    ] {
        criterion(tx, phase_id, code, done, user_id).await?;
    }
    sqlx::query("UPDATE phases SET status='IN_PROGRESS',started_at=COALESCE(started_at,now()),updated_at=now() WHERE id=$1 AND status='AVAILABLE'").bind(phase_id).execute(&mut **tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}

async fn workspace(
    state: &AppState,
    phase_id: Uuid,
    project_id: Uuid,
) -> Result<TestWorkspace, StatusCode> {
    let build_plan_ready=sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM phases b JOIN deliverables d ON d.phase_id=b.id AND d.type='BUILD_BUILD_PLAN' AND d.status='READY' AND d.deleted_at IS NULL WHERE b.project_id=$1 AND b.phase_type='BUILD' AND b.status='VALIDATED')").bind(project_id).fetch_one(&state.db).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    let requirements=sqlx::query_as::<_,(Uuid,String,String)>("SELECT id,code,title FROM requirements WHERE project_id=$1 AND deleted_at IS NULL ORDER BY code").bind(project_id).fetch_all(&state.db).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?.into_iter().map(|x|Link{id:x.0,code:x.1,title:x.2}).collect();
    let tasks = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT id,code,title FROM tasks WHERE project_id=$1 AND deleted_at IS NULL ORDER BY code",
    )
    .bind(project_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .into_iter()
    .map(|x| Link {
        id: x.0,
        code: x.1,
        title: x.2,
    })
    .collect();
    let test_cases=sqlx::query_as::<_,TestCase>("SELECT id,requirement_id,task_id,code,title,test_type,expected_result,actual_result,status FROM test_cases WHERE phase_id=$1 AND deleted_at IS NULL ORDER BY created_at").bind(phase_id).fetch_all(&state.db).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    let defects=sqlx::query_as::<_,Defect>("SELECT id,test_case_id,title,description,severity,status,resolution FROM test_defects WHERE phase_id=$1 AND deleted_at IS NULL ORDER BY created_at").bind(phase_id).fetch_all(&state.db).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    let total = test_cases.len() as i64;
    let executed = test_cases.iter().filter(|x| x.status != "NOT_RUN").count() as i64;
    let passed = test_cases.iter().filter(|x| x.status == "PASSED").count() as i64;
    let failed = test_cases.iter().filter(|x| x.status == "FAILED").count() as i64;
    let blocked = test_cases.iter().filter(|x| x.status == "BLOCKED").count() as i64;
    let artifacts=sqlx::query_scalar::<_,String>("SELECT substring(type FROM 6) FROM deliverables WHERE phase_id=$1 AND type LIKE 'TEST_%' AND deleted_at IS NULL ORDER BY type").bind(phase_id).fetch_all(&state.db).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(TestWorkspace {
        build_plan_ready,
        requirements,
        tasks,
        test_cases,
        defects,
        progress: Progress {
            total,
            executed,
            passed,
            failed,
            blocked,
            percent: if total == 0 {
                0
            } else {
                executed * 100 / total
            },
            pass_rate: if executed == 0 {
                0
            } else {
                passed * 100 / executed
            },
        },
        artifacts,
    })
}

async fn load(
    State(state): State<AppState>,
    Path(phase_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<TestWorkspace>, StatusCode> {
    let (project_id, _, _) = access(&state, phase_id, user(&headers, &state)?).await?;
    Ok(Json(workspace(&state, phase_id, project_id).await?))
}

fn test_type(value: &str) -> bool {
    matches!(
        value,
        "UNIT" | "INTEGRATION" | "E2E" | "SECURITY" | "PERFORMANCE" | "MANUAL"
    )
}

async fn create_case(
    State(state): State<AppState>,
    Path(phase_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateCase>,
) -> Result<(StatusCode, Json<TestWorkspace>), StatusCode> {
    let user_id = user(&headers, &state)?;
    let (project_id, role, status) = access(&state, phase_id, user_id).await?;
    writable(&role, &status)?;
    let kind = input.test_type.to_uppercase();
    if input.title.trim().is_empty()
        || input.expected_result.trim().is_empty()
        || !test_type(&kind)
        || (input.requirement_id.is_none() && input.task_id.is_none())
    {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if let Some(id) = input.requirement_id {
        let ok=sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM requirements WHERE id=$1 AND project_id=$2 AND deleted_at IS NULL)").bind(id).bind(project_id).fetch_one(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
        if !ok {
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
    }
    if let Some(id) = input.task_id {
        let ok=sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM tasks WHERE id=$1 AND project_id=$2 AND deleted_at IS NULL)").bind(id).bind(project_id).fetch_one(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
        if !ok {
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
    }
    sqlx::query_scalar::<_, Uuid>("SELECT id FROM phases WHERE id=$1 FOR UPDATE")
        .bind(phase_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let current=sqlx::query_scalar::<_,i32>("SELECT COALESCE(MAX(CASE WHEN code~'^TEST-[0-9]+$' THEN substring(code FROM 6)::int END),0) FROM test_cases WHERE phase_id=$1").bind(phase_id).fetch_one(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("INSERT INTO test_cases(phase_id,requirement_id,task_id,code,title,test_type,expected_result,created_by)VALUES($1,$2,$3,$4,$5,$6,$7,$8)").bind(phase_id).bind(input.requirement_id).bind(input.task_id).bind(format!("TEST-{:03}",current+1)).bind(input.title.trim()).bind(kind).bind(input.expected_result.trim()).bind(user_id).execute(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    refresh(&mut tx, phase_id, project_id, user_id).await?;
    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((
        StatusCode::CREATED,
        Json(workspace(&state, phase_id, project_id).await?),
    ))
}

async fn execute_case(
    State(state): State<AppState>,
    Path((phase_id, case_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(input): Json<ExecuteCase>,
) -> Result<Json<TestWorkspace>, StatusCode> {
    let user_id = user(&headers, &state)?;
    let (project_id, role, phase_status) = access(&state, phase_id, user_id).await?;
    writable(&role, &phase_status)?;
    let status = input.status.to_uppercase();
    if !matches!(status.as_str(), "NOT_RUN" | "PASSED" | "FAILED" | "BLOCKED") {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let changed=sqlx::query("UPDATE test_cases SET status=$3,actual_result=$4,executed_by=CASE WHEN $3='NOT_RUN' THEN NULL ELSE $5 END,executed_at=CASE WHEN $3='NOT_RUN' THEN NULL ELSE now() END,updated_at=now() WHERE id=$1 AND phase_id=$2 AND deleted_at IS NULL").bind(case_id).bind(phase_id).bind(status).bind(input.actual_result).bind(user_id).execute(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    if changed.rows_affected() != 1 {
        return Err(StatusCode::NOT_FOUND);
    }
    refresh(&mut tx, phase_id, project_id, user_id).await?;
    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(workspace(&state, phase_id, project_id).await?))
}

async fn create_defect(
    State(state): State<AppState>,
    Path(phase_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateDefect>,
) -> Result<(StatusCode, Json<TestWorkspace>), StatusCode> {
    let user_id = user(&headers, &state)?;
    let (project_id, role, status) = access(&state, phase_id, user_id).await?;
    writable(&role, &status)?;
    let severity = input.severity.to_uppercase();
    if input.title.trim().is_empty()
        || input.description.trim().is_empty()
        || !matches!(severity.as_str(), "BLOCKING" | "NON_BLOCKING")
    {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let exists=sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM test_cases WHERE id=$1 AND phase_id=$2 AND deleted_at IS NULL)").bind(input.test_case_id).bind(phase_id).fetch_one(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    if !exists {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    sqlx::query("INSERT INTO test_defects(phase_id,test_case_id,title,description,severity,created_by)VALUES($1,$2,$3,$4,$5,$6)").bind(phase_id).bind(input.test_case_id).bind(input.title.trim()).bind(input.description.trim()).bind(severity).bind(user_id).execute(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    refresh(&mut tx, phase_id, project_id, user_id).await?;
    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((
        StatusCode::CREATED,
        Json(workspace(&state, phase_id, project_id).await?),
    ))
}

async fn resolve_defect(
    State(state): State<AppState>,
    Path((phase_id, defect_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(input): Json<ResolveDefect>,
) -> Result<Json<TestWorkspace>, StatusCode> {
    let user_id = user(&headers, &state)?;
    let (project_id, role, phase_status) = access(&state, phase_id, user_id).await?;
    writable(&role, &phase_status)?;
    let status = input.status.to_uppercase();
    if !matches!(status.as_str(), "OPEN" | "RESOLVED" | "ACCEPTED")
        || status != "OPEN"
            && input
                .resolution
                .as_deref()
                .is_none_or(|x| x.trim().is_empty())
    {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let changed=sqlx::query("UPDATE test_defects SET status=$3,resolution=$4,updated_at=now() WHERE id=$1 AND phase_id=$2 AND deleted_at IS NULL").bind(defect_id).bind(phase_id).bind(status).bind(input.resolution).execute(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    if changed.rows_affected() != 1 {
        return Err(StatusCode::NOT_FOUND);
    }
    refresh(&mut tx, phase_id, project_id, user_id).await?;
    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(workspace(&state, phase_id, project_id).await?))
}

async fn generate_report(
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
    refresh(&mut tx, phase_id, project_id, user_id).await?;
    let ready=sqlx::query_scalar::<_,bool>("SELECT COUNT(*)=6 FROM validation_criteria WHERE phase_id=$1 AND code IN ('build_plan_available','test_cases_defined','test_traceability_complete','tests_executed','tests_passing','no_blocking_defects') AND completed").bind(phase_id).fetch_one(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    if !ready {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let content = serde_json::to_value(workspace(&state, phase_id, project_id).await?)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let changed=sqlx::query("UPDATE deliverables SET content=$2,status='READY',version=version+1,updated_at=now() WHERE phase_id=$1 AND type='TEST_TEST_REPORT' AND deleted_at IS NULL").bind(phase_id).bind(&content).execute(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    if changed.rows_affected() == 0 {
        sqlx::query("INSERT INTO deliverables(phase_id,title,type,content,status,created_by)VALUES($1,'Test Report','TEST_TEST_REPORT',$2,'READY',$3)").bind(phase_id).bind(&content).bind(user_id).execute(&mut*tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    criterion(&mut tx, phase_id, "test_report_ready", true, user_id).await?;
    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(content))
}
