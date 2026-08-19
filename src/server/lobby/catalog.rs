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
use std::collections::BTreeSet;

pub const OPERATOR_CATALOG_SCHEMA_VERSION: u16 = 1;
pub const MAX_OPERATOR_CATALOG_BYTES: usize = 16 * 1024;

#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedLobbyCatalog {
    pub server_name: String,
    pub revision: CatalogRevision,
    pub game_types: Vec<AdvertisedGameType>,
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
    rules_profile: String,
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
    let lifecycle = MatchLifecycleRules::default()
        .validate()
        .map_err(str::to_string)?;
    let wipeout = WipeoutRules::default().validate().map_err(str::to_string)?;
    let hot_zone = HotZoneRules::default()
        .validate_with(&lifecycle)
        .map_err(str::to_string)?;
    let mut advertised = Vec::with_capacity(operator.game_types.len());
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
            || entry.players_per_team != lifecycle.maximum_participants_per_team
            || entry.players_per_team != 2
        {
            return Err(format!(
                "game type {} exceeds the current exact 2v2 runtime capacity",
                id.as_str()
            ));
        }
        if entry.rules_profile != "standard" {
            return Err(format!(
                "game type {} has an unknown rules profile",
                id.as_str()
            ));
        }
        if entry.maps.is_empty() || entry.maps.len() > MAX_MAPS_PER_GAME_TYPE {
            return Err(format!(
                "game type {} must contain 1..={MAX_MAPS_PER_GAME_TYPE} maps",
                id.as_str()
            ));
        }

        let (mode_definition_id, requirements, rules_summary) = match entry.mode.as_str() {
            "wipeout" => (
                WIPEOUT_MODE_DEFINITION,
                MapLayoutRequirements::wipeout(),
                AdvertisedRulesSummary::Wipeout {
                    target_score: wipeout.target_score,
                    active_limit_ticks: lifecycle.active_limit_ticks,
                },
            ),
            "hot-zone" => (
                HOT_ZONE_MODE_DEFINITION,
                MapLayoutRequirements::hot_zone(),
                AdvertisedRulesSummary::HotZone {
                    target_progress_ticks: hot_zone.target_progress_ticks,
                    active_limit_ticks: lifecycle.active_limit_ticks,
                },
            ),
            _ => {
                return Err(format!("game type {} has an unknown mode key", id.as_str()));
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
            maps.resolve_preset(preset.id, MapInstanceId(1), &requirements)
                .map_err(|error| {
                    format!(
                        "game type {} map {key} is incompatible: {error}",
                        id.as_str()
                    )
                })?;
            map_ids.push(MapPresetId(preset.id.0));
        }

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
        player_id: crate::protocol::PlayerId(1),
        accepted_display_name: "Brawler-FFFF".to_string(),
        server_name: server_name.clone(),
        catalog_revision: revision,
        game_types: advertised.clone(),
    };
    let welcome_bytes = postcard::to_allocvec(&welcome)
        .map_err(|error| format!("lobby welcome encoding failed: {error}"))?;
    if welcome_bytes.len() > crate::protocol::MAX_LOBBY_WELCOME_BYTES {
        return Err("resolved lobby welcome exceeds 12 KiB".to_string());
    }
    Ok(ResolvedLobbyCatalog {
        server_name,
        revision,
        game_types: advertised,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = include_str!("../../../config/server/game-types.ron");

    #[test]
    fn checked_in_catalog_resolves_to_the_golden_advertisement() {
        let catalog = resolve_operator_catalog(VALID.as_bytes()).unwrap();
        assert_eq!(catalog.server_name, "Local Brawler");
        assert_eq!(catalog.game_types.len(), 2);
        assert_eq!(
            catalog.revision.0,
            [
                0xd5, 0x4c, 0xc5, 0x84, 0x64, 0xa4, 0xe0, 0xbd, 0x2f, 0x06, 0x89, 0x5e, 0xfa, 0xd2,
                0xfe, 0xe6, 0x67, 0x63, 0x23, 0x3d, 0xa7, 0x18, 0xa8, 0x6a, 0xf2, 0x56, 0xd6, 0x1c,
                0x32, 0x26, 0x47, 0xec,
            ]
        );
    }

    #[test]
    fn catalog_rejects_unknown_fields_modes_maps_profiles_and_3v3() {
        for invalid in [
            VALID.replace("schema_version: 1", "schema_version: 1, surprise: true"),
            VALID.replace("mode: \"wipeout\"", "mode: \"unknown\""),
            VALID.replace("crossroads-facility\"", "missing-map\""),
            VALID.replace("rules_profile: \"standard\"", "rules_profile: \"fast\""),
            VALID.replace("players_per_team: 2", "players_per_team: 3"),
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
