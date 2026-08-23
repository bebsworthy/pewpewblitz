//! Process-level movement/combat evidence validation and report parsing.
#![allow(clippy::wildcard_imports)]

use super::*;

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
    build_telemetry: Res<crate::builds::BuildTelemetry>,
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
    let build_preset_ids = summary
        .participants
        .iter()
        .map(|participant| participant.selected_build)
        .filter_map(|build| build.source_build_preset_id)
        .map(|id| id.0.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let custom_builds = summary
        .participants
        .iter()
        .map(|participant| participant.selected_build)
        .filter(|build| build.source_build_preset_id.is_none())
        .count();
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
        "initial_match_id={}\nrestarted_match_id={}\nparticipant_count={}\nsummary_participant_count={}\nmap_instance_id={}\nmap_recipe_fingerprint={}\ncontent_fingerprint={}\nrules_revision={}\nmode_definition_id={}\nfinal_score_team_1={}\nfinal_score_team_2={}\nresult={:?}\nactive_duration_ticks={}\ndefeats={}\nrespawns={}\nparticipant_active_ticks_team_1={}\nparticipant_active_ticks_team_2={}\nrecords={}\ndropped_records={}\nsummary_count={}\nweapon_aggregate_count={}\nweapon_preset_ids={}\nbuild_preset_ids={}\ncustom_builds={}\nbuild_fingerprints={}\nbuild_total_points={}\nultimate_ids={}\npassive_ids={}\nfirst_full_charge_ticks={}\nfirst_full_charge_active_ticks={}\nability_uses_by_owner={}\ncharge_dealt_by_owner={}\ncharge_received_by_owner={}\npassive_triggers={}\npreset_defeats={}\npreset_deaths={}\npreset_death_rates={}\naccepted_attacks={}\nattacks_with_hostile_contact={}\nbuild_selections={}\nbuild_dropped_records={}\nability_attempts={}\nability_accepts={}\ndash_uses={}\nsentry_uses={}\nself_cloak_uses={}\nself_cloak_active_ticks={}\nself_cloak_end_reasons={:?}\nreveal_scan_uses={}\nreveal_scan_targets={}\nsentry_shots={}\nability_dropped_records={}\nwasted_charge={}\nready_to_use_delay_ticks={}\nready_to_use_count={}\nability_rejections={:?}\ndash_requested_distance_milli={:?}\ndash_actual_distance_milli={:?}\ndash_map_collision_truncations={:?}\ndash_contacts={:?}\ndash_interruptions={:?}\nability_damage={:?}\nability_targets={:?}\nability_defeats={:?}\nsentry_cleanup_reasons={:?}\nconcurrent_sentry_high_water={}\nsentries={:?}\npassive_active_ticks={:?}\npassive_modified_amounts={:?}\npassive_unused_triggers={:?}\nmap_catalog_schema_version={}\nmap_dynamic_fingerprint={}\nmap_placement_count={}\nmap_terminal_state_count={}\nmap_dynamic_revision={}\nmap_dynamic_digest={}\nmap_destruction_revision={}\nmap_placements_changed={}\nmap_collider_updates={}\nmap_recovery_requests={}\nmap_recovery_responses={}\nmap_recovery_rejections={}\nmap_recovery_snapshot_bytes={}\nmap_event_min_bytes={:?}\nmap_event_max_bytes={:?}\nmap_defensive_repairs={}\nmap_dropped_records={}\n",
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
        build_preset_ids,
        custom_builds,
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
        build_telemetry.selections.len(),
        build_telemetry.dropped_records,
        ability_telemetry.attempts,
        ability_telemetry.accepts,
        ability_telemetry.dash_uses,
        ability_telemetry.sentry_uses,
        ability_telemetry.self_cloak_uses,
        ability_telemetry.self_cloak_active_ticks,
        ability_telemetry.self_cloak_end_reasons,
        ability_telemetry.reveal_scan_uses,
        ability_telemetry.reveal_scan_targets,
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

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
pub(super) fn verify_process_combat(
    mut check: ResMut<ProcessCombatCheck>,
    diagnostics: Option<Res<crate::diagnostics::ProcessDiagnosticsSettings>>,
    mut classification: ResMut<crate::diagnostics::ProcessExitClassification>,
    telemetry: Res<CombatTelemetry>,
    weapon_telemetry: Res<WeaponTelemetry>,
    evidence: Res<CombatEvidenceSnapshots>,
    catalog: Res<WeaponCatalogResource>,
    fighters: Res<crate::combat::FighterDefinitions>,
    sessions: Query<&ServerSession, With<LinkOf>>,
    selected_fighters: Query<
        (
            &crate::builds::SelectedBuild,
            &crate::builds::ResolvedMatchLoadout,
        ),
        With<Fighter>,
    >,
    mut app_exit: MessageWriter<AppExit>,
) {
    if !check.enabled || check.completed {
        return;
    }
    let active_sessions = sessions
        .iter()
        .filter(|session| matches!(session.phase, ServerSessionPhase::Active { .. }))
        .count();
    let accepted_attacks: u64 = weapon_telemetry.accepted_attacks.values().copied().sum();
    let Some(expected_preset_id) = check.expected_preset_id else {
        error!("combat process assertion is missing BRAWLER_NETWORK_WEAPON_PRESET");
        check.completed = true;
        fail_verification(
            diagnostics.as_deref(),
            &mut classification,
            &mut app_exit,
            "combat process assertion is missing BRAWLER_NETWORK_WEAPON_PRESET",
        );
        return;
    };
    let Some(_) = catalog.0.preset(expected_preset_id) else {
        error!(
            preset_id = expected_preset_id.0,
            "combat assertion requested an unknown preset"
        );
        check.completed = true;
        fail_verification(
            diagnostics.as_deref(),
            &mut classification,
            &mut app_exit,
            "combat assertion requested an unknown preset",
        );
        return;
    };
    let Some(fighter_definition) = fighters.get(crate::combat::STANDARD_FIGHTER_DEFINITION) else {
        return;
    };
    let Ok(expected_resolved) = catalog
        .0
        .resolve_preset(expected_preset_id, fighter_definition)
    else {
        error!(
            preset_id = expected_preset_id.0,
            "combat assertion could not resolve the requested preset"
        );
        check.completed = true;
        fail_verification(
            diagnostics.as_deref(),
            &mut classification,
            &mut app_exit,
            "combat assertion could not resolve the requested preset",
        );
        return;
    };
    let tested_fighter = selected_fighters.iter().any(|(build, loadout)| {
        build.source_build_preset_id.is_some()
            && loadout.primary_weapon.source_preset_id == Some(expected_preset_id)
            && loadout.primary_weapon.recipe_fingerprint == expected_resolved.recipe_fingerprint
    });
    let expected_attacks = weapon_telemetry
        .accepted_attacks
        .get(&expected_preset_id)
        .copied()
        .unwrap_or(0);
    let expected_deliveries = weapon_telemetry
        .emitted_deliveries
        .get(&expected_preset_id)
        .copied()
        .unwrap_or(0);
    let expected_aggregate = weapon_telemetry.source_aggregates.get(&WeaponTelemetryKey {
        preset_id: expected_preset_id,
        recipe_fingerprint: expected_resolved.recipe_fingerprint,
    });
    let expected_family_exercised = expected_aggregate.is_some_and(|aggregate| {
        aggregate.accepted_attacks > 0
            && aggregate.emitted_deliveries > 0
            && aggregate.accepted_attacks == expected_attacks
            && aggregate.emitted_deliveries == expected_deliveries
    });
    let clients_observed = check.client_ready_dir.as_ref().is_some_and(|directory| {
        [1_u64, 2].iter().all(|client_id| {
            directory
                .join(format!("client-{client_id}.ready"))
                .is_file()
        })
    });
    if active_sessions < 2
        || accepted_attacks < 4
        || telemetry.applied_damage == 0
        || telemetry.defeats == 0
        || !tested_fighter
        || !expected_family_exercised
        || !clients_observed
    {
        return;
    }
    let Some(path) = check.ready_file.clone() else {
        error!("combat process assertion is enabled without a readiness file");
        check.completed = true;
        fail_verification(
            diagnostics.as_deref(),
            &mut classification,
            &mut app_exit,
            "combat process assertion is enabled without a readiness file",
        );
        return;
    };

    let Some(client_ready_dir) = check.client_ready_dir.clone() else {
        error!("combat process assertion is enabled without a client evidence directory");
        check.completed = true;
        fail_verification(
            diagnostics.as_deref(),
            &mut classification,
            &mut app_exit,
            "combat process assertion is enabled without a client evidence directory",
        );
        return;
    };
    let client_one_path = client_ready_dir.join("client-1.ready");
    let client_two_path = client_ready_dir.join("client-2.ready");
    let client_one = match fs::read_to_string(&client_one_path) {
        Ok(contents) => contents,
        Err(error) => {
            error!(path = %client_one_path.display(), ?error, "client one combat evidence could not be read");
            check.completed = true;
            fail_verification(
                diagnostics.as_deref(),
                &mut classification,
                &mut app_exit,
                "client one combat evidence could not be read",
            );
            return;
        }
    };
    let client_two = match fs::read_to_string(&client_two_path) {
        Ok(contents) => contents,
        Err(error) => {
            error!(path = %client_two_path.display(), ?error, "client two combat evidence could not be read");
            check.completed = true;
            fail_verification(
                diagnostics.as_deref(),
                &mut classification,
                &mut app_exit,
                "client two combat evidence could not be read",
            );
            return;
        }
    };
    let client_evidence_drops = [client_one.as_str(), client_two.as_str()]
        .into_iter()
        .map(|contents| {
            parse_report_counter(contents, "dropped_cue_stream")
                + parse_report_counter(contents, "dropped_cue_timestamps")
        })
        .sum::<u64>();
    if telemetry.dropped_cues > 0
        || telemetry.dropped_records > 0
        || telemetry.dropped_accepted_shot_timestamps > 0
        || client_evidence_drops > 0
    {
        error!(
            server_dropped_cues = telemetry.dropped_cues,
            server_dropped_records = telemetry.dropped_records,
            server_dropped_timestamps = telemetry.dropped_accepted_shot_timestamps,
            client_evidence_drops,
            "combat evidence history was truncated"
        );
        check.completed = true;
        fail_verification(
            diagnostics.as_deref(),
            &mut classification,
            &mut app_exit,
            "combat evidence history was truncated",
        );
        return;
    }
    let through_reset = |cues: &[CombatCue]| {
        cues.iter()
            .take_while(|cue| {
                !matches!(
                    cue,
                    CombatCue::Reset { .. } | CombatCue::FighterReset { .. }
                )
            })
            .chain(
                cues.iter()
                    .skip_while(|cue| {
                        !matches!(
                            cue,
                            CombatCue::Reset { .. } | CombatCue::FighterReset { .. }
                        )
                    })
                    .take(1),
            )
            .cloned()
            .collect::<Vec<_>>()
    };
    let expected_cue_stream = through_reset(&telemetry.cues);
    let client_one_cue_stream = through_reset(&parse_client_cue_stream(&client_one));
    let client_two_cue_stream = through_reset(&parse_client_cue_stream(&client_two));
    let cue_converged = !expected_cue_stream.is_empty()
        && client_one_cue_stream.as_slice() == expected_cue_stream.as_slice()
        && client_two_cue_stream.as_slice() == expected_cue_stream.as_slice();
    if !cue_converged {
        let first_client_one_mismatch = expected_cue_stream
            .iter()
            .zip(&client_one_cue_stream)
            .position(|(expected, actual)| expected != actual);
        let first_client_two_mismatch = expected_cue_stream
            .iter()
            .zip(&client_two_cue_stream)
            .position(|(expected, actual)| expected != actual);
        error!(
            accepted_attacks,
            expected_cue_count = expected_cue_stream.len(),
            client_one_cue_count = client_one_cue_stream.len(),
            client_two_cue_count = client_two_cue_stream.len(),
            first_client_one_mismatch = ?first_client_one_mismatch,
            first_client_two_mismatch = ?first_client_two_mismatch,
            expected_cue_stream = ?expected_cue_stream,
            client_one_cue_stream = ?client_one_cue_stream,
            client_two_cue_stream = ?client_two_cue_stream,
            "combat cue stream evidence is incomplete"
        );
        check.completed = true;
        fail_verification(
            diagnostics.as_deref(),
            &mut classification,
            &mut app_exit,
            "combat cue stream evidence is incomplete",
        );
        return;
    }
    let required_checkpoints = required_process_checkpoints(expected_preset_id);
    let checkpoint_converged = required_checkpoints
        .iter()
        .all(|required| evidence.checkpoints.contains_key(*required))
        && evidence.checkpoints.keys().all(|checkpoint| {
            let client_one_snapshot =
                parse_report_value(&client_one, &format!("checkpoint_{checkpoint}"));
            let client_two_snapshot =
                parse_report_value(&client_two, &format!("checkpoint_{checkpoint}"));
            evidence
                .checkpoint_candidates
                .get(checkpoint)
                .is_some_and(|candidates| {
                    candidates.iter().any(|(snapshot, _)| {
                        report_matches_snapshot(
                            &client_one,
                            &format!("checkpoint_{checkpoint}"),
                            snapshot,
                        ) && report_matches_snapshot(
                            &client_two,
                            &format!("checkpoint_{checkpoint}"),
                            snapshot,
                        )
                    }) && client_one_snapshot.is_some()
                        && client_two_snapshot.is_some()
                })
        });
    if !checkpoint_converged {
        for checkpoint in evidence.checkpoints.keys() {
            let client_one_value =
                parse_report_value(&client_one, &format!("checkpoint_{checkpoint}"));
            let client_two_value =
                parse_report_value(&client_two, &format!("checkpoint_{checkpoint}"));
            let matches_both =
                evidence
                    .checkpoint_candidates
                    .get(checkpoint)
                    .is_some_and(|candidates| {
                        candidates.iter().any(|(snapshot, _)| {
                            report_matches_snapshot(
                                &client_one,
                                &format!("checkpoint_{checkpoint}"),
                                snapshot,
                            ) && report_matches_snapshot(
                                &client_two,
                                &format!("checkpoint_{checkpoint}"),
                                snapshot,
                            )
                        })
                    });
            error!(
                checkpoint,
                server_candidates = evidence
                    .checkpoint_candidates
                    .get(checkpoint)
                    .map_or(0, Vec::len),
                client_one_present = client_one_value.is_some(),
                client_two_present = client_two_value.is_some(),
                matches_both,
                "combat checkpoint diagnostic"
            );
        }
        error!(
            server_checkpoints = ?evidence.checkpoints.keys().collect::<Vec<_>>(),
            "authoritative combat state snapshots did not converge on both clients"
        );
        check.completed = true;
        fail_verification(
            diagnostics.as_deref(),
            &mut classification,
            &mut app_exit,
            "authoritative combat state snapshots did not converge on both clients",
        );
        return;
    }
    let client_one_state_latencies =
        checkpoint_latencies(&evidence.checkpoint_candidates, &client_one);
    let client_two_state_latencies =
        checkpoint_latencies(&evidence.checkpoint_candidates, &client_two);
    let Some((client_one_state_median_us, client_one_state_p95_us)) =
        median_p95(&client_one_state_latencies)
    else {
        error!("client one state convergence latency evidence is incomplete");
        check.completed = true;
        fail_verification(
            diagnostics.as_deref(),
            &mut classification,
            &mut app_exit,
            "client one state convergence latency evidence is incomplete",
        );
        return;
    };
    let Some((client_two_state_median_us, client_two_state_p95_us)) =
        median_p95(&client_two_state_latencies)
    else {
        error!("client two state convergence latency evidence is incomplete");
        check.completed = true;
        fail_verification(
            diagnostics.as_deref(),
            &mut classification,
            &mut app_exit,
            "client two state convergence latency evidence is incomplete",
        );
        return;
    };
    if let Some(report_path) = check.report_file.clone() {
        let client_one_cues = parse_client_cue_timestamps(&client_one);
        let client_two_cues = parse_client_cue_timestamps(&client_two);
        let mut latency_evidence = String::new();
        for (shot_id, fired_at) in &telemetry.accepted_shot_timestamps {
            for (client_name, cues) in [
                ("client_one", &client_one_cues),
                ("client_two", &client_two_cues),
            ] {
                let Some((_, cue_at)) = cues.iter().find(|(candidate, _)| candidate == &shot_id.0)
                else {
                    continue;
                };
                if *cue_at >= *fired_at {
                    let _ = writeln!(
                        latency_evidence,
                        "fire_to_cue_{client_name}_us={}",
                        cue_at.saturating_sub(*fired_at)
                    );
                }
            }
        }
        if latency_evidence.is_empty() {
            error!("combat fire-to-cue latency evidence is incomplete");
            check.completed = true;
            fail_verification(
                diagnostics.as_deref(),
                &mut classification,
                &mut app_exit,
                "combat fire-to-cue latency evidence is incomplete",
            );
            return;
        }
        let client_one_cue_count = parse_report_counter(&client_one, "cue_count");
        let client_two_cue_count = parse_report_counter(&client_two, "cue_count");
        if client_one_cue_count == 0 || client_two_cue_count == 0 {
            error!(
                client_one_cue_count,
                client_two_cue_count, "client cue volume evidence is incomplete"
            );
            check.completed = true;
            fail_verification(
                diagnostics.as_deref(),
                &mut classification,
                &mut app_exit,
                "client cue volume evidence is incomplete",
            );
            return;
        }
        let report = format!(
            "run_id={}\nprofile={}\nserver_elapsed_ms={}\ntested_preset_id={}\ntested_recipe_fingerprint={}\ntested_accepted_attacks={}\ntested_emitted_deliveries={}\naccepted_shots={}\nhostile_hits={}\napplied_damage={}\ndefeats={}\nserver_cue_count={}\nclient_one_cue_count={}\nclient_two_cue_count={}\nserver_state_mutation_count={}\nclient_one_state_mutation_count={}\nclient_two_state_mutation_count={}\nstate_convergence_client_one_us_median={}\nstate_convergence_client_one_us_p95={}\nstate_convergence_client_two_us_median={}\nstate_convergence_client_two_us_p95={}\nserver_dropped_cues={}\nserver_dropped_records={}\nserver_dropped_timestamps={}\nstate_converged={}\ncue_converged={}\nordered_cue_stream_converged={}\n{}client_one={}client_two={}",
            check.run_id,
            env::var("BRAWLER_NETWORK_PROFILE").unwrap_or_else(|_| "local".to_string()),
            check.started_at.elapsed().as_millis(),
            expected_preset_id.0,
            expected_resolved.recipe_fingerprint.0,
            expected_attacks,
            expected_deliveries,
            accepted_attacks,
            telemetry.hostile_fighter_hits,
            telemetry.applied_damage,
            telemetry.defeats,
            telemetry.cues.len(),
            client_one_cue_count,
            client_two_cue_count,
            evidence.state_mutation_timestamps.len(),
            parse_report_counter(&client_one, "state_mutation_count"),
            parse_report_counter(&client_two, "state_mutation_count"),
            client_one_state_median_us,
            client_one_state_p95_us,
            client_two_state_median_us,
            client_two_state_p95_us,
            telemetry.dropped_cues,
            telemetry.dropped_records,
            telemetry.dropped_accepted_shot_timestamps,
            u8::from(checkpoint_converged),
            u8::from(cue_converged),
            u8::from(cue_converged),
            latency_evidence,
            client_one,
            client_two,
        );
        if let Err(error) = fs::write(&report_path, report) {
            error!(path = %report_path.display(), ?error, "combat report write failed");
            check.completed = true;
            fail_verification(
                diagnostics.as_deref(),
                &mut classification,
                &mut app_exit,
                "combat report write failed",
            );
            return;
        }
    }
    if let Err(error) = fs::write(&path, b"combat-ready\n") {
        error!(path = %path.display(), ?error, "combat readiness signal failed");
        check.completed = true;
        fail_verification(
            diagnostics.as_deref(),
            &mut classification,
            &mut app_exit,
            "combat readiness signal failed",
        );
        return;
    }
    check.completed = true;
    info!(path = %path.display(), "network combat readiness signal written");
}

pub(super) fn parse_client_cue_timestamps(contents: &str) -> Vec<(u64, u128)> {
    contents
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("cue_shot_id=")?;
            let (shot_id, timestamp) = rest.split_once("_epoch_us=")?;
            Some((shot_id.parse().ok()?, timestamp.parse().ok()?))
        })
        .collect()
}

pub(super) fn parse_report_counter(contents: &str, key: &str) -> u64 {
    contents
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{key}=")))
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

pub(super) fn checkpoint_latencies(
    server_candidates: &BTreeMap<String, Vec<(CombatStateSnapshot, u128)>>,
    client_report: &str,
) -> Vec<u128> {
    server_candidates
        .iter()
        .filter_map(|(checkpoint, candidates)| {
            let client_timestamp = parse_report_value(
                client_report,
                &format!("checkpoint_{checkpoint}_observed_epoch_us"),
            )?
            .parse::<u128>()
            .ok()?;
            let client_tick =
                parse_report_value(client_report, &format!("checkpoint_{checkpoint}_tick"))?
                    .parse::<u64>()
                    .ok()?;
            let (_, server_timestamp) = candidates
                .iter()
                .find(|(snapshot, _)| snapshot.authoritative_tick == client_tick)?;
            client_timestamp.checked_sub(*server_timestamp)
        })
        .collect()
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

pub(super) fn median_p95(values: &[u128]) -> Option<(u128, u128)> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let median = sorted[(sorted.len() - 1) / 2];
    let p95_rank = (sorted.len() * 95).saturating_add(99) / 100;
    let p95 = sorted[p95_rank.saturating_sub(1).min(sorted.len() - 1)];
    Some((median, p95))
}

pub(super) fn parse_report_value<'a>(contents: &'a str, key: &str) -> Option<&'a str> {
    contents
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{key}=")))
}

pub(super) fn report_matches_snapshot(
    report: &str,
    key: &str,
    authoritative_snapshot: &CombatStateSnapshot,
) -> bool {
    let Some(encoded) = encode_state_snapshot(authoritative_snapshot) else {
        return false;
    };
    parse_report_value(report, key) == Some(encoded.as_str())
        || report
            .lines()
            .any(|line| line.strip_prefix(&format!("{key}_candidate=")) == Some(encoded.as_str()))
}

pub(super) fn parse_client_cue_stream(contents: &str) -> Vec<CombatCue> {
    contents
        .lines()
        .filter_map(|line| decode_combat_cue(line.strip_prefix("cue_stream=")?))
        .collect()
}
