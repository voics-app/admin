use thiserror::Error;

const MAX_KEY_LEN: usize = 64;
const MAX_DESCRIPTION_LEN: usize = 280;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FlagError {
    #[error("flag key must be 2-64 chars of a-z, 0-9, '.', '_' or '-', starting with a letter")]
    InvalidKey,
    #[error("rollout percent must be between 0 and 100")]
    InvalidRollout,
    #[error("description must be at most 280 characters")]
    DescriptionTooLong,
}

/// A validated flag identifier such as `voice.av1_simulcast`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FlagKey(String);

impl FlagKey {
    pub fn parse(value: &str) -> Result<Self, FlagError> {
        let starts_with_letter = value.chars().next().is_some_and(|c| c.is_ascii_lowercase());
        let allowed = value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'));
        let sized = (2..=MAX_KEY_LEN).contains(&value.len());
        if starts_with_letter && allowed && sized {
            Ok(Self(value.to_owned()))
        } else {
            Err(FlagError::InvalidKey)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Share of subjects (0-100) that see an enabled flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RolloutPercent(u8);

impl RolloutPercent {
    pub fn parse(value: i64) -> Result<Self, FlagError> {
        u8::try_from(value)
            .ok()
            .filter(|v| *v <= 100)
            .map(Self)
            .ok_or(FlagError::InvalidRollout)
    }

    pub fn value(self) -> u8 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureFlag {
    pub key: FlagKey,
    pub description: String,
    pub enabled: bool,
    pub rollout_percent: RolloutPercent,
}

impl FeatureFlag {
    pub fn new(
        key: FlagKey,
        description: &str,
        enabled: bool,
        rollout_percent: RolloutPercent,
    ) -> Result<Self, FlagError> {
        let description = description.trim();
        if description.chars().count() > MAX_DESCRIPTION_LEN {
            return Err(FlagError::DescriptionTooLong);
        }
        Ok(Self {
            key,
            description: description.to_owned(),
            enabled,
            rollout_percent,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_dotted_snake_case_keys() {
        assert_eq!(
            FlagKey::parse("voice.av1_simulcast").unwrap().as_str(),
            "voice.av1_simulcast"
        );
        assert!(FlagKey::parse("p2p-dms").is_ok());
    }

    #[test]
    fn rejects_malformed_keys() {
        for bad in [
            "",
            "a",
            "1abc",
            "Voice",
            "has space",
            "emoji🙂",
            &"x".repeat(65),
        ] {
            assert_eq!(FlagKey::parse(bad), Err(FlagError::InvalidKey), "{bad}");
        }
    }

    #[test]
    fn accepts_the_longest_allowed_key() {
        assert!(FlagKey::parse(&"x".repeat(64)).is_ok());
    }

    #[test]
    fn rollout_must_be_a_percentage() {
        assert_eq!(RolloutPercent::parse(0).unwrap().value(), 0);
        assert_eq!(RolloutPercent::parse(100).unwrap().value(), 100);
        assert_eq!(RolloutPercent::parse(101), Err(FlagError::InvalidRollout));
        assert_eq!(RolloutPercent::parse(-1), Err(FlagError::InvalidRollout));
    }

    #[test]
    fn flag_description_is_trimmed_and_bounded() {
        let key = FlagKey::parse("watch_together").unwrap();
        let rollout = RolloutPercent::parse(50).unwrap();
        let flag = FeatureFlag::new(key.clone(), "  Sync playback  ", true, rollout).unwrap();
        assert_eq!(flag.description, "Sync playback");
        assert_eq!(
            FeatureFlag::new(key, &"d".repeat(281), true, rollout),
            Err(FlagError::DescriptionTooLong)
        );
    }
}
