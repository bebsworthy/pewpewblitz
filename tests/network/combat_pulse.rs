//! Network integration scenarios extracted from the shared harness.

use super::*;

#[test]
#[allow(clippy::too_many_lines)]
fn authoritative_pulse_hits_dummy_and_sandbox_reset_restores_durable_state() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
            && harness.client_ids(0).len() == 2
            && harness.client_ids(1).len() == 2
            && harness.loadout_is_ready(0)
            && harness.loadout_is_ready(1)
    });
    let player_id = harness.controlled_player_id(0);
    {
        let world = harness.server.world_mut();
        let source_position = {
            let mut source = world
                .query_filtered::<(&PlayerId, &Position), (With<Fighter>, Without<TestDummy>)>();
            source
                .iter(world)
                .find(|(candidate, _)| **candidate == player_id)
                .map(|(_, position)| position.0)
                .expect("controlled fighter position")
        };
        let (dummy_entity, direction) = {
            let mut dummy = world.query_filtered::<(Entity, &Position), With<TestDummy>>();
            let (entity, position) = dummy.single(world).expect("test dummy position");
            (entity, (position.0 - source_position).normalize_or(Vec2::X))
        };
        let position = source_position + direction * 250.0;
        world.entity_mut(dummy_entity).insert((
            Position(position),
            SpawnState {
                position,
                facing: 0.0,
            },
        ));
    }
    let dummy_aim = harness.aim_at_dummy(0);
    let (dummy_entity, dummy_spawn, dummy_initial_rotation, dummy_initial_layers) = {
        let world = harness.server.world_mut();
        let mut query = world
            .query_filtered::<(Entity, &SpawnState, &Rotation, &CollisionLayers), With<TestDummy>>(
            );
        let (entity, spawn, rotation, layers) = query.single(world).expect("dummy spawn state");
        (entity, *spawn, *rotation, *layers)
    };

    // Cross the server's strictly-newer-input activation barrier before the first fire intent.
    harness.set_controlled_input(0, FighterInput::default());
    harness.step();
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(dummy_aim), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .accepted_shots
            >= 1
            && harness.server_projectile_count() > 0
    });
    let server_body = {
        let world = harness.server.world_mut();
        let mut query =
            world.query_filtered::<&brawler::combat::ProjectileBody, With<Projectile>>();
        *query.iter(world).next().expect("server projectile body")
    };
    assert_eq!(server_body, brawler::combat::ProjectileBody::circle(2.0));
    harness.step_until(|harness| {
        [0, 1].into_iter().all(|client| {
            let world = harness.clients[client].world_mut();
            let mut query =
                world.query_filtered::<&brawler::combat::ProjectileBody, With<Projectile>>();
            query
                .iter(world)
                .any(|body| *body == brawler::combat::ProjectileBody::circle(2.0))
        })
    });
    let first_tick_projectile_travelled = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&ComposedProjectileRuntime, With<Projectile>>();
        query
            .iter(world)
            .next()
            .expect("first-tick projectile")
            .travelled
    };
    assert!(first_tick_projectile_travelled > 0.0);
    harness.step_until(|harness| {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&Defeated, With<TestDummy>>();
        query.iter(world).next().is_some()
    });

    harness.step_until(|harness| {
        [0, 1].into_iter().all(|index| {
            let (health, _, defeated) =
                harness.client_fighter_combat_state(index, DUMMY_NETWORK_ENTITY);
            health.0 == 0 && defeated
        })
    });

    let defeated_health = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&CurrentHealth, With<TestDummy>>();
        query.single(world).expect("dummy health").0
    };
    assert_eq!(defeated_health, 0);
    let (accepted_shots, applied_damage, defeats) = {
        let telemetry = harness.server.world().resource::<CombatTelemetry>();
        (
            telemetry.accepted_shots,
            telemetry.applied_damage,
            telemetry.defeats,
        )
    };
    assert!(accepted_shots >= 1);
    assert_eq!(applied_damage, 100);
    assert_eq!(defeats, 1);

    let reset_at_tick = {
        let world = harness.server.world_mut();
        let mut defeated = world.query_filtered::<&TestDummyResetDeadline, With<TestDummy>>();
        defeated.single(world).expect("dummy defeat deadline").0
    };
    // Disturb every durable pose/state field after defeat so reset verification cannot pass by
    // observing the unchanged spawn state. The authored SpawnState and original collision layers
    // remain untouched and are the expected restoration values.
    {
        let world = harness.server.world_mut();
        world.entity_mut(dummy_entity).insert((
            Position::from_xy(
                dummy_spawn.position.x + 137.0,
                dummy_spawn.position.y + 71.0,
            ),
            Rotation::radians(dummy_spawn.facing + 0.75),
            CurrentHealth(1),
            WeaponState {
                ammo: 0,
                phase: WeaponPhase::Ready,
                ammo_recovery: Some(AmmoRecovery {
                    started_at_tick: reset_at_tick,
                    ready_at_tick: reset_at_tick.saturating_add(10),
                }),
            },
        ));
    }
    while harness.server_simulation_tick() < reset_at_tick {
        harness.step();
    }
    assert_eq!(
        harness.server_simulation_tick(),
        reset_at_tick,
        "reset deadline must be evaluated in the authoritative SimulationTick"
    );
    let still_defeated = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<Entity, With<TestDummy>>();
        query
            .iter(world)
            .all(|entity| world.get::<Defeated>(entity).is_some())
    };
    assert!(still_defeated);
    harness.step();
    let (reset_tick, reset_position) = {
        let world = harness.server.world_mut();
        let telemetry = world.resource::<CombatTelemetry>();
        telemetry
            .records
            .iter()
            .rev()
            .find_map(|record| match record {
                CombatLogRecord::Reset {
                    tick,
                    target,
                    position,
                    ..
                } if *target == DUMMY_NETWORK_ENTITY => Some((*tick, *position)),
                _ => None,
            })
            .expect("authoritative reset record")
    };
    assert_eq!(reset_tick, reset_at_tick);
    assert_eq!(reset_position, WorldPoint::from(dummy_spawn.position));
    let reset_state = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(
            &Position,
            &Rotation,
            &CurrentHealth,
            &WeaponState,
            &CollisionLayers,
            Option<&Defeated>,
        ), With<TestDummy>>();
        let (position, rotation, health, weapon, layers, defeated) =
            query.single(world).expect("reset dummy state");
        (
            *position,
            *rotation,
            *health,
            *weapon,
            *layers,
            defeated.is_some(),
        )
    };
    assert_eq!(
        reset_state.0,
        Position::from_xy(dummy_spawn.position.x, dummy_spawn.position.y)
    );
    assert!((reset_state.1.as_radians() - dummy_initial_rotation.as_radians()).abs() < 0.001);
    assert_eq!(reset_state.2, CurrentHealth(100));
    assert_eq!(reset_state.3.ammo, 4);
    assert!(matches!(reset_state.3.phase, WeaponPhase::Ready));
    assert_eq!(reset_state.4, dummy_initial_layers);
    assert!(!reset_state.5);
    harness.step_until(|harness| {
        [0, 1].into_iter().all(|index| {
            let (health, weapon, defeated) =
                harness.client_fighter_combat_state(index, DUMMY_NETWORK_ENTITY);
            health.0 == 100
                && weapon.ammo == 4
                && matches!(weapon.phase, WeaponPhase::Ready)
                && !defeated
        })
    });
    let reset_health = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&CurrentHealth, With<TestDummy>>();
        query.single(world).expect("reset dummy health").0
    };
    assert_eq!(reset_health, 100);
    assert!(
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .records
            .iter()
            .any(|record| matches!(record, CombatLogRecord::Reset { .. }))
    );
    let expected_cue_stream = harness
        .server
        .world()
        .resource::<CombatTelemetry>()
        .cues
        .clone();
    let expected_muzzle_count = expected_cue_stream
        .iter()
        .filter(|cue| matches!(cue, CombatCue::Muzzle { .. }))
        .count() as u64;
    let current_accepted_shots = harness
        .server
        .world()
        .resource::<CombatTelemetry>()
        .accepted_shots;
    assert_eq!(expected_muzzle_count, current_accepted_shots);
    assert_eq!(harness.client_cues(0), expected_cue_stream.as_slice());
    assert_eq!(harness.client_cues(1), expected_cue_stream.as_slice());

    harness.set_controlled_input(0, FighterInput::default());
    for _ in 0..20 {
        harness.step();
    }
    let defeats_before_repeats = harness.server.world().resource::<CombatTelemetry>().defeats;
    for repeat in 0..2 {
        harness.set_controlled_input(
            0,
            FighterInput::from_axes(Vec2::ZERO, Some(dummy_aim), FighterInput::PRIMARY_FIRE),
        );
        let mut saw_defeat = false;
        // Per-round recovery deliberately spaces sustained shots, and the idle dummy now
        // recovers health while it is not attacking. Keep this a bounded eventual-defeat check.
        for _ in 0..600 {
            harness.step();
            let world = harness.server.world_mut();
            let mut query = world.query_filtered::<Entity, With<Defeated>>();
            if query
                .iter(world)
                .any(|entity| world.get::<TestDummy>(entity).is_some())
            {
                saw_defeat = true;
                break;
            }
        }
        assert!(
            saw_defeat,
            "repeat {repeat} did not defeat the dummy; telemetry={:?}",
            harness.server.world().resource::<CombatTelemetry>()
        );
        harness.step_until(|harness| {
            let world = harness.server.world_mut();
            let mut query = world.query_filtered::<Entity, With<TestDummy>>();
            let dummy = query.single(world).expect("dummy");
            world.get::<Defeated>(dummy).is_none()
        });
        harness.set_controlled_input(0, FighterInput::default());
        harness.step_until(|harness| {
            [0, 1].into_iter().all(|index| {
                let (health, weapon, defeated) =
                    harness.client_fighter_combat_state(index, DUMMY_NETWORK_ENTITY);
                health.0 == 100
                    && weapon.ammo == 4
                    && matches!(weapon.phase, WeaponPhase::Ready)
                    && !defeated
            })
        });
    }
    assert!(
        harness.server.world().resource::<CombatTelemetry>().defeats >= defeats_before_repeats + 2
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn newly_spawned_projectile_can_hit_the_target_in_its_first_fixed_tick() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
    });

    let (source_entity, dummy_entity) = {
        let world = harness.server.world_mut();
        let mut source_query =
            world.query_filtered::<Entity, (With<Fighter>, Without<TestDummy>)>();
        let source_entity = source_query.iter(world).next().expect("source fighter");
        let mut dummy_query = world.query_filtered::<Entity, With<TestDummy>>();
        let dummy_entity = dummy_query.iter(world).next().expect("dummy fighter");
        (source_entity, dummy_entity)
    };
    {
        let world = harness.server.world_mut();
        // The muzzle starts at x=-66 and advances 8.33 units during the fixed sweep. Placing the
        // target at x=-42 leaves it outside the initial overlap but inside that first sweep.
        world
            .entity_mut(source_entity)
            .insert((Position::from_xy(-100.0, -300.0), Rotation::IDENTITY));
        world
            .entity_mut(dummy_entity)
            .insert(Position::from_xy(-42.0, -300.0));
    }
    let records_before = harness
        .server
        .world()
        .resource::<CombatTelemetry>()
        .records
        .len();
    let dummy_aim = harness.aim_at_dummy(0);
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(dummy_aim), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .records
            .iter()
            .skip(records_before)
            .any(|record| {
                matches!(
                    record,
                    CombatLogRecord::Damage {
                        target: DUMMY_NETWORK_ENTITY,
                        applied: 100,
                        ..
                    }
                )
            })
    });

    let records = &harness.server.world().resource::<CombatTelemetry>().records;
    let shot_tick = records
        .iter()
        .skip(records_before)
        .find_map(|record| match record {
            CombatLogRecord::Shot { tick, .. } => Some(*tick),
            _ => None,
        });
    let impact_tick = records
        .iter()
        .skip(records_before)
        .find_map(|record| match record {
            CombatLogRecord::Hit {
                tick,
                target: Some(target),
                ..
            } if *target == DUMMY_NETWORK_ENTITY => Some(*tick),
            _ => None,
        });
    let damage_tick = records
        .iter()
        .skip(records_before)
        .find_map(|record| match record {
            CombatLogRecord::Damage {
                tick,
                target: DUMMY_NETWORK_ENTITY,
                ..
            } => Some(*tick),
            _ => None,
        });
    let (shot_tick, impact_tick, damage_tick) = (
        shot_tick.expect("first-tick shot record"),
        impact_tick.expect("first-tick impact record"),
        damage_tick.expect("first-tick damage record"),
    );
    assert_eq!(shot_tick, impact_tick);
    assert_eq!(impact_tick, damage_tick);
    assert_eq!(
        harness.server_simulation_tick(),
        damage_tick.saturating_add(1),
        "the damage was emitted before the fixed tick advanced"
    );
    assert_eq!(harness.server_projectile_count(), 0);
    let dummy_health = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&CurrentHealth, With<TestDummy>>();
        query.single(world).expect("dummy health").0
    };
    assert_eq!(dummy_health, 0);
}

#[test]
fn fixed_schedule_ammo_recovery_restores_one_round_and_fires_on_the_ready_tick() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
    });
    let player_id = harness.controlled_player_id(0);
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(Vec2::X), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(&PlayerId, &WeaponState), With<Fighter>>();
        query.iter(world).any(|(candidate, state)| {
            *candidate == player_id && state.ammo == 0 && state.ammo_recovery.is_some()
        })
    });
    let recovery_at_tick = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(&PlayerId, &WeaponState), With<Fighter>>();
        query
            .iter(world)
            .find(|(candidate, _)| **candidate == player_id)
            .and_then(|(_, state)| state.ammo_recovery)
            .map(|recovery| recovery.ready_at_tick)
            .expect("ammunition recovery deadline")
    };
    let shot_count_before_reload = harness
        .server
        .world()
        .resource::<CombatTelemetry>()
        .records
        .iter()
        .filter(|record| matches!(record, CombatLogRecord::Shot { .. }))
        .count();
    let shots_before_reload = harness
        .server
        .world()
        .resource::<CombatTelemetry>()
        .accepted_shots;
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .accepted_shots
            > shots_before_reload
    });
    let (shots_after_reload, state_after_reload, reload_shot_tick) = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(&PlayerId, &WeaponState), With<Fighter>>();
        let state = query
            .iter(world)
            .find(|(candidate, _)| **candidate == player_id)
            .map(|(_, state)| *state)
            .expect("fighter weapon");
        let reload_shot_tick = world
            .resource::<CombatTelemetry>()
            .records
            .iter()
            .filter_map(|record| match record {
                CombatLogRecord::Shot { tick, .. } => Some(*tick),
                _ => None,
            })
            .nth(shot_count_before_reload)
            .expect("shot record after reload");
        (
            world.resource::<CombatTelemetry>().accepted_shots,
            state,
            reload_shot_tick,
        )
    };
    assert_eq!(shots_after_reload, shots_before_reload + 1);
    assert_eq!(reload_shot_tick, recovery_at_tick);
    assert_eq!(
        harness.server_simulation_tick(),
        reload_shot_tick.saturating_add(1)
    );
    assert!(matches!(
        state_after_reload.phase,
        WeaponPhase::Cooldown { .. }
    ));
    assert_eq!(state_after_reload.ammo, 0);
    assert!(state_after_reload.ammo_recovery.is_some());
}

#[test]
fn reciprocal_lethal_hits_defeat_both_fighters_with_stable_attribution() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
            && harness.client_ids(0).len() == 2
            && harness.client_ids(1).len() == 2
    });
    for index in 0..2 {
        harness.install_saved_brawler_recipe(index, 2, 1, 1, [3, 6]);
    }

    {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(
            &PlayerId,
            &mut Position,
            &mut Rotation,
            &mut CurrentHealth,
        ), With<Fighter>>();
        for (player, mut position, mut rotation, mut health) in query.iter_mut(world) {
            if player.0 == 0 {
                continue;
            }
            let right_side = player.0 % 2 == 0;
            position.0 = Vec2::new(if right_side { 140.0 } else { -140.0 }, 160.0);
            *rotation = Rotation::radians(if right_side {
                std::f32::consts::PI
            } else {
                0.0
            });
            health.0 = 25;
        }
    }

    for index in 0..2 {
        let right_side = harness.controlled_player_id(index).0.is_multiple_of(2);
        harness.set_controlled_input(
            index,
            FighterInput::from_axes(
                Vec2::ZERO,
                Some(if right_side { -Vec2::X } else { Vec2::X }),
                FighterInput::PRIMARY_FIRE,
            ),
        );
    }
    harness.step_until(|harness| {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&NetworkEntityId, (With<Fighter>, With<Defeated>)>();
        query.iter(world).count() == 2
    });

    let telemetry = harness.server.world().resource::<CombatTelemetry>();
    assert_eq!(telemetry.defeats, 2);
    let cue_event_ids: Vec<_> = telemetry
        .cues
        .iter()
        .map(|cue| match cue {
            CombatCue::AttackAccepted { event_id, .. }
            | CombatCue::DeliveryImpact { event_id, .. }
            | CombatCue::LobLanded { event_id, .. }
            | CombatCue::MeleeContact { event_id, .. }
            | CombatCue::DamageApplied { event_id, .. }
            | CombatCue::EffectApplied { event_id, .. }
            | CombatCue::FighterDefeated { event_id, .. }
            | CombatCue::FighterReset { event_id, .. }
            | CombatCue::SentryFired { event_id, .. }
            | CombatCue::Muzzle { event_id, .. }
            | CombatCue::Impact { event_id, .. }
            | CombatCue::Damage { event_id, .. }
            | CombatCue::Defeat { event_id, .. }
            | CombatCue::DeployableRemoved { event_id, .. }
            | CombatCue::Reset { event_id, .. }
            | CombatCue::SelfCloakActivated { event_id, .. }
            | CombatCue::SelfCloakEnded { event_id, .. }
            | CombatCue::RevealScanActivated { event_id, .. }
            | CombatCue::DemolitionStrikeActivated { event_id, .. }
            | CombatCue::ForcedRevealApplied { event_id, .. } => event_id.0,
        })
        .collect();
    assert!(
        cue_event_ids.windows(2).all(|window| window[0] < window[1]),
        "combat cue event IDs must be globally increasing: {cue_event_ids:?}"
    );
    let mut defeated_targets: Vec<_> = telemetry
        .records
        .iter()
        .filter_map(|record| match record {
            CombatLogRecord::Defeat {
                source: Some(brawler::combat::DamageSource::PlayerWeapon { shot_id, .. }),
                target,
                ..
            } => Some((*target, *shot_id)),
            _ => None,
        })
        .collect();
    defeated_targets.sort_by_key(|(target, shot_id)| (target.0, shot_id.0));
    assert_eq!(defeated_targets.len(), 2);
    assert_ne!(defeated_targets[0].0, defeated_targets[1].0);
    assert_ne!(defeated_targets[0].1, defeated_targets[1].1);
}
