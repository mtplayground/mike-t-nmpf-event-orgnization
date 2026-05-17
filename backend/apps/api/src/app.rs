use std::sync::Arc;

use axum::{Json, Router, extract::State, routing::{get, post}};
use http::{HeaderValue, Method};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tower_http::{
    compression::CompressionLayer,
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};
use validator::Validate;

use crate::{
    error::{ApiResponse, AppError},
    extract::ValidatedJson,
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

#[derive(Debug, Deserialize, Validate)]
struct ValidationProbe {
    #[validate(length(min = 3, max = 64, message = "display_name must be between 3 and 64 characters"))]
    display_name: String,
}

#[derive(Debug, Serialize)]
struct ValidationProbeResponse {
    accepted: bool,
    display_name: String,
}

pub fn router(state: SharedAppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/validation-probe", post(validation_probe))
        .with_state(state)
        .layer(CompressionLayer::new())
        .layer(build_cors_layer())
        .layer(TraceLayer::new_for_http())
}

async fn health(State(state): State<SharedAppState>) -> Json<ApiResponse<HealthResponse>> {
    let _db_pool = &state.db_pool;
    let _object_storage = &state.object_storage;
    Json(ApiResponse::new(HealthResponse { status: "ok" }))
}

async fn validation_probe(
    State(state): State<SharedAppState>,
    payload: ValidatedJson<ValidationProbe>,
) -> Result<Json<ApiResponse<ValidationProbeResponse>>, AppError> {
    let _db_pool = &state.db_pool;

    if payload.display_name.eq_ignore_ascii_case("forbidden") {
        return Err(AppError::bad_request(
            "display_name cannot use the reserved value 'forbidden'",
        ));
    }

    Ok(Json(ApiResponse::new(ValidationProbeResponse {
        accepted: true,
        display_name: payload.display_name.clone(),
    })))
}

fn build_cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_methods([Method::GET, Method::HEAD, Method::OPTIONS])
        .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _| {
            origin == HeaderValue::from_static("http://localhost:8080")
                || origin == HeaderValue::from_static("http://127.0.0.1:8080")
        }))
}
