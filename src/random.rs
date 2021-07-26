use rand::{distributions::Alphanumeric, Rng};

/// Random URL configuration.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct RandomURLConfig {
    /// Use a random name instead of original file names.
    pub enabled: bool,
    /// Count of words that pet name will include.
    pub words: Option<u8>,
    /// Separator between the words.
    pub separator: Option<String>,
    /// Length of the random string to generate.
    pub length: Option<usize>,
    /// Type of the random URL.
    #[serde(rename = "type")]
    pub type_: RandomURLType,
}

impl RandomURLConfig {
    /// Generates and returns a random URL (if `enabled`).
    pub fn generate(&self) -> Option<String> {
        if self.enabled {
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
        } else {
            None
        }
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
mod test {
    use super::*;

    #[test]
    fn test_generate_url() {
        let random_config = RandomURLConfig {
            enabled: true,
            words: Some(3),
            separator: Some(String::from("~")),
            type_: RandomURLType::PetName,
            ..RandomURLConfig::default()
        };
        let random_url = random_config.generate().unwrap();
        assert_eq!(3, random_url.split("~").collect::<Vec<&str>>().len());

        let random_config = RandomURLConfig {
            enabled: true,
            length: Some(21),
            type_: RandomURLType::Alphanumeric,
            ..RandomURLConfig::default()
        };
        let random_url = random_config.generate().unwrap();
        assert_eq!(21, random_url.len());

        let random_config = RandomURLConfig {
            enabled: false,
            ..RandomURLConfig::default()
        };
        assert!(random_config.generate().is_none());
    }
}
