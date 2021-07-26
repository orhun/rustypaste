use crate::auth;
use crate::config::Config;
use crate::file;
use crate::header::ContentDisposition;
use actix_files::NamedFile;
use actix_multipart::Multipart;
use actix_web::{error, get, post, web, Error, HttpRequest, HttpResponse, Responder};
use byte_unit::Byte;
use futures_util::stream::StreamExt;
use std::convert::TryFrom;
use std::env;

/// Shows the landing page.
#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::Found()
        .header("Location", env!("CARGO_PKG_HOMEPAGE"))
        .finish()
}

/// Serves a file from the upload directory.
#[get("/{file}")]
async fn serve(
    request: HttpRequest,
    path: web::Path<String>,
    config: web::Data<Config>,
) -> Result<HttpResponse, Error> {
    let path = config.server.upload_path.join(&*path);
    let file = NamedFile::open(&path)?
        .disable_content_disposition()
        .prefer_utf8(true);
    let response = file.into_response(&request)?;
    Ok(response)
}

/// Handles file upload by processing `multipart/form-data`.
#[post("/")]
async fn upload(
    request: HttpRequest,
    mut payload: Multipart,
    config: web::Data<Config>,
) -> Result<HttpResponse, Error> {
    let connection = request.connection_info();
    let host = connection.remote_addr().unwrap_or("unknown host");
    auth::check(host, request.headers(), env::var("AUTH_TOKEN").ok())?;
    let mut urls: Vec<String> = Vec::new();
    while let Some(item) = payload.next().await {
        let mut field = item?;
        let content = ContentDisposition::try_from(field.content_disposition())?;
        if content.has_form_field("file") {
            let mut bytes = Vec::<u8>::new();
            while let Some(chunk) = field.next().await {
                bytes.append(&mut chunk?.to_vec());
            }
            let bytes_unit = Byte::from_bytes(bytes.len() as u128).get_appropriate_unit(false);
            if bytes.len() as u128 > config.server.max_content_length.get_bytes() {
                log::warn!("upload rejected for {} ({})", host, bytes_unit);
                return Err(error::ErrorPayloadTooLarge("upload limit exceeded"));
            }
            let file_name = &file::save(content.get_file_name()?, &bytes, &config)?;
            log::info!("{} ({}) is uploaded from {}", file_name, bytes_unit, host);
            urls.push(format!(
                "{}://{}/{}\n",
                connection.scheme(),
                connection.host(),
                file_name
            ));
        } else {
            log::warn!("{} sent an invalid form field", host);
            return Err(error::ErrorBadRequest("invalid form field"));
        }
    }
    Ok(HttpResponse::Ok().body(urls.join("")))
}

/// Configures the server routes.
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(index)
        .service(serve)
        .service(upload)
        .route("", web::head().to(HttpResponse::MethodNotAllowed));
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{http, test, App};

    #[actix_rt::test]
    async fn test_index() {
        let mut app = test::init_service(App::new().service(index)).await;
        let req = test::TestRequest::with_header("content-type", "text/plain").to_request();
        let resp = test::call_service(&mut app, req).await;
        assert!(resp.status().is_redirection());
        assert_eq!(http::StatusCode::FOUND, resp.status());
    }

    #[actix_rt::test]
    async fn test_serve() {
        let mut app = test::init_service(App::new().service(serve)).await;
        let req = test::TestRequest::default().to_request();
        let resp = test::call_service(&mut app, req).await;
        assert_eq!(http::StatusCode::NOT_FOUND, resp.status());
    }

    // TODO: add test for upload
}
