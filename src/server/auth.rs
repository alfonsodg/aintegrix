use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use subtle::ConstantTimeEq;

/// Bearer token authentication middleware.
/// FAIL-CLOSED: rejects all requests if AINTEGRIX_API_KEY is not set.
pub async fn auth_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = request.uri().path();
    if path == "/health" || path == "/readiness" {
        return Ok(next.run(request).await);
    }

    let token = std::env::var("AINTEGRIX_API_KEY").unwrap_or_default();
    if token.is_empty() {
        tracing::error!("AINTEGRIX_API_KEY not set — rejecting request (fail-closed)");
        return Err(StatusCode::UNAUTHORIZED);
    }

    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            let provided = &header.as_bytes()[7..];
            let expected = token.as_bytes();
            if provided.len() == expected.len() && provided.ct_eq(expected).into() {
                Ok(next.run(request).await)
            } else {
                Err(StatusCode::UNAUTHORIZED)
            }
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ct_check(provided: &str, expected: &str) -> bool {
        let p = provided.as_bytes();
        let e = expected.as_bytes();
        p.len() == e.len() && p.ct_eq(e).into()
    }

    #[test]
    fn test_valid_token() {
        assert!(ct_check("mysecret", "mysecret"));
    }

    #[test]
    fn test_invalid_token() {
        assert!(!ct_check("wrong", "mysecret"));
    }

    #[test]
    fn test_different_length() {
        assert!(!ct_check("short", "longersecret"));
    }
}
