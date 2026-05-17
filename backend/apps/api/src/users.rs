#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub display_name: String,
    pub avatar_object_key: Option<String>,
    pub bio: Option<String>,
    pub email_verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct NewUser {
    pub email: String,
    pub password_hash: String,
    pub display_name: String,
}

pub async fn find_user_by_email(
    pool: &PgPool,
    email: &str,
) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>(
        r#"
        SELECT
            id,
            email,
            password_hash,
            display_name,
            avatar_object_key,
            bio,
            email_verified_at,
            created_at,
            updated_at
        FROM users
        WHERE email = $1
        "#,
    )
    .bind(email)
    .fetch_optional(pool)
    .await
}

pub async fn find_user_by_id(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>(
        r#"
        SELECT
            id,
            email,
            password_hash,
            display_name,
            avatar_object_key,
            bio,
            email_verified_at,
            created_at,
            updated_at
        FROM users
        WHERE id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

pub async fn insert_user(
    transaction: &mut Transaction<'_, Postgres>,
    new_user: &NewUser,
) -> Result<User, sqlx::Error> {
    sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (
            email,
            password_hash,
            display_name
        )
        VALUES ($1, $2, $3)
        RETURNING
            id,
            email,
            password_hash,
            display_name,
            avatar_object_key,
            bio,
            email_verified_at,
            created_at,
            updated_at
        "#,
    )
    .bind(&new_user.email)
    .bind(&new_user.password_hash)
    .bind(&new_user.display_name)
    .fetch_one(&mut **transaction)
    .await
}

pub async fn mark_email_verified(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    verified_at: DateTime<Utc>,
) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>(
        r#"
        UPDATE users
        SET
            email_verified_at = COALESCE(email_verified_at, $2),
            updated_at = NOW()
        WHERE id = $1
        RETURNING
            id,
            email,
            password_hash,
            display_name,
            avatar_object_key,
            bio,
            email_verified_at,
            created_at,
            updated_at
        "#,
    )
    .bind(user_id)
    .bind(verified_at)
    .fetch_optional(&mut **transaction)
    .await
}

pub async fn update_password_hash(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    password_hash: &str,
) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>(
        r#"
        UPDATE users
        SET
            password_hash = $2,
            updated_at = NOW()
        WHERE id = $1
        RETURNING
            id,
            email,
            password_hash,
            display_name,
            avatar_object_key,
            bio,
            email_verified_at,
            created_at,
            updated_at
        "#,
    )
    .bind(user_id)
    .bind(password_hash)
    .fetch_optional(&mut **transaction)
    .await
}
