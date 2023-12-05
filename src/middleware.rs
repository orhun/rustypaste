use actix_web::dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::header::CONTENT_LENGTH;
use actix_web::http::StatusCode;
use actix_web::{body::EitherBody, Error};
use actix_web::{HttpMessage, HttpResponseBuilder};
use byte_unit::Byte;
use futures_util::{Future, TryStreamExt};
use std::{
    future::{ready, Ready},
    pin::Pin,
    rc::Rc,
};

/// Content length limiter middleware.
#[derive(Debug)]
pub struct ContentLengthLimiter {
    // Maximum amount of bytes to allow.
    max_bytes: Byte,
}

impl ContentLengthLimiter {
    /// Constructs a new instance.
    pub fn new(max_bytes: Byte) -> Self {
        Self { max_bytes }
    }
}

impl<S, B> Transform<S, ServiceRequest> for ContentLengthLimiter
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = ContentLengthLimiterMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;
    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(ContentLengthLimiterMiddleware {
            service: Rc::new(service),
            max_bytes: self.max_bytes,
        }))
    }
}

/// Content length limiter middleware implementation.
#[derive(Debug)]
pub struct ContentLengthLimiterMiddleware<S> {
    service: Rc<S>,
    max_bytes: Byte,
}

impl<S, B> Service<ServiceRequest> for ContentLengthLimiterMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;
    forward_ready!(service);
    fn call(&self, mut request: ServiceRequest) -> Self::Future {
        let service = Rc::clone(&self.service);
        if let Some(content_length) = request
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<Byte>().ok())
        {
            if content_length > self.max_bytes {
                warn!(
                    "Upload rejected due to exceeded limit. ({:-#} > {:-#})",
                    content_length, self.max_bytes
                );
                return Box::pin(async move {
                    // drain the body due to https://github.com/actix/actix-web/issues/2695
                    let mut payload = request.take_payload();
                    while let Ok(Some(_)) = payload.try_next().await {}
                    Ok(request.into_response(
                        HttpResponseBuilder::new(StatusCode::PAYLOAD_TOO_LARGE)
                            .body("upload limit exceeded")
                            .map_into_right_body(),
                    ))
                });
            }
        }
        Box::pin(async move {
            service
                .call(request)
                .await
                .map(ServiceResponse::map_into_left_body)
        })
    }
}
