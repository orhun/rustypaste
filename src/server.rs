use crate::auth::{extract_tokens, handle_unauthorized_error, unauthorized_error};
use crate::config::{Config, LandingPageConfig, TokenType};
use crate::file::Directory;
use crate::header::{self, ContentDisposition};
use crate::mime as mime_util;
use crate::paste::{Paste, PasteType};
use crate::util::{self, safe_path_join};
use actix_files::NamedFile;
use actix_multipart::Multipart;
use actix_web::http::StatusCode;
use actix_web::middleware::ErrorHandlers;
use actix_web::{delete, error, get, post, web, Error, HttpRequest, HttpResponse};
use actix_web_grants::GrantsMiddleware;
use awc::Client;
use byte_unit::{Byte, UnitType};
use futures_util::stream::StreamExt;
use mime::TEXT_PLAIN_UTF_8;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::env;
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs;
use tokio::sync::RwLock;
use uts2ts;

/// Shows the landing page.
#[get("/")]
#[allow(deprecated)]
async fn index(config: web::Data<RwLock<Config>>) -> Result<HttpResponse, Error> {
    let mut config = config.read().await.clone();
    let redirect = HttpResponse::Found()
        .append_header(("Location", env!("CARGO_PKG_HOMEPAGE")))
        .finish();
    if config.server.landing_page.is_some() {
        if config.landing_page.is_none() {
            config.landing_page = Some(LandingPageConfig::default());
        }
        if let Some(ref mut landing_page) = config.landing_page {
            landing_page.text = config.server.landing_page;
        }
    }
    if config.server.landing_page_content_type.is_some() {
        if config.landing_page.is_none() {
            config.landing_page = Some(LandingPageConfig::default());
        }
        if let Some(ref mut landing_page) = config.landing_page {
            landing_page.content_type = config.server.landing_page_content_type;
        }
    }
    if let Some(mut landing_page) = config.landing_page {
        if let Some(file) = landing_page.file {
            landing_page.text = fs::read_to_string(file).await.ok();
        }
        match landing_page.text {
            Some(page) => Ok(HttpResponse::Ok()
                .content_type(
                    landing_page
                        .content_type
                        .unwrap_or(TEXT_PLAIN_UTF_8.to_string()),
                )
                .body(page)),
            None => Ok(redirect),
        }
    } else {
        Ok(redirect)
    }
}

/// File serving options (i.e. query parameters).
#[derive(Debug, Deserialize)]
struct ServeOptions {
    /// If set to `true`, change the MIME type to `application/octet-stream` and force downloading
    /// the file.
    download: bool,
}

/// Serves a file from the upload directory.
#[get("/{file}")]
async fn serve(
    request: HttpRequest,
    file: web::Path<String>,
    options: Option<web::Query<ServeOptions>>,
    config: web::Data<RwLock<Config>>,
) -> Result<HttpResponse, Error> {
    let config = config.read().await;
    let mut path =
        util::glob_match_file(safe_path_join(&config.server.upload_path, &*file)?).await?;
    let mut paste_type = PasteType::File;
    if !path.exists() || path.is_dir() {
        for type_ in &[PasteType::Url, PasteType::Oneshot, PasteType::OneshotUrl] {
            let alt_path = safe_path_join(type_.get_path(&config.server.upload_path)?, &*file)?;
            let alt_path = util::glob_match_file(alt_path).await?;
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
        return Err(error::ErrorNotFound("file is not found or expired :(\n"));
    }
    match paste_type {
        PasteType::File | PasteType::RemoteFile | PasteType::Oneshot => {
            let mime_type = if options.map(|v| v.download).unwrap_or(false) {
                mime::APPLICATION_OCTET_STREAM
            } else {
                mime_util::get_mime_type(&config.paste.mime_override, file.to_string())
                    .map_err(error::ErrorInternalServerError)?
            };
            let response = NamedFile::open(&path)?
                .disable_content_disposition()
                .set_content_type(mime_type)
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
                )
                .await?;
            }
            Ok(response)
        }
        PasteType::Url => Ok(HttpResponse::Found()
            .append_header(("Location", fs::read_to_string(&path).await?))
            .finish()),
        PasteType::OneshotUrl => {
            let resp = HttpResponse::Found()
                .append_header(("Location", fs::read_to_string(&path).await?))
                .finish();
            fs::rename(
                &path,
                path.with_file_name(format!("{}.{}", file, util::get_system_time()?.as_millis())),
            )
            .await?;
            Ok(resp)
        }
    }
}

/// Remove a file from the upload directory.
#[delete("/{file}")]
#[actix_web_grants::protect("TokenType::Delete", ty = TokenType, error = unauthorized_error)]
async fn delete(
    file: web::Path<String>,
    config: web::Data<RwLock<Config>>,
) -> Result<HttpResponse, Error> {
    let config = config.read().await;
    let path = util::glob_match_file(safe_path_join(&config.server.upload_path, &*file)?).await?;
    if !path.is_file() || !path.exists() {
        return Err(error::ErrorNotFound("file is not found or expired :(\n"));
    }
    match fs::remove_file(path).await {
        Ok(_) => info!("deleted file: {:?}", file.to_string()),
        Err(e) => {
            error!("cannot delete file: {}", e);
            return Err(error::ErrorInternalServerError("cannot delete file"));
        }
    }
    Ok(HttpResponse::Ok().body(String::from("file deleted\n")))
}

/// Expose version endpoint
#[get("/version")]
#[actix_web_grants::protect("TokenType::Auth", ty = TokenType, error = unauthorized_error)]
async fn version(config: web::Data<RwLock<Config>>) -> Result<HttpResponse, Error> {
    let config = config.read().await;
    if !config.server.expose_version.unwrap_or(false) {
        warn!("server is not configured to expose version endpoint");
        Err(error::ErrorNotFound(""))?;
    }

    let version = env!("CARGO_PKG_VERSION");
    Ok(HttpResponse::Ok().body(version.to_owned() + "\n"))
}

/// Handles file upload by processing `multipart/form-data`.
#[post("/")]
#[actix_web_grants::protect("TokenType::Auth", ty = TokenType, error = unauthorized_error)]
async fn upload(
    request: HttpRequest,
    mut payload: Multipart,
    client: web::Data<Client>,
    config: web::Data<RwLock<Config>>,
) -> Result<HttpResponse, Error> {
    let connection = request.connection_info().clone();
    let host = connection.realip_remote_addr().unwrap_or("unknown host");
    let config = config.read().await;
    let server_url = match config.server.url.clone() {
        Some(v) => v,
        None => {
            format!("{}://{}", connection.scheme(), connection.host(),)
        }
    };
    let time = util::get_system_time()?;
    let mut expiry_date = header::parse_expiry_date(request.headers(), time)?;
    if expiry_date.is_none() {
        expiry_date = config
            .paste
            .default_expiry
            .and_then(|v| time.checked_add(v).map(|t| t.as_millis()));
    }
    let mut urls: Vec<String> = Vec::new();
    while let Some(item) = payload.next().await {
        let header_filename = header::parse_header_filename(request.headers())?;
        let mut field = item?;
        let content = ContentDisposition::from(field.content_disposition().clone());
        if let Ok(paste_type) = PasteType::try_from(&content) {
            let mut bytes = Vec::<u8>::new();
            while let Some(chunk) = field.next().await {
                bytes.append(&mut chunk?.to_vec());
            }
            if bytes.is_empty() {
                warn!("{} sent zero bytes", host);
                return Err(error::ErrorBadRequest("invalid file size"));
            }
            if paste_type != PasteType::Oneshot
                && paste_type != PasteType::RemoteFile
                && paste_type != PasteType::OneshotUrl
                && expiry_date.is_none()
                && !config.paste.duplicate_files.unwrap_or(true)
            {
                let bytes_checksum = util::sha256_digest(&*bytes)?;
                if let Some(file) = Directory::try_from(config.server.upload_path.as_path())
                    .map_err(error::ErrorInternalServerError)?
                    .get_file(bytes_checksum)
                {
                    urls.push(format!(
                        "{}/{}\n",
                        server_url,
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
            let mut file_name = match paste.type_ {
                PasteType::File | PasteType::Oneshot => {
                    paste
                        .store_file(
                            content.get_file_name()?,
                            expiry_date,
                            header_filename,
                            &config,
                        )
                        .await?
                }
                PasteType::RemoteFile => {
                    paste
                        .store_remote_file(expiry_date, &client, &config)
                        .await?
                }
                PasteType::Url | PasteType::OneshotUrl => {
                    paste.store_url(expiry_date, &config).await?
                }
            };
            info!(
                "{} ({}) is uploaded from {}",
                file_name,
                Byte::from_u128(paste.data.len() as u128)
                    .unwrap_or_default()
                    .get_appropriate_unit(UnitType::Decimal),
                host
            );
            if let Some(handle_spaces_config) = config.server.handle_spaces {
                file_name = handle_spaces_config.process_filename(&file_name);
            }
            urls.push(format!("{}/{}\n", server_url, file_name));
        } else {
            warn!("{} sent an invalid form field", host);
            return Err(error::ErrorBadRequest("invalid form field"));
        }
    }
    Ok(HttpResponse::Ok().body(urls.join("")))
}

/// File entry item for list endpoint.
#[derive(Serialize, Deserialize)]
pub struct ListItem {
    /// Uploaded file name.
    pub file_name: PathBuf,
    /// Size of the file in bytes.
    pub file_size: u64,
    /// ISO8601 formatted date-time string of the expiration timestamp if one exists for this file.
    pub expires_at_utc: Option<String>,
}

/// Returns the list of files.
#[get("/list")]
#[actix_web_grants::protect("TokenType::Auth", ty = TokenType, error = unauthorized_error)]
async fn list(config: web::Data<RwLock<Config>>) -> Result<HttpResponse, Error> {
    let config = config.read().await;

    if !config.server.expose_list.unwrap_or(false) {
        warn!("server is not configured to expose list endpoint");
        return Err(error::ErrorNotFound(""));
    }

    let mut entries: Vec<ListItem> = Vec::new();

    let mut dir_contents = fs::read_dir(&config.server.upload_path).await?;

    let system_time = util::get_system_time()?;

    while let Ok(Some(entry)) = dir_contents.next_entry().await {
        let metadata = match entry.metadata().await {
            Ok(metadata) if metadata.is_dir() => continue,
            Ok(metadata) => metadata,
            Err(e) => {
                error!("failed to read metadata: {e}");
                continue;
            }
        };

        let mut file_name = PathBuf::from(entry.file_name());

        let expires_at_utc = match file_name
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(|v| v.parse::<u64>().ok())
        {
            Some(expiration) if system_time > Duration::from_millis(expiration) => continue,
            Some(expiration) => {
                file_name.set_extension("");
                Some(
                    uts2ts::uts2ts(
                        (expiration / 1000)
                            .try_into()
                            .map_err(|_| error::ErrorInternalServerError("Invalid timestamp"))?,
                    )
                    .as_string(),
                )
            }
            None => None,
        };

        entries.push(ListItem {
            file_name,
            file_size: metadata.len(),
            expires_at_utc,
        });
    }

    Ok(HttpResponse::Ok().json(entries))
}

/// Configures the server routes.
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .service(index)
            .service(version)
            .service(list)
            .service(serve)
            .service(upload)
            .service(delete)
            .route("", web::head().to(HttpResponse::MethodNotAllowed))
            .wrap(GrantsMiddleware::with_extractor(extract_tokens))
            .wrap(
                ErrorHandlers::new().handler(StatusCode::UNAUTHORIZED, handle_unauthorized_error),
            ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LandingPageConfig;
    use crate::middleware::ContentLengthLimiter;
    use crate::random::{RandomURLConfig, RandomURLType};
    use actix_web::body::MessageBody;
    use actix_web::body::{BodySize, BoxBody};
    use actix_web::error::Error;
    use actix_web::http::header::AUTHORIZATION;
    use actix_web::http::{header, StatusCode};
    use actix_web::test::{self, TestRequest};
    use actix_web::web::Data;
    use actix_web::App;
    use awc::ClientBuilder;
    use glob::glob;
    use std::fs::File;
    use std::io::Write;
    use std::path::PathBuf;
    use std::str;
    use std::thread;
    use std::time::Duration;

    fn get_multipart_request(data: &str, name: &str, filename: &str) -> TestRequest {
        let multipart_data = format!(
            "\r\n\
             --multipart_bound\r\n\
             Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\n\
             Content-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\n\r\n\
             {}\r\n\
             --multipart_bound--\r\n",
            name,
            filename,
            data.bytes().len(),
            data,
        );
        TestRequest::post()
            .insert_header((
                header::CONTENT_TYPE,
                header::HeaderValue::from_static("multipart/mixed; boundary=\"multipart_bound\""),
            ))
            .insert_header((
                header::CONTENT_LENGTH,
                header::HeaderValue::from_str(&data.bytes().len().to_string())
                    .expect("cannot create header value"),
            ))
            .set_payload(multipart_data)
    }

    async fn assert_body(body: BoxBody, expected: &str) -> Result<(), Error> {
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
    async fn test_index() {
        let config = Config::default();
        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(config)))
                .service(index),
        )
        .await;
        let request = TestRequest::default()
            .insert_header(("content-type", "text/plain"))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(StatusCode::FOUND, response.status());
    }

    #[actix_web::test]
    async fn test_index_with_landing_page() -> Result<(), Error> {
        let config = Config {
            landing_page: Some(LandingPageConfig {
                text: Some(String::from("landing page")),
                ..Default::default()
            }),
            ..Default::default()
        };
        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(config)))
                .service(index),
        )
        .await;
        let request = TestRequest::default()
            .insert_header(("content-type", "text/plain"))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(StatusCode::OK, response.status());
        assert_body(response.into_body(), "landing page").await?;
        Ok(())
    }

    #[actix_web::test]
    async fn test_index_with_landing_page_file() -> Result<(), Error> {
        let filename = "landing_page.txt";
        let config = Config {
            landing_page: Some(LandingPageConfig {
                file: Some(filename.to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut file = File::create(filename)?;
        file.write_all("landing page from file".as_bytes())?;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(config)))
                .service(index),
        )
        .await;
        let request = TestRequest::default()
            .insert_header(("content-type", "text/plain"))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(StatusCode::OK, response.status());
        assert_body(response.into_body(), "landing page from file").await?;
        fs::remove_file(filename).await?;
        Ok(())
    }

    #[actix_web::test]
    async fn test_index_with_landing_page_file_not_found() -> Result<(), Error> {
        let filename = "landing_page.txt";
        let config = Config {
            landing_page: Some(LandingPageConfig {
                text: Some(String::from("landing page")),
                file: Some(filename.to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(config)))
                .service(index),
        )
        .await;
        let request = TestRequest::default()
            .insert_header(("content-type", "text/plain"))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(StatusCode::FOUND, response.status());
        Ok(())
    }

    #[actix_web::test]
    async fn test_version_without_auth() -> Result<(), Error> {
        let mut config = Config::default();
        config.server.auth_tokens = Some(["test".to_string()].into());
        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(config)))
                .app_data(Data::new(Client::default()))
                .configure(configure_routes),
        )
        .await;

        let request = TestRequest::default()
            .insert_header(("content-type", "text/plain"))
            .uri("/version")
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(StatusCode::UNAUTHORIZED, response.status());
        assert_body(response.into_body(), "unauthorized\n").await?;
        Ok(())
    }

    #[actix_web::test]
    async fn test_version_without_config() -> Result<(), Error> {
        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(Config::default())))
                .app_data(Data::new(Client::default()))
                .configure(configure_routes),
        )
        .await;

        let request = TestRequest::default()
            .insert_header(("content-type", "text/plain"))
            .uri("/version")
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(StatusCode::NOT_FOUND, response.status());
        assert_body(response.into_body(), "").await?;
        Ok(())
    }

    #[actix_web::test]
    async fn test_version() -> Result<(), Error> {
        let mut config = Config::default();
        config.server.expose_version = Some(true);
        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(config)))
                .app_data(Data::new(Client::default()))
                .configure(configure_routes),
        )
        .await;

        let request = TestRequest::default()
            .insert_header(("content-type", "text/plain"))
            .uri("/version")
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(StatusCode::OK, response.status());
        assert_body(
            response.into_body(),
            &(env!("CARGO_PKG_VERSION").to_owned() + "\n"),
        )
        .await?;
        Ok(())
    }

    #[actix_web::test]
    async fn test_list() -> Result<(), Error> {
        let mut config = Config::default();
        config.server.expose_list = Some(true);

        let test_upload_dir = "test_upload";
        fs::create_dir(test_upload_dir).await?;
        config.server.upload_path = PathBuf::from(test_upload_dir);

        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(config)))
                .app_data(Data::new(Client::default()))
                .configure(configure_routes),
        )
        .await;

        let filename = "test_file.txt";
        let timestamp = util::get_system_time()?.as_secs().to_string();
        test::call_service(
            &app,
            get_multipart_request(&timestamp, "file", filename).to_request(),
        )
        .await;

        let request = TestRequest::default()
            .insert_header(("content-type", "text/plain"))
            .uri("/list")
            .to_request();
        let result: Vec<ListItem> = test::call_and_read_body_json(&app, request).await;

        assert_eq!(result.len(), 1);
        assert_eq!(
            result.first().expect("json object").file_name,
            PathBuf::from(filename)
        );

        fs::remove_dir_all(test_upload_dir).await?;

        Ok(())
    }

    #[actix_web::test]
    async fn test_list_expired() -> Result<(), Error> {
        let mut config = Config::default();
        config.server.expose_list = Some(true);

        let test_upload_dir = "test_upload";
        fs::create_dir(test_upload_dir).await?;
        config.server.upload_path = PathBuf::from(test_upload_dir);

        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(config)))
                .app_data(Data::new(Client::default()))
                .configure(configure_routes),
        )
        .await;

        let filename = "test_file.txt";
        let timestamp = util::get_system_time()?.as_secs().to_string();
        test::call_service(
            &app,
            get_multipart_request(&timestamp, "file", filename)
                .insert_header((
                    header::HeaderName::from_static("expire"),
                    header::HeaderValue::from_static("50ms"),
                ))
                .to_request(),
        )
        .await;

        thread::sleep(Duration::from_millis(500));

        let request = TestRequest::default()
            .insert_header(("content-type", "text/plain"))
            .uri("/list")
            .to_request();
        let result: Vec<ListItem> = test::call_and_read_body_json(&app, request).await;

        assert!(result.is_empty());

        fs::remove_dir_all(test_upload_dir).await?;

        Ok(())
    }

    #[actix_web::test]
    async fn test_auth() -> Result<(), Error> {
        let mut config = Config::default();
        config.server.auth_tokens = Some(["test".to_string()].into());

        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(config)))
                .app_data(Data::new(Client::default()))
                .configure(configure_routes),
        )
        .await;

        let response =
            test::call_service(&app, get_multipart_request("", "", "").to_request()).await;
        assert_eq!(StatusCode::UNAUTHORIZED, response.status());
        assert_body(response.into_body(), "unauthorized\n").await?;

        Ok(())
    }

    #[actix_web::test]
    async fn test_payload_limit() -> Result<(), Error> {
        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(Config::default())))
                .app_data(Data::new(Client::default()))
                .wrap(ContentLengthLimiter::new(Byte::from_u64(1)))
                .configure(configure_routes),
        )
        .await;

        let response = test::call_service(
            &app,
            get_multipart_request("test", "file", "test").to_request(),
        )
        .await;
        assert_eq!(StatusCode::PAYLOAD_TOO_LARGE, response.status());
        assert_body(response.into_body().boxed(), "upload limit exceeded").await?;

        Ok(())
    }

    #[actix_web::test]
    async fn test_delete_file() -> Result<(), Error> {
        let mut config = Config::default();
        config.server.delete_tokens = Some(["test".to_string()].into());
        config.server.upload_path = env::current_dir()?;

        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(config)))
                .app_data(Data::new(Client::default()))
                .configure(configure_routes),
        )
        .await;

        let file_name = "test_file.txt";
        let timestamp = util::get_system_time()?.as_secs().to_string();
        test::call_service(
            &app,
            get_multipart_request(&timestamp, "file", file_name).to_request(),
        )
        .await;

        let request = TestRequest::delete()
            .insert_header((AUTHORIZATION, header::HeaderValue::from_static("test")))
            .uri(&format!("/{file_name}"))
            .to_request();
        let response = test::call_service(&app, request).await;

        assert_eq!(StatusCode::OK, response.status());
        assert_body(response.into_body(), "file deleted\n").await?;

        let path = PathBuf::from(file_name);
        assert!(!path.exists());

        Ok(())
    }

    #[actix_web::test]
    async fn test_delete_file_without_token_in_config() -> Result<(), Error> {
        let mut config = Config::default();
        config.server.upload_path = env::current_dir()?;

        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(config)))
                .app_data(Data::new(Client::default()))
                .configure(configure_routes),
        )
        .await;

        let file_name = "test_file.txt";
        let request = TestRequest::delete()
            .insert_header((AUTHORIZATION, header::HeaderValue::from_static("test")))
            .uri(&format!("/{file_name}"))
            .to_request();
        let response = test::call_service(&app, request).await;

        assert_eq!(StatusCode::NOT_FOUND, response.status());
        assert_body(response.into_body(), "").await?;

        Ok(())
    }

    #[actix_web::test]
    async fn test_upload_file() -> Result<(), Error> {
        let mut config = Config::default();
        config.server.upload_path = env::current_dir()?;

        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(config)))
                .app_data(Data::new(Client::default()))
                .configure(configure_routes),
        )
        .await;

        let file_name = "test_file.txt";
        let timestamp = util::get_system_time()?.as_secs().to_string();
        let response = test::call_service(
            &app,
            get_multipart_request(&timestamp, "file", file_name).to_request(),
        )
        .await;
        assert_eq!(StatusCode::OK, response.status());
        assert_body(
            response.into_body(),
            &format!("http://localhost:8080/{file_name}\n"),
        )
        .await?;

        let serve_request = TestRequest::get()
            .uri(&format!("/{file_name}"))
            .to_request();
        let response = test::call_service(&app, serve_request).await;
        assert_eq!(StatusCode::OK, response.status());
        assert_body(response.into_body(), &timestamp).await?;

        fs::remove_file(file_name).await?;
        let serve_request = TestRequest::get()
            .uri(&format!("/{file_name}"))
            .to_request();
        let response = test::call_service(&app, serve_request).await;
        assert_eq!(StatusCode::NOT_FOUND, response.status());

        Ok(())
    }

    #[actix_web::test]
    async fn test_upload_file_override_filename() -> Result<(), Error> {
        let mut config = Config::default();
        config.server.upload_path = env::current_dir()?;

        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(config)))
                .app_data(Data::new(Client::default()))
                .configure(configure_routes),
        )
        .await;

        let file_name = "test_file.txt";
        let header_filename = "fn_from_header.txt";
        let timestamp = util::get_system_time()?.as_secs().to_string();
        let response = test::call_service(
            &app,
            get_multipart_request(&timestamp, "file", file_name)
                .insert_header((
                    header::HeaderName::from_static("filename"),
                    header::HeaderValue::from_static("fn_from_header.txt"),
                ))
                .to_request(),
        )
        .await;
        assert_eq!(StatusCode::OK, response.status());
        assert_body(
            response.into_body(),
            &format!("http://localhost:8080/{header_filename}\n"),
        )
        .await?;

        let serve_request = TestRequest::get()
            .uri(&format!("/{header_filename}"))
            .to_request();
        let response = test::call_service(&app, serve_request).await;
        assert_eq!(StatusCode::OK, response.status());
        assert_body(response.into_body(), &timestamp).await?;

        fs::remove_file(header_filename).await?;
        let serve_request = TestRequest::get()
            .uri(&format!("/{header_filename}"))
            .to_request();
        let response = test::call_service(&app, serve_request).await;
        assert_eq!(StatusCode::NOT_FOUND, response.status());

        Ok(())
    }

    #[actix_web::test]
    #[allow(deprecated)]
    async fn test_upload_duplicate_file() -> Result<(), Error> {
        let test_upload_dir = "test_upload";
        fs::create_dir(test_upload_dir).await?;

        let mut config = Config::default();
        config.server.upload_path = PathBuf::from(&test_upload_dir);
        config.paste.duplicate_files = Some(false);
        config.paste.random_url = Some(RandomURLConfig {
            enabled: Some(true),
            type_: RandomURLType::Alphanumeric,
            ..Default::default()
        });

        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(config)))
                .app_data(Data::new(Client::default()))
                .configure(configure_routes),
        )
        .await;

        let response = test::call_service(
            &app,
            get_multipart_request("test", "file", "x").to_request(),
        )
        .await;
        assert_eq!(StatusCode::OK, response.status());
        let body = response.into_body();
        let first_body_bytes = actix_web::body::to_bytes(body).await?;

        let response = test::call_service(
            &app,
            get_multipart_request("test", "file", "x").to_request(),
        )
        .await;
        assert_eq!(StatusCode::OK, response.status());
        let body = response.into_body();
        let second_body_bytes = actix_web::body::to_bytes(body).await?;

        assert_eq!(first_body_bytes, second_body_bytes);

        fs::remove_dir_all(test_upload_dir).await?;

        Ok(())
    }

    #[actix_web::test]
    async fn test_upload_expiring_file() -> Result<(), Error> {
        let mut config = Config::default();
        config.server.upload_path = env::current_dir()?;

        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(config)))
                .app_data(Data::new(Client::default()))
                .configure(configure_routes),
        )
        .await;

        let file_name = "test_file.txt";
        let timestamp = util::get_system_time()?.as_secs().to_string();
        let response = test::call_service(
            &app,
            get_multipart_request(&timestamp, "file", file_name)
                .insert_header((
                    header::HeaderName::from_static("expire"),
                    header::HeaderValue::from_static("20ms"),
                ))
                .to_request(),
        )
        .await;
        assert_eq!(StatusCode::OK, response.status());
        assert_body(
            response.into_body(),
            &format!("http://localhost:8080/{file_name}\n"),
        )
        .await?;

        let serve_request = TestRequest::get()
            .uri(&format!("/{file_name}"))
            .to_request();
        let response = test::call_service(&app, serve_request).await;
        assert_eq!(StatusCode::OK, response.status());
        assert_body(response.into_body(), &timestamp).await?;

        thread::sleep(Duration::from_millis(40));

        let serve_request = TestRequest::get()
            .uri(&format!("/{file_name}"))
            .to_request();
        let response = test::call_service(&app, serve_request).await;
        assert_eq!(StatusCode::NOT_FOUND, response.status());

        if let Some(glob_path) = glob(&format!("{file_name}.[0-9]*"))
            .map_err(error::ErrorInternalServerError)?
            .next()
        {
            fs::remove_file(glob_path.map_err(error::ErrorInternalServerError)?).await?;
        }

        Ok(())
    }

    #[actix_web::test]
    async fn test_upload_remote_file() -> Result<(), Error> {
        let mut config = Config::default();
        config.server.upload_path = env::current_dir()?;
        config.server.max_content_length = Byte::from_u128(30000).unwrap_or_default();

        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(config)))
                .app_data(Data::new(
                    ClientBuilder::new()
                        .timeout(Duration::from_secs(30))
                        .finish(),
                ))
                .configure(configure_routes),
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
        assert_eq!(StatusCode::OK, response.status());
        assert_body(
            response.into_body().boxed(),
            &format!("http://localhost:8080/{file_name}\n"),
        )
        .await?;

        let serve_request = TestRequest::get()
            .uri(&format!("/{file_name}"))
            .to_request();
        let response = test::call_service(&app, serve_request).await;
        assert_eq!(StatusCode::OK, response.status());

        let body = response.into_body();
        let body_bytes = actix_web::body::to_bytes(body).await?;
        assert_eq!(
            "70ff72a2f7651b5fae3aa9834e03d2a2233c52036610562f7fa04e089e8198ed",
            util::sha256_digest(&*body_bytes)?
        );

        fs::remove_file(file_name).await?;

        let serve_request = TestRequest::get()
            .uri(&format!("/{file_name}"))
            .to_request();
        let response = test::call_service(&app, serve_request).await;
        assert_eq!(StatusCode::NOT_FOUND, response.status());

        Ok(())
    }

    #[actix_web::test]
    async fn test_upload_url() -> Result<(), Error> {
        let mut config = Config::default();
        config.server.upload_path = env::current_dir()?;

        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(config.clone())))
                .app_data(Data::new(Client::default()))
                .configure(configure_routes),
        )
        .await;

        let url_upload_path = PasteType::Url
            .get_path(&config.server.upload_path)
            .expect("Bad upload path");
        fs::create_dir_all(&url_upload_path).await?;

        let response = test::call_service(
            &app,
            get_multipart_request(env!("CARGO_PKG_HOMEPAGE"), "url", "").to_request(),
        )
        .await;
        assert_eq!(StatusCode::OK, response.status());
        assert_body(response.into_body(), "http://localhost:8080/url\n").await?;

        let serve_request = TestRequest::get().uri("/url").to_request();
        let response = test::call_service(&app, serve_request).await;
        assert_eq!(StatusCode::FOUND, response.status());

        fs::remove_file(url_upload_path.join("url")).await?;
        fs::remove_dir(url_upload_path).await?;

        let serve_request = TestRequest::get().uri("/url").to_request();
        let response = test::call_service(&app, serve_request).await;
        assert_eq!(StatusCode::NOT_FOUND, response.status());

        Ok(())
    }

    #[actix_web::test]
    async fn test_upload_oneshot() -> Result<(), Error> {
        let mut config = Config::default();
        config.server.upload_path = env::current_dir()?;

        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(config.clone())))
                .app_data(Data::new(Client::default()))
                .configure(configure_routes),
        )
        .await;

        let oneshot_upload_path = PasteType::Oneshot
            .get_path(&config.server.upload_path)
            .expect("Bad upload path");
        fs::create_dir_all(&oneshot_upload_path).await?;

        let file_name = "oneshot.txt";
        let timestamp = util::get_system_time()?.as_secs().to_string();
        let response = test::call_service(
            &app,
            get_multipart_request(&timestamp, "oneshot", file_name).to_request(),
        )
        .await;
        assert_eq!(StatusCode::OK, response.status());
        assert_body(
            response.into_body(),
            &format!("http://localhost:8080/{file_name}\n"),
        )
        .await?;

        let serve_request = TestRequest::get()
            .uri(&format!("/{file_name}"))
            .to_request();
        let response = test::call_service(&app, serve_request).await;
        assert_eq!(StatusCode::OK, response.status());
        assert_body(response.into_body(), &timestamp).await?;

        let serve_request = TestRequest::get()
            .uri(&format!("/{file_name}"))
            .to_request();
        let response = test::call_service(&app, serve_request).await;
        assert_eq!(StatusCode::NOT_FOUND, response.status());

        if let Some(glob_path) = glob(
            &oneshot_upload_path
                .join(format!("{file_name}.[0-9]*"))
                .to_string_lossy(),
        )
        .map_err(error::ErrorInternalServerError)?
        .next()
        {
            fs::remove_file(glob_path.map_err(error::ErrorInternalServerError)?).await?;
        }
        fs::remove_dir(oneshot_upload_path).await?;

        Ok(())
    }

    #[actix_web::test]
    async fn test_upload_oneshot_url() -> Result<(), Error> {
        let mut config = Config::default();
        config.server.upload_path = env::current_dir()?;

        let oneshot_url_suffix = "oneshot_url";

        let app = test::init_service(
            App::new()
                .app_data(Data::new(RwLock::new(config.clone())))
                .app_data(Data::new(Client::default()))
                .configure(configure_routes),
        )
        .await;

        let url_upload_path = PasteType::OneshotUrl
            .get_path(&config.server.upload_path)
            .expect("Bad upload path");
        fs::create_dir_all(&url_upload_path).await?;

        let response = test::call_service(
            &app,
            get_multipart_request(
                env!("CARGO_PKG_HOMEPAGE"),
                oneshot_url_suffix,
                oneshot_url_suffix,
            )
            .to_request(),
        )
        .await;
        assert_eq!(StatusCode::OK, response.status());
        assert_body(
            response.into_body(),
            &format!("http://localhost:8080/{}\n", oneshot_url_suffix),
        )
        .await?;

        // Make the oneshot_url request, ensure it is found.
        let serve_request = TestRequest::with_uri(&format!("/{}", oneshot_url_suffix)).to_request();
        let response = test::call_service(&app, serve_request).await;
        assert_eq!(StatusCode::FOUND, response.status());

        // Make the same request again, and ensure that the oneshot_url is not found.
        let serve_request = TestRequest::with_uri(&format!("/{}", oneshot_url_suffix)).to_request();
        let response = test::call_service(&app, serve_request).await;
        assert_eq!(StatusCode::NOT_FOUND, response.status());

        // Cleanup
        fs::remove_dir_all(url_upload_path).await?;

        Ok(())
    }
}
