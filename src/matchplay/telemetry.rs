use super::{MatchId, MatchResult};
use crate::combat::{
    CombatOutcomeFact, CombatOutcomeKind, DistanceBand, TeamId, WeaponPresetId, WeaponTelemetry,
    WeaponTelemetryAggregate, WeaponTelemetryKey, distance_band,
};
use crate::content::GameplayContentFingerprint;
use crate::map::ResolvedMapIdentity;
use bevy::prelude::*;
use std::collections::{BTreeMap, VecDeque};

/// Bounded, process-lifetime counters for authoritative outcome facts rejected by match scoring.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MatchOutcomeDiagnostics {
    pub stale_tick: u64,
    pub duplicate_event: u64,
    pub unknown_or_wrong_match_target: u64,
    pub friendly_invalid_defeat: u64,
    pub stale_mode_outcome: u64,
    pub duplicate_mode_outcome: u64,
    pub wrong_match_outcome: u64,
    pub wrong_tick_outcome: u64,
}

/// Fully typed mode-specific terminal summary attached to one common match summary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeSummary {
    Wipeout(WipeoutSummary),
    HotZone(crate::matchplay::HotZoneSummary),
    Heist(crate::matchplay::HeistSummary),
}

/// Wipeout's terminal scores, preserving the pre-M09 report meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WipeoutSummary {
    pub final_scores: [u16; 2],
    pub target_score: u16,
    pub score_margin: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MatchSummary {
    pub match_id: MatchId,
    pub map_identity: Option<ResolvedMapIdentity>,
    pub content_fingerprint: Option<GameplayContentFingerprint>,
    pub rules_revision: u16,
    pub mode_definition_id: crate::map::ModeDefinitionId,
    pub mode_summary: ModeSummary,
    pub participants: Vec<MatchParticipantSummary>,
    pub active_started_at_tick: u64,
    pub active_duration_ticks: u64,
    pub time_to_first_hostile_damage_ticks: Option<u64>,
    pub result: MatchResult,
    pub applied_damage_by_distance: [u64; 3],
    pub credited_defeats_by_team: [u32; 2],
    pub suffered_deaths_by_team: [u32; 2],
    pub credited_defeats_by_preset: Vec<(WeaponPresetId, u32)>,
    pub suffered_deaths_by_preset: Vec<(WeaponPresetId, u32)>,
    pub participant_active_ticks_by_team: [u64; 2],
    pub participant_active_ticks_by_preset: Vec<(WeaponPresetId, u64)>,
    pub credited_defeats_per_participant_minute: [f64; 2],
    pub suffered_deaths_per_participant_minute: [f64; 2],
    pub credited_defeats_per_participant_minute_by_preset: Vec<(WeaponPresetId, f64)>,
    pub suffered_deaths_per_participant_minute_by_preset: Vec<(WeaponPresetId, f64)>,
    pub protected_contacts: u32,
    pub respawns: u32,
    pub fight_duration_ticks: Vec<u64>,
    pub respawn_to_defeat_ticks: Vec<u64>,
    pub movement_ticks_by_player: Vec<(u64, u64, u64)>,
    pub weapon_aggregates: Vec<(WeaponTelemetryKey, WeaponTelemetryAggregate)>,
    pub weapon_hostile_contact_rates: Vec<(WeaponTelemetryKey, f64)>,
    pub ability_telemetry: crate::abilities::AbilityTelemetry,
    pub disconnects: u32,
    pub dropped_records: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchParticipantSummary {
    pub player_id: u64,
    pub network_entity_id: u64,
    pub team: TeamId,
    pub selected_build: crate::builds::SelectedBuild,
    pub weapon_preset: Option<WeaponPresetId>,
    pub total_points: Option<u8>,
    pub ultimate_id: Option<crate::builds::UltimateDefinitionId>,
    pub passive_ids: Option<[crate::builds::PassiveDefinitionId; 2]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MatchTelemetryContext {
    pub map_identity: ResolvedMapIdentity,
    pub content_fingerprint: GameplayContentFingerprint,
    pub rules_revision: u16,
    pub participants: Vec<MatchParticipantSummary>,
}

#[derive(Clone, Debug, PartialEq)]
struct LiveMatchTelemetry {
    match_id: MatchId,
    active_start_tick: u64,
    first_hostile_damage_tick: Option<u64>,
    damage_by_distance: [u64; 3],
    defeats_by_team: [u32; 2],
    deaths_by_team: [u32; 2],
    defeats_by_preset: BTreeMap<WeaponPresetId, u32>,
    deaths_by_preset: BTreeMap<WeaponPresetId, u32>,
    protected_contacts: u32,
    respawns: u32,
    first_damage_by_target: BTreeMap<u64, u64>,
    life_start_by_target: BTreeMap<u64, u64>,
    fight_duration_ticks: Vec<u64>,
    respawn_to_defeat_ticks: Vec<u64>,
    movement_ticks_by_player: BTreeMap<u64, (u64, u64)>,
    participant_active_ticks_by_team: [u64; 2],
    participant_active_ticks_by_preset: BTreeMap<WeaponPresetId, u64>,
    disconnects: u32,
    weapon_aggregate_start: BTreeMap<WeaponTelemetryKey, WeaponTelemetryAggregate>,
    ability_telemetry_start: Option<crate::abilities::AbilityTelemetry>,
    context: Option<MatchTelemetryContext>,
    dropped_records_at_start: u64,
}

#[derive(Resource, Debug, Default)]
pub struct MatchTelemetry {
    pub records: VecDeque<CombatOutcomeFact>,
    pub summaries: VecDeque<MatchSummary>,
    pub dropped_records: u64,
    pub dropped_summaries: u64,
    live: Option<LiveMatchTelemetry>,
}

impl MatchTelemetry {
    pub fn begin(&mut self, match_id: MatchId, tick: u64) {
        if self
            .live
            .as_ref()
            .is_some_and(|live| live.match_id == match_id)
        {
            return;
        }
        self.live = Some(LiveMatchTelemetry {
            match_id,
            active_start_tick: tick,
            first_hostile_damage_tick: None,
            damage_by_distance: [0; 3],
            defeats_by_team: [0; 2],
            deaths_by_team: [0; 2],
            defeats_by_preset: BTreeMap::new(),
            deaths_by_preset: BTreeMap::new(),
            protected_contacts: 0,
            respawns: 0,
            first_damage_by_target: BTreeMap::new(),
            life_start_by_target: BTreeMap::new(),
            fight_duration_ticks: Vec::new(),
            respawn_to_defeat_ticks: Vec::new(),
            movement_ticks_by_player: BTreeMap::new(),
            participant_active_ticks_by_team: [0; 2],
            participant_active_ticks_by_preset: BTreeMap::new(),
            disconnects: 0,
            weapon_aggregate_start: BTreeMap::new(),
            ability_telemetry_start: None,
            context: None,
            dropped_records_at_start: self.dropped_records,
        });
    }

    pub fn begin_with_weapons(&mut self, match_id: MatchId, tick: u64, weapons: &WeaponTelemetry) {
        self.begin(match_id, tick);
        if let Some(live) = self.live.as_mut()
            && live.match_id == match_id
            && live.weapon_aggregate_start.is_empty()
        {
            live.weapon_aggregate_start
                .clone_from(&weapons.source_aggregates);
        }
    }

    #[cfg(feature = "server")]
    pub(crate) fn begin_with_sources(
        &mut self,
        match_id: MatchId,
        tick: u64,
        weapons: &WeaponTelemetry,
        abilities: &crate::abilities::AbilityTelemetry,
    ) {
        self.begin_with_weapons(match_id, tick, weapons);
        if let Some(live) = self.live.as_mut()
            && live.match_id == match_id
            && live.ability_telemetry_start.is_none()
        {
            live.ability_telemetry_start = Some(abilities.clone());
        }
    }

    #[cfg_attr(not(feature = "server"), allow(dead_code))]
    pub(crate) fn set_context(&mut self, context: MatchTelemetryContext) {
        if let Some(live) = self.live.as_mut()
            && live.context.is_none()
        {
            live.context = Some(context);
        }
    }

    pub fn record_participant_active_tick(&mut self, team: TeamId, preset: Option<WeaponPresetId>) {
        if let Some(live) = self.live.as_mut()
            && team.0 <= 1
        {
            let index = usize::from(team.0);
            live.participant_active_ticks_by_team[index] =
                live.participant_active_ticks_by_team[index].saturating_add(1);
            if let Some(preset) = preset {
                let ticks = live
                    .participant_active_ticks_by_preset
                    .entry(preset)
                    .or_default();
                *ticks = ticks.saturating_add(1);
            }
        }
    }

    pub fn record_respawn(&mut self, network_id: u64, tick: u64) {
        if let Some(live) = self.live.as_mut() {
            live.respawns = live.respawns.saturating_add(1);
            live.life_start_by_target.insert(network_id, tick);
            live.first_damage_by_target.remove(&network_id);
        }
    }

    pub fn record_movement(&mut self, player_id: u64, moved: bool) {
        if let Some(live) = self.live.as_mut() {
            let entry = live.movement_ticks_by_player.entry(player_id).or_default();
            entry.1 = entry.1.saturating_add(1);
            if moved {
                entry.0 = entry.0.saturating_add(1);
            }
        }
    }

    pub fn record_disconnects(&mut self, count: usize) {
        if let Some(live) = self.live.as_mut() {
            live.disconnects = live
                .disconnects
                .saturating_add(u32::try_from(count).unwrap_or(u32::MAX));
        }
    }

    pub fn record(&mut self, fact: CombatOutcomeFact, maximum_records: usize) {
        if self.records.len() == maximum_records {
            self.records.pop_front();
            self.dropped_records = self.dropped_records.saturating_add(1);
        }
        self.records.push_back(fact);
        let Some(live) = self.live.as_mut() else {
            return;
        };
        match fact.kind {
            CombatOutcomeKind::ProtectedContact => {
                live.protected_contacts = live.protected_contacts.saturating_add(1);
            }
            CombatOutcomeKind::Damage { amount }
                if fact
                    .source_team
                    .is_some_and(|source| source != fact.target_team) =>
            {
                live.first_hostile_damage_tick.get_or_insert(fact.tick);
                live.first_damage_by_target
                    .entry(fact.target_network_id.0)
                    .or_insert(fact.tick);
                let index = match distance_band(fact.engagement_distance) {
                    DistanceBand::Close => 0,
                    DistanceBand::Mid => 1,
                    DistanceBand::Long => 2,
                };
                live.damage_by_distance[index] =
                    live.damage_by_distance[index].saturating_add(u64::from(amount));
            }
            CombatOutcomeKind::Defeat => {
                let target_preset = live.context.as_ref().and_then(|context| {
                    context
                        .participants
                        .iter()
                        .find(|participant| {
                            participant.network_entity_id == fact.target_network_id.0
                        })
                        .and_then(|participant| participant.weapon_preset)
                });
                if let Some(first_damage) = live
                    .first_damage_by_target
                    .remove(&fact.target_network_id.0)
                {
                    live.fight_duration_ticks
                        .push(fact.tick.saturating_sub(first_damage));
                }
                let life_start = live
                    .life_start_by_target
                    .remove(&fact.target_network_id.0)
                    .unwrap_or(live.active_start_tick);
                live.respawn_to_defeat_ticks
                    .push(fact.tick.saturating_sub(life_start));
                if fact.target_team.0 <= 1 {
                    let index = usize::from(fact.target_team.0);
                    live.deaths_by_team[index] = live.deaths_by_team[index].saturating_add(1);
                }
                if let Some(preset) = target_preset {
                    let deaths = live.deaths_by_preset.entry(preset).or_default();
                    *deaths = deaths.saturating_add(1);
                }
                if let Some(source) = fact.source_team
                    && source.0 <= 1
                    && source != fact.target_team
                {
                    let index = usize::from(source.0);
                    live.defeats_by_team[index] = live.defeats_by_team[index].saturating_add(1);
                    if let Some(preset) = fact.preset_id {
                        let defeats = live.defeats_by_preset.entry(preset).or_default();
                        *defeats = defeats.saturating_add(1);
                    }
                }
            }
            CombatOutcomeKind::Damage { .. } | CombatOutcomeKind::DeployableDestroyed => {}
        }
    }

    #[cfg_attr(not(feature = "server"), allow(dead_code))]
    #[allow(
        clippy::cast_precision_loss,
        clippy::too_many_arguments,
        clippy::too_many_lines
    )]
    pub(crate) fn complete_with_mode(
        &mut self,
        tick: u64,
        mode_definition_id: crate::map::ModeDefinitionId,
        mode_summary: ModeSummary,
        result: MatchResult,
        maximum_summaries: usize,
        weapons: &WeaponTelemetry,
        abilities: &crate::abilities::AbilityTelemetry,
    ) {
        let Some(live) = self.live.take() else {
            return;
        };
        if self.summaries.len() == maximum_summaries {
            self.summaries.pop_front();
            self.dropped_summaries = self.dropped_summaries.saturating_add(1);
        }
        let weapon_aggregates: Vec<_> = weapons
            .source_aggregates
            .iter()
            .filter_map(|(key, aggregate)| {
                let start = live
                    .weapon_aggregate_start
                    .get(key)
                    .cloned()
                    .unwrap_or_default();
                let delta = aggregate_delta(aggregate, &start);
                (delta != WeaponTelemetryAggregate::default()).then_some((*key, delta))
            })
            .collect();
        let weapon_hostile_contact_rates = weapon_aggregates
            .iter()
            .map(|(key, aggregate)| {
                let rate = if aggregate.accepted_attacks == 0 {
                    0.0
                } else {
                    aggregate.attacks_with_hostile_contact as f64
                        / aggregate.accepted_attacks as f64
                };
                (*key, rate)
            })
            .collect();
        let credited_defeats_per_participant_minute_by_preset = per_preset_participant_minute(
            &live.defeats_by_preset,
            &live.participant_active_ticks_by_preset,
        );
        let suffered_deaths_per_participant_minute_by_preset = per_preset_participant_minute(
            &live.deaths_by_preset,
            &live.participant_active_ticks_by_preset,
        );
        self.summaries.push_back(MatchSummary {
            match_id: live.match_id,
            map_identity: live.context.as_ref().map(|context| context.map_identity),
            content_fingerprint: live
                .context
                .as_ref()
                .map(|context| context.content_fingerprint),
            rules_revision: live
                .context
                .as_ref()
                .map_or(0, |context| context.rules_revision),
            mode_definition_id,
            mode_summary,
            participants: live
                .context
                .as_ref()
                .map_or_else(Vec::new, |context| context.participants.clone()),
            active_started_at_tick: live.active_start_tick,
            active_duration_ticks: tick.saturating_sub(live.active_start_tick),
            time_to_first_hostile_damage_ticks: live
                .first_hostile_damage_tick
                .map(|damage| damage.saturating_sub(live.active_start_tick)),
            result,
            applied_damage_by_distance: live.damage_by_distance,
            credited_defeats_by_team: live.defeats_by_team,
            suffered_deaths_by_team: live.deaths_by_team,
            credited_defeats_by_preset: live.defeats_by_preset.into_iter().collect(),
            suffered_deaths_by_preset: live.deaths_by_preset.into_iter().collect(),
            participant_active_ticks_by_team: live.participant_active_ticks_by_team,
            participant_active_ticks_by_preset: live
                .participant_active_ticks_by_preset
                .into_iter()
                .collect(),
            credited_defeats_per_participant_minute: per_participant_minute(
                live.defeats_by_team,
                live.participant_active_ticks_by_team,
            ),
            suffered_deaths_per_participant_minute: per_participant_minute(
                live.deaths_by_team,
                live.participant_active_ticks_by_team,
            ),
            credited_defeats_per_participant_minute_by_preset,
            suffered_deaths_per_participant_minute_by_preset,
            protected_contacts: live.protected_contacts,
            respawns: live.respawns,
            fight_duration_ticks: live.fight_duration_ticks,
            respawn_to_defeat_ticks: live.respawn_to_defeat_ticks,
            movement_ticks_by_player: live
                .movement_ticks_by_player
                .into_iter()
                .map(|(player, (moving, eligible))| (player, moving, eligible))
                .collect(),
            weapon_aggregates,
            weapon_hostile_contact_rates,
            ability_telemetry: live
                .ability_telemetry_start
                .as_ref()
                .map_or_else(crate::abilities::AbilityTelemetry::default, |start| {
                    abilities.delta_since(start, live.active_start_tick)
                }),
            disconnects: live.disconnects,
            dropped_records: self
                .dropped_records
                .saturating_sub(live.dropped_records_at_start),
        });
    }
}

#[cfg_attr(not(feature = "server"), allow(dead_code))]
#[allow(clippy::cast_precision_loss)]
fn per_preset_participant_minute(
    counts: &BTreeMap<WeaponPresetId, u32>,
    active_ticks: &BTreeMap<WeaponPresetId, u64>,
) -> Vec<(WeaponPresetId, f64)> {
    active_ticks
        .iter()
        .map(|(preset, ticks)| {
            let count = counts.get(preset).copied().unwrap_or(0);
            let rate = if *ticks == 0 {
                0.0
            } else {
                f64::from(count) * 3_600.0 / *ticks as f64
            };
            (*preset, rate)
        })
        .collect()
}

#[cfg_attr(not(feature = "server"), allow(dead_code))]
#[allow(clippy::cast_precision_loss)]
fn per_participant_minute(counts: [u32; 2], active_ticks: [u64; 2]) -> [f64; 2] {
    std::array::from_fn(|index| {
        if active_ticks[index] == 0 {
            0.0
        } else {
            f64::from(counts[index]) * 3_600.0 / active_ticks[index] as f64
        }
    })
}

#[cfg_attr(not(feature = "server"), allow(dead_code))]
fn aggregate_delta(
    end: &WeaponTelemetryAggregate,
    start: &WeaponTelemetryAggregate,
) -> WeaponTelemetryAggregate {
    WeaponTelemetryAggregate {
        selections: end.selections.saturating_sub(start.selections),
        accepted_attacks: end.accepted_attacks.saturating_sub(start.accepted_attacks),
        emitted_deliveries: end
            .emitted_deliveries
            .saturating_sub(start.emitted_deliveries),
        hostile_delivery_contacts: end
            .hostile_delivery_contacts
            .saturating_sub(start.hostile_delivery_contacts),
        attacks_with_hostile_contact: end
            .attacks_with_hostile_contact
            .saturating_sub(start.attacks_with_hostile_contact),
        hostile_damage: end.hostile_damage.saturating_sub(start.hostile_damage),
        self_damage: end.self_damage.saturating_sub(start.self_damage),
        hostile_damage_events: end
            .hostile_damage_events
            .saturating_sub(start.hostile_damage_events),
        self_damage_events: end
            .self_damage_events
            .saturating_sub(start.self_damage_events),
        defeats: end.defeats.saturating_sub(start.defeats),
        close_hits: end.close_hits.saturating_sub(start.close_hits),
        mid_hits: end.mid_hits.saturating_sub(start.mid_hits),
        long_hits: end.long_hits.saturating_sub(start.long_hits),
        close_damage: end.close_damage.saturating_sub(start.close_damage),
        mid_damage: end.mid_damage.saturating_sub(start.mid_damage),
        long_damage: end.long_damage.saturating_sub(start.long_damage),
    }
}
