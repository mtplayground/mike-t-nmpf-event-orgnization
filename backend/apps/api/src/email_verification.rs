#![allow(dead_code)]

use std::fmt::{self, Write as _};

use chrono::{DateTime, Duration, Utc};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct EmailVerificationTokenRecord {
    pub token_hash: String,
    pub user_id: Uuid,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct IssuedEmailVerificationToken {
    pub token: String,
    pub user_id: Uuid,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct EmailVerificationService {
    ttl: Duration,
}

impl EmailVerificationService {
    pub fn new() -> Self {
        Self {
            ttl: Duration::hours(24),
        }
    }

    pub async fn issue_for_user(
        &self,
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<IssuedEmailVerificationToken, EmailVerificationError> {
        let mut transaction = pool.begin().await?;
        let issued = self.issue_for_user_in_tx(&mut transaction, user_id).await?;
        transaction.commit().await?;
        Ok(issued)
    }

    pub async fn issue_for_user_in_tx(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        user_id: Uuid,
    ) -> Result<IssuedEmailVerificationToken, EmailVerificationError> {
        clear_pending_tokens(transaction, user_id).await?;

        let token = generate_verification_token();
        let token_hash = hash_verification_token(&token);
        let expires_at = Utc::now() + self.ttl;

        sqlx::query(
            r#"
            INSERT INTO email_verification_tokens (
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

        Ok(IssuedEmailVerificationToken {
            token,
            user_id,
            expires_at,
        })
    }

    pub async fn consume_token_in_tx(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        token: &str,
    ) -> Result<EmailVerificationTokenRecord, EmailVerificationError> {
        let token_hash = hash_verification_token(token);
        let now = Utc::now();

        let record = sqlx::query_as::<_, EmailVerificationTokenRecord>(
            r#"
            SELECT
                token_hash,
                user_id,
                expires_at,
                used_at,
                created_at
            FROM email_verification_tokens
            WHERE token_hash = $1
            FOR UPDATE
            "#,
        )
        .bind(&token_hash)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or(EmailVerificationError::TokenNotFound)?;

        if record.used_at.is_some() {
            return Err(EmailVerificationError::TokenAlreadyUsed);
        }

        if record.expires_at <= now {
            sqlx::query(
                r#"
                UPDATE email_verification_tokens
                SET used_at = $2
                WHERE token_hash = $1
                  AND used_at IS NULL
                "#,
            )
            .bind(&token_hash)
            .bind(now)
            .execute(&mut **transaction)
            .await?;

            return Err(EmailVerificationError::TokenExpired);
        }

        sqlx::query(
            r#"
            UPDATE email_verification_tokens
            SET used_at = $2
            WHERE token_hash = $1
              AND used_at IS NULL
            "#,
        )
        .bind(&token_hash)
        .bind(now)
        .execute(&mut **transaction)
        .await?;

        Ok(record)
    }
}

impl Default for EmailVerificationService {
    fn default() -> Self {
        Self::new()
    }
}

pub fn hash_verification_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    let mut encoded = String::with_capacity(digest.len() * 2);

    for byte in digest {
        let _ = write!(&mut encoded, "{byte:02x}");
    }

    encoded
}

fn generate_verification_token() -> String {
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
        UPDATE email_verification_tokens
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

#[derive(Debug)]
pub enum EmailVerificationError {
    Database(sqlx::Error),
    TokenNotFound,
    TokenAlreadyUsed,
    TokenExpired,
}

impl fmt::Display for EmailVerificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "email verification database error: {error}"),
            Self::TokenNotFound => formatter.write_str("email verification token was not found"),
            Self::TokenAlreadyUsed => formatter.write_str("email verification token has already been used"),
            Self::TokenExpired => formatter.write_str("email verification token has expired"),
        }
    }
}

impl std::error::Error for EmailVerificationError {}

impl From<sqlx::Error> for EmailVerificationError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

#[cfg(test)]
mod tests {
    use super::hash_verification_token;

    #[test]
    fn verification_token_hash_is_deterministic() {
        let first = hash_verification_token("verification-token");
        let second = hash_verification_token("verification-token");

        assert_eq!(first, second);
        assert_eq!(first.len(), 64);
    }

    #[test]
    fn verification_token_hash_changes_with_input() {
        let first = hash_verification_token("verification-token");
        let second = hash_verification_token("other-token");

        assert_ne!(first, second);
    }
}
