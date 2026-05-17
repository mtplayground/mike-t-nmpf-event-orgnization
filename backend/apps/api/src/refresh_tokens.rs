#![allow(dead_code)]

use std::fmt::{self, Write as _};

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::{Executor, FromRow, PgPool, Postgres};
use uuid::Uuid;

use crate::auth::{AuthError, JwtService, TokenEnvelope};

#[derive(Debug, Clone, FromRow)]
pub struct RefreshTokenRecord {
    pub token_hash: String,
    pub user_id: Uuid,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct RefreshTokenService {
    jwt_service: JwtService,
}

impl RefreshTokenService {
    pub fn new(jwt_service: JwtService) -> Self {
        Self { jwt_service }
    }

    pub async fn issue_for_user(
        &self,
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<TokenEnvelope, RefreshTokenError> {
        let token = self.jwt_service.issue_refresh_token(user_id)?;
        let token_hash = hash_refresh_token(&token.token);

        insert_refresh_token(
            pool,
            &token_hash,
            user_id,
            token.claims.expires_at,
            None,
        )
        .await?;

        Ok(token)
    }

    pub async fn rotate(
        &self,
        pool: &PgPool,
        presented_token: &str,
    ) -> Result<TokenEnvelope, RefreshTokenError> {
        let claims = self.jwt_service.verify_refresh_token(presented_token)?;
        let current_hash = hash_refresh_token(presented_token);
        let mut transaction = pool.begin().await?;
        let now = Utc::now();

        let existing = find_refresh_token_for_update(&mut *transaction, &current_hash)
            .await?
            .ok_or(RefreshTokenError::RefreshTokenNotFound)?;

        if existing.user_id != claims.subject {
            return Err(RefreshTokenError::TokenSubjectMismatch {
                expected: existing.user_id,
                actual: claims.subject,
            });
        }

        if existing.revoked_at.is_some() {
            return Err(RefreshTokenError::RefreshTokenRevoked);
        }

        if existing.expires_at <= now {
            revoke_refresh_token(&mut *transaction, &existing.token_hash, now).await?;
            return Err(RefreshTokenError::RefreshTokenExpired);
        }

        revoke_refresh_token(&mut *transaction, &existing.token_hash, now).await?;

        let replacement = self.jwt_service.issue_refresh_token(existing.user_id)?;
        let replacement_hash = hash_refresh_token(&replacement.token);

        insert_refresh_token(
            &mut *transaction,
            &replacement_hash,
            existing.user_id,
            replacement.claims.expires_at,
            None,
        )
        .await?;

        transaction.commit().await?;

        Ok(replacement)
    }

    pub async fn revoke_all_for_user(
        &self,
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<u64, RefreshTokenError> {
        let result = sqlx::query(
            r#"
            UPDATE refresh_tokens
            SET revoked_at = NOW()
            WHERE user_id = $1
              AND revoked_at IS NULL
            "#,
        )
        .bind(user_id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }
}

pub fn hash_refresh_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    let mut encoded = String::with_capacity(digest.len() * 2);

    for byte in digest {
        let _ = write!(&mut encoded, "{byte:02x}");
    }

    encoded
}

#[derive(Debug)]
pub enum RefreshTokenError {
    Auth(AuthError),
    Database(sqlx::Error),
    RefreshTokenNotFound,
    RefreshTokenRevoked,
    RefreshTokenExpired,
    TokenSubjectMismatch { expected: Uuid, actual: Uuid },
}

impl fmt::Display for RefreshTokenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auth(error) => write!(formatter, "{error}"),
            Self::Database(error) => write!(formatter, "refresh token database error: {error}"),
            Self::RefreshTokenNotFound => formatter.write_str("refresh token was not found"),
            Self::RefreshTokenRevoked => formatter.write_str("refresh token has already been revoked"),
            Self::RefreshTokenExpired => formatter.write_str("refresh token has expired"),
            Self::TokenSubjectMismatch { expected, actual } => write!(
                formatter,
                "refresh token subject mismatch: expected {expected}, received {actual}"
            ),
        }
    }
}

impl std::error::Error for RefreshTokenError {}

impl From<AuthError> for RefreshTokenError {
    fn from(value: AuthError) -> Self {
        Self::Auth(value)
    }
}

impl From<sqlx::Error> for RefreshTokenError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

async fn insert_refresh_token<'a, E>(
    executor: E,
    token_hash: &str,
    user_id: Uuid,
    expires_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
) -> Result<(), sqlx::Error>
where
    E: Executor<'a, Database = Postgres>,
{
    sqlx::query(
        r#"
        INSERT INTO refresh_tokens (
            token_hash,
            user_id,
            expires_at,
            revoked_at
        )
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(token_hash)
    .bind(user_id)
    .bind(expires_at)
    .bind(revoked_at)
    .execute(executor)
    .await?;

    Ok(())
}

async fn find_refresh_token_for_update<'a, E>(
    executor: E,
    token_hash: &str,
) -> Result<Option<RefreshTokenRecord>, sqlx::Error>
where
    E: Executor<'a, Database = Postgres>,
{
    sqlx::query_as::<_, RefreshTokenRecord>(
        r#"
        SELECT
            token_hash,
            user_id,
            expires_at,
            revoked_at,
            created_at
        FROM refresh_tokens
        WHERE token_hash = $1
        FOR UPDATE
        "#,
    )
    .bind(token_hash)
    .fetch_optional(executor)
    .await
}

async fn revoke_refresh_token<'a, E>(
    executor: E,
    token_hash: &str,
    revoked_at: DateTime<Utc>,
) -> Result<(), sqlx::Error>
where
    E: Executor<'a, Database = Postgres>,
{
    sqlx::query(
        r#"
        UPDATE refresh_tokens
        SET revoked_at = $2
        WHERE token_hash = $1
          AND revoked_at IS NULL
        "#,
    )
    .bind(token_hash)
    .bind(revoked_at)
    .execute(executor)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::hash_refresh_token;

    #[test]
    fn refresh_token_hash_is_deterministic() {
        let first = hash_refresh_token("refresh-token-value");
        let second = hash_refresh_token("refresh-token-value");

        assert_eq!(first, second);
        assert_eq!(first.len(), 64);
    }

    #[test]
    fn refresh_token_hash_changes_with_input() {
        let first = hash_refresh_token("refresh-token-value");
        let second = hash_refresh_token("different-refresh-token-value");

        assert_ne!(first, second);
    }
}
