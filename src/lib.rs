//! oops is a file upload/pastebin service.
#![warn(missing_docs, clippy::unwrap_used)]

/// Configuration file parser.
pub mod config;

/// Server routes.
pub mod server;

/// File handler.
pub mod file;

/// HTTP headers.
pub mod header;
