//! Server-only operator catalog parsing and authoritative gameplay resolution.

use crate::{
    lobby::{
        AdvertisedGameType, AdvertisedRulesSummary, CatalogRevision, GameTypeId, MAX_GAME_TYPES,
        MAX_MAPS_PER_GAME_TYPE, canonical_catalog_bytes, validate_catalog,
        validate_presentation_name,
    },
    map::{MapDimensionLimits, MapInstanceId, MapPresetId},
    matchplay::{HeistRules, HotZoneRules, MatchLifecycleRules, WipeoutRules},
};
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const OPERATOR_CATALOG_SCHEMA_VERSION: u16 = 4;
pub const MAX_OPERATOR_CATALOG_BYTES: usize = 16 * 1024;

#[derive(Resource, Clone, Debug, PartialEq)]
pub(crate) struct ResolvedLobbyCatalog {
    pub server_name: String,
    pub map_dimension_limits: MapDimensionLimits,
    pub revision: CatalogRevision,
    pub game_types: Vec<AdvertisedGameType>,
    pub brawler_catalog: crate::profiles::AdvertisedBrawlerCatalog,
    policy_revision: CatalogRevision,
    game_rules: BTreeMap<GameTypeId, ResolvedGameRules>,
    map_admission_revisions: BTreeMap<MapPresetId, u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedGameRules {
    pub objective_target: u16,
    pub match_duration_ticks: u64,
    pub countdown_ticks: u64,
    pub respawn_ticks: u64,
    pub spawn_protection_ticks: u64,
    pub completed_input_lock_ticks: u64,
    pub wipeout_recent_hostile_damage_credit_ticks: u64,
    pub heist_critical_health_percent: u8,
}

impl ResolvedLobbyCatalog {
    pub(crate) fn rules(&self, game_type_id: &GameTypeId) -> Option<ResolvedGameRules> {
        self.game_rules.get(game_type_id).copied()
    }

    pub(crate) fn map_admission_revision(&self, preset_id: MapPresetId) -> Option<u16> {
        self.map_admission_revisions.get(&preset_id).copied()
    }

    pub(crate) fn first_rules_for_mode(
        &self,
        mode_definition_id: crate::map::ModeDefinitionId,
    ) -> Option<ResolvedGameRules> {
        self.game_types
            .iter()
            .find(|game| game.mode_definition_id == mode_definition_id)
            .and_then(|game| self.rules(&game.id))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OperatorCatalog {
    schema_version: u16,
    server_name: String,
    common_lifecycle: OperatorCommonLifecyclePolicy,
    mode_policies: OperatorModePolicies,
    map_dimension_limits: MapDimensionLimits,
    game_types: Vec<OperatorGameType>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct OperatorCommonLifecyclePolicy {
    spawn_protection_ticks: u64,
    completed_input_lock_ticks: u64,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct OperatorModePolicies {
    wipeout: OperatorWipeoutPolicy,
    heist: OperatorHeistPolicy,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct OperatorWipeoutPolicy {
    recent_hostile_damage_credit_ticks: u64,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct OperatorHeistPolicy {
    critical_health_percent: u8,
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
    #[serde(default)]
    safe_health: u16,
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
    operator.map_dimension_limits.validate()?;
    if operator.game_types.is_empty() || operator.game_types.len() > MAX_GAME_TYPES {
        return Err(format!(
            "game-type catalog must contain 1..={MAX_GAME_TYPES} entries"
        ));
    }

    let maps = crate::map::MapContentCatalog::embedded()?;
    let map_dimension_limits = operator.map_dimension_limits;
    let map_admission_revisions: BTreeMap<_, _> = maps
        .presets
        .iter()
        .map(|preset| (preset.id, preset.admission_revision))
        .collect();
    let common_lifecycle = operator.common_lifecycle;
    let mode_policies = operator.mode_policies;
    let lifecycle = MatchLifecycleRules {
        spawn_protection_ticks: common_lifecycle.spawn_protection_ticks,
        completed_input_lock_ticks: common_lifecycle.completed_input_lock_ticks,
        ..MatchLifecycleRules::default()
    }
    .validate()
    .map_err(str::to_string)?;
    WipeoutRules {
        target_score: 1,
        recent_hostile_damage_credit_ticks: mode_policies
            .wipeout
            .recent_hostile_damage_credit_ticks,
    }
    .validate()
    .map_err(str::to_string)?;
    HeistRules {
        safe_maximum_health: 1,
        critical_health_percent: mode_policies.heist.critical_health_percent,
    }
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

        let mode = crate::modes::descriptor_for_key(&entry.mode).ok_or_else(|| {
            format!(
                "game type {} has an unknown mode or mismatched objective",
                id.as_str()
            )
        })?;
        let (objective_target, rules_summary) = match mode.mode {
            crate::config::GameMode::Wipeout
                if entry.capture_seconds == 0 && entry.safe_health == 0 =>
            {
                let target_score = entry.kills_to_win;
                WipeoutRules {
                    target_score,
                    recent_hostile_damage_credit_ticks: mode_policies
                        .wipeout
                        .recent_hostile_damage_credit_ticks,
                }
                .validate()
                .map_err(|error| {
                    format!("game type {} has invalid objective: {error}", id.as_str())
                })?;
                (
                    target_score,
                    AdvertisedRulesSummary::Wipeout {
                        target_score,
                        active_limit_ticks: match_duration_ticks,
                    },
                )
            }
            crate::config::GameMode::HotZone
                if entry.kills_to_win == 0 && entry.safe_health == 0 =>
            {
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
                    target_progress_ticks,
                    AdvertisedRulesSummary::HotZone {
                        target_progress_ticks,
                        active_limit_ticks: match_duration_ticks,
                    },
                )
            }
            crate::config::GameMode::Heist
                if entry.kills_to_win == 0
                    && entry.capture_seconds == 0
                    && entry.safe_health > 0 =>
            {
                let safe_maximum_health = entry.safe_health;
                HeistRules {
                    safe_maximum_health,
                    critical_health_percent: mode_policies.heist.critical_health_percent,
                }
                .validate()
                .map_err(|error| {
                    format!("game type {} has invalid objective: {error}", id.as_str())
                })?;
                (
                    safe_maximum_health,
                    AdvertisedRulesSummary::Heist {
                        safe_maximum_health,
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
        let mode_definition_id = mode.definition_id;

        let mut map_ids = Vec::with_capacity(entry.maps.len());
        let mut unique_maps = BTreeSet::new();
        for key in entry.maps {
            let preset = maps.presets.iter().find(|preset| preset.key == key);
            let preset_id = preset
                .map(|preset| preset.id)
                .ok_or_else(|| format!("game type {} has an unknown map key {key}", id.as_str()))?;
            if !unique_maps.insert(preset_id) {
                return Err(format!(
                    "game type {} contains a duplicate map",
                    id.as_str()
                ));
            }
            let preset = preset.expect("resolved preset exists");
            map_dimension_limits
                .validate_dimensions(preset.recipe.dimensions)
                .map_err(|error| {
                    format!(
                        "game type {} map {key} is outside the configured map dimensions: {error}",
                        id.as_str()
                    )
                })?;
            if !mode.accepts_map(preset.recipe.mode_definition_id) {
                return Err(format!(
                    "game type {} map {key} is incompatible with its mode",
                    id.as_str()
                ));
            }
            let resolved = maps
                .resolve_preset(preset.id, MapInstanceId(1))
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
                .validate_against_spawn_catalog(&crate::map::SpawnPointCatalog(
                    resolved.spawn_points_by_team,
                ))
                .map_err(|error| {
                    format!(
                        "game type {} map {key} lacks topology capacity: {error}",
                        id.as_str()
                    )
                })?;
            map_ids.push(preset_id);
        }

        game_rules.insert(
            id.clone(),
            ResolvedGameRules {
                objective_target,
                match_duration_ticks,
                countdown_ticks,
                respawn_ticks,
                spawn_protection_ticks: common_lifecycle.spawn_protection_ticks,
                completed_input_lock_ticks: common_lifecycle.completed_input_lock_ticks,
                wipeout_recent_hostile_damage_credit_ticks: mode_policies
                    .wipeout
                    .recent_hostile_damage_credit_ticks,
                heist_critical_health_percent: mode_policies.heist.critical_health_percent,
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
    let revision_material = canonical_catalog_bytes(&advertised)
        .map_err(|error| format!("catalog revision failed: {error}"))?;
    let revision = CatalogRevision(Sha256::digest(revision_material).into());
    let policy_material = postcard::to_allocvec(&(
        OPERATOR_CATALOG_SCHEMA_VERSION,
        common_lifecycle,
        mode_policies,
    ))
    .map_err(|error| format!("catalog policy revision failed: {error}"))?;
    let policy_revision = CatalogRevision(Sha256::digest(policy_material).into());
    let brawler_catalog = crate::profiles::AdvertisedBrawlerCatalog::from_content(
        &crate::builds::BuildCatalog::embedded()?,
        &crate::combat::WeaponCatalog::embedded()?,
    )?;
    let welcome = crate::protocol::LobbyJoinOutcome::Accepted {
        logical_server_id: 1,
        player_id: crate::protocol::PlayerId(1),
        accepted_display_name: "Brawler-FFFF".to_string(),
        server_name: server_name.clone(),
        catalog_revision: revision,
        game_types: advertised.clone(),
        brawler_catalog: Box::new(brawler_catalog.clone()),
        profile: Box::new(crate::profiles::ProfileSnapshot::empty(
            crate::profiles::AccountId::new(1).expect("constant account ID is nonzero"),
        )),
    };
    let welcome_bytes = postcard::to_allocvec(&welcome)
        .map_err(|error| format!("lobby welcome encoding failed: {error}"))?;
    if welcome_bytes.len() > crate::protocol::MAX_LOBBY_WELCOME_BYTES {
        return Err("resolved lobby welcome exceeds 32 KiB".to_string());
    }
    Ok(ResolvedLobbyCatalog {
        server_name,
        map_dimension_limits,
        revision,
        game_types: advertised,
        brawler_catalog,
        policy_revision,
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
        assert_eq!(catalog.map_dimension_limits, MapDimensionLimits::default());
        assert_eq!(catalog.game_types.len(), 9);
        let wipeout_three_vs_three = &catalog.game_types[3];
        assert_eq!(
            wipeout_three_vs_three.display_name,
            "Verdant Crossfire Wipeout 3v3"
        );
        assert_eq!(wipeout_three_vs_three.map_preset_ids, vec![MapPresetId(10)]);
        assert_eq!(wipeout_three_vs_three.configuration_revision, 4);
        let hot_zone_one_vs_one = &catalog.game_types[4];
        assert_eq!(hot_zone_one_vs_one.id.as_str(), "hot-zone-1v1");
        assert_eq!(
            hot_zone_one_vs_one.display_name,
            "Feature Yard Hot Zone 1v1"
        );
        assert_eq!(hot_zone_one_vs_one.map_preset_ids, vec![MapPresetId(8)]);
        assert_eq!(hot_zone_one_vs_one.players_per_team, 1);
        let hot_zone_three_vs_three = catalog
            .game_types
            .iter()
            .find(|game| game.id.as_str() == "hot-zone-3v3")
            .unwrap();
        assert_eq!(hot_zone_three_vs_three.configuration_revision, 5);
        assert_eq!(
            hot_zone_three_vs_three.map_preset_ids,
            vec![MapPresetId(11)]
        );
        let wipeout_two_vs_two = catalog
            .game_types
            .iter()
            .find(|game| game.id.as_str() == "wipeout-2v2")
            .unwrap();
        assert_eq!(wipeout_two_vs_two.configuration_revision, 3);
        assert_eq!(wipeout_two_vs_two.map_preset_ids, vec![MapPresetId(7)]);
        assert_eq!(wipeout_two_vs_two.players_per_team, 2);
        assert_eq!(
            wipeout_two_vs_two.rules_summary,
            AdvertisedRulesSummary::Wipeout {
                target_score: 10,
                active_limit_ticks: MatchLifecycleRules::default().active_limit_ticks,
            }
        );
        assert_eq!(
            catalog.rules(&wipeout_two_vs_two.id),
            Some(ResolvedGameRules {
                objective_target: 10,
                match_duration_ticks: 10_800,
                countdown_ticks: 180,
                respawn_ticks: 180,
                spawn_protection_ticks: 90,
                completed_input_lock_ticks: 60,
                wipeout_recent_hostile_damage_credit_ticks: 300,
                heist_critical_health_percent: 25,
            })
        );
        let heist = catalog
            .game_types
            .iter()
            .filter(|game| game.mode_definition_id == crate::map::HEIST_MODE_DEFINITION)
            .collect::<Vec<_>>();
        assert_eq!(heist.len(), 3);
        assert_eq!(heist[0].map_preset_ids, vec![MapPresetId(9)]);
        assert_eq!(heist[2].players_per_team, 3);
        assert_eq!(heist[2].configuration_revision, 6);
        assert_eq!(heist[2].map_preset_ids, vec![MapPresetId(12)]);
        assert_eq!(
            catalog.revision.0,
            [
                0x4f, 0xe0, 0x8f, 0x5b, 0x69, 0xcb, 0x3d, 0x54, 0xac, 0x96, 0x0e, 0x68, 0x13, 0xac,
                0x4a, 0x5c, 0x35, 0x18, 0xca, 0xfb, 0xe1, 0x44, 0x76, 0x72, 0xab, 0x55, 0xc3, 0x9c,
                0xb4, 0x49, 0x88, 0x32,
            ]
        );
    }

    #[test]
    fn policy_only_changes_resolve_and_change_the_private_policy_revision() {
        let baseline = resolve_operator_catalog(VALID.as_bytes()).unwrap();
        let changed_source = VALID
            .replace("spawn_protection_ticks: 90", "spawn_protection_ticks: 91")
            .replace(
                "recent_hostile_damage_credit_ticks: 300",
                "recent_hostile_damage_credit_ticks: 301",
            )
            .replace("critical_health_percent: 25", "critical_health_percent: 26");
        let changed = resolve_operator_catalog(changed_source.as_bytes()).unwrap();
        let rules = changed
            .first_rules_for_mode(crate::map::WIPEOUT_MODE_DEFINITION)
            .unwrap();

        assert_eq!(rules.spawn_protection_ticks, 91);
        assert_eq!(rules.wipeout_recent_hostile_damage_credit_ticks, 301);
        assert_eq!(rules.heist_critical_health_percent, 26);
        assert_eq!(changed.revision, baseline.revision);
        assert_ne!(changed.policy_revision, baseline.policy_revision);
    }

    #[test]
    fn catalog_rejects_unknown_fields_modes_maps_objectives_and_unsupported_topology() {
        for invalid in [
            VALID.replace("schema_version: 4", "schema_version: 4, surprise: true"),
            VALID.replace("mode: \"wipeout\"", "mode: \"unknown\""),
            VALID.replace("feature-yard-wipeout\"", "missing-map\""),
            VALID.replace("players_per_team: 2", "players_per_team: 4"),
            VALID.replace("kills_to_win: 10", "kills_to_win: 0"),
            VALID.replace("capture_seconds: 30", "kills_to_win: 10"),
            VALID.replace("countdown_seconds: 3", "countdown_seconds: 0"),
            VALID.replace("spawn_protection_ticks: 90", "spawn_protection_ticks: 0"),
            VALID.replace(
                "completed_input_lock_ticks: 60",
                "completed_input_lock_ticks: 0",
            ),
            VALID.replace(
                "recent_hostile_damage_credit_ticks: 300",
                "recent_hostile_damage_credit_ticks: 0",
            ),
            VALID.replace(
                "critical_health_percent: 25",
                "critical_health_percent: 100",
            ),
            VALID.replace("minimum_width: 20", "minimum_width: 0"),
            VALID.replace("maximum_width: 512", "maximum_width: 513"),
            VALID.replace("minimum_height: 20", "minimum_height: 513"),
        ] {
            assert!(resolve_operator_catalog(invalid.as_bytes()).is_err());
        }
    }

    #[test]
    fn catalog_applies_configured_map_dimension_limits_to_advertised_maps() {
        let excludes_feature_yard = VALID.replace("minimum_width: 20", "minimum_width: 65");
        let error = resolve_operator_catalog(excludes_feature_yard.as_bytes()).unwrap_err();
        assert!(error.contains("outside the configured map dimensions"));
    }

    #[test]
    fn catalog_rejects_duplicate_ids_and_maps_as_one_unit() {
        let duplicate_id = VALID.replace("hot-zone-2v2", "wipeout-2v2");
        assert!(resolve_operator_catalog(duplicate_id.as_bytes()).is_err());
        let duplicate_map = VALID.replace(
            "maps: [\"feature-yard-wipeout\"]",
            "maps: [\"feature-yard-wipeout\", \"feature-yard-wipeout\"]",
        );
        assert!(resolve_operator_catalog(duplicate_map.as_bytes()).is_err());
    }
}
