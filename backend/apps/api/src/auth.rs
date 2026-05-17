#![allow(dead_code)]

use std::fmt;

use argon2::{
    Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version,
    password_hash::{Error as PasswordHashError, SaltString},
};
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{
    Algorithm as JwtAlgorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode,
    errors::ErrorKind as JwtErrorKind,
};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::JwtConfig;

#[derive(Clone)]
pub struct PasswordService {
    argon2: Argon2<'static>,
}

impl PasswordService {
    pub fn new() -> Result<Self, AuthError> {
        let params = Params::new(19_456, 2, 1, None)
            .map_err(|error| AuthError::PasswordConfig(error.to_string()))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        Ok(Self { argon2 })
    }

    pub fn hash_password(&self, password: &str) -> Result<String, AuthError> {
        let salt = SaltString::generate(&mut OsRng);

        self.argon2
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(AuthError::PasswordHash)
    }

    pub fn verify_password(
        &self,
        password: &str,
        password_hash: &str,
    ) -> Result<bool, AuthError> {
        let parsed_hash = PasswordHash::new(password_hash).map_err(AuthError::PasswordHash)?;

        match self
            .argon2
            .verify_password(password.as_bytes(), &parsed_hash)
        {
            Ok(()) => Ok(true),
            Err(PasswordHashError::Password) => Ok(false),
            Err(error) => Err(AuthError::PasswordHash(error)),
        }
    }
}

#[derive(Clone)]
pub struct JwtService {
    issuer: String,
    access_secret: String,
    refresh_secret: String,
    access_ttl: Duration,
    refresh_ttl: Duration,
}

impl JwtService {
    pub fn from_config(config: &JwtConfig) -> Result<Self, AuthError> {
        let access_ttl = Duration::from_std(std::time::Duration::from_secs(
            config.access_ttl_seconds,
        ))
        .map_err(|error| AuthError::InvalidTtl(error.to_string()))?;
        let refresh_ttl = Duration::from_std(std::time::Duration::from_secs(
            config.refresh_ttl_seconds,
        ))
        .map_err(|error| AuthError::InvalidTtl(error.to_string()))?;

        Ok(Self {
            issuer: config.issuer.clone(),
            access_secret: config.access_secret.clone(),
            refresh_secret: config.refresh_secret.clone(),
            access_ttl,
            refresh_ttl,
        })
    }

    pub fn issue_access_token(&self, user_id: Uuid) -> Result<TokenEnvelope, AuthError> {
        self.issue_token(user_id, TokenKind::Access, self.access_ttl, &self.access_secret)
    }

    pub fn issue_refresh_token(&self, user_id: Uuid) -> Result<TokenEnvelope, AuthError> {
        self.issue_token(user_id, TokenKind::Refresh, self.refresh_ttl, &self.refresh_secret)
    }

    pub fn verify_access_token(&self, token: &str) -> Result<TokenClaims, AuthError> {
        self.verify_token(token, TokenKind::Access, &self.access_secret)
    }

    pub fn verify_refresh_token(&self, token: &str) -> Result<TokenClaims, AuthError> {
        self.verify_token(token, TokenKind::Refresh, &self.refresh_secret)
    }

    fn issue_token(
        &self,
        user_id: Uuid,
        token_kind: TokenKind,
        ttl: Duration,
        secret: &str,
    ) -> Result<TokenEnvelope, AuthError> {
        let issued_at = Utc::now();
        let expires_at = issued_at + ttl;
        let claims = JwtClaims {
            sub: user_id,
            token_type: token_kind,
            iss: self.issuer.clone(),
            iat: issued_at.timestamp(),
            exp: expires_at.timestamp(),
        };

        let token = encode(
            &Header::new(JwtAlgorithm::HS256),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .map_err(AuthError::TokenEncode)?;

        Ok(TokenEnvelope {
            token,
            claims: TokenClaims::from_jwt_claims(claims)?,
        })
    }

    fn verify_token(
        &self,
        token: &str,
        expected_kind: TokenKind,
        secret: &str,
    ) -> Result<TokenClaims, AuthError> {
        let mut validation = Validation::new(JwtAlgorithm::HS256);
        validation.set_issuer(&[self.issuer.as_str()]);
        validation.validate_exp = true;

        let decoded = decode::<JwtClaims>(
            token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &validation,
        )
        .map_err(AuthError::TokenDecode)?;

        if decoded.claims.token_type != expected_kind {
            return Err(AuthError::UnexpectedTokenKind {
                expected: expected_kind,
                actual: decoded.claims.token_type,
            });
        }

        TokenClaims::from_jwt_claims(decoded.claims)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TokenKind {
    Access,
    Refresh,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Access => formatter.write_str("access"),
            Self::Refresh => formatter.write_str("refresh"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TokenEnvelope {
    pub token: String,
    pub claims: TokenClaims,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenClaims {
    pub subject: Uuid,
    pub token_type: TokenKind,
    pub issuer: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl TokenClaims {
    fn from_jwt_claims(claims: JwtClaims) -> Result<Self, AuthError> {
        Ok(Self {
            subject: claims.sub,
            token_type: claims.token_type,
            issuer: claims.iss,
            issued_at: timestamp_to_utc(claims.iat)?,
            expires_at: timestamp_to_utc(claims.exp)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JwtClaims {
    sub: Uuid,
    token_type: TokenKind,
    iss: String,
    iat: i64,
    exp: i64,
}

#[derive(Debug)]
pub enum AuthError {
    PasswordConfig(String),
    PasswordHash(PasswordHashError),
    InvalidTtl(String),
    InvalidTimestamp(i64),
    TokenEncode(jsonwebtoken::errors::Error),
    TokenDecode(jsonwebtoken::errors::Error),
    UnexpectedTokenKind {
        expected: TokenKind,
        actual: TokenKind,
    },
}

impl fmt::Display for AuthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PasswordConfig(message) => {
                write!(formatter, "invalid password hashing configuration: {message}")
            }
            Self::PasswordHash(error) => write!(formatter, "password hashing error: {error}"),
            Self::InvalidTtl(message) => write!(formatter, "invalid JWT TTL configuration: {message}"),
            Self::InvalidTimestamp(value) => {
                write!(formatter, "invalid JWT timestamp value: {value}")
            }
            Self::TokenEncode(error) => write!(formatter, "failed to encode JWT: {error}"),
            Self::TokenDecode(error) => match error.kind() {
                JwtErrorKind::ExpiredSignature => formatter.write_str("JWT has expired"),
                _ => write!(formatter, "failed to decode JWT: {error}"),
            },
            Self::UnexpectedTokenKind { expected, actual } => write!(
                formatter,
                "unexpected token kind: expected {expected}, received {actual}"
            ),
        }
    }
}

impl std::error::Error for AuthError {}

fn timestamp_to_utc(value: i64) -> Result<DateTime<Utc>, AuthError> {
    DateTime::from_timestamp(value, 0).ok_or(AuthError::InvalidTimestamp(value))
}

#[cfg(test)]
mod tests {
    use super::{JwtService, PasswordService, TokenKind};
    use crate::config::JwtConfig;
    use uuid::Uuid;

    fn jwt_config() -> JwtConfig {
        JwtConfig {
            access_secret: "access-secret-for-tests".to_owned(),
            refresh_secret: "refresh-secret-for-tests".to_owned(),
            issuer: "event-organization-api".to_owned(),
            access_ttl_seconds: 900,
            refresh_ttl_seconds: 2_592_000,
        }
    }

    #[test]
    fn password_hash_round_trip_succeeds() {
        let service = match PasswordService::new() {
            Ok(service) => service,
            Err(error) => panic!("password service should initialize: {error}"),
        };

        let hash = match service.hash_password("correct horse battery staple") {
            Ok(hash) => hash,
            Err(error) => panic!("password hash should succeed: {error}"),
        };

        let verified = match service.verify_password("correct horse battery staple", &hash) {
            Ok(verified) => verified,
            Err(error) => panic!("password verify should succeed: {error}"),
        };

        assert!(verified);
    }

    #[test]
    fn password_hash_rejects_wrong_password() {
        let service = match PasswordService::new() {
            Ok(service) => service,
            Err(error) => panic!("password service should initialize: {error}"),
        };

        let hash = match service.hash_password("correct horse battery staple") {
            Ok(hash) => hash,
            Err(error) => panic!("password hash should succeed: {error}"),
        };

        let verified = match service.verify_password("wrong password", &hash) {
            Ok(verified) => verified,
            Err(error) => panic!("password verify should succeed: {error}"),
        };

        assert!(!verified);
    }

    #[test]
    fn access_token_round_trip_succeeds() {
        let service = match JwtService::from_config(&jwt_config()) {
            Ok(service) => service,
            Err(error) => panic!("jwt service should initialize: {error}"),
        };
        let user_id = Uuid::new_v4();

        let token = match service.issue_access_token(user_id) {
            Ok(token) => token,
            Err(error) => panic!("access token issuance should succeed: {error}"),
        };

        let claims = match service.verify_access_token(&token.token) {
            Ok(claims) => claims,
            Err(error) => panic!("access token verification should succeed: {error}"),
        };

        assert_eq!(claims.subject, user_id);
        assert_eq!(claims.token_type, TokenKind::Access);
        assert_eq!(claims.issuer, "event-organization-api");
    }

    #[test]
    fn refresh_token_is_rejected_by_access_verifier() {
        let service = match JwtService::from_config(&jwt_config()) {
            Ok(service) => service,
            Err(error) => panic!("jwt service should initialize: {error}"),
        };

        let token = match service.issue_refresh_token(Uuid::new_v4()) {
            Ok(token) => token,
            Err(error) => panic!("refresh token issuance should succeed: {error}"),
        };

        let error = match service.verify_access_token(&token.token) {
            Ok(_) => panic!("refresh token should not verify as access token"),
            Err(error) => error,
        };

        assert!(error
            .to_string()
            .contains("unexpected token kind"));
    }
}
