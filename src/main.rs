use actix_multipart::form::MultipartFormConfig;
use actix_multipart::MultipartError;
use actix_web::middleware::Logger;
use actix_web::web::Data;
use actix_web::Error;
use actix_web::HttpRequest;
use actix_web::{App, HttpServer};
use awc::ClientBuilder;
use byte_unit::Byte;
use hotwatch::notify::event::ModifyKind;
use hotwatch::{Event, EventKind, Hotwatch};
use rustypaste::config::{Config, ServerConfig};
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
use tracing_subscriber::{
    filter::LevelFilter, layer::SubscriberExt as _, util::SubscriberInitExt as _, EnvFilter,
};

// Use macros from tracing crate.
#[macro_use]
extern crate tracing;

fn handle_multipart_error(err: MultipartError, _req: &HttpRequest) -> Error {
    error!("Multipart error: {}", err);
    Error::from(err)
}

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
            unsafe {
                env::remove_var(CONFIG_ENV);
            }
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
    let server_config = config.server.clone();
    let paste_config = RwLock::new(config.paste.clone());
    let (config_sender, config_receiver) = mpsc::channel::<Config>();

    // Create necessary directories.
    fs::create_dir_all(&server_config.upload_path)?;
    for paste_type in &[PasteType::Url, PasteType::Oneshot, PasteType::OneshotUrl] {
        fs::create_dir_all(paste_type.get_path(&server_config.upload_path)?)?;
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
            (event.kind, event.paths.first())
        {
            match Config::parse(path) {
                Ok(config) => match cloned_config.write() {
                    Ok(mut cloned_config) => {
                        *cloned_config = config.clone();
                        info!("Configuration has been updated.");
                        if let Err(e) = config_sender.send(config) {
                            error!("Failed to send config for the cleanup routine: {}", e)
                        }
                        cloned_config.warn_deprecation();
                    }
                    Err(e) => {
                        error!("Failed to acquire config: {}", e);
                    }
                },
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
    let upload_path = server_config.upload_path.clone();
    thread::spawn(move || loop {
        let mut enabled = false;
        if let Some(ref cleanup_config) = paste_config
            .read()
            .ok()
            .and_then(|v| v.delete_expired_files.clone())
        {
            if cleanup_config.enabled {
                debug!("Running cleanup...");
                for file in util::get_expired_files(&upload_path) {
                    match fs::remove_file(&file) {
                        Ok(()) => info!("Removed expired file: {:?}", file),
                        Err(e) => error!("Cannot remove expired file: {}", e),
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
                    error!("Failed to update config for the cleanup routine: {}", e);
                }
            }
        }
    });

    Ok((config, server_config, hotwatch))
}

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
            .app_data(
                MultipartFormConfig::default()
                    .total_limit(
                        Byte::parse_str(server_config.max_content_length.to_string(), true)
                            .expect("cannot parse byte")
                            .as_u64()
                            .try_into()
                            .unwrap(),
                    )
                    .memory_limit(10 * 1024 * 1024)
                    .error_handler(handle_multipart_error),
            )
            .wrap(Logger::new(
                "%{r}a \"%r\" %s %b \"%{Referer}i\" \"%{User-Agent}i\" %T",
            ))
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
