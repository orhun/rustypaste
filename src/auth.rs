use actix_web::http::header::{HeaderMap, AUTHORIZATION};
use actix_web::{error, Error};

/// Checks the authorization header for the specified token.
///
/// `Authorization: (type) <token>`
pub fn check(host: &str, headers: &HeaderMap, tokens: Option<Vec<String>>) -> Result<(), Error> {
    if let Some(tokens) = tokens {
        let auth_header = headers
            .get(AUTHORIZATION)
            .map(|v| v.to_str().unwrap_or_default())
            .map(|v| v.split_whitespace().last().unwrap_or_default());
        if !tokens.iter().any(|v| v == auth_header.unwrap_or_default()) {
            #[cfg(debug_assertions)]
            tracing::warn!(
                "authorization failure for {host} (token: {})",
                auth_header.unwrap_or("none"),
            );
            #[cfg(not(debug_assertions))]
            tracing::warn!("authorization failure for {host}");
            return Err(error::ErrorUnauthorized("unauthorized\n"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http::header::HeaderValue;

    #[test]
    fn test_check_auth() -> Result<(), Error> {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("basic test_token"));
        assert!(check("", &headers, Some(vec!["test_token".to_string()])).is_ok());
        assert!(check("", &headers, Some(vec!["invalid_token".to_string()])).is_err());
        assert!(check(
            "",
            &headers,
            Some(vec!["invalid1".to_string(), "test_token".to_string()])
        )
        .is_ok());
        assert!(check(
            "",
            &headers,
            Some(vec!["invalid1".to_string(), "invalid2".to_string()])
        )
        .is_err());
        assert!(check("", &headers, None).is_ok());
        assert!(check("", &HeaderMap::new(), None).is_ok());
        assert!(check("", &HeaderMap::new(), Some(vec!["token".to_string()])).is_err());
        Ok(())
    }
}
