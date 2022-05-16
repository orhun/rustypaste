use actix_files::file_extension_to_mime;
use mime::{FromStrError, Mime};
use regex::Regex;
use std::path::PathBuf;
use std::str::FromStr;

/// Matcher for MIME types.
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct MimeMatcher {
    /// MIME type to set for the matched file name.
    pub mime: String,
    /// Regex for matching the file name.
    #[serde(with = "serde_regex")]
    pub regex: Option<Regex>,
}

/// Returns the appropriate media type using an array of
/// [`MIME matcher`]s and the file name.
///
/// [`MIME matcher`]: MimeMatcher
pub fn get_mime_type(
    mime_matchers: &[MimeMatcher],
    file_name: String,
) -> Result<Mime, FromStrError> {
    let path = PathBuf::from(&file_name);
    let mut mime_type = file_extension_to_mime(
        path.extension()
            .and_then(|v| v.to_str())
            .unwrap_or_default(),
    );
    for matcher in mime_matchers {
        if matcher
            .regex
            .as_ref()
            .map(|r| r.is_match(&file_name))
            .unwrap_or(false)
        {
            mime_type = Mime::from_str(&matcher.mime)?;
            break;
        }
    }
    Ok(mime_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_mime_type() -> Result<(), FromStrError> {
        assert_eq!(
            mime::TEXT_PLAIN,
            get_mime_type(
                &[MimeMatcher {
                    mime: String::from("text/plain"),
                    regex: Regex::new("^.*\\.test$").ok(),
                }],
                String::from("mime.test")
            )?
        );
        assert_eq!(
            mime::IMAGE_PNG,
            get_mime_type(
                &[MimeMatcher {
                    mime: String::from("image/png"),
                    regex: Regex::new("^.*\\.PNG$").ok(),
                }],
                String::from("image.PNG")
            )?
        );
        assert_eq!(
            mime::APPLICATION_PDF,
            get_mime_type(&[], String::from("book.pdf"))?
        );
        assert_eq!(
            mime::APPLICATION_OCTET_STREAM,
            get_mime_type(&[], String::from("x.unknown"))?
        );
        Ok(())
    }
}
