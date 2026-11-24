use actix_web::middleware::Logger;
use actix_web::web::Data;
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
use std::io::{Read, Result as IoResult, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{mpsc, RwLock};
use std::thread;
use std::time::Duration;
use tracing_subscriber::{
    filter::LevelFilter, layer::SubscriberExt as _, util::SubscriberInitExt as _, EnvFilter,
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
    for paste_type in &[
        PasteType::Url,
        PasteType::Oneshot,
        PasteType::OneshotUrl,
        PasteType::ProtectedFile,
    ] {
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
                // Clean up expired files
                for file in util::get_expired_files(&upload_path) {
                    // Delete content file first, then password (prevents unprotected window)
                    match fs::remove_file(&file) {
                        Ok(()) => {
                            info!("Removed expired file: {:?}", file);
                            rustypaste::password::delete_password_file(&file).ok();
                        }
                        Err(e) => error!("Cannot remove expired file: {}", e),
                    }
                }

                // Clean orphaned password files
                for password_file in util::get_orphaned_password_files(&upload_path) {
                    match fs::remove_file(&password_file) {
                        Ok(()) => info!("Removed orphaned password: {:?}", password_file),
                        Err(e) => error!("Cannot remove orphaned password: {}", e),
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

#[cfg(unix)]
fn remove_stale_unix_socket(path: &std::path::Path) -> std::io::Result<()> {
    use std::fs;
    use std::io::{Error, ErrorKind};
    use std::os::unix::fs::FileTypeExt;
    use std::os::unix::net::UnixStream;

    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };

    // Never remove regular files, symlinks, or other filesystem entries.
    if !metadata.file_type().is_socket() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            format!(
                "Unix socket path exists but is not a socket: {}",
                path.display()
            ),
        ));
    }

    match UnixStream::connect(path) {
        Ok(_) => {
            // A listener is accepting connections; do not remove its socket.
            return Err(Error::new(
                ErrorKind::AddrInUse,
                format!("Unix socket is already in use: {}", path.display()),
            ));
        }
        Err(e) if e.kind() == ErrorKind::ConnectionRefused => {
            // No listener is accepting connections: likely a stale socket.
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
            // Another process may have removed it between our checks.
            return Ok(());
        }
        Err(e) => return Err(e),
    }

    // Recheck that the path still refers to a socket before removing it.
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };

    if !metadata.file_type().is_socket() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            format!("Socket path changed during cleanup: {}", path.display()),
        ));
    }

    fs::remove_file(path)
}

fn healthcheck(host: &str, port: u16) -> IoResult<bool> {
    let mut stream = TcpStream::connect((host, port))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.write_all(format!("GET /health HTTP/1.0\r\nHost: {host}:{port}\r\n\r\n").as_bytes())?;
    // "HTTP/1.1 200"
    let mut status = [0; 12];
    stream.read_exact(&mut status)?;
    Ok(status.starts_with(b"HTTP/1.") && status.ends_with(b" 200"))
}

#[actix_web::main]
async fn main() -> IoResult<ExitCode> {
    let args: Vec<String> = env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "healthcheck") {
        return Ok(
            // Panicking on incorrect use is fine since there is no public CLI
            if healthcheck(&args[2], args[3].parse().unwrap())
                // Run `sudo docker exec -it rustypaste ./rustypaste healthcheck 127.0.0.1 8000`
                //  on the command line to see why the healthcheck fails
                .inspect_err(|err| println!("{err:?}"))
                .unwrap_or(false)
            {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            },
        );
    }

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
            .wrap(ContentLengthLimiter::new(server_config.max_content_length))
            .configure(server::configure_routes)
    })
    .bind(&server_config.address)?;

    // Set worker count for the server.
    if let Some(workers) = server_config.workers {
        http_server = http_server.workers(workers);
    }

    // Optionally bind to unix socket
    #[cfg(unix)]
    if let Some(socket) = server_config.socket.as_deref() {
        remove_stale_unix_socket(socket)?;
        http_server = http_server.bind_uds(socket)?;
    }

    // Run the server.
    info!("Server is running at {}", server_config.address);
    #[cfg(unix)]
    if let Some(socket) = server_config.socket.as_deref() {
        info!("Server is listening on Unix socket {}", socket.display());
    }
    http_server.run().await?;
    Ok(ExitCode::SUCCESS)
}
