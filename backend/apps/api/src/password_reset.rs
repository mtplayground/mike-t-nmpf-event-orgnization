#![allow(dead_code)]

use std::fmt::{self, Write as _};

use chrono::{DateTime, Duration, Utc};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct PasswordResetTokenRecord {
    pub token_hash: String,
    pub user_id: Uuid,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct IssuedPasswordResetToken {
    pub token: String,
    pub user_id: Uuid,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct PasswordResetService {
    ttl: Duration,
}

impl PasswordResetService {
    pub fn new() -> Self {
        Self {
            ttl: Duration::hours(1),
        }
    }

    pub async fn issue_for_user(
        &self,
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<IssuedPasswordResetToken, PasswordResetError> {
        let mut transaction = pool.begin().await?;
        let issued = self.issue_for_user_in_tx(&mut transaction, user_id).await?;
        transaction.commit().await?;
        Ok(issued)
    }

    pub async fn issue_for_user_in_tx(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        user_id: Uuid,
    ) -> Result<IssuedPasswordResetToken, PasswordResetError> {
        clear_pending_tokens(transaction, user_id).await?;

        let token = generate_reset_token();
        let token_hash = hash_password_reset_token(&token);
        let expires_at = Utc::now() + self.ttl;

        sqlx::query(
            r#"
            INSERT INTO password_reset_tokens (
                token_hash,
                user_id,
                expires_at
            )
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(&token_hash)
        .bind(user_id)
        .bind(expires_at)
        .execute(&mut **transaction)
        .await?;

        Ok(IssuedPasswordResetToken {
            token,
            user_id,
            expires_at,
        })
    }

    pub async fn consume_token_in_tx(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        token: &str,
    ) -> Result<PasswordResetTokenRecord, PasswordResetError> {
        let token_hash = hash_password_reset_token(token);
        let now = Utc::now();

        let record = sqlx::query_as::<_, PasswordResetTokenRecord>(
            r#"
            SELECT
                token_hash,
                user_id,
                expires_at,
                used_at,
                created_at
            FROM password_reset_tokens
            WHERE token_hash = $1
            FOR UPDATE
            "#,
        )
        .bind(&token_hash)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or(PasswordResetError::TokenNotFound)?;

        if record.used_at.is_some() {
            return Err(PasswordResetError::TokenAlreadyUsed);
        }

        if record.expires_at <= now {
            mark_token_used(transaction, &token_hash, now).await?;
            return Err(PasswordResetError::TokenExpired);
        }

        mark_token_used(transaction, &token_hash, now).await?;

        Ok(record)
    }
}

impl Default for PasswordResetService {
    fn default() -> Self {
        Self::new()
    }
}

pub fn hash_password_reset_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    let mut encoded = String::with_capacity(digest.len() * 2);

    for byte in digest {
        let _ = write!(&mut encoded, "{byte:02x}");
    }

    encoded
}

fn generate_reset_token() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);

    let mut encoded = String::with_capacity(bytes.len() * 2);

    for byte in bytes {
        let _ = write!(&mut encoded, "{byte:02x}");
    }

    encoded
}

async fn clear_pending_tokens(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE password_reset_tokens
        SET used_at = NOW()
        WHERE user_id = $1
          AND used_at IS NULL
        "#,
    )
    .bind(user_id)
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

async fn mark_token_used(
    transaction: &mut Transaction<'_, Postgres>,
    token_hash: &str,
    used_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE password_reset_tokens
        SET used_at = $2
        WHERE token_hash = $1
          AND used_at IS NULL
        "#,
    )
    .bind(token_hash)
    .bind(used_at)
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

#[derive(Debug)]
pub enum PasswordResetError {
    Database(sqlx::Error),
    TokenNotFound,
    TokenAlreadyUsed,
    TokenExpired,
}

impl fmt::Display for PasswordResetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "password reset database error: {error}"),
            Self::TokenNotFound => formatter.write_str("password reset token was not found"),
            Self::TokenAlreadyUsed => formatter.write_str("password reset token has already been used"),
            Self::TokenExpired => formatter.write_str("password reset token has expired"),
        }
    }
}

impl std::error::Error for PasswordResetError {}

impl From<sqlx::Error> for PasswordResetError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

#[cfg(test)]
mod tests {
    use super::hash_password_reset_token;

    #[test]
    fn password_reset_token_hash_is_deterministic() {
        let first = hash_password_reset_token("password-reset-token");
        let second = hash_password_reset_token("password-reset-token");

        assert_eq!(first, second);
        assert_eq!(first.len(), 64);
    }

    #[test]
    fn password_reset_token_hash_changes_with_input() {
        let first = hash_password_reset_token("password-reset-token");
        let second = hash_password_reset_token("other-password-reset-token");

        assert_ne!(first, second);
    }
}
