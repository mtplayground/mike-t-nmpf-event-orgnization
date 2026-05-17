mod app;
mod logging;
mod shutdown;

use std::{io, net::SocketAddr};

use tokio::net::TcpListener;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    logging::init();

    let address = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = TcpListener::bind(address).await?;

    info!(address = %address, "starting HTTP server");

    axum::serve(listener, app::router())
        .with_graceful_shutdown(shutdown::wait_for_signal())
        .await
        .map_err(|error| {
            error!(error = %error, "server exited with error");
            io::Error::other(error)
        })
}
