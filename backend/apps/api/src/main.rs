mod app;
mod config;
mod logging;
mod shutdown;

use std::io;

use tokio::net::TcpListener;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    logging::init();

    let config = config::Config::load().map_err(io::Error::other)?;
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

    axum::serve(listener, app::router())
        .with_graceful_shutdown(shutdown::wait_for_signal())
        .await
        .map_err(|error| {
            error!(error = %error, "server exited with error");
            io::Error::other(error)
        })
}
