use crate::config::{Config, TokenType};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::http::header::AUTHORIZATION;
use actix_web::http::Method;
use actix_web::middleware::ErrorHandlerResponse;
use actix_web::{error, web, Error};
use std::collections::HashSet;
use std::sync::RwLock;

/// Extracts the tokens from the authorization header by token type.
///
/// `Authorization: (type) <token>`
pub(crate) async fn extract_tokens(req: &ServiceRequest) -> Result<HashSet<TokenType>, Error> {
    let config = req
        .app_data::<web::Data<RwLock<Config>>>()
        .map(|cfg| cfg.read())
        .and_then(Result::ok)
        .ok_or_else(|| error::ErrorInternalServerError("cannot acquire config"))?;

    let mut user_tokens = HashSet::with_capacity(2);

    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .map(|v| v.to_str().unwrap_or_default())
        .map(|v| v.split_whitespace().last().unwrap_or_default());

    for token_type in [TokenType::Auth, TokenType::Delete] {
        let maybe_tokens = config.get_tokens(token_type);
        if let Some(configured_tokens) = maybe_tokens {
            if configured_tokens.contains(auth_header.unwrap_or_default()) {
                user_tokens.insert(token_type);
            }
        } else if token_type == TokenType::Auth {
            // not configured `auth_tokens` means that the user is allowed to access the endpoints
            user_tokens.insert(token_type);
        } else if token_type == TokenType::Delete && req.method() == Method::DELETE {
            // explicitly disable `DELETE` methods if no `delete_tokens` are set
            warn!("delete endpoint is not served because there are no delete_tokens set");
            Err(error::ErrorNotFound(""))?;
        }
    }

    Ok(user_tokens)
}

/// Returns `HttpResponse` with unauthorized (`401`) error and `unauthorized\n` as body.
pub(crate) fn unauthorized_error() -> actix_web::HttpResponse {
    error::ErrorUnauthorized("unauthorized\n").into()
}

/// Log all unauthorized requests.
pub(crate) fn handle_unauthorized_error<B>(
    res: ServiceResponse<B>,
) -> actix_web::Result<ErrorHandlerResponse<B>> {
    let connection = res.request().connection_info().clone();
    let host = connection.realip_remote_addr().unwrap_or("unknown host");

    #[cfg(debug_assertions)]
    {
        let auth_header = res
            .request()
            .headers()
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("none");

        warn!("authorization failure for {host} (token: {auth_header})",);
    }
    #[cfg(not(debug_assertions))]
    warn!("authorization failure for {host}");

    Ok(ErrorHandlerResponse::Response(res.map_into_left_body()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http::header::HeaderValue;
    use actix_web::test::TestRequest;
    use actix_web::web::Data;
    use actix_web::HttpResponse;
    use awc::http::StatusCode;

    #[actix_web::test]
    async fn test_extract_tokens() -> Result<(), Error> {
        let mut config = Config::default();

        // request without configured auth-tokens
        let request = TestRequest::default()
            .app_data(Data::new(RwLock::new(config.clone())))
            .insert_header((AUTHORIZATION, HeaderValue::from_static("basic test_token")))
            .to_srv_request();
        let tokens = extract_tokens(&request).await?;
        assert_eq!(HashSet::from([TokenType::Auth]), tokens);

        // request with configured auth-tokens
        config.server.auth_tokens = Some(["test_token".to_string()].into());
        let request = TestRequest::default()
            .app_data(Data::new(RwLock::new(config.clone())))
            .insert_header((AUTHORIZATION, HeaderValue::from_static("basic test_token")))
            .to_srv_request();
        let tokens = extract_tokens(&request).await?;
        assert_eq!(HashSet::from([TokenType::Auth]), tokens);

        // request with configured auth-tokens but wrong token in request
        config.server.auth_tokens = Some(["test_token".to_string()].into());
        let request = TestRequest::default()
            .app_data(Data::new(RwLock::new(config.clone())))
            .insert_header((
                AUTHORIZATION,
                HeaderValue::from_static("basic invalid_token"),
            ))
            .to_srv_request();
        let tokens = extract_tokens(&request).await?;
        assert_eq!(HashSet::new(), tokens);

        // DELETE request without configured delete-tokens
        let request = TestRequest::default()
            .method(Method::DELETE)
            .app_data(Data::new(RwLock::new(config.clone())))
            .insert_header((AUTHORIZATION, HeaderValue::from_static("basic test_token")))
            .to_srv_request();
        let res = extract_tokens(&request).await;
        assert!(res.is_err());
        assert_eq!(
            Some(StatusCode::NOT_FOUND),
            res.err()
                .as_ref()
                .map(Error::error_response)
                .as_ref()
                .map(HttpResponse::status)
        );

        // DELETE request with configured delete-tokens
        config.server.delete_tokens = Some(["delete_token".to_string()].into());
        let request = TestRequest::default()
            .method(Method::DELETE)
            .app_data(Data::new(RwLock::new(config.clone())))
            .insert_header((
                AUTHORIZATION,
                HeaderValue::from_static("basic delete_token"),
            ))
            .to_srv_request();
        let tokens = extract_tokens(&request).await?;
        assert_eq!(HashSet::from([TokenType::Delete]), tokens);

        // DELETE request with configured delete-tokens but wrong token in request
        let request = TestRequest::default()
            .method(Method::DELETE)
            .app_data(Data::new(RwLock::new(config.clone())))
            .insert_header((
                AUTHORIZATION,
                HeaderValue::from_static("basic invalid_token"),
            ))
            .to_srv_request();
        let tokens = extract_tokens(&request).await?;
        assert_eq!(HashSet::new(), tokens);

        Ok(())
    }
}
