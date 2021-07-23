use crate::config::Config;
use std::fs::File;
use std::io::{Result as IoResult, Write};

/// Writes the bytes to a file in upload directory.
///
/// - If `file_name` does not have an extension, it is replaced with [`default_extension`].
/// - If `file_name` is "-", it is replaced with "stdin".
/// - If [`pet_names`] is `true`, `file_name` is replaced with a pet name.
///
/// [`default_extension`]: crate::config::PasteConfig::default_extension
/// [`pet_names`]: crate::config::PasteConfig::pet_names
pub fn save(mut file_name: &str, bytes: &[u8], config: &Config) -> IoResult<String> {
    if file_name == "-" {
        file_name = "stdin";
    }
    let mut path = config.server.upload_path.join(file_name);
    match path.clone().extension() {
        Some(extension) => {
            if config.paste.pet_names {
                path.set_file_name(petname::petname(2, "-"));
                path.set_extension(extension);
            }
        }
        None => {
            if config.paste.pet_names {
                path.set_file_name(petname::petname(2, "-"));
            }
            path.set_extension(&config.paste.default_extension);
        }
    }
    let mut buffer = File::create(&path)?;
    buffer.write_all(bytes)?;
    Ok(path
        .file_name()
        .map(|v| v.to_string_lossy())
        .unwrap_or_default()
        .to_string())
}
