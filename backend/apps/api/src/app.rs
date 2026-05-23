use std::{collections::HashMap, io::Cursor, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    middleware,
    routing::{get, post},
};
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use http::{HeaderValue, Method};
use image::{DynamicImage, GenericImageView, ImageFormat, imageops::FilterType};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tokio::sync::Mutex;
use tower_http::{
    compression::CompressionLayer,
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};
use tracing::info;
use uuid::Uuid;
use validator::Validate;

use crate::{
    auth::{JwtService, PasswordService},
    auth_middleware,
    email::{EmailError, EmailService},
    email_verification::{EmailVerificationError, EmailVerificationService},
    error::{ApiResponse, AppError},
    event_images::{self, EventImageVariant},
    events::{self, EventLocationType, EventStatus, EventVisibility},
    extract::{CurrentUser, ValidatedJson},
    object_storage::{ObjectMetadata, ObjectStorageError},
    password_reset::{PasswordResetError, PasswordResetService},
    refresh_tokens::{RefreshTokenError, RefreshTokenService},
    users::{self, NewUser},
};

const AVATAR_UPLOAD_MAX_SIZE_BYTES: i64 = 5 * 1024 * 1024;
const AVATAR_UPLOAD_URL_TTL_SECONDS: u64 = 900;
const EVENT_COVER_SOURCE_MAX_SIZE_BYTES: i64 = 10 * 1024 * 1024;
const EVENT_COVER_UPLOAD_URL_TTL_SECONDS: u64 = 900;
const EVENT_COVER_HERO_MAX_WIDTH: u32 = 1200;
const EVENT_COVER_THUMBNAIL_MAX_WIDTH: u32 = 480;
const PUBLIC_EVENT_PAGE_SIZE: usize = 20;

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
    #[validate(length(
        min = 3,
        max = 64,
        message = "display_name must be between 3 and 64 characters"
    ))]
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
    #[validate(length(
        min = 8,
        max = 128,
        message = "password must be between 8 and 128 characters"
    ))]
    password: String,
    #[validate(length(
        min = 3,
        max = 64,
        message = "display_name must be between 3 and 64 characters"
    ))]
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
    #[validate(length(
        min = 64,
        max = 64,
        message = "token must be a valid password reset token"
    ))]
    token: String,
    #[validate(length(
        min = 8,
        max = 128,
        message = "password must be between 8 and 128 characters"
    ))]
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
    #[validate(length(
        min = 8,
        max = 128,
        message = "password must be between 8 and 128 characters"
    ))]
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
    #[validate(length(
        min = 3,
        max = 64,
        message = "display_name must be between 3 and 64 characters"
    ))]
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
    hero: ProcessedEventImageResponse,
    thumbnail: ProcessedEventImageResponse,
}

#[derive(Debug, Serialize)]
struct ProcessedEventImageResponse {
    object_key: String,
    variant: &'static str,
    width: i32,
    height: i32,
    bytes: i64,
}

#[derive(Debug, Deserialize, Validate)]
struct CreateEventRequest {
    #[validate(length(
        min = 1,
        max = 180,
        message = "title must be between 1 and 180 characters"
    ))]
    title: String,
    #[validate(length(
        max = 20_000,
        message = "description_md must be 20000 characters or fewer"
    ))]
    description_md: Option<String>,
    start_at: DateTime<Utc>,
    end_at: DateTime<Utc>,
    #[validate(length(
        min = 1,
        max = 100,
        message = "timezone must be between 1 and 100 characters"
    ))]
    timezone: String,
    location_type: EventLocationType,
    #[validate(length(max = 500, message = "location_text must be 500 characters or fewer"))]
    location_text: Option<String>,
    #[validate(length(max = 1000, message = "location_url must be 1000 characters or fewer"))]
    location_url: Option<String>,
    capacity: Option<i32>,
    visibility: Option<EventVisibility>,
    status: Option<EventStatus>,
    cover_image_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Validate)]
struct UpdateEventRequest {
    #[validate(length(
        min = 1,
        max = 180,
        message = "title must be between 1 and 180 characters"
    ))]
    title: Option<String>,
    #[validate(length(
        max = 20_000,
        message = "description_md must be 20000 characters or fewer"
    ))]
    description_md: Option<String>,
    start_at: Option<DateTime<Utc>>,
    end_at: Option<DateTime<Utc>>,
    #[validate(length(
        min = 1,
        max = 100,
        message = "timezone must be between 1 and 100 characters"
    ))]
    timezone: Option<String>,
    location_type: Option<EventLocationType>,
    #[validate(length(max = 500, message = "location_text must be 500 characters or fewer"))]
    location_text: Option<String>,
    #[validate(length(max = 1000, message = "location_url must be 1000 characters or fewer"))]
    location_url: Option<String>,
    capacity: Option<i32>,
    visibility: Option<EventVisibility>,
    status: Option<EventStatus>,
    cover_image_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
struct EventResponse {
    id: Uuid,
    host_id: Uuid,
    title: String,
    slug: String,
    description_md: String,
    start_at: DateTime<Utc>,
    end_at: DateTime<Utc>,
    timezone: String,
    location_type: EventLocationType,
    location_text: Option<String>,
    location_url: Option<String>,
    capacity: Option<i32>,
    visibility: EventVisibility,
    status: EventStatus,
    cover_image_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    cancelled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
struct ListEventsQuery {
    status: Option<String>,
    page: Option<i64>,
    per_page: Option<i64>,
}

#[derive(Debug, Serialize)]
struct HostEventListResponse {
    items: Vec<HostEventListItemResponse>,
    page: i64,
    per_page: i64,
    total_count: i64,
    total_pages: i64,
}

#[derive(Debug, Serialize)]
struct HostEventListItemResponse {
    id: Uuid,
    host_id: Uuid,
    title: String,
    slug: String,
    description_md: String,
    start_at: DateTime<Utc>,
    end_at: DateTime<Utc>,
    timezone: String,
    location_type: EventLocationType,
    location_text: Option<String>,
    location_url: Option<String>,
    capacity: Option<i32>,
    visibility: EventVisibility,
    status: EventStatus,
    cover_image_id: Option<Uuid>,
    attendee_count: i64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    cancelled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
struct PublicEventsQuery {
    query: Option<String>,
    from: Option<String>,
    to: Option<String>,
    cursor: Option<String>,
}

#[derive(Debug, Serialize)]
struct PublicEventListResponse {
    items: Vec<PublicEventResponse>,
    next_cursor: Option<String>,
}

#[derive(Debug, Serialize)]
struct PublicEventResponse {
    id: Uuid,
    title: String,
    slug: String,
    description_md: String,
    start_at: DateTime<Utc>,
    end_at: DateTime<Utc>,
    timezone: String,
    location_type: EventLocationType,
    location_text: Option<String>,
    location_url: Option<String>,
    capacity: Option<i32>,
    thumbnail: Option<PublicEventThumbnailResponse>,
}

#[derive(Debug, Serialize)]
struct PublicEventThumbnailResponse {
    object_key: String,
    width: i32,
    height: i32,
    bytes: i64,
}

pub fn router(state: SharedAppState) -> Router {
    let protected_routes = Router::new()
        .route("/auth/me", get(current_user))
        .route("/me", get(read_profile).patch(update_profile))
        .route("/me/events", get(list_my_events))
        .route("/me/avatar/upload-url", post(create_avatar_upload_url))
        .route("/me/avatar/confirm", post(confirm_avatar_upload))
        .route("/events", post(create_event))
        .route("/events/{id}", get(read_event).patch(update_event).delete(cancel_event))
        .route("/events/{id}/duplicate", post(duplicate_event))
        .route("/events/{id}/cover/upload-url", post(create_event_cover_upload_url))
        .route("/events/{id}/cover/confirm", post(confirm_event_cover_upload))
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
        .route("/events", get(list_public_events))
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

async fn list_public_events(
    State(state): State<SharedAppState>,
    Query(query): Query<PublicEventsQuery>,
) -> Result<Json<ApiResponse<PublicEventListResponse>>, AppError> {
    let search_query = normalize_public_search_query(query.query.as_deref())?;
    let from = parse_optional_public_datetime(query.from.as_deref(), "from")?;
    let to = parse_optional_public_datetime(query.to.as_deref(), "to")?;
    validate_public_event_time_range(from, to)?;
    let cursor = parse_public_event_cursor(query.cursor.as_deref())?;
    let limit = PUBLIC_EVENT_PAGE_SIZE + 1;
    let mut rows = events::list_public_events(
        &state.db_pool,
        search_query.as_deref(),
        from,
        to,
        cursor,
        limit as i64,
    )
    .await
    .map_err(AppError::from)?;
    let has_next_page = rows.len() > PUBLIC_EVENT_PAGE_SIZE;

    if has_next_page {
        rows.truncate(PUBLIC_EVENT_PAGE_SIZE);
    }

    let next_cursor =
        if has_next_page { rows.last().map(public_event_cursor_for_row) } else { None };
    let items = rows.into_iter().map(build_public_event_response).collect();

    Ok(Json(ApiResponse::new(PublicEventListResponse { items, next_cursor })))
}

async fn register(
    State(state): State<SharedAppState>,
    payload: ValidatedJson<RegisterRequest>,
) -> Result<Json<ApiResponse<RegisterResponse>>, AppError> {
    let email = normalize_email(&payload.email);
    let display_name = payload.display_name.trim().to_owned();

    if users::find_user_by_email(&state.db_pool, &email).await.map_err(AppError::from)?.is_some() {
        return Err(AppError::conflict("email is already registered"));
    }

    let password_hash = state
        .password_service
        .hash_password(payload.password.as_str())
        .map_err(|error| AppError::internal(format!("failed to hash password: {error}")))?;

    let mut transaction = state.db_pool.begin().await.map_err(AppError::from)?;
    let user = users::insert_user(
        &mut transaction,
        &NewUser { email: email.clone(), password_hash, display_name: display_name.clone() },
    )
    .await
    .map_err(AppError::from)?;

    let verification = state
        .email_verification_service
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
    let verification = state
        .email_verification_service
        .consume_token_in_tx(&mut transaction, payload.token.trim())
        .await
        .map_err(map_email_verification_error)?;

    let user = users::mark_email_verified(&mut transaction, verification.user_id, Utc::now())
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| {
            AppError::bad_request("email verification token references an unknown user")
        })?;

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
    let Some(user) =
        users::find_user_by_email(&state.db_pool, &email).await.map_err(AppError::from)?
    else {
        return Ok(Json(ApiResponse::new(ResendVerificationResponse { accepted: true })));
    };

    if user.email_verified_at.is_some() {
        return Ok(Json(ApiResponse::new(ResendVerificationResponse { accepted: true })));
    }

    let verification = state
        .email_verification_service
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

    Ok(Json(ApiResponse::new(ResendVerificationResponse { accepted: true })))
}

async fn login(
    State(state): State<SharedAppState>,
    payload: ValidatedJson<LoginRequest>,
) -> Result<Json<ApiResponse<TokenResponse>>, AppError> {
    let email = normalize_email(&payload.email);
    state.login_rate_limiter.check(&email).await?;

    let Some(user) =
        users::find_user_by_email(&state.db_pool, &email).await.map_err(AppError::from)?
    else {
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

async fn current_user(current_user: CurrentUser) -> Json<ApiResponse<CurrentUserResponse>> {
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
        return Err(AppError::bad_request("at least one profile field must be provided"));
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
            if trimmed.is_empty() { None } else { Some(trimmed.to_owned()) }
        }
        None => existing_user.bio.clone(),
    };

    let user =
        users::update_profile(&state.db_pool, current_user.id, &display_name, bio.as_deref())
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

async fn create_event(
    State(state): State<SharedAppState>,
    current_user: CurrentUser,
    payload: ValidatedJson<CreateEventRequest>,
) -> Result<Json<ApiResponse<EventResponse>>, AppError> {
    let title = normalize_required_text(&payload.title, "title")?;
    let description_md =
        payload.description_md.as_deref().map(str::trim).unwrap_or_default().to_owned();
    let timezone = normalize_required_text(&payload.timezone, "timezone")?;
    let location_text = normalize_optional_text(payload.location_text.as_deref());
    let location_url = normalize_optional_text(payload.location_url.as_deref());
    let visibility = payload.visibility.unwrap_or(EventVisibility::Draft);
    let status = payload.status.unwrap_or_else(|| default_event_status(visibility));

    validate_event_times(payload.start_at, payload.end_at)?;
    validate_event_location(
        payload.location_type,
        location_text.as_deref(),
        location_url.as_deref(),
    )?;
    validate_event_capacity(payload.capacity)?;
    validate_event_visibility_status(visibility, status)?;

    if payload.cover_image_id.is_some() {
        return Err(AppError::bad_request(
            "cover_image_id can only be set after the event is created",
        ));
    }

    let slug = unique_event_slug(&state, &title, None).await?;
    let event = events::insert_event(
        &state.db_pool,
        &events::NewEvent {
            host_id: current_user.id,
            title,
            slug,
            description_md,
            start_at: payload.start_at,
            end_at: payload.end_at,
            timezone,
            location_type: payload.location_type,
            location_text,
            location_url,
            capacity: payload.capacity,
            visibility,
            status,
            cover_image_id: None,
        },
    )
    .await
    .map_err(map_event_write_error)?;

    Ok(Json(ApiResponse::new(build_event_response(event))))
}

async fn read_event(
    State(state): State<SharedAppState>,
    Path(event_id): Path<Uuid>,
    current_user: CurrentUser,
) -> Result<Json<ApiResponse<EventResponse>>, AppError> {
    let event = require_host_event(&state, event_id, current_user.id).await?;

    Ok(Json(ApiResponse::new(build_event_response(event))))
}

async fn list_my_events(
    State(state): State<SharedAppState>,
    current_user: CurrentUser,
    Query(query): Query<ListEventsQuery>,
) -> Result<Json<ApiResponse<HostEventListResponse>>, AppError> {
    let filter = parse_host_event_list_filter(query.status.as_deref())?;
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20);

    validate_pagination(page, per_page)?;

    let offset = page
        .checked_sub(1)
        .and_then(|page_index| page_index.checked_mul(per_page))
        .ok_or_else(|| AppError::bad_request("page is too large"))?;
    let items =
        events::list_events_for_host(&state.db_pool, current_user.id, filter, per_page, offset)
            .await
            .map_err(AppError::from)?
            .into_iter()
            .map(build_host_event_list_item_response)
            .collect();
    let total_count = events::count_events_for_host(&state.db_pool, current_user.id, filter)
        .await
        .map_err(AppError::from)?;
    let total_pages = if total_count == 0 { 0 } else { (total_count + per_page - 1) / per_page };

    Ok(Json(ApiResponse::new(HostEventListResponse {
        items,
        page,
        per_page,
        total_count,
        total_pages,
    })))
}

async fn update_event(
    State(state): State<SharedAppState>,
    Path(event_id): Path<Uuid>,
    current_user: CurrentUser,
    payload: ValidatedJson<UpdateEventRequest>,
) -> Result<Json<ApiResponse<EventResponse>>, AppError> {
    let existing = require_host_event(&state, event_id, current_user.id).await?;
    let title = match payload.title.as_deref() {
        Some(value) => normalize_required_text(value, "title")?,
        None => existing.title.clone(),
    };
    let slug = if title == existing.title {
        existing.slug.clone()
    } else {
        unique_event_slug(&state, &title, Some(event_id)).await?
    };
    let description_md = match payload.description_md.as_deref() {
        Some(value) => value.trim().to_owned(),
        None => existing.description_md.clone(),
    };
    let start_at = payload.start_at.unwrap_or(existing.start_at);
    let end_at = payload.end_at.unwrap_or(existing.end_at);
    let timezone = match payload.timezone.as_deref() {
        Some(value) => normalize_required_text(value, "timezone")?,
        None => existing.timezone.clone(),
    };
    let location_type = payload.location_type.unwrap_or(existing.location_type);
    let location_text = match payload.location_text.as_deref() {
        Some(value) => normalize_optional_text(Some(value)),
        None => existing.location_text.clone(),
    };
    let location_url = match payload.location_url.as_deref() {
        Some(value) => normalize_optional_text(Some(value)),
        None => existing.location_url.clone(),
    };
    let capacity = payload.capacity.or(existing.capacity);
    let visibility = payload.visibility.unwrap_or(existing.visibility);
    let status = payload.status.unwrap_or(existing.status);
    let cover_image_id = payload.cover_image_id.or(existing.cover_image_id);

    validate_event_times(start_at, end_at)?;
    validate_event_location(location_type, location_text.as_deref(), location_url.as_deref())?;
    validate_event_capacity(capacity)?;
    validate_event_visibility_status(visibility, status)?;

    if let Some(cover_image_id) = cover_image_id {
        ensure_cover_image_belongs_to_event(&state, cover_image_id, event_id).await?;
    }

    let event = events::update_event_for_host(
        &state.db_pool,
        event_id,
        current_user.id,
        &events::EventChanges {
            title,
            slug,
            description_md,
            start_at,
            end_at,
            timezone,
            location_type,
            location_text,
            location_url,
            capacity,
            visibility,
            status,
            cover_image_id,
        },
    )
    .await
    .map_err(map_event_write_error)?
    .ok_or_else(|| AppError::not_found("event was not found"))?;

    Ok(Json(ApiResponse::new(build_event_response(event))))
}

async fn cancel_event(
    State(state): State<SharedAppState>,
    Path(event_id): Path<Uuid>,
    current_user: CurrentUser,
) -> Result<Json<ApiResponse<EventResponse>>, AppError> {
    let event = events::cancel_event_for_host(&state.db_pool, event_id, current_user.id)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::not_found("event was not found"))?;

    Ok(Json(ApiResponse::new(build_event_response(event))))
}

async fn duplicate_event(
    State(state): State<SharedAppState>,
    Path(event_id): Path<Uuid>,
    current_user: CurrentUser,
) -> Result<Json<ApiResponse<EventResponse>>, AppError> {
    let existing = require_host_event(&state, event_id, current_user.id).await?;
    let duplicate_title = format!("{} Copy", existing.title);
    let slug = unique_event_slug(&state, &duplicate_title, None).await?;
    let event = events::insert_event(
        &state.db_pool,
        &events::duplicate_event_template(&existing, duplicate_title, slug),
    )
    .await
    .map_err(map_event_write_error)?;

    Ok(Json(ApiResponse::new(build_event_response(event))))
}

async fn create_event_cover_upload_url(
    State(state): State<SharedAppState>,
    Path(event_id): Path<Uuid>,
    current_user: CurrentUser,
    payload: ValidatedJson<EventCoverUploadUrlRequest>,
) -> Result<Json<ApiResponse<EventCoverUploadUrlResponse>>, AppError> {
    let _event = require_host_event(&state, event_id, current_user.id).await?;
    let content_type = normalize_image_content_type(payload.content_type.trim())?;
    validate_cover_source_size(payload.size_bytes)?;

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
        max_size_bytes: EVENT_COVER_SOURCE_MAX_SIZE_BYTES,
    })))
}

async fn confirm_event_cover_upload(
    State(state): State<SharedAppState>,
    Path(event_id): Path<Uuid>,
    current_user: CurrentUser,
    payload: ValidatedJson<ConfirmEventCoverUploadRequest>,
) -> Result<Json<ApiResponse<ConfirmEventCoverUploadResponse>>, AppError> {
    let _event = require_host_event(&state, event_id, current_user.id).await?;
    let object_key = payload.object_key.trim();
    ensure_event_cover_key_is_owned(event_id, current_user.id, object_key)?;
    let existing_images = event_images::find_event_images_by_event_id(&state.db_pool, event_id)
        .await
        .map_err(AppError::from)?;

    let source_object = state
        .object_storage
        .get_object_bytes(object_key)
        .await
        .map_err(map_object_storage_error)?
        .ok_or_else(|| AppError::bad_request("uploaded event cover object was not found"))?;

    let processed = process_event_cover_upload(
        event_id,
        current_user.id,
        &source_object,
        payload.width,
        payload.height,
    )?;

    state
        .object_storage
        .put_object_bytes(
            &processed.hero.object_key,
            processed.hero.bytes.clone(),
            processed.hero.content_type,
        )
        .await
        .map_err(map_object_storage_error)?;

    if let Err(error) = state
        .object_storage
        .put_object_bytes(
            &processed.thumbnail.object_key,
            processed.thumbnail.bytes.clone(),
            processed.thumbnail.content_type,
        )
        .await
    {
        let _ = state.object_storage.delete_object(&processed.hero.object_key).await;
        return Err(map_object_storage_error(error));
    }

    let hero_record = match event_images::upsert_event_image(
        &state.db_pool,
        event_id,
        &processed.hero.object_key,
        EventImageVariant::Hero,
        processed.hero.width,
        processed.hero.height,
        processed.hero.size_bytes,
    )
    .await
    {
        Ok(record) => record,
        Err(error) => {
            cleanup_uploaded_cover_variants(&state, &processed).await;
            return Err(AppError::from(error));
        }
    };

    let thumbnail_record = match event_images::upsert_event_image(
        &state.db_pool,
        event_id,
        &processed.thumbnail.object_key,
        EventImageVariant::Thumbnail,
        processed.thumbnail.width,
        processed.thumbnail.height,
        processed.thumbnail.size_bytes,
    )
    .await
    {
        Ok(record) => record,
        Err(error) => {
            cleanup_uploaded_cover_variants(&state, &processed).await;
            return Err(AppError::from(error));
        }
    };

    cleanup_replaced_event_images(&state, &existing_images, &processed).await;

    let _ = state.object_storage.delete_object(object_key).await;

    Ok(Json(ApiResponse::new(ConfirmEventCoverUploadResponse {
        event_id,
        hero: ProcessedEventImageResponse {
            object_key: hero_record.object_key,
            variant: hero_record.variant.as_str(),
            width: hero_record.width,
            height: hero_record.height,
            bytes: hero_record.bytes,
        },
        thumbnail: ProcessedEventImageResponse {
            object_key: thumbnail_record.object_key,
            variant: thumbnail_record.variant.as_str(),
            width: thumbnail_record.width,
            height: thumbnail_record.height,
            bytes: thumbnail_record.bytes,
        },
    })))
}

async fn forgot_password(
    State(state): State<SharedAppState>,
    payload: ValidatedJson<ForgotPasswordRequest>,
) -> Result<Json<ApiResponse<ForgotPasswordResponse>>, AppError> {
    let email = normalize_email(&payload.email);
    let Some(user) =
        users::find_user_by_email(&state.db_pool, &email).await.map_err(AppError::from)?
    else {
        return Ok(Json(ApiResponse::new(ForgotPasswordResponse { accepted: true })));
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
        .send_password_reset_email(&user.email, &user.display_name, &reset.token, reset.expires_at)
        .await
        .map_err(map_email_send_error)?;

    Ok(Json(ApiResponse::new(ForgotPasswordResponse { accepted: true })))
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
        .allow_methods([
            Method::GET,
            Method::HEAD,
            Method::OPTIONS,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
        ])
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

fn build_event_response(event: events::Event) -> EventResponse {
    EventResponse {
        id: event.id,
        host_id: event.host_id,
        title: event.title,
        slug: event.slug,
        description_md: event.description_md,
        start_at: event.start_at,
        end_at: event.end_at,
        timezone: event.timezone,
        location_type: event.location_type,
        location_text: event.location_text,
        location_url: event.location_url,
        capacity: event.capacity,
        visibility: event.visibility,
        status: event.status,
        cover_image_id: event.cover_image_id,
        created_at: event.created_at,
        updated_at: event.updated_at,
        cancelled_at: event.cancelled_at,
    }
}

fn build_host_event_list_item_response(row: events::HostEventListRow) -> HostEventListItemResponse {
    HostEventListItemResponse {
        id: row.id,
        host_id: row.host_id,
        title: row.title,
        slug: row.slug,
        description_md: row.description_md,
        start_at: row.start_at,
        end_at: row.end_at,
        timezone: row.timezone,
        location_type: row.location_type,
        location_text: row.location_text,
        location_url: row.location_url,
        capacity: row.capacity,
        visibility: row.visibility,
        status: row.status,
        cover_image_id: row.cover_image_id,
        attendee_count: row.attendee_count,
        created_at: row.created_at,
        updated_at: row.updated_at,
        cancelled_at: row.cancelled_at,
    }
}

fn build_public_event_response(row: events::PublicEventListRow) -> PublicEventResponse {
    let thumbnail = row.thumbnail_object_key.map(|object_key| PublicEventThumbnailResponse {
        object_key,
        width: row.thumbnail_width.unwrap_or_default(),
        height: row.thumbnail_height.unwrap_or_default(),
        bytes: row.thumbnail_bytes.unwrap_or_default(),
    });

    PublicEventResponse {
        id: row.id,
        title: row.title,
        slug: row.slug,
        description_md: row.description_md,
        start_at: row.start_at,
        end_at: row.end_at,
        timezone: row.timezone,
        location_type: row.location_type,
        location_text: row.location_text,
        location_url: row.location_url,
        capacity: row.capacity,
        thumbnail,
    }
}

async fn require_host_event(
    state: &SharedAppState,
    event_id: Uuid,
    host_id: Uuid,
) -> Result<events::Event, AppError> {
    events::find_event_for_host(&state.db_pool, event_id, host_id)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::not_found("event was not found"))
}

fn parse_host_event_list_filter(
    value: Option<&str>,
) -> Result<events::HostEventListFilter, AppError> {
    match value.map(str::trim) {
        None | Some("") => Err(AppError::bad_request("status must be provided")),
        Some(value) if value.len() > 32 => Err(AppError::bad_request("status is too long")),
        Some("draft") => Ok(events::HostEventListFilter::Draft),
        Some("upcoming") => Ok(events::HostEventListFilter::Upcoming),
        Some("past") => Ok(events::HostEventListFilter::Past),
        _ => Err(AppError::bad_request("status must be one of draft, upcoming, or past")),
    }
}

fn validate_pagination(page: i64, per_page: i64) -> Result<(), AppError> {
    if page < 1 {
        return Err(AppError::bad_request("page must be greater than zero"));
    }

    if !(1..=100).contains(&per_page) {
        return Err(AppError::bad_request("per_page must be between 1 and 100"));
    }

    Ok(())
}

fn normalize_public_search_query(value: Option<&str>) -> Result<Option<String>, AppError> {
    let Some(value) = normalize_optional_text(value) else {
        return Ok(None);
    };

    if value.len() > 200 {
        return Err(AppError::bad_request("query must be 200 characters or fewer"));
    }

    Ok(Some(value))
}

fn parse_optional_public_datetime(
    value: Option<&str>,
    field_name: &'static str,
) -> Result<Option<DateTime<Utc>>, AppError> {
    let Some(value) = normalize_optional_text(value) else {
        return Ok(None);
    };

    DateTime::parse_from_rfc3339(&value)
        .map(|value| Some(value.with_timezone(&Utc)))
        .map_err(|_| AppError::bad_request(format!("{field_name} must be an RFC3339 timestamp")))
}

fn validate_public_event_time_range(
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
) -> Result<(), AppError> {
    if matches!((from, to), (Some(from), Some(to)) if to < from) {
        return Err(AppError::bad_request("to must be at or after from"));
    }

    Ok(())
}

fn parse_public_event_cursor(
    value: Option<&str>,
) -> Result<Option<events::PublicEventCursor>, AppError> {
    let Some(value) = normalize_optional_text(value) else {
        return Ok(None);
    };

    if value.len() > 128 {
        return Err(AppError::bad_request("cursor is too long"));
    }

    let (start_at, event_id) =
        value.split_once('|').ok_or_else(|| AppError::bad_request("cursor is invalid"))?;
    let start_at = DateTime::parse_from_rfc3339(start_at)
        .map_err(|_| AppError::bad_request("cursor is invalid"))?
        .with_timezone(&Utc);
    let id = Uuid::parse_str(event_id).map_err(|_| AppError::bad_request("cursor is invalid"))?;

    Ok(Some(events::PublicEventCursor { start_at, id }))
}

fn public_event_cursor_for_row(row: &events::PublicEventListRow) -> String {
    format!("{}|{}", row.start_at.to_rfc3339_opts(SecondsFormat::Micros, true), row.id)
}

fn normalize_required_text(value: &str, field_name: &'static str) -> Result<String, AppError> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return Err(AppError::bad_request(format!("{field_name} cannot be blank")));
    }

    Ok(trimmed.to_owned())
}

fn normalize_optional_text(value: Option<&str>) -> Option<String> {
    value.map(str::trim).filter(|value| !value.is_empty()).map(ToOwned::to_owned)
}

fn validate_event_times(start_at: DateTime<Utc>, end_at: DateTime<Utc>) -> Result<(), AppError> {
    if end_at < start_at {
        return Err(AppError::bad_request("end_at must be at or after start_at"));
    }

    Ok(())
}

fn validate_event_capacity(capacity: Option<i32>) -> Result<(), AppError> {
    if matches!(capacity, Some(value) if value <= 0) {
        return Err(AppError::bad_request("capacity must be greater than zero"));
    }

    Ok(())
}

fn validate_event_location(
    location_type: EventLocationType,
    location_text: Option<&str>,
    location_url: Option<&str>,
) -> Result<(), AppError> {
    match location_type {
        EventLocationType::InPerson if location_text.is_none() => {
            Err(AppError::bad_request("location_text is required for in-person events"))
        }
        EventLocationType::Virtual if location_url.is_none() => {
            Err(AppError::bad_request("location_url is required for virtual events"))
        }
        EventLocationType::Hybrid if location_text.is_none() || location_url.is_none() => Err(
            AppError::bad_request("location_text and location_url are required for hybrid events"),
        ),
        _ => Ok(()),
    }
}

fn default_event_status(visibility: EventVisibility) -> EventStatus {
    match visibility {
        EventVisibility::Draft => EventStatus::Draft,
        EventVisibility::Public | EventVisibility::Unlisted | EventVisibility::Private => {
            EventStatus::Published
        }
    }
}

fn validate_event_visibility_status(
    visibility: EventVisibility,
    status: EventStatus,
) -> Result<(), AppError> {
    if status == EventStatus::Draft && visibility != EventVisibility::Draft {
        return Err(AppError::bad_request("draft events must use draft visibility"));
    }

    if status == EventStatus::Published && visibility == EventVisibility::Draft {
        return Err(AppError::bad_request("published events cannot use draft visibility"));
    }

    Ok(())
}

async fn unique_event_slug(
    state: &SharedAppState,
    title: &str,
    existing_event_id: Option<Uuid>,
) -> Result<String, AppError> {
    let base_slug = slugify_event_title(title);

    for suffix in 0..100 {
        let candidate =
            if suffix == 0 { base_slug.clone() } else { format!("{base_slug}-{suffix}") };

        let exists = match existing_event_id {
            Some(event_id) => {
                events::slug_exists_for_other_event(&state.db_pool, &candidate, event_id).await
            }
            None => events::slug_exists(&state.db_pool, &candidate).await,
        }
        .map_err(AppError::from)?;

        if !exists {
            return Ok(candidate);
        }
    }

    Ok(format!("{base_slug}-{}", Uuid::new_v4()))
}

fn slugify_event_title(title: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;

    for character in title.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash && !slug.is_empty() {
            slug.push('-');
            previous_dash = true;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    if slug.is_empty() { "event".to_owned() } else { slug }
}

async fn ensure_cover_image_belongs_to_event(
    state: &SharedAppState,
    cover_image_id: Uuid,
    event_id: Uuid,
) -> Result<(), AppError> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM event_images WHERE id = $1 AND event_id = $2)",
    )
    .bind(cover_image_id)
    .bind(event_id)
    .fetch_one(&state.db_pool)
    .await
    .map_err(AppError::from)?;

    if !exists {
        return Err(AppError::bad_request("cover_image_id does not belong to the event"));
    }

    Ok(())
}

fn map_event_write_error(error: sqlx::Error) -> AppError {
    if is_event_slug_unique_violation(&error) {
        AppError::conflict("event slug is already in use")
    } else {
        AppError::from(error)
    }
}

fn is_event_slug_unique_violation(error: &sqlx::Error) -> bool {
    error.as_database_error().and_then(|database_error| database_error.constraint())
        == Some("events_slug_unique_idx")
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

fn build_event_cover_object_key(event_id: Uuid, user_id: Uuid, content_type: &str) -> String {
    let extension = file_extension_for_content_type(content_type);

    format!("events/{event_id}/covers/{user_id}/hero-{}.{}", Uuid::new_v4(), extension)
}

fn build_processed_event_cover_object_key(event_id: Uuid, variant: EventImageVariant) -> String {
    format!("events/{event_id}/images/{}-{}.png", variant.as_str(), Uuid::new_v4())
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

fn validate_cover_source_size(size_bytes: i64) -> Result<(), AppError> {
    if size_bytes <= 0 {
        return Err(AppError::bad_request("size_bytes must be greater than zero"));
    }

    if size_bytes > EVENT_COVER_SOURCE_MAX_SIZE_BYTES {
        return Err(AppError::bad_request(format!(
            "size_bytes cannot exceed {EVENT_COVER_SOURCE_MAX_SIZE_BYTES}"
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
    let content_length = metadata.content_length.ok_or_else(|| {
        AppError::bad_request("uploaded avatar object is missing a content length")
    })?;

    validate_avatar_size(content_length)?;

    if normalized_content_type != content_type {
        return Err(AppError::bad_request(
            "uploaded avatar object content type does not match the allowed set",
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

#[derive(Debug)]
struct ProcessedEventCoverUpload {
    hero: ProcessedEventImage,
    thumbnail: ProcessedEventImage,
}

#[derive(Debug)]
struct ProcessedEventImage {
    object_key: String,
    content_type: &'static str,
    bytes: Vec<u8>,
    width: i32,
    height: i32,
    size_bytes: i64,
}

fn process_event_cover_upload(
    event_id: Uuid,
    user_id: Uuid,
    source_object: &crate::object_storage::ObjectBody,
    reported_width: i32,
    reported_height: i32,
) -> Result<ProcessedEventCoverUpload, AppError> {
    ensure_event_cover_key_is_owned(event_id, user_id, &source_object.key)?;
    validate_positive_dimensions(reported_width, reported_height)?;
    validate_cover_source_size(source_object.bytes.len() as i64)?;

    let source_content_type = source_object.content_type.as_deref().ok_or_else(|| {
        AppError::bad_request("uploaded event cover object is missing a content type")
    })?;
    let normalized_content_type = normalize_image_content_type(source_content_type)?;
    let format = image::guess_format(&source_object.bytes).map_err(|error| {
        AppError::bad_request(format!("unable to detect uploaded image format: {error}"))
    })?;
    let detected_content_type = content_type_for_image_format(format).ok_or_else(|| {
        AppError::bad_request("uploaded event cover image format is not supported")
    })?;

    if detected_content_type != normalized_content_type {
        return Err(AppError::bad_request(
            "uploaded event cover object content type does not match the detected image format",
        ));
    }

    let image =
        image::load_from_memory_with_format(&source_object.bytes, format).map_err(|error| {
            AppError::bad_request(format!("failed to decode uploaded event cover image: {error}"))
        })?;
    let (actual_width, actual_height) = image.dimensions();

    if actual_width == 0 || actual_height == 0 {
        return Err(AppError::bad_request("uploaded event cover image has invalid dimensions"));
    }

    if actual_width as i32 != reported_width || actual_height as i32 != reported_height {
        return Err(AppError::bad_request(
            "reported event cover dimensions do not match the uploaded image",
        ));
    }

    let hero = build_processed_event_image(
        event_id,
        EventImageVariant::Hero,
        &image,
        EVENT_COVER_HERO_MAX_WIDTH,
    )?;
    let thumbnail = build_processed_event_image(
        event_id,
        EventImageVariant::Thumbnail,
        &image,
        EVENT_COVER_THUMBNAIL_MAX_WIDTH,
    )?;

    Ok(ProcessedEventCoverUpload { hero, thumbnail })
}

fn build_processed_event_image(
    event_id: Uuid,
    variant: EventImageVariant,
    image: &DynamicImage,
    max_width: u32,
) -> Result<ProcessedEventImage, AppError> {
    let resized = resize_for_max_width(image, max_width);
    let (width, height) = resized.dimensions();
    let bytes = encode_png_image(&resized)?;
    let size_bytes = bytes.len() as i64;
    let width = i32::try_from(width)
        .map_err(|_| AppError::internal("generated image width exceeded supported range"))?;
    let height = i32::try_from(height)
        .map_err(|_| AppError::internal("generated image height exceeded supported range"))?;

    Ok(ProcessedEventImage {
        object_key: build_processed_event_cover_object_key(event_id, variant),
        content_type: "image/png",
        bytes,
        width,
        height,
        size_bytes,
    })
}

fn resize_for_max_width(image: &DynamicImage, max_width: u32) -> DynamicImage {
    if image.width() <= max_width {
        return image.clone();
    }

    image.resize(max_width, u32::MAX, FilterType::Lanczos3)
}

fn encode_png_image(image: &DynamicImage) -> Result<Vec<u8>, AppError> {
    let mut buffer = Cursor::new(Vec::new());
    image.write_to(&mut buffer, ImageFormat::Png).map_err(|error| {
        AppError::internal(format!("failed to encode processed event image: {error}"))
    })?;

    Ok(buffer.into_inner())
}

fn content_type_for_image_format(format: ImageFormat) -> Option<&'static str> {
    match format {
        ImageFormat::Jpeg => Some("image/jpeg"),
        ImageFormat::Png => Some("image/png"),
        ImageFormat::WebP => Some("image/webp"),
        _ => None,
    }
}

async fn cleanup_uploaded_cover_variants(
    state: &SharedAppState,
    processed: &ProcessedEventCoverUpload,
) {
    let _ = state.object_storage.delete_object(&processed.hero.object_key).await;
    let _ = state.object_storage.delete_object(&processed.thumbnail.object_key).await;
}

async fn cleanup_replaced_event_images(
    state: &SharedAppState,
    existing_images: &[event_images::EventImage],
    processed: &ProcessedEventCoverUpload,
) {
    for image in existing_images {
        let keep_key = match image.variant {
            EventImageVariant::Hero => &processed.hero.object_key,
            EventImageVariant::Thumbnail => &processed.thumbnail.object_key,
        };

        if image.object_key != *keep_key {
            let _ = state.object_storage.delete_object(&image.object_key).await;
        }
    }
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
        | RefreshTokenError::TokenSubjectMismatch { .. } => {
            AppError::bad_request(error.to_string())
        }
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
        Self { entries: Mutex::new(HashMap::new()) }
    }

    async fn check(&self, key: &str) -> Result<(), AppError> {
        let mut entries = self.entries.lock().await;
        let now = Utc::now();

        match entries.get_mut(key) {
            Some(entry) if entry.blocked_until > now => {
                Err(AppError::bad_request("too many login attempts; please retry later"))
            }
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
        let entry = entries
            .entry(key.to_owned())
            .or_insert(LoginAttemptEntry { failures: 0, blocked_until: now });

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
    use chrono::{Duration, Utc};

    use super::{
        EventLocationType, EventStatus, EventVisibility, LoginRateLimiter, default_event_status,
        slugify_event_title, validate_event_capacity, validate_event_location,
        validate_event_times, validate_event_visibility_status,
    };

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

        assert!(error.to_string().contains("too many login attempts"));
    }

    #[tokio::test]
    async fn login_rate_limiter_resets_on_success() {
        let limiter = LoginRateLimiter::new();
        limiter.record_failure("user@example.com").await;
        limiter.record_success("user@example.com").await;

        assert!(limiter.check("user@example.com").await.is_ok());
    }

    #[test]
    fn event_time_validation_rejects_end_before_start() {
        let start_at = Utc::now();
        let end_at = start_at - Duration::minutes(1);

        let error = validate_event_times(start_at, end_at)
            .expect_err("end before start should fail validation");

        assert!(error.to_string().contains("end_at must be at or after start_at"));
    }

    #[test]
    fn event_location_validation_requires_fields_by_location_type() {
        let in_person_error = validate_event_location(EventLocationType::InPerson, None, None)
            .expect_err("in-person events require a physical location");
        let virtual_error = validate_event_location(EventLocationType::Virtual, None, None)
            .expect_err("virtual events require a URL");
        let hybrid_error = validate_event_location(EventLocationType::Hybrid, Some("Hall"), None)
            .expect_err("hybrid events require both location forms");

        assert!(in_person_error.to_string().contains("location_text is required"));
        assert!(virtual_error.to_string().contains("location_url is required"));
        assert!(hybrid_error.to_string().contains("location_text and location_url"));
        assert!(
            validate_event_location(EventLocationType::Hybrid, Some("Hall"), Some("https://meet"))
                .is_ok()
        );
    }

    #[test]
    fn event_capacity_validation_rejects_non_positive_values() {
        assert!(validate_event_capacity(Some(1)).is_ok());

        let zero_error = validate_event_capacity(Some(0)).expect_err("zero capacity should fail");
        let negative_error =
            validate_event_capacity(Some(-1)).expect_err("negative capacity should fail");

        assert!(zero_error.to_string().contains("capacity must be greater than zero"));
        assert!(negative_error.to_string().contains("capacity must be greater than zero"));
    }

    #[test]
    fn event_visibility_status_validation_enforces_draft_pairing() {
        assert_eq!(default_event_status(EventVisibility::Draft), EventStatus::Draft);
        assert_eq!(default_event_status(EventVisibility::Public), EventStatus::Published);
        assert!(
            validate_event_visibility_status(EventVisibility::Draft, EventStatus::Draft).is_ok()
        );
        assert!(
            validate_event_visibility_status(EventVisibility::Public, EventStatus::Published)
                .is_ok()
        );

        let public_draft_error =
            validate_event_visibility_status(EventVisibility::Public, EventStatus::Draft)
                .expect_err("draft status with public visibility should fail");
        let draft_published_error =
            validate_event_visibility_status(EventVisibility::Draft, EventStatus::Published)
                .expect_err("published status with draft visibility should fail");

        assert!(public_draft_error.to_string().contains("draft events must use draft visibility"));
        assert!(
            draft_published_error
                .to_string()
                .contains("published events cannot use draft visibility")
        );
    }

    #[test]
    fn event_slug_generation_normalizes_titles() {
        assert_eq!(slugify_event_title("  Mike T: Spring Gala!  "), "mike-t-spring-gala");
        assert_eq!(slugify_event_title("!!!"), "event");
    }
}
