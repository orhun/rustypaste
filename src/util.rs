use crate::paste::PasteType;
use actix_web::{error, Error as ActixError};
use glob::glob;
use lazy_regex::{lazy_regex, Lazy, Regex};
use path_clean::PathClean;
use ring::digest::{Context, SHA256};
use std::fmt::Write;
use std::io::{BufReader, Read};
use std::io::{Error as IoError, ErrorKind as IoErrorKind, Result as IoResult};
use std::net::{IpAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use url::Url;

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
pub fn glob_match_file(mut path: PathBuf) -> Result<PathBuf, ActixError> {
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
    if let Some(glob_path) = glob(&format!("{}.[0-9]*", path.to_string_lossy()))
        .map_err(error::ErrorInternalServerError)?
        .last()
    {
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
            write!(output, "{b:02x}").map_err(|e| IoError::other(e.to_string()))?;
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

/// Returns the size of the directory at the given path.
///
/// This function is recursive, and will calculate the size of all files and directories.
/// If a symlink is encountered, the size of the symlink itself is counted, not its target.
///
/// Adopted from <https://docs.rs/fs_extra/latest/src/fs_extra/dir.rs.html>
pub fn get_dir_size(path: &Path) -> IoResult<u64> {
    let path_metadata = path.symlink_metadata()?;
    let mut size_in_bytes = 0;
    if path_metadata.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let entry_metadata = entry.metadata()?;
            if entry_metadata.is_dir() {
                size_in_bytes += get_dir_size(&entry.path())?;
            } else {
                size_in_bytes += entry_metadata.len();
            }
        }
    } else {
        size_in_bytes = path_metadata.len();
    }
    Ok(size_in_bytes)
}

/// Validates that the URL uses an allowed scheme and does not resolve to disallowed IPs.
pub fn validate_remote_url(url: &Url) -> IoResult<()> {
    let scheme = url.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            "unsupported URL scheme",
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| IoError::new(IoErrorKind::InvalidInput, "URL host is missing"))?;
    if host == "localhost" || host.ends_with(".localhost") {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            "localhost is not allowed",
        ));
    }
    let port = url
        .port_or_known_default()
        .ok_or_else(|| IoError::new(IoErrorKind::InvalidInput, "URL port is missing"))?;
    let addrs = (host, port)
        .to_socket_addrs()
        .map_err(|e| IoError::new(IoErrorKind::InvalidInput, e.to_string()))?;
    let mut resolved = false;
    for addr in addrs {
        resolved = true;
        if is_disallowed_ip(addr.ip()) {
            return Err(IoError::new(
                IoErrorKind::InvalidInput,
                "URL resolves to a disallowed address",
            ));
        }
    }
    if !resolved {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            "URL host did not resolve",
        ));
    }
    Ok(())
}

fn is_disallowed_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_disallowed_ipv4(v4),
        IpAddr::V6(v6) => is_disallowed_ipv6(v6),
    }
}

fn is_disallowed_ipv4(v4: std::net::Ipv4Addr) -> bool {
    let o = v4.octets();
    if v4.is_loopback()
        || v4.is_private()
        || v4.is_link_local()
        || v4.is_multicast()
        || v4.is_broadcast()
        || v4.is_documentation()
        || v4.is_unspecified()
    {
        return true;
    }
    // Carrier-grade NAT: 100.64.0.0/10
    if o[0] == 100 && (o[1] & 0b1100_0000) == 0b0100_0000 {
        return true;
    }
    // Benchmarking: 198.18.0.0/15
    if o[0] == 198 && (o[1] == 18 || o[1] == 19) {
        return true;
    }
    // IETF protocol assignments: 192.0.0.0/24
    if o[0] == 192 && o[1] == 0 && o[2] == 0 {
        return true;
    }
    // Reserved for future use: 240.0.0.0/4
    if o[0] >= 240 {
        return true;
    }
    // Explicit metadata IP block.
    o == [169, 254, 169, 254]
}

fn is_disallowed_ipv6(v6: std::net::Ipv6Addr) -> bool {
    if let Some(v4) = v6.to_ipv4() {
        return is_disallowed_ipv4(v4);
    }
    if v6.is_loopback()
        || v6.is_unspecified()
        || v6.is_multicast()
        || v6.is_unique_local()
        || v6.is_unicast_link_local()
    {
        return true;
    }
    let seg = v6.segments();
    // Documentation: 2001:db8::/32
    if seg[0] == 0x2001 && seg[1] == 0x0db8 {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::thread;
    #[test]
    fn test_system_time() -> Result<(), ActixError> {
        let system_time = get_system_time()?.as_millis();
        thread::sleep(Duration::from_millis(1));
        assert!(system_time < get_system_time()?.as_millis());
        Ok(())
    }

    #[test]
    fn test_glob_match() -> Result<(), ActixError> {
        let path = PathBuf::from(format!(
            "expired.file1.{}",
            get_system_time()?.as_millis() + 50
        ));
        fs::write(&path, String::new())?;
        assert_eq!(path, glob_match_file(PathBuf::from("expired.file1"))?);

        thread::sleep(Duration::from_millis(75));
        assert_eq!(
            PathBuf::from("expired.file1"),
            glob_match_file(PathBuf::from("expired.file1"))?
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
        let current_dir = env::current_dir()?;
        let expiration_time = get_system_time()?.as_millis() + 50;
        let path = PathBuf::from(format!("expired.file2.{expiration_time}"));
        fs::write(&path, String::new())?;
        assert_eq!(Vec::<PathBuf>::new(), get_expired_files(&current_dir));
        thread::sleep(Duration::from_millis(75));
        assert_eq!(
            vec![current_dir.join(&path)],
            get_expired_files(&current_dir)
        );
        fs::remove_file(path)?;
        assert_eq!(Vec::<PathBuf>::new(), get_expired_files(&current_dir));
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
