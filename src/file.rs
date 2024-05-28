use crate::util;
use glob::glob;
use std::convert::TryFrom;
use std::fs::File as OsFile;
use std::path::{Path, PathBuf};

/// [`PathBuf`] wrapper for storing checksums.
#[derive(Debug)]
pub struct File {
    /// Path of the file.
    pub path: PathBuf,
    /// SHA256 checksum.
    pub sha256sum: String,
}

/// Directory that contains [`File`]s.
pub struct Directory {
    /// Files in the directory.
    pub files: Vec<File>,
}

impl<'a> TryFrom<&'a Path> for Directory {
    type Error = String;
    fn try_from(directory: &'a Path) -> Result<Self, Self::Error> {
        let files = glob(
            directory
                .join("**")
                .join("*")
                .to_str()
                .ok_or_else(|| String::from("directory contains invalid characters"))?,
        )
        .map_err(|e| e.msg)?
        .filter_map(Result::ok)
        .filter(|path| !path.is_dir())
        .filter_map(|path| match OsFile::open(&path) {
            Ok(file) => Some((path, file)),
            _ => None,
        })
        .filter_map(|(path, file)| match util::sha256_digest(file) {
            Ok(sha256sum) => Some(File { path, sha256sum }),
            _ => None,
        })
        .collect();
        Ok(Self { files })
    }
}

impl Directory {
    /// Returns the file that matches the given checksum.
    pub fn get_file<S: AsRef<str>>(self, sha256sum: S) -> Option<File> {
        self.files.into_iter().find(|file| {
            file.sha256sum == sha256sum.as_ref()
                && !util::TIMESTAMP_EXTENSION_REGEX.is_match(&file.path.to_string_lossy())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn test_file_checksum() -> Result<(), String> {
        assert_eq!(
            Some(OsString::from("rustypaste_logo.png").as_ref()),
            Directory::try_from(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("img")
                    .as_path()
            )?
            .get_file("2073f6f567dcba3b468c568d29cf8ed2e9d3f0f7305b9ab1b5a22861f5922e61")
            .expect("cannot get file with checksum")
            .path
            .file_name()
        );
        Ok(())
    }
}
