//! Server-only operator catalog parsing and authoritative gameplay resolution.

use crate::{
    lobby::{
        AdvertisedGameType, AdvertisedRulesSummary, CatalogRevision, GameTypeId, MAX_GAME_TYPES,
        MAX_MAPS_PER_GAME_TYPE, catalog_revision, validate_catalog, validate_presentation_name,
    },
    map::{
        HOT_ZONE_MODE_DEFINITION, MapContentCatalog, MapInstanceId, MapLayoutRequirements,
        MapPresetId, WIPEOUT_MODE_DEFINITION,
    },
    matchplay::{HotZoneRules, MatchLifecycleRules, WipeoutRules},
};
use bevy::prelude::Resource;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

pub const OPERATOR_CATALOG_SCHEMA_VERSION: u16 = 1;
pub const MAX_OPERATOR_CATALOG_BYTES: usize = 16 * 1024;

#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedLobbyCatalog {
    pub server_name: String,
    pub revision: CatalogRevision,
    pub game_types: Vec<AdvertisedGameType>,
    game_rules: BTreeMap<GameTypeId, ResolvedGameRules>,
    map_admission_revisions: BTreeMap<MapPresetId, u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedGameRules {
    pub objective_target: u16,
    pub match_duration_ticks: u64,
    pub countdown_ticks: u64,
    pub respawn_ticks: u64,
}

impl ResolvedLobbyCatalog {
    pub(crate) fn rules(&self, game_type_id: &GameTypeId) -> Option<ResolvedGameRules> {
        self.game_rules.get(game_type_id).copied()
    }

    pub(crate) fn map_admission_revision(&self, preset_id: MapPresetId) -> Option<u16> {
        self.map_admission_revisions.get(&preset_id).copied()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OperatorCatalog {
    schema_version: u16,
    server_name: String,
    game_types: Vec<OperatorGameType>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OperatorGameType {
    id: String,
    revision: u32,
    name: String,
    mode: String,
    maps: Vec<String>,
    teams: u8,
    players_per_team: u8,
    #[serde(default)]
    kills_to_win: u16,
    #[serde(default)]
    capture_seconds: u16,
    match_duration_seconds: u16,
    countdown_seconds: u16,
    respawn_seconds: u16,
}

#[allow(
    clippy::too_many_lines,
    reason = "one fail-closed startup transaction parses, resolves, and sizes the immutable catalog"
)]
pub(crate) fn resolve_operator_catalog(bytes: &[u8]) -> Result<ResolvedLobbyCatalog, String> {
    if bytes.len() > MAX_OPERATOR_CATALOG_BYTES {
        return Err("game-type catalog exceeds 16 KiB".to_string());
    }
    let source = std::str::from_utf8(bytes)
        .map_err(|_| "game-type catalog must be valid UTF-8".to_string())?;
    let operator: OperatorCatalog = ron::from_str(source)
        .map_err(|error| format!("game-type catalog parse failed: {error}"))?;
    if operator.schema_version != OPERATOR_CATALOG_SCHEMA_VERSION {
        return Err("unsupported game-type catalog schema".to_string());
    }
    let server_name = validate_presentation_name(&operator.server_name)
        .map_err(|error| format!("invalid server name: {error}"))?;
    if operator.game_types.is_empty() || operator.game_types.len() > MAX_GAME_TYPES {
        return Err(format!(
            "game-type catalog must contain 1..={MAX_GAME_TYPES} entries"
        ));
    }

    let maps = MapContentCatalog::embedded()?;
    let map_admission_revisions = maps
        .presets
        .iter()
        .map(|preset| (preset.id, preset.admission_revision))
        .collect();
    let lifecycle = MatchLifecycleRules::default()
        .validate()
        .map_err(str::to_string)?;
    let mut advertised = Vec::with_capacity(operator.game_types.len());
    let mut game_rules = BTreeMap::new();
    for entry in operator.game_types {
        let id =
            GameTypeId::new(entry.id).map_err(|error| format!("invalid game-type ID: {error}"))?;
        let display_name = validate_presentation_name(&entry.name)
            .map_err(|error| format!("invalid game-type name: {error}"))?;
        if entry.revision == 0 {
            return Err(format!("game type {} has a zero revision", id.as_str()));
        }
        if entry.teams != lifecycle.team_count
            || entry.teams != 2
            || !matches!(entry.players_per_team, 1..=3)
        {
            return Err(format!(
                "game type {} must use the supported exact 1v1, 2v2, or 3v3 topology",
                id.as_str()
            ));
        }
        let seconds_to_ticks = |seconds: u16| {
            u64::from(seconds)
                .checked_mul(crate::timing::SIMULATION_TICK_HZ)
                .filter(|ticks| *ticks > 0)
                .ok_or_else(|| format!("game type {} has invalid timing", id.as_str()))
        };
        let match_duration_ticks = seconds_to_ticks(entry.match_duration_seconds)?;
        let countdown_ticks = seconds_to_ticks(entry.countdown_seconds)?;
        let respawn_ticks = seconds_to_ticks(entry.respawn_seconds)?;
        let entry_lifecycle = MatchLifecycleRules {
            active_limit_ticks: match_duration_ticks,
            countdown_ticks,
            respawn_delay_ticks: respawn_ticks,
            ..lifecycle
        }
        .validate()
        .map_err(|error| format!("game type {} has invalid timing: {error}", id.as_str()))?;
        if entry.maps.is_empty() || entry.maps.len() > MAX_MAPS_PER_GAME_TYPE {
            return Err(format!(
                "game type {} must contain 1..={MAX_MAPS_PER_GAME_TYPE} maps",
                id.as_str()
            ));
        }

        let (mode_definition_id, requirements, objective_target, rules_summary) =
            match entry.mode.as_str() {
                "wipeout" if entry.capture_seconds == 0 => {
                    let target_score = entry.kills_to_win;
                    WipeoutRules { target_score }.validate().map_err(|error| {
                        format!("game type {} has invalid objective: {error}", id.as_str())
                    })?;
                    (
                        WIPEOUT_MODE_DEFINITION,
                        MapLayoutRequirements::wipeout(),
                        target_score,
                        AdvertisedRulesSummary::Wipeout {
                            target_score,
                            active_limit_ticks: match_duration_ticks,
                        },
                    )
                }
                "hot-zone" if entry.kills_to_win == 0 => {
                    let capture_seconds = entry.capture_seconds;
                    let target_progress_ticks = u16::try_from(seconds_to_ticks(capture_seconds)?)
                        .map_err(|_| {
                        format!("game type {} capture duration is too long", id.as_str())
                    })?;
                    HotZoneRules {
                        target_progress_ticks,
                    }
                    .validate_with(&entry_lifecycle)
                    .map_err(|error| {
                        format!("game type {} has invalid objective: {error}", id.as_str())
                    })?;
                    (
                        HOT_ZONE_MODE_DEFINITION,
                        MapLayoutRequirements::hot_zone(),
                        target_progress_ticks,
                        AdvertisedRulesSummary::HotZone {
                            target_progress_ticks,
                            active_limit_ticks: match_duration_ticks,
                        },
                    )
                }
                _ => {
                    return Err(format!(
                        "game type {} has an unknown mode or mismatched objective",
                        id.as_str()
                    ));
                }
            };

        let mut map_ids = Vec::with_capacity(entry.maps.len());
        let mut unique_maps = BTreeSet::new();
        for key in entry.maps {
            let preset = maps
                .presets
                .iter()
                .find(|preset| preset.key == key)
                .ok_or_else(|| format!("game type {} has an unknown map key {key}", id.as_str()))?;
            if !unique_maps.insert(preset.id) {
                return Err(format!(
                    "game type {} contains a duplicate map",
                    id.as_str()
                ));
            }
            let snapshot = maps
                .resolve_preset(preset.id, MapInstanceId(1), &requirements)
                .map_err(|error| {
                    format!(
                        "game type {} map {key} is incompatible: {error}",
                        id.as_str()
                    )
                })?;
            let capacity =
                crate::matchplay::ResolvedMatchCapacity::from_rules(&MatchLifecycleRules {
                    minimum_participants_per_team: entry.players_per_team,
                    maximum_participants_per_team: entry.players_per_team,
                    ..entry_lifecycle
                })
                .ok_or_else(|| format!("game type {} has invalid capacity", id.as_str()))?;
            capacity
                .validate_against_map(&snapshot.snapshot)
                .map_err(|error| {
                    format!(
                        "game type {} map {key} lacks topology capacity: {error}",
                        id.as_str()
                    )
                })?;
            map_ids.push(MapPresetId(preset.id.0));
        }

        game_rules.insert(
            id.clone(),
            ResolvedGameRules {
                objective_target,
                match_duration_ticks,
                countdown_ticks,
                respawn_ticks,
            },
        );
        advertised.push(AdvertisedGameType {
            id,
            configuration_revision: entry.revision,
            display_name,
            mode_definition_id,
            map_preset_ids: map_ids,
            team_count: entry.teams,
            players_per_team: entry.players_per_team,
            rules_summary,
        });
    }
    validate_catalog(&advertised)
        .map_err(|error| format!("resolved game-type catalog is invalid: {error}"))?;
    let revision = catalog_revision(&advertised)
        .map_err(|error| format!("catalog revision failed: {error}"))?;
    let welcome = crate::protocol::LobbyJoinOutcome::Accepted {
        logical_server_id: 1,
        player_id: crate::protocol::PlayerId(1),
        accepted_display_name: "Brawler-FFFF".to_string(),
        server_name: server_name.clone(),
        catalog_revision: revision,
        game_types: advertised.clone(),
        profile: crate::profiles::ProfileSnapshot::empty(
            crate::profiles::AccountId::new(1).expect("constant account ID is nonzero"),
        ),
    };
    let welcome_bytes = postcard::to_allocvec(&welcome)
        .map_err(|error| format!("lobby welcome encoding failed: {error}"))?;
    if welcome_bytes.len() > crate::protocol::MAX_LOBBY_WELCOME_BYTES {
        return Err("resolved lobby welcome exceeds 32 KiB".to_string());
    }
    Ok(ResolvedLobbyCatalog {
        server_name,
        revision,
        game_types: advertised,
        game_rules,
        map_admission_revisions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = include_str!("../../../config/server/game-types.ron");

    #[test]
    fn checked_in_catalog_resolves_to_the_golden_advertisement() {
        let catalog = resolve_operator_catalog(VALID.as_bytes()).unwrap();
        assert_eq!(catalog.server_name, "Local PewPew Blitz");
        assert_eq!(catalog.game_types.len(), 4);
        let first_blood = &catalog.game_types[3];
        assert_eq!(first_blood.display_name, "First Blood");
        assert_eq!(first_blood.configuration_revision, 2);
        assert_eq!(first_blood.map_preset_ids, vec![MapPresetId(3)]);
        assert_eq!(first_blood.players_per_team, 1);
        assert_eq!(
            first_blood.rules_summary,
            AdvertisedRulesSummary::Wipeout {
                target_score: 1,
                active_limit_ticks: MatchLifecycleRules::default().active_limit_ticks,
            }
        );
        assert_eq!(
            catalog.rules(&first_blood.id),
            Some(ResolvedGameRules {
                objective_target: 1,
                match_duration_ticks: 10_800,
                countdown_ticks: 180,
                respawn_ticks: 180,
            })
        );
        assert_eq!(
            catalog.revision.0,
            [
                0x7b, 0xd1, 0xef, 0x3b, 0x3c, 0xd1, 0xe0, 0xc1, 0x8d, 0x6c, 0x5e, 0xb1, 0x5a, 0xdb,
                0x52, 0x3b, 0xa9, 0xe1, 0x4f, 0x88, 0x4e, 0x92, 0xaf, 0x11, 0x68, 0xfb, 0x17, 0x9e,
                0x87, 0xac, 0xb4, 0xaa,
            ]
        );
    }

    #[test]
    fn catalog_rejects_unknown_fields_modes_maps_objectives_and_unsupported_topology() {
        for invalid in [
            VALID.replace("schema_version: 1", "schema_version: 1, surprise: true"),
            VALID.replace("mode: \"wipeout\"", "mode: \"unknown\""),
            VALID.replace("crossroads-facility\"", "missing-map\""),
            VALID.replace("players_per_team: 2", "players_per_team: 4"),
            VALID.replace("kills_to_win: 1", "kills_to_win: 0"),
            VALID.replace("capture_seconds: 30", "kills_to_win: 10"),
            VALID.replace("countdown_seconds: 3", "countdown_seconds: 0"),
        ] {
            assert!(resolve_operator_catalog(invalid.as_bytes()).is_err());
        }
    }

    #[test]
    fn catalog_rejects_duplicate_ids_and_maps_as_one_unit() {
        let duplicate_id = VALID.replace("hot-zone-2v2", "wipeout-2v2");
        assert!(resolve_operator_catalog(duplicate_id.as_bytes()).is_err());
        let duplicate_map = VALID.replace(
            "maps: [\"crossroads-facility\"]",
            "maps: [\"crossroads-facility\", \"crossroads-facility\"]",
        );
        assert!(resolve_operator_catalog(duplicate_map.as_bytes()).is_err());
    }
}
