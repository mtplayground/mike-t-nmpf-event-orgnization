use std::sync::Arc;

use axum::{Json, Router, extract::State, routing::get};
use http::{HeaderValue, Method};
use serde::Serialize;
use sqlx::PgPool;
use tower_http::{
    compression::CompressionLayer,
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};

#[derive(Clone)]
pub struct AppState {
    pub db_pool: PgPool,
    pub object_storage: crate::object_storage::ObjectStorageClient,
}

pub type SharedAppState = Arc<AppState>;

#[derive(Debug, Clone, Serialize)]
struct HealthResponse {
    status: &'static str,
}

pub fn router(state: SharedAppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .with_state(state)
        .layer(CompressionLayer::new())
        .layer(build_cors_layer())
        .layer(TraceLayer::new_for_http())
}

async fn health(State(state): State<SharedAppState>) -> Json<HealthResponse> {
    let _db_pool = &state.db_pool;
    let _object_storage = &state.object_storage;
    Json(HealthResponse { status: "ok" })
}

fn build_cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_methods([Method::GET, Method::HEAD, Method::OPTIONS])
        .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _| {
            origin == HeaderValue::from_static("http://localhost:8080")
                || origin == HeaderValue::from_static("http://127.0.0.1:8080")
        }))
}
