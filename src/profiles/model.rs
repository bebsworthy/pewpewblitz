use crate::characters::{CharacterChoice, CharacterId, MAX_CHARACTERS};
use serde::{Deserialize, Serialize};
use std::{fmt, num::NonZeroU64, str::FromStr};

pub const MAX_PROFILE_SNAPSHOT_BYTES: usize = 4 * 1024;

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct AccountId(u128);

impl<'de> Deserialize<'de> for AccountId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = u128::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

impl AccountId {
    pub fn new(value: u128) -> Result<Self, ProfileModelError> {
        (value != 0)
            .then_some(Self(value))
            .ok_or(ProfileModelError::ZeroId)
    }

    pub fn random() -> Result<Self, ProfileModelError> {
        let mut bytes = [0_u8; 16];
        getrandom::fill(&mut bytes).map_err(|_| ProfileModelError::EntropyUnavailable)?;
        Self::new(u128::from_be_bytes(bytes))
    }

    #[must_use]
    pub const fn get(self) -> u128 {
        self.0
    }

    #[must_use]
    pub const fn to_bytes(self) -> [u8; 16] {
        self.0.to_be_bytes()
    }

    pub fn from_bytes(bytes: [u8; 16]) -> Result<Self, ProfileModelError> {
        Self::new(u128::from_be_bytes(bytes))
    }
}

impl fmt::Display for AccountId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:032x}", self.0)
    }
}

impl fmt::Debug for AccountId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AccountId(")?;
        fmt::Display::fmt(self, formatter)?;
        formatter.write_str(")")
    }
}

impl FromStr for AccountId {
    type Err = ProfileModelError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 32
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ProfileModelError::MalformedId);
        }
        let raw = u128::from_str_radix(value, 16).map_err(|_| ProfileModelError::MalformedId)?;
        Self::new(raw)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct ProfileRevision(NonZeroU64);

impl ProfileRevision {
    pub const INITIAL: Self = Self(NonZeroU64::MIN);

    pub fn new(value: u64) -> Result<Self, ProfileModelError> {
        NonZeroU64::new(value)
            .map(Self)
            .ok_or(ProfileModelError::InvalidRevision)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }

    pub fn next(self) -> Result<Self, ProfileModelError> {
        self.get()
            .checked_add(1)
            .ok_or(ProfileModelError::InvalidRevision)
            .and_then(Self::new)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileModelError {
    ZeroId,
    MalformedId,
    EntropyUnavailable,
    InvalidRevision,
    TooManyChoices,
    DuplicateChoice,
    MissingSelection,
    InvalidChoice,
}

impl fmt::Display for ProfileModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

/// One account's character pick: the selected character and the ultimate and passive remembered
/// for every character the player has picked. The authority normalizes it against the character
/// catalog before it reaches the wire, so `choices` always holds the selected character's pick.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ProfileSnapshot {
    pub account_id: AccountId,
    pub revision: ProfileRevision,
    pub selected_character_id: CharacterId,
    /// Ascending by character, one per character.
    pub choices: Vec<CharacterChoice>,
}

impl ProfileSnapshot {
    pub fn validate(&self) -> Result<(), ProfileModelError> {
        if self.choices.len() > MAX_CHARACTERS {
            return Err(ProfileModelError::TooManyChoices);
        }
        if self
            .choices
            .windows(2)
            .any(|pair| pair[0].character_id >= pair[1].character_id)
        {
            return Err(ProfileModelError::DuplicateChoice);
        }
        if self.choice(self.selected_character_id).is_none() {
            return Err(ProfileModelError::MissingSelection);
        }
        Ok(())
    }

    pub fn validate_bounded(&self) -> Result<(), ProfileModelError> {
        self.validate()?;
        if postcard::to_allocvec(self)
            .map_or(true, |bytes| bytes.len() > MAX_PROFILE_SNAPSHOT_BYTES)
        {
            return Err(ProfileModelError::TooManyChoices);
        }
        Ok(())
    }

    /// The remembered pick for one character, if the player has picked it.
    #[must_use]
    pub fn choice(&self, character_id: CharacterId) -> Option<CharacterChoice> {
        self.choices
            .iter()
            .copied()
            .find(|choice| choice.character_id == character_id)
    }

    /// The selected character with its chosen options.
    #[must_use]
    pub fn selection(&self) -> CharacterChoice {
        self.choice(self.selected_character_id)
            .expect("a validated profile snapshot holds its selection")
    }
}

/// Select a character with one of its ultimates and one of its passives. The pick is remembered
/// for that character.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ProfileCommand {
    pub request_id: u64,
    pub expected_profile_revision: ProfileRevision,
    pub choice: CharacterChoice,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ProfileDecision {
    Accepted,
    InvalidRequest,
    StaleRevision,
    QueueLocked,
    TemporarilyUnavailable,
    StorageFault,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ProfileOutcome {
    pub request_id: u64,
    pub decision: ProfileDecision,
    pub snapshot: Option<ProfileSnapshot>,
}

/// Immutable character pick handed through routing without account or storage authority.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchCharacterSnapshot {
    pub schema_version: u8,
    pub choice: CharacterChoice,
    pub accepted_identity: crate::builds::SelectedBuild,
}

impl MatchCharacterSnapshot {
    pub const SCHEMA_VERSION: u8 = 8;

    pub fn from_choice(
        choice: CharacterChoice,
        catalogs: crate::characters::LoadoutCatalogs<'_>,
    ) -> Result<Self, crate::builds::BuildResolutionError> {
        let resolved = catalogs.resolve(choice)?;
        Ok(Self {
            schema_version: Self::SCHEMA_VERSION,
            choice,
            accepted_identity: resolved.identity,
        })
    }

    pub fn encode(self) -> Result<brawler_routing::MatchBuildSnapshot, String> {
        let bytes = postcard::to_allocvec(&self)
            .map_err(|error| format!("match character snapshot encode failed: {error}"))?;
        brawler_routing::MatchBuildSnapshot::new(&bytes)
            .map_err(|error| format!("match character snapshot exceeds bound: {error:?}"))
    }

    pub fn decode(snapshot: &brawler_routing::MatchBuildSnapshot) -> Result<Self, String> {
        let value: Self = postcard::from_bytes(snapshot.as_bytes())
            .map_err(|error| format!("match character snapshot decode failed: {error}"))?;
        if value.schema_version != Self::SCHEMA_VERSION {
            return Err("unsupported match character snapshot version".to_string());
        }
        Ok(value)
    }

    /// Resolve against the match's catalogs and require the identity the lobby accepted.
    pub fn resolve(
        self,
        catalogs: crate::characters::LoadoutCatalogs<'_>,
    ) -> Result<crate::builds::ResolvedMatchLoadout, crate::builds::BuildResolutionError> {
        let resolved = catalogs.resolve(self.choice)?;
        if resolved.identity != self.accepted_identity {
            return Err(crate::builds::BuildResolutionError::InvalidCombination);
        }
        Ok(resolved)
    }

    /// Re-resolve a snapshot whose accepted identity was already checked at the immutable match
    /// manifest boundary against a validated, development-only revised catalog.
    #[cfg(feature = "balance-lab")]
    pub(crate) fn resolve_revised_balance_lab_catalogs(
        self,
        catalogs: crate::characters::LoadoutCatalogs<'_>,
    ) -> Result<crate::builds::ResolvedMatchLoadout, crate::builds::BuildResolutionError> {
        catalogs.resolve(self.revised_balance_lab_choice(catalogs.characters))
    }

    /// The admitted pick carried onto a Balance Lab-revised roster by option slot.
    #[cfg(feature = "balance-lab")]
    #[must_use]
    pub(crate) fn revised_balance_lab_choice(
        self,
        characters: &crate::characters::CharacterCatalog,
    ) -> CharacterChoice {
        characters.carry_balance_lab_choice(self.choice)
    }
}
