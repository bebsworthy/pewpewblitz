//! Process-level movement and match evidence validation.

use super::{
    Fighter, MatchParticipant, MatchPhase, MatchRoot, MatchState, PlayerId, Position,
    ProcessMatchCheck, ProcessMovementCheck, Rotation, WeaponPresetId, fs, record_server_failure,
};
use bevy::prelude::*;

/// Every failure path in this module funnels through here: classify the exit as
/// `verification-failed`, append the bounded failure record when the
/// `BRAWLER_FAILURE_REPORT` control selects one, and request the error exit, so process
/// verification failures keep the structured-failure contract instead of reporting as
/// unclassified `shutdown-incomplete`.
fn fail_verification(
    diagnostics: Option<&crate::diagnostics::ProcessDiagnosticsSettings>,
    classification: &mut crate::diagnostics::ProcessExitClassification,
    app_exit: &mut MessageWriter<AppExit>,
    message: &str,
) {
    record_server_failure(
        crate::diagnostics::FailureCategory::VerificationFailed,
        message,
        diagnostics,
        classification,
        app_exit,
    );
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
pub(super) fn verify_process_match(
    mut check: ResMut<ProcessMatchCheck>,
    diagnostics: Option<Res<crate::diagnostics::ProcessDiagnosticsSettings>>,
    mut classification: ResMut<crate::diagnostics::ProcessExitClassification>,
    roots: Query<&MatchState, With<MatchRoot>>,
    telemetry: Res<crate::matchplay::MatchTelemetry>,
    maps: Query<
        (
            &crate::map::ResolvedMapSnapshot,
            &crate::map::MapDynamicState,
        ),
        With<crate::map::MapRoot>,
    >,
    participants: Query<&MatchParticipant, With<Fighter>>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if !check.enabled || check.completed {
        return;
    }
    let Ok(state) = roots.single() else { return };
    let initial = *check.initial_match_id.get_or_insert(state.match_id);
    let Some(summary) = telemetry.summaries.back() else {
        return;
    };
    let ability_telemetry = &summary.ability_telemetry;
    if state.match_id.0 <= initial.0 || !matches!(state.phase, MatchPhase::Waiting) {
        return;
    }
    let participant_count = participants
        .iter()
        .filter(|participant| participant.match_id == state.match_id)
        .count();
    let (Some(map_identity), Some(content_fingerprint)) =
        (summary.map_identity, summary.content_fingerprint)
    else {
        error!("match summary omitted map or content identity");
        fail_verification(
            diagnostics.as_deref(),
            &mut classification,
            &mut app_exit,
            "match summary omitted map or content identity",
        );
        check.completed = true;
        return;
    };
    if summary.participants.len() != 4 {
        error!(
            participant_count = summary.participants.len(),
            "match summary omitted initial participant identity"
        );
        fail_verification(
            diagnostics.as_deref(),
            &mut classification,
            &mut app_exit,
            "match summary omitted initial participant identity",
        );
        check.completed = true;
        return;
    }
    if !has_preset_outcome_evidence(summary) {
        error!("match summary omitted preset defeat/death evidence");
        fail_verification(
            diagnostics.as_deref(),
            &mut classification,
            &mut app_exit,
            "match summary omitted preset defeat/death evidence",
        );
        check.completed = true;
        return;
    }
    if summary.respawns == 0 {
        error!("match summary did not prove a completed respawn");
        fail_verification(
            diagnostics.as_deref(),
            &mut classification,
            &mut app_exit,
            "match summary did not prove a completed respawn",
        );
        check.completed = true;
        return;
    }
    let weapon_preset_ids = summary
        .weapon_aggregates
        .iter()
        .map(|(key, _)| key.preset_id.0.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let accepted_attacks = summary
        .weapon_aggregates
        .iter()
        .map(|(_, aggregate)| aggregate.accepted_attacks)
        .sum::<u64>();
    let attacks_with_hostile_contact = summary
        .weapon_aggregates
        .iter()
        .map(|(_, aggregate)| aggregate.attacks_with_hostile_contact)
        .sum::<u64>();
    let preset_defeats = format_preset_counts(&summary.credited_defeats_by_preset);
    let preset_deaths = format_preset_counts(&summary.suffered_deaths_by_preset);
    let preset_death_rates =
        format_preset_rates(&summary.suffered_deaths_per_participant_minute_by_preset);
    let build_fingerprints = summary
        .participants
        .iter()
        .map(|participant| participant.selected_build)
        .map(|build| build.recipe_fingerprint.0.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let build_total_points = summary
        .participants
        .iter()
        .filter_map(|participant| participant.total_points)
        .map(|points| points.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let passive_ids = summary
        .participants
        .iter()
        .filter_map(|participant| participant.passive_ids)
        .map(|passives| format!("{}+{}", passives[0].0, passives[1].0))
        .collect::<Vec<_>>()
        .join(",");
    let ultimate_ids = summary
        .participants
        .iter()
        .filter_map(|participant| participant.ultimate_id)
        .map(|id| id.0.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let first_full_charge_ticks = ability_telemetry
        .first_full_charge_tick_by_owner
        .iter()
        .map(|(owner, tick)| format!("{}:{tick}", owner.0))
        .collect::<Vec<_>>()
        .join(",");
    let first_full_charge_active_ticks = ability_telemetry
        .first_full_charge_tick_by_owner
        .iter()
        .map(|(owner, tick)| {
            format!(
                "{}:{}",
                owner.0,
                tick.saturating_sub(summary.active_started_at_tick)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let ability_uses_by_owner = ability_telemetry
        .uses_by_owner
        .iter()
        .map(|(owner, uses)| format!("{}:{uses}", owner.0))
        .collect::<Vec<_>>()
        .join(",");
    let passive_triggers = ability_telemetry
        .passive_triggers
        .iter()
        .map(|(passive, triggers)| format!("{}:{triggers}", passive.0))
        .collect::<Vec<_>>()
        .join(",");
    let charge_dealt_by_owner = ability_telemetry
        .charge_damage_dealt_by_owner
        .iter()
        .map(|(owner, damage)| format!("{}:{damage}", owner.0))
        .collect::<Vec<_>>()
        .join(",");
    let charge_received_by_owner = ability_telemetry
        .charge_damage_received_by_owner
        .iter()
        .map(|(owner, damage)| format!("{}:{damage}", owner.0))
        .collect::<Vec<_>>()
        .join(",");
    let (mode_definition_id, final_score_team_1, final_score_team_2) = match &summary.mode_summary {
        crate::matchplay::ModeSummary::Wipeout(wipeout) => (
            summary.mode_definition_id.0,
            wipeout.final_scores[0],
            wipeout.final_scores[1],
        ),
        crate::matchplay::ModeSummary::HotZone(hot_zone) => (
            summary.mode_definition_id.0,
            hot_zone.final_progress_ticks[0],
            hot_zone.final_progress_ticks[1],
        ),
        crate::matchplay::ModeSummary::Heist(heist) => (
            summary.mode_definition_id.0,
            heist.final_health[0],
            heist.final_health[1],
        ),
        crate::matchplay::ModeSummary::Zombie(zombie) => (
            summary.mode_definition_id.0,
            zombie.total_defeated,
            zombie.total_spawned,
        ),
    };
    // Canonical map process evidence: exact authored identity and dynamic terminal state.
    let map_dynamic = maps.single().ok().map(|(snapshot, state)| {
        let dynamic_bytes = postcard::to_allocvec(state).unwrap_or_default();
        (
            snapshot.identity.recipe_fingerprint.0,
            snapshot.placements.len(),
            u32::try_from(state.terminal_states.len()).unwrap_or(u32::MAX),
            state.revision,
            crate::content::fnv1a64(&dynamic_bytes),
            state.revision,
            u64::try_from(state.terminal_states.len()).unwrap_or(u64::MAX),
            0,
            0,
            0,
            0,
            dynamic_bytes.len(),
            None::<usize>,
            None::<usize>,
            0,
            0,
        )
    });
    let report = format!(
        "initial_match_id={}\nrestarted_match_id={}\nparticipant_count={}\nsummary_participant_count={}\nmap_instance_id={}\nmap_recipe_fingerprint={}\ncontent_fingerprint={}\nrules_revision={}\nmode_definition_id={}\nfinal_score_team_1={}\nfinal_score_team_2={}\nresult={:?}\nactive_duration_ticks={}\ndefeats={}\nrespawns={}\nparticipant_active_ticks_team_1={}\nparticipant_active_ticks_team_2={}\nrecords={}\ndropped_records={}\nsummary_count={}\nweapon_aggregate_count={}\nweapon_preset_ids={}\nbuild_fingerprints={}\nbuild_total_points={}\nultimate_ids={}\npassive_ids={}\nfirst_full_charge_ticks={}\nfirst_full_charge_active_ticks={}\nability_uses_by_owner={}\ncharge_dealt_by_owner={}\ncharge_received_by_owner={}\npassive_triggers={}\npreset_defeats={}\npreset_deaths={}\npreset_death_rates={}\naccepted_attacks={}\nattacks_with_hostile_contact={}\nability_attempts={}\nability_accepts={}\ndash_uses={}\nsentry_uses={}\nself_cloak_uses={}\nself_cloak_active_ticks={}\nself_cloak_end_reasons={:?}\nreveal_scan_uses={}\nreveal_scan_targets={}\nconcealment_field_uses={}\nsentry_shots={}\nability_dropped_records={}\nwasted_charge={}\nready_to_use_delay_ticks={}\nready_to_use_count={}\nability_rejections={:?}\ndash_requested_distance_milli={:?}\ndash_actual_distance_milli={:?}\ndash_map_collision_truncations={:?}\ndash_contacts={:?}\ndash_interruptions={:?}\nability_damage={:?}\nability_targets={:?}\nability_defeats={:?}\nsentry_cleanup_reasons={:?}\nconcurrent_sentry_high_water={}\nsentries={:?}\npassive_active_ticks={:?}\npassive_modified_amounts={:?}\npassive_unused_triggers={:?}\nmap_catalog_schema_version={}\nmap_dynamic_fingerprint={}\nmap_placement_count={}\nmap_terminal_state_count={}\nmap_dynamic_revision={}\nmap_dynamic_digest={}\nmap_destruction_revision={}\nmap_placements_changed={}\nmap_collider_updates={}\nmap_recovery_requests={}\nmap_recovery_responses={}\nmap_recovery_rejections={}\nmap_recovery_snapshot_bytes={}\nmap_event_min_bytes={:?}\nmap_event_max_bytes={:?}\nmap_defensive_repairs={}\nmap_dropped_records={}\n",
        initial.0,
        state.match_id.0,
        participant_count,
        summary.participants.len(),
        map_identity.instance_id.0,
        map_identity.recipe_fingerprint.0,
        content_fingerprint.0,
        summary.rules_revision,
        mode_definition_id,
        final_score_team_1,
        final_score_team_2,
        summary.result,
        summary.active_duration_ticks,
        summary.suffered_deaths_by_team.iter().sum::<u32>(),
        summary.respawns,
        summary.participant_active_ticks_by_team[0],
        summary.participant_active_ticks_by_team[1],
        telemetry.records.len(),
        summary.dropped_records,
        telemetry.summaries.len(),
        summary.weapon_aggregates.len(),
        weapon_preset_ids,
        build_fingerprints,
        build_total_points,
        ultimate_ids,
        passive_ids,
        first_full_charge_ticks,
        first_full_charge_active_ticks,
        ability_uses_by_owner,
        charge_dealt_by_owner,
        charge_received_by_owner,
        passive_triggers,
        preset_defeats,
        preset_deaths,
        preset_death_rates,
        accepted_attacks,
        attacks_with_hostile_contact,
        ability_telemetry.attempts,
        ability_telemetry.accepts,
        ability_telemetry.dash_uses,
        ability_telemetry.sentry_uses,
        ability_telemetry.self_cloak_uses,
        ability_telemetry.self_cloak_active_ticks,
        ability_telemetry.self_cloak_end_reasons,
        ability_telemetry.reveal_scan_uses,
        ability_telemetry.reveal_scan_targets,
        ability_telemetry.concealment_field_uses,
        ability_telemetry.sentry_shots,
        ability_telemetry.dropped_records,
        ability_telemetry.wasted_charge,
        ability_telemetry.ready_to_use_delay_ticks,
        ability_telemetry.ready_to_use_count,
        ability_telemetry.rejections_by_reason,
        ability_telemetry.dash_requested_distance_milli_by_owner,
        ability_telemetry.dash_actual_distance_milli_by_owner,
        ability_telemetry.dash_map_collision_truncations_by_owner,
        ability_telemetry.dash_contacts_by_owner,
        ability_telemetry.dash_interruptions_by_owner,
        ability_telemetry.ability_damage_by_owner,
        ability_telemetry.ability_targets_by_owner,
        ability_telemetry.ability_defeats_by_owner,
        ability_telemetry.sentry_cleanup_reasons,
        ability_telemetry.concurrent_sentry_high_water,
        ability_telemetry.sentries,
        ability_telemetry.passive_active_ticks,
        ability_telemetry.passive_modified_amounts,
        ability_telemetry.passive_unused_triggers,
        crate::map::MAP_RECIPE_SCHEMA_VERSION,
        map_dynamic.map_or(0, |row| row.0),
        map_dynamic.map_or(0, |row| row.1),
        map_dynamic.map_or(0, |row| row.2),
        map_dynamic.map_or(0, |row| row.3),
        map_dynamic.map_or(0, |row| row.4),
        map_dynamic.map_or(0, |row| row.5),
        map_dynamic.map_or(0, |row| row.6),
        map_dynamic.map_or(0, |row| row.7),
        map_dynamic.map_or(0, |row| row.8),
        map_dynamic.map_or(0, |row| row.9),
        map_dynamic.map_or(0, |row| row.10),
        map_dynamic.map_or(0, |row| row.11),
        map_dynamic.and_then(|row| row.12),
        map_dynamic.and_then(|row| row.13),
        map_dynamic.map_or(0, |row| row.14),
        map_dynamic.map_or(0, |row| row.15),
    );
    if let Some(path) = &check.report_file
        && let Err(error) = fs::write(path, report.as_bytes())
    {
        error!(path = %path.display(), ?error, "match report write failed");
        fail_verification(
            diagnostics.as_deref(),
            &mut classification,
            &mut app_exit,
            "match report write failed",
        );
        check.completed = true;
        return;
    }
    info!(%report, "authoritative Wipeout process verification complete");
    check.completed = true;
    app_exit.write(AppExit::Success);
}

fn has_preset_outcome_evidence(summary: &crate::matchplay::MatchSummary) -> bool {
    !summary.credited_defeats_by_preset.is_empty()
        && !summary.suffered_deaths_by_preset.is_empty()
        && !summary
            .suffered_deaths_per_participant_minute_by_preset
            .is_empty()
}

fn format_preset_counts(values: &[(WeaponPresetId, u32)]) -> String {
    values
        .iter()
        .map(|(preset, count)| format!("{}:{count}", preset.0))
        .collect::<Vec<_>>()
        .join(",")
}

fn format_preset_rates(values: &[(WeaponPresetId, f64)]) -> String {
    values
        .iter()
        .map(|(preset, rate)| format!("{}:{rate:.3}", preset.0))
        .collect::<Vec<_>>()
        .join(",")
}

/// The movement-smoke window must outlast a full production lifecycle: client join, build
/// selection, the 180-tick countdown, and travel to displacement. Displacement is compared
/// against the join-time baseline, so a wider window cannot pass a fighter that never moved.
const MOVEMENT_SMOKE_WINDOW_TICKS: u64 = 420;

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn verify_process_movement(
    mut check: ResMut<ProcessMovementCheck>,
    diagnostics: Option<Res<crate::diagnostics::ProcessDiagnosticsSettings>>,
    mut classification: ResMut<crate::diagnostics::ProcessExitClassification>,
    tick: Res<crate::timing::SimulationTick>,
    fighters: Query<(&PlayerId, &Position, &Rotation), With<Fighter>>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if !check.enabled || check.completed {
        return;
    }
    let mut current: Vec<_> = fighters
        .iter()
        .map(|(player, position, rotation)| (*player, position.0, rotation.as_radians()))
        .collect();
    current.sort_by_key(|(player, _, _)| player.0);
    if current.len() < 2 {
        return;
    }
    if check.initial_poses.is_empty() {
        check.initial_poses.clone_from(&current);
        check.initial_tick = Some(tick.0);
        return;
    }
    if check.initial_tick.is_none_or(|initial_tick| {
        tick.0 < initial_tick.saturating_add(MOVEMENT_SMOKE_WINDOW_TICKS)
    }) {
        return;
    }
    let moved = current.iter().any(|(player, position, _)| {
        check
            .initial_poses
            .iter()
            .find(|(initial_player, _, _)| initial_player == player)
            .is_some_and(|(_, initial_position, _)| {
                (*position - *initial_position).length() > 100.0
            })
    });
    let aimed = current.iter().any(|(player, _, facing)| {
        check
            .initial_poses
            .iter()
            .find(|(initial_player, _, _)| initial_player == player)
            .is_some_and(|(_, _, initial_facing)| (facing - initial_facing).abs() > 0.5)
    });
    if moved && aimed {
        info!(tick = tick.0, "network movement smoke assertion passed");
        if let Some(path) = check.ready_file.as_ref()
            && let Err(error) = fs::write(path, b"passed\n")
        {
            error!(
                ?path,
                ?error,
                "network movement smoke readiness signal failed"
            );
            fail_verification(
                diagnostics.as_deref(),
                &mut classification,
                &mut app_exit,
                "network movement smoke readiness signal failed",
            );
        }
        check.completed = true;
    } else {
        error!(
            tick = tick.0,
            moved, aimed, "network movement smoke assertion failed"
        );
        fail_verification(
            diagnostics.as_deref(),
            &mut classification,
            &mut app_exit,
            "network movement smoke assertion failed",
        );
        check.completed = true;
    }
}

pub(crate) fn required_process_checkpoints(preset_id: WeaponPresetId) -> &'static [&'static str] {
    match preset_id.0 {
        2 => &["active_scatter_flight", "defeat", "reset"],
        3 => &[
            "active_lob_flight",
            "active_slow",
            "active_knockback",
            "defeat",
            "reset",
        ],
        4 => &["active_knockback", "defeat", "reset"],
        _ => &["defeat", "reset"],
    }
}
