use actix_web::middleware::Logger;
use actix_web::web::Data;
use actix_web::{App, HttpServer};
use awc::ClientBuilder;
use hotwatch::{Event, Hotwatch};
use rustypaste::config::Config;
use rustypaste::paste::PasteType;
use rustypaste::server;
use rustypaste::CONFIG_ENV;
use std::env;
use std::fs;
use std::io::Result as IoResult;
use std::path::PathBuf;
use std::sync::RwLock;
use std::time::Duration;

#[actix_web::main]
async fn main() -> IoResult<()> {
    // Initialize logger.
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // Parse configuration.
    dotenv::dotenv().ok();
    let config_path = match env::var(CONFIG_ENV).ok() {
        Some(path) => {
            env::remove_var(CONFIG_ENV);
            PathBuf::from(path)
        }
        None => PathBuf::from("config.toml"),
    };
    let config = Config::parse(&config_path).expect("failed to parse config");
    let server_config = config.server.clone();

    // Create necessary directories.
    fs::create_dir_all(&server_config.upload_path)?;
    for paste_type in &[PasteType::Url, PasteType::Oneshot] {
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
        if let Event::Write(path) = event {
            match Config::parse(&path) {
                Ok(config) => match cloned_config.write() {
                    Ok(mut cloned_config) => {
                        *cloned_config = config;
                        log::info!("Configuration has been updated.");
                    }
                    Err(e) => {
                        log::error!("Failed to acquire configuration: {}", e);
                    }
                },
                Err(e) => {
                    log::error!("Failed to update configuration: {}", e);
                }
            }
        }
    };
    hotwatch
        .watch(&config_path, config_watcher)
        .unwrap_or_else(|_| panic!("failed to watch {:?}", config_path));

    // Create a HTTP server.
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
            .wrap(Logger::default())
            .configure(server::configure_routes)
    })
    .bind(server_config.address)?;

    // Set worker count for the server.
    if let Some(workers) = server_config.workers {
        http_server = http_server.workers(workers);
    }

    // Run the server.
    http_server.run().await
}
