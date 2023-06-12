use actix_web::middleware::Logger;
use actix_web::web::Data;
#[cfg(not(feature = "shuttle"))]
use actix_web::{App, HttpServer};
use awc::ClientBuilder;
use hotwatch::notify::event::ModifyKind;
use hotwatch::{Event, EventKind, Hotwatch};
use rustypaste::config::{Config, ServerConfig};
use rustypaste::middleware::ContentLengthLimiter;
use rustypaste::paste::PasteType;
use rustypaste::server;
use rustypaste::util;
use rustypaste::CONFIG_ENV;
use std::env;
use std::fs;
use std::io::Result as IoResult;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, RwLock};
use std::thread;
use std::time::Duration;
#[cfg(feature = "shuttle")]
use {
    actix_web::web::{self, ServiceConfig},
    shuttle_actix_web::ShuttleActixWeb,
};

/// Sets up the application.
///
/// * loads the configuration
/// * initializes the logger
/// * creates the necessary directories
/// * spawns the threads
fn setup(config_folder: &Path) -> IoResult<(Data<RwLock<Config>>, ServerConfig, Hotwatch)> {
    // Load the .env file.
    dotenvy::dotenv().ok();

    // Initialize logger.
    #[cfg(not(feature = "shuttle"))]
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // Parse configuration.
    let config_path = match env::var(CONFIG_ENV).ok() {
        Some(path) => {
            env::remove_var(CONFIG_ENV);
            PathBuf::from(path)
        }
        None => config_folder.join("config.toml"),
    };
    let config = Config::parse(&config_path).expect("failed to parse config");
    log::trace!("{:#?}", config);
    let server_config = config.server.clone();
    let paste_config = RwLock::new(config.paste.clone());
    let (config_sender, config_receiver) = mpsc::channel::<Config>();

    // Create necessary directories.
    fs::create_dir_all(&server_config.upload_path)?;
    for paste_type in &[PasteType::Url, PasteType::Oneshot, PasteType::OneshotUrl] {
        fs::create_dir_all(paste_type.get_path(&server_config.upload_path))?;
    }

    // Set up a watcher for the configuration file changes.
    let mut hotwatch = Hotwatch::new_with_custom_delay(
        config
            .settings
            .as_ref()
            .map(|v| v.refresh_rate)
            .unwrap_or_else(|| Duration::from_secs(1)),
    )
    .expect("failed to initialize configuration file watcher");

    // Hot-reload the configuration file.
    let config = Data::new(RwLock::new(config));
    let cloned_config = Data::clone(&config);
    let config_watcher = move |event: Event| {
        if let (EventKind::Modify(ModifyKind::Data(_)), Some(path)) =
            (event.kind, event.paths.get(0))
        {
            match Config::parse(path) {
                Ok(config) => match cloned_config.write() {
                    Ok(mut cloned_config) => {
                        *cloned_config = config.clone();
                        log::info!("Configuration has been updated.");
                        if let Err(e) = config_sender.send(config) {
                            log::error!("Failed to send config for the cleanup routine: {}", e)
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to acquire config: {}", e);
                    }
                },
                Err(e) => {
                    log::error!("Failed to update config: {}", e);
                }
            }
        }
    };
    hotwatch
        .watch(&config_path, config_watcher)
        .unwrap_or_else(|_| panic!("failed to watch {config_path:?}"));

    // Create a thread for cleaning up expired files.
    let upload_path = server_config.upload_path.clone();
    thread::spawn(move || loop {
        let mut enabled = false;
        if let Some(ref cleanup_config) = paste_config
            .read()
            .ok()
            .and_then(|v| v.delete_expired_files.clone())
        {
            if cleanup_config.enabled {
                log::debug!("Running cleanup...");
                for file in util::get_expired_files(&upload_path) {
                    match fs::remove_file(&file) {
                        Ok(()) => log::info!("Removed expired file: {:?}", file),
                        Err(e) => log::error!("Cannot remove expired file: {}", e),
                    }
                }
                thread::sleep(cleanup_config.interval);
            }
            enabled = cleanup_config.enabled;
        }
        if let Some(new_config) = if enabled {
            config_receiver.try_recv().ok()
        } else {
            config_receiver.recv().ok()
        } {
            match paste_config.write() {
                Ok(mut paste_config) => {
                    *paste_config = new_config.paste;
                }
                Err(e) => {
                    log::error!("Failed to update config for the cleanup routine: {}", e);
                }
            }
        }
    });

    Ok((config, server_config, hotwatch))
}

#[cfg(not(feature = "shuttle"))]
#[actix_web::main]
async fn main() -> IoResult<()> {
    // Set up the application.
    let (config, server_config, _hotwatch) = setup(&PathBuf::new())?;

    // Create an HTTP server.
    let mut http_server = HttpServer::new(move || {
        let http_client = ClientBuilder::new()
            .timeout(
                server_config
                    .timeout
                    .unwrap_or_else(|| Duration::from_secs(30)),
            )
            .disable_redirects()
            .finish();
        App::new()
            .app_data(Data::clone(&config))
            .app_data(Data::new(http_client))
            .wrap(Logger::new(
                "%{r}a \"%r\" %s %b \"%{Referer}i\" \"%{User-Agent}i\" %T",
            ))
            .wrap(ContentLengthLimiter::new(
                server_config.max_content_length.get_bytes(),
            ))
            .configure(server::configure_routes)
    })
    .bind(&server_config.address)?;

    // Set worker count for the server.
    if let Some(workers) = server_config.workers {
        http_server = http_server.workers(workers);
    }

    // Run the server.
    log::info!("Server is running at {}", server_config.address);
    http_server.run().await
}

#[cfg(feature = "shuttle")]
#[shuttle_runtime::main]
async fn actix_web(
    #[shuttle_static_folder::StaticFolder(folder = "shuttle")] static_folder: PathBuf,
) -> ShuttleActixWeb<impl FnOnce(&mut ServiceConfig) + Send + Clone + 'static> {
    // Set up the application.
    let (config, server_config, _hotwatch) = setup(&PathBuf::new())?;

    // Create the service.
    let service_config = move |cfg: &mut ServiceConfig| {
        let http_client = ClientBuilder::new()
            .timeout(
                server_config
                    .timeout
                    .unwrap_or_else(|| Duration::from_secs(30)),
            )
            .disable_redirects()
            .finish();
        cfg.service(
            web::scope("")
                .app_data(Data::clone(&config))
                .app_data(Data::new(http_client))
                .wrap(Logger::new(
                    "%{r}a \"%r\" %s %b \"%{Referer}i\" \"%{User-Agent}i\" %T",
                ))
                .wrap(ContentLengthLimiter::new(
                    server_config.max_content_length.get_bytes(),
                ))
                .configure(server::configure_routes),
        );
    };

    Ok(service_config.into())
}
