use actix_web::middleware::Logger;
use actix_web::web::Data;
#[cfg(not(feature = "shuttle"))]
use actix_web::{App, HttpServer};
use awc::ClientBuilder;
use hotwatch::notify::event::ModifyKind;
use hotwatch::{Event, EventKind, Hotwatch};
use rustypaste::config::{Config, DEFAULT_CLEANUP_INTERVAL};
use rustypaste::middleware::ContentLengthLimiter;
use rustypaste::paste::PasteType;
use rustypaste::server;
use rustypaste::util;
use rustypaste::CONFIG_ENV;
use std::env;
use std::fs;
use std::io::Result as IoResult;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use tokio::sync::RwLock;
#[cfg(not(feature = "shuttle"))]
use tracing_subscriber::{
    filter::LevelFilter, layer::SubscriberExt as _, util::SubscriberInitExt as _, EnvFilter,
};
#[cfg(feature = "shuttle")]
use {
    actix_web::web::{self, ServiceConfig},
    shuttle_actix_web::ShuttleActixWeb,
};

// Use macros from tracing crate.
#[macro_use]
extern crate tracing;

/// Sets up the application.
///
/// * loads the configuration
/// * initializes the logger
/// * creates the necessary directories
/// * spawns the threads
async fn setup(config_folder: &Path) -> IoResult<(Data<RwLock<Config>>, Hotwatch)> {
    // Load the .env file.
    dotenvy::dotenv().ok();

    // Initialize logger.
    #[cfg(not(feature = "shuttle"))]
    tracing_subscriber::registry()
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Parse configuration.
    let config_path = match env::var(CONFIG_ENV).ok() {
        Some(path) => {
            env::remove_var(CONFIG_ENV);
            PathBuf::from(path)
        }
        None => config_folder.join("config.toml"),
    };

    if !config_path.exists() {
        error!(
            "{} is not found, please provide a configuration file.",
            config_path.display()
        );
        std::process::exit(1);
    }

    let config = Config::parse(&config_path).expect("failed to parse config");
    trace!("{:#?}", config);
    config.warn_deprecation();

    // Create necessary directories.
    fs::create_dir_all(&config.server.upload_path)?;
    for paste_type in &[PasteType::Url, PasteType::Oneshot, PasteType::OneshotUrl] {
        fs::create_dir_all(paste_type.get_path(&config.server.upload_path)?)?;
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

    let config_lock = Data::new(RwLock::new(config));

    // Hot-reload the configuration file.
    let config_watcher_config = config_lock.clone();
    let config_watcher = move |event: Event| {
        if let (EventKind::Modify(ModifyKind::Data(_)), Some(path)) =
            (event.kind, event.paths.first())
        {
            info!("Reloading configuration");

            match Config::parse(path) {
                Ok(new_config) => {
                    let mut locked_config = config_watcher_config.blocking_write();
                    *locked_config = new_config;
                    info!("Configuration has been updated.");
                    locked_config.warn_deprecation();
                }
                Err(e) => {
                    error!("Failed to update config: {}", e);
                }
            }
        }
    };

    hotwatch
        .watch(&config_path, config_watcher)
        .unwrap_or_else(|_| panic!("failed to watch {config_path:?}"));

    // Create a thread for cleaning up expired files.
    let expired_files_config = config_lock.clone();
    let mut cleanup_interval = DEFAULT_CLEANUP_INTERVAL;
    thread::spawn(move || loop {
        // Additional context block to ensure the config lock is dropped
        {
            let locked_config = expired_files_config.blocking_read();
            let upload_path = locked_config.server.upload_path.clone();

            if let Some(ref cleanup_config) = locked_config.paste.delete_expired_files {
                if cleanup_config.enabled {
                    debug!("Running cleanup...");
                    for file in util::get_expired_files(&upload_path) {
                        match fs::remove_file(&file) {
                            Ok(()) => info!("Removed expired file: {:?}", file),
                            Err(e) => error!("Cannot remove expired file: {}", e),
                        }
                    }
                    cleanup_interval = cleanup_config.interval;
                }
            }
        }

        thread::sleep(cleanup_interval);
    });

    Ok((config_lock, hotwatch))
}

#[cfg(not(feature = "shuttle"))]
#[actix_web::main]
async fn main() -> IoResult<()> {
    // Set up the application.
    let (config, _hotwatch) = setup(&PathBuf::new()).await?;

    // Extra context block ensures the lock is stopped
    let server_config = { config.read().await.server.clone() };

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
            .wrap(ContentLengthLimiter::new(server_config.max_content_length))
            .configure(server::configure_routes)
    })
    .bind(&server_config.address)?;

    // Set worker count for the server.
    if let Some(workers) = server_config.workers {
        http_server = http_server.workers(workers);
    }

    // Run the server.
    info!("Server is running at {}", server_config.address);
    http_server.run().await
}

#[cfg(feature = "shuttle")]
#[shuttle_runtime::main]
async fn actix_web() -> ShuttleActixWeb<impl FnOnce(&mut ServiceConfig) + Send + Clone + 'static> {
    // Set up the application.
    let (config, _hotwatch) = setup(Path::new("shuttle"))?;

    // Extra context block ensures the lock is stopped
    let server_config = { config.read().await.server.clone() };

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
                .wrap(ContentLengthLimiter::new(server_config.max_content_length))
                .configure(server::configure_routes),
        );
    };

    Ok(service_config.into())
}
