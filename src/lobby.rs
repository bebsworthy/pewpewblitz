//! Shared bounded lobby wire model and canonical catalog identity.
//!
//! Operator configuration and playability decisions remain server-owned. This module only owns
//! the stable presentation-safe snapshot, structural validation, display-name primitives, and the
//! canonical revision encoding used on both sides of the authenticated lobby session.

use crate::map::{MapPresetId, ModeDefinitionId};
use serde::{Deserialize, Deserializer, Serialize, de::SeqAccess, de::Visitor};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization as _;
use unicode_segmentation::UnicodeSegmentation as _;

mod queue;

pub use queue::{
    MAX_QUEUE_OUTCOME_BYTES, QueueCancelCommand, QueueClientMessage, QueueCommand,
    QueueCommandOutcome, QueueDecision, QueueJoinCommand, QueueMembership, QueuePoolRow,
    QueuePoolSnapshot, QueueRejection, QueueRequestId, QueueTicketId,
};

pub const MAX_GAME_TYPES: usize = 8;
pub const MAX_MAPS_PER_GAME_TYPE: usize = 8;
pub const MAX_GAME_TYPE_ID_BYTES: usize = 32;
pub const MAX_DISPLAY_NAME_GRAPHEMES: usize = 48;
pub const MAX_DISPLAY_NAME_BYTES: usize = 96;
pub const MAX_PLAYER_NAME_GRAPHEMES: usize = 24;
pub const MAX_PLAYER_NAME_BYTES: usize = 64;
pub const CATALOG_REVISION_BYTES: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LobbyModelError {
    EmptyCatalog,
    TooManyGameTypes,
    InvalidGameTypeId,
    DuplicateGameTypeId,
    InvalidRevision,
    InvalidDisplayName,
    InvalidMode,
    InvalidMapCount,
    DuplicateMap,
    InvalidTopology,
    InvalidRules,
    InvalidPlayerName,
}

impl core::fmt::Display for LobbyModelError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "invalid lobby model: {self:?}")
    }
}

impl std::error::Error for LobbyModelError {}

#[derive(Serialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(into = "String")]
pub struct GameTypeId(String);

impl GameTypeId {
    pub fn new(value: impl Into<String>) -> Result<Self, LobbyModelError> {
        let value = value.into();
        let bytes = value.as_bytes();
        if bytes.is_empty()
            || bytes.len() > MAX_GAME_TYPE_ID_BYTES
            || (!bytes[0].is_ascii_lowercase() && !bytes[0].is_ascii_digit())
            || bytes
                .iter()
                .any(|byte| !byte.is_ascii_lowercase() && !byte.is_ascii_digit() && *byte != b'-')
        {
            return Err(LobbyModelError::InvalidGameTypeId);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for GameTypeId {
    type Error = LobbyModelError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<GameTypeId> for String {
    fn from(value: GameTypeId) -> Self {
        value.0
    }
}

impl<'de> Deserialize<'de> for GameTypeId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct IdVisitor;

        impl Visitor<'_> for IdVisitor {
            type Value = GameTypeId;

            fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str("a bounded lowercase game-type ID")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                GameTypeId::new(value).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(IdVisitor)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CatalogRevision(pub [u8; CATALOG_REVISION_BYTES]);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdvertisedRulesSummary {
    Wipeout {
        target_score: u16,
        active_limit_ticks: u64,
    },
    HotZone {
        target_progress_ticks: u16,
        active_limit_ticks: u64,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AdvertisedGameType {
    pub id: GameTypeId,
    pub configuration_revision: u32,
    #[serde(deserialize_with = "deserialize_presentation_name")]
    pub display_name: String,
    pub mode_definition_id: ModeDefinitionId,
    #[serde(deserialize_with = "deserialize_map_ids")]
    pub map_preset_ids: Vec<MapPresetId>,
    pub team_count: u8,
    pub players_per_team: u8,
    pub rules_summary: AdvertisedRulesSummary,
}

pub(crate) fn deserialize_presentation_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_validated_string(deserializer, MAX_DISPLAY_NAME_BYTES, |value| {
        validate_presentation_name(value)
            .ok()
            .filter(|normalized| normalized == value)
    })
}

pub(crate) fn deserialize_player_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_validated_string(deserializer, MAX_PLAYER_NAME_BYTES, |value| {
        normalize_proposed_display_name(value).ok()
    })
}

pub(crate) fn deserialize_accepted_player_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_validated_string(deserializer, MAX_PLAYER_NAME_BYTES, |value| {
        normalize_proposed_display_name(value)
            .ok()
            .filter(|normalized| normalized == value)
    })
}

fn deserialize_validated_string<'de, D>(
    deserializer: D,
    maximum_bytes: usize,
    validate: impl Fn(&str) -> Option<String>,
) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringVisitor<F> {
        maximum_bytes: usize,
        validate: F,
    }

    impl<F> Visitor<'_> for StringVisitor<F>
    where
        F: Fn(&str) -> Option<String>,
    {
        type Value = String;

        fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(
                formatter,
                "a normalized string no longer than {} bytes",
                self.maximum_bytes
            )
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if value.len() > self.maximum_bytes {
                return Err(E::invalid_length(value.len(), &self));
            }
            (self.validate)(value).ok_or_else(|| E::custom("invalid bounded lobby string"))
        }
    }

    deserializer.deserialize_str(StringVisitor {
        maximum_bytes,
        validate,
    })
}

fn deserialize_map_ids<'de, D>(deserializer: D) -> Result<Vec<MapPresetId>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec::<D, MapPresetId, MAX_MAPS_PER_GAME_TYPE>(deserializer)
}

pub(crate) fn deserialize_game_types<'de, D>(
    deserializer: D,
) -> Result<Vec<AdvertisedGameType>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec::<D, AdvertisedGameType, MAX_GAME_TYPES>(deserializer)
}

fn deserialize_bounded_vec<'de, D, T, const MAXIMUM: usize>(
    deserializer: D,
) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct BoundedVecVisitor<T, const MAXIMUM: usize>(core::marker::PhantomData<T>);

    impl<'de, T, const MAXIMUM: usize> Visitor<'de> for BoundedVecVisitor<T, MAXIMUM>
    where
        T: Deserialize<'de>,
    {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(formatter, "a sequence with at most {MAXIMUM} entries")
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            if sequence.size_hint().is_some_and(|length| length > MAXIMUM) {
                return Err(serde::de::Error::invalid_length(
                    sequence.size_hint().unwrap_or(MAXIMUM + 1),
                    &self,
                ));
            }
            let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(MAXIMUM));
            while let Some(value) = sequence.next_element()? {
                if values.len() == MAXIMUM {
                    return Err(serde::de::Error::invalid_length(MAXIMUM + 1, &self));
                }
                values.push(value);
            }
            Ok(values)
        }
    }

    deserializer.deserialize_seq(BoundedVecVisitor::<T, MAXIMUM>(core::marker::PhantomData))
}

impl AdvertisedGameType {
    pub fn validate(&self) -> Result<(), LobbyModelError> {
        GameTypeId::new(self.id.as_str())?;
        if self.configuration_revision == 0 {
            return Err(LobbyModelError::InvalidRevision);
        }
        validate_presentation_name(&self.display_name)?;
        if self.mode_definition_id.0 == 0 {
            return Err(LobbyModelError::InvalidMode);
        }
        if self.map_preset_ids.is_empty()
            || self.map_preset_ids.len() > MAX_MAPS_PER_GAME_TYPE
            || self.map_preset_ids.iter().any(|id| id.0 == 0)
        {
            return Err(LobbyModelError::InvalidMapCount);
        }
        let mut maps = self.map_preset_ids.clone();
        maps.sort_unstable();
        if maps.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(LobbyModelError::DuplicateMap);
        }
        if self.team_count != 2 || self.players_per_team != 2 {
            return Err(LobbyModelError::InvalidTopology);
        }
        match self.rules_summary {
            AdvertisedRulesSummary::Wipeout {
                target_score,
                active_limit_ticks,
            } if target_score > 0 && active_limit_ticks > 0 => {}
            AdvertisedRulesSummary::HotZone {
                target_progress_ticks,
                active_limit_ticks,
            } if target_progress_ticks > 0 && active_limit_ticks > 0 => {}
            _ => return Err(LobbyModelError::InvalidRules),
        }
        Ok(())
    }
}

pub fn validate_catalog(game_types: &[AdvertisedGameType]) -> Result<(), LobbyModelError> {
    if game_types.is_empty() {
        return Err(LobbyModelError::EmptyCatalog);
    }
    if game_types.len() > MAX_GAME_TYPES {
        return Err(LobbyModelError::TooManyGameTypes);
    }
    let mut ids = Vec::with_capacity(game_types.len());
    for game_type in game_types {
        game_type.validate()?;
        ids.push(game_type.id.clone());
    }
    ids.sort_unstable();
    if ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(LobbyModelError::DuplicateGameTypeId);
    }
    Ok(())
}

pub fn validate_presentation_name(value: &str) -> Result<String, LobbyModelError> {
    let normalized: String = value.trim().nfc().collect();
    let graphemes = normalized.graphemes(true).count();
    if graphemes == 0
        || graphemes > MAX_DISPLAY_NAME_GRAPHEMES
        || normalized.len() > MAX_DISPLAY_NAME_BYTES
        || normalized.chars().any(invalid_text_character)
    {
        return Err(LobbyModelError::InvalidDisplayName);
    }
    Ok(normalized)
}

pub fn normalize_proposed_display_name(value: &str) -> Result<String, LobbyModelError> {
    let normalized: String = value.trim().nfc().collect();
    let graphemes = normalized.graphemes(true).count();
    if !(3..=MAX_PLAYER_NAME_GRAPHEMES).contains(&graphemes)
        || normalized.len() > MAX_PLAYER_NAME_BYTES
        || normalized.chars().any(invalid_text_character)
    {
        return Err(LobbyModelError::InvalidPlayerName);
    }
    Ok(normalized)
}

#[must_use]
pub fn generated_display_name(client_id: u64) -> String {
    format!("Brawler-{:04X}", client_id & 0xffff)
}

pub fn duplicate_display_name(base: &str, suffix: u32) -> Result<String, LobbyModelError> {
    let base = normalize_proposed_display_name(base)?;
    if suffix < 2 {
        return Ok(base);
    }
    let suffix = format!(" #{suffix}");
    let suffix_graphemes = suffix.graphemes(true).count();
    let max_base_graphemes = MAX_PLAYER_NAME_GRAPHEMES.saturating_sub(suffix_graphemes);
    let max_base_bytes = MAX_PLAYER_NAME_BYTES.saturating_sub(suffix.len());
    let mut truncated = String::new();
    for grapheme in base.graphemes(true).take(max_base_graphemes) {
        if truncated.len() + grapheme.len() > max_base_bytes {
            break;
        }
        truncated.push_str(grapheme);
    }
    while truncated.ends_with(char::is_whitespace) {
        truncated.pop();
    }
    if truncated.graphemes(true).count() < 3 {
        return Err(LobbyModelError::InvalidPlayerName);
    }
    truncated.push_str(&suffix);
    Ok(truncated)
}

fn invalid_text_character(character: char) -> bool {
    character.is_control() || matches!(character, '\u{2028}' | '\u{2029}')
}

pub fn canonical_catalog_bytes(
    game_types: &[AdvertisedGameType],
) -> Result<Vec<u8>, LobbyModelError> {
    validate_catalog(game_types)?;
    let mut bytes = Vec::with_capacity(256);
    bytes.extend_from_slice(b"brawler:lobby-catalog\0");
    bytes.push(u8::try_from(game_types.len()).expect("validated catalog count fits u8"));
    for game_type in game_types {
        put_string(&mut bytes, game_type.id.as_str());
        bytes.extend_from_slice(&game_type.configuration_revision.to_be_bytes());
        let display_name = validate_presentation_name(&game_type.display_name)?;
        put_string(&mut bytes, &display_name);
        bytes.extend_from_slice(&game_type.mode_definition_id.0.to_be_bytes());
        bytes.push(
            u8::try_from(game_type.map_preset_ids.len()).expect("validated map count fits u8"),
        );
        for map_id in &game_type.map_preset_ids {
            bytes.extend_from_slice(&map_id.0.to_be_bytes());
        }
        bytes.push(game_type.team_count);
        bytes.push(game_type.players_per_team);
        match game_type.rules_summary {
            AdvertisedRulesSummary::Wipeout {
                target_score,
                active_limit_ticks,
            } => {
                bytes.push(1);
                bytes.extend_from_slice(&target_score.to_be_bytes());
                bytes.extend_from_slice(&active_limit_ticks.to_be_bytes());
            }
            AdvertisedRulesSummary::HotZone {
                target_progress_ticks,
                active_limit_ticks,
            } => {
                bytes.push(2);
                bytes.extend_from_slice(&target_progress_ticks.to_be_bytes());
                bytes.extend_from_slice(&active_limit_ticks.to_be_bytes());
            }
        }
    }
    Ok(bytes)
}

pub fn catalog_revision(
    game_types: &[AdvertisedGameType],
) -> Result<CatalogRevision, LobbyModelError> {
    let digest: [u8; 32] = Sha256::digest(canonical_catalog_bytes(game_types)?).into();
    Ok(CatalogRevision(digest))
}

fn put_string(bytes: &mut Vec<u8>, value: &str) {
    let length = u16::try_from(value.len()).expect("validated lobby string fits u16");
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn golden_catalog() -> Vec<AdvertisedGameType> {
        vec![
            AdvertisedGameType {
                id: GameTypeId::new("wipeout-2v2").unwrap(),
                configuration_revision: 1,
                display_name: "Wipeout 2v2".to_string(),
                mode_definition_id: ModeDefinitionId(2),
                map_preset_ids: vec![MapPresetId(1)],
                team_count: 2,
                players_per_team: 2,
                rules_summary: AdvertisedRulesSummary::Wipeout {
                    target_score: 10,
                    active_limit_ticks: 10_800,
                },
            },
            AdvertisedGameType {
                id: GameTypeId::new("hot-zone-2v2").unwrap(),
                configuration_revision: 1,
                display_name: "Hot Zone 2v2".to_string(),
                mode_definition_id: ModeDefinitionId(3),
                map_preset_ids: vec![MapPresetId(2)],
                team_count: 2,
                players_per_team: 2,
                rules_summary: AdvertisedRulesSummary::HotZone {
                    target_progress_ticks: 1_800,
                    active_limit_ticks: 10_800,
                },
            },
        ]
    }

    #[test]
    fn canonical_catalog_matches_committed_golden_vector() {
        let catalog = golden_catalog();
        let bytes = canonical_catalog_bytes(&catalog).unwrap();
        assert_eq!(bytes.len(), 121);
        assert_eq!(
            catalog_revision(&catalog).unwrap().0,
            [
                0xd5, 0x4c, 0xc5, 0x84, 0x64, 0xa4, 0xe0, 0xbd, 0x2f, 0x06, 0x89, 0x5e, 0xfa, 0xd2,
                0xfe, 0xe6, 0x67, 0x63, 0x23, 0x3d, 0xa7, 0x18, 0xa8, 0x6a, 0xf2, 0x56, 0xd6, 0x1c,
                0x32, 0x26, 0x47, 0xec,
            ]
        );
    }

    #[test]
    fn catalog_validation_rejects_duplicate_ids_maps_and_bad_topology() {
        let mut catalog = golden_catalog();
        catalog[1].id = catalog[0].id.clone();
        assert_eq!(
            validate_catalog(&catalog),
            Err(LobbyModelError::DuplicateGameTypeId)
        );

        let mut catalog = golden_catalog();
        catalog[0].map_preset_ids.push(MapPresetId(1));
        assert_eq!(
            validate_catalog(&catalog),
            Err(LobbyModelError::DuplicateMap)
        );

        let mut catalog = golden_catalog();
        catalog[0].players_per_team = 3;
        assert_eq!(
            validate_catalog(&catalog),
            Err(LobbyModelError::InvalidTopology)
        );
    }

    #[test]
    fn names_normalize_and_duplicate_within_wire_bounds() {
        assert_eq!(
            normalize_proposed_display_name("  Cafe\u{301}  ").unwrap(),
            "Café"
        );
        assert_eq!(generated_display_name(0xab), "Brawler-00AB");
        let long = "abcdefghijklmnopqrstuvwx";
        let accepted = duplicate_display_name(long, 12).unwrap();
        assert!(accepted.graphemes(true).count() <= MAX_PLAYER_NAME_GRAPHEMES);
        assert!(accepted.len() <= MAX_PLAYER_NAME_BYTES);
        assert!(accepted.ends_with(" #12"));
    }

    #[test]
    fn every_advertised_field_changes_catalog_revision() {
        let original = golden_catalog();
        let revision = catalog_revision(&original).unwrap();
        let mut mutations = Vec::new();
        let mut changed = original.clone();
        changed[0].configuration_revision = 2;
        mutations.push(changed);
        let mut changed = original.clone();
        changed[0].display_name.push('!');
        mutations.push(changed);
        let mut changed = original.clone();
        changed[0].mode_definition_id = ModeDefinitionId(3);
        mutations.push(changed);
        let mut changed = original.clone();
        changed[0].map_preset_ids[0] = MapPresetId(2);
        mutations.push(changed);
        let mut changed = original.clone();
        changed[0].rules_summary = AdvertisedRulesSummary::Wipeout {
            target_score: 11,
            active_limit_ticks: 10_800,
        };
        mutations.push(changed);
        for changed in mutations {
            assert_ne!(catalog_revision(&changed).unwrap(), revision);
        }
    }
}
