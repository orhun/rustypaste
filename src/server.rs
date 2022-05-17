use crate::auth;
use crate::config::Config;
use crate::file::Directory;
use crate::header::{self, ContentDisposition};
use crate::mime;
use crate::paste::{Paste, PasteType};
use crate::util;
use crate::AUTH_TOKEN_ENV;
use actix_files::NamedFile;
use actix_multipart::Multipart;
use actix_web::{error, get, post, web, Error, HttpRequest, HttpResponse, Responder};
use awc::Client;
use byte_unit::Byte;
use futures_util::stream::StreamExt;
use std::convert::TryFrom;
use std::env;
use std::fs;
use std::sync::RwLock;

/// Shows the landing page.
#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::Found()
        .append_header(("Location", env!("CARGO_PKG_HOMEPAGE")))
        .finish()
}

/// Serves a file from the upload directory.
#[get("/{file}")]
async fn serve(
    request: HttpRequest,
    file: web::Path<String>,
    config: web::Data<RwLock<Config>>,
) -> Result<HttpResponse, Error> {
    let config = config
        .read()
        .map_err(|_| error::ErrorInternalServerError("cannot acquire config"))?;
    let path = config.server.upload_path.join(&*file);
    let mut path = util::glob_match_file(path)?;
    let mut paste_type = PasteType::File;
    if !path.exists() || path.is_dir() {
        for type_ in &[PasteType::Url, PasteType::Oneshot] {
            let alt_path = type_.get_path(&config.server.upload_path).join(&*file);
            let alt_path = util::glob_match_file(alt_path)?;
            if alt_path.exists()
                || path.file_name().and_then(|v| v.to_str()) == Some(&type_.get_dir())
            {
                path = alt_path;
                paste_type = *type_;
                break;
            }
        }
    }
    if !path.is_file() || !path.exists() {
        return Err(error::ErrorNotFound("file is not found or expired :("));
    }
    match paste_type {
        PasteType::File | PasteType::RemoteFile | PasteType::Oneshot => {
            let response = NamedFile::open(&path)?
                .disable_content_disposition()
                .set_content_type(
                    mime::get_mime_type(&config.paste.mime_override, file.to_string())
                        .map_err(error::ErrorInternalServerError)?,
                )
                .prefer_utf8(true)
                .into_response(&request);
            if paste_type.is_oneshot() {
                fs::rename(
                    &path,
                    path.with_file_name(format!(
                        "{}.{}",
                        file,
                        util::get_system_time()?.as_millis()
                    )),
                )?;
            }
            Ok(response)
        }
        PasteType::Url => Ok(HttpResponse::Found()
            .append_header(("Location", fs::read_to_string(&path)?))
            .finish()),
    }
}

/// Handles file upload by processing `multipart/form-data`.
#[post("/")]
async fn upload(
    request: HttpRequest,
    mut payload: Multipart,
    client: web::Data<Client>,
    config: web::Data<RwLock<Config>>,
) -> Result<HttpResponse, Error> {
    let connection = request.connection_info().clone();
    let host = connection.peer_addr().unwrap_or("unknown host");
    auth::check(
        host,
        request.headers(),
        env::var(AUTH_TOKEN_ENV).ok().or(config
            .read()
            .map_err(|_| error::ErrorInternalServerError("cannot acquire config"))?
            .server
            .auth_token
            .as_ref()
            .cloned()),
    )?;
    let expiry_date = header::parse_expiry_date(request.headers())?;
    let mut urls: Vec<String> = Vec::new();
    while let Some(item) = payload.next().await {
        let mut field = item?;
        let content = ContentDisposition::from(field.content_disposition().clone());
        if let Ok(paste_type) = PasteType::try_from(&content) {
            let mut bytes = Vec::<u8>::new();
            while let Some(chunk) = field.next().await {
                bytes.append(&mut chunk?.to_vec());
                let config = config
                    .read()
                    .map_err(|_| error::ErrorInternalServerError("cannot acquire config"))?;
                if bytes.len() as u128 > config.server.max_content_length.get_bytes() {
                    log::warn!("Upload rejected for {}", host);
                    return Err(error::ErrorPayloadTooLarge("upload limit exceeded"));
                }
            }
            if bytes.is_empty() {
                log::warn!("{} sent zero bytes", host);
                return Err(error::ErrorBadRequest("invalid file size"));
            }
            if paste_type != PasteType::Oneshot
                && paste_type != PasteType::RemoteFile
                && expiry_date.is_none()
                && !config
                    .read()
                    .map_err(|_| error::ErrorInternalServerError("cannot acquire config"))?
                    .paste
                    .duplicate_files
                    .unwrap_or(true)
            {
                let bytes_checksum = util::sha256_digest(&*bytes)?;
                let config = config
                    .read()
                    .map_err(|_| error::ErrorInternalServerError("cannot acquire config"))?;
                if let Some(file) = Directory::try_from(config.server.upload_path.as_path())?
                    .get_file(bytes_checksum)
                {
                    urls.push(format!(
                        "{}://{}/{}\n",
                        connection.scheme(),
                        connection.host(),
                        file.path
                            .file_name()
                            .map(|v| v.to_string_lossy())
                            .unwrap_or_default()
                    ));
                    continue;
                }
            }
            let mut paste = Paste {
                data: bytes.to_vec(),
                type_: paste_type,
            };
            let file_name = match paste.type_ {
                PasteType::File | PasteType::Oneshot => {
                    let config = config
                        .read()
                        .map_err(|_| error::ErrorInternalServerError("cannot acquire config"))?;
                    paste.store_file(content.get_file_name()?, expiry_date, &config)?
                }
                PasteType::RemoteFile => {
                    paste
                        .store_remote_file(expiry_date, &client, &config)
                        .await?
                }
                PasteType::Url => {
                    let config = config
                        .read()
                        .map_err(|_| error::ErrorInternalServerError("cannot acquire config"))?;
                    paste.store_url(expiry_date, &config)?
                }
            };
            log::info!(
                "{} ({}) is uploaded from {}",
                file_name,
                Byte::from_bytes(paste.data.len() as u128).get_appropriate_unit(false),
                host
            );
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
    use actix_web::body::BodySize;
    use actix_web::body::MessageBody;
    use actix_web::dev::ServiceResponse;
    use actix_web::error::Error;
    use actix_web::http::header;
    use actix_web::test::{self, TestRequest};
    use actix_web::web::Data;
    use actix_web::{http, App};
    use awc::ClientBuilder;
    use byte_unit::Byte;
    use glob::glob;
    use std::str;
    use std::thread;
    use std::time::Duration;

    #[actix_web::test]
    async fn test_index() {
        let app = test::init_service(App::new().service(index)).await;
        let request = TestRequest::default()
            .insert_header(("content-type", "text/plain"))
            .to_request();
        let resp = test::call_service(&app, request).await;
        assert!(resp.status().is_redirection());
        assert_eq!(http::StatusCode::FOUND, resp.status());
    }

    fn get_multipart_request(data: &str, name: &str, file_name: &str) -> TestRequest {
        let multipart_data = format!(
            "\r\n\
             --multipart_bound\r\n\
             Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\n\
             Content-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\n\r\n\
             {}\r\n\
             --multipart_bound--\r\n",
            name,
            file_name,
            data.bytes().len(),
            data,
        );
        TestRequest::post()
            .insert_header((
                header::CONTENT_TYPE,
                header::HeaderValue::from_static("multipart/mixed; boundary=\"multipart_bound\""),
            ))
            .set_payload(multipart_data)
    }

    async fn assert_body(response: ServiceResponse, expected: &str) -> Result<(), Error> {
        let body = response.into_body();
        if let BodySize::Sized(size) = body.size() {
            assert_eq!(size, expected.as_bytes().len() as u64);
            let body_bytes = actix_web::body::to_bytes(body).await?;
            let body_text = str::from_utf8(&body_bytes)?;
            assert_eq!(expected, body_text);
            Ok(())
        } else {
            Err(error::ErrorInternalServerError("unexpected body type"))
        }
    }

    #[actix_web::test]
    async fn test_upload_file() -> Result<(), Error> {
        let mut config = Config::default();
        config.server.upload_path = env::current_dir()?;
        config.server.max_content_length = Byte::from_bytes(100);

        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(config)))
                .app_data(Data::new(Client::default()))
                .service(serve)
                .service(upload),
        )
        .await;

        let file_name = "test_file.txt";
        let timestamp = util::get_system_time()?.as_secs().to_string();
        let response = test::call_service(
            &app,
            get_multipart_request(&timestamp, "file", file_name).to_request(),
        )
        .await;
        assert_eq!(http::StatusCode::OK, response.status());
        assert_body(response, &format!("http://localhost:8080/{}\n", file_name)).await?;

        let serve_request = TestRequest::get()
            .uri(&format!("/{}", file_name))
            .to_request();
        let response = test::call_service(&app, serve_request).await;
        assert_eq!(http::StatusCode::OK, response.status());
        assert_body(response, &timestamp).await?;

        fs::remove_file(file_name)?;
        let serve_request = TestRequest::get()
            .uri(&format!("/{}", file_name))
            .to_request();
        let response = test::call_service(&app, serve_request).await;
        assert_eq!(http::StatusCode::NOT_FOUND, response.status());

        Ok(())
    }

    #[actix_web::test]
    async fn test_upload_expiring_file() -> Result<(), Error> {
        let mut config = Config::default();
        config.server.upload_path = env::current_dir()?;
        config.server.max_content_length = Byte::from_bytes(100);

        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(config)))
                .app_data(Data::new(Client::default()))
                .service(serve)
                .service(upload),
        )
        .await;

        let file_name = "test_file.txt";
        let timestamp = util::get_system_time()?.as_secs().to_string();
        let response = test::call_service(
            &app,
            get_multipart_request(&timestamp, "file", file_name)
                .insert_header((
                    header::HeaderName::from_static("expire"),
                    header::HeaderValue::from_static("10ms"),
                ))
                .to_request(),
        )
        .await;
        assert_eq!(http::StatusCode::OK, response.status());
        assert_body(response, &format!("http://localhost:8080/{}\n", file_name)).await?;

        let serve_request = TestRequest::get()
            .uri(&format!("/{}", file_name))
            .to_request();
        let response = test::call_service(&app, serve_request).await;
        assert_eq!(http::StatusCode::OK, response.status());
        assert_body(response, &timestamp).await?;

        thread::sleep(Duration::from_millis(20));

        let serve_request = TestRequest::get()
            .uri(&format!("/{}", file_name))
            .to_request();
        let response = test::call_service(&app, serve_request).await;
        assert_eq!(http::StatusCode::NOT_FOUND, response.status());

        if let Some(glob_path) = glob(&format!("{}.[0-9]*", file_name))
            .map_err(error::ErrorInternalServerError)?
            .next()
        {
            fs::remove_file(glob_path.map_err(error::ErrorInternalServerError)?)?;
        }

        Ok(())
    }

    #[actix_web::test]
    async fn test_upload_remote_file() -> Result<(), Error> {
        let mut config = Config::default();
        config.server.upload_path = env::current_dir()?;
        config.server.max_content_length = Byte::from_bytes(30000);

        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(config)))
                .app_data(Data::new(
                    ClientBuilder::new()
                        .timeout(Duration::from_secs(30))
                        .finish(),
                ))
                .service(serve)
                .service(upload),
        )
        .await;

        let file_name = "Example.jpg";
        let response = test::call_service(
            &app,
            get_multipart_request(
                "https://upload.wikimedia.org/wikipedia/en/a/a9/Example.jpg",
                "remote",
                file_name,
            )
            .to_request(),
        )
        .await;
        assert_eq!(http::StatusCode::OK, response.status());
        assert_body(response, &format!("http://localhost:8080/{}\n", file_name)).await?;

        let serve_request = TestRequest::get()
            .uri(&format!("/{}", file_name))
            .to_request();
        let response = test::call_service(&app, serve_request).await;
        assert_eq!(http::StatusCode::OK, response.status());

        let body = response.into_body();
        let body_bytes = actix_web::body::to_bytes(body).await?;
        assert_eq!(
            "8c712905b799905357b8202d0cb7a244cefeeccf7aa5eb79896645ac50158ffa",
            util::sha256_digest(&*body_bytes)?
        );

        fs::remove_file(file_name)?;

        let serve_request = TestRequest::get()
            .uri(&format!("/{}", file_name))
            .to_request();
        let response = test::call_service(&app, serve_request).await;
        assert_eq!(http::StatusCode::NOT_FOUND, response.status());

        Ok(())
    }

    #[actix_web::test]
    async fn test_upload_url() -> Result<(), Error> {
        let mut config = Config::default();
        config.server.upload_path = env::current_dir()?;
        config.server.max_content_length = Byte::from_bytes(100);

        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(config.clone())))
                .app_data(Data::new(Client::default()))
                .service(serve)
                .service(upload),
        )
        .await;

        let url_upload_path = PasteType::Url.get_path(&config.server.upload_path);
        fs::create_dir_all(&url_upload_path)?;

        let response = test::call_service(
            &app,
            get_multipart_request(env!("CARGO_PKG_HOMEPAGE"), "url", "").to_request(),
        )
        .await;
        assert_eq!(http::StatusCode::OK, response.status());
        assert_body(response, "http://localhost:8080/url\n").await?;

        let serve_request = TestRequest::get().uri("/url").to_request();
        let response = test::call_service(&app, serve_request).await;
        assert_eq!(http::StatusCode::FOUND, response.status());

        fs::remove_file(url_upload_path.join("url"))?;
        fs::remove_dir(url_upload_path)?;

        let serve_request = TestRequest::get().uri("/url").to_request();
        let response = test::call_service(&app, serve_request).await;
        assert_eq!(http::StatusCode::NOT_FOUND, response.status());

        Ok(())
    }

    #[actix_web::test]
    async fn test_upload_oneshot() -> Result<(), Error> {
        let mut config = Config::default();
        config.server.upload_path = env::current_dir()?;
        config.server.max_content_length = Byte::from_bytes(100);

        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(config.clone())))
                .app_data(Data::new(Client::default()))
                .service(serve)
                .service(upload),
        )
        .await;

        let oneshot_upload_path = PasteType::Oneshot.get_path(&config.server.upload_path);
        fs::create_dir_all(&oneshot_upload_path)?;

        let file_name = "oneshot.txt";
        let timestamp = util::get_system_time()?.as_secs().to_string();
        let response = test::call_service(
            &app,
            get_multipart_request(&timestamp, "oneshot", file_name).to_request(),
        )
        .await;
        assert_eq!(http::StatusCode::OK, response.status());
        assert_body(response, &format!("http://localhost:8080/{}\n", file_name)).await?;

        let serve_request = TestRequest::get()
            .uri(&format!("/{}", file_name))
            .to_request();
        let response = test::call_service(&app, serve_request).await;
        assert_eq!(http::StatusCode::OK, response.status());
        assert_body(response, &timestamp).await?;

        let serve_request = TestRequest::get()
            .uri(&format!("/{}", file_name))
            .to_request();
        let response = test::call_service(&app, serve_request).await;
        assert_eq!(http::StatusCode::NOT_FOUND, response.status());

        if let Some(glob_path) = glob(
            &oneshot_upload_path
                .join(format!("{}.[0-9]*", file_name))
                .to_string_lossy(),
        )
        .map_err(error::ErrorInternalServerError)?
        .next()
        {
            fs::remove_file(glob_path.map_err(error::ErrorInternalServerError)?)?;
        }
        fs::remove_dir(oneshot_upload_path)?;

        Ok(())
    }
}
