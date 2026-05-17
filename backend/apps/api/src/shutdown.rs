use std::io;

use tracing::info;

pub async fn wait_for_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.map_err(io::Error::other)
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};

        let mut stream = signal(SignalKind::terminate())?;
        stream.recv().await;
        Ok::<(), io::Error>(())
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<Result<(), io::Error>>();

    tokio::select! {
        result = ctrl_c => {
            if let Err(error) = result {
                tracing::warn!(error = %error, "failed to install CTRL+C handler");
            }
        }
        result = terminate => {
            if let Err(error) = result {
                tracing::warn!(error = %error, "failed to install terminate signal handler");
            }
        }
    }

    info!("shutdown signal received");
}
