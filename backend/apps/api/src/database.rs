use std::{fmt, str::FromStr, time::Duration};

use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions, PgSslMode},
};
use tracing::info;

use crate::config::{DatabaseConfig, DatabaseSslMode};

pub type DatabasePool = PgPool;

pub async fn connect(config: &DatabaseConfig) -> Result<DatabasePool, DatabaseError> {
    let connect_options = PgConnectOptions::from_str(&config.url)
        .map_err(DatabaseError::InvalidUrl)?
        .ssl_mode(map_ssl_mode(config.ssl_mode));

    let pool = PgPoolOptions::new()
        .min_connections(config.min_connections)
        .max_connections(config.max_connections)
        .acquire_timeout(Duration::from_secs(config.acquire_timeout_seconds))
        .idle_timeout(Some(Duration::from_secs(config.idle_timeout_seconds)))
        .max_lifetime(Some(Duration::from_secs(config.max_lifetime_seconds)))
        .connect_with(connect_options)
        .await
        .map_err(DatabaseError::Connect)?;

    let version: String = sqlx::query_scalar("SELECT version()")
        .fetch_one(&pool)
        .await
        .map_err(DatabaseError::Probe)?;

    info!(database_version = %version, "connected to PostgreSQL");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(DatabaseError::Migrate)?;

    info!("database migrations applied");

    Ok(pool)
}

#[derive(Debug)]
pub enum DatabaseError {
    InvalidUrl(sqlx::Error),
    Connect(sqlx::Error),
    Probe(sqlx::Error),
    Migrate(sqlx::migrate::MigrateError),
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUrl(error) => write!(formatter, "failed to parse PostgreSQL connection options: {error}"),
            Self::Connect(error) => write!(formatter, "failed to connect to PostgreSQL: {error}"),
            Self::Probe(error) => write!(formatter, "failed to verify PostgreSQL connection: {error}"),
            Self::Migrate(error) => write!(formatter, "failed to run database migrations: {error}"),
        }
    }
}

impl std::error::Error for DatabaseError {}

fn map_ssl_mode(mode: DatabaseSslMode) -> PgSslMode {
    match mode {
        DatabaseSslMode::Disable => PgSslMode::Disable,
        DatabaseSslMode::Prefer => PgSslMode::Prefer,
        DatabaseSslMode::Require => PgSslMode::Require,
        DatabaseSslMode::VerifyCa => PgSslMode::VerifyCa,
        DatabaseSslMode::VerifyFull => PgSslMode::VerifyFull,
    }
}
