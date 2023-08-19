use crate::mime::MimeMatcher;
use crate::random::RandomURLConfig;
use crate::AUTH_TOKEN_ENV;
use byte_unit::Byte;
use config::{self, ConfigError};
use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Configuration values.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Config {
    /// Configuration settings.
    #[serde(rename = "config")]
    pub settings: Option<Settings>,
    /// Server configuration.
    pub server: ServerConfig,
    /// Paste configuration.
    pub paste: PasteConfig,
    /// Landing page configuration.
    pub landing_page: Option<LandingPageConfig>,
}

/// General settings for configuration.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    /// Refresh rate of the configuration file.
    #[serde(with = "humantime_serde")]
    pub refresh_rate: Duration,
}

/// Server configuration.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ServerConfig {
    /// The socket address to bind.
    pub address: String,
    /// URL that can be used to access the server externally.
    pub url: Option<String>,
    /// Number of workers to start.
    pub workers: Option<usize>,
    /// Maximum content length.
    pub max_content_length: Byte,
    /// Storage path.
    pub upload_path: PathBuf,
    /// Request timeout.
    #[serde(default, with = "humantime_serde")]
    pub timeout: Option<Duration>,
    /// Authentication token.
    #[deprecated(note = "use [server].auth_tokens instead")]
    pub auth_token: Option<String>,
    /// Authentication tokens.
    pub auth_tokens: Option<Vec<String>>,
    /// Expose version.
    pub expose_version: Option<bool>,
    /// Landing page text.
    #[deprecated(note = "use the [landing_page] table")]
    pub landing_page: Option<String>,
    /// Landing page content-type.
    #[deprecated(note = "use the [landing_page] table")]
    pub landing_page_content_type: Option<String>,
    /// Path of the JSON index.
    pub expose_list: Option<bool>,
}

/// Landing page configuration.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct LandingPageConfig {
    /// Landing page text.
    pub text: Option<String>,
    /// Landing page file.
    pub file: Option<String>,
    /// Landing page content-type
    pub content_type: Option<String>,
}

/// Paste configuration.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PasteConfig {
    /// Random URL configuration.
    pub random_url: Option<RandomURLConfig>,
    /// Default file extension.
    pub default_extension: String,
    /// Media type override options.
    #[serde(default)]
    pub mime_override: Vec<MimeMatcher>,
    /// Media type blacklist.
    #[serde(default)]
    pub mime_blacklist: Vec<String>,
    /// Allow duplicate uploads.
    pub duplicate_files: Option<bool>,
    /// Default expiry time.
    #[serde(default, with = "humantime_serde")]
    pub default_expiry: Option<Duration>,
    /// Delete expired files.
    pub delete_expired_files: Option<CleanupConfig>,
}

/// Cleanup configuration.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CleanupConfig {
    /// Enable cleaning up.
    pub enabled: bool,
    /// Interval between clean-ups.
    #[serde(default, with = "humantime_serde")]
    pub interval: Duration,
}

impl Config {
    /// Parses the config file and returns the values.
    pub fn parse(path: &Path) -> Result<Config, ConfigError> {
        config::Config::builder()
            .add_source(config::File::from(path))
            .add_source(config::Environment::default().separator("__"))
            .build()?
            .try_deserialize()
    }

    /// Retrieves all configured tokens.
    #[allow(deprecated)]
    pub fn get_tokens(&self) -> Option<Vec<String>> {
        let mut tokens = self.server.auth_tokens.clone().unwrap_or_default();
        if let Some(token) = &self.server.auth_token {
            tokens.insert(0, token.to_string());
        }
        if let Ok(env_token) = env::var(AUTH_TOKEN_ENV) {
            tokens.insert(0, env_token);
        }
        (!tokens.is_empty()).then_some(tokens)
    }

    /// Print deprecation warnings.
    #[allow(deprecated)]
    pub fn warn_deprecation(&self) {
        if self.server.auth_token.is_some() {
            log::warn!("[server].auth_token is deprecated, please use [server].auth_tokens");
        }
        if self.server.landing_page.is_some() {
            log::warn!("[server].landing_page is deprecated, please use [landing_page].text");
        }
        if self.server.landing_page_content_type.is_some() {
            log::warn!(
                "[server].landing_page_content_type is deprecated, please use [landing_page].content_type"
            );
        }
        if let Some(random_url) = &self.paste.random_url {
            if random_url.enabled.is_some() {
                log::warn!(
                    "[paste].random_url.enabled is deprecated, to disable comment out [paste].random_url"
                );
            }
        }
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
