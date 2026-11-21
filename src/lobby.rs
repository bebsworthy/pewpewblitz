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

mod publishing;
mod queue;

pub use publishing::{
    MAX_LISTED_PUBLISHED_MAPS, MAX_PUBLISH_COOLDOWN_SECONDS, MAX_PUBLISHED_MAP_REASON_BYTES,
    MapPublishDecision, MapPublishOutcome, MapPublishRejection, MapPublishRequest,
    MapPublishRequestId, PUBLISHED_MAP_RECIPE_ID_BASE, PUBLISHED_MAP_RECIPE_REVISION,
    PublishedMapCredit, PublishedMapId, PublishedMapListing, PublishedMapState,
    PublishedMapSummary, PublishedMapsAction, PublishedMapsRejection, PublishedMapsReply,
    PublishedMapsRequest, PublishedMapsRequestId, PublishedMapsResult, WITHDRAWN_BY_AUTHOR_REASON,
};

pub use queue::{
    BeginMatchConnect, FormationAvailability, MAX_QUEUE_FORMATION_SIZE, MAX_QUEUE_OUTCOME_BYTES,
    MAX_QUEUE_RETRY_AFTER_MILLIS, MAX_QUEUE_TICKETS, MatchLoadingPhase, MatchRejoinStatus,
    MatchReservationId, MatchStartFailure, MatchmakingClientAction, MatchmakingClientMessage,
    MatchmakingServerMessage, MatchmakingServerPhase, PracticeRequestId, PracticeStartRejection,
    PracticeStartRequest, QueueCancelCommand, QueueClientMessage, QueueCommand,
    QueueCommandOutcome, QueueDecision, QueueJoinCommand, QueueMembership, QueuePoolRow,
    QueuePoolSnapshot, QueueRejection, QueueRequestId, QueueTicketId, ReservationStarted,
};

pub const MAX_GAME_TYPES: usize = 16;
pub const MAX_MAPS_PER_GAME_TYPE: usize = 8;
pub const MAX_PRACTICE_MAPS: usize = MAX_GAME_TYPES * MAX_MAPS_PER_GAME_TYPE;
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
    Takedown {
        target_score: u16,
        active_limit_ticks: u64,
    },
    TurfHold {
        target_progress_ticks: u16,
        active_limit_ticks: u64,
    },
    VaultBreak {
        safe_maximum_health: u16,
        active_limit_ticks: u64,
    },
    Zombie {
        pressure_stage_count: u8,
        active_limit_ticks: u64,
    },
    ShardRush {
        target_shards: u8,
        hold_ticks: u16,
        active_limit_ticks: u64,
    },
    LastStand {
        fighters: u8,
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
    #[serde(deserialize_with = "deserialize_practice_map_ids")]
    pub practice_map_preset_ids: Vec<MapPresetId>,
    pub map_authoring_limits: Option<crate::map::MapAuthoringLimits>,
    /// The operator mixes approved published maps into this game type's queue rotation, so
    /// Map Maker offers it as a Publish target (BRL-w9z1).
    pub community_maps: bool,
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

fn deserialize_practice_map_ids<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<MapPresetId>, D::Error> {
    deserialize_bounded_vec::<D, MapPresetId, MAX_PRACTICE_MAPS>(deserializer)
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
        if self.practice_map_preset_ids.is_empty()
            || self.practice_map_preset_ids.len() > MAX_PRACTICE_MAPS
            || self.practice_map_preset_ids.iter().any(|id| id.0 == 0)
        {
            return Err(LobbyModelError::InvalidMapCount);
        }
        let mut practice_maps = self.practice_map_preset_ids.clone();
        practice_maps.sort_unstable();
        if practice_maps.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(LobbyModelError::DuplicateMap);
        }
        if self
            .map_preset_ids
            .iter()
            .any(|id| !practice_maps.contains(id))
        {
            return Err(LobbyModelError::InvalidMapCount);
        }
        let supports_authoring = matches!(
            self.mode_definition_id,
            crate::map::TAKEDOWN_MODE_DEFINITION
                | crate::map::TURF_HOLD_MODE_DEFINITION
                | crate::map::VAULT_BREAK_MODE_DEFINITION
        );
        if self.map_authoring_limits != supports_authoring.then_some(crate::map::AUTHORING_LIMITS)
            || (self.community_maps && !supports_authoring)
        {
            return Err(LobbyModelError::InvalidMapCount);
        }
        let topology_valid = match self.rules_summary {
            AdvertisedRulesSummary::Zombie { .. } => {
                self.team_count == 1 && matches!(self.players_per_team, 2..=3)
            }
            AdvertisedRulesSummary::LastStand { fighters, .. } => {
                fighters == crate::matchplay::LAST_STAND_FIGHTERS
                    && self.team_count == fighters
                    && self.players_per_team == 1
            }
            _ => self.team_count == 2 && matches!(self.players_per_team, 1..=3),
        };
        if !topology_valid {
            return Err(LobbyModelError::InvalidTopology);
        }
        match self.rules_summary {
            AdvertisedRulesSummary::Takedown {
                target_score,
                active_limit_ticks,
            } if target_score > 0 && active_limit_ticks > 0 => {}
            AdvertisedRulesSummary::TurfHold {
                target_progress_ticks,
                active_limit_ticks,
            } if target_progress_ticks > 0 && active_limit_ticks > 0 => {}
            AdvertisedRulesSummary::VaultBreak {
                safe_maximum_health,
                active_limit_ticks,
            } if safe_maximum_health > 0 && active_limit_ticks > 0 => {}
            AdvertisedRulesSummary::Zombie {
                pressure_stage_count,
                active_limit_ticks,
            } if pressure_stage_count > 0 && active_limit_ticks > 0 => {}
            AdvertisedRulesSummary::ShardRush {
                target_shards,
                hold_ticks,
                active_limit_ticks,
            } if target_shards > 0 && hold_ticks > 0 && active_limit_ticks > 0 => {}
            AdvertisedRulesSummary::LastStand {
                active_limit_ticks, ..
            } if active_limit_ticks > 0 => {}
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

/// Why a name a player is entering now is rejected; its `Display` is the player-facing sentence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayNameRejection {
    Length,
    UnshowableCharacter,
}

impl core::fmt::Display for DisplayNameRejection {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Length => "Name must be 3 to 24 characters",
            Self::UnshowableCharacter => "Name has a character that can't be shown",
        })
    }
}

impl std::error::Error for DisplayNameRejection {}

/// The structural player-name rule: NFC, trimmed, 3..=24 graphemes, bounded bytes, no controls.
///
/// This is the rule for names that already exist: wire decoding, saved brawlers, persisted client
/// state, match admission, and lobby hellos (a reconnecting client re-sends its stored name). New
/// player input goes through [`normalize_new_display_name`] instead (BRL-aj2q).
pub fn normalize_proposed_display_name(value: &str) -> Result<String, LobbyModelError> {
    check_display_name(value, false).map_err(|_| LobbyModelError::InvalidPlayerName)
}

/// The strict rule for a name a player is entering now: the structural rule plus no code point
/// that no font can draw (private use, noncharacters, and format characters outside an emoji or
/// script joining sequence).
pub fn normalize_new_display_name(value: &str) -> Result<String, DisplayNameRejection> {
    check_display_name(value, true)
}

/// A rename of a stored name: an unchanged stored name keeps the structural rule, so a name that
/// predates the strict rule stays editable; any other text is new input.
pub fn normalize_renamed_display_name(
    value: &str,
    stored: &str,
) -> Result<String, DisplayNameRejection> {
    match normalize_proposed_display_name(value) {
        Ok(normalized) if normalized == stored => Ok(normalized),
        _ => normalize_new_display_name(value),
    }
}

fn check_display_name(value: &str, strict: bool) -> Result<String, DisplayNameRejection> {
    let normalized: String = value.trim().nfc().collect();
    if normalized.chars().any(invalid_text_character)
        || (strict && normalized.graphemes(true).any(unshowable_grapheme))
    {
        return Err(DisplayNameRejection::UnshowableCharacter);
    }
    let graphemes = normalized.graphemes(true).count();
    if !(3..=MAX_PLAYER_NAME_GRAPHEMES).contains(&graphemes)
        || normalized.len() > MAX_PLAYER_NAME_BYTES
    {
        return Err(DisplayNameRejection::Length);
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

/// Private-use (BMP area and planes 15-16) and noncharacter code points: never drawable.
fn undrawable_character(character: char) -> bool {
    let code = u32::from(character);
    matches!(code, 0xE000..=0xF8FF | 0xF_0000..=0x10_FFFF | 0xFDD0..=0xFDEF)
        || code & 0xFFFE == 0xFFFE
}

/// `General_Category=Cf` as of Unicode 16. The set is small and changes rarely, so a range table
/// avoids a Unicode property dependency in the shared crate (BRL-aj2q).
fn format_character(character: char) -> bool {
    matches!(
        u32::from(character),
        0x00AD
            | 0x0600..=0x0605
            | 0x061C
            | 0x06DD
            | 0x070F
            | 0x0890..=0x0891
            | 0x08E2
            | 0x180E
            | 0x200B..=0x200F
            | 0x202A..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x206F
            | 0xFEFF
            | 0xFFF9..=0xFFFB
            | 0x110BD
            | 0x110CD
            | 0x13430..=0x1343F
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0001
            | 0xE0020..=0xE007F
    )
}

const WAVING_BLACK_FLAG: char = '\u{1F3F4}';

/// Joiners and emoji presentation selectors draw nothing alone but are valid after a visible base:
/// ZWNJ/ZWJ (script joining, emoji ZWJ sequences), VS15/VS16, and tag characters after the black
/// flag (emoji subdivision flags).
fn permitted_joiner(character: char, base: char) -> bool {
    matches!(character, '\u{200C}' | '\u{200D}' | '\u{FE0E}' | '\u{FE0F}')
        || (base == WAVING_BLACK_FLAG && matches!(u32::from(character), 0xE0020..=0xE007F))
}

fn unshowable_grapheme(grapheme: &str) -> bool {
    let mut characters = grapheme.chars();
    let Some(base) = characters.next() else {
        return false;
    };
    if undrawable_character(base) || format_character(base) || permitted_joiner(base, base) {
        return true;
    }
    let visible_base = !base.is_whitespace();
    characters.any(|character| {
        undrawable_character(character)
            || if permitted_joiner(character, base) {
                !visible_base
            } else {
                format_character(character)
            }
    })
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
        bytes.push(
            u8::try_from(game_type.practice_map_preset_ids.len())
                .expect("validated Practice map count fits u8"),
        );
        for map_id in &game_type.practice_map_preset_ids {
            bytes.extend_from_slice(&map_id.0.to_be_bytes());
        }
        if let Some(limits) = game_type.map_authoring_limits {
            bytes.push(1);
            bytes.extend_from_slice(&limits.minimum_axis_cells.to_be_bytes());
            bytes.extend_from_slice(&limits.maximum_axis_cells.to_be_bytes());
            bytes.extend_from_slice(&limits.maximum_total_cells.to_be_bytes());
            bytes.extend_from_slice(&limits.maximum_source_items.to_be_bytes());
            bytes.extend_from_slice(&limits.maximum_expanded_placements.to_be_bytes());
            bytes.extend_from_slice(&limits.maximum_recipe_bytes.to_be_bytes());
        } else {
            bytes.push(0);
        }
        bytes.push(u8::from(game_type.community_maps));
        bytes.push(game_type.team_count);
        bytes.push(game_type.players_per_team);
        match game_type.rules_summary {
            AdvertisedRulesSummary::Takedown {
                target_score,
                active_limit_ticks,
            } => {
                bytes.push(1);
                bytes.extend_from_slice(&target_score.to_be_bytes());
                bytes.extend_from_slice(&active_limit_ticks.to_be_bytes());
            }
            AdvertisedRulesSummary::TurfHold {
                target_progress_ticks,
                active_limit_ticks,
            } => {
                bytes.push(2);
                bytes.extend_from_slice(&target_progress_ticks.to_be_bytes());
                bytes.extend_from_slice(&active_limit_ticks.to_be_bytes());
            }
            AdvertisedRulesSummary::VaultBreak {
                safe_maximum_health,
                active_limit_ticks,
            } => {
                bytes.push(3);
                bytes.extend_from_slice(&safe_maximum_health.to_be_bytes());
                bytes.extend_from_slice(&active_limit_ticks.to_be_bytes());
            }
            AdvertisedRulesSummary::Zombie {
                pressure_stage_count,
                active_limit_ticks,
            } => {
                bytes.push(4);
                bytes.push(pressure_stage_count);
                bytes.extend_from_slice(&active_limit_ticks.to_be_bytes());
            }
            AdvertisedRulesSummary::ShardRush {
                target_shards,
                hold_ticks,
                active_limit_ticks,
            } => {
                bytes.push(5);
                bytes.push(target_shards);
                bytes.extend_from_slice(&hold_ticks.to_be_bytes());
                bytes.extend_from_slice(&active_limit_ticks.to_be_bytes());
            }
            AdvertisedRulesSummary::LastStand {
                fighters,
                active_limit_ticks,
            } => {
                bytes.push(6);
                bytes.push(fighters);
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
                id: GameTypeId::new("takedown-2v2").unwrap(),
                configuration_revision: 1,
                display_name: "Takedown 2v2".to_string(),
                mode_definition_id: ModeDefinitionId(2),
                map_preset_ids: vec![MapPresetId(1)],
                practice_map_preset_ids: vec![MapPresetId(1)],
                map_authoring_limits: Some(crate::map::AUTHORING_LIMITS),
                community_maps: false,
                team_count: 2,
                players_per_team: 2,
                rules_summary: AdvertisedRulesSummary::Takedown {
                    target_score: 10,
                    active_limit_ticks: 10_800,
                },
            },
            AdvertisedGameType {
                id: GameTypeId::new("turf-hold-2v2").unwrap(),
                configuration_revision: 1,
                display_name: "Turf Hold 2v2".to_string(),
                mode_definition_id: ModeDefinitionId(3),
                map_preset_ids: vec![MapPresetId(2)],
                practice_map_preset_ids: vec![MapPresetId(2)],
                map_authoring_limits: Some(crate::map::AUTHORING_LIMITS),
                community_maps: false,
                team_count: 2,
                players_per_team: 2,
                rules_summary: AdvertisedRulesSummary::TurfHold {
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
        assert_eq!(bytes.len(), 167);
        assert_eq!(
            catalog_revision(&catalog).unwrap().0,
            [
                0xe5, 0x43, 0x96, 0xcf, 0x51, 0xf6, 0xe4, 0x57, 0xe3, 0xaa, 0xf1, 0x8e, 0x58, 0x36,
                0x6e, 0x5b, 0x3a, 0x24, 0x08, 0x04, 0xb2, 0xef, 0x0d, 0xe9, 0x75, 0xb8, 0x7b, 0xc4,
                0x21, 0xf4, 0x80, 0x85,
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
        catalog[0].players_per_team = 4;
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
    fn new_names_reject_code_points_no_font_can_draw() {
        for rejected in [
            "Ab\u{E000}c",   // private use, BMP
            "Ab\u{F0001}c",  // private use, plane 15
            "Ab\u{10FFFD}c", // private use, plane 16
            "Ab\u{FDD0}c",   // noncharacter
            "Ab\u{FFFE}c",   // noncharacter
            "Ab\u{1FFFF}c",  // plane noncharacter
            "\u{200D}Abc",   // lone ZWJ
            "Ab \u{200D}cd", // ZWJ on a space, no visible base
            "Ab\u{200B}c",   // zero-width space (Cf)
            "Ab\u{00AD}c",   // soft hyphen (Cf)
            "Ab\u{202E}c",   // bidi override (Cf)
            "\u{FE0F}Abc",   // lone VS16
            "Abc\u{E0067}",  // tag outside a flag sequence
        ] {
            assert_eq!(
                normalize_new_display_name(rejected),
                Err(DisplayNameRejection::UnshowableCharacter),
                "{rejected:?}"
            );
            assert!(
                normalize_proposed_display_name(rejected).is_ok(),
                "stored names keep the structural rule: {rejected:?}"
            );
        }
        for accepted in [
            "Café",
            "Zoë Ångström",
            "Сергей",
            "Αλέξης",
            "李小龍",
            "김민준",
            "محمد",
            "می\u{200C}خواهم", // Persian ZWNJ
            "Ace 🔥😎",
            "👩\u{200D}💻 Coder", // emoji ZWJ sequence
            "Red ❤\u{FE0F}",      // VS16
            "\u{1F3F4}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F} Eng",
        ] {
            assert!(normalize_new_display_name(accepted).is_ok(), "{accepted:?}");
        }
        assert_eq!(
            normalize_new_display_name("ab"),
            Err(DisplayNameRejection::Length)
        );
        assert_eq!(
            DisplayNameRejection::UnshowableCharacter.to_string(),
            "Name has a character that can't be shown"
        );
    }

    #[test]
    fn renaming_keeps_an_unchanged_stored_name() {
        let stored = "Ab\u{E000}c";
        assert_eq!(
            normalize_renamed_display_name(" Ab\u{E000}c ", stored).as_deref(),
            Ok(stored)
        );
        assert_eq!(
            normalize_renamed_display_name("Ab\u{E000}cd", stored),
            Err(DisplayNameRejection::UnshowableCharacter)
        );
        assert_eq!(
            normalize_renamed_display_name("Abcd", stored).as_deref(),
            Ok("Abcd")
        );
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
        changed[0].practice_map_preset_ids.push(MapPresetId(2));
        mutations.push(changed);
        let mut changed = original.clone();
        changed[0].practice_map_preset_ids.push(MapPresetId(3));
        mutations.push(changed);
        let mut changed = original.clone();
        changed[0].rules_summary = AdvertisedRulesSummary::Takedown {
            target_score: 11,
            active_limit_ticks: 10_800,
        };
        mutations.push(changed);
        for changed in mutations {
            assert_ne!(catalog_revision(&changed).unwrap(), revision);
        }
    }
}
