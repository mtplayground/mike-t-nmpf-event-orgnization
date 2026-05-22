#![allow(dead_code)]

use std::{fmt, str::FromStr};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Event {
    pub id: Uuid,
    pub host_id: Uuid,
    pub title: String,
    pub slug: String,
    pub description_md: String,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    pub timezone: String,
    pub location_type: EventLocationType,
    pub location_text: Option<String>,
    pub location_url: Option<String>,
    pub capacity: Option<i32>,
    pub visibility: EventVisibility,
    pub status: EventStatus,
    pub cover_image_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub cancelled_at: Option<DateTime<Utc>>,
}

pub struct NewEvent {
    pub host_id: Uuid,
    pub title: String,
    pub slug: String,
    pub description_md: String,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    pub timezone: String,
    pub location_type: EventLocationType,
    pub location_text: Option<String>,
    pub location_url: Option<String>,
    pub capacity: Option<i32>,
    pub visibility: EventVisibility,
    pub status: EventStatus,
    pub cover_image_id: Option<Uuid>,
}

pub struct EventChanges {
    pub title: String,
    pub slug: String,
    pub description_md: String,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    pub timezone: String,
    pub location_type: EventLocationType,
    pub location_text: Option<String>,
    pub location_url: Option<String>,
    pub capacity: Option<i32>,
    pub visibility: EventVisibility,
    pub status: EventStatus,
    pub cover_image_id: Option<Uuid>,
}

pub async fn insert_event(pool: &PgPool, event: &NewEvent) -> Result<Event, sqlx::Error> {
    sqlx::query_as::<_, Event>(
        r#"
        INSERT INTO events (
            host_id,
            title,
            slug,
            description_md,
            start_at,
            end_at,
            timezone,
            location_type,
            location_text,
            location_url,
            capacity,
            visibility,
            status,
            cover_image_id,
            cancelled_at
        )
        VALUES (
            $1,
            $2,
            $3,
            $4,
            $5,
            $6,
            $7,
            $8,
            $9,
            $10,
            $11,
            $12,
            $13,
            $14,
            CASE WHEN $13 = 'cancelled' THEN NOW() ELSE NULL END
        )
        RETURNING
            id,
            host_id,
            title,
            slug,
            description_md,
            start_at,
            end_at,
            timezone,
            location_type,
            location_text,
            location_url,
            capacity,
            visibility,
            status,
            cover_image_id,
            created_at,
            updated_at,
            cancelled_at
        "#,
    )
    .bind(event.host_id)
    .bind(&event.title)
    .bind(&event.slug)
    .bind(&event.description_md)
    .bind(event.start_at)
    .bind(event.end_at)
    .bind(&event.timezone)
    .bind(event.location_type.as_str())
    .bind(event.location_text.as_deref())
    .bind(event.location_url.as_deref())
    .bind(event.capacity)
    .bind(event.visibility.as_str())
    .bind(event.status.as_str())
    .bind(event.cover_image_id)
    .fetch_one(pool)
    .await
}

pub async fn find_event_for_host(
    pool: &PgPool,
    event_id: Uuid,
    host_id: Uuid,
) -> Result<Option<Event>, sqlx::Error> {
    sqlx::query_as::<_, Event>(EVENT_SELECT_BY_ID_AND_HOST)
        .bind(event_id)
        .bind(host_id)
        .fetch_optional(pool)
        .await
}

pub async fn slug_exists(pool: &PgPool, slug: &str) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>("SELECT EXISTS (SELECT 1 FROM events WHERE slug = $1)")
        .bind(slug)
        .fetch_one(pool)
        .await
}

pub async fn slug_exists_for_other_event(
    pool: &PgPool,
    slug: &str,
    event_id: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM events WHERE slug = $1 AND id <> $2)",
    )
    .bind(slug)
    .bind(event_id)
    .fetch_one(pool)
    .await
}

pub async fn update_event_for_host(
    pool: &PgPool,
    event_id: Uuid,
    host_id: Uuid,
    changes: &EventChanges,
) -> Result<Option<Event>, sqlx::Error> {
    sqlx::query_as::<_, Event>(
        r#"
        UPDATE events
        SET
            title = $3,
            slug = $4,
            description_md = $5,
            start_at = $6,
            end_at = $7,
            timezone = $8,
            location_type = $9,
            location_text = $10,
            location_url = $11,
            capacity = $12,
            visibility = $13,
            status = $14,
            cover_image_id = $15,
            cancelled_at = CASE
                WHEN $14 = 'cancelled' THEN COALESCE(cancelled_at, NOW())
                ELSE NULL
            END,
            updated_at = NOW()
        WHERE id = $1
          AND host_id = $2
        RETURNING
            id,
            host_id,
            title,
            slug,
            description_md,
            start_at,
            end_at,
            timezone,
            location_type,
            location_text,
            location_url,
            capacity,
            visibility,
            status,
            cover_image_id,
            created_at,
            updated_at,
            cancelled_at
        "#,
    )
    .bind(event_id)
    .bind(host_id)
    .bind(&changes.title)
    .bind(&changes.slug)
    .bind(&changes.description_md)
    .bind(changes.start_at)
    .bind(changes.end_at)
    .bind(&changes.timezone)
    .bind(changes.location_type.as_str())
    .bind(changes.location_text.as_deref())
    .bind(changes.location_url.as_deref())
    .bind(changes.capacity)
    .bind(changes.visibility.as_str())
    .bind(changes.status.as_str())
    .bind(changes.cover_image_id)
    .fetch_optional(pool)
    .await
}

pub async fn cancel_event_for_host(
    pool: &PgPool,
    event_id: Uuid,
    host_id: Uuid,
) -> Result<Option<Event>, sqlx::Error> {
    sqlx::query_as::<_, Event>(
        r#"
        UPDATE events
        SET
            status = 'cancelled',
            cancelled_at = COALESCE(cancelled_at, NOW()),
            updated_at = NOW()
        WHERE id = $1
          AND host_id = $2
        RETURNING
            id,
            host_id,
            title,
            slug,
            description_md,
            start_at,
            end_at,
            timezone,
            location_type,
            location_text,
            location_url,
            capacity,
            visibility,
            status,
            cover_image_id,
            created_at,
            updated_at,
            cancelled_at
        "#,
    )
    .bind(event_id)
    .bind(host_id)
    .fetch_optional(pool)
    .await
}

pub async fn insert_event_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    event: &NewEvent,
) -> Result<Event, sqlx::Error> {
    sqlx::query_as::<_, Event>(
        r#"
        INSERT INTO events (
            host_id,
            title,
            slug,
            description_md,
            start_at,
            end_at,
            timezone,
            location_type,
            location_text,
            location_url,
            capacity,
            visibility,
            status,
            cover_image_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        RETURNING
            id,
            host_id,
            title,
            slug,
            description_md,
            start_at,
            end_at,
            timezone,
            location_type,
            location_text,
            location_url,
            capacity,
            visibility,
            status,
            cover_image_id,
            created_at,
            updated_at,
            cancelled_at
        "#,
    )
    .bind(event.host_id)
    .bind(&event.title)
    .bind(&event.slug)
    .bind(&event.description_md)
    .bind(event.start_at)
    .bind(event.end_at)
    .bind(&event.timezone)
    .bind(event.location_type.as_str())
    .bind(event.location_text.as_deref())
    .bind(event.location_url.as_deref())
    .bind(event.capacity)
    .bind(event.visibility.as_str())
    .bind(event.status.as_str())
    .bind(event.cover_image_id)
    .fetch_one(&mut **transaction)
    .await
}

pub async fn duplicate_event_images_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    source_event_id: Uuid,
    duplicate_event_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO event_images (
            event_id,
            object_key,
            variant,
            width,
            height,
            bytes
        )
        SELECT
            $2,
            object_key,
            variant,
            width,
            height,
            bytes
        FROM event_images
        WHERE event_id = $1
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(source_event_id)
    .bind(duplicate_event_id)
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

const EVENT_SELECT_BY_ID_AND_HOST: &str = r#"
    SELECT
        id,
        host_id,
        title,
        slug,
        description_md,
        start_at,
        end_at,
        timezone,
        location_type,
        location_text,
        location_url,
        capacity,
        visibility,
        status,
        cover_image_id,
        created_at,
        updated_at,
        cancelled_at
    FROM events
    WHERE id = $1
      AND host_id = $2
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum EventLocationType {
    InPerson,
    Virtual,
    Hybrid,
}

impl EventLocationType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InPerson => "in_person",
            Self::Virtual => "virtual",
            Self::Hybrid => "hybrid",
        }
    }
}

impl fmt::Display for EventLocationType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for EventLocationType {
    type Err = EventEnumParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "in_person" => Ok(Self::InPerson),
            "virtual" => Ok(Self::Virtual),
            "hybrid" => Ok(Self::Hybrid),
            _ => Err(EventEnumParseError::new("location_type", value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum EventVisibility {
    Draft,
    Public,
    Unlisted,
    Private,
}

impl EventVisibility {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Public => "public",
            Self::Unlisted => "unlisted",
            Self::Private => "private",
        }
    }
}

impl fmt::Display for EventVisibility {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for EventVisibility {
    type Err = EventEnumParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "draft" => Ok(Self::Draft),
            "public" => Ok(Self::Public),
            "unlisted" => Ok(Self::Unlisted),
            "private" => Ok(Self::Private),
            _ => Err(EventEnumParseError::new("visibility", value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum EventStatus {
    Draft,
    Published,
    Cancelled,
    Completed,
}

impl EventStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Published => "published",
            Self::Cancelled => "cancelled",
            Self::Completed => "completed",
        }
    }
}

impl fmt::Display for EventStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for EventStatus {
    type Err = EventEnumParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "draft" => Ok(Self::Draft),
            "published" => Ok(Self::Published),
            "cancelled" => Ok(Self::Cancelled),
            "completed" => Ok(Self::Completed),
            _ => Err(EventEnumParseError::new("status", value)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EventEnumParseError {
    kind: &'static str,
    value: String,
}

impl EventEnumParseError {
    fn new(kind: &'static str, value: &str) -> Self {
        Self { kind, value: value.to_owned() }
    }
}

impl fmt::Display for EventEnumParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unsupported event {} value: {}", self.kind, self.value)
    }
}

impl std::error::Error for EventEnumParseError {}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{EventLocationType, EventStatus, EventVisibility};

    #[test]
    fn event_location_type_round_trip_strings() {
        assert_eq!(EventLocationType::InPerson.as_str(), "in_person");
        assert_eq!(EventLocationType::Virtual.as_str(), "virtual");
        assert_eq!(EventLocationType::Hybrid.as_str(), "hybrid");
        assert_eq!(
            EventLocationType::from_str("hybrid").expect("hybrid should parse"),
            EventLocationType::Hybrid,
        );
    }

    #[test]
    fn event_visibility_round_trip_strings() {
        assert_eq!(EventVisibility::Draft.as_str(), "draft");
        assert_eq!(EventVisibility::Public.as_str(), "public");
        assert_eq!(EventVisibility::Unlisted.as_str(), "unlisted");
        assert_eq!(EventVisibility::Private.as_str(), "private");
        assert_eq!(
            EventVisibility::from_str("public").expect("public should parse"),
            EventVisibility::Public,
        );
    }

    #[test]
    fn event_status_round_trip_strings() {
        assert_eq!(EventStatus::Draft.as_str(), "draft");
        assert_eq!(EventStatus::Published.as_str(), "published");
        assert_eq!(EventStatus::Cancelled.as_str(), "cancelled");
        assert_eq!(EventStatus::Completed.as_str(), "completed");
        assert_eq!(
            EventStatus::from_str("published").expect("published should parse"),
            EventStatus::Published,
        );
    }
}
