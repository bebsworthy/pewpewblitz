//! Deterministic Heist authority, replication, outcome, and restart scenarios.

use super::*;

fn server_match(harness: &mut Harness) -> MatchState {
    let world = harness.server.world_mut();
    let mut query = world.query_filtered::<&MatchState, With<MatchRootMarker>>();
    *query.single(world).expect("one server match")
}

#[track_caller]
fn step_until_budget(
    harness: &mut Harness,
    budget: usize,
    mut condition: impl FnMut(&mut Harness) -> bool,
) {
    for _ in 0..budget {
        harness.step();
        if condition(harness) {
            return;
        }
    }
    panic!("Heist harness condition did not become true within {budget} steps");
}

fn select_ready_and_activate(harness: &mut Harness, request_base: u64) {
    select_ready_and_activate_with_loadouts(
        harness,
        request_base,
        [(1, 1, [3, 4]), (2, 1, [3, 4])],
    );
}

fn select_ready_and_activate_with_loadouts(
    harness: &mut Harness,
    request_base: u64,
    loadouts: [(u16, u16, [u16; 2]); 2],
) {
    harness.step_until(|harness| (0..2).all(|index| harness.client_is_active(index)));
    for index in 0..2 {
        let (weapon, ultimate, passives) = loadouts[index];
        harness.install_saved_brawler_loadout(index, weapon, ultimate, passives);
    }
    harness.step_until(|harness| (0..2).all(|index| harness.loadout_is_ready(index)));
    let waiting = server_match(harness);
    for index in 0..2 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: request_base + 1,
                match_id: waiting.match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    step_until_budget(harness, 600, |harness| {
        matches!(server_match(harness).phase, MatchPhase::Active { .. })
    });
}

fn safe_health(harness: &mut Harness, client: Option<usize>) -> [u16; 2] {
    let world = if let Some(index) = client {
        harness.clients[index].world_mut()
    } else {
        harness.server.world_mut()
    };
    let mut query = world.query::<(
        &brawler::matchplay::HeistSafe,
        &CurrentHealth,
        &brawler::map::DamageableMaximumHealth,
    )>();
    let mut health = [u16::MAX; 2];
    for (safe, current, maximum) in query.iter(world) {
        assert!(current.0 <= maximum.0);
        if safe.defending_team.0 <= 1 {
            health[usize::from(safe.defending_team.0)] = current.0;
        }
    }
    health
}

fn drain_heist_cues(
    harness: &mut Harness,
    client: usize,
) -> Vec<brawler::matchplay::HeistObjectiveCue> {
    std::mem::take(&mut harness.client_heist_cues[client])
}

fn source_and_target(
    harness: &mut Harness,
    attacking_team: TeamId,
    attack_id: u64,
) -> (AttackSource, DamageableTargetIdentity) {
    let match_id = server_match(harness).match_id;
    let world = harness.server.world_mut();
    let mut fighters = world.query_filtered::<
        (&PlayerId, &NetworkEntityId, &TeamId, &Position),
        (With<Fighter>, With<ActiveCombatant>),
    >();
    let (player, network_id, team, position) = fighters
        .iter(world)
        .find(|(_, _, team, _)| **team == attacking_team)
        .expect("attacking team fighter");
    let source = AttackSource {
        kind: CombatSourceKind::PrimaryWeapon,
        attack_id: AttackId(attack_id),
        player_id: *player,
        owner_network_entity_id: *network_id,
        team_id: *team,
        recipe_fingerprint: WeaponRecipeFingerprint::default(),
        presentation_profile_id: brawler::combat::WeaponPresentationProfileId(3),
        legacy_compatibility: false,
        source_preset_id: None,
        origin: WorldPoint::from(position.0),
        facing: 0.0,
    };
    let mut safes = world.query::<&DamageableTargetIdentity>();
    let target = *safes
        .iter(world)
        .find(|identity| {
            matches!(
                identity,
                DamageableTargetIdentity::HeistSafe {
                    match_id: target_match,
                    defending_team,
                    ..
                } if *target_match == match_id && *defending_team != attacking_team
            )
        })
        .expect("enemy safe target");
    (source, target)
}

fn queue_safe_damage(harness: &mut Harness, attacking_team: TeamId, attack_id: u64, damage: u16) {
    let (source, target) = source_and_target(harness, attacking_team, attack_id);
    harness
        .server
        .world_mut()
        .resource_mut::<brawler::matchplay::PendingModeObjectiveDamages>()
        .0
        .push(brawler::matchplay::PendingModeObjectiveDamage {
            target,
            source,
            requested_damage: damage,
            delivery_index: 0,
            bundle_index: 0,
            effect_index: 0,
        });
}

fn restart_after_completion(harness: &mut Harness, request_base: u64) {
    step_until_budget(harness, 120, |harness| {
        matches!(
            server_match(harness).phase,
            MatchPhase::Completed {
                restart_unlocked_at_tick,
                ..
            } if harness.server_simulation_tick() >= restart_unlocked_at_tick
        )
    });
    let completed = server_match(harness);
    for index in 0..2 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: request_base,
                match_id: completed.match_id,
                command: MatchCommand::ReadyForRestart,
            },
        );
    }
    step_until_budget(harness, 120, |harness| {
        matches!(server_match(harness).phase, MatchPhase::Waiting)
    });
}

#[test]
fn heist_safe_damage_threshold_cues_converge_and_restart_cleanly() {
    let mut harness = Harness::new_heist_match(2);
    select_ready_and_activate(&mut harness, 10);
    assert_eq!(safe_health(&mut harness, None), [2_000, 2_000]);

    let (rejected_source, enemy_target) = source_and_target(&mut harness, TeamId(0), 698);
    let friendly_target = {
        let world = harness.server.world_mut();
        let mut safes = world.query::<&DamageableTargetIdentity>();
        *safes
            .iter(world)
            .find(|identity| {
                matches!(
                    identity,
                    DamageableTargetIdentity::HeistSafe {
                        defending_team: TeamId(0),
                        ..
                    }
                )
            })
            .unwrap()
    };
    {
        let mut pending = harness
            .server
            .world_mut()
            .resource_mut::<brawler::matchplay::PendingModeObjectiveDamages>();
        for (target, requested_damage, effect_index) in
            [(enemy_target, 0, 0), (friendly_target, 100, 1)]
        {
            pending
                .0
                .push(brawler::matchplay::PendingModeObjectiveDamage {
                    target,
                    source: rejected_source,
                    requested_damage,
                    delivery_index: 0,
                    bundle_index: 0,
                    effect_index,
                });
        }
    }
    harness.step();
    assert_eq!(safe_health(&mut harness, None), [2_000, 2_000]);
    assert_eq!(
        harness
            .server
            .world()
            .resource::<brawler::matchplay::HeistTelemetry>()
            .invalid_rejections,
        2
    );

    let (environment_source, _) = source_and_target(&mut harness, TeamId(0), 699);
    let explosion_tick = harness.server_simulation_tick();
    {
        let world = harness.server.world_mut();
        let (barrel, safe_position) = {
            let mut barrels =
                world.query_filtered::<&DamageableTargetIdentity, With<DamageableWorldObject>>();
            let barrel = *barrels.iter(world).next().expect("Twin Vaults barrel");
            let mut safes = world.query::<(&brawler::matchplay::HeistSafe, &Position)>();
            let safe_position = safes
                .iter(world)
                .find(|(safe, _)| safe.defending_team == TeamId(1))
                .unwrap()
                .1
                .0;
            (barrel, safe_position)
        };
        world
            .resource_mut::<brawler::map::WorldObjectExplosionFacts>()
            .0
            .push(brawler::map::WorldObjectExplosionFact {
                event_id: CombatEventId(699),
                tick: explosion_tick,
                source: environment_source,
                target: barrel,
                position: safe_position,
                radius: 10.0,
                damage: 2_000,
            });
    }
    harness.step();
    assert_eq!(
        safe_health(&mut harness, None),
        [2_000, 2_000],
        "barrel explosion reactions must not damage mode objectives"
    );

    queue_safe_damage(&mut harness, TeamId(0), 700, 1_500);
    step_until_budget(&mut harness, 240, |harness| {
        (0..2).all(|index| safe_health(harness, Some(index)) == [2_000, 500])
    });
    for index in 0..2 {
        assert!(drain_heist_cues(&mut harness, index).iter().any(|cue| {
            cue.kind == brawler::matchplay::HeistObjectiveCueKind::Critical
                && cue.health_after == 500
        }));
    }
    queue_safe_damage(&mut harness, TeamId(0), 701, 500);
    step_until_budget(&mut harness, 30, |harness| {
        matches!(server_match(harness).phase, MatchPhase::Completed { .. })
    });
    assert_eq!(safe_health(&mut harness, None), [2_000, 0]);
    assert!(matches!(
        server_match(&mut harness).phase,
        MatchPhase::Completed {
            result: brawler::matchplay::MatchResult::TeamVictory { team: TeamId(0) },
            ..
        }
    ));
    let telemetry = *harness
        .server
        .world()
        .resource::<brawler::matchplay::HeistTelemetry>();
    assert_eq!(telemetry.accepted_hits, 2);
    assert_eq!(telemetry.applied_damage, 2_000);
    assert!(telemetry.destroyed_at_tick[1].is_some());

    step_until_budget(&mut harness, 240, |harness| {
        (0..2).all(|index| safe_health(harness, Some(index)) == [2_000, 0])
    });
    for index in 0..2 {
        assert!(drain_heist_cues(&mut harness, index).iter().any(|cue| {
            cue.kind == brawler::matchplay::HeistObjectiveCueKind::Destroyed
                && cue.health_after == 0
        }));
    }

    let previous = server_match(&mut harness).match_id;
    restart_after_completion(&mut harness, 20);
    let restarted = server_match(&mut harness);
    assert_ne!(restarted.match_id, previous);
    assert_eq!(safe_health(&mut harness, None), [2_000, 2_000]);
    let world = harness.server.world_mut();
    let map_generation = {
        let mut maps = world.query_filtered::<&MapDynamicState, With<MapRoot>>();
        maps.single(world).unwrap().generation_id()
    };
    let mut safes = world.query::<&brawler::matchplay::HeistSafe>();
    assert!(
        safes.iter(world).all(|safe| {
            safe.match_id == restarted.match_id && safe.generation == map_generation
        })
    );
}

#[test]
fn heist_same_tick_double_destruction_draws_and_timeout_uses_fraction() {
    let mut draw = Harness::new_heist_match(2);
    select_ready_and_activate(&mut draw, 30);
    queue_safe_damage(&mut draw, TeamId(0), 710, 2_000);
    queue_safe_damage(&mut draw, TeamId(1), 711, 2_000);
    step_until_budget(&mut draw, 30, |harness| {
        matches!(server_match(harness).phase, MatchPhase::Completed { .. })
    });
    assert_eq!(safe_health(&mut draw, None), [0, 0]);
    assert!(matches!(
        server_match(&mut draw).phase,
        MatchPhase::Completed {
            result: brawler::matchplay::MatchResult::Draw,
            ..
        }
    ));

    let mut timeout = Harness::new_heist_match(2);
    select_ready_and_activate(&mut timeout, 40);
    let tick = timeout.server_simulation_tick();
    {
        let world = timeout.server.world_mut();
        let mut safes = world.query::<(&brawler::matchplay::HeistSafe, &mut CurrentHealth)>();
        for (safe, mut health) in safes.iter_mut(world) {
            health.0 = if safe.defending_team == TeamId(0) {
                1_000
            } else {
                900
            };
        }
        let mut roots = world.query_filtered::<&mut MatchState, With<MatchRootMarker>>();
        let mut state = roots.single_mut(world).unwrap();
        state.phase = MatchPhase::Active {
            ends_at_tick: tick + 1,
        };
    }
    step_until_budget(&mut timeout, 10, |harness| {
        matches!(server_match(harness).phase, MatchPhase::Completed { .. })
    });
    assert!(matches!(
        server_match(&mut timeout).phase,
        MatchPhase::Completed {
            result: brawler::matchplay::MatchResult::TeamVictory { team: TeamId(0) },
            ..
        }
    ));
}

#[test]
fn heist_objective_buffer_deduplicates_and_caps_authoritative_requests() {
    let mut harness = Harness::new_heist_match(2);
    select_ready_and_activate(&mut harness, 60);
    let (source, target) = source_and_target(&mut harness, TeamId(0), 800);
    let request = brawler::matchplay::PendingModeObjectiveDamage {
        target,
        source,
        requested_damage: 100,
        delivery_index: 0,
        bundle_index: 0,
        effect_index: 0,
    };
    {
        let mut pending = harness
            .server
            .world_mut()
            .resource_mut::<brawler::matchplay::PendingModeObjectiveDamages>();
        pending.0.extend([request, request]);
    }
    harness.step();
    assert_eq!(safe_health(&mut harness, None), [2_000, 1_900]);
    let telemetry = *harness
        .server
        .world()
        .resource::<brawler::matchplay::HeistTelemetry>();
    assert_eq!(telemetry.accepted_hits, 1);
    assert_eq!(telemetry.invalid_rejections, 1);

    let friendly_target = {
        let world = harness.server.world_mut();
        let mut safes = world.query::<&DamageableTargetIdentity>();
        *safes
            .iter(world)
            .find(|identity| {
                matches!(
                    identity,
                    DamageableTargetIdentity::HeistSafe {
                        defending_team: TeamId(0),
                        ..
                    }
                )
            })
            .unwrap()
    };
    {
        let mut pending = harness
            .server
            .world_mut()
            .resource_mut::<brawler::matchplay::PendingModeObjectiveDamages>();
        for effect_index in 0..65 {
            pending
                .0
                .push(brawler::matchplay::PendingModeObjectiveDamage {
                    target: friendly_target,
                    source: AttackSource {
                        attack_id: AttackId(801 + u64::from(effect_index)),
                        ..source
                    },
                    requested_damage: 1,
                    delivery_index: 0,
                    bundle_index: 0,
                    effect_index,
                });
        }
    }
    harness.step();
    assert_eq!(safe_health(&mut harness, None), [2_000, 1_900]);
    let telemetry = *harness
        .server
        .world()
        .resource::<brawler::matchplay::HeistTelemetry>();
    assert_eq!(telemetry.capacity_rejections, 1);
    assert_eq!(telemetry.invalid_rejections, 65);
}

#[test]
fn heist_sentry_uses_enemy_safe_only_when_no_hostile_fighter_qualifies() {
    let mut harness = Harness::new_heist_match(2);
    select_ready_and_activate_with_loadouts(&mut harness, 90, [(3, 2, [5, 6]), (1, 1, [3, 4])]);
    let owner_player = harness.controlled_player_id(0);
    let (enemy_safe, enemy_safe_position) = {
        let world = harness.server.world_mut();
        let mut safes = world.query::<(
            &brawler::matchplay::HeistSafe,
            &DamageableTargetIdentity,
            &Position,
        )>();
        let (_, identity, position) = safes
            .iter(world)
            .find(|(safe, _, _)| safe.defending_team == TeamId(1))
            .unwrap();
        (*identity, position.0)
    };
    {
        let world = harness.server.world_mut();
        let mut fighters = world.query_filtered::<(
            &PlayerId,
            &mut Position,
            &mut Rotation,
            &mut brawler::builds::AbilityState,
        ), With<Fighter>>();
        for (player, mut position, mut rotation, mut ability) in fighters.iter_mut(world) {
            if *player == owner_player {
                position.0 = enemy_safe_position - Vec2::X * 180.0;
                *rotation = Rotation::radians(0.0);
                *ability = brawler::builds::AbilityState {
                    charge: 1_000,
                    phase: brawler::builds::AbilityPhase::Ready,
                };
            } else {
                position.0 = enemy_safe_position - Vec2::X * 700.0;
            }
        }
    }
    let fighter_entities = {
        let world = harness.server.world_mut();
        let mut fighters = world.query_filtered::<Entity, With<Fighter>>();
        fighters.iter(world).collect::<Vec<_>>()
    };
    for entity in fighter_entities {
        harness
            .server
            .world_mut()
            .entity_mut(entity)
            .remove::<brawler::matchplay::SpawnProtection>();
    }
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, None, FighterInput::ULTIMATE),
    );
    step_until_budget(&mut harness, 60, |harness| {
        let world = harness.server.world_mut();
        let mut sentries = world.query_filtered::<Entity, With<brawler::abilities::Sentry>>();
        sentries.iter(world).next().is_some()
    });
    harness.set_controlled_input(0, FighterInput::default());
    step_until_budget(&mut harness, 180, |harness| {
        let world = harness.server.world_mut();
        let mut safes = world.query::<(&DamageableTargetIdentity, &CurrentHealth)>();
        safes
            .iter(world)
            .find(|(identity, _)| **identity == enemy_safe)
            .is_some_and(|(_, health)| health.0 < 2_000)
    });
    assert_eq!(safe_health(&mut harness, None), [2_000, 1_990]);
    assert!(
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .cues
            .iter()
            .any(|cue| matches!(cue, CombatCue::SentryFired { target: None, .. }))
    );
}
