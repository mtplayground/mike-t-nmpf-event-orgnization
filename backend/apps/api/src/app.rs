use std::{collections::HashMap, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path, State},
    middleware,
    routing::{get, post},
};
use chrono::{DateTime, Duration, Utc};
use http::{HeaderValue, Method};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tokio::sync::Mutex;
use tower_http::{
    compression::CompressionLayer,
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};
use tracing::info;
use validator::Validate;
use uuid::Uuid;

use crate::{
    auth::{JwtService, PasswordService},
    auth_middleware,
    email::{EmailError, EmailService},
    email_verification::{EmailVerificationError, EmailVerificationService},
    error::{ApiResponse, AppError},
    event_images::{self, EventImageVariant},
    extract::{CurrentUser, ValidatedJson},
    password_reset::{PasswordResetError, PasswordResetService},
    refresh_tokens::{RefreshTokenError, RefreshTokenService},
    object_storage::{ObjectMetadata, ObjectStorageError},
    users::{self, NewUser},
};

const AVATAR_UPLOAD_MAX_SIZE_BYTES: i64 = 5 * 1024 * 1024;
const AVATAR_UPLOAD_URL_TTL_SECONDS: u64 = 900;
const EVENT_COVER_UPLOAD_MAX_SIZE_BYTES: i64 = 10 * 1024 * 1024;
const EVENT_COVER_UPLOAD_URL_TTL_SECONDS: u64 = 900;

#[derive(Clone)]
pub struct AppState {
    pub db_pool: PgPool,
    pub object_storage: crate::object_storage::ObjectStorageClient,
    pub password_service: PasswordService,
    pub jwt_service: JwtService,
    pub refresh_token_service: RefreshTokenService,
    pub email_service: EmailService,
    pub email_verification_service: EmailVerificationService,
    pub password_reset_service: PasswordResetService,
    pub login_rate_limiter: Arc<LoginRateLimiter>,
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

#[derive(Debug, Deserialize, Validate)]
struct ForgotPasswordRequest {
    #[validate(email(message = "email must be a valid email address"))]
    email: String,
}

#[derive(Debug, Serialize)]
struct ForgotPasswordResponse {
    accepted: bool,
}

#[derive(Debug, Deserialize, Validate)]
struct ResetPasswordRequest {
    #[validate(length(min = 64, max = 64, message = "token must be a valid password reset token"))]
    token: String,
    #[validate(length(min = 8, max = 128, message = "password must be between 8 and 128 characters"))]
    password: String,
}

#[derive(Debug, Serialize)]
struct ResetPasswordResponse {
    user_id: Uuid,
    email: String,
    password_reset: bool,
}

#[derive(Debug, Deserialize, Validate)]
struct LoginRequest {
    #[validate(email(message = "email must be a valid email address"))]
    email: String,
    #[validate(length(min = 8, max = 128, message = "password must be between 8 and 128 characters"))]
    password: String,
}

#[derive(Debug, Deserialize, Validate)]
struct RefreshRequest {
    #[validate(length(min = 32, message = "refresh_token must be provided"))]
    refresh_token: String,
}

#[derive(Debug, Deserialize, Validate)]
struct LogoutRequest {
    #[validate(length(min = 32, message = "refresh_token must be provided"))]
    refresh_token: String,
}

#[derive(Debug, Serialize)]
struct AuthUserResponse {
    id: Uuid,
    email: String,
    display_name: String,
    email_verified: bool,
}

#[derive(Debug, Serialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    token_type: &'static str,
    expires_in_seconds: i64,
    refresh_expires_in_seconds: i64,
    user: AuthUserResponse,
}

#[derive(Debug, Serialize)]
struct LogoutResponse {
    revoked: bool,
}

#[derive(Debug, Serialize)]
struct CurrentUserResponse {
    id: Uuid,
    email: String,
    display_name: String,
    bio: Option<String>,
    avatar_object_key: Option<String>,
    email_verified: bool,
}

#[derive(Debug, Deserialize, Validate)]
struct UpdateProfileRequest {
    #[validate(length(min = 3, max = 64, message = "display_name must be between 3 and 64 characters"))]
    display_name: Option<String>,
    #[validate(length(max = 500, message = "bio must be 500 characters or fewer"))]
    bio: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
struct AvatarUploadUrlRequest {
    #[validate(length(min = 1, max = 128, message = "content_type must be provided"))]
    content_type: String,
    size_bytes: i64,
}

#[derive(Debug, Serialize)]
struct AvatarUploadUrlResponse {
    object_key: String,
    method: String,
    upload_url: String,
    headers: Vec<PresignedHeader>,
    expires_in_seconds: u64,
    max_size_bytes: i64,
}

#[derive(Debug, Serialize)]
struct PresignedHeader {
    name: String,
    value: String,
}

#[derive(Debug, Deserialize, Validate)]
struct ConfirmAvatarUploadRequest {
    #[validate(length(min = 1, max = 512, message = "object_key must be provided"))]
    object_key: String,
}

#[derive(Debug, Deserialize, Validate)]
struct EventCoverUploadUrlRequest {
    #[validate(length(min = 1, max = 128, message = "content_type must be provided"))]
    content_type: String,
    size_bytes: i64,
}

#[derive(Debug, Serialize)]
struct EventCoverUploadUrlResponse {
    event_id: Uuid,
    object_key: String,
    method: String,
    upload_url: String,
    headers: Vec<PresignedHeader>,
    expires_in_seconds: u64,
    max_size_bytes: i64,
}

#[derive(Debug, Deserialize, Validate)]
struct ConfirmEventCoverUploadRequest {
    #[validate(length(min = 1, max = 512, message = "object_key must be provided"))]
    object_key: String,
    width: i32,
    height: i32,
}

#[derive(Debug, Serialize)]
struct ConfirmEventCoverUploadResponse {
    event_id: Uuid,
    object_key: String,
    variant: &'static str,
    width: i32,
    height: i32,
    bytes: i64,
}

pub fn router(state: SharedAppState) -> Router {
    let protected_routes = Router::new()
        .route("/auth/me", get(current_user))
        .route("/me", get(read_profile).patch(update_profile))
        .route("/me/avatar/upload-url", post(create_avatar_upload_url))
        .route("/me/avatar/confirm", post(confirm_avatar_upload))
        .route("/events/:id/cover/upload-url", post(create_event_cover_upload_url))
        .route("/events/:id/cover/confirm", post(confirm_event_cover_upload))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware::require_current_user,
        ));

    Router::new()
        .route("/health", get(health))
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/refresh", post(refresh))
        .route("/auth/logout", post(logout))
        .route("/auth/forgot-password", post(forgot_password))
        .route("/auth/reset-password", post(reset_password))
        .route("/auth/verify-email", post(verify_email))
        .route("/auth/resend-verification", post(resend_verification))
        .route("/validation-probe", post(validation_probe))
        .merge(protected_routes)
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

    let password_hash = state
        .password_service
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

    let verification = state.email_verification_service
        .issue_for_user_in_tx(&mut transaction, user.id)
        .await
        .map_err(map_email_verification_error)?;

    transaction.commit().await.map_err(AppError::from)?;

    info!(
        user_id = %user.id,
        email = %user.email,
        verification_expires_at = %verification.expires_at,
        "created user registration and queued email verification"
    );

    state
        .email_service
        .send_verification_email(
            &user.email,
            &user.display_name,
            &verification.token,
            verification.expires_at,
        )
        .await
        .map_err(map_email_send_error)?;

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
    let verification = state.email_verification_service
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

    let verification = state.email_verification_service
        .issue_for_user(&state.db_pool, user.id)
        .await
        .map_err(map_email_verification_error)?;

    info!(
        user_id = %user.id,
        email = %user.email,
        verification_expires_at = %verification.expires_at,
        "requeued email verification"
    );

    state
        .email_service
        .send_verification_email(
            &user.email,
            &user.display_name,
            &verification.token,
            verification.expires_at,
        )
        .await
        .map_err(map_email_send_error)?;

    Ok(Json(ApiResponse::new(ResendVerificationResponse {
        accepted: true,
    })))
}

async fn login(
    State(state): State<SharedAppState>,
    payload: ValidatedJson<LoginRequest>,
) -> Result<Json<ApiResponse<TokenResponse>>, AppError> {
    let email = normalize_email(&payload.email);
    state.login_rate_limiter.check(&email).await?;

    let Some(user) = users::find_user_by_email(&state.db_pool, &email)
        .await
        .map_err(AppError::from)? else {
        state.login_rate_limiter.record_failure(&email).await;
        return Err(AppError::bad_request("invalid email or password"));
    };

    let verified = state
        .password_service
        .verify_password(payload.password.trim_end(), &user.password_hash)
        .map_err(|error| AppError::internal(format!("failed to verify password: {error}")))?;

    if !verified {
        state.login_rate_limiter.record_failure(&email).await;
        return Err(AppError::bad_request("invalid email or password"));
    }

    if user.email_verified_at.is_none() {
        state.login_rate_limiter.record_failure(&email).await;
        return Err(AppError::bad_request("email address has not been verified"));
    }

    state.login_rate_limiter.record_success(&email).await;

    let access_token = state
        .jwt_service
        .issue_access_token(user.id)
        .map_err(|error| AppError::internal(format!("failed to issue access token: {error}")))?;
    let refresh_token = state
        .refresh_token_service
        .issue_for_user(&state.db_pool, user.id)
        .await
        .map_err(map_refresh_token_error)?;

    Ok(Json(ApiResponse::new(build_token_response(
        &user,
        access_token.token,
        access_token.claims.expires_at,
        refresh_token.token,
        refresh_token.claims.expires_at,
    ))))
}

async fn refresh(
    State(state): State<SharedAppState>,
    payload: ValidatedJson<RefreshRequest>,
) -> Result<Json<ApiResponse<TokenResponse>>, AppError> {
    let rotated = state
        .refresh_token_service
        .rotate(&state.db_pool, payload.refresh_token.trim())
        .await
        .map_err(map_refresh_token_error)?;
    let user = users::find_user_by_id(&state.db_pool, rotated.claims.subject)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::bad_request("refresh token references an unknown user"))?;

    if user.email_verified_at.is_none() {
        return Err(AppError::bad_request("email address has not been verified"));
    }

    let access_token = state
        .jwt_service
        .issue_access_token(user.id)
        .map_err(|error| AppError::internal(format!("failed to issue access token: {error}")))?;

    Ok(Json(ApiResponse::new(build_token_response(
        &user,
        access_token.token,
        access_token.claims.expires_at,
        rotated.token,
        rotated.claims.expires_at,
    ))))
}

async fn logout(
    State(state): State<SharedAppState>,
    payload: ValidatedJson<LogoutRequest>,
) -> Result<Json<ApiResponse<LogoutResponse>>, AppError> {
    let revoked = state
        .refresh_token_service
        .revoke_token(&state.db_pool, payload.refresh_token.trim())
        .await
        .map_err(map_refresh_token_error)?;

    Ok(Json(ApiResponse::new(LogoutResponse { revoked })))
}

async fn current_user(
    current_user: CurrentUser,
) -> Json<ApiResponse<CurrentUserResponse>> {
    Json(ApiResponse::new(CurrentUserResponse {
        id: current_user.id,
        email: current_user.email.clone(),
        display_name: current_user.display_name.clone(),
        bio: None,
        avatar_object_key: None,
        email_verified: current_user.email_verified_at.is_some(),
    }))
}

async fn read_profile(
    State(state): State<SharedAppState>,
    current_user: CurrentUser,
) -> Result<Json<ApiResponse<CurrentUserResponse>>, AppError> {
    let user = users::find_user_by_id(&state.db_pool, current_user.id)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::unauthorized("authenticated user was not found"))?;

    Ok(Json(ApiResponse::new(build_current_user_response(&user))))
}

async fn update_profile(
    State(state): State<SharedAppState>,
    current_user: CurrentUser,
    payload: ValidatedJson<UpdateProfileRequest>,
) -> Result<Json<ApiResponse<CurrentUserResponse>>, AppError> {
    if payload.display_name.is_none() && payload.bio.is_none() {
        return Err(AppError::bad_request(
            "at least one profile field must be provided",
        ));
    }

    let existing_user = users::find_user_by_id(&state.db_pool, current_user.id)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::unauthorized("authenticated user was not found"))?;

    let display_name = match payload.display_name.as_deref() {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(AppError::bad_request("display_name cannot be blank"));
            }

            trimmed.to_owned()
        }
        None => existing_user.display_name.clone(),
    };

    let bio = match payload.bio.as_ref() {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_owned())
            }
        }
        None => existing_user.bio.clone(),
    };

    let user = users::update_profile(&state.db_pool, current_user.id, &display_name, bio.as_deref())
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::unauthorized("authenticated user was not found"))?;

    Ok(Json(ApiResponse::new(build_current_user_response(&user))))
}

async fn create_avatar_upload_url(
    State(state): State<SharedAppState>,
    current_user: CurrentUser,
    payload: ValidatedJson<AvatarUploadUrlRequest>,
) -> Result<Json<ApiResponse<AvatarUploadUrlResponse>>, AppError> {
    let content_type = normalize_avatar_content_type(payload.content_type.trim())?;
    validate_avatar_size(payload.size_bytes)?;

    let object_key = build_avatar_object_key(current_user.id, content_type);
    let presigned = state
        .object_storage
        .put_presigned_url(
            &object_key,
            Some(content_type),
            Some(payload.size_bytes),
            std::time::Duration::from_secs(AVATAR_UPLOAD_URL_TTL_SECONDS),
        )
        .await
        .map_err(map_object_storage_error)?;

    Ok(Json(ApiResponse::new(AvatarUploadUrlResponse {
        object_key,
        method: presigned.method,
        upload_url: presigned.uri,
        headers: presigned
            .headers
            .into_iter()
            .map(|(name, value)| PresignedHeader { name, value })
            .collect(),
        expires_in_seconds: presigned.expires_in.as_secs(),
        max_size_bytes: AVATAR_UPLOAD_MAX_SIZE_BYTES,
    })))
}

async fn confirm_avatar_upload(
    State(state): State<SharedAppState>,
    current_user: CurrentUser,
    payload: ValidatedJson<ConfirmAvatarUploadRequest>,
) -> Result<Json<ApiResponse<CurrentUserResponse>>, AppError> {
    let object_key = payload.object_key.trim();
    ensure_avatar_key_belongs_to_user(current_user.id, object_key)?;

    let metadata = state
        .object_storage
        .head_object(object_key)
        .await
        .map_err(map_object_storage_error)?
        .ok_or_else(|| AppError::bad_request("uploaded avatar object was not found"))?;

    validate_confirmed_avatar_metadata(&metadata)?;

    let user = users::update_avatar_object_key(&state.db_pool, current_user.id, Some(object_key))
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::unauthorized("authenticated user was not found"))?;

    Ok(Json(ApiResponse::new(build_current_user_response(&user))))
}

async fn create_event_cover_upload_url(
    State(state): State<SharedAppState>,
    Path(event_id): Path<Uuid>,
    current_user: CurrentUser,
    payload: ValidatedJson<EventCoverUploadUrlRequest>,
) -> Result<Json<ApiResponse<EventCoverUploadUrlResponse>>, AppError> {
    let content_type = normalize_image_content_type(payload.content_type.trim())?;
    validate_cover_size(payload.size_bytes)?;

    let object_key = build_event_cover_object_key(event_id, current_user.id, content_type);
    let presigned = state
        .object_storage
        .put_presigned_url(
            &object_key,
            Some(content_type),
            Some(payload.size_bytes),
            std::time::Duration::from_secs(EVENT_COVER_UPLOAD_URL_TTL_SECONDS),
        )
        .await
        .map_err(map_object_storage_error)?;

    Ok(Json(ApiResponse::new(EventCoverUploadUrlResponse {
        event_id,
        object_key,
        method: presigned.method,
        upload_url: presigned.uri,
        headers: presigned
            .headers
            .into_iter()
            .map(|(name, value)| PresignedHeader { name, value })
            .collect(),
        expires_in_seconds: presigned.expires_in.as_secs(),
        max_size_bytes: EVENT_COVER_UPLOAD_MAX_SIZE_BYTES,
    })))
}

async fn confirm_event_cover_upload(
    State(state): State<SharedAppState>,
    Path(event_id): Path<Uuid>,
    current_user: CurrentUser,
    payload: ValidatedJson<ConfirmEventCoverUploadRequest>,
) -> Result<Json<ApiResponse<ConfirmEventCoverUploadResponse>>, AppError> {
    let object_key = payload.object_key.trim();
    ensure_event_cover_key_is_owned(event_id, current_user.id, object_key)?;
    validate_positive_dimensions(payload.width, payload.height)?;

    let metadata = state
        .object_storage
        .head_object(object_key)
        .await
        .map_err(map_object_storage_error)?
        .ok_or_else(|| AppError::bad_request("uploaded event cover object was not found"))?;

    validate_confirmed_cover_metadata(&metadata)?;

    let image = event_images::upsert_event_image(
        &state.db_pool,
        event_id,
        object_key,
        EventImageVariant::Hero,
        payload.width,
        payload.height,
        metadata
            .content_length
            .ok_or_else(|| AppError::bad_request("uploaded event cover object is missing a content length"))?,
    )
    .await
    .map_err(AppError::from)?;

    Ok(Json(ApiResponse::new(ConfirmEventCoverUploadResponse {
        event_id: image.event_id,
        object_key: image.object_key,
        variant: image.variant.as_str(),
        width: image.width,
        height: image.height,
        bytes: image.bytes,
    })))
}

async fn forgot_password(
    State(state): State<SharedAppState>,
    payload: ValidatedJson<ForgotPasswordRequest>,
) -> Result<Json<ApiResponse<ForgotPasswordResponse>>, AppError> {
    let email = normalize_email(&payload.email);
    let Some(user) = users::find_user_by_email(&state.db_pool, &email)
        .await
        .map_err(AppError::from)? else {
        return Ok(Json(ApiResponse::new(ForgotPasswordResponse {
            accepted: true,
        })));
    };

    let reset = state
        .password_reset_service
        .issue_for_user(&state.db_pool, user.id)
        .await
        .map_err(map_password_reset_error)?;

    info!(
        user_id = %user.id,
        email = %user.email,
        password_reset_expires_at = %reset.expires_at,
        "queued password reset email"
    );

    state
        .email_service
        .send_password_reset_email(
            &user.email,
            &user.display_name,
            &reset.token,
            reset.expires_at,
        )
        .await
        .map_err(map_email_send_error)?;

    Ok(Json(ApiResponse::new(ForgotPasswordResponse {
        accepted: true,
    })))
}

async fn reset_password(
    State(state): State<SharedAppState>,
    payload: ValidatedJson<ResetPasswordRequest>,
) -> Result<Json<ApiResponse<ResetPasswordResponse>>, AppError> {
    let password_hash = state
        .password_service
        .hash_password(payload.password.as_str())
        .map_err(|error| AppError::internal(format!("failed to hash password: {error}")))?;

    let mut transaction = state.db_pool.begin().await.map_err(AppError::from)?;
    let reset = state
        .password_reset_service
        .consume_token_in_tx(&mut transaction, payload.token.trim())
        .await
        .map_err(map_password_reset_error)?;

    let user = users::update_password_hash(&mut transaction, reset.user_id, &password_hash)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::bad_request("password reset token references an unknown user"))?;

    state
        .refresh_token_service
        .revoke_all_for_user_in_tx(&mut transaction, user.id)
        .await
        .map_err(map_refresh_token_error)?;

    transaction.commit().await.map_err(AppError::from)?;

    Ok(Json(ApiResponse::new(ResetPasswordResponse {
        user_id: user.id,
        email: user.email,
        password_reset: true,
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

fn build_token_response(
    user: &users::User,
    access_token: String,
    access_expires_at: DateTime<Utc>,
    refresh_token: String,
    refresh_expires_at: DateTime<Utc>,
) -> TokenResponse {
    let now = Utc::now();

    TokenResponse {
        access_token,
        refresh_token,
        token_type: "Bearer",
        expires_in_seconds: (access_expires_at - now).num_seconds().max(0),
        refresh_expires_in_seconds: (refresh_expires_at - now).num_seconds().max(0),
        user: AuthUserResponse {
            id: user.id,
            email: user.email.clone(),
            display_name: user.display_name.clone(),
            email_verified: user.email_verified_at.is_some(),
        },
    }
}

fn build_current_user_response(user: &users::User) -> CurrentUserResponse {
    CurrentUserResponse {
        id: user.id,
        email: user.email.clone(),
        display_name: user.display_name.clone(),
        bio: user.bio.clone(),
        avatar_object_key: user.avatar_object_key.clone(),
        email_verified: user.email_verified_at.is_some(),
    }
}

fn build_avatar_object_key(user_id: Uuid, content_type: &str) -> String {
    let extension = match content_type {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        _ => "bin",
    };

    format!("avatars/{user_id}/{}.{}", Uuid::new_v4(), extension)
}

fn build_event_cover_object_key(
    event_id: Uuid,
    user_id: Uuid,
    content_type: &str,
) -> String {
    let extension = file_extension_for_content_type(content_type);

    format!("events/{event_id}/covers/{user_id}/hero-{}.{}", Uuid::new_v4(), extension)
}

fn file_extension_for_content_type(content_type: &str) -> &'static str {
    match content_type {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        _ => "bin",
    }
}

fn normalize_avatar_content_type(content_type: &str) -> Result<&'static str, AppError> {
    normalize_image_content_type(content_type)
}

fn normalize_image_content_type(content_type: &str) -> Result<&'static str, AppError> {
    match content_type.to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => Ok("image/jpeg"),
        "image/png" => Ok("image/png"),
        "image/webp" => Ok("image/webp"),
        _ => Err(AppError::bad_request(
            "content_type must be one of image/jpeg, image/png, or image/webp",
        )),
    }
}

fn validate_avatar_size(size_bytes: i64) -> Result<(), AppError> {
    if size_bytes <= 0 {
        return Err(AppError::bad_request("size_bytes must be greater than zero"));
    }

    if size_bytes > AVATAR_UPLOAD_MAX_SIZE_BYTES {
        return Err(AppError::bad_request(format!(
            "size_bytes cannot exceed {AVATAR_UPLOAD_MAX_SIZE_BYTES}"
        )));
    }

    Ok(())
}

fn validate_cover_size(size_bytes: i64) -> Result<(), AppError> {
    if size_bytes <= 0 {
        return Err(AppError::bad_request("size_bytes must be greater than zero"));
    }

    if size_bytes > EVENT_COVER_UPLOAD_MAX_SIZE_BYTES {
        return Err(AppError::bad_request(format!(
            "size_bytes cannot exceed {EVENT_COVER_UPLOAD_MAX_SIZE_BYTES}"
        )));
    }

    Ok(())
}

fn ensure_avatar_key_belongs_to_user(user_id: Uuid, object_key: &str) -> Result<(), AppError> {
    let expected_prefix = format!("avatars/{user_id}/");

    if !object_key.starts_with(&expected_prefix) {
        return Err(AppError::bad_request("object_key is not valid for the current user"));
    }

    Ok(())
}

fn ensure_event_cover_key_is_owned(
    event_id: Uuid,
    user_id: Uuid,
    object_key: &str,
) -> Result<(), AppError> {
    let expected_prefix = format!("events/{event_id}/covers/{user_id}/");

    if !object_key.starts_with(&expected_prefix) {
        return Err(AppError::bad_request(
            "object_key is not valid for the current user and event",
        ));
    }

    Ok(())
}

fn validate_confirmed_avatar_metadata(metadata: &ObjectMetadata) -> Result<(), AppError> {
    let content_type = metadata
        .content_type
        .as_deref()
        .ok_or_else(|| AppError::bad_request("uploaded avatar object is missing a content type"))?;
    let normalized_content_type = normalize_avatar_content_type(content_type)?;
    let content_length = metadata
        .content_length
        .ok_or_else(|| AppError::bad_request("uploaded avatar object is missing a content length"))?;

    validate_avatar_size(content_length)?;

    if normalized_content_type != content_type {
        return Err(AppError::bad_request(
            "uploaded avatar object content type does not match the allowed set",
        ));
    }

    Ok(())
}

fn validate_confirmed_cover_metadata(metadata: &ObjectMetadata) -> Result<(), AppError> {
    let content_type = metadata
        .content_type
        .as_deref()
        .ok_or_else(|| AppError::bad_request("uploaded event cover object is missing a content type"))?;
    let normalized_content_type = normalize_image_content_type(content_type)?;
    let content_length = metadata
        .content_length
        .ok_or_else(|| AppError::bad_request("uploaded event cover object is missing a content length"))?;

    validate_cover_size(content_length)?;

    if normalized_content_type != content_type {
        return Err(AppError::bad_request(
            "uploaded event cover object content type does not match the allowed set",
        ));
    }

    Ok(())
}

fn validate_positive_dimensions(width: i32, height: i32) -> Result<(), AppError> {
    if width <= 0 {
        return Err(AppError::bad_request("width must be greater than zero"));
    }

    if height <= 0 {
        return Err(AppError::bad_request("height must be greater than zero"));
    }

    Ok(())
}

fn map_email_verification_error(error: EmailVerificationError) -> AppError {
    match error {
        EmailVerificationError::Database(error) => AppError::from(error),
        EmailVerificationError::TokenNotFound
        | EmailVerificationError::TokenAlreadyUsed
        | EmailVerificationError::TokenExpired => AppError::bad_request(error.to_string()),
    }
}

fn map_refresh_token_error(error: RefreshTokenError) -> AppError {
    match error {
        RefreshTokenError::Database(error) => AppError::from(error),
        RefreshTokenError::Auth(error) => AppError::bad_request(error.to_string()),
        RefreshTokenError::RefreshTokenNotFound
        | RefreshTokenError::RefreshTokenRevoked
        | RefreshTokenError::RefreshTokenExpired
        | RefreshTokenError::TokenSubjectMismatch { .. } => AppError::bad_request(error.to_string()),
    }
}

fn map_password_reset_error(error: PasswordResetError) -> AppError {
    match error {
        PasswordResetError::Database(error) => AppError::from(error),
        PasswordResetError::TokenNotFound
        | PasswordResetError::TokenAlreadyUsed
        | PasswordResetError::TokenExpired => AppError::bad_request(error.to_string()),
    }
}

fn map_email_send_error(error: EmailError) -> AppError {
    AppError::internal(format!("failed to queue transactional email: {error}"))
}

fn map_object_storage_error(error: ObjectStorageError) -> AppError {
    AppError::internal(format!("object storage operation failed: {error}"))
}

#[derive(Debug)]
pub struct LoginRateLimiter {
    entries: Mutex<HashMap<String, LoginAttemptEntry>>,
}

impl LoginRateLimiter {
    const FAILURE_LIMIT: u32 = 5;
    const BLOCK_DURATION_MINUTES: i64 = 15;

    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    async fn check(&self, key: &str) -> Result<(), AppError> {
        let mut entries = self.entries.lock().await;
        let now = Utc::now();

        match entries.get_mut(key) {
            Some(entry) if entry.blocked_until > now => Err(AppError::bad_request(
                "too many login attempts; please retry later",
            )),
            Some(entry) => {
                if entry.blocked_until <= now {
                    entries.remove(key);
                }

                Ok(())
            }
            None => Ok(()),
        }
    }

    async fn record_failure(&self, key: &str) {
        let mut entries = self.entries.lock().await;
        let now = Utc::now();
        let entry = entries.entry(key.to_owned()).or_insert(LoginAttemptEntry {
            failures: 0,
            blocked_until: now,
        });

        entry.failures += 1;
        if entry.failures >= Self::FAILURE_LIMIT {
            entry.blocked_until = now + Duration::minutes(Self::BLOCK_DURATION_MINUTES);
        } else {
            entry.blocked_until = now;
        }
    }

    async fn record_success(&self, key: &str) {
        self.entries.lock().await.remove(key);
    }
}

#[derive(Debug)]
struct LoginAttemptEntry {
    failures: u32,
    blocked_until: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::LoginRateLimiter;

    #[tokio::test]
    async fn login_rate_limiter_blocks_after_failure_limit() {
        let limiter = LoginRateLimiter::new();

        for _ in 0..LoginRateLimiter::FAILURE_LIMIT {
            limiter.record_failure("user@example.com").await;
        }

        let error = match limiter.check("user@example.com").await {
            Ok(()) => panic!("limiter should block after repeated failures"),
            Err(error) => error,
        };

        assert!(error
            .to_string()
            .contains("too many login attempts"));
    }

    #[tokio::test]
    async fn login_rate_limiter_resets_on_success() {
        let limiter = LoginRateLimiter::new();
        limiter.record_failure("user@example.com").await;
        limiter.record_success("user@example.com").await;

        assert!(limiter.check("user@example.com").await.is_ok());
    }
}
