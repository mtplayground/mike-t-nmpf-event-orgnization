#![allow(dead_code)]

use std::{fmt, time::Duration};

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use tokio::time::{self, MissedTickBehavior};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::email::EmailService;

const REMINDER_SCAN_INTERVAL: Duration = Duration::from_secs(5 * 60);

#[derive(Clone)]
pub struct ReminderService;

impl ReminderService {
    pub fn spawn(pool: PgPool, email_service: EmailService) -> Self {
        tokio::spawn(run_24h_reminder_worker(pool, email_service));
        Self
    }
}

async fn run_24h_reminder_worker(pool: PgPool, email_service: EmailService) {
    let mut interval = time::interval(REMINDER_SCAN_INTERVAL);
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        interval.tick().await;

        match enqueue_due_24h_reminders(&pool, &email_service).await {
            Ok(enqueued_count) if enqueued_count > 0 => {
                info!(enqueued_count, "enqueued 24h event reminder emails");
            }
            Ok(_) => {}
            Err(error) => {
                error!(error = %error, "failed to scan 24h event reminders");
            }
        }
    }
}

pub async fn enqueue_due_24h_reminders(
    pool: &PgPool,
    email_service: &EmailService,
) -> Result<usize, ReminderError> {
    let due_reminders = claim_due_24h_reminders(pool).await?;
    let mut enqueued_count = 0;

    for reminder in due_reminders {
        let location = location_description(
            reminder.location_text.as_deref(),
            reminder.location_url.as_deref(),
        );
        match email_service
            .send_event_reminder_email(
                &reminder.email,
                &reminder.display_name,
                &reminder.event_title,
                reminder.start_at,
                location.as_deref(),
            )
            .await
        {
            Ok(()) => {
                enqueued_count += 1;
            }
            Err(error) => {
                warn!(
                    error = %error,
                    reminder_id = %reminder.reminder_id,
                    registration_id = %reminder.registration_id,
                    "failed to enqueue 24h reminder email"
                );
                release_reminder_claim(pool, reminder.reminder_id).await?;
            }
        }
    }

    Ok(enqueued_count)
}

pub async fn claim_due_24h_reminders(
    pool: &PgPool,
) -> Result<Vec<DueReminderRecipient>, sqlx::Error> {
    sqlx::query_as::<_, DueReminderRecipient>(CLAIM_DUE_24H_REMINDERS_SQL)
        .fetch_all(pool)
        .await
}

async fn release_reminder_claim(pool: &PgPool, reminder_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(RELEASE_REMINDER_CLAIM_SQL)
        .bind(reminder_id)
        .execute(pool)
        .await?;

    Ok(())
}

#[derive(Debug, Clone, FromRow)]
pub struct DueReminderRecipient {
    pub reminder_id: Uuid,
    pub registration_id: Uuid,
    pub event_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub event_title: String,
    pub start_at: DateTime<Utc>,
    pub location_text: Option<String>,
    pub location_url: Option<String>,
}

pub const CLAIM_DUE_24H_REMINDERS_SQL: &str = r#"
    WITH claimed AS (
        INSERT INTO registration_reminders (
            registration_id,
            event_id,
            reminder_kind
        )
        SELECT
            registrations.id,
            registrations.event_id,
            '24h'
        FROM registrations
        INNER JOIN events
          ON events.id = registrations.event_id
        WHERE registrations.status = 'registered'
          AND events.visibility = 'public'
          AND events.status = 'published'
          AND events.cancelled_at IS NULL
          AND events.start_at >= NOW() + INTERVAL '23 hours 30 minutes'
          AND events.start_at < NOW() + INTERVAL '24 hours 30 minutes'
        ON CONFLICT (registration_id, reminder_kind) DO NOTHING
        RETURNING
            id,
            registration_id,
            event_id
    )
    SELECT
        claimed.id AS reminder_id,
        claimed.registration_id,
        claimed.event_id,
        users.email,
        users.display_name,
        events.title AS event_title,
        events.start_at,
        events.location_text,
        events.location_url
    FROM claimed
    INNER JOIN registrations
      ON registrations.id = claimed.registration_id
    INNER JOIN users
      ON users.id = registrations.user_id
    INNER JOIN events
      ON events.id = claimed.event_id
    ORDER BY events.start_at ASC, users.email ASC
"#;

pub const RELEASE_REMINDER_CLAIM_SQL: &str = r#"
    DELETE FROM registration_reminders
    WHERE id = $1
"#;

fn location_description(location_text: Option<&str>, location_url: Option<&str>) -> Option<String> {
    match (location_text, location_url) {
        (Some(text), Some(url)) if !text.trim().is_empty() && !url.trim().is_empty() => {
            Some(format!("{} ({})", text.trim(), url.trim()))
        }
        (Some(text), _) if !text.trim().is_empty() => Some(text.trim().to_owned()),
        (_, Some(url)) if !url.trim().is_empty() => Some(url.trim().to_owned()),
        _ => None,
    }
}

#[derive(Debug)]
pub enum ReminderError {
    Database(sqlx::Error),
}

impl fmt::Display for ReminderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "database error: {error}"),
        }
    }
}

impl std::error::Error for ReminderError {}

impl From<sqlx::Error> for ReminderError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CLAIM_DUE_24H_REMINDERS_SQL, RELEASE_REMINDER_CLAIM_SQL, location_description,
    };

    #[test]
    fn reminder_query_claims_only_due_active_public_registrations() {
        assert!(CLAIM_DUE_24H_REMINDERS_SQL.contains("INSERT INTO registration_reminders"));
        assert!(CLAIM_DUE_24H_REMINDERS_SQL.contains("ON CONFLICT"));
        assert!(CLAIM_DUE_24H_REMINDERS_SQL.contains("registrations.status = 'registered'"));
        assert!(CLAIM_DUE_24H_REMINDERS_SQL.contains("events.visibility = 'public'"));
        assert!(CLAIM_DUE_24H_REMINDERS_SQL.contains("events.status = 'published'"));
        assert!(CLAIM_DUE_24H_REMINDERS_SQL.contains("events.cancelled_at IS NULL"));
        assert!(
            CLAIM_DUE_24H_REMINDERS_SQL
                .contains("events.start_at >= NOW() + INTERVAL '23 hours 30 minutes'")
        );
        assert!(
            CLAIM_DUE_24H_REMINDERS_SQL
                .contains("events.start_at < NOW() + INTERVAL '24 hours 30 minutes'")
        );
    }

    #[test]
    fn reminder_claim_release_deletes_by_claim_id() {
        assert!(RELEASE_REMINDER_CLAIM_SQL.contains("DELETE FROM registration_reminders"));
        assert!(RELEASE_REMINDER_CLAIM_SQL.contains("WHERE id = $1"));
    }

    #[test]
    fn reminder_location_description_combines_text_and_url() {
        assert_eq!(
            location_description(Some("Main Hall"), Some("https://example.com/live")),
            Some("Main Hall (https://example.com/live)".to_owned()),
        );
        assert_eq!(
            location_description(Some("  Main Hall  "), None),
            Some("Main Hall".to_owned()),
        );
        assert_eq!(location_description(None, Some("  https://example.com/live  ")), Some(
            "https://example.com/live".to_owned()
        ));
        assert_eq!(location_description(Some(" "), Some(" ")), None);
    }
}
