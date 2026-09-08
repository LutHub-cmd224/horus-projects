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
        db,
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
    assert_eq!(overview["requirement_count"], 0);
    assert_eq!(overview["open_task_count"], 0);
    assert_eq!(overview["decision_count"], 0);
}
