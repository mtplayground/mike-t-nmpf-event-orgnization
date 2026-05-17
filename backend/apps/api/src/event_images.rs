#![allow(dead_code)]

use std::{fmt, str::FromStr};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EventImage {
    pub id: Uuid,
    pub event_id: Uuid,
    pub object_key: String,
    pub variant: EventImageVariant,
    pub width: i32,
    pub height: i32,
    pub bytes: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum EventImageVariant {
    Hero,
    Thumbnail,
}

impl EventImageVariant {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hero => "hero",
            Self::Thumbnail => "thumbnail",
        }
    }
}

impl fmt::Display for EventImageVariant {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for EventImageVariant {
    type Err = EventImageVariantParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "hero" => Ok(Self::Hero),
            "thumbnail" => Ok(Self::Thumbnail),
            _ => Err(EventImageVariantParseError(value.to_owned())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EventImageVariantParseError(String);

impl fmt::Display for EventImageVariantParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unsupported event image variant: {}", self.0)
    }
}

impl std::error::Error for EventImageVariantParseError {}

pub async fn upsert_event_image(
    pool: &PgPool,
    event_id: Uuid,
    object_key: &str,
    variant: EventImageVariant,
    width: i32,
    height: i32,
    bytes: i64,
) -> Result<EventImage, sqlx::Error> {
    sqlx::query_as::<_, EventImage>(
        r#"
        INSERT INTO event_images (
            event_id,
            object_key,
            variant,
            width,
            height,
            bytes
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (event_id, variant)
        DO UPDATE SET
            object_key = EXCLUDED.object_key,
            width = EXCLUDED.width,
            height = EXCLUDED.height,
            bytes = EXCLUDED.bytes
        RETURNING
            id,
            event_id,
            object_key,
            variant,
            width,
            height,
            bytes,
            created_at
        "#,
    )
    .bind(event_id)
    .bind(object_key)
    .bind(variant.as_str())
    .bind(width)
    .bind(height)
    .bind(bytes)
    .fetch_one(pool)
    .await
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::EventImageVariant;

    #[test]
    fn event_image_variant_round_trip_strings() {
        assert_eq!(EventImageVariant::Hero.as_str(), "hero");
        assert_eq!(EventImageVariant::Thumbnail.as_str(), "thumbnail");
        assert_eq!(
            EventImageVariant::from_str("hero").expect("hero should parse"),
            EventImageVariant::Hero,
        );
        assert_eq!(
            EventImageVariant::from_str("thumbnail")
                .expect("thumbnail should parse"),
            EventImageVariant::Thumbnail,
        );
    }

    #[test]
    fn event_image_variant_rejects_unknown_values() {
        let error = EventImageVariant::from_str("banner")
            .expect_err("unknown variant should fail");

        assert!(error
            .to_string()
            .contains("unsupported event image variant"));
    }
}
