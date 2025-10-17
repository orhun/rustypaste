//! **Rustypaste** is a minimal file upload/pastebin service.
#![warn(missing_docs, clippy::unwrap_used)]

/// Configuration file parser.
pub mod config;

/// Random URL generator.
pub mod random;

/// Server routes.
pub mod server;

/// HTTP headers.
pub mod header;

/// Auth handler.
pub mod auth;

/// Storage handler.
pub mod paste;

/// File metadata handler.
pub mod file;

/// Media type handler.
pub mod mime;

/// Helper functions.
pub mod util;

// Use macros from tracing crate.
#[macro_use]
extern crate tracing;

/// Environment variable for setting the configuration file path.
pub const CONFIG_ENV: &str = "CONFIG";

/// Environment variable for setting the authentication token.
pub const AUTH_TOKEN_ENV: &str = "AUTH_TOKEN";

/// Environment variable for the path to a file containing multiple authentication token.
pub const AUTH_TOKENS_FILE_ENV: &str = "AUTH_TOKENS_FILE";

/// Environment variable for setting the deletion token.
pub const DELETE_TOKEN_ENV: &str = "DELETE_TOKEN";

/// Environment variable for the path to a file containing multiple deletion token.
pub const DELETE_TOKENS_FILE_ENV: &str = "DELETE_TOKENS_FILE";
