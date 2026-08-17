//! Terrain wire convergence, recovery, forgery resistance, and restart scenarios.

use super::*;

fn terrain_generation(harness: &mut Harness) -> brawler::terrain::TerrainGeneration {
    let world = harness.server.world_mut();
    world
        .query::<&brawler::terrain::TerrainRoot>()
        .iter(world)
        .next()
        .expect("authoritative terrain root")
        .generation()
}

fn server_match(harness: &mut Harness) -> MatchState {
    let world = harness.server.world_mut();
    let mut query = world.query_filtered::<&MatchState, With<MatchRootMarker>>();
    *query.single(world).unwrap()
}

fn both_clients_terrain_ready(harness: &mut Harness) -> bool {
    (0..harness.clients.len()).all(|index| {
        matches!(
            harness.client_terrain(index).0,
            brawler::terrain::ClientTerrainReadiness::Ready
        )
    })
}

#[test]
fn two_live_clients_converge_on_authoritative_terrain_events() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| (0..2).all(|index| harness.client_is_active(index)));
    harness.step_until(both_clients_terrain_ready);
    let initial_digest = harness.server_terrain_digest();

    harness.inject_terrain_brush(1, (0.0, 0.0), 48.0);
    harness.step_until(|harness| {
        let revision = harness.server_terrain_revision().unwrap();
        revision > 0 && (0..2).all(|index| harness.client_terrain(index).1 == revision)
    });

    let server_digest = harness.server_terrain_digest();
    assert_ne!(server_digest, initial_digest, "the brush changed occupancy");
    for index in 0..2 {
        let (readiness, revision, digest) = harness.client_terrain(index);
        assert!(matches!(
            readiness,
            brawler::terrain::ClientTerrainReadiness::Ready
        ));
        assert_eq!(revision, 1);
        assert_eq!(digest, server_digest, "client {index} matches the server");
    }

    // A second brush at another spot keeps both clients converging in revision order.
    harness.inject_terrain_brush(2, (64.0, -64.0), 48.0);
    harness.step_until(|harness| {
        let revision = harness.server_terrain_revision().unwrap();
        revision > 1 && (0..2).all(|index| harness.client_terrain(index).1 == revision)
    });
    let server_digest = harness.server_terrain_digest();
    for index in 0..2 {
        let (_, revision, digest) = harness.client_terrain(index);
        assert_eq!(revision, 2);
        assert_eq!(digest, server_digest);
    }
}

#[test]
fn impaired_and_late_joining_clients_converge_via_recovery() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| harness.client_is_active(0));
    harness.step_until(both_clients_terrain_ready);
    let initial_digest = harness.server_terrain_digest();

    // The server accrues destruction history before the late client ever connects.
    harness.inject_terrain_brush(1, (8.0, 8.0), 48.0);
    harness.inject_terrain_brush(2, (-72.0, 40.0), 48.0);
    harness.step_until(|harness| harness.server_terrain_revision().unwrap() >= 2);
    harness.step_until(|harness| harness.client_terrain(0).1 == 2);

    // Packet impairment on the live client must not break ordered-reliable convergence.
    harness.arm_packet_impairment(0);
    harness.inject_terrain_brush(3, (40.0, 40.0), 48.0);
    harness.step_until(|harness| harness.client_terrain(0).1 == 3);

    // A newly accepted peer never replays history: it recovers the full snapshot.
    harness.add_client(2);
    harness.step_until(|harness| harness.client_is_active(1));
    harness.step_until(|harness| {
        let revision = harness.server_terrain_revision().unwrap();
        (0..2).all(|index| harness.client_terrain(index).1 == revision)
    });
    let server_digest = harness.server_terrain_digest();
    assert_ne!(server_digest, initial_digest);
    for index in 0..2 {
        let (readiness, _, digest) = harness.client_terrain(index);
        assert!(matches!(
            readiness,
            brawler::terrain::ClientTerrainReadiness::Ready
        ));
        assert_eq!(digest, server_digest);
    }
}

#[test]
fn forged_recovery_requests_cannot_mutate_target_or_amplify() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| (0..2).all(|index| harness.client_is_active(index)));
    harness.step_until(both_clients_terrain_ready);
    let baseline = harness.server_terrain_aggregates();
    let baseline_digest = harness.server_terrain_digest();
    let baseline_revision = harness.server_terrain_revision().unwrap();
    let baseline_clients: Vec<_> = (0..2).map(|index| harness.client_terrain(index)).collect();
    let generation = terrain_generation(&mut harness);

    let mut stale_match = generation;
    stale_match.match_id = brawler::matchplay::MatchId(999);
    harness.send_forged_terrain_request(0, stale_match);
    let mut foreign_map = generation;
    foreign_map.map_instance_id = MapInstanceId(999);
    harness.send_forged_terrain_request(0, foreign_map);
    let mut wrong_fingerprint = generation;
    wrong_fingerprint.terrain_fingerprint = wrong_fingerprint.terrain_fingerprint.wrapping_add(1);
    harness.send_forged_terrain_request(0, wrong_fingerprint);
    for _ in 0..4 {
        harness.step();
    }

    let aggregates = harness.server_terrain_aggregates();
    assert_eq!(
        aggregates.recovery_rejections - baseline.recovery_rejections,
        3,
        "every forged generation is rejected with a counted reason"
    );
    assert_eq!(
        aggregates.recovery_responses - baseline.recovery_responses,
        0,
        "no invalid request amplifies into a response"
    );
    assert_eq!(
        harness.server_terrain_revision().unwrap(),
        baseline_revision
    );
    assert_eq!(harness.server_terrain_digest(), baseline_digest);
    for index in 0..2 {
        let terrain = harness.client_terrain(index);
        assert_eq!(terrain.1, baseline_clients[index].1);
        assert_eq!(terrain.2, baseline_clients[index].2);
    }

    // One valid request is served; an immediate duplicate hits the per-link cooldown.
    // First advance past the cooldown window consumed by the client's initial recovery.
    for _ in 0..35 {
        harness.step();
    }
    let before = harness.server_terrain_aggregates();
    harness.send_forged_terrain_request(0, generation);
    harness.step();
    harness.step();
    harness.send_forged_terrain_request(0, generation);
    harness.step();
    harness.step();
    let aggregates = harness.server_terrain_aggregates();
    assert_eq!(aggregates.recovery_responses - before.recovery_responses, 1);
    assert!(aggregates.recovery_rejections > before.recovery_rejections);
    // The response addresses only the requesting link: the other client is untouched.
    for index in 0..2 {
        let terrain = harness.client_terrain(index);
        assert!(matches!(
            terrain.0,
            brawler::terrain::ClientTerrainReadiness::Ready
        ));
        assert_eq!(terrain.2, baseline_clients[index].2);
    }
}

#[test]
fn restart_returns_server_and_clients_to_revision_zero_ignoring_stale_history() {
    let mut harness = Harness::new_match(2);
    harness.step_until(|harness| {
        (0..2).all(|index| harness.client_is_active(index) && harness.selection_is_complete(index))
    });
    let waiting = server_match(&mut harness);
    for index in 0..2 {
        harness.send_build_selection(
            index,
            BuildSelectionRequest {
                request_id: 10,
                match_id: waiting.match_id,
                selection: BuildSelection::Preset(BuildPresetId(1)),
            },
        );
    }
    harness.step_until(|harness| {
        harness.server_links.iter().all(|link| {
            harness
                .server
                .world()
                .get::<ServerSession>(*link)
                .and_then(|session| session.last_selection_response)
                .is_some_and(|outcome| {
                    outcome.request_id == 10 && outcome.decision == BuildSelectionDecision::Accepted
                })
        })
    });
    for index in 0..2 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: 100,
                match_id: waiting.match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    harness.step_until(|harness| matches!(server_match(harness).phase, MatchPhase::Active { .. }));
    harness.step_until(both_clients_terrain_ready);
    let initial_digest = harness.server_terrain_digest();

    // Destroy terrain so the restart actually has to restore occupancy.
    harness.inject_terrain_brush(1, (0.0, 0.0), 48.0);
    harness.inject_terrain_brush(2, (-64.0, 64.0), 48.0);
    harness.step_until(|harness| {
        let revision = harness.server_terrain_revision().unwrap();
        revision >= 2 && (0..2).all(|index| harness.client_terrain(index).1 == revision)
    });
    assert_ne!(harness.server_terrain_digest(), initial_digest);

    // Complete the match and restart into a fresh authoritative match.
    {
        let world = harness.server.world_mut();
        let mut query =
            world.query_filtered::<&mut brawler::matchplay::WipeoutState, With<MatchRootMarker>>();
        query.single_mut(world).unwrap().target_score = 1;
    }
    let players: Vec<_> = {
        let world = harness.server.world_mut();
        let mut query =
            world.query_filtered::<(&PlayerId, &NetworkEntityId, &TeamId), With<Fighter>>();
        let mut players: Vec<_> = query
            .iter(world)
            .filter(|(player, _, _)| player.0 != 0)
            .map(|(player, network, team)| (*player, *network, *team))
            .collect();
        players.sort_by_key(|(player, _, _)| player.0);
        players
    };
    let source = players
        .iter()
        .find(|(_, _, team)| team.0 == 0)
        .copied()
        .unwrap();
    let target = players
        .iter()
        .find(|(_, _, team)| team.0 == 1)
        .copied()
        .unwrap();
    let tick = harness.server.world().resource::<SimulationTick>().0;
    harness
        .server
        .world_mut()
        .resource_mut::<CombatOutcomeFacts>()
        .0
        .push(CombatOutcomeFact {
            event_id: CombatEventId(50_000),
            tick,
            attack_id: AttackId(50_000),
            source_kind: CombatSourceKind::PrimaryWeapon,
            source_player: Some(source.0),
            source_network_id: Some(source.1),
            source_team: Some(source.2),
            target_network_id: target.1,
            target_kind: brawler::combat::CombatTargetKind::Fighter,
            target_team: target.2,
            preset_id: Some(WeaponPresetId(1)),
            recipe_fingerprint: None,
            position: WorldPoint { x: 0.0, y: 0.0 },
            engagement_distance: 100.0,
            kind: CombatOutcomeKind::Defeat,
        });
    let completed = {
        harness.step_until(|harness| {
            matches!(server_match(harness).phase, MatchPhase::Completed { .. })
        });
        server_match(&mut harness)
    };
    for index in 0..2 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: 200,
                match_id: completed.match_id,
                command: MatchCommand::ReadyForRestart,
            },
        );
    }
    // The completion input lock rejects immediate restart votes; wait it out and re-vote.
    for _ in 0..60 {
        harness.step();
    }
    for index in 0..2 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: 201,
                match_id: completed.match_id,
                command: MatchCommand::ReadyForRestart,
            },
        );
    }
    harness.step_until(|harness| server_match(harness).match_id.0 > completed.match_id.0);
    let restarted = server_match(&mut harness);

    // The server restored revision-zero initial occupancy for the new match generation.
    harness.step_until(|harness| harness.server_terrain_revision() == Some(0));
    assert_eq!(harness.server_terrain_digest(), initial_digest);
    let generation = terrain_generation(&mut harness);
    assert_eq!(generation.match_id, restarted.match_id);

    // Both clients return to identical revision-zero occupancy regardless of whether the
    // reset event or a recovery snapshot carried the transition.
    harness.step_until(|harness| {
        (0..2).all(|index| {
            let terrain = harness.client_terrain(index);
            matches!(terrain.0, brawler::terrain::ClientTerrainReadiness::Ready)
                && terrain.1 == 0
                && terrain.2 == initial_digest
        })
    });
}

#[test]
fn one_arc_landing_erases_multiple_chunks_but_plays_one_landed_cue() {
    let mut harness = Harness::new_match(2);
    harness.step_until(|harness| {
        (0..2).all(|index| harness.client_is_active(index) && harness.selection_is_complete(index))
    });
    let waiting = server_match(&mut harness);
    // Client 0 takes the Controller build (Arc Launcher), client 1 keeps a preset.
    harness.send_build_selection(
        0,
        BuildSelectionRequest {
            request_id: 10,
            match_id: waiting.match_id,
            selection: BuildSelection::Preset(BuildPresetId(3)),
        },
    );
    harness.send_build_selection(
        1,
        BuildSelectionRequest {
            request_id: 10,
            match_id: waiting.match_id,
            selection: BuildSelection::Preset(BuildPresetId(1)),
        },
    );
    harness.step_until(|harness| {
        (0..2).all(|index| {
            harness.server_links.iter().any(|link| {
                harness
                    .server
                    .world()
                    .get::<ServerSession>(*link)
                    .and_then(|session| session.last_selection_response)
                    .is_some_and(|outcome| {
                        outcome.request_id == 10
                            && outcome.decision == BuildSelectionDecision::Accepted
                    })
            }) && harness.selection_is_complete(index)
        })
    });
    for index in 0..2 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: 100,
                match_id: waiting.match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    harness.step_until(|harness| matches!(server_match(harness).phase, MatchPhase::Active { .. }));
    harness.step_until(both_clients_terrain_ready);
    let initial_digest = harness.server_terrain_digest();

    // Fire one Arc lob from west of the block so it lands inside the origin chunk seam.
    let player = harness.controlled_player_id(0);
    {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(
            &PlayerId,
            &mut avian2d::prelude::Position,
            &mut avian2d::prelude::Rotation,
        ), With<Fighter>>();
        for (candidate, mut position, mut rotation) in query.iter_mut(world) {
            if *candidate == player {
                position.0 = Vec2::new(-300.0, 0.0);
                *rotation = avian2d::prelude::Rotation::radians(0.0);
            }
        }
    }
    harness.set_controlled_input(
        0,
        FighterInput::from_axes_with_aim_distance(
            Vec2::ZERO,
            Some(Vec2::X),
            Some(300.0),
            FighterInput::PRIMARY_FIRE,
        ),
    );
    // so exactly one lob lands inside the block.
    for _ in 0..10 {
        harness.step();
    }
    harness.set_controlled_input(0, FighterInput::default());
    harness.step_until(|harness| harness.server_terrain_revision().unwrap() >= 1);

    // The single committed brush split across several chunks.
    {
        let world = harness.server.world();
        let telemetry = world.resource::<brawler::terrain::telemetry::TerrainTelemetry>();
        let record = telemetry
            .records
            .iter()
            .rev()
            .find(|record| {
                matches!(
                    record.outcome,
                    brawler::terrain::telemetry::TerrainTelemetryOutcome::Applied
                )
            })
            .expect("an applied terrain record");
        assert!(
            record.affected_chunks.len() > 1,
            "the origin crater spans a chunk seam: {:?}",
            record.affected_chunks
        );
    }
    assert_ne!(harness.server_terrain_digest(), initial_digest);
    harness.step_until(|harness| harness.client_terrain(0).1 >= 1);

    // Exactly one landed-delivery cue reached the client for that one delivery,
    // independent of how many chunks the erasure split across.
    let landed = harness
        .client_cues(0)
        .iter()
        .filter(|cue| matches!(cue, CombatCue::LobLanded { .. }))
        .count();
    assert_eq!(landed, 1, "one landed delivery plays one landed cue");
    let (readiness, _, _) = harness.client_terrain(0);
    assert!(matches!(
        readiness,
        brawler::terrain::ClientTerrainReadiness::Ready
    ));
}
