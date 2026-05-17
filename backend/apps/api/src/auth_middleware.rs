use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::Response,
};

use crate::{
    app::SharedAppState,
    error::AppError,
    users::{self, AuthUserContext},
};

pub async fn require_current_user(
    State(state): State<SharedAppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = bearer_token(&request)?;
    let claims = state
        .jwt_service
        .verify_access_token(token)
        .map_err(|error| AppError::unauthorized(error.to_string()))?;
    let user = users::find_auth_user_by_id(&state.db_pool, claims.subject)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::unauthorized("authenticated user was not found"))?;

    request.extensions_mut().insert::<AuthUserContext>(user);

    Ok(next.run(request).await)
}

fn bearer_token(request: &Request) -> Result<&str, AppError> {
    let header_value = request
        .headers()
        .get(header::AUTHORIZATION)
        .ok_or_else(|| AppError::unauthorized("missing Authorization header"))?;
    let header_str = header_value
        .to_str()
        .map_err(|_| AppError::unauthorized("invalid Authorization header"))?;
    let (scheme, token) = header_str
        .split_once(' ')
        .ok_or_else(|| AppError::unauthorized("invalid Authorization header"))?;

    if !scheme.eq_ignore_ascii_case("bearer") || token.trim().is_empty() {
        return Err(AppError::unauthorized("invalid bearer token"));
    }

    Ok(token.trim())
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::{Request, header}};

    use super::bearer_token;

    #[test]
    fn bearer_token_accepts_valid_header() {
        let request = Request::builder()
            .header(header::AUTHORIZATION, "Bearer token-value")
            .body(Body::empty())
            .expect("request should build");

        let token = bearer_token(&request).expect("token should parse");

        assert_eq!(token, "token-value");
    }

    #[test]
    fn bearer_token_rejects_missing_header() {
        let request = Request::builder()
            .body(Body::empty())
            .expect("request should build");

        let error = bearer_token(&request).expect_err("missing header should fail");

        assert!(error.to_string().contains("missing Authorization header"));
    }
}
