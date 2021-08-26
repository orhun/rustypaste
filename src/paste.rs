use crate::config::Config;
use crate::header::ContentDisposition;
use std::convert::TryFrom;
use std::fs::{self, File};
use std::io::{Error as IoError, ErrorKind as IoErrorKind, Result as IoResult, Write};
use std::path::{Path, PathBuf};
use std::str;
use url::Url;

/// Type of the data to store.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PasteType {
    /// Any type of file.
    File,
    /// A file that allowed to be accessed once.
    Oneshot,
    /// A file that only contains an URL.
    Url,
    /// A file that is expired or deleted.
    Trash,
}

impl<'a> TryFrom<&'a ContentDisposition> for PasteType {
    type Error = ();
    fn try_from(content_disposition: &'a ContentDisposition) -> Result<Self, Self::Error> {
        if content_disposition.has_form_field("file") {
            Ok(Self::File)
        } else if content_disposition.has_form_field("oneshot") {
            Ok(Self::Oneshot)
        } else if content_disposition.has_form_field("url") {
            Ok(Self::Url)
        } else {
            Err(())
        }
    }
}

impl PasteType {
    /// Returns the corresponding directory of the paste type.
    pub fn get_dir(&self) -> String {
        match self {
            Self::File => String::new(),
            Self::Oneshot => String::from("oneshot"),
            Self::Url => String::from("url"),
            Self::Trash => String::from("trash"),
        }
    }

    /// Returns the given path with [`directory`](Self::get_dir) adjoined.
    pub fn get_path(&self, path: &Path) -> PathBuf {
        let dir = self.get_dir();
        if dir.is_empty() {
            path.to_path_buf()
        } else {
            path.join(dir)
        }
    }

    /// Returns `true` if the variant is [`Oneshot`](Self::Oneshot).
    pub fn is_oneshot(&self) -> bool {
        self == &Self::Oneshot
    }
}

/// Representation of a single paste.
#[derive(Debug)]
pub struct Paste {
    /// Data to store.
    pub data: Vec<u8>,
    /// Type of the data.
    pub type_: PasteType,
}

impl Paste {
    /// Writes the bytes to a file in upload directory.
    ///
    /// - If `file_name` does not have an extension, it is replaced with [`default_extension`].
    /// - If `file_name` is "-", it is replaced with "stdin".
    /// - If [`random_url.enabled`] is `true`, `file_name` is replaced with a pet name or random string.
    ///
    /// [`default_extension`]: crate::config::PasteConfig::default_extension
    /// [`random_url.enabled`]: crate::random::RandomURLConfig::enabled
    pub fn store_file(&self, file_name: &str, config: &Config) -> IoResult<String> {
        let file_type = infer::get(&self.data);
        if let Some(file_type) = file_type {
            for mime_type in &config.paste.mime_blacklist {
                if mime_type == file_type.mime_type() {
                    return Err(IoError::new(
                        IoErrorKind::Other,
                        String::from("this file type is not permitted"),
                    ));
                }
            }
        }
        let file_name = match PathBuf::from(file_name)
            .file_name()
            .map(|v| v.to_str())
            .flatten()
        {
            Some("-") => String::from("stdin"),
            Some(v) => v.to_string(),
            None => String::from("file"),
        };
        let mut path = self
            .type_
            .get_path(&config.server.upload_path)
            .join(file_name);
        match path.clone().extension() {
            Some(extension) => {
                if let Some(file_name) = config.paste.random_url.generate() {
                    path.set_file_name(file_name);
                    path.set_extension(extension);
                }
            }
            None => {
                if let Some(file_name) = config.paste.random_url.generate() {
                    path.set_file_name(file_name);
                }
                path.set_extension(
                    file_type
                        .map(|t| t.extension())
                        .unwrap_or(&config.paste.default_extension),
                );
            }
        }
        let mut buffer = File::create(&path)?;
        buffer.write_all(&self.data)?;
        Ok(path
            .file_name()
            .map(|v| v.to_string_lossy())
            .unwrap_or_default()
            .to_string())
    }

    /// Writes an URL to a file in upload directory.
    ///
    /// - Checks if the data is a valid URL.
    /// - If [`random_url.enabled`] is `true`, file name is set to a pet name or random string.
    ///
    /// [`random_url.enabled`]: crate::random::RandomURLConfig::enabled
    pub fn store_url(&self, config: &Config) -> IoResult<String> {
        let data = str::from_utf8(&self.data)
            .map_err(|e| IoError::new(IoErrorKind::Other, e.to_string()))?;
        let url = Url::parse(data).map_err(|e| IoError::new(IoErrorKind::Other, e.to_string()))?;
        let file_name = config
            .paste
            .random_url
            .generate()
            .unwrap_or_else(|| PasteType::Url.get_dir());
        let path = PasteType::Url
            .get_path(&config.server.upload_path)
            .join(&file_name);
        fs::write(&path, url.to_string())?;
        Ok(file_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::{RandomURLConfig, RandomURLType};
    use std::env;

    #[test]
    fn test_paste_data() -> IoResult<()> {
        let mut config = Config::default();
        config.server.upload_path = env::current_dir()?;
        config.paste.random_url = RandomURLConfig {
            enabled: true,
            words: Some(3),
            separator: Some(String::from("_")),
            type_: RandomURLType::PetName,
            ..RandomURLConfig::default()
        };
        let paste = Paste {
            data: vec![65, 66, 67],
            type_: PasteType::File,
        };
        let file_name = paste.store_file("test.txt", &config)?;
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
        config.paste.random_url.enabled = false;
        config.paste.random_url = RandomURLConfig {
            enabled: true,
            length: Some(10),
            type_: RandomURLType::Alphanumeric,
            ..RandomURLConfig::default()
        };
        let paste = Paste {
            data: vec![120, 121, 122],
            type_: PasteType::File,
        };
        let file_name = paste.store_file("random", &config)?;
        assert_eq!("xyz", fs::read_to_string(&file_name)?);
        assert_eq!(
            Some("bin"),
            PathBuf::from(&file_name)
                .extension()
                .map(|v| v.to_str())
                .flatten()
        );
        fs::remove_file(file_name)?;

        for paste_type in &[PasteType::Url, PasteType::Oneshot] {
            fs::create_dir_all(paste_type.get_path(&config.server.upload_path))?;
        }

        config.paste.random_url.enabled = false;
        let paste = Paste {
            data: vec![116, 101, 115, 116],
            type_: PasteType::Oneshot,
        };
        let file_name = paste.store_file("test.file", &config)?;
        let file_path = PasteType::Oneshot
            .get_path(&config.server.upload_path)
            .join(&file_name);
        assert_eq!("test.file", &file_name);
        assert_eq!("test", fs::read_to_string(&file_path)?);
        fs::remove_file(file_path)?;

        config.paste.random_url.enabled = true;
        let url = String::from("https://orhun.dev/");
        let paste = Paste {
            data: url.as_bytes().to_vec(),
            type_: PasteType::Url,
        };
        let file_name = paste.store_url(&config)?;
        let file_path = PasteType::Url
            .get_path(&config.server.upload_path)
            .join(&file_name);
        assert_eq!(url, fs::read_to_string(&file_path)?);
        fs::remove_file(file_path)?;

        let url = String::from("testurl.com");
        let paste = Paste {
            data: url.as_bytes().to_vec(),
            type_: PasteType::Url,
        };
        assert!(paste.store_url(&config).is_err());

        for paste_type in &[PasteType::Url, PasteType::Oneshot] {
            fs::remove_dir(paste_type.get_path(&config.server.upload_path))?;
        }

        Ok(())
    }
}
