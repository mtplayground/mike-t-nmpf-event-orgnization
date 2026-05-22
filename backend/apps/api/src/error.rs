use std::{
    collections::{BTreeMap, HashMap},
    fmt,
};

use axum::{
    Json,
    extract::rejection::JsonRejection,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::Value;
use validator::ValidationErrors;

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub data: T,
}

impl<T> ApiResponse<T> {
    pub fn new(data: T) -> Self {
        Self { data }
    }
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: ErrorEnvelope,
}

#[derive(Debug, Serialize)]
pub struct ErrorEnvelope {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<BTreeMap<String, Vec<String>>>,
}

#[derive(Debug)]
pub enum AppError {
    BadRequest { message: String },
    Conflict { message: String },
    NotFound { message: String },
    Unauthorized { message: String },
    Internal { message: String },
    Validation(ValidationErrors),
    Json(JsonRejection),
    Database(sqlx::Error),
}

impl AppError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest { message: message.into() }
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict { message: message.into() }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound { message: message.into() }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::Unauthorized { message: message.into() }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal { message: message.into() }
    }

    fn status_code(&self) -> StatusCode {
        match self {
            Self::BadRequest { .. } | Self::Validation(_) | Self::Json(_) => {
                StatusCode::BAD_REQUEST
            }
            Self::Conflict { .. } => StatusCode::CONFLICT,
            Self::NotFound { .. } => StatusCode::NOT_FOUND,
            Self::Unauthorized { .. } => StatusCode::UNAUTHORIZED,
            Self::Internal { .. } | Self::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn code(&self) -> &'static str {
        match self {
            Self::BadRequest { .. } => "bad_request",
            Self::Conflict { .. } => "conflict",
            Self::NotFound { .. } => "not_found",
            Self::Unauthorized { .. } => "unauthorized",
            Self::Internal { .. } => "internal_error",
            Self::Validation(_) => "validation_failed",
            Self::Json(_) => "invalid_json",
            Self::Database(_) => "database_error",
        }
    }

    fn message(&self) -> String {
        match self {
            Self::BadRequest { message } => message.clone(),
            Self::Conflict { message } => message.clone(),
            Self::NotFound { message } => message.clone(),
            Self::Unauthorized { message } => message.clone(),
            Self::Internal { message } => message.clone(),
            Self::Validation(_) => "One or more request fields failed validation.".to_owned(),
            Self::Json(rejection) => rejection.body_text(),
            Self::Database(_) => {
                "A database error occurred while processing the request.".to_owned()
            }
        }
    }

    fn fields(&self) -> Option<BTreeMap<String, Vec<String>>> {
        match self {
            Self::Validation(errors) => Some(flatten_validation_errors(errors)),
            _ => None,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let response = ErrorResponse {
            error: ErrorEnvelope {
                code: self.code(),
                message: self.message(),
                fields: self.fields(),
            },
        };

        (status, Json(response)).into_response()
    }
}

impl From<JsonRejection> for AppError {
    fn from(value: JsonRejection) -> Self {
        Self::Json(value)
    }
}

impl From<ValidationErrors> for AppError {
    fn from(value: ValidationErrors) -> Self {
        Self::Validation(value)
    }
}

impl From<sqlx::Error> for AppError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadRequest { message } => formatter.write_str(message),
            Self::Conflict { message } => formatter.write_str(message),
            Self::NotFound { message } => formatter.write_str(message),
            Self::Unauthorized { message } => formatter.write_str(message),
            Self::Internal { message } => formatter.write_str(message),
            Self::Validation(_) => formatter.write_str("request validation failed"),
            Self::Json(rejection) => write!(formatter, "invalid request body: {rejection}"),
            Self::Database(error) => write!(formatter, "database error: {error}"),
        }
    }
}

impl std::error::Error for AppError {}

fn flatten_validation_errors(errors: &ValidationErrors) -> BTreeMap<String, Vec<String>> {
    let mut field_errors = BTreeMap::new();

    for (field, kind) in errors.field_errors() {
        let messages = kind
            .iter()
            .map(|error| {
                error.message.as_ref().map(ToString::to_string).unwrap_or_else(|| {
                    default_validation_message(error.code.as_ref(), &error.params)
                })
            })
            .collect();

        field_errors.insert(field.to_string(), messages);
    }

    field_errors
}

fn default_validation_message(
    code: &str,
    params: &HashMap<std::borrow::Cow<'static, str>, Value>,
) -> String {
    match code {
        "length" => "has an invalid length".to_owned(),
        "email" => "must be a valid email address".to_owned(),
        "range" => "is outside the allowed range".to_owned(),
        "required" => "is required".to_owned(),
        "must_match" => "does not match the required format".to_owned(),
        _ => {
            if let Some(value) = params.get("value") {
                format!("is invalid: {value}")
            } else {
                "is invalid".to_owned()
            }
        }
    }
}
