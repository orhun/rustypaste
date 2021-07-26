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

/// File handler.
pub mod file;

/// Auth handler.
pub mod auth;
