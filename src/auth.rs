use actix_web::http::header::{HeaderMap, AUTHORIZATION};
use actix_web::{error, Error};

/// Checks the authorization header for the specified token.
///
/// `Authorization: (type) <token>`
pub fn check(host: &str, headers: &HeaderMap, token: Option<String>) -> Result<(), Error> {
    if let Some(token) = token {
        if !token.is_empty() {
            let auth_header = headers
                .get(AUTHORIZATION)
                .map(|v| v.to_str().unwrap_or_default())
                .map(|v| v.split_whitespace().last().unwrap_or_default());
            if auth_header.unwrap_or_default() != token {
                log::warn!(
                    "authorization failure for {} (header: {})",
                    host,
                    auth_header.unwrap_or("none"),
                );
                return Err(error::ErrorUnauthorized("unauthorized"));
            }
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
        assert!(check("", &headers, Some(String::from("test_token"))).is_ok());
        assert!(check("", &headers, Some(String::from("invalid_token"))).is_err());
        assert!(check("", &headers, None).is_ok());
        assert!(check("", &HeaderMap::new(), None).is_ok());
        assert!(check("", &HeaderMap::new(), Some(String::from("token"))).is_err());
        Ok(())
    }
}
