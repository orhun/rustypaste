use crate::util;
use actix_web::http::header::{
    ContentDisposition as ActixContentDisposition, DispositionParam, DispositionType, HeaderMap,
};
use actix_web::{error, Error as ActixError};

/// Custom HTTP header for expiry dates.
pub const EXPIRE: &str = "expire";

/// Parses the expiry date from the [`custom HTTP header`](EXPIRE).
pub fn parse_expiry_date(headers: &HeaderMap) -> Result<Option<u128>, ActixError> {
    if let Some(expire_time) = headers.get(EXPIRE).and_then(|v| v.to_str().ok()) {
        let timestamp = util::get_system_time()?;
        let expire_time =
            humantime::parse_duration(expire_time).map_err(error::ErrorInternalServerError)?;
        Ok(timestamp.checked_add(expire_time).map(|t| t.as_millis()))
    } else {
        Ok(None)
    }
}

/// Wrapper for Actix content disposition header.
///
/// Aims to parse the file data from multipart body.
///
/// e.g. `Content-Disposition: form-data; name="field_name"; filename="filename.jpg"`
pub struct ContentDisposition {
    inner: ActixContentDisposition,
}

impl From<ActixContentDisposition> for ContentDisposition {
    fn from(content_disposition: ActixContentDisposition) -> Self {
        Self {
            inner: content_disposition,
        }
    }
}

impl ContentDisposition {
    /// Checks if the content disposition is a form data
    /// and has the field `field_name`.
    pub fn has_form_field(&self, field_name: &str) -> bool {
        self.inner.disposition == DispositionType::FormData
            && self
                .inner
                .parameters
                .contains(&DispositionParam::Name(field_name.to_string()))
    }

    /// Parses the file name from parameters if it exists.
    pub fn get_file_name(&self) -> Result<&str, ActixError> {
        self.inner
            .parameters
            .iter()
            .find(|param| param.is_filename())
            .and_then(|param| param.as_filename())
            .filter(|file_name| !file_name.is_empty())
            .ok_or_else(|| error::ErrorBadRequest("file data not present"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http::{HeaderName, HeaderValue};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_content_disposition() -> Result<(), ActixError> {
        assert!(ContentDisposition::try_from(None).is_err());

        let actix_content_disposition = Some(ActixContentDisposition {
            disposition: DispositionType::FormData,
            parameters: vec![
                DispositionParam::Name(String::from("file")),
                DispositionParam::Filename(String::from("x.txt")),
            ],
        });
        let content_disposition = ContentDisposition::try_from(actix_content_disposition)?;
        assert!(content_disposition.has_form_field("file"));
        assert!(!content_disposition.has_form_field("test"));
        assert_eq!("x.txt", content_disposition.get_file_name()?);

        let actix_content_disposition = Some(ActixContentDisposition {
            disposition: DispositionType::Attachment,
            parameters: vec![DispositionParam::Name(String::from("file"))],
        });
        let content_disposition = ContentDisposition::try_from(actix_content_disposition)?;
        assert!(!content_disposition.has_form_field("file"));
        assert!(content_disposition.get_file_name().is_err());
        Ok(())
    }

    #[test]
    fn test_expiry_date() -> Result<(), ActixError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static(EXPIRE),
            HeaderValue::from_static("5ms"),
        );
        let expiry_time = parse_expiry_date(&headers)?.unwrap();
        assert!(expiry_time > util::get_system_time()?.as_millis());
        thread::sleep(Duration::from_millis(10));
        assert!(expiry_time < util::get_system_time()?.as_millis());
        Ok(())
    }
}
