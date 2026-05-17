use std::sync::Arc;

use axum::{Json, Router, extract::State, routing::{get, post}};
use chrono::Utc;
use http::{HeaderValue, Method};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tower_http::{
    compression::CompressionLayer,
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};
use tracing::info;
use validator::Validate;
use uuid::Uuid;

use crate::{
    auth::PasswordService,
    email_verification::{EmailVerificationError, EmailVerificationService},
    error::{ApiResponse, AppError},
    extract::ValidatedJson,
    users::{self, NewUser},
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

#[derive(Debug, Deserialize, Validate)]
struct RegisterRequest {
    #[validate(email(message = "email must be a valid email address"))]
    email: String,
    #[validate(length(min = 8, max = 128, message = "password must be between 8 and 128 characters"))]
    password: String,
    #[validate(length(min = 3, max = 64, message = "display_name must be between 3 and 64 characters"))]
    display_name: String,
}

#[derive(Debug, Serialize)]
struct RegisterResponse {
    user_id: Uuid,
    email: String,
    display_name: String,
    email_verification_required: bool,
}

#[derive(Debug, Deserialize, Validate)]
struct VerifyEmailRequest {
    #[validate(length(min = 64, max = 64, message = "token must be a valid verification token"))]
    token: String,
}

#[derive(Debug, Serialize)]
struct VerifyEmailResponse {
    user_id: Uuid,
    email: String,
    email_verified: bool,
    verified_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, Deserialize, Validate)]
struct ResendVerificationRequest {
    #[validate(email(message = "email must be a valid email address"))]
    email: String,
}

#[derive(Debug, Serialize)]
struct ResendVerificationResponse {
    accepted: bool,
}

pub fn router(state: SharedAppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/auth/register", post(register))
        .route("/auth/verify-email", post(verify_email))
        .route("/auth/resend-verification", post(resend_verification))
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

async fn register(
    State(state): State<SharedAppState>,
    payload: ValidatedJson<RegisterRequest>,
) -> Result<Json<ApiResponse<RegisterResponse>>, AppError> {
    let email = normalize_email(&payload.email);
    let display_name = payload.display_name.trim().to_owned();

    if users::find_user_by_email(&state.db_pool, &email)
        .await
        .map_err(AppError::from)?
        .is_some()
    {
        return Err(AppError::conflict("email is already registered"));
    }

    let password_service = PasswordService::new()
        .map_err(|error| AppError::internal(format!("failed to initialize password service: {error}")))?;
    let password_hash = password_service
        .hash_password(payload.password.as_str())
        .map_err(|error| AppError::internal(format!("failed to hash password: {error}")))?;

    let mut transaction = state.db_pool.begin().await.map_err(AppError::from)?;
    let user = users::insert_user(
        &mut transaction,
        &NewUser {
            email: email.clone(),
            password_hash,
            display_name: display_name.clone(),
        },
    )
    .await
    .map_err(AppError::from)?;

    let verification = EmailVerificationService::new()
        .issue_for_user_in_tx(&mut transaction, user.id)
        .await
        .map_err(map_email_verification_error)?;

    transaction.commit().await.map_err(AppError::from)?;

    info!(
        user_id = %user.id,
        email = %user.email,
        verification_expires_at = %verification.expires_at,
        verification_token = %verification.token,
        "created user registration and issued email verification token"
    );

    Ok(Json(ApiResponse::new(RegisterResponse {
        user_id: user.id,
        email: user.email,
        display_name: user.display_name,
        email_verification_required: true,
    })))
}

async fn verify_email(
    State(state): State<SharedAppState>,
    payload: ValidatedJson<VerifyEmailRequest>,
) -> Result<Json<ApiResponse<VerifyEmailResponse>>, AppError> {
    let mut transaction = state.db_pool.begin().await.map_err(AppError::from)?;
    let verification = EmailVerificationService::new()
        .consume_token_in_tx(&mut transaction, payload.token.trim())
        .await
        .map_err(map_email_verification_error)?;

    let user = users::mark_email_verified(&mut transaction, verification.user_id, Utc::now())
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::bad_request("email verification token references an unknown user"))?;

    transaction.commit().await.map_err(AppError::from)?;

    Ok(Json(ApiResponse::new(VerifyEmailResponse {
        user_id: user.id,
        email: user.email,
        email_verified: user.email_verified_at.is_some(),
        verified_at: user.email_verified_at,
    })))
}

async fn resend_verification(
    State(state): State<SharedAppState>,
    payload: ValidatedJson<ResendVerificationRequest>,
) -> Result<Json<ApiResponse<ResendVerificationResponse>>, AppError> {
    let email = normalize_email(&payload.email);
    let Some(user) = users::find_user_by_email(&state.db_pool, &email)
        .await
        .map_err(AppError::from)? else {
        return Ok(Json(ApiResponse::new(ResendVerificationResponse {
            accepted: true,
        })));
    };

    if user.email_verified_at.is_some() {
        return Ok(Json(ApiResponse::new(ResendVerificationResponse {
            accepted: true,
        })));
    }

    let verification = EmailVerificationService::new()
        .issue_for_user(&state.db_pool, user.id)
        .await
        .map_err(map_email_verification_error)?;

    info!(
        user_id = %user.id,
        email = %user.email,
        verification_expires_at = %verification.expires_at,
        verification_token = %verification.token,
        "reissued email verification token"
    );

    Ok(Json(ApiResponse::new(ResendVerificationResponse {
        accepted: true,
    })))
}

fn build_cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_methods([Method::GET, Method::HEAD, Method::OPTIONS, Method::POST])
        .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _| {
            origin == HeaderValue::from_static("http://localhost:8080")
                || origin == HeaderValue::from_static("http://127.0.0.1:8080")
        }))
}

fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

fn map_email_verification_error(error: EmailVerificationError) -> AppError {
    match error {
        EmailVerificationError::Database(error) => AppError::from(error),
        EmailVerificationError::TokenNotFound
        | EmailVerificationError::TokenAlreadyUsed
        | EmailVerificationError::TokenExpired => AppError::bad_request(error.to_string()),
    }
}
