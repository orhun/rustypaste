use crate::paste::PasteType;
use actix_web::{error, Error as ActixError};
use glob::glob;
use lazy_regex::{lazy_regex, Lazy, Regex};
use path_clean::PathClean;
use ring::digest::{Context, SHA256};
use std::fmt::Write;
use std::io::{BufReader, Read};
use std::io::{Error as IoError, ErrorKind as IoErrorKind, Result as IoResult};
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::task::spawn_blocking;

/// Regex for matching the timestamp extension of a path.
pub static TIMESTAMP_EXTENSION_REGEX: Lazy<Regex> = lazy_regex!(r#"\.[0-9]{10,}$"#);

/// Returns the system time as [`Duration`](Duration).
pub fn get_system_time() -> Result<Duration, ActixError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(error::ErrorInternalServerError)
}

/// Returns the first _unexpired_ path matched by a custom glob pattern.
///
/// The file extension is accepted as a timestamp that points to the expiry date.
pub async fn glob_match_file(mut path: PathBuf) -> Result<PathBuf, ActixError> {
    path = PathBuf::from(
        TIMESTAMP_EXTENSION_REGEX
            .replacen(
                path.to_str().ok_or_else(|| {
                    error::ErrorInternalServerError("path contains invalid characters")
                })?,
                1,
                "",
            )
            .to_string(),
    );

    let path_string = path.to_string_lossy().into_owned();
    let glob_match = match spawn_blocking(move || glob(&format!("{}.[0-9]*", path_string))).await {
        Ok(Ok(m)) => m.last(),
        Ok(Err(e)) => return Err(error::ErrorInternalServerError(e)),
        Err(e) => return Err(error::ErrorInternalServerError(e)),
    };

    if let Some(glob_path) = glob_match {
        let glob_path = glob_path.map_err(error::ErrorInternalServerError)?;
        if let Some(extension) = glob_path
            .extension()
            .and_then(|v| v.to_str())
            .and_then(|v| v.parse().ok())
        {
            if get_system_time()? < Duration::from_millis(extension) {
                path = glob_path;
            }
        }
    }
    Ok(path)
}

/// Returns the found expired files in the possible upload locations.
///
/// Fail-safe, omits errors.
pub fn get_expired_files(base_path: &Path) -> Vec<PathBuf> {
    [
        PasteType::File,
        PasteType::Oneshot,
        PasteType::Url,
        PasteType::OneshotUrl,
    ]
    .into_iter()
    .filter_map(|v| v.get_path(base_path).ok())
    .filter_map(|v| glob(&v.join("*.[0-9]*").to_string_lossy()).ok())
    .flat_map(|glob| glob.filter_map(|v| v.ok()).collect::<Vec<PathBuf>>())
    .filter(|path| {
        if let Some(extension) = path
            .extension()
            .and_then(|v| v.to_str())
            .and_then(|v| v.parse().ok())
        {
            get_system_time()
                .map(|system_time| system_time > Duration::from_millis(extension))
                .unwrap_or(false)
        } else {
            false
        }
    })
    .collect()
}

/// Returns the SHA256 digest of the given input.
pub fn sha256_digest<R: Read>(input: R) -> Result<String, ActixError> {
    let mut reader = BufReader::new(input);
    let mut context = Context::new(&SHA256);
    let mut buffer = [0; 1024];
    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read != 0 {
            context.update(&buffer[..bytes_read]);
        } else {
            break;
        }
    }
    Ok(context
        .finish()
        .as_ref()
        .iter()
        .collect::<Vec<&u8>>()
        .iter()
        .try_fold::<String, _, IoResult<String>>(String::new(), |mut output, b| {
            write!(output, "{b:02x}")
                .map_err(|e| IoError::new(IoErrorKind::Other, e.to_string()))?;
            Ok(output)
        })?)
}

/// Joins the paths whilst ensuring the path doesn't drastically change.
/// `base` is assumed to be a trusted value.
pub fn safe_path_join<B: AsRef<Path>, P: AsRef<Path>>(base: B, part: P) -> IoResult<PathBuf> {
    let new_path = base.as_ref().join(part).clean();

    let cleaned_base = base.as_ref().clean();

    if !new_path.starts_with(cleaned_base) {
        return Err(IoError::new(
            IoErrorKind::InvalidData,
            format!(
                "{} is outside of {}",
                new_path.display(),
                base.as_ref().display()
            ),
        ));
    }

    Ok(new_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::thread;
    use tempfile::tempdir;

    #[test]
    fn test_system_time() -> Result<(), ActixError> {
        let system_time = get_system_time()?.as_millis();
        thread::sleep(Duration::from_millis(1));
        assert!(system_time < get_system_time()?.as_millis());
        Ok(())
    }

    #[actix_rt::test]
    async fn test_glob_match() -> Result<(), ActixError> {
        let path = PathBuf::from(format!(
            "expired.file1.{}",
            get_system_time()?.as_millis() + 50
        ));
        fs::write(&path, String::new())?;
        assert_eq!(path, glob_match_file(PathBuf::from("expired.file1")).await?);

        thread::sleep(Duration::from_millis(75));
        assert_eq!(
            PathBuf::from("expired.file1"),
            glob_match_file(PathBuf::from("expired.file1")).await?
        );
        fs::remove_file(path)?;

        Ok(())
    }

    #[test]
    fn test_sha256sum() -> Result<(), ActixError> {
        assert_eq!(
            "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
            sha256_digest(String::from("test").as_bytes())?
        );
        assert_eq!(
            "2fc36f72540bb9145e95e67c41dccdc440c95173257032e32e111ebd7b6df960",
            sha256_digest(env!("CARGO_PKG_NAME").as_bytes())?
        );
        Ok(())
    }

    #[test]
    fn test_get_expired_files() -> Result<(), ActixError> {
        let test_temp_dir = tempdir()?;
        let test_dir = test_temp_dir.path();
        let expiration_time = get_system_time()?.as_millis() + 50;
        let path = test_dir.join(format!("expired.file2.{expiration_time}"));
        fs::write(&path, String::new())?;
        assert_eq!(Vec::<PathBuf>::new(), get_expired_files(test_dir));
        thread::sleep(Duration::from_millis(75));
        assert_eq!(vec![path.clone()], get_expired_files(test_dir));
        fs::remove_file(&path)?;
        assert_eq!(Vec::<PathBuf>::new(), get_expired_files(test_dir));
        Ok(())
    }

    #[test]
    fn test_safe_join_path() {
        assert_eq!(safe_path_join("/foo", "bar").ok(), Some("/foo/bar".into()));
        assert_eq!(safe_path_join("/", "bar").ok(), Some("/bar".into()));
        assert_eq!(safe_path_join("/", "././bar").ok(), Some("/bar".into()));
        assert_eq!(
            safe_path_join("/foo/bar", "baz/").ok(),
            Some("/foo/bar/baz/".into())
        );
        assert_eq!(
            safe_path_join("/foo/bar/../", "baz").ok(),
            Some("/foo/baz".into())
        );

        assert!(safe_path_join("/foo", "/foobar").is_err());
        assert!(safe_path_join("/foo", "/bar").is_err());
        assert!(safe_path_join("/foo/bar", "..").is_err());
        assert!(safe_path_join("/foo/bar", "../").is_err());
    }
}
