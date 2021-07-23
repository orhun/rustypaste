use actix_web::http::header::{
    ContentDisposition as ActixContentDisposition, DispositionParam, DispositionType,
};
use actix_web::{error, Error as ActixError};
use std::convert::TryFrom;

/// Wrapper for Actix content disposition header.
///
/// Aims to parse the file data from multipart body.
///
/// e.g. `Content-Disposition: form-data; name="field_name"; filename="filename.jpg"`
pub struct ContentDisposition {
    inner: ActixContentDisposition,
}

impl TryFrom<Option<ActixContentDisposition>> for ContentDisposition {
    type Error = ActixError;
    fn try_from(content_disposition: Option<ActixContentDisposition>) -> Result<Self, Self::Error> {
        match content_disposition {
            Some(inner) => Ok(Self { inner }),
            None => Err(error::ErrorUnprocessableEntity(
                "content disposition does not exist",
            )),
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
            .map(|param| param.as_filename())
            .flatten()
            .ok_or_else(|| error::ErrorUnprocessableEntity("file data not present"))
    }
}
