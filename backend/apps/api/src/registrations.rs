#![allow(dead_code)]

use std::{fmt, str::FromStr};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Registration {
    pub id: Uuid,
    pub event_id: Uuid,
    pub user_id: Uuid,
    pub status: RegistrationStatus,
    pub registered_at: DateTime<Utc>,
    pub cancelled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct HostAttendeeRow {
    pub registration_id: Uuid,
    pub user_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub status: RegistrationStatus,
    pub registered_at: DateTime<Utc>,
    pub cancelled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationInsertOutcome {
    Registered,
    EventNotFound,
    CapacityFull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum RegistrationStatus {
    Registered,
    Cancelled,
}

impl RegistrationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Registered => "registered",
            Self::Cancelled => "cancelled",
        }
    }
}

impl fmt::Display for RegistrationStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for RegistrationStatus {
    type Err = RegistrationStatusParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "registered" => Ok(Self::Registered),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(RegistrationStatusParseError { value: value.to_owned() }),
        }
    }
}

pub async fn register_for_event(
    pool: &PgPool,
    event_id: Uuid,
    user_id: Uuid,
) -> Result<(RegistrationInsertOutcome, Option<Registration>), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let Some(event) = sqlx::query_as::<_, RegistrationEventLock>(
        "SELECT id, capacity FROM events WHERE id = $1 FOR UPDATE",
    )
    .bind(event_id)
    .fetch_optional(&mut *transaction)
    .await?
    else {
        transaction.commit().await?;
        return Ok((RegistrationInsertOutcome::EventNotFound, None));
    };

    if let Some(existing) =
        find_registration_for_user_in_tx(&mut transaction, event_id, user_id).await?
    {
        if existing.status == RegistrationStatus::Registered {
            transaction.commit().await?;
            return Ok((RegistrationInsertOutcome::Registered, Some(existing)));
        }
    }

    let active_count =
        active_registration_count_for_event_in_tx(&mut transaction, event.id).await?;

    if matches!(event.capacity, Some(capacity) if active_count >= i64::from(capacity)) {
        transaction.commit().await?;
        return Ok((RegistrationInsertOutcome::CapacityFull, None));
    }

    let registration = sqlx::query_as::<_, Registration>(UPSERT_REGISTERED_REGISTRATION_SQL)
        .bind(event_id)
        .bind(user_id)
        .fetch_one(&mut *transaction)
        .await?;

    transaction.commit().await?;

    Ok((RegistrationInsertOutcome::Registered, Some(registration)))
}

pub async fn cancel_registration(
    pool: &PgPool,
    event_id: Uuid,
    user_id: Uuid,
) -> Result<Option<Registration>, sqlx::Error> {
    sqlx::query_as::<_, Registration>(CANCEL_REGISTRATION_SQL)
        .bind(event_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
}

pub async fn find_registration_for_user(
    pool: &PgPool,
    event_id: Uuid,
    user_id: Uuid,
) -> Result<Option<Registration>, sqlx::Error> {
    sqlx::query_as::<_, Registration>(SELECT_REGISTRATION_FOR_USER_SQL)
        .bind(event_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
}

pub async fn active_registration_count_for_event(
    pool: &PgPool,
    event_id: Uuid,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(ACTIVE_REGISTRATION_COUNT_SQL).bind(event_id).fetch_one(pool).await
}

pub async fn list_attendees_for_event(
    pool: &PgPool,
    event_id: Uuid,
) -> Result<Vec<HostAttendeeRow>, sqlx::Error> {
    sqlx::query_as::<_, HostAttendeeRow>(LIST_ATTENDEES_FOR_EVENT_SQL)
        .bind(event_id)
        .fetch_all(pool)
        .await
}

pub async fn list_active_attendees_for_event(
    pool: &PgPool,
    event_id: Uuid,
) -> Result<Vec<HostAttendeeRow>, sqlx::Error> {
    sqlx::query_as::<_, HostAttendeeRow>(
        r#"
        SELECT
            registrations.id AS registration_id,
            users.id AS user_id,
            users.email,
            users.display_name,
            registrations.status,
            registrations.registered_at,
            registrations.cancelled_at
        FROM registrations
        INNER JOIN users
          ON users.id = registrations.user_id
        WHERE registrations.event_id = $1
          AND registrations.status = 'registered'
        ORDER BY registrations.registered_at ASC, users.display_name ASC
        "#,
    )
    .bind(event_id)
    .fetch_all(pool)
    .await
}

async fn find_registration_for_user_in_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event_id: Uuid,
    user_id: Uuid,
) -> Result<Option<Registration>, sqlx::Error> {
    sqlx::query_as::<_, Registration>(
        r#"
        SELECT
            id,
            event_id,
            user_id,
            status,
            registered_at,
            cancelled_at
        FROM registrations
        WHERE event_id = $1
          AND user_id = $2
        FOR UPDATE
        "#,
    )
    .bind(event_id)
    .bind(user_id)
    .fetch_optional(&mut **transaction)
    .await
}

async fn active_registration_count_for_event_in_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event_id: Uuid,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(ACTIVE_REGISTRATION_COUNT_SQL)
        .bind(event_id)
        .fetch_one(&mut **transaction)
        .await
}

#[derive(Debug, Clone, FromRow)]
struct RegistrationEventLock {
    id: Uuid,
    capacity: Option<i32>,
}

pub const UPSERT_REGISTERED_REGISTRATION_SQL: &str = r#"
    INSERT INTO registrations (
        event_id,
        user_id,
        status,
        registered_at,
        cancelled_at
    )
    VALUES ($1, $2, 'registered', NOW(), NULL)
    ON CONFLICT (event_id, user_id) DO UPDATE
    SET
        status = 'registered',
        registered_at = CASE
            WHEN registrations.status = 'registered' THEN registrations.registered_at
            ELSE NOW()
        END,
        cancelled_at = NULL
    RETURNING
        id,
        event_id,
        user_id,
        status,
        registered_at,
        cancelled_at
"#;

pub const CANCEL_REGISTRATION_SQL: &str = r#"
    UPDATE registrations
    SET
        status = 'cancelled',
        cancelled_at = COALESCE(cancelled_at, NOW())
    WHERE event_id = $1
      AND user_id = $2
    RETURNING
        id,
        event_id,
        user_id,
        status,
        registered_at,
        cancelled_at
"#;

pub const SELECT_REGISTRATION_FOR_USER_SQL: &str = r#"
    SELECT
        id,
        event_id,
        user_id,
        status,
        registered_at,
        cancelled_at
    FROM registrations
    WHERE event_id = $1
      AND user_id = $2
"#;

pub const ACTIVE_REGISTRATION_COUNT_SQL: &str = r#"
    SELECT COUNT(*)::BIGINT
    FROM registrations
    WHERE event_id = $1
      AND status = 'registered'
"#;

pub const LIST_ATTENDEES_FOR_EVENT_SQL: &str = r#"
    SELECT
        registrations.id AS registration_id,
        users.id AS user_id,
        users.email,
        users.display_name,
        registrations.status,
        registrations.registered_at,
        registrations.cancelled_at
    FROM registrations
    INNER JOIN users
      ON users.id = registrations.user_id
    WHERE registrations.event_id = $1
    ORDER BY registrations.registered_at ASC, users.display_name ASC
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationStatusParseError {
    value: String,
}

impl fmt::Display for RegistrationStatusParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unsupported registration status value: {}", self.value)
    }
}

impl std::error::Error for RegistrationStatusParseError {}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{
        ACTIVE_REGISTRATION_COUNT_SQL, CANCEL_REGISTRATION_SQL, LIST_ATTENDEES_FOR_EVENT_SQL,
        RegistrationStatus, UPSERT_REGISTERED_REGISTRATION_SQL,
    };

    #[test]
    fn registration_status_round_trip_strings() {
        assert_eq!(RegistrationStatus::Registered.as_str(), "registered");
        assert_eq!(RegistrationStatus::Cancelled.as_str(), "cancelled");
        assert_eq!(
            RegistrationStatus::from_str("registered").expect("registered should parse"),
            RegistrationStatus::Registered,
        );
    }

    #[test]
    fn registration_write_queries_manage_active_status() {
        assert!(UPSERT_REGISTERED_REGISTRATION_SQL.contains("ON CONFLICT (event_id, user_id)"));
        assert!(UPSERT_REGISTERED_REGISTRATION_SQL.contains("status = 'registered'"));
        assert!(UPSERT_REGISTERED_REGISTRATION_SQL.contains("cancelled_at = NULL"));
        assert!(CANCEL_REGISTRATION_SQL.contains("status = 'cancelled'"));
        assert!(CANCEL_REGISTRATION_SQL.contains("cancelled_at = COALESCE(cancelled_at, NOW())"));
    }

    #[test]
    fn active_registration_count_only_counts_registered_rows() {
        assert!(ACTIVE_REGISTRATION_COUNT_SQL.contains("status = 'registered'"));
    }

    #[test]
    fn attendee_list_query_joins_registration_users() {
        assert!(LIST_ATTENDEES_FOR_EVENT_SQL.contains("INNER JOIN users"));
        assert!(LIST_ATTENDEES_FOR_EVENT_SQL.contains("registrations.event_id = $1"));
        assert!(LIST_ATTENDEES_FOR_EVENT_SQL.contains("users.email"));
    }
}
