use axum::{Json, Router, routing::get};
use http::{HeaderValue, Method};
use serde::Serialize;
use tower_http::{
    compression::CompressionLayer,
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};

#[derive(Debug, Clone, Serialize)]
struct HealthResponse {
    status: &'static str,
}

pub fn router() -> Router {
    Router::new()
        .route("/health", get(health))
        .layer(CompressionLayer::new())
        .layer(build_cors_layer())
        .layer(TraceLayer::new_for_http())
}

async fn health() -> Json<HealthResponse> {
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
