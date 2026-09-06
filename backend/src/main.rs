mod auth;

use axum::{Json, Router, routing::get};
use serde::Serialize;
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;

#[derive(Serialize)]
struct HealthResponse { status: &'static str, service: &'static str }

async fn health() -> Json<HealthResponse> { Json(HealthResponse { status: "ok", service: "horus-api" }) }

fn app() -> Router {
    Router::new()
        .route("/api/v1/health", get(health))
        .nest("/api/v1/auth", auth::routes::router())
        .layer(TraceLayer::new_for_http())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "horus_api=debug,tower_http=debug".into())).init();
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = tokio::net::TcpListener::bind(addr).await.expect("failed to bind API listener");
    tracing::info!(%addr, "HORUS API listening");
    axum::serve(listener, app()).with_graceful_shutdown(shutdown_signal()).await.expect("API server failed");
}

async fn shutdown_signal() {
    let ctrl_c = async { tokio::signal::ctrl_c().await.expect("failed to install Ctrl+C handler"); };
    #[cfg(unix)]
    let terminate = async { tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("failed to install SIGTERM handler").recv().await; };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = terminate => {}, }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::{Request, StatusCode}};
    use tower::ServiceExt;
    #[tokio::test]
    async fn health_returns_ok() {
        let response = app().oneshot(Request::builder().uri("/api/v1/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
    #[tokio::test]
    async fn me_requires_authentication() {
        let response = app().oneshot(Request::builder().uri("/api/v1/auth/me").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
