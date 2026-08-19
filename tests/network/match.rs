use super::*;

fn server_match(harness: &mut Harness) -> MatchState {
    let world = harness.server.world_mut();
    let mut query = world.query_filtered::<&MatchState, With<MatchRootMarker>>();
    *query.single(world).expect("one server match")
}

fn server_wipeout(harness: &mut Harness) -> brawler::matchplay::WipeoutState {
    let world = harness.server.world_mut();
    let mut query =
        world.query_filtered::<&brawler::matchplay::WipeoutState, With<MatchRootMarker>>();
    *query.single(world).expect("one server wipeout state")
}

fn client_match(harness: &mut Harness, index: usize) -> Option<MatchState> {
    let world = harness.clients[index].world_mut();
    let mut query = world.query_filtered::<&MatchState, With<MatchRootMarker>>();
    query.iter(world).next().copied()
}

fn server_player_position(harness: &mut Harness, player_id: PlayerId) -> Vec2 {
    let world = harness.server.world_mut();
    let mut fighters = world.query_filtered::<(&PlayerId, &Position), With<Fighter>>();
    fighters
        .iter(world)
        .find(|(player, _)| **player == player_id)
        .map(|(_, position)| position.0)
        .expect("server player position")
}

#[test]
fn ready_commands_are_link_scoped_idempotent_and_match_scoped() {
    let mut harness = Harness::new_match(1);
    harness.step_until(|harness| harness.client_is_active(0) && harness.selection_is_complete(0));
    let state = server_match(&mut harness);
    let ready = MatchCommandRequest {
        request_id: 9,
        match_id: state.match_id,
        command: MatchCommand::SetReady(true),
    };
    harness.send_match_command(0, ready);
    harness.send_match_command(0, ready);
    harness.step();
    let session = harness
        .server
        .world()
        .get::<ServerSession>(harness.server_links[0])
        .unwrap();
    assert_eq!(
        session.last_match_outcome.unwrap().decision,
        MatchCommandDecision::Accepted
    );
    harness.send_match_command(
        0,
        MatchCommandRequest {
            request_id: 10,
            match_id: brawler::matchplay::MatchId(state.match_id.0 + 1),
            command: MatchCommand::SetReady(false),
        },
    );
    harness.step();
    let session = harness
        .server
        .world()
        .get::<ServerSession>(harness.server_links[0])
        .unwrap();
    assert_eq!(
        session.last_match_outcome.unwrap().decision,
        MatchCommandDecision::WrongMatch
    );
    let world = harness.server.world_mut();
    let mut query = world.query_filtered::<&MatchParticipant, With<Fighter>>();
    assert!(query.single(world).unwrap().ready);

    {
        let world = harness.clients[0].world_mut();
        let mut query =
            world.query_filtered::<&mut brawler::matchplay::WipeoutState, With<MatchRootMarker>>();
        query.single_mut(world).unwrap().team_scores = [u16::MAX, u16::MAX];
    }
    for _ in 0..6 {
        harness.step();
    }
    assert_eq!(server_wipeout(&mut harness).team_scores, [0, 0]);
}

#[test]
fn countdown_cancellation_returns_to_waiting_and_clears_ready_quorum() {
    let mut harness = Harness::new_match(2);
    harness.step_until(|harness| {
        (0..2).all(|index| harness.client_is_active(index) && harness.selection_is_complete(index))
    });
    let waiting = server_match(&mut harness);
    for index in 0..2 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: 1,
                match_id: waiting.match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    harness
        .step_until(|harness| matches!(server_match(harness).phase, MatchPhase::Countdown { .. }));
    harness.send_match_command(
        0,
        MatchCommandRequest {
            request_id: 2,
            match_id: waiting.match_id,
            command: MatchCommand::SetReady(false),
        },
    );
    harness.step_until(|harness| matches!(server_match(harness).phase, MatchPhase::Waiting));
    let world = harness.server.world_mut();
    let mut participants = world.query_filtered::<&MatchParticipant, With<Fighter>>();
    assert!(
        participants
            .iter(world)
            .all(|participant| !participant.ready)
    );

    harness.clients[0].world_mut().trigger(Disconnect {
        entity: harness.client_entities[0],
    });
    harness.step_until(|harness| harness.server_ids().len() == 1);
    assert!(matches!(
        server_match(&mut harness).phase,
        MatchPhase::Waiting
    ));
    harness.add_client(3);
    harness.step_until(|harness| harness.client_is_active(2) && harness.selection_is_complete(2));
    for (index, request_id) in [(1, 3), (2, 1)] {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id,
                match_id: waiting.match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    harness
        .step_until(|harness| matches!(server_match(harness).phase, MatchPhase::Countdown { .. }));
    harness.clients[2].world_mut().trigger(Disconnect {
        entity: harness.client_entities[2],
    });
    harness.step_until(|harness| {
        harness.server_ids().len() == 1
            && matches!(server_match(harness).phase, MatchPhase::Waiting)
    });
    let world = harness.server.world_mut();
    let mut participants = world.query_filtered::<&MatchParticipant, With<Fighter>>();
    assert!(
        participants
            .iter(world)
            .all(|participant| !participant.ready)
    );
}

#[test]
fn countdown_departure_cancels_a_still_valid_two_versus_two_roster() {
    let mut harness = Harness::new_match(4);
    harness.step_until(|harness| {
        (0..4).all(|index| harness.client_is_active(index) && harness.selection_is_complete(index))
    });
    let waiting = server_match(&mut harness);
    for index in 0..4 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: 1,
                match_id: waiting.match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    harness
        .step_until(|harness| matches!(server_match(harness).phase, MatchPhase::Countdown { .. }));
    harness.clients[0].world_mut().trigger(Disconnect {
        entity: harness.client_entities[0],
    });
    harness.step_until(|harness| {
        harness.server_ids().len() == 3
            && matches!(server_match(harness).phase, MatchPhase::Waiting)
    });
    {
        let world = harness.server.world_mut();
        let mut participants = world.query_filtered::<&MatchParticipant, With<Fighter>>();
        assert!(
            participants
                .iter(world)
                .all(|participant| !participant.ready)
        );
    }
    for _ in 0..200 {
        harness.step();
    }
    assert!(matches!(
        server_match(&mut harness).phase,
        MatchPhase::Waiting
    ));
}

#[test]
fn same_frame_admission_assigns_stable_teams_by_remote_identity() {
    fn assignments() -> Vec<TeamId> {
        let mut harness = Harness::new_match(4);
        harness.step_until(|harness| (0..4).all(|index| harness.client_is_active(index)));
        (0..4)
            .map(|index| {
                let player = harness.controlled_player_id(index);
                let world = harness.server.world_mut();
                let mut query = world.query_filtered::<(&PlayerId, &TeamId), With<Fighter>>();
                *query
                    .iter(world)
                    .find(|(candidate, _)| **candidate == player)
                    .expect("admitted player has a team")
                    .1
            })
            .collect()
    }

    let first = assignments();
    let second = assignments();
    assert_eq!(first, second);
    assert_eq!(first, vec![TeamId(0), TeamId(1), TeamId(0), TeamId(1)]);
}

#[test]
fn initial_admission_uses_the_shared_spawn_selector() {
    let mut harness = Harness::new_match(4);
    harness.step_until(|harness| (0..4).all(|index| harness.client_is_active(index)));
    let state = server_match(&mut harness);
    let tuning = *harness.server.world().resource::<MovementTuning>();
    let spawn_points = harness
        .server
        .world()
        .resource::<SpawnPointCatalog>()
        .clone();
    let mut fighters = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(
            &PlayerId,
            &TeamId,
            &Position,
            &SpawnAssignment,
        ), (With<Fighter>, With<MatchParticipant>)>();
        query
            .iter(world)
            .map(|(player, team, position, assignment)| (*player, *team, position.0, *assignment))
            .collect::<Vec<_>>()
    };
    fighters.sort_by_key(|(player, _, _, _)| player.0);
    let mut living = Vec::new();
    for (player, team, position, assignment) in fighters {
        let candidates = spawn_points
            .0
            .get(&team.0)
            .unwrap()
            .iter()
            .map(|point| brawler::matchplay::SpawnCandidate {
                id: point.spawn_point_id,
                position: point.position,
                facing: point.facing,
            })
            .collect();
        let expected = brawler::matchplay::select_spawn(
            candidates,
            &living,
            team,
            tuning.radius * 2.0 + tuning.skin_width,
            state.match_id,
            player,
            0,
        )
        .unwrap();
        assert_eq!(assignment.spawn_point_id, expected.id);
        assert_eq!(position, expected.position);
        living.push((team, position));
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn four_clients_converge_named_builds_and_restart_three_authoritative_matches() {
    let mut harness = Harness::new_match(4);
    harness.step_until(|harness| {
        (0..4).all(|index| harness.client_is_active(index) && harness.selection_is_complete(index))
    });
    let waiting = server_match(&mut harness);
    for index in 0..4 {
        harness.send_build_selection(
            index,
            BuildSelectionRequest {
                request_id: 10,
                match_id: waiting.match_id,
                selection: BuildSelection::Preset(BuildPresetId(u16::try_from(index + 1).unwrap())),
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
    for index in 0..4 {
        let player = harness.controlled_player_id(index);
        let world = harness.server.world_mut();
        let mut query = world
            .query_filtered::<(&PlayerId, &brawler::builds::ResolvedMatchLoadout), With<Fighter>>();
        assert_eq!(
            query
                .iter(world)
                .find(|(candidate, _)| **candidate == player)
                .map(|(_, loadout)| loadout.identity.source_build_preset_id)
                .unwrap(),
            Some(BuildPresetId(u16::try_from(index + 1).unwrap()))
        );
    }
    harness.step_until(|harness| {
        (0..4).all(|index| {
            let entity = harness.controlled_entity(index);
            harness.clients[index]
                .world()
                .get::<brawler::builds::ResolvedMatchLoadout>(entity)
                .is_some_and(|loadout| {
                    loadout.identity.source_build_preset_id
                        == Some(BuildPresetId(u16::try_from(index + 1).unwrap()))
                })
        })
    });
    let mut teams = [0_u8; 2];
    {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(&TeamId, &MatchParticipant), With<Fighter>>();
        for (team, participant) in query.iter(world) {
            if participant.match_id == waiting.match_id {
                teams[usize::from(team.0)] += 1;
            }
        }
    }
    assert_eq!(teams, [2, 2]);
    for index in 0..4 {
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
    let active = server_match(&mut harness);
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(Vec2::X), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| {
        let world = harness.server.world_mut();
        let mut projectiles = world.query_filtered::<&MatchMember, With<Projectile>>();
        projectiles
            .iter(world)
            .any(|member| member.0 == active.match_id)
    });
    harness.set_controlled_input(0, FighterInput::default());
    {
        let world = harness.server.world_mut();
        let mut query =
            world.query_filtered::<&mut brawler::matchplay::WipeoutState, With<MatchRootMarker>>();
        query.single_mut(world).unwrap().target_score = 1;
    }
    let tick = harness.server.world().resource::<SimulationTick>().0;
    let players = {
        let world = harness.server.world_mut();
        let mut query =
            world.query_filtered::<(&PlayerId, &NetworkEntityId, &TeamId), With<Fighter>>();
        let mut values: Vec<_> = query
            .iter(world)
            .filter(|(player, _, _)| player.0 != 0)
            .map(|(p, n, t)| (*p, *n, *t))
            .collect();
        values.sort_by_key(|(player, _, _)| player.0);
        values
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
    harness
        .step_until(|harness| matches!(server_match(harness).phase, MatchPhase::Completed { .. }));
    let completed = server_match(&mut harness);
    assert_eq!(server_wipeout(&mut harness).team_scores, [1, 0]);
    {
        let world = harness.server.world_mut();
        let mut stale_lifecycle = world.query_filtered::<(), (
            With<MatchParticipant>,
            Or<(
                With<ActiveCombatant>,
                With<brawler::matchplay::RespawnState>,
                With<brawler::matchplay::SpawnProtection>,
            )>,
        )>();
        assert_eq!(stale_lifecycle.iter(world).count(), 0);
    }
    harness
        .step_until(|harness| (0..4).all(|index| client_match(harness, index) == Some(completed)));
    harness.send_match_command(
        0,
        MatchCommandRequest {
            request_id: 101,
            match_id: completed.match_id,
            command: MatchCommand::ReadyForRestart,
        },
    );
    harness.step();
    assert_eq!(
        harness
            .server
            .world()
            .get::<ServerSession>(harness.server_links[0])
            .unwrap()
            .last_match_outcome
            .unwrap()
            .decision,
        MatchCommandDecision::Locked
    );
    for _ in 0..60 {
        harness.step();
    }
    for index in 0..4 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: if index == 0 { 102 } else { 101 },
                match_id: completed.match_id,
                command: MatchCommand::ReadyForRestart,
            },
        );
    }
    harness.step_until(|harness| server_match(harness).match_id.0 > active.match_id.0);
    let restarted = server_match(&mut harness);
    assert!(matches!(restarted.phase, MatchPhase::Waiting));
    assert_eq!(server_wipeout(&mut harness).team_scores, [0, 0]);
    let world = harness.server.world_mut();
    let mut stale_projectiles = world.query_filtered::<&MatchMember, With<Projectile>>();
    assert!(
        stale_projectiles
            .iter(world)
            .all(|member| member.0 != active.match_id)
    );
    let mut members = world.query_filtered::<(&MatchMember, &MatchParticipant), With<Fighter>>();
    assert!(members.iter(world).all(|(member, participant)| {
        member.0 == restarted.match_id && participant.match_id == restarted.match_id
    }));
    let mut stale_fighter_state = world.query_filtered::<(), (
        With<MatchParticipant>,
        Or<(
            With<Defeated>,
            With<brawler::matchplay::RespawnState>,
            With<brawler::matchplay::SpawnProtection>,
            With<ActiveCombatant>,
        )>,
    )>();
    assert_eq!(stale_fighter_state.iter(world).count(), 0);
    assert!(world.resource::<ActiveAttackTrackers>().active.is_empty());
    assert!(
        world
            .resource::<ActiveAttackTrackers>()
            .completed
            .is_empty()
    );
    assert!(world.resource::<CombatOutbox>().0.is_empty());
    assert!(world.resource::<CombatOutcomeFacts>().0.is_empty());
    assert!(world.resource::<Messages<PendingPayload>>().is_empty());
    assert!(world.resource::<Messages<PendingDelivery>>().is_empty());
    assert!(world.resource::<Messages<MeleeAttack>>().is_empty());

    let _ = world;
    for index in 0..4 {
        harness.send_build_selection(
            index,
            BuildSelectionRequest {
                request_id: 20,
                match_id: restarted.match_id,
                selection: BuildSelection::Preset(BuildPresetId(
                    u16::try_from((index + 1) % 4 + 1).unwrap(),
                )),
            },
        );
    }
    harness.step_until(|harness| {
        (0..4).all(|index| {
            let player = harness.controlled_player_id(index);
            let expected = BuildPresetId(u16::try_from((index + 1) % 4 + 1).unwrap());
            let world = harness.server.world_mut();
            let mut query = world.query_filtered::<
                (&PlayerId, &brawler::builds::ResolvedMatchLoadout),
                With<Fighter>,
            >();
            query.iter(world).any(|(candidate, loadout)| {
                *candidate == player && loadout.identity.source_build_preset_id == Some(expected)
            })
        })
    });
    for index in 0..4 {
        let player = harness.controlled_player_id(index);
        let expected = BuildPresetId(u16::try_from((index + 1) % 4 + 1).unwrap());
        let world = harness.server.world_mut();
        let mut query = world
            .query_filtered::<(&PlayerId, &brawler::builds::ResolvedMatchLoadout), With<Fighter>>();
        assert_eq!(
            query
                .iter(world)
                .find(|(candidate, _)| **candidate == player)
                .map(|(_, loadout)| loadout.identity.source_build_preset_id)
                .unwrap(),
            Some(expected)
        );
    }
    harness.step_until(|harness| {
        (0..4).all(|index| {
            let entity = harness.controlled_entity(index);
            let expected = BuildPresetId(u16::try_from((index + 1) % 4 + 1).unwrap());
            harness.clients[index]
                .world()
                .get::<brawler::builds::ResolvedMatchLoadout>(entity)
                .is_some_and(|loadout| loadout.identity.source_build_preset_id == Some(expected))
        })
    });

    for index in 0..4 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: if index == 0 { 103 } else { 102 },
                match_id: restarted.match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    harness.step_until(|harness| matches!(server_match(harness).phase, MatchPhase::Active { .. }));
    {
        let world = harness.server.world_mut();
        let mut query =
            world.query_filtered::<&mut brawler::matchplay::WipeoutState, With<MatchRootMarker>>();
        query.single_mut(world).unwrap().target_score = 1;
    }
    let second_tick = harness.server.world().resource::<SimulationTick>().0;
    harness
        .server
        .world_mut()
        .resource_mut::<CombatOutcomeFacts>()
        .0
        .push(CombatOutcomeFact {
            event_id: CombatEventId(50_001),
            tick: second_tick,
            attack_id: AttackId(50_001),
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
    harness
        .step_until(|harness| matches!(server_match(harness).phase, MatchPhase::Completed { .. }));
    let second_completed = server_match(&mut harness);
    for _ in 0..60 {
        harness.step();
    }
    for index in 0..4 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: if index == 0 { 104 } else { 103 },
                match_id: second_completed.match_id,
                command: MatchCommand::ReadyForRestart,
            },
        );
    }
    harness.step_until(|harness| server_match(harness).match_id != second_completed.match_id);
    let twice_restarted = server_match(&mut harness);
    assert!(twice_restarted.match_id.0 > restarted.match_id.0);
    assert!(matches!(twice_restarted.phase, MatchPhase::Waiting));
    for index in 0..4 {
        harness.send_build_selection(
            index,
            BuildSelectionRequest {
                request_id: 30,
                match_id: twice_restarted.match_id,
                selection: BuildSelection::Preset(BuildPresetId(u16::try_from(index + 1).unwrap())),
            },
        );
    }
    harness.step_until(|harness| {
        (0..4).all(|index| {
            let player = harness.controlled_player_id(index);
            let expected = BuildPresetId(u16::try_from(index + 1).unwrap());
            let world = harness.server.world_mut();
            let mut query = world.query_filtered::<
                (&PlayerId, &brawler::builds::ResolvedMatchLoadout),
                With<Fighter>,
            >();
            query.iter(world).any(|(candidate, loadout)| {
                *candidate == player && loadout.identity.source_build_preset_id == Some(expected)
            })
        })
    });
    for index in 0..4 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: if index == 0 { 105 } else { 104 },
                match_id: twice_restarted.match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    harness.step_until(|harness| matches!(server_match(harness).phase, MatchPhase::Active { .. }));
    {
        let world = harness.server.world_mut();
        let mut query =
            world.query_filtered::<&mut brawler::matchplay::WipeoutState, With<MatchRootMarker>>();
        query.single_mut(world).unwrap().target_score = 1;
    }
    let third_tick = harness.server.world().resource::<SimulationTick>().0;
    harness
        .server
        .world_mut()
        .resource_mut::<CombatOutcomeFacts>()
        .0
        .push(CombatOutcomeFact {
            event_id: CombatEventId(50_002),
            tick: third_tick,
            attack_id: AttackId(50_002),
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
    harness
        .step_until(|harness| matches!(server_match(harness).phase, MatchPhase::Completed { .. }));
    let third_completed = server_match(&mut harness);
    for _ in 0..60 {
        harness.step();
    }
    for index in 0..4 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: if index == 0 { 106 } else { 105 },
                match_id: third_completed.match_id,
                command: MatchCommand::ReadyForRestart,
            },
        );
    }
    harness.step_until(|harness| server_match(harness).match_id != third_completed.match_id);
    let thrice_restarted = server_match(&mut harness);
    assert!(thrice_restarted.match_id.0 > twice_restarted.match_id.0);
    assert!(matches!(thrice_restarted.phase, MatchPhase::Waiting));
    let world = harness.server.world_mut();
    let mut roots = world.query_filtered::<(), With<MatchRootMarker>>();
    assert_eq!(roots.iter(world).count(), 1);
    let mut fighters = world.query_filtered::<(), (With<Fighter>, With<MatchParticipant>)>();
    assert_eq!(fighters.iter(world).count(), 4);
    let mut projectiles = world.query_filtered::<(), With<Projectile>>();
    assert_eq!(projectiles.iter(world).count(), 0);
    let mut sentries = world.query_filtered::<(), With<brawler::abilities::Sentry>>();
    assert_eq!(sentries.iter(world).count(), 0);
    let mut runtime = world.query_filtered::<(
        &brawler::builds::AbilityState,
        &brawler::builds::PassiveRuntimeState,
    ), With<Fighter>>();
    assert!(runtime.iter(world).all(|(ability, passives)| {
        *ability == brawler::builds::AbilityState::default()
            && *passives == brawler::builds::PassiveRuntimeState::default()
    }));
    assert!(world.resource::<CombatOutcomeFacts>().0.is_empty());
    assert!(world.resource::<ActiveAttackTrackers>().active.is_empty());
    assert!(world.resource::<CombatOutbox>().0.is_empty());
    assert_eq!(
        world
            .resource::<brawler::matchplay::MatchTelemetry>()
            .summaries
            .len(),
        3
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn active_disconnect_continues_shorthanded_then_forfeits_empty_team() {
    let mut harness = Harness::new_match(4);
    harness.step_until(|harness| {
        (0..4).all(|index| harness.client_is_active(index) && harness.selection_is_complete(index))
    });
    let waiting = server_match(&mut harness);
    for index in 0..4 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: 1,
                match_id: waiting.match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    harness.step_until(|harness| matches!(server_match(harness).phase, MatchPhase::Active { .. }));
    harness.add_client(5);
    harness.step_until(|harness| {
        let world = harness.clients[4].world_mut();
        let mut query = world.query_filtered::<&ClientJoinStatus, With<Client>>();
        query.iter(world).any(|status| {
            matches!(
                status.phase,
                ClientJoinPhase::Rejected(brawler::protocol::MatchJoinRejection::MatchInProgress)
            )
        })
    });
    assert_eq!(harness.server_ids().len(), 4);
    let team_zero_clients: Vec<_> = (0..4)
        .filter(|index| {
            let player = harness.controlled_player_id(*index);
            let world = harness.server.world_mut();
            let mut query = world.query_filtered::<(&PlayerId, &TeamId), With<Fighter>>();
            query
                .iter(world)
                .find(|(candidate, _)| **candidate == player)
                .unwrap()
                .1
                .0
                == 0
        })
        .collect();
    assert_eq!(team_zero_clients.len(), 2);
    let first = team_zero_clients[0];
    harness.clients[first].world_mut().trigger(Disconnect {
        entity: harness.client_entities[first],
    });
    harness.step_until(|harness| harness.server_ids().len() == 3);
    assert!(matches!(
        server_match(&mut harness).phase,
        MatchPhase::Active { .. }
    ));
    let pending_respawn = {
        let tick = harness.server.world().resource::<SimulationTick>().0;
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(Entity, &TeamId), With<MatchParticipant>>();
        let entity = query.iter(world).find(|(_, team)| team.0 == 1).unwrap().0;
        world.entity_mut(entity).insert((
            brawler::matchplay::RespawnState {
                respawn_at_tick: tick.saturating_add(1),
            },
            brawler::matchplay::SpawnProtection {
                expires_at_tick: tick.saturating_add(100),
            },
        ));
        entity
    };
    let second = team_zero_clients[1];
    harness.clients[second].world_mut().trigger(Disconnect {
        entity: harness.client_entities[second],
    });
    harness
        .step_until(|harness| matches!(server_match(harness).phase, MatchPhase::Completed { .. }));
    assert!(matches!(
        server_match(&mut harness).phase,
        MatchPhase::Completed {
            result: brawler::matchplay::MatchResult::Forfeit {
                winner: TeamId(1),
                departed_team: TeamId(0)
            },
            ..
        }
    ));
    assert!(
        harness
            .server
            .world()
            .get::<brawler::matchplay::RespawnState>(pending_respawn)
            .is_none()
    );
    assert!(
        harness
            .server
            .world()
            .get::<brawler::matchplay::SpawnProtection>(pending_respawn)
            .is_none()
    );
    let completed = server_match(&mut harness);
    let team_one_clients: Vec<_> = (0..4)
        .filter(|index| !team_zero_clients.contains(index))
        .collect();
    let completed_departure = team_one_clients[0];
    harness.clients[completed_departure]
        .world_mut()
        .trigger(Disconnect {
            entity: harness.client_entities[completed_departure],
        });
    harness.step_until(|harness| harness.server_ids().len() == 1);
    assert_eq!(server_match(&mut harness), completed);
    for _ in 0..60 {
        harness.step();
    }
    for index in team_one_clients
        .into_iter()
        .filter(|index| *index != completed_departure)
    {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: 2,
                match_id: completed.match_id,
                command: MatchCommand::ReadyForRestart,
            },
        );
    }
    harness.step_until(|harness| server_match(harness).match_id != completed.match_id);
    assert!(matches!(
        server_match(&mut harness).phase,
        MatchPhase::Waiting
    ));

    harness.add_client(6);
    harness.step_until(|harness| harness.client_is_active(5) && harness.selection_is_complete(5));
    let admitted = harness.controlled_player_id(5);
    let world = harness.server.world_mut();
    let mut fighters = world.query_filtered::<(&PlayerId, &TeamId), With<Fighter>>();
    assert_eq!(
        fighters
            .iter(world)
            .find(|(player, _)| **player == admitted)
            .unwrap()
            .1,
        &TeamId(0)
    );
}

#[test]
fn simultaneous_last_team_disconnect_draws_and_empty_roster_restarts() {
    let mut harness = Harness::new_match(2);
    harness.step_until(|harness| {
        (0..2).all(|index| harness.client_is_active(index) && harness.selection_is_complete(index))
    });
    let waiting = server_match(&mut harness);
    for index in 0..2 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: 1,
                match_id: waiting.match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    harness.step_until(|harness| matches!(server_match(harness).phase, MatchPhase::Active { .. }));
    for index in 0..2 {
        harness.clients[index].world_mut().trigger(Disconnect {
            entity: harness.client_entities[index],
        });
    }
    harness
        .step_until(|harness| matches!(server_match(harness).phase, MatchPhase::Completed { .. }));
    let completed = server_match(&mut harness);
    assert!(matches!(
        completed.phase,
        MatchPhase::Completed {
            result: brawler::matchplay::MatchResult::Draw,
            ..
        }
    ));
    harness.step_until(|harness| server_match(harness).match_id != completed.match_id);
    let restarted = server_match(&mut harness);
    assert!(matches!(restarted.phase, MatchPhase::Waiting));
    assert!(restarted.match_id.0 > completed.match_id.0);
    assert!(harness.server_ids().is_empty());
}

#[test]
#[allow(clippy::too_many_lines)]
fn fighter_intent_is_gated_in_waiting_countdown_and_completed() {
    let mut harness = Harness::new_match(2);
    harness.step_until(|harness| {
        (0..2).all(|index| harness.client_is_active(index) && harness.selection_is_complete(index))
    });
    let waiting = server_match(&mut harness);
    let controlled = harness.controlled_player_id(0);
    let initial = server_player_position(&mut harness, controlled);
    let active_input = FighterInput::from_axes(Vec2::X, Some(Vec2::X), FighterInput::PRIMARY_FIRE);
    harness.set_controlled_input(0, active_input);
    for _ in 0..10 {
        harness.step();
    }
    assert_eq!(server_player_position(&mut harness, controlled), initial);
    {
        let world = harness.server.world_mut();
        let mut projectiles = world.query_filtered::<(), With<Projectile>>();
        assert_eq!(projectiles.iter(world).count(), 0);
    }
    for index in 0..2 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: 1,
                match_id: waiting.match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    harness
        .step_until(|harness| matches!(server_match(harness).phase, MatchPhase::Countdown { .. }));
    for _ in 0..10 {
        harness.step();
    }
    assert_eq!(server_player_position(&mut harness, controlled), initial);
    harness.step_until(|harness| matches!(server_match(harness).phase, MatchPhase::Active { .. }));
    harness.step_until(|harness| {
        server_player_position(harness, controlled) != initial
            && harness
                .server
                .world()
                .resource::<WeaponTelemetry>()
                .accepted_attacks
                .values()
                .sum::<u64>()
                > 0
    });
    let players = {
        let world = harness.server.world_mut();
        let mut query =
            world.query_filtered::<(&PlayerId, &NetworkEntityId, &TeamId), With<Fighter>>();
        query
            .iter(world)
            .filter(|(player, _, _)| player.0 != 0)
            .map(|(player, network, team)| (*player, *network, *team))
            .collect::<Vec<_>>()
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
    {
        let world = harness.server.world_mut();
        let mut roots =
            world.query_filtered::<&mut brawler::matchplay::WipeoutState, With<MatchRootMarker>>();
        roots.single_mut(world).unwrap().target_score = 1;
    }
    let tick = harness.server.world().resource::<SimulationTick>().0;
    harness
        .server
        .world_mut()
        .resource_mut::<CombatOutcomeFacts>()
        .0
        .push(CombatOutcomeFact {
            event_id: CombatEventId(60_000),
            tick,
            attack_id: AttackId(60_000),
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
    harness
        .step_until(|harness| matches!(server_match(harness).phase, MatchPhase::Completed { .. }));
    harness.step();
    {
        let world = harness.server.world_mut();
        let mut active =
            world.query_filtered::<(), (With<MatchParticipant>, With<ActiveCombatant>)>();
        assert_eq!(active.iter(world).count(), 0);
    }
    let completed_position = server_player_position(&mut harness, controlled);
    let accepted_before: u64 = harness
        .server
        .world()
        .resource::<WeaponTelemetry>()
        .accepted_attacks
        .values()
        .sum();
    for _ in 0..10 {
        harness.step();
    }
    assert_eq!(
        server_player_position(&mut harness, controlled),
        completed_position
    );
    let accepted_after: u64 = harness
        .server
        .world()
        .resource::<WeaponTelemetry>()
        .accepted_attacks
        .values()
        .sum();
    assert_eq!(accepted_after, accepted_before);
}

#[test]
#[allow(clippy::too_many_lines)]
fn spawn_protection_blocks_hostile_payload_allows_movement_breaks_and_expires() {
    let mut harness = Harness::new_match(2);
    harness.step_until(|harness| {
        (0..2).all(|index| harness.client_is_active(index) && harness.selection_is_complete(index))
    });
    let waiting = server_match(&mut harness);
    for index in 0..2 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: 1,
                match_id: waiting.match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    harness.step_until(|harness| matches!(server_match(harness).phase, MatchPhase::Active { .. }));
    let source_player = harness.controlled_player_id(0);
    let target_player = harness.controlled_player_id(1);
    let (source_entity, target_entity, target_health) = {
        let world = harness.server.world_mut();
        let mut fighters =
            world.query_filtered::<(Entity, &PlayerId, &CurrentHealth), With<Fighter>>();
        let source = fighters
            .iter(world)
            .find(|(_, player, _)| **player == source_player)
            .map(|(entity, _, health)| (entity, health.0))
            .unwrap();
        let target = fighters
            .iter(world)
            .find(|(_, player, _)| **player == target_player)
            .map(|(entity, _, health)| (entity, health.0))
            .unwrap();
        (source.0, target.0, target.1)
    };
    harness
        .server
        .world_mut()
        .entity_mut(source_entity)
        .insert(Position::from_xy(-140.0, 160.0));
    harness
        .server
        .world_mut()
        .entity_mut(target_entity)
        .insert(Position::from_xy(140.0, 160.0));
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(Vec2::X), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .resource::<brawler::matchplay::MatchTelemetry>()
            .records
            .iter()
            .any(|fact| matches!(fact.kind, CombatOutcomeKind::ProtectedContact))
    });
    assert_eq!(
        harness
            .server
            .world()
            .get::<CurrentHealth>(target_entity)
            .unwrap()
            .0,
        target_health
    );
    assert!(
        harness
            .server
            .world()
            .get::<brawler::matchplay::SpawnProtection>(source_entity)
            .is_none()
    );
    assert!(
        harness
            .server
            .world()
            .get::<brawler::matchplay::SpawnProtection>(target_entity)
            .is_some()
    );
    assert_eq!(
        harness
            .server
            .world()
            .resource::<WeaponTelemetry>()
            .hostile_delivery_contacts
            .values()
            .sum::<u64>(),
        0
    );
    harness.set_controlled_input(0, FighterInput::default());
    let before_move = server_player_position(&mut harness, target_player);
    harness.set_controlled_input(1, FighterInput::from_axes(Vec2::Y, Some(Vec2::X), 0));
    for _ in 0..5 {
        harness.step();
    }
    assert_ne!(
        server_player_position(&mut harness, target_player),
        before_move
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .get::<brawler::matchplay::SpawnProtection>(target_entity)
            .is_none()
    });
}

#[test]
#[allow(clippy::too_many_lines)]
fn defeat_schedules_one_exact_respawn_and_duplicate_event_is_harmless() {
    let mut harness = Harness::new_match(2);
    harness.step_until(|harness| {
        (0..2).all(|index| harness.client_is_active(index) && harness.selection_is_complete(index))
    });
    let waiting = server_match(&mut harness);
    for index in 0..2 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: 1,
                match_id: waiting.match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    harness.step_until(|harness| matches!(server_match(harness).phase, MatchPhase::Active { .. }));
    let players = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(
            Entity,
            &PlayerId,
            &NetworkEntityId,
            &TeamId,
            &CurrentHealth,
            &WeaponState,
            &brawler::builds::SelectedBuild,
        ), (With<Fighter>, With<MatchParticipant>)>();
        query
            .iter(world)
            .map(|(entity, player, network, team, health, weapon, build)| {
                (entity, *player, *network, *team, *health, *weapon, *build)
            })
            .collect::<Vec<_>>()
    };
    let source = players
        .iter()
        .find(|(_, _, _, team, _, _, _)| team.0 == 0)
        .unwrap();
    let target = players
        .iter()
        .find(|(_, _, _, team, _, _, _)| team.0 == 1)
        .unwrap();
    let defeat_event = CombatEventId(70_000);
    harness.server.world_mut().entity_mut(target.0).insert((
        CurrentHealth(0),
        brawler::builds::AbilityState {
            charge: 777,
            phase: brawler::builds::AbilityPhase::Charging,
        },
        brawler::builds::PassiveRuntimeState::default(),
        Defeated {
            event_id: defeat_event,
        },
    ));
    let tick = harness.server.world().resource::<SimulationTick>().0;
    let fact = CombatOutcomeFact {
        event_id: defeat_event,
        tick,
        attack_id: AttackId(70_000),
        source_kind: CombatSourceKind::PrimaryWeapon,
        source_player: Some(source.1),
        source_network_id: Some(source.2),
        source_team: Some(source.3),
        target_network_id: target.2,
        target_kind: brawler::combat::CombatTargetKind::Fighter,
        target_team: target.3,
        preset_id: None,
        recipe_fingerprint: None,
        position: WorldPoint { x: 0.0, y: 0.0 },
        engagement_distance: 200.0,
        kind: CombatOutcomeKind::Defeat,
    };
    harness.arm_packet_impairment(0);
    harness
        .server
        .world_mut()
        .resource_mut::<CombatOutcomeFacts>()
        .0
        .extend([
            CombatOutcomeFact {
                event_id: CombatEventId(69_999),
                attack_id: AttackId(69_999),
                kind: CombatOutcomeKind::Damage { amount: 10 },
                ..fact
            },
            fact,
        ]);
    harness.step_until(|harness| {
        let server_has_respawn = harness
            .server
            .world()
            .get::<brawler::matchplay::RespawnState>(target.0)
            .is_some();
        let client_has_respawn = {
            let world = harness.clients[0].world_mut();
            let mut query = world.query::<(&NetworkEntityId, &brawler::matchplay::RespawnState)>();
            query
                .iter(world)
                .any(|(network_id, _)| *network_id == target.2)
        };
        server_has_respawn && client_has_respawn && harness.packet_impairment(0).injected
    });
    let impairment = harness.packet_impairment(0);
    assert!(impairment.injected);
    assert!(impairment.dropped_packets > 0);
    assert!(impairment.delayed_packets > 0);
    assert!(impairment.duplicated_packets > 0);
    assert!(impairment.reordered_batches > 0);
    let triggered_passive = harness
        .server
        .world()
        .get::<brawler::builds::PassiveRuntimeState>(target.0)
        .unwrap();
    assert!(triggered_passive.adrenaline_until_tick.is_some());
    assert!(triggered_passive.adrenaline_rearm_at_tick.is_some());
    assert_eq!(server_wipeout(&mut harness).team_scores, [1, 0]);
    let deadline = *harness
        .server
        .world()
        .get::<brawler::matchplay::RespawnState>(target.0)
        .unwrap();
    let duplicate_tick = harness.server.world().resource::<SimulationTick>().0;
    harness
        .server
        .world_mut()
        .resource_mut::<CombatOutcomeFacts>()
        .0
        .push(CombatOutcomeFact {
            tick: duplicate_tick,
            ..fact
        });
    harness.step();
    assert_eq!(server_wipeout(&mut harness).team_scores, [1, 0]);
    assert_eq!(
        harness
            .server
            .world()
            .resource::<brawler::matchplay::MatchOutcomeDiagnostics>()
            .duplicate_event,
        1
    );
    assert_eq!(
        harness
            .server
            .world()
            .get::<brawler::matchplay::RespawnState>(target.0),
        Some(&deadline)
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .get::<brawler::matchplay::RespawnState>(target.0)
            .is_none()
    });
    assert_eq!(
        harness.server.world().get::<CurrentHealth>(target.0),
        Some(&target.4)
    );
    assert_eq!(
        harness.server.world().get::<WeaponState>(target.0),
        Some(&target.5)
    );
    assert!(harness.server.world().get::<Defeated>(target.0).is_none());
    assert_eq!(
        harness
            .server
            .world()
            .get::<brawler::builds::AbilityState>(target.0),
        Some(&brawler::builds::AbilityState {
            charge: 807,
            phase: brawler::builds::AbilityPhase::Charging,
        })
    );
    assert_eq!(
        harness
            .server
            .world()
            .get::<brawler::builds::PassiveRuntimeState>(target.0),
        Some(&brawler::builds::PassiveRuntimeState::default())
    );
    assert!(
        harness
            .server
            .world()
            .get::<brawler::matchplay::SpawnProtection>(target.0)
            .is_some()
    );
    let assignment = *harness
        .server
        .world()
        .get::<SpawnAssignment>(target.0)
        .unwrap();
    assert!(
        harness
            .server
            .world()
            .resource::<SpawnPointCatalog>()
            .0
            .get(&target.3.0)
            .is_some_and(|points| points
                .iter()
                .any(|point| point.spawn_point_id == assignment.spawn_point_id))
    );
    let target_network_id = target.2;
    harness.step_until(|harness| {
        (0..2).all(|index| {
            let world = harness.clients[index].world_mut();
            let mut query = world.query::<(&NetworkEntityId, &SpawnAssignment)>();
            query.iter(world).any(|(network_id, replicated)| {
                *network_id == target_network_id && *replicated == assignment
            })
        })
    });

    let diagnostics_tick = harness.server.world().resource::<SimulationTick>().0;
    let mut rejected = vec![
        CombatOutcomeFact {
            event_id: CombatEventId(fact.event_id.0 + 1),
            attack_id: AttackId(fact.attack_id.0 + 1),
            tick: diagnostics_tick.saturating_sub(1),
            ..fact
        },
        CombatOutcomeFact {
            event_id: CombatEventId(fact.event_id.0 + 2),
            attack_id: AttackId(fact.attack_id.0 + 2),
            tick: diagnostics_tick,
            target_network_id: NetworkEntityId(u64::MAX - 1),
            ..fact
        },
        CombatOutcomeFact {
            event_id: CombatEventId(fact.event_id.0 + 3),
            attack_id: AttackId(fact.attack_id.0 + 3),
            tick: diagnostics_tick,
            source_team: Some(target.3),
            ..fact
        },
    ];
    harness
        .server
        .world_mut()
        .resource_mut::<CombatOutcomeFacts>()
        .0
        .append(&mut rejected);
    harness.step();
    let diagnostics = *harness
        .server
        .world()
        .resource::<brawler::matchplay::MatchOutcomeDiagnostics>();
    assert_eq!(diagnostics.stale_tick, 1);
    assert_eq!(diagnostics.unknown_or_wrong_match_target, 1);
    assert_eq!(diagnostics.friendly_invalid_defeat, 1);
    assert_eq!(server_wipeout(&mut harness).team_scores, [1, 0]);

    {
        let world = harness.server.world_mut();
        let mut roots =
            world.query_filtered::<&mut brawler::matchplay::WipeoutState, With<MatchRootMarker>>();
        roots.single_mut(world).unwrap().target_score = 2;
    }
    let completion_tick = harness.server.world().resource::<SimulationTick>().0;
    harness
        .server
        .world_mut()
        .resource_mut::<CombatOutcomeFacts>()
        .0
        .push(CombatOutcomeFact {
            event_id: CombatEventId(fact.event_id.0 + 10),
            attack_id: AttackId(fact.attack_id.0 + 10),
            tick: completion_tick,
            ..fact
        });
    harness.step_until(|harness| {
        let server = server_match(harness);
        matches!(server.phase, MatchPhase::Completed { .. })
            && (0..2).all(|index| client_match(harness, index) == Some(server))
    });
}
