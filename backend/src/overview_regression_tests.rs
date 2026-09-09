use axum::{Router, body::Body, http::{Request, StatusCode, header}};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;
use uuid::Uuid;
use crate::{app, state::AppState};

async fn request(app: &Router, method: &str, uri: &str, token: Option<&str>, body: Option<Value>) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token { builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}")); }
    if body.is_some() { builder = builder.header(header::CONTENT_TYPE, "application/json"); }
    let response = app.clone().oneshot(builder.body(body.map(|v| Body::from(v.to_string())).unwrap_or_else(Body::empty)).unwrap()).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, if bytes.is_empty() { Value::Null } else { serde_json::from_slice(&bytes).unwrap() })
}

#[tokio::test]
async fn overview_is_200_when_model_is_current_phase() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for tests");
    let db = PgPoolOptions::new().max_connections(5).connect(&database_url).await.unwrap();
    sqlx::migrate!().run(&db).await.unwrap();
    let app = app(AppState { db, jwt_secret: b"ci-overview-regression-secret-32-bytes".to_vec() });
    let email = format!("overview-{}@horus.test", Uuid::now_v7());
    let (status, auth) = request(&app, "POST", "/api/v1/auth/register", None, Some(json!({"email":email,"password":"integration-password-123","display_name":"Regression"}))).await;
    assert_eq!(status, StatusCode::CREATED);
    let token = auth["access_token"].as_str().unwrap();
    let (_, workspaces) = request(&app, "GET", "/api/v1/workspaces", Some(token), None).await;
    let workspace_id = workspaces[0]["id"].as_str().unwrap();
    let (status, project) = request(&app, "POST", "/api/v1/projects", Some(token), Some(json!({"workspace_id":workspace_id,"name":"PRISM regression","description":"Historical project"}))).await;
    assert_eq!(status, StatusCode::CREATED);
    let project_id = project["id"].as_str().unwrap();
    let (_, overview) = request(&app, "GET", &format!("/api/v1/projects/{project_id}/overview"), Some(token), None).await;
    let analyze_id = overview["phases"][0]["id"].as_str().unwrap();
    let (status, _) = request(&app, "PUT", &format!("/api/v1/phases/{analyze_id}/analyze"), Some(token), Some(json!({"problem":"Trouver le bon professionnel","target_audiences":["Particulier"],"target_details":"Besoin précis","value_proposition":"Mise en relation pertinente","success_objectives":["Obtenir une proposition"],"budget":"Limité","deadline":"3 mois","platform":"Web","special_constraints":"Confiance et vie privée","constraints_unknown":false,"mvp_features":["Publier un besoin","Recevoir une proposition"]}))).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = request(&app, "POST", &format!("/api/v1/phases/{analyze_id}/validate"), Some(token), Some(json!({"comment":"Analyse terminée"}))).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, overview) = request(&app, "GET", &format!("/api/v1/projects/{project_id}/overview"), Some(token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(overview["next_action"]["phase"], "MODEL");
    assert_eq!(overview["next_action"]["step"], "entities_defined");
}
