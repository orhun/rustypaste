use crate::config::Config;
use crate::file::Directory;
use crate::header::ContentDisposition;
use crate::util;
use actix_web::{error, Error};
use awc::Client;
use std::convert::{TryFrom, TryInto};
use std::fs::{self, File};
use std::io::{Error as IoError, ErrorKind as IoErrorKind, Result as IoResult, Write};
use std::path::{Path, PathBuf};
use std::str;
use std::sync::RwLock;
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
    pub fn store_file(
        &self,
        file_name: &str,
        expiry_date: Option<u128>,
        config: &Config,
    ) -> IoResult<String> {
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
            .and_then(|v| v.to_str())
        {
            Some("-") => String::from("stdin"),
            Some(v) => v.to_string(),
            None => String::from("file"),
        };
        let mut input = file_name.to_string();

        let mut path = self
            .type_
            .get_path(&config.server.upload_path)
            .join(file_name);

        // set_file_name does not accept names that contain a . unless it is the first character (e.g. "file.name" not ok, ".file" ok)
        // as a workaround we use set_extension, because it can set multiple extensions (e.g. "tar.gz")
        // never accept a filename "." - similar to above where an empty string is replaced by file, and - by stdin
        if input == "." {
            input = "file".to_string();
        }
        // Split the string into an array
        let mut v: Vec<&str> = input.split('.').collect();
        let mut filename;
        let mut dotfile = false;
        if !v[0].is_empty() {
            filename = v[0].to_string();
        } else {
            // If the first array element is empty, it means the file started with a dot (e.g.: .foo)
            filename = format!(".{}", v[1]);
            // Index shifts one to the right in the array for the rest of the string (the extension)
            dotfile = true;
        }
        let mut extension;
        if v.len() > 1 {
            // To get the rest (the extension), we have to remove the first element of the array, which is the filename
            v.remove(0);
            if dotfile {
                // If the filename starts with a dot, we have to remove another element, because the first element was empty
                v.remove(0);
            }
            extension = v.join(".");
        } else {
            extension = file_type
                .map(|t| t.extension())
                .unwrap_or(&config.paste.default_extension)
                .to_string()
        }

        if let Some(random_text) = config.paste.random_url.generate() {
            if let Some(suffix_mode) = config.paste.random_url.suffix_mode {
                if suffix_mode {
                    extension = format!("{}.{}", random_text, extension);
                } else {
                    filename = random_text;
                }
            } else {
                filename = random_text;
            }
        }

        path.set_file_name(filename);
        path.set_extension(extension);

        let file_name = path
            .file_name()
            .map(|v| v.to_string_lossy())
            .unwrap_or_default()
            .to_string();
        if let Some(timestamp) = expiry_date {
            path.set_file_name(format!("{file_name}.{timestamp}"));
        }
        let mut buffer = File::create(&path)?;
        buffer.write_all(&self.data)?;
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
        config: &RwLock<Config>,
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
            .read()
            .map_err(|_| error::ErrorInternalServerError("cannot acquire config"))?
            .server
            .max_content_length
            .get_bytes()
            .try_into()
            .map_err(error::ErrorInternalServerError)?;
        let bytes = response
            .body()
            .limit(payload_limit)
            .await
            .map_err(error::ErrorInternalServerError)?
            .to_vec();
        let config = config
            .read()
            .map_err(|_| error::ErrorInternalServerError("cannot acquire config"))?;
        let bytes_checksum = util::sha256_digest(&*bytes)?;
        self.data = bytes;
        if !config.paste.duplicate_files.unwrap_or(true) && expiry_date.is_none() {
            if let Some(file) =
                Directory::try_from(config.server.upload_path.as_path())?.get_file(bytes_checksum)
            {
                return Ok(file
                    .path
                    .file_name()
                    .map(|v| v.to_string_lossy())
                    .unwrap_or_default()
                    .to_string());
            }
        }
        Ok(self.store_file(file_name, expiry_date, &config)?)
    }

    /// Writes an URL to a file in upload directory.
    ///
    /// - Checks if the data is a valid URL.
    /// - If [`random_url.enabled`] is `true`, file name is set to a pet name or random string.
    ///
    /// [`random_url.enabled`]: crate::random::RandomURLConfig::enabled
    pub fn store_url(&self, expiry_date: Option<u128>, config: &Config) -> IoResult<String> {
        let data = str::from_utf8(&self.data)
            .map_err(|e| IoError::new(IoErrorKind::Other, e.to_string()))?;
        let url = Url::parse(data).map_err(|e| IoError::new(IoErrorKind::Other, e.to_string()))?;
        let file_name = config
            .paste
            .random_url
            .generate()
            .unwrap_or_else(|| self.type_.get_dir());
        let mut path = self
            .type_
            .get_path(&config.server.upload_path)
            .join(&file_name);
        if let Some(timestamp) = expiry_date {
            path.set_file_name(format!("{file_name}.{timestamp}"));
        }
        fs::write(&path, url.to_string())?;
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
    use std::env;
    use std::time::Duration;

    #[actix_rt::test]
    async fn test_paste_data() -> Result<(), Error> {
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
        let file_name = paste.store_file("test.txt", None, &config)?;
        assert_eq!("ABC", fs::read_to_string(&file_name)?);
        assert_eq!(
            Some("txt"),
            PathBuf::from(&file_name)
                .extension()
                .and_then(|v| v.to_str())
        );
        fs::remove_file(file_name)?;

        config.paste.random_url = RandomURLConfig {
            enabled: true,
            length: Some(4),
            type_: RandomURLType::Alphanumeric,
            suffix_mode: Some(true),
            ..RandomURLConfig::default()
        };
        let paste = Paste {
            data: vec![116, 101, 115, 115, 117, 115],
            type_: PasteType::File,
        };
        let file_name = paste.store_file("foo.tar.gz", None, &config)?;
        assert_eq!("tessus", fs::read_to_string(&file_name)?);
        assert!(file_name.ends_with(".tar.gz"));
        assert!(file_name.starts_with("foo."));
        fs::remove_file(file_name)?;

        config.paste.random_url = RandomURLConfig {
            enabled: true,
            length: Some(4),
            type_: RandomURLType::Alphanumeric,
            suffix_mode: Some(true),
            ..RandomURLConfig::default()
        };
        let paste = Paste {
            data: vec![116, 101, 115, 115, 117, 115],
            type_: PasteType::File,
        };
        let file_name = paste.store_file(".foo.tar.gz", None, &config)?;
        assert_eq!("tessus", fs::read_to_string(&file_name)?);
        assert!(file_name.ends_with(".tar.gz"));
        assert!(file_name.starts_with(".foo."));
        fs::remove_file(file_name)?;

        config.paste.random_url = RandomURLConfig {
            enabled: true,
            length: Some(4),
            type_: RandomURLType::Alphanumeric,
            suffix_mode: Some(false),
            ..RandomURLConfig::default()
        };
        let paste = Paste {
            data: vec![116, 101, 115, 115, 117, 115],
            type_: PasteType::File,
        };
        let file_name = paste.store_file("foo.tar.gz", None, &config)?;
        assert_eq!("tessus", fs::read_to_string(&file_name)?);
        assert!(file_name.ends_with(".tar.gz"));
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
        let file_name = paste.store_file("random", None, &config)?;
        assert_eq!("xyz", fs::read_to_string(&file_name)?);
        assert_eq!(
            Some("bin"),
            PathBuf::from(&file_name)
                .extension()
                .and_then(|v| v.to_str())
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
        let expiry_date = util::get_system_time()?.as_millis() + 100;
        let file_name = paste.store_file("test.file", Some(expiry_date), &config)?;
        let file_path = PasteType::Oneshot
            .get_path(&config.server.upload_path)
            .join(format!("{file_name}.{expiry_date}"));
        assert_eq!("test", fs::read_to_string(&file_path)?);
        fs::remove_file(file_path)?;

        config.paste.random_url.enabled = true;
        let url = String::from("https://orhun.dev/");
        let paste = Paste {
            data: url.as_bytes().to_vec(),
            type_: PasteType::Url,
        };
        let file_name = paste.store_url(None, &config)?;
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
        assert!(paste.store_url(None, &config).is_err());

        config.server.max_content_length = Byte::from_str("30k").expect("cannot parse byte");
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
        let file_name = paste
            .store_remote_file(None, &client_data, &RwLock::new(config.clone()))
            .await?;
        let file_path = PasteType::RemoteFile
            .get_path(&config.server.upload_path)
            .join(file_name);
        assert_eq!(
            "8c712905b799905357b8202d0cb7a244cefeeccf7aa5eb79896645ac50158ffa",
            util::sha256_digest(&*paste.data)?
        );
        fs::remove_file(file_path)?;

        for paste_type in &[PasteType::Url, PasteType::Oneshot] {
            fs::remove_dir(paste_type.get_path(&config.server.upload_path))?;
        }

        Ok(())
    }
}
