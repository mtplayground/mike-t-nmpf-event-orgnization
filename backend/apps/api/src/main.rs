mod app;
mod auth;
mod auth_middleware;
mod config;
mod database;
mod email;
mod email_verification;
mod error;
mod events;
mod event_images;
mod extract;
mod logging;
mod object_storage;
mod password_reset;
mod reminders;
mod registrations;
mod refresh_tokens;
mod shutdown;
mod users;

use std::{io, sync::Arc};

use tokio::net::TcpListener;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    logging::init();

    let config = config::Config::load().map_err(io::Error::other)?;
    let password_service = auth::PasswordService::new().map_err(io::Error::other)?;
    let jwt_service = auth::JwtService::from_config(&config.jwt).map_err(io::Error::other)?;
    let refresh_token_service = refresh_tokens::RefreshTokenService::new(jwt_service.clone());
    let email_service = email::EmailService::spawn(&config.smtp).map_err(io::Error::other)?;
    let email_verification_service = email_verification::EmailVerificationService::new();
    let password_reset_service = password_reset::PasswordResetService::new();
    let db_pool = database::connect(&config.database)
        .await
        .map_err(io::Error::other)?;
    let _reminder_service =
        reminders::ReminderService::spawn(db_pool.clone(), email_service.clone());
    let object_storage = object_storage::ObjectStorageClient::from_config(&config.object_storage)
        .await
        .map_err(io::Error::other)?;
    let app_state = Arc::new(app::AppState {
        db_pool,
        object_storage,
        password_service,
        jwt_service,
        refresh_token_service,
        email_service,
        email_verification_service,
        password_reset_service,
        login_rate_limiter: Arc::new(app::LoginRateLimiter::new()),
        announcement_rate_limiter: Arc::new(app::AnnouncementRateLimiter::new()),
    });
    let address = config.server.socket_addr();
    let listener = TcpListener::bind(address).await?;

    info!(
        address = %address,
        jwt_issuer = %config.jwt.issuer,
        smtp_host = %config.smtp.host,
        smtp_port = config.smtp.port,
        smtp_starttls = config.smtp.use_starttls,
        object_storage_endpoint = %config.object_storage.endpoint,
        object_storage_bucket = %config.object_storage.bucket,
        object_storage_public_base_url = config.object_storage.public_base_url.as_deref().unwrap_or(""),
        "starting HTTP server"
    );

    axum::serve(listener, app::router(app_state))
        .with_graceful_shutdown(shutdown::wait_for_signal())
        .await
        .map_err(|error| {
            error!(error = %error, "server exited with error");
            io::Error::other(error)
        })
}
