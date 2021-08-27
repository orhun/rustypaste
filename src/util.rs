use actix_web::{error, Error as ActixError};
use glob::glob;
use std::path::PathBuf;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

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
    if let Some(glob_path) = glob(&format!(
        "{}.[0-9]*",
        path.to_str()
            .ok_or_else(|| error::ErrorInternalServerError(
                "file name contains invalid characters"
            ))?,
    ))
    .map_err(error::ErrorInternalServerError)?
    .next()
    {
        let glob_path = glob_path.map_err(error::ErrorInternalServerError)?;
        if let Some(extension) = glob_path
            .extension()
            .map(|v| v.to_str())
            .flatten()
            .map(|v| v.parse().ok())
            .flatten()
        {
            if get_system_time()? < Duration::from_millis(extension) {
                path = glob_path;
            }
        }
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
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
            "expired.file.{}",
            get_system_time()?.as_millis() + 50
        ));
        fs::write(&path, String::new())?;
        assert_eq!(path, glob_match_file(PathBuf::from("expired.file"))?);

        thread::sleep(Duration::from_millis(75));
        assert_eq!(
            PathBuf::from("expired.file"),
            glob_match_file(PathBuf::from("expired.file"))?
        );
        fs::remove_file(path)?;

        Ok(())
    }
}
