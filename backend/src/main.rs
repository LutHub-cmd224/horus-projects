mod auth;
mod phases;
mod projects;
mod state;
mod workspaces;

use axum::{Json, Router, routing::get};
use serde::Serialize;
use sqlx::postgres::PgPoolOptions;
use state::AppState;
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "horus-api",
    })
}

fn app(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/health", get(health))
        .nest("/api/v1/auth", auth::routes::router())
        .nest("/api/v1/workspaces", workspaces::routes::router())
        .nest("/api/v1/projects", projects::routes::router())
        .nest("/api/v1", phases::routes::router())
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "horus_api=debug,tower_http=debug".into()),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let jwt_secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .expect("failed to connect to PostgreSQL");
    sqlx::migrate!()
        .run(&db)
        .await
        .expect("failed to run database migrations");

    let state = AppState {
        db,
        jwt_secret: jwt_secret.into_bytes(),
    };
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind API listener");
    tracing::info!(%addr, "HORUS API listening");
    axum::serve(listener, app(state))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("API server failed");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = terminate => {}, }
}
