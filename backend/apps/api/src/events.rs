#![allow(dead_code)]

use std::{fmt, str::FromStr};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
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
        Self {
            kind,
            value: value.to_owned(),
        }
    }
}

impl fmt::Display for EventEnumParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unsupported event {} value: {}",
            self.kind,
            self.value
        )
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
            EventStatus::from_str("published")
                .expect("published should parse"),
            EventStatus::Published,
        );
    }
}
