use crate::auth;
use crate::config::Config;
use crate::header::{self, ContentDisposition};
use crate::mime;
use crate::paste::{Paste, PasteType};
use crate::util;
use actix_files::NamedFile;
use actix_multipart::Multipart;
use actix_web::{error, get, post, web, Error, HttpRequest, HttpResponse, Responder};
use byte_unit::Byte;
use futures_util::stream::StreamExt;
use std::convert::TryFrom;
use std::env;
use std::fs;

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
    file: web::Path<String>,
    config: web::Data<Config>,
) -> Result<HttpResponse, Error> {
    let path = config.server.upload_path.join(&*file);
    let mut path = util::glob_match_file(path)?;
    let mut paste_type = PasteType::File;
    if !path.exists() || path.is_dir() {
        for type_ in &[PasteType::Url, PasteType::Oneshot] {
            let alt_path = type_.get_path(&config.server.upload_path).join(&*file);
            let alt_path = util::glob_match_file(alt_path)?;
            if alt_path.exists()
                || path.file_name().map(|v| v.to_str()).flatten() == Some(&type_.get_dir())
            {
                path = alt_path;
                paste_type = *type_;
                break;
            }
        }
    }
    match paste_type {
        PasteType::File | PasteType::Oneshot => {
            let response = NamedFile::open(&path)
                .map_err(|_| error::ErrorNotFound("file is not found or expired :("))?
                .disable_content_disposition()
                .set_content_type(
                    mime::get_mime_type(&config.paste.mime_override, file.to_string())
                        .map_err(error::ErrorInternalServerError)?,
                )
                .prefer_utf8(true)
                .into_response(&request)?;
            if paste_type.is_oneshot() {
                fs::rename(
                    path,
                    PasteType::Trash
                        .get_path(&config.server.upload_path)
                        .join(&*file),
                )?;
            }
            Ok(response)
        }
        PasteType::Url => Ok(HttpResponse::Found()
            .header("Location", fs::read_to_string(&path)?)
            .finish()),
        PasteType::Trash => unreachable!(),
    }
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
    let expiry_date = header::parse_expiry_date(request.headers())?;
    let mut urls: Vec<String> = Vec::new();
    while let Some(item) = payload.next().await {
        let mut field = item?;
        let content = ContentDisposition::try_from(field.content_disposition())?;
        if let Ok(paste_type) = PasteType::try_from(&content) {
            let mut bytes = Vec::<u8>::new();
            while let Some(chunk) = field.next().await {
                bytes.append(&mut chunk?.to_vec());
                if bytes.len() as u128 > config.server.max_content_length.get_bytes() {
                    log::warn!("upload rejected for {}", host);
                    return Err(error::ErrorPayloadTooLarge("upload limit exceeded"));
                }
            }
            if bytes.is_empty() {
                log::warn!("{} sent zero bytes", host);
                return Err(error::ErrorBadRequest("invalid file size"));
            }
            let bytes_unit = Byte::from_bytes(bytes.len() as u128).get_appropriate_unit(false);
            let paste = Paste {
                data: bytes.to_vec(),
                type_: paste_type,
            };
            let file_name = match paste_type {
                PasteType::File | PasteType::Oneshot => {
                    paste.store_file(content.get_file_name()?, expiry_date, &config)?
                }
                PasteType::Url => paste.store_url(expiry_date, &config)?,
                PasteType::Trash => unreachable!(),
            };
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
