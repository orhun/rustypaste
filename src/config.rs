use crate::mime::MimeMatcher;
use crate::random::RandomURLConfig;
use byte_unit::Byte;
use config::{self, ConfigError};
use std::path::{Path, PathBuf};

/// Configuration values.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Config {
    /// Server configuration.
    pub server: ServerConfig,
    /// Paste configuration.
    pub paste: PasteConfig,
}

/// Server configuration.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ServerConfig {
    /// The socket address to bind.
    pub address: String,
    /// Number of workers to start.
    pub workers: Option<usize>,
    /// Maximum content length.
    pub max_content_length: Byte,
    /// Storage path.
    pub upload_path: PathBuf,
}

/// Paste configuration.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PasteConfig {
    /// Random URL configuration.
    pub random_url: RandomURLConfig,
    /// Default file extension.
    pub default_extension: String,
    /// Media type override options.
    pub mime_override: Vec<MimeMatcher>,
    /// Media type blacklist.
    pub mime_blacklist: Vec<String>,
    /// Allow duplicate uploads
    pub duplicate_files: Option<bool>,
}

impl Config {
    /// Parses the config file and returns the values.
    pub fn parse(path: &Path) -> Result<Config, ConfigError> {
        let mut config = config::Config::default();
        config
            .merge(config::File::from(path))?
            .merge(config::Environment::new().separator("__"))?;
        config.try_into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_parse_config() -> Result<(), ConfigError> {
        let config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config.toml");
        env::set_var("SERVER__ADDRESS", "0.0.1.1");
        let config = Config::parse(&config_path)?;
        assert_eq!("0.0.1.1", config.server.address);
        Ok(())
    }
}
