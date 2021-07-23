use crate::config::Config;
use rand::{distributions::Alphanumeric, Rng};
use std::fs::File;
use std::io::{Result as IoResult, Write};

/// Writes the bytes to a file in upload directory.
///
/// - If `file_name` does not have an extension, it is replaced with [`default_extension`].
/// - If `file_name` is "-", it is replaced with "stdin".
/// - If [`pet_names.enabled`] is `true`, `file_name` is replaced with a pet name.
/// - If [`random.enabled`] is `true`, `file_name` is replaced with a random string.
///
/// [`default_extension`]: crate::config::PasteConfig::default_extension
/// [`pet_names.enabled`]: crate::config::PetNamesConfig::enabled
/// [`random.enabled`]: crate::config::RandomConfig::enabled
pub fn save(mut file_name: &str, bytes: &[u8], config: &Config) -> IoResult<String> {
    if file_name == "-" {
        file_name = "stdin";
    }
    let mut path = config.server.upload_path.join(file_name);
    match path.clone().extension() {
        Some(extension) => {
            if config.paste.pet_names.enabled {
                path.set_file_name(petname::petname(
                    config.paste.pet_names.words,
                    &config.paste.pet_names.separator,
                ));
                path.set_extension(extension);
            } else if config.paste.random.enabled {
                path.set_file_name(
                    rand::thread_rng()
                        .sample_iter(&Alphanumeric)
                        .take(config.paste.random.length)
                        .map(char::from)
                        .collect::<String>(),
                );
                path.set_extension(extension);
            }
        }
        None => {
            if config.paste.pet_names.enabled {
                path.set_file_name(petname::petname(
                    config.paste.pet_names.words,
                    &config.paste.pet_names.separator,
                ));
            } else if config.paste.random.enabled {
                path.set_file_name(
                    rand::thread_rng()
                        .sample_iter(&Alphanumeric)
                        .take(config.paste.random.length)
                        .map(char::from)
                        .collect::<String>(),
                );
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
