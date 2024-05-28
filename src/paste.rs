use crate::config::Config;
use crate::file::Directory;
use crate::header::ContentDisposition;
use crate::util;
use actix_web::{error, Error};
use awc::Client;
use std::convert::{TryFrom, TryInto};
use std::io::{Error as IoError, ErrorKind as IoErrorKind, Result as IoResult};
use std::path::{Path, PathBuf};
use std::str;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;
use tokio::task::spawn_blocking;
use url::Url;

/// Type of the data to store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteType {
    /// Any type of file.
    File,
    /// A file that is on a remote URL.
    RemoteFile,
    /// A file that allowed to be accessed once.
    Oneshot,
    /// A file that only contains an URL.
    Url,
    /// A oneshot url.
    OneshotUrl,
}

impl<'a> TryFrom<&'a ContentDisposition> for PasteType {
    type Error = ();
    fn try_from(content_disposition: &'a ContentDisposition) -> Result<Self, Self::Error> {
        if content_disposition.has_form_field("file") {
            Ok(Self::File)
        } else if content_disposition.has_form_field("remote") {
            Ok(Self::RemoteFile)
        } else if content_disposition.has_form_field("oneshot") {
            Ok(Self::Oneshot)
        } else if content_disposition.has_form_field("oneshot_url") {
            Ok(Self::OneshotUrl)
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
            Self::File | Self::RemoteFile => String::new(),
            Self::Oneshot => String::from("oneshot"),
            Self::Url => String::from("url"),
            Self::OneshotUrl => String::from("oneshot_url"),
        }
    }

    /// Returns the given path with [`directory`](Self::get_dir) adjoined.
    pub fn get_path(&self, path: &Path) -> IoResult<PathBuf> {
        let dir = self.get_dir();
        if dir.is_empty() {
            Ok(path.to_path_buf())
        } else {
            util::safe_path_join(path, Path::new(&dir))
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
    /// - If `header_filename` is set, it will override the filename.
    ///
    /// [`default_extension`]: crate::config::PasteConfig::default_extension
    /// [`random_url.enabled`]: crate::random::RandomURLConfig::enabled
    pub async fn store_file(
        &self,
        file_name: &str,
        expiry_date: Option<u128>,
        header_filename: Option<String>,
        config: &Config,
    ) -> Result<String, Error> {
        let file_type = infer::get(&self.data);
        if let Some(file_type) = file_type {
            for mime_type in &config.paste.mime_blacklist {
                if mime_type == file_type.mime_type() {
                    return Err(error::ErrorUnsupportedMediaType(
                        "this file type is not permitted",
                    ));
                }
            }
        }
        let mut file_name = match PathBuf::from(file_name)
            .file_name()
            .and_then(|v| v.to_str())
        {
            Some("-") => String::from("stdin"),
            Some(".") => String::from("file"),
            Some(v) => v.to_string(),
            None => String::from("file"),
        };
        if let Some(handle_spaces_config) = config.server.handle_spaces {
            file_name = handle_spaces_config.process_filename(&file_name);
        }

        let mut path =
            util::safe_path_join(self.type_.get_path(&config.server.upload_path)?, &file_name)?;
        let mut parts: Vec<&str> = file_name.split('.').collect();
        let mut dotfile = false;
        let mut lower_bound = 1;
        let mut file_name = match parts[0] {
            "" => {
                // Index shifts one to the right in the array for the rest of the string (the extension)
                dotfile = true;
                lower_bound = 2;
                // If the first array element is empty, it means the file started with a dot (e.g.: .foo)
                format!(".{}", parts[1])
            }
            _ => parts[0].to_string(),
        };
        let mut extension = if parts.len() > lower_bound {
            // To get the rest (the extension), we have to remove the first element of the array, which is the filename
            parts.remove(0);
            if dotfile {
                // If the filename starts with a dot, we have to remove another element, because the first element was empty
                parts.remove(0);
            }
            parts.join(".")
        } else {
            file_type
                .map(|t| t.extension())
                .unwrap_or(&config.paste.default_extension)
                .to_string()
        };
        if let Some(random_url) = &config.paste.random_url {
            if let Some(random_text) = random_url.generate() {
                if let Some(suffix_mode) = random_url.suffix_mode {
                    if suffix_mode {
                        extension = format!("{}.{}", random_text, extension);
                    } else {
                        file_name = random_text;
                    }
                } else {
                    file_name = random_text;
                }
            }
        }
        path.set_file_name(file_name);
        path.set_extension(extension);
        if let Some(header_filename) = header_filename {
            file_name = header_filename;
            path.set_file_name(file_name);
        }
        let file_name = path
            .file_name()
            .map(|v| v.to_string_lossy())
            .unwrap_or_default()
            .to_string();
        let file_path = util::glob_match_file(path.clone())
            .await
            .map_err(|_| IoError::new(IoErrorKind::Other, String::from("path is not valid")))?;
        if file_path.is_file() && file_path.exists() {
            return Err(error::ErrorConflict("file already exists\n"));
        }
        if let Some(timestamp) = expiry_date {
            path.set_file_name(format!("{file_name}.{timestamp}"));
        }
        let mut buffer = File::create(&path).await?;
        buffer.write_all(&self.data).await?;
        Ok(file_name)
    }

    /// Downloads a file from URL and stores it with [`store_file`].
    ///
    /// - File name is inferred from URL if the last URL segment is a file.
    /// - Same content length configuration is applied for download limit.
    /// - Checks SHA256 digest of the downloaded file for preventing duplication.
    /// - Assumes `self.data` contains a valid URL, otherwise returns an error.
    ///
    /// [`store_file`]: Self::store_file
    pub async fn store_remote_file(
        &mut self,
        expiry_date: Option<u128>,
        client: &Client,
        config: &Config,
    ) -> Result<String, Error> {
        let data = str::from_utf8(&self.data).map_err(error::ErrorBadRequest)?;
        let url = Url::parse(data).map_err(error::ErrorBadRequest)?;
        let file_name = url
            .path_segments()
            .and_then(|segments| segments.last())
            .and_then(|name| if name.is_empty() { None } else { Some(name) })
            .unwrap_or("file");
        let mut response = client
            .get(url.as_str())
            .send()
            .await
            .map_err(error::ErrorInternalServerError)?;
        let payload_limit = config
            .server
            .max_content_length
            .try_into()
            .map_err(error::ErrorInternalServerError)?;
        let bytes = response
            .body()
            .limit(payload_limit)
            .await
            .map_err(error::ErrorInternalServerError)?
            .to_vec();
        let bytes_checksum = util::sha256_digest(&*bytes)?;
        self.data = bytes;
        if !config.paste.duplicate_files.unwrap_or(true) && expiry_date.is_none() {
            let upload_path = config.server.upload_path.clone();

            let directory =
                match spawn_blocking(move || Directory::try_from(upload_path.as_path())).await {
                    Ok(Ok(d)) => d,
                    Ok(Err(e)) => return Err(error::ErrorInternalServerError(e)),
                    Err(e) => return Err(error::ErrorInternalServerError(e)),
                };

            if let Some(file) = directory.get_file(bytes_checksum) {
                return Ok(file
                    .path
                    .file_name()
                    .map(|v| v.to_string_lossy())
                    .unwrap_or_default()
                    .to_string());
            }
        }
        self.store_file(file_name, expiry_date, None, config).await
    }

    /// Writes an URL to a file in upload directory.
    ///
    /// - Checks if the data is a valid URL.
    /// - If [`random_url.enabled`] is `true`, file name is set to a pet name or random string.
    ///
    /// [`random_url.enabled`]: crate::random::RandomURLConfig::enabled
    #[allow(deprecated)]
    pub async fn store_url(&self, expiry_date: Option<u128>, config: &Config) -> IoResult<String> {
        let data = str::from_utf8(&self.data)
            .map_err(|e| IoError::new(IoErrorKind::Other, e.to_string()))?;
        let url = Url::parse(data).map_err(|e| IoError::new(IoErrorKind::Other, e.to_string()))?;
        let mut file_name = self.type_.get_dir();
        if let Some(random_url) = &config.paste.random_url {
            if let Some(random_text) = random_url.generate() {
                file_name = random_text;
            }
        }
        let mut path =
            util::safe_path_join(self.type_.get_path(&config.server.upload_path)?, &file_name)?;
        if let Some(timestamp) = expiry_date {
            path.set_file_name(format!("{file_name}.{timestamp}"));
        }
        fs::write(&path, url.to_string()).await?;
        Ok(file_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::{RandomURLConfig, RandomURLType};
    use crate::util;
    use actix_web::web::Data;
    use awc::ClientBuilder;
    use byte_unit::Byte;
    use std::str::FromStr;
    use std::time::Duration;
    use tempfile::tempdir;

    #[actix_rt::test]
    #[allow(deprecated)]
    async fn test_paste() -> Result<(), Error> {
        let temp_upload_path = tempdir()?;
        let mut config = Config::default();
        config.server.upload_path = temp_upload_path.path().to_path_buf();
        config.paste.random_url = Some(RandomURLConfig {
            enabled: Some(true),
            words: Some(3),
            separator: Some(String::from("_")),
            type_: RandomURLType::PetName,
            ..RandomURLConfig::default()
        });
        let paste = Paste {
            data: vec![65, 66, 67],
            type_: PasteType::File,
        };
        let file_name = paste.store_file("test.txt", None, None, &config).await?;
        let file_path = temp_upload_path.path().join(file_name);
        assert_eq!("ABC", fs::read_to_string(&file_path).await?);
        assert_eq!(Some("txt"), file_path.extension().and_then(|v| v.to_str()));

        Ok(())
    }

    #[actix_rt::test]
    #[allow(deprecated)]
    async fn test_paste_random() -> Result<(), Error> {
        let temp_upload_path = tempdir()?;
        let mut config = Config::default();
        config.server.upload_path = temp_upload_path.path().to_path_buf();
        config.paste.random_url = Some(RandomURLConfig {
            length: Some(4),
            type_: RandomURLType::Alphanumeric,
            suffix_mode: Some(true),
            ..RandomURLConfig::default()
        });
        let paste = Paste {
            data: vec![116, 101, 115, 115, 117, 115],
            type_: PasteType::File,
        };
        let file_name = paste.store_file("foo.tar.gz", None, None, &config).await?;
        let file_path = temp_upload_path.path().join(&file_name);
        assert_eq!("tessus", fs::read_to_string(&file_path).await?);
        assert!(file_name.ends_with(".tar.gz"));
        assert!(file_name.starts_with("foo."));

        config.paste.random_url = Some(RandomURLConfig {
            length: Some(4),
            type_: RandomURLType::Alphanumeric,
            suffix_mode: Some(true),
            ..RandomURLConfig::default()
        });
        let paste = Paste {
            data: vec![116, 101, 115, 115, 117, 115],
            type_: PasteType::File,
        };
        let file_name = paste.store_file(".foo.tar.gz", None, None, &config).await?;
        let file_path = temp_upload_path.path().join(&file_name);
        assert_eq!("tessus", fs::read_to_string(&file_path).await?);
        assert!(file_name.ends_with(".tar.gz"));
        assert!(file_name.starts_with(".foo."));

        config.paste.random_url = Some(RandomURLConfig {
            length: Some(4),
            type_: RandomURLType::Alphanumeric,
            suffix_mode: Some(false),
            ..RandomURLConfig::default()
        });
        let paste = Paste {
            data: vec![116, 101, 115, 115, 117, 115],
            type_: PasteType::File,
        };
        let file_name = paste.store_file("foo.tar.gz", None, None, &config).await?;
        let file_path = temp_upload_path.path().join(&file_name);
        assert_eq!("tessus", fs::read_to_string(&file_path).await?);
        assert!(file_name.ends_with(".tar.gz"));

        Ok(())
    }

    #[actix_rt::test]
    #[allow(deprecated)]
    async fn test_paste_with_extension() -> Result<(), Error> {
        let temp_upload_path = tempdir()?;
        let mut config = Config::default();
        config.server.upload_path = temp_upload_path.path().to_path_buf();
        config.paste.default_extension = String::from("txt");
        config.paste.random_url = None;
        let paste = Paste {
            data: vec![120, 121, 122],
            type_: PasteType::File,
        };
        let file_name = paste.store_file(".foo", None, None, &config).await?;
        let file_path = temp_upload_path.path().join(&file_name);
        assert_eq!("xyz", fs::read_to_string(&file_path).await?);
        assert_eq!(".foo.txt", file_name);

        config.paste.default_extension = String::from("bin");
        config.paste.random_url = Some(RandomURLConfig {
            length: Some(10),
            type_: RandomURLType::Alphanumeric,
            ..RandomURLConfig::default()
        });
        let paste = Paste {
            data: vec![120, 121, 122],
            type_: PasteType::File,
        };
        let file_name = paste.store_file("random", None, None, &config).await?;
        let file_path = temp_upload_path.path().join(&file_name);
        assert_eq!(Some("bin"), file_path.extension().and_then(|v| v.to_str()));
        assert_eq!("xyz", fs::read_to_string(&file_path).await?);

        Ok(())
    }

    #[actix_rt::test]
    #[allow(deprecated)]
    async fn test_paste_filename_from_header() -> Result<(), Error> {
        let temp_upload_path = tempdir()?;
        let mut config = Config::default();
        config.server.upload_path = temp_upload_path.path().to_path_buf();
        config.paste.random_url = Some(RandomURLConfig {
            length: Some(4),
            type_: RandomURLType::Alphanumeric,
            suffix_mode: Some(true),
            ..RandomURLConfig::default()
        });
        let paste = Paste {
            data: vec![116, 101, 115, 115, 117, 115],
            type_: PasteType::File,
        };
        let file_name = paste
            .store_file(
                "filename.txt",
                None,
                Some("fn_from_header.txt".to_string()),
                &config,
            )
            .await?;
        assert_eq!("fn_from_header.txt", file_name);
        let file_path = temp_upload_path.path().join(&file_name);
        assert_eq!("tessus", fs::read_to_string(&file_path).await?);

        config.paste.random_url = Some(RandomURLConfig {
            length: Some(4),
            type_: RandomURLType::Alphanumeric,
            suffix_mode: Some(true),
            ..RandomURLConfig::default()
        });
        let paste = Paste {
            data: vec![116, 101, 115, 115, 117, 115],
            type_: PasteType::File,
        };
        let file_name = paste
            .store_file(
                "filename.txt",
                None,
                Some("fn_from_header".to_string()),
                &config,
            )
            .await?;
        let file_path = temp_upload_path.path().join(&file_name);
        assert_eq!("tessus", fs::read_to_string(&file_path).await?);
        assert_eq!("fn_from_header", file_name);

        Ok(())
    }

    #[actix_rt::test]
    #[allow(deprecated)]
    async fn test_paste_oneshot() -> Result<(), Error> {
        let mut config = Config::default();
        config.server.upload_path = tempdir()?.path().to_path_buf();
        config.paste.random_url = None;

        fs::create_dir_all(
            PasteType::Oneshot
                .get_path(&config.server.upload_path)
                .expect("Bad upload path"),
        )
        .await?;

        let paste = Paste {
            data: vec![116, 101, 115, 116],
            type_: PasteType::Oneshot,
        };
        let expiry_date = util::get_system_time()?.as_millis() + 100;
        let file_name = paste
            .store_file("test.file", Some(expiry_date), None, &config)
            .await?;
        let file_path = PasteType::Oneshot
            .get_path(&config.server.upload_path)
            .expect("Bad upload path")
            .join(format!("{file_name}.{expiry_date}"));
        assert_eq!("test", fs::read_to_string(&file_path).await?);
        fs::remove_file(file_path).await?;

        Ok(())
    }

    #[actix_rt::test]
    #[allow(deprecated)]
    async fn test_paste_url() -> Result<(), Error> {
        let mut config = Config::default();
        config.server.upload_path = tempdir()?.path().to_path_buf();
        config.paste.random_url = Some(RandomURLConfig {
            enabled: Some(true),
            ..RandomURLConfig::default()
        });

        fs::create_dir_all(
            PasteType::Url
                .get_path(&config.server.upload_path)
                .expect("Bad upload path"),
        )
        .await?;

        let url = String::from("https://orhun.dev/");
        let paste = Paste {
            data: url.as_bytes().to_vec(),
            type_: PasteType::Url,
        };
        let file_name = paste.store_url(None, &config).await?;
        let file_path = PasteType::Url
            .get_path(&config.server.upload_path)
            .expect("Bad upload path")
            .join(&file_name);
        assert_eq!(url, fs::read_to_string(&file_path).await?);
        fs::remove_file(file_path).await?;

        let url = String::from("testurl.com");
        let paste = Paste {
            data: url.as_bytes().to_vec(),
            type_: PasteType::Url,
        };
        assert!(paste.store_url(None, &config).await.is_err());

        Ok(())
    }

    #[actix_rt::test]
    #[allow(deprecated)]
    async fn test_paste_remote_url() -> Result<(), Error> {
        let mut config = Config::default();
        config.server.upload_path = tempdir()?.path().to_path_buf();
        config.server.max_content_length = Byte::from_str("30k").expect("cannot parse byte");

        fs::create_dir_all(
            PasteType::Url
                .get_path(&config.server.upload_path)
                .expect("Bad upload path"),
        )
        .await?;

        let url = String::from("https://upload.wikimedia.org/wikipedia/en/a/a9/Example.jpg");
        let mut paste = Paste {
            data: url.as_bytes().to_vec(),
            type_: PasteType::RemoteFile,
        };
        let client_data = Data::new(
            ClientBuilder::new()
                .timeout(Duration::from_secs(30))
                .finish(),
        );
        let _ = paste.store_remote_file(None, &client_data, &config).await?;

        assert_eq!(
            "70ff72a2f7651b5fae3aa9834e03d2a2233c52036610562f7fa04e089e8198ed",
            util::sha256_digest(&*paste.data)?
        );

        Ok(())
    }
}
