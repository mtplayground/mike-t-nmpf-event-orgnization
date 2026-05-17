use std::ops::{Deref, DerefMut};

use axum::{
    Json,
    extract::{FromRequest, FromRequestParts, Request},
    http::request::Parts,
};
use serde::de::DeserializeOwned;
use validator::Validate;

use crate::error::AppError;
use crate::users::AuthUserContext;

pub struct ValidatedJson<T>(pub T);

impl<T> Deref for ValidatedJson<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for ValidatedJson<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<S, T> FromRequest<S> for ValidatedJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Validate,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state).await?;
        value.validate()?;
        Ok(Self(value))
    }
}

#[derive(Debug, Clone)]
pub struct CurrentUser(pub AuthUserContext);

impl Deref for CurrentUser {
    type Target = AuthUserContext;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<S> FromRequestParts<S> for CurrentUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthUserContext>()
            .cloned()
            .map(Self)
            .ok_or_else(|| AppError::unauthorized("authentication is required"))
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        extract::{FromRequest, FromRequestParts, Request},
        http::request::Parts,
    };
    use chrono::Utc;
    use serde::Deserialize;
    use uuid::Uuid;
    use validator::Validate;

    use super::{CurrentUser, ValidatedJson};
    use crate::{error::AppError, users::AuthUserContext};

    #[derive(Debug, Deserialize, Validate)]
    struct ValidationPayload {
        #[validate(length(min = 3, message = "name must be at least 3 characters"))]
        name: String,
    }

    #[tokio::test]
    async fn validated_json_accepts_valid_payload() {
        let request = json_request(r#"{"name":"valid-name"}"#);

        let payload = ValidatedJson::<ValidationPayload>::from_request(request, &())
            .await
            .expect("valid payload should pass validation");

        assert_eq!(payload.name, "valid-name");
    }

    #[tokio::test]
    async fn validated_json_rejects_invalid_payload() {
        let request = json_request(r#"{"name":"no"}"#);

        let error = match ValidatedJson::<ValidationPayload>::from_request(request, &()).await {
            Ok(_) => panic!("short value should fail validation"),
            Err(error) => error,
        };

        match error {
            AppError::Validation(errors) => {
                let field_errors = errors.field_errors();
                let messages = field_errors
                    .get("name")
                    .expect("name field should have a validation error");

                assert_eq!(messages.len(), 1);
            }
            other => panic!("expected validation error, received {other}"),
        }
    }

    #[tokio::test]
    async fn validated_json_rejects_malformed_json() {
        let request = json_request(r#"{"name":}"#);

        let error = match ValidatedJson::<ValidationPayload>::from_request(request, &()).await {
            Ok(_) => panic!("malformed json should fail"),
            Err(error) => error,
        };

        assert!(matches!(error, AppError::Json(_)));
    }

    #[tokio::test]
    async fn current_user_reads_user_from_extensions() {
        let user = AuthUserContext {
            id: Uuid::new_v4(),
            email: "user@example.com".to_owned(),
            display_name: "Example User".to_owned(),
            email_verified_at: Some(Utc::now()),
        };
        let mut parts = request_parts();
        parts.extensions.insert(user.clone());

        let current_user = CurrentUser::from_request_parts(&mut parts, &())
            .await
            .expect("current user should extract from request extensions");

        assert_eq!(current_user.id, user.id);
        assert_eq!(current_user.email, user.email);
    }

    #[tokio::test]
    async fn current_user_requires_auth_extension() {
        let mut parts = request_parts();

        let error = CurrentUser::from_request_parts(&mut parts, &())
            .await
            .expect_err("missing extension should be unauthorized");

        assert!(matches!(error, AppError::Unauthorized { .. }));
    }

    fn json_request(body: &str) -> Request {
        Request::builder()
            .header("content-type", "application/json")
            .body(Body::from(body.to_owned()))
            .expect("request should build")
    }

    fn request_parts() -> Parts {
        let request = Request::builder()
            .body(Body::empty())
            .expect("request should build");

        let (parts, _) = request.into_parts();
        parts
    }
}
