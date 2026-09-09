use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;
use uuid::Uuid;

use crate::{app, state::AppState};

async fn json_request(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    if body.is_some() {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
    }
    let payload = body
        .map(|value| Body::from(value.to_string()))
        .unwrap_or_else(Body::empty);
    let response = app
        .clone()
        .oneshot(builder.body(payload).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, value)
}

#[tokio::test]
async fn register_create_project_and_load_overview() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for tests");
    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .unwrap();
    sqlx::migrate!().run(&db).await.unwrap();

    let app = app(AppState {
        db: db.clone(),
        jwt_secret: b"ci-integration-secret-at-least-32-bytes".to_vec(),
    });

    let email = format!("integration-{}@horus.test", Uuid::now_v7());
    let (status, auth) = json_request(
        &app,
        "POST",
        "/api/v1/auth/register",
        None,
        Some(json!({
            "email": email,
            "password": "integration-password-123",
            "display_name": "Integration Test"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let access_token = auth["access_token"].as_str().unwrap();

    let (status, workspaces) =
        json_request(&app, "GET", "/api/v1/workspaces", Some(access_token), None).await;
    assert_eq!(status, StatusCode::OK);
    let workspace_id = workspaces[0]["id"].as_str().unwrap();

    let (status, project) = json_request(
        &app,
        "POST",
        "/api/v1/projects",
        Some(access_token),
        Some(json!({
            "workspace_id": workspace_id,
            "name": "Integration Project",
            "description": "Full API smoke path"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let project_id = project["id"].as_str().unwrap();

    let (status, overview) = json_request(
        &app,
        "GET",
        &format!("/api/v1/projects/{project_id}/overview"),
        Some(access_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(overview["project"]["name"], "Integration Project");
    assert_eq!(overview["phases"].as_array().unwrap().len(), 6);
    assert_eq!(overview["phases"][0]["phase_type"], "ANALYZE");
    assert_eq!(overview["phases"][0]["status"], "AVAILABLE");
    assert_eq!(overview["phases"][0]["required_criteria"], 6);
    assert_eq!(overview["phases"][1]["phase_type"], "MODEL");
    assert_eq!(overview["phases"][1]["required_criteria"], 6);
    assert_eq!(overview["phases"][2]["phase_type"], "DESIGN");
    assert_eq!(overview["phases"][2]["required_criteria"], 7);
    assert_eq!(overview["phases"][3]["phase_type"], "BUILD");
    assert_eq!(overview["phases"][3]["required_criteria"], 6);
    assert_eq!(overview["phases"][4]["phase_type"], "TEST");
    assert_eq!(overview["phases"][4]["required_criteria"], 7);
    assert_eq!(overview["requirement_count"], 0);
    assert_eq!(overview["open_task_count"], 0);
    assert_eq!(overview["decision_count"], 0);

    let model_phase_id = Uuid::parse_str(overview["phases"][1]["id"].as_str().unwrap()).unwrap();
    sqlx::query("UPDATE phases SET status = 'AVAILABLE' WHERE id = $1")
        .bind(model_phase_id)
        .execute(&db)
        .await
        .unwrap();
    let model = json!({
        "entities": [
            {"conceptual_name":"User","logical_name":"User","physical_name":"users","description":null,"attributes":[]},
            {"conceptual_name":"Project","logical_name":"Project","physical_name":"projects","description":null,"attributes":[]}
        ],
        "relationships": [{"name":"owns","source_entity":"User","target_entity":"Project","source_cardinality":"1","target_cardinality":"0..N","description":null}],
        "business_rules": [],
        "artifacts": []
    });
    let (status, _) = json_request(
        &app,
        "PUT",
        &format!("/api/v1/phases/{model_phase_id}/model"),
        Some(access_token),
        Some(model),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    sqlx::query("DELETE FROM validation_criteria WHERE phase_id = $1 AND code = 'mcd_defined'")
        .bind(model_phase_id)
        .execute(&db)
        .await
        .unwrap();
    let (status, _) = json_request(
        &app,
        "POST",
        &format!("/api/v1/phases/{model_phase_id}/model/artifacts/MCD"),
        Some(access_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    let deliverable_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM deliverables WHERE phase_id = $1 AND type = 'MODEL_MCD'",
    )
    .bind(model_phase_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(
        deliverable_count, 0,
        "artifact insert must roll back with its criterion update"
    );

    let design_phase_id = Uuid::parse_str(overview["phases"][2]["id"].as_str().unwrap()).unwrap();
    sqlx::query("UPDATE phases SET status = 'AVAILABLE' WHERE id = $1")
        .bind(design_phase_id)
        .execute(&db)
        .await
        .unwrap();
    let design = json!({
        "components": [
            {"name":"Web","category":"FRONTEND","responsibility":"User experience","technology":"React"},
            {"name":"API","category":"BACKEND","responsibility":"Business rules","technology":"Rust/Axum"},
            {"name":"Database","category":"DATABASE","responsibility":"Persistence","technology":"PostgreSQL"}
        ],
        "connections": [
            {"source_name":"Web","target_name":"API","protocol":"HTTPS/JSON","description":"Authenticated API"},
            {"source_name":"API","target_name":"Database","protocol":"SQL","description":"SQLx queries"}
        ],
        "ux_flows": [{"title":"Create project","description":"Guide the owner through setup"}],
        "features": [{"title":"Design workspace","description":"Capture design decisions"}],
        "api_contracts": [{"title":"Design API","description":"Load, save and generate artifacts"}],
        "security_controls": [{"title":"Authorization","description":"Enforce workspace roles"}],
        "decisions": [{"title":"ADR-001","description":"Keep the existing modular monolith","status":"PROPOSED"}],
        "artifacts": []
    });
    let (status, saved_design) = json_request(
        &app,
        "PUT",
        &format!("/api/v1/phases/{design_phase_id}/design"),
        Some(access_token),
        Some(design),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved_design["components"].as_array().unwrap().len(), 3);

    let technology_decisions_accepted = sqlx::query_scalar::<_, bool>(
        "SELECT completed FROM validation_criteria WHERE phase_id = $1 AND code = 'technology_decisions_accepted'",
    )
    .bind(design_phase_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(!technology_decisions_accepted);

    let (status, _) = json_request(
        &app,
        "POST",
        &format!("/api/v1/phases/{design_phase_id}/design/artifacts/DESIGN_PACK"),
        Some(access_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let mut accepted_design = saved_design;
    accepted_design["decisions"][0]["status"] = json!("ACCEPTED");
    let (status, _) = json_request(
        &app,
        "PUT",
        &format!("/api/v1/phases/{design_phase_id}/design"),
        Some(access_token),
        Some(accepted_design),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let technology_decisions_accepted = sqlx::query_scalar::<_, bool>(
        "SELECT completed FROM validation_criteria WHERE phase_id = $1 AND code = 'technology_decisions_accepted'",
    )
    .bind(design_phase_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(technology_decisions_accepted);

    let (status, _) = json_request(
        &app,
        "POST",
        &format!("/api/v1/phases/{design_phase_id}/design/artifacts/DESIGN_PACK"),
        Some(access_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let design_pack_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM deliverables WHERE phase_id = $1 AND type = 'DESIGN_DESIGN_PACK' AND status = 'READY'",
    )
    .bind(design_phase_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(design_pack_count, 1);
    let design_pack_ready = sqlx::query_scalar::<_, bool>(
        "SELECT completed FROM validation_criteria WHERE phase_id = $1 AND code = 'design_pack_ready'",
    )
    .bind(design_phase_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(design_pack_ready);

    let (status, _) = json_request(
        &app,
        "POST",
        &format!("/api/v1/phases/{design_phase_id}/validate"),
        Some(access_token),
        Some(json!({"comment":"Design ready for Build"})),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let build_phase_id = Uuid::parse_str(overview["phases"][3]["id"].as_str().unwrap()).unwrap();
    let (status, requirement) = json_request(
        &app,
        "POST",
        &format!("/api/v1/projects/{project_id}/requirements"),
        Some(access_token),
        Some(json!({
            "code":"REQ-BUILD-001",
            "title":"Provide a traceable Build workspace",
            "requirement_type":"FUNCTIONAL",
            "priority":"HIGH",
            "status":"APPROVED"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let requirement_id = requirement["id"].as_str().unwrap();

    let (status, build) = json_request(
        &app,
        "POST",
        &format!("/api/v1/phases/{build_phase_id}/build/tasks"),
        Some(access_token),
        Some(json!({
            "requirement_id":requirement_id,
            "title":"Implement Build workspace",
            "priority":"HIGH"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(build["tasks"].as_array().unwrap().len(), 1);
    let task_id = build["tasks"][0]["id"].as_str().unwrap();

    let (status, _) = json_request(
        &app,
        "PUT",
        &format!("/api/v1/phases/{build_phase_id}/build/github"),
        Some(access_token),
        Some(json!({
            "repository_url":"https://github.com/example/horus",
            "default_branch":"main",
            "integration_strategy":"PULL_REQUEST",
            "ci_required":false,
            "ci_configured":false,
            "definition_of_done":["Tests pass","Review approved"]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let github_ready = sqlx::query_scalar::<_, bool>(
        "SELECT completed FROM validation_criteria WHERE phase_id=$1 AND code='github_ready'",
    )
    .bind(build_phase_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(github_ready, "optional CI must not block GitHub readiness");

    let (status, _) = json_request(
        &app,
        "PUT",
        &format!("/api/v1/phases/{build_phase_id}/build/github"),
        Some(access_token),
        Some(json!({
            "repository_url":"https://github.com/example/horus",
            "default_branch":"main",
            "integration_strategy":"PULL_REQUEST",
            "ci_required":true,
            "ci_configured":false,
            "definition_of_done":["Tests pass","Review approved"]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let github_ready = sqlx::query_scalar::<_, bool>(
        "SELECT completed FROM validation_criteria WHERE phase_id=$1 AND code='github_ready'",
    )
    .bind(build_phase_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(!github_ready, "required CI needs explicit configuration");

    let (status, _) = json_request(
        &app,
        "PUT",
        &format!("/api/v1/phases/{build_phase_id}/build/github"),
        Some(access_token),
        Some(json!({
            "repository_url":"https://github.com/example/horus",
            "default_branch":"main",
            "integration_strategy":"PULL_REQUEST",
            "ci_required":true,
            "ci_configured":true,
            "definition_of_done":["Tests pass","Review approved"]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let github_ready = sqlx::query_scalar::<_, bool>(
        "SELECT completed FROM validation_criteria WHERE phase_id=$1 AND code='github_ready'",
    )
    .bind(build_phase_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(github_ready, "configured required CI completes readiness");

    let (status, _) = json_request(
        &app,
        "POST",
        &format!("/api/v1/phases/{build_phase_id}/build/decisions"),
        Some(access_token),
        Some(json!({
            "title":"Feature branch workflow",
            "decision":"Every change is reviewed in a pull request",
            "status":"ACCEPTED"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, _) = json_request(
        &app,
        "POST",
        &format!("/api/v1/phases/{build_phase_id}/build/artifacts/BUILD_PLAN"),
        Some(access_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, build) = json_request(
        &app,
        "PATCH",
        &format!("/api/v1/phases/{build_phase_id}/build/tasks/{task_id}"),
        Some(access_token),
        Some(json!({"status":"DONE"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(build["progress"]["percent"], 100);
    assert!(
        build["artifacts"]
            .as_array()
            .unwrap()
            .contains(&json!("BUILD_PLAN"))
    );

    let (status, _) = json_request(
        &app,
        "POST",
        &format!("/api/v1/phases/{build_phase_id}/validate"),
        Some(access_token),
        Some(json!({"comment":"Build Plan ready for Test"})),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = json_request(
        &app,
        "PATCH",
        &format!("/api/v1/phases/{build_phase_id}/build/tasks/{task_id}"),
        Some(access_token),
        Some(json!({"status":"TODO"})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let test_phase_id = Uuid::parse_str(overview["phases"][4]["id"].as_str().unwrap()).unwrap();
    let deploy_phase_id = Uuid::parse_str(overview["phases"][5]["id"].as_str().unwrap()).unwrap();
    let (status, tests) = json_request(
        &app,
        "POST",
        &format!("/api/v1/phases/{test_phase_id}/test/cases"),
        Some(access_token),
        Some(json!({
            "requirement_id":requirement_id,
            "task_id":task_id,
            "title":"Build workspace can be completed",
            "test_type":"E2E",
            "expected_result":"The workflow reaches a validated Build Plan"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let case_id = tests["test_cases"][0]["id"].as_str().unwrap();

    let (status, _) = json_request(
        &app,
        "PATCH",
        &format!("/api/v1/phases/{test_phase_id}/test/cases/{case_id}"),
        Some(access_token),
        Some(json!({"status":"BLOCKED","actual_result":"Deployment gate unavailable"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, tests) = json_request(
        &app,
        "POST",
        &format!("/api/v1/phases/{test_phase_id}/test/defects"),
        Some(access_token),
        Some(json!({
            "test_case_id":case_id,
            "title":"Deployment gate unavailable",
            "description":"The workflow cannot continue",
            "severity":"BLOCKING"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let defect_id = tests["defects"][0]["id"].as_str().unwrap();
    let (status, _) = json_request(
        &app,
        "POST",
        &format!("/api/v1/phases/{test_phase_id}/test/artifacts/TEST_REPORT"),
        Some(access_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, _) = json_request(
        &app,
        "PATCH",
        &format!("/api/v1/phases/{test_phase_id}/test/defects/{defect_id}"),
        Some(access_token),
        Some(json!({"status":"RESOLVED","resolution":"Gate restored and verified"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, tests) = json_request(
        &app,
        "PATCH",
        &format!("/api/v1/phases/{test_phase_id}/test/cases/{case_id}"),
        Some(access_token),
        Some(json!({"status":"PASSED","actual_result":"Workflow completed"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tests["progress"]["pass_rate"], 100);

    let (status, _) = json_request(
        &app,
        "POST",
        &format!("/api/v1/phases/{test_phase_id}/test/defects"),
        Some(access_token),
        Some(json!({
            "test_case_id":case_id,
            "title":"Minor visual discrepancy",
            "description":"Does not prevent the expected workflow",
            "severity":"NON_BLOCKING"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, _) = json_request(
        &app,
        "POST",
        &format!("/api/v1/phases/{test_phase_id}/test/artifacts/TEST_REPORT"),
        Some(access_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = json_request(
        &app,
        "POST",
        &format!("/api/v1/phases/{test_phase_id}/test/artifacts/TEST_REPORT"),
        Some(access_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let report_version = sqlx::query_scalar::<_, i32>(
        "SELECT version FROM deliverables WHERE phase_id=$1 AND type='TEST_TEST_REPORT' AND deleted_at IS NULL",
    )
    .bind(test_phase_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(report_version, 2);
    let (status, _) = json_request(
        &app,
        "POST",
        &format!("/api/v1/phases/{test_phase_id}/validate"),
        Some(access_token),
        Some(json!({"comment":"Test Report accepted"})),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let deploy_status =
        sqlx::query_scalar::<_, String>("SELECT status::text FROM phases WHERE id=$1")
            .bind(deploy_phase_id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(deploy_status, "AVAILABLE");

    let (status, _) = json_request(
        &app,
        "PATCH",
        &format!("/api/v1/phases/{test_phase_id}/test/cases/{case_id}"),
        Some(access_token),
        Some(json!({"status":"FAILED","actual_result":"Regression"})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}
