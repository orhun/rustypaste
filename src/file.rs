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
            path.set_extension(
                infer::get(bytes)
                    .map(|t| t.extension())
                    .unwrap_or(&config.paste.default_extension),
            );
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::config::{PetNamesConfig, RandomConfig};
    use std::env;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn test_save_file() -> IoResult<()> {
        let mut config = Config::default();
        config.server.upload_path = env::current_dir()?;
        config.paste.pet_names = PetNamesConfig {
            enabled: true,
            words: 3,
            separator: String::from("_"),
        };
        let file_name = save("test.txt", &[65, 66, 67], &config)?;
        assert_eq!("ABC", fs::read_to_string(&file_name)?);
        assert_eq!(
            Some("txt"),
            PathBuf::from(&file_name)
                .extension()
                .map(|v| v.to_str())
                .flatten()
        );
        fs::remove_file(file_name)?;

        config.paste.default_extension = String::from("bin");
        config.paste.pet_names.enabled = false;
        config.paste.random = RandomConfig {
            enabled: true,
            length: 10,
        };
        let file_name = save("random", &[120, 121, 122], &config)?;
        assert_eq!("xyz", fs::read_to_string(&file_name)?);
        assert_eq!(
            Some("bin"),
            PathBuf::from(&file_name)
                .extension()
                .map(|v| v.to_str())
                .flatten()
        );
        fs::remove_file(file_name)?;

        config.paste.random.enabled = false;
        let file_name = save("test.file", &[116, 101, 115, 116], &config)?;
        assert_eq!("test.file", &file_name);
        assert_eq!("test", fs::read_to_string(&file_name)?);
        fs::remove_file(file_name)?;

        Ok(())
    }
}
