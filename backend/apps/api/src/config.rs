use std::{
    fmt,
    net::{IpAddr, SocketAddr},
};

use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub jwt: JwtConfig,
    pub smtp: SmtpConfig,
    pub object_storage: ObjectStorageConfig,
}

impl Config {
    pub fn load() -> Result<Self, ConfigLoadError> {
        let raw = envy::from_env::<RawConfig>().map_err(ConfigLoadError::Parse)?;

        let config = Self {
            server: ServerConfig {
                host: raw.host,
                port: raw.port,
            },
            database: DatabaseConfig {
                url: raw.database_url,
            },
            jwt: JwtConfig {
                access_secret: raw.jwt_access_secret,
                refresh_secret: raw.jwt_refresh_secret,
                issuer: raw.jwt_issuer,
                access_ttl_seconds: raw.jwt_access_ttl_seconds,
                refresh_ttl_seconds: raw.jwt_refresh_ttl_seconds,
            },
            smtp: SmtpConfig {
                host: raw.smtp_host,
                port: raw.smtp_port,
                username: raw.smtp_username,
                password: raw.smtp_password,
                from_email: raw.smtp_from_email,
                from_name: raw.smtp_from_name,
                use_starttls: raw.smtp_use_starttls,
            },
            object_storage: ObjectStorageConfig {
                endpoint: raw.object_storage_endpoint,
                region: raw.object_storage_region,
                bucket: raw.object_storage_bucket,
                access_key_id: raw.object_storage_access_key_id,
                secret_access_key: raw.object_storage_secret_access_key,
                public_base_url: raw.object_storage_public_base_url,
            },
        };

        config.validate().map_err(ConfigLoadError::Invalid)?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        require_non_empty("DATABASE_URL", &self.database.url)?;
        require_non_empty("JWT_ACCESS_SECRET", &self.jwt.access_secret)?;
        require_non_empty("JWT_REFRESH_SECRET", &self.jwt.refresh_secret)?;
        require_non_empty("JWT_ISSUER", &self.jwt.issuer)?;
        require_non_empty("SMTP_HOST", &self.smtp.host)?;
        require_non_empty("SMTP_USERNAME", &self.smtp.username)?;
        require_non_empty("SMTP_PASSWORD", &self.smtp.password)?;
        require_non_empty("SMTP_FROM_EMAIL", &self.smtp.from_email)?;
        require_non_empty("SMTP_FROM_NAME", &self.smtp.from_name)?;
        require_non_empty("OBJECT_STORAGE_ENDPOINT", &self.object_storage.endpoint)?;
        require_non_empty("OBJECT_STORAGE_REGION", &self.object_storage.region)?;
        require_non_empty("OBJECT_STORAGE_BUCKET", &self.object_storage.bucket)?;
        require_non_empty(
            "OBJECT_STORAGE_ACCESS_KEY_ID",
            &self.object_storage.access_key_id,
        )?;
        require_non_empty(
            "OBJECT_STORAGE_SECRET_ACCESS_KEY",
            &self.object_storage.secret_access_key,
        )?;

        if self.server.port == 0 {
            return Err(ConfigError::invalid("PORT", "port must be greater than zero"));
        }

        if self.jwt.access_ttl_seconds == 0 {
            return Err(ConfigError::invalid(
                "JWT_ACCESS_TTL_SECONDS",
                "must be greater than zero",
            ));
        }

        if self.jwt.refresh_ttl_seconds == 0 {
            return Err(ConfigError::invalid(
                "JWT_REFRESH_TTL_SECONDS",
                "must be greater than zero",
            ));
        }

        if self.smtp.port == 0 {
            return Err(ConfigError::invalid(
                "SMTP_PORT",
                "port must be greater than zero",
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: IpAddr,
    pub port: u16,
}

impl ServerConfig {
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::from((self.host, self.port))
    }
}

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct JwtConfig {
    pub access_secret: String,
    pub refresh_secret: String,
    pub issuer: String,
    pub access_ttl_seconds: u64,
    pub refresh_ttl_seconds: u64,
}

#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_email: String,
    pub from_name: String,
    pub use_starttls: bool,
}

#[derive(Debug, Clone)]
pub struct ObjectStorageConfig {
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub public_base_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    #[serde(default = "default_host")]
    host: IpAddr,
    #[serde(default = "default_port")]
    port: u16,
    database_url: String,
    jwt_access_secret: String,
    jwt_refresh_secret: String,
    #[serde(default = "default_jwt_issuer")]
    jwt_issuer: String,
    #[serde(default = "default_access_ttl_seconds")]
    jwt_access_ttl_seconds: u64,
    #[serde(default = "default_refresh_ttl_seconds")]
    jwt_refresh_ttl_seconds: u64,
    smtp_host: String,
    #[serde(default = "default_smtp_port")]
    smtp_port: u16,
    smtp_username: String,
    smtp_password: String,
    smtp_from_email: String,
    #[serde(default = "default_smtp_from_name")]
    smtp_from_name: String,
    #[serde(default = "default_smtp_use_starttls")]
    smtp_use_starttls: bool,
    object_storage_endpoint: String,
    #[serde(default = "default_object_storage_region")]
    object_storage_region: String,
    object_storage_bucket: String,
    object_storage_access_key_id: String,
    object_storage_secret_access_key: String,
    object_storage_public_base_url: Option<String>,
}

#[derive(Debug)]
pub struct ConfigError {
    message: String,
}

impl ConfigError {
    fn invalid(field: &str, reason: impl Into<String>) -> Self {
        Self {
            message: format!("invalid configuration value for {field}: {}", reason.into()),
        }
    }

    fn missing_required(field: &str) -> Self {
        Self {
            message: format!("missing required configuration value: {field}"),
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ConfigError {}

#[derive(Debug)]
pub enum ConfigLoadError {
    Parse(envy::Error),
    Invalid(ConfigError),
}

impl fmt::Display for ConfigLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => {
                write!(formatter, "failed to parse environment configuration: {error}")
            }
            Self::Invalid(error) => write!(formatter, "configuration validation failed: {error}"),
        }
    }
}

impl std::error::Error for ConfigLoadError {}

fn require_non_empty(field: &str, value: &str) -> Result<(), ConfigError> {
    if value.trim().is_empty() {
        return Err(ConfigError::missing_required(field));
    }

    Ok(())
}

fn default_host() -> IpAddr {
    IpAddr::from([0, 0, 0, 0])
}

fn default_port() -> u16 {
    8080
}

fn default_jwt_issuer() -> String {
    "event-organization-api".to_owned()
}

fn default_access_ttl_seconds() -> u64 {
    900
}

fn default_refresh_ttl_seconds() -> u64 {
    2_592_000
}

fn default_smtp_port() -> u16 {
    587
}

fn default_smtp_from_name() -> String {
    "Event Organization".to_owned()
}

fn default_smtp_use_starttls() -> bool {
    true
}

fn default_object_storage_region() -> String {
    "auto".to_owned()
}
