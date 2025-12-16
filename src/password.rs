//! Password generation, hashing, and verification for protected files.
//!
//! Protected files use Argon2id hashing with 19MB memory and 2 iterations.
//! Passwords are stored in sidecar files (filename.txt.password) alongside
//! the uploaded content.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2, ParamsBuilder,
};
use rand::{distr::Alphanumeric, Rng};
use std::fs;
use std::io::{Error as IoError, ErrorKind as IoErrorKind, Result as IoResult};
use std::path::{Path, PathBuf};

/// Generate random alphanumeric password (24 chars = ~143 bits entropy)
pub fn generate_password() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(24)
        .map(char::from)
        .collect()
}

/// Hash password using Argon2id (19MB memory, 2 iterations)
pub fn hash_password(password: &str) -> Result<String, IoError> {
    let salt = SaltString::generate(&mut OsRng);
    let params = ParamsBuilder::new()
        .m_cost(19456) // 19MB
        .t_cost(2)
        .p_cost(1)
        .build()
        .map_err(|e| IoError::other(format!("argon2 params: {}", e)))?;

    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);

    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| IoError::other(format!("hash failed: {}", e)))
}

/// Verify password against hash (constant-time)
pub fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .ok()
        .and_then(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .ok()
        })
        .is_some()
}

/// Get password file path for a given file
pub fn get_password_file_path(file_path: &Path) -> IoResult<PathBuf> {
    let current_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| {
            IoError::new(
                IoErrorKind::InvalidInput,
                "file path contains invalid characters",
            )
        })?;

    let mut path = file_path.to_path_buf();
    path.set_file_name(format!("{}.password", current_name));
    Ok(path)
}

/// Store password hash in sidecar file (file.txt -> file.txt.password)
pub fn store_password_hash(file_path: &Path, password: &str) -> IoResult<()> {
    let hash = hash_password(password)?;
    let password_path = get_password_file_path(file_path)?;
    fs::write(password_path, hash)
}

/// Check if file has password protection
pub fn has_password(file_path: &Path) -> bool {
    get_password_file_path(file_path)
        .map(|p| p.exists())
        .unwrap_or(false)
}

/// Verify password for a file
pub fn verify_file_password(file_path: &Path, password: &str) -> IoResult<bool> {
    let password_path = get_password_file_path(file_path)?;
    let hash = fs::read_to_string(password_path)?;
    Ok(verify_password(password, hash.trim()))
}

/// Delete password file
pub fn delete_password_file(file_path: &Path) -> IoResult<()> {
    let password_path = get_password_file_path(file_path)?;
    if password_path.exists() {
        fs::remove_file(password_path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_password_hashing() {
        let password = "test_password_123";
        let hash = hash_password(password).unwrap();
        assert!(verify_password(password, &hash));
        assert!(!verify_password("wrong", &hash));
    }

    #[test]
    fn test_password_file_path() -> IoResult<()> {
        let test_path = PathBuf::from("/tmp/test_file.txt");
        let password_path = get_password_file_path(&test_path)?;

        assert_eq!(PathBuf::from("/tmp/test_file.txt.password"), password_path);

        Ok(())
    }

    #[test]
    fn test_password_file_path_invalid_utf8() {
        // This tests the error handling for invalid paths
        // On Unix, we can create paths with invalid UTF-8
        #[cfg(unix)]
        {
            use std::ffi::OsStr;
            use std::os::unix::ffi::OsStrExt;

            // Create a path with invalid UTF-8 bytes
            let invalid_bytes = &[0x66, 0x6f, 0x6f, 0x80, 0x81];
            let invalid_os_str = OsStr::from_bytes(invalid_bytes);
            let invalid_path = PathBuf::from(invalid_os_str);

            // Should return error, not panic
            assert!(get_password_file_path(&invalid_path).is_err());
        }
    }

    #[test]
    fn test_store_and_verify_password() -> IoResult<()> {
        let current_dir = env::current_dir()?;
        let test_file = current_dir.join("test_password_roundtrip.txt");

        // Create test file
        fs::write(&test_file, "test content")?;

        let password = "my_test_password";

        // Store password hash
        store_password_hash(&test_file, password)?;

        // Verify password file exists
        assert!(has_password(&test_file));

        // Verify correct password
        assert!(verify_file_password(&test_file, password)?);

        // Verify wrong password fails
        assert!(!verify_file_password(&test_file, "wrong_password")?);

        // Cleanup
        delete_password_file(&test_file)?;
        fs::remove_file(&test_file)?;

        assert!(!has_password(&test_file));

        Ok(())
    }
}
