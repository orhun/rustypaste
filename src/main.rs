use actix_web::middleware::Logger;
use actix_web::{App, HttpServer};
use rustypaste::config::Config;
use rustypaste::paste::PasteType;
use rustypaste::server;
use std::env;
use std::fs;
use std::io::Result as IoResult;

#[actix_web::main]
async fn main() -> IoResult<()> {
    dotenv::dotenv().ok();
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    let config = Config::parse(env::var("CONFIG").as_deref().unwrap_or("config"))
        .expect("failed to parse config");
    let server_config = config.server.clone();
    fs::create_dir_all(&server_config.upload_path)?;
    for paste_type in &[PasteType::Url, PasteType::Oneshot] {
        fs::create_dir_all(paste_type.get_path(&server_config.upload_path))?;
    }
    let mut http_server = HttpServer::new(move || {
        App::new()
            .data(config.clone())
            .wrap(Logger::default())
            .configure(server::configure_routes)
    })
    .bind(server_config.address)?;
    if let Some(workers) = server_config.workers {
        http_server = http_server.workers(workers);
    }
    http_server.run().await
}
