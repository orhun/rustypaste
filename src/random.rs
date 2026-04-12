use rand::distr::{Alphanumeric, SampleString};

/// Random URL configuration.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct RandomURLConfig {
    /// Use a random name instead of original file names.
    #[deprecated(note = "disable by commenting out [paste].random_url")]
    pub enabled: Option<bool>,
    /// Count of words that pet name will include.
    pub words: Option<u8>,
    /// Separator between the words.
    pub separator: Option<String>,
    /// Length of the random string to generate.
    pub length: Option<usize>,
    /// Type of the random URL.
    #[serde(rename = "type")]
    pub type_: RandomURLType,
    /// Append a random string to the original filename.
    pub suffix_mode: Option<bool>,
    /// Do not add or keep an extension.
    pub no_extension: Option<bool>,
}

#[allow(deprecated)]
impl RandomURLConfig {
    /// Generates and returns a random URL (if `enabled`).
    pub fn generate(&self) -> Option<String> {
        if !self.enabled.unwrap_or(true) {
            return None;
        }
        Some(match self.type_ {
            RandomURLType::PetName => {
                let mut buf = String::new();
                petname::Petnames::large()
                    .namer(
                        self.words.unwrap_or(2),
                        self.separator.as_deref().unwrap_or("-"),
                    )
                    .generate_into(&mut buf, &mut rand::rng());
                buf
            }
            RandomURLType::Alphanumeric => {
                Alphanumeric.sample_string(&mut rand::rng(), self.length.unwrap_or(8))
            }
        })
    }
}

/// Type of the random URL.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum RandomURLType {
    /// Generate a random pet name.
    #[default]
    PetName,
    /// Generate a random alphanumeric string.
    Alphanumeric,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(deprecated)]
    #[test]
    fn test_generate_url() {
        let random_config = RandomURLConfig {
            enabled: Some(true),
            words: Some(3),
            separator: Some(String::from("~")),
            type_: RandomURLType::PetName,
            ..RandomURLConfig::default()
        };
        let random_url = random_config
            .generate()
            .expect("cannot generate random URL");
        assert_eq!(3, random_url.split('~').count());

        let random_config = RandomURLConfig {
            length: Some(21),
            type_: RandomURLType::Alphanumeric,
            ..RandomURLConfig::default()
        };
        let random_url = random_config
            .generate()
            .expect("cannot generate random URL");
        assert_eq!(21, random_url.len());

        let random_config = RandomURLConfig {
            enabled: Some(false),
            ..RandomURLConfig::default()
        };
        assert!(random_config.generate().is_none());
    }
}
