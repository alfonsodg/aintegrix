#![allow(dead_code)]

use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;

/// Bearer token authentication middleware.
/// Validates the Authorization header against the configured API key.
pub async fn auth_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Skip auth for health endpoints
    let path = request.uri().path();
    if path == "/health" || path == "/readiness" || path == "/metrics" {
        return Ok(next.run(request).await);
    }

    let token = std::env::var("AINTEGRIX_API_KEY").unwrap_or_default();
    if token.is_empty() {
        // No auth configured, allow all
        return Ok(next.run(request).await);
    }

    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            let provided = &header[7..];
            if provided == token {
                Ok(next.run(request).await)
            } else {
                Err(StatusCode::UNAUTHORIZED)
            }
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

/// Validate a bearer token against expected value (testable without env mutation)
pub fn validate_token(provided: Option<&str>, expected: &str) -> bool {
    match provided {
        Some(header) if header.starts_with("Bearer ") => &header[7..] == expected,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_token() {
        assert!(validate_token(Some("Bearer mysecret"), "mysecret"));
    }

    #[test]
    fn test_invalid_token() {
        assert!(!validate_token(Some("Bearer wrong"), "mysecret"));
    }

    #[test]
    fn test_missing_bearer_prefix() {
        assert!(!validate_token(Some("mysecret"), "mysecret"));
    }

    #[test]
    fn test_no_header() {
        assert!(!validate_token(None, "mysecret"));
    }

    #[test]
    fn test_empty_token_value() {
        // "Bearer " with nothing after = empty string provided, matches empty expected
        // This case is handled at middleware level (empty expected = skip auth)
        assert!(validate_token(Some("Bearer "), ""));
    }
}
