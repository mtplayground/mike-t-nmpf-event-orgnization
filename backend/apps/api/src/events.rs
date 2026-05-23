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

pub fn duplicate_event_template(existing: &Event, title: String, slug: String) -> NewEvent {
    NewEvent {
        host_id: existing.host_id,
        title,
        slug,
        description_md: existing.description_md.clone(),
        start_at: existing.start_at,
        end_at: existing.end_at,
        timezone: existing.timezone.clone(),
        location_type: existing.location_type,
        location_text: existing.location_text.clone(),
        location_url: existing.location_url.clone(),
        capacity: existing.capacity,
        visibility: EventVisibility::Draft,
        status: EventStatus::Draft,
        cover_image_id: None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostEventListFilter {
    Draft,
    Upcoming,
    Past,
}

#[derive(Debug, Clone, FromRow)]
pub struct HostEventListRow {
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
    pub attendee_count: i64,
}

#[derive(Debug, Clone)]
pub struct PublicEventCursor {
    pub start_at: DateTime<Utc>,
    pub id: Uuid,
}

#[derive(Debug, Clone, FromRow)]
pub struct PublicEventListRow {
    pub id: Uuid,
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
    pub thumbnail_object_key: Option<String>,
    pub thumbnail_width: Option<i32>,
    pub thumbnail_height: Option<i32>,
    pub thumbnail_bytes: Option<i64>,
}

#[derive(Debug, Clone, FromRow)]
pub struct PublicEventDetailRow {
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
    pub host_display_name: String,
    pub host_avatar_object_key: Option<String>,
    pub attendee_count: i64,
    pub thumbnail_object_key: Option<String>,
    pub thumbnail_width: Option<i32>,
    pub thumbnail_height: Option<i32>,
    pub thumbnail_bytes: Option<i64>,
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

pub async fn list_public_events(
    pool: &PgPool,
    query: Option<&str>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    cursor: Option<PublicEventCursor>,
    limit: i64,
) -> Result<Vec<PublicEventListRow>, sqlx::Error> {
    let search_query = query.map(str::trim).filter(|value| !value.is_empty());
    let cursor_start_at = cursor.as_ref().map(|cursor| cursor.start_at);
    let cursor_id = cursor.as_ref().map(|cursor| cursor.id);

    sqlx::query_as::<_, PublicEventListRow>(PUBLIC_EVENT_LIST_SQL)
        .bind(search_query)
        .bind(from)
        .bind(to)
        .bind(cursor_start_at)
        .bind(cursor_id)
        .bind(limit)
        .fetch_all(pool)
        .await
}

pub async fn find_public_event_by_slug(
    pool: &PgPool,
    slug: &str,
) -> Result<Option<PublicEventDetailRow>, sqlx::Error> {
    sqlx::query_as::<_, PublicEventDetailRow>(PUBLIC_EVENT_DETAIL_SQL)
        .bind(slug)
        .fetch_optional(pool)
        .await
}

pub async fn list_events_for_host(
    pool: &PgPool,
    host_id: Uuid,
    filter: HostEventListFilter,
    limit: i64,
    offset: i64,
) -> Result<Vec<HostEventListRow>, sqlx::Error> {
    let query = format!(
        r#"
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
            cancelled_at,
            0::BIGINT AS attendee_count
        FROM events
        WHERE host_id = $1
          AND {}
        ORDER BY start_at ASC, created_at ASC, id ASC
        LIMIT $2
        OFFSET $3
        "#,
        host_event_list_filter_sql(filter),
    );

    sqlx::query_as::<_, HostEventListRow>(&query)
        .bind(host_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
}

pub async fn count_events_for_host(
    pool: &PgPool,
    host_id: Uuid,
    filter: HostEventListFilter,
) -> Result<i64, sqlx::Error> {
    let query = format!(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM events
        WHERE host_id = $1
          AND {}
        "#,
        host_event_list_filter_sql(filter),
    );

    sqlx::query_scalar::<_, i64>(&query).bind(host_id).fetch_one(pool).await
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
    sqlx::query_as::<_, Event>(UPDATE_EVENT_FOR_HOST_SQL)
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
    sqlx::query_as::<_, Event>(CANCEL_EVENT_FOR_HOST_SQL)
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

const PUBLIC_EVENT_LIST_SQL: &str = r#"
    SELECT
        events.id,
        events.title,
        events.slug,
        events.description_md,
        events.start_at,
        events.end_at,
        events.timezone,
        events.location_type,
        events.location_text,
        events.location_url,
        events.capacity,
        thumbnail.object_key AS thumbnail_object_key,
        thumbnail.width AS thumbnail_width,
        thumbnail.height AS thumbnail_height,
        thumbnail.bytes AS thumbnail_bytes
    FROM events
    LEFT JOIN event_images AS thumbnail
      ON thumbnail.event_id = events.id
     AND thumbnail.variant = 'thumbnail'
    WHERE events.visibility = 'public'
      AND events.status = 'published'
      AND events.cancelled_at IS NULL
      AND events.end_at >= NOW()
      AND (
          $1::TEXT IS NULL
          OR events.title ILIKE '%' || $1 || '%'
          OR events.description_md ILIKE '%' || $1 || '%'
          OR events.location_text ILIKE '%' || $1 || '%'
      )
      AND ($2::TIMESTAMPTZ IS NULL OR events.start_at >= $2)
      AND ($3::TIMESTAMPTZ IS NULL OR events.start_at <= $3)
      AND (
          $4::TIMESTAMPTZ IS NULL
          OR events.start_at > $4
          OR (events.start_at = $4 AND events.id > $5::UUID)
      )
    ORDER BY events.start_at ASC, events.id ASC
    LIMIT $6
"#;

const PUBLIC_EVENT_DETAIL_SQL: &str = r#"
    SELECT
        events.id,
        events.host_id,
        events.title,
        events.slug,
        events.description_md,
        events.start_at,
        events.end_at,
        events.timezone,
        events.location_type,
        events.location_text,
        events.location_url,
        events.capacity,
        events.visibility,
        events.status,
        events.cover_image_id,
        events.created_at,
        events.updated_at,
        events.cancelled_at,
        users.display_name AS host_display_name,
        users.avatar_object_key AS host_avatar_object_key,
        0::BIGINT AS attendee_count,
        thumbnail.object_key AS thumbnail_object_key,
        thumbnail.width AS thumbnail_width,
        thumbnail.height AS thumbnail_height,
        thumbnail.bytes AS thumbnail_bytes
    FROM events
    INNER JOIN users
      ON users.id = events.host_id
    LEFT JOIN event_images AS thumbnail
      ON thumbnail.event_id = events.id
     AND thumbnail.variant = 'thumbnail'
    WHERE events.slug = $1
      AND events.visibility = 'public'
      AND events.status = 'published'
      AND events.cancelled_at IS NULL
"#;

const UPDATE_EVENT_FOR_HOST_SQL: &str = r#"
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
"#;

const CANCEL_EVENT_FOR_HOST_SQL: &str = r#"
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
"#;

fn host_event_list_filter_sql(filter: HostEventListFilter) -> &'static str {
    match filter {
        HostEventListFilter::Draft => "status = 'draft'",
        HostEventListFilter::Upcoming => "status = 'published' AND end_at >= NOW()",
        HostEventListFilter::Past => {
            "(status = 'completed' OR (status = 'published' AND end_at < NOW()))"
        }
    }
}

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

    use chrono::{Duration, Utc};
    use uuid::Uuid;

    use super::{
        CANCEL_EVENT_FOR_HOST_SQL, EVENT_SELECT_BY_ID_AND_HOST, Event, EventLocationType,
        EventStatus, EventVisibility, HostEventListFilter, PUBLIC_EVENT_DETAIL_SQL,
        PUBLIC_EVENT_LIST_SQL, UPDATE_EVENT_FOR_HOST_SQL, duplicate_event_template,
        host_event_list_filter_sql,
    };

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

    #[test]
    fn duplicate_event_template_resets_publish_state_and_cover() {
        let existing = event_fixture(
            Uuid::new_v4(),
            "Original",
            EventVisibility::Public,
            EventStatus::Published,
        );

        let duplicate = duplicate_event_template(
            &existing,
            "Original Copy".to_owned(),
            "original-copy".to_owned(),
        );

        assert_eq!(duplicate.host_id, existing.host_id);
        assert_eq!(duplicate.title, "Original Copy");
        assert_eq!(duplicate.slug, "original-copy");
        assert_eq!(duplicate.description_md, existing.description_md);
        assert_eq!(duplicate.start_at, existing.start_at);
        assert_eq!(duplicate.end_at, existing.end_at);
        assert_eq!(duplicate.timezone, existing.timezone);
        assert_eq!(duplicate.location_type, existing.location_type);
        assert_eq!(duplicate.location_text, existing.location_text);
        assert_eq!(duplicate.location_url, existing.location_url);
        assert_eq!(duplicate.capacity, existing.capacity);
        assert_eq!(duplicate.visibility, EventVisibility::Draft);
        assert_eq!(duplicate.status, EventStatus::Draft);
        assert_eq!(duplicate.cover_image_id, None);
    }

    #[test]
    fn host_event_queries_are_scoped_by_event_and_host() {
        for query in
            [EVENT_SELECT_BY_ID_AND_HOST, UPDATE_EVENT_FOR_HOST_SQL, CANCEL_EVENT_FOR_HOST_SQL]
        {
            assert!(query.contains("WHERE id = $1"));
            assert!(query.contains("AND host_id = $2"));
        }
    }

    #[test]
    fn status_transition_queries_manage_cancelled_at() {
        assert!(CANCEL_EVENT_FOR_HOST_SQL.contains("status = 'cancelled'"));
        assert!(CANCEL_EVENT_FOR_HOST_SQL.contains("cancelled_at = COALESCE(cancelled_at, NOW())"));
        assert!(UPDATE_EVENT_FOR_HOST_SQL.contains("WHEN $14 = 'cancelled'"));
        assert!(UPDATE_EVENT_FOR_HOST_SQL.contains("ELSE NULL"));
    }

    #[test]
    fn host_event_list_filters_match_status_rules() {
        assert_eq!(host_event_list_filter_sql(HostEventListFilter::Draft), "status = 'draft'");
        assert_eq!(
            host_event_list_filter_sql(HostEventListFilter::Upcoming),
            "status = 'published' AND end_at >= NOW()"
        );
        assert_eq!(
            host_event_list_filter_sql(HostEventListFilter::Past),
            "(status = 'completed' OR (status = 'published' AND end_at < NOW()))"
        );
    }

    #[test]
    fn public_event_list_query_filters_to_public_upcoming_events_with_thumbnail_join() {
        assert!(PUBLIC_EVENT_LIST_SQL.contains("events.visibility = 'public'"));
        assert!(PUBLIC_EVENT_LIST_SQL.contains("events.status = 'published'"));
        assert!(PUBLIC_EVENT_LIST_SQL.contains("events.cancelled_at IS NULL"));
        assert!(PUBLIC_EVENT_LIST_SQL.contains("events.end_at >= NOW()"));
        assert!(PUBLIC_EVENT_LIST_SQL.contains("thumbnail.variant = 'thumbnail'"));
        assert!(PUBLIC_EVENT_LIST_SQL.contains("ORDER BY events.start_at ASC, events.id ASC"));
    }

    #[test]
    fn public_event_detail_query_filters_by_slug_and_includes_host_context() {
        assert!(PUBLIC_EVENT_DETAIL_SQL.contains("events.slug = $1"));
        assert!(PUBLIC_EVENT_DETAIL_SQL.contains("events.visibility = 'public'"));
        assert!(PUBLIC_EVENT_DETAIL_SQL.contains("events.status = 'published'"));
        assert!(PUBLIC_EVENT_DETAIL_SQL.contains("events.cancelled_at IS NULL"));
        assert!(PUBLIC_EVENT_DETAIL_SQL.contains("INNER JOIN users"));
        assert!(PUBLIC_EVENT_DETAIL_SQL.contains("0::BIGINT AS attendee_count"));
        assert!(PUBLIC_EVENT_DETAIL_SQL.contains("thumbnail.variant = 'thumbnail'"));
    }

    fn event_fixture(
        host_id: Uuid,
        title: &str,
        visibility: EventVisibility,
        status: EventStatus,
    ) -> Event {
        let now = Utc::now();

        Event {
            id: Uuid::new_v4(),
            host_id,
            title: title.to_owned(),
            slug: "original".to_owned(),
            description_md: "Event description".to_owned(),
            start_at: now + Duration::days(2),
            end_at: now + Duration::days(2) + Duration::hours(2),
            timezone: "UTC".to_owned(),
            location_type: EventLocationType::Hybrid,
            location_text: Some("Main Hall".to_owned()),
            location_url: Some("https://example.com/room".to_owned()),
            capacity: Some(50),
            visibility,
            status,
            cover_image_id: None,
            created_at: now,
            updated_at: now,
            cancelled_at: None,
        }
    }
}
