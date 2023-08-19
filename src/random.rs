use rand::{distributions::Alphanumeric, Rng};

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
}

#[allow(deprecated)]
impl RandomURLConfig {
    /// Generates and returns a random URL (if `enabled`).
    pub fn generate(&self) -> Option<String> {
        if let Some(enabled) = self.enabled {
            if !enabled {
                return None;
            }
        }
        Some(match self.type_ {
            RandomURLType::PetName => petname::petname(
                self.words.unwrap_or(2),
                self.separator.as_deref().unwrap_or("-"),
            ),
            RandomURLType::Alphanumeric => rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(self.length.unwrap_or(8))
                .map(char::from)
                .collect::<String>(),
        })
    }
}

/// Type of the random URL.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RandomURLType {
    /// Generate a random pet name.
    PetName,
    /// Generate a random alphanumeric string.
    Alphanumeric,
}

impl Default for RandomURLType {
    fn default() -> Self {
        Self::PetName
    }
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
