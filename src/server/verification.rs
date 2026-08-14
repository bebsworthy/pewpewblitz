//! Process-level movement/combat evidence validation and report parsing.
#![allow(clippy::wildcard_imports)]

use super::*;

pub(super) fn verify_process_movement(
    mut check: ResMut<ProcessMovementCheck>,
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
    if check
        .initial_tick
        .is_none_or(|initial_tick| tick.0 < initial_tick.saturating_add(120))
    {
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
            app_exit.write(AppExit::error());
        }
        check.completed = true;
    } else {
        error!(
            tick = tick.0,
            moved, aimed, "network movement smoke assertion failed"
        );
        app_exit.write(AppExit::error());
        check.completed = true;
    }
}

#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
pub(super) fn verify_process_combat(
    mut check: ResMut<ProcessCombatCheck>,
    telemetry: Res<CombatTelemetry>,
    weapon_telemetry: Res<WeaponTelemetry>,
    evidence: Res<CombatEvidenceSnapshots>,
    catalog: Res<WeaponCatalogResource>,
    fighters: Res<crate::combat::FighterDefinitions>,
    sessions: Query<&ServerSession, With<LinkOf>>,
    selected_fighters: Query<(&SelectedBuild, &ResolvedWeapon), With<Fighter>>,
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
        app_exit.write(AppExit::error());
        return;
    };
    let Some(_) = catalog.0.preset(expected_preset_id) else {
        error!(
            preset_id = expected_preset_id.0,
            "combat assertion requested an unknown preset"
        );
        check.completed = true;
        app_exit.write(AppExit::error());
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
        app_exit.write(AppExit::error());
        return;
    };
    let tested_fighter = selected_fighters.iter().any(|(build, resolved)| {
        build.source_preset_id == Some(expected_preset_id)
            && resolved.source_preset_id == Some(expected_preset_id)
            && resolved.recipe_fingerprint == expected_resolved.recipe_fingerprint
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
        app_exit.write(AppExit::error());
        return;
    };

    let Some(client_ready_dir) = check.client_ready_dir.clone() else {
        error!("combat process assertion is enabled without a client evidence directory");
        check.completed = true;
        app_exit.write(AppExit::error());
        return;
    };
    let client_one_path = client_ready_dir.join("client-1.ready");
    let client_two_path = client_ready_dir.join("client-2.ready");
    let client_one = match fs::read_to_string(&client_one_path) {
        Ok(contents) => contents,
        Err(error) => {
            error!(path = %client_one_path.display(), ?error, "client one combat evidence could not be read");
            check.completed = true;
            app_exit.write(AppExit::error());
            return;
        }
    };
    let client_two = match fs::read_to_string(&client_two_path) {
        Ok(contents) => contents,
        Err(error) => {
            error!(path = %client_two_path.display(), ?error, "client two combat evidence could not be read");
            check.completed = true;
            app_exit.write(AppExit::error());
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
        app_exit.write(AppExit::error());
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
        app_exit.write(AppExit::error());
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
        app_exit.write(AppExit::error());
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
        app_exit.write(AppExit::error());
        return;
    };
    let Some((client_two_state_median_us, client_two_state_p95_us)) =
        median_p95(&client_two_state_latencies)
    else {
        error!("client two state convergence latency evidence is incomplete");
        check.completed = true;
        app_exit.write(AppExit::error());
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
            app_exit.write(AppExit::error());
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
            app_exit.write(AppExit::error());
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
            app_exit.write(AppExit::error());
            return;
        }
    }
    if let Err(error) = fs::write(&path, b"combat-ready\n") {
        error!(path = %path.display(), ?error, "combat readiness signal failed");
        check.completed = true;
        app_exit.write(AppExit::error());
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

pub(super) fn required_process_checkpoints(preset_id: WeaponPresetId) -> &'static [&'static str] {
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
