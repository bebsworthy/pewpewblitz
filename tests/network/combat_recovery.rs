//! Network integration scenarios extracted from the shared harness.

use super::*;

#[test]
fn health_recovers_after_attack_idle_delay_and_damage_does_not_restart_it() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.selection_is_complete(0)
    });
    harness.set_controlled_input(0, FighterInput::default());
    let player_id = harness.controlled_player_id(0);
    let start_tick = harness.server_simulation_tick();
    {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<
            (&PlayerId, &mut CurrentHealth, &mut HealthRecoveryState),
            With<Fighter>,
        >();
        let (_, mut health, mut recovery) = query
            .iter_mut(world)
            .find(|(candidate, _, _)| **candidate == player_id)
            .expect("controlled fighter");
        health.0 = 50;
        *recovery = HealthRecoveryState::starting_at(start_tick);
    }

    for _ in 0..90 {
        harness.step();
    }
    // Receiving damage is deliberately irrelevant to the attack-idle clock.
    {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(&PlayerId, &mut CurrentHealth), With<Fighter>>();
        let (_, mut health) = query
            .iter_mut(world)
            .find(|(candidate, _)| **candidate == player_id)
            .expect("controlled fighter");
        health.0 = 40;
    }
    for _ in 0..90 {
        harness.step();
    }
    assert_eq!(controlled_server_health(&mut harness, player_id), 40);
    for _ in 0..60 {
        harness.step();
    }
    assert_eq!(controlled_server_health(&mut harness, player_id), 50);

    let previous_attack_tick = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(&PlayerId, &HealthRecoveryState), With<Fighter>>();
        query
            .iter(world)
            .find(|(candidate, _)| **candidate == player_id)
            .map(|(_, recovery)| recovery.last_accepted_attack_tick)
            .expect("controlled fighter recovery")
    };
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(Vec2::X), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(&PlayerId, &HealthRecoveryState), With<Fighter>>();
        query.iter(world).any(|(candidate, recovery)| {
            *candidate == player_id && recovery.last_accepted_attack_tick > previous_attack_tick
        })
    });
    harness.set_controlled_input(0, FighterInput::default());
    let accepted_attack_tick = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(&PlayerId, &HealthRecoveryState), With<Fighter>>();
        query
            .iter(world)
            .find(|(candidate, _)| **candidate == player_id)
            .map(|(_, recovery)| recovery.last_accepted_attack_tick)
            .expect("accepted attack reset")
    };
    assert!(accepted_attack_tick > previous_attack_tick);
    {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(&PlayerId, &mut CurrentHealth), With<Fighter>>();
        let (_, mut health) = query
            .iter_mut(world)
            .find(|(candidate, _)| **candidate == player_id)
            .expect("controlled fighter");
        health.0 = 40;
    }
    for _ in 0..180 {
        harness.step();
    }
    assert_eq!(controlled_server_health(&mut harness, player_id), 40);
}

fn controlled_server_health(harness: &mut Harness, player_id: PlayerId) -> u16 {
    let world = harness.server.world_mut();
    let mut query = world.query_filtered::<(&PlayerId, &CurrentHealth), With<Fighter>>();
    query
        .iter(world)
        .find(|(candidate, _)| **candidate == player_id)
        .map(|(_, health)| health.0)
        .expect("controlled fighter health")
}

#[test]
fn firing_again_does_not_restart_in_progress_ammo_recovery() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.selection_is_complete(0)
    });
    let player_id = harness.controlled_player_id(0);
    let fire = FighterInput::from_axes(Vec2::ZERO, Some(Vec2::X), FighterInput::PRIMARY_FIRE);
    harness.set_controlled_input(0, fire);
    harness.step_until(|harness| server_ammo_state(harness, player_id).ammo == 5);
    harness.set_controlled_input(0, FighterInput::default());
    let first_recovery = server_ammo_state(&mut harness, player_id)
        .ammo_recovery
        .expect("first shot starts recovery");
    for _ in 0..20 {
        harness.step();
    }
    harness.set_controlled_input(0, fire);
    harness.step_until(|harness| server_ammo_state(harness, player_id).ammo == 4);
    harness.set_controlled_input(0, FighterInput::default());
    let after_second_shot = server_ammo_state(&mut harness, player_id);
    assert_eq!(after_second_shot.ammo_recovery, Some(first_recovery));
}

fn server_ammo_state(harness: &mut Harness, player_id: PlayerId) -> WeaponState {
    let world = harness.server.world_mut();
    let mut query = world.query_filtered::<(&PlayerId, &WeaponState), With<Fighter>>();
    query
        .iter(world)
        .find(|(candidate, _)| **candidate == player_id)
        .map(|(_, weapon)| *weapon)
        .expect("controlled fighter ammunition")
}

#[test]
fn late_join_recovers_active_projectile_and_defeated_durable_state() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
            && harness.selection_is_complete(0)
    });
    harness
        .server
        .world_mut()
        .resource_mut::<FighterDefinitions>()
        .entries[0]
        .defeat_reset_delay_ticks = 600;

    harness.set_controlled_input(0, FighterInput::default());
    harness.step();
    let dummy_aim = harness.aim_at_dummy(0);
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(dummy_aim), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| harness.server_projectile_count() > 0);

    harness.add_client(2);
    harness.step_until(|harness| {
        harness.client_is_active(1)
            && harness.server_ids().len() == 2
            && harness.client_ids(1).len() == 2
            && harness.client_projectile_count(1) > 0
    });
    harness.step_until(|harness| {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<Entity, With<TestDummy>>();
        let dummy = query.single(world).expect("dummy");
        world.get::<Defeated>(dummy).is_some()
    });
    harness.step_until(|harness| {
        let (health, _, defeated) = harness.client_fighter_combat_state(1, DUMMY_NETWORK_ENTITY);
        health.0 == 0 && defeated
    });
}

#[test]
fn late_join_recovers_in_progress_ammo_recovery_state() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
    });
    {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&mut Position, With<TestDummy>>();
        query.single_mut(world).expect("dummy").0.y = 300.0;
    }
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(Vec2::X), FighterInput::PRIMARY_FIRE),
    );
    let player_network_id = harness.server_ids()[0].1;
    harness.step_until(|harness| {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(&NetworkEntityId, &WeaponState), With<Fighter>>();
        query.iter(world).any(|(network_id, weapon)| {
            *network_id == player_network_id && weapon.ammo == 0 && weapon.ammo_recovery.is_some()
        })
    });

    harness.add_client(2);
    harness.step_until(|harness| {
        harness.client_is_active(1)
            && harness.client_ids(1).len() == 2
            && harness
                .client_fighter_combat_state(1, player_network_id)
                .1
                .ammo_recovery
                .is_some()
    });
}

#[test]
fn duplicate_and_reordered_fire_inputs_do_not_bypass_server_cadence() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
    });
    let target = harness.controlled_entity(0);
    let first_tick = harness.server_tick().saturating_add(1);
    let fire = FighterInput::from_axes(Vec2::ZERO, Some(Vec2::X), FighterInput::PRIMARY_FIRE);
    for _ in 0..4 {
        harness.send_forged_input(
            0,
            lightyear::input::input_message::InputTarget::Entity(target),
            first_tick,
            fire,
        );
    }
    harness.step();
    let first_shot_count = harness
        .server
        .world()
        .resource::<CombatTelemetry>()
        .accepted_shots;
    assert_eq!(first_shot_count, 1);

    for stale_tick in [first_tick, first_tick.saturating_sub(1)] {
        for _ in 0..3 {
            harness.send_forged_input(
                0,
                lightyear::input::input_message::InputTarget::Entity(target),
                stale_tick,
                fire,
            );
        }
    }
    for _ in 0..4 {
        harness.step();
    }
    assert_eq!(
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .accepted_shots,
        first_shot_count
    );
    let diagnostics = harness
        .server
        .world_mut()
        .query::<&InputValidationState>()
        .iter(harness.server.world())
        .copied()
        .collect::<Vec<_>>();
    assert!(
        diagnostics
            .iter()
            .any(|state| state.stale_or_reordered_rejections > 0),
        "duplicate/stale inputs should be diagnosed: {diagnostics:?}"
    );

    let drop_tick = harness.server_tick().saturating_add(1);
    harness.send_forged_input(
        0,
        lightyear::input::input_message::InputTarget::Entity(target),
        drop_tick,
        FighterInput::default(),
    );
    for _ in 0..14 {
        harness.step();
    }
    let after_drop_count = harness
        .server
        .world()
        .resource::<CombatTelemetry>()
        .accepted_shots;
    assert_eq!(after_drop_count, first_shot_count);

    let release_tick = harness.server_tick().saturating_add(1);
    harness.send_forged_input(
        0,
        lightyear::input::input_message::InputTarget::Entity(target),
        release_tick,
        fire,
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .accepted_shots
            > after_drop_count
    });
}

#[test]
fn delayed_dropped_duplicated_and_reordered_packets_converge_to_one_full_cue_stream() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
            && harness.client_ids(0).len() == 2
            && harness.client_ids(1).len() == 2
    });
    let dummy_aim = if harness.controlled_player_id(0).0 == 1 {
        Vec2::X
    } else {
        -Vec2::X
    };
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
    });
    harness.arm_packet_impairment(0);
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .accepted_shots
            >= 4
    });
    harness.set_controlled_input(0, FighterInput::default());
    harness.step_until(|harness| {
        let expected_len = harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .cues
            .len();
        harness.packet_impairment(0).injected
            && harness.client_cues(0).len() == expected_len
            && harness.client_cues(1).len() == expected_len
    });

    let expected = harness
        .server
        .world()
        .resource::<CombatTelemetry>()
        .cues
        .clone();
    let impairment = harness.packet_impairment(0);
    assert!(impairment.duplicated_packets > 0);
    assert!(impairment.dropped_packets > 0);
    assert!(impairment.delayed_packets > 0);
    assert!(impairment.reordered_batches > 0);
    assert_eq!(harness.client_cues(0), expected.as_slice());
    assert_eq!(harness.client_cues(1), expected.as_slice());
}
