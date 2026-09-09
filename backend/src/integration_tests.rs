use axum::{Router, body::Body, http::{Request, StatusCode, header}};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;
use uuid::Uuid;
use crate::{app, state::AppState};

async fn json_request(app:&Router, method:&str, uri:&str, token:Option<&str>, body:Option<Value>) -> (StatusCode,Value) {
    let mut builder=Request::builder().method(method).uri(uri);
    if let Some(token)=token { builder=builder.header(header::AUTHORIZATION,format!("Bearer {token}")); }
    if body.is_some() { builder=builder.header(header::CONTENT_TYPE,"application/json"); }
    let payload=body.map(|v|Body::from(v.to_string())).unwrap_or_else(Body::empty);
    let response=app.clone().oneshot(builder.body(payload).unwrap()).await.unwrap();
    let status=response.status(); let bytes=response.into_body().collect().await.unwrap().to_bytes();
    let value=if bytes.is_empty(){Value::Null}else{serde_json::from_slice(&bytes).unwrap()}; (status,value)
}

#[tokio::test]
async fn overview_remains_available_after_analyze_advances_to_model() {
    let database_url=std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for tests");
    let db=PgPoolOptions::new().max_connections(5).connect(&database_url).await.unwrap();
    sqlx::migrate!().run(&db).await.unwrap();
    let app=app(AppState{db:db.clone(),jwt_secret:b"ci-integration-secret-at-least-32-bytes".to_vec()});
    let email=format!("overview-model-{}@horus.test",Uuid::now_v7());
    let (status,auth)=json_request(&app,"POST","/api/v1/auth/register",None,Some(json!({"email":email,"password":"integration-password-123","display_name":"Overview Model Test"}))).await;
    assert_eq!(status,StatusCode::CREATED); let token=auth["access_token"].as_str().unwrap();
    let (status,workspaces)=json_request(&app,"GET","/api/v1/workspaces",Some(token),None).await; assert_eq!(status,StatusCode::OK);
    let workspace_id=workspaces[0]["id"].as_str().unwrap();
    let (status,project)=json_request(&app,"POST","/api/v1/projects",Some(token),Some(json!({"workspace_id":workspace_id,"name":"PRISM regression","description":"Overview after Analyze"}))).await;
    assert_eq!(status,StatusCode::CREATED); let project_id=project["id"].as_str().unwrap();
    let (status,overview)=json_request(&app,"GET",&format!("/api/v1/projects/{project_id}/overview"),Some(token),None).await; assert_eq!(status,StatusCode::OK);
    let analyze_id=overview["phases"][0]["id"].as_str().unwrap();
    let (status,_)=json_request(&app,"PUT",&format!("/api/v1/phases/{analyze_id}/analyze"),Some(token),Some(json!({"problem":"People struggle to find the right professional","target_audiences":["Particulier"],"target_details":"People with a precise need","value_proposition":"Connect quickly with a relevant professional","success_objectives":["A useful introduction is made"],"budget":"Small","deadline":"Three months","platform":"Web","special_constraints":"Privacy and trust","constraints_unknown":false,"mvp_features":["Publish need","Receive proposal"]}))).await;
    assert_eq!(status,StatusCode::OK);
    let (status,_)=json_request(&app,"POST",&format!("/api/v1/phases/{analyze_id}/validate"),Some(token),Some(json!({"comment":"Analyze complete"}))).await; assert_eq!(status,StatusCode::NO_CONTENT);
    let (status,model_overview)=json_request(&app,"GET",&format!("/api/v1/projects/{project_id}/overview"),Some(token),None).await;
    assert_eq!(status,StatusCode::OK,"overview must not fail when current phase is MODEL");
    assert_eq!(model_overview["next_action"]["phase"],"MODEL");
    assert_eq!(model_overview["next_action"]["step"],"entities_defined");
}
