//! Continuous process, transport, participant, and gameplay aggregate sampling.

use super::{
    GameplayAggregatesV1, ManifestParticipant, ProcessDiagnosticsState, participant_build_identity,
};
use bevy::app::AppExit;
use bevy::prelude::*;
use lightyear::prelude::Link;
use std::time::{Duration, Instant};

pub(super) fn begin_fixed_tick_observation(mut state: ResMut<ProcessDiagnosticsState>) {
    state.tick_started_at = Some(Instant::now());
}

pub(super) fn finish_fixed_tick_observation(mut state: ResMut<ProcessDiagnosticsState>) {
    if let Some(started) = state.tick_started_at.take() {
        let elapsed = started.elapsed();
        let micros = duration_to_micros(elapsed);
        state.fixed_tick_samples.push(micros);
        state.fixed_ticks = state.fixed_ticks.saturating_add(1);
    }
}

pub(super) fn observe_process_counts(
    mut exits: MessageReader<AppExit>,
    entities: &bevy::ecs::entity::Entities,
    links: Query<(), With<Link>>,
    mut state: ResMut<ProcessDiagnosticsState>,
) {
    // Every observed error exit is one failed check or lifecycle path; the closeout report
    // uses this count to prove zero errors rather than merely carrying the field.
    for exit in exits.read() {
        if exit.is_error() {
            state.error_count = state.error_count.saturating_add(1);
        }
    }
    let entity_count = entities.len();
    let link_count = u32::try_from(links.iter().count()).unwrap_or(u32::MAX);
    state.entity_high_water = state.entity_high_water.max(entity_count);
    state.link_high_water = state.link_high_water.max(link_count);
    state.terminal_entities = Some(entity_count);
    state.terminal_links = Some(link_count);
}

pub(super) fn sample_link_stats(links: Query<&Link>, mut state: ResMut<ProcessDiagnosticsState>) {
    let Some(worst) = links.iter().max_by_key(|link| link.stats.rtt) else {
        return;
    };
    state.rtt_samples.push(duration_to_micros(worst.stats.rtt));
    state
        .jitter_samples
        .push(duration_to_micros(worst.stats.jitter));
}

/// Cache the manifest participant rows while fighters are live. The observation runs every
/// frame of a diagnostics-enabled process, so build replacements update their row in place
/// and the cache stays sorted by stable player identity; finalization then reads the cache
/// even after the role shutdown chain has despawned every replicated fighter.
pub(in crate::diagnostics) fn observe_manifest_participants(
    mut state: ResMut<ProcessDiagnosticsState>,
    fighters: Query<
        (&crate::protocol::PlayerId, &crate::builds::SelectedBuild),
        With<crate::protocol::Fighter>,
    >,
) {
    let mut participants = std::mem::take(&mut state.manifest_participants);
    for (player, build) in &fighters {
        let row = ManifestParticipant {
            player_id: player.0,
            build_identity: participant_build_identity(build),
        };
        match participants.binary_search_by_key(&player.0, |row| row.player_id) {
            Ok(index) => participants[index] = row,
            Err(index) if participants.len() < crate::diagnostics::MAX_MANIFEST_PARTICIPANTS => {
                participants.insert(index, row);
            }
            _ => {}
        }
    }
    state.manifest_participants = participants;
}

/// Consolidate the bounded gameplay telemetry summaries into the terminal observation
/// state, so the consolidated report ties the run to its build, ability, match/mode, and
/// map observations instead of process/network measurements alone. The authoritative
/// match and ability telemetry resources exist only in the server process; both
/// roles observe the same canonical map state. Reading aggregates here — while the process still
/// owns the resources — keeps report finalization free of gameplay-query parameters.
#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(in crate::diagnostics) fn observe_gameplay_aggregates(
    mut state: ResMut<ProcessDiagnosticsState>,
    match_telemetry: Option<Res<crate::matchplay::MatchTelemetry>>,
    #[cfg(feature = "server")] map_telemetry: Option<Res<crate::map::MapDynamicTelemetry>>,
) {
    let mut gameplay = GameplayAggregatesV1::default();
    #[cfg(feature = "server")]
    if let Some(map) = map_telemetry.as_deref() {
        gameplay.map_destruction_requested = map.destruction_requests;
        gameplay.map_destruction_applied = map.destruction_applied;
        gameplay.map_destruction_no_ops = map.destruction_no_ops;
        gameplay.map_placements_changed = map.placements_changed;
    }
    if let Some(matches) = match_telemetry.as_deref() {
        gameplay.matches_completed = u32::try_from(
            u64::try_from(matches.summaries.len())
                .unwrap_or(u64::MAX)
                .saturating_add(matches.dropped_summaries),
        )
        .unwrap_or(u32::MAX);
        if let Some(summary) = matches.summaries.back() {
            consolidate_match_summary(summary, &mut gameplay);
        }
    }
    state.gameplay = gameplay;
}

/// Fold the latest completed match summary's mode, ability, and weapon aggregates into
/// the gameplay block. The mode identity and its typed summary ride along so closeouts
/// carry the objective evidence (Wipeout scores, Hot Zone terminal state) the match
/// telemetry already owns. Weapon aggregates are summed across every preset that fought.
fn consolidate_match_summary(
    summary: &crate::matchplay::MatchSummary,
    gameplay: &mut GameplayAggregatesV1,
) {
    use crate::matchplay::ModeSummary;
    gameplay.match_result = Some(summary.result.report_label());
    gameplay.mode_definition_id = Some(summary.mode_definition_id.0);
    match summary.mode_summary {
        ModeSummary::Wipeout(wipeout) => {
            gameplay.wipeout_final_scores = Some(wipeout.final_scores);
            gameplay.wipeout_target_score = Some(wipeout.target_score);
            gameplay.wipeout_score_margin = Some(wipeout.score_margin);
        }
        ModeSummary::HotZone(hot_zone) => {
            gameplay.hot_zone_final_progress = Some(hot_zone.final_progress_ticks);
            gameplay.hot_zone_target_progress_ticks = Some(hot_zone.target_progress_ticks);
            gameplay.hot_zone_controlled_ticks = Some(hot_zone.controlled_ticks_by_team);
            gameplay.hot_zone_contested_ticks = Some(hot_zone.contested_ticks);
            gameplay.hot_zone_control_gained_transitions =
                Some(hot_zone.control_gained_transitions_by_team);
            gameplay.hot_zone_longest_control_ticks =
                Some(hot_zone.longest_consecutive_control_ticks_by_team);
        }
        ModeSummary::Heist(_) => {}
    }
    gameplay.match_active_ticks = summary.active_duration_ticks;
    gameplay.match_respawns = summary.respawns;
    gameplay.team_a_defeats = summary.credited_defeats_by_team[0];
    gameplay.team_b_defeats = summary.credited_defeats_by_team[1];
    gameplay.first_hostile_damage_tick = summary.time_to_first_hostile_damage_ticks;
    gameplay.ability_attempts = summary.ability_telemetry.attempts;
    gameplay.ability_accepts = summary.ability_telemetry.accepts;
    gameplay.dash_uses = summary.ability_telemetry.dash_uses;
    gameplay.sentry_uses = summary.ability_telemetry.sentry_uses;
    for (_, aggregate) in &summary.weapon_aggregates {
        gameplay.accepted_attacks = gameplay
            .accepted_attacks
            .saturating_add(aggregate.accepted_attacks);
        gameplay.emitted_deliveries = gameplay
            .emitted_deliveries
            .saturating_add(aggregate.emitted_deliveries);
        gameplay.attacks_with_hostile_contact = gameplay
            .attacks_with_hostile_contact
            .saturating_add(aggregate.attacks_with_hostile_contact);
        gameplay.hostile_damage = gameplay
            .hostile_damage
            .saturating_add(aggregate.hostile_damage);
    }
}

#[cfg(feature = "process-metrics")]
pub(super) fn sample_lightyear_metrics(
    registry: Option<Res<lightyear::metrics::prelude::MetricsRegistry>>,
    mut state: ResMut<ProcessDiagnosticsState>,
) {
    use lightyear::metrics::metrics::Key;
    let Some(registry) = registry else {
        return;
    };
    fn counter(
        registry: &lightyear::metrics::prelude::MetricsRegistry,
        name: &'static str,
    ) -> Option<u64> {
        registry
            .get_counter_value(&Key::from_static_name(name))
            .map(|value| value as u64)
    }
    // Lightyear records cumulative byte totals as gauges, not counters.
    fn byte_gauge(
        registry: &lightyear::metrics::prelude::MetricsRegistry,
        name: &'static str,
    ) -> Option<u64> {
        registry
            .get_gauge_value(&Key::from_static_name(name))
            .map(|value| value.max(0.0) as u64)
    }
    state.transport.bytes_sent =
        byte_gauge(&registry, "transport/send_bytes").unwrap_or(state.transport.bytes_sent);
    state.transport.bytes_received =
        byte_gauge(&registry, "transport/recv_bytes").unwrap_or(state.transport.bytes_received);
    state.transport.packets_sent =
        counter(&registry, "packets/send").unwrap_or(state.transport.packets_sent);
    state.transport.packets_received =
        counter(&registry, "packets/received").unwrap_or(state.transport.packets_received);
    state.transport.channel_messages_sent = counter(&registry, "channel/send_messages")
        .unwrap_or(state.transport.channel_messages_sent);
    state.transport.channel_messages_received = counter(&registry, "channel/recv_messages")
        .unwrap_or(state.transport.channel_messages_received);
}

fn duration_to_micros(duration: Duration) -> u32 {
    u32::try_from(duration.as_micros()).unwrap_or(u32::MAX)
}
