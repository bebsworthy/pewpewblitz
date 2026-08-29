//! Authoritative cone-spray replication and damage scenarios.

use super::*;

#[test]
fn spray_is_a_stationary_replicated_volume_with_repeated_server_damage() {
    let mut harness = Harness::new(1);
    harness.clients[0]
        .world_mut()
        .resource_mut::<ClientNetworkConfig>()
        .weapon_preset = Some(6);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.loadout_is_ready(0)
    });
    harness.install_saved_brawler_loadout(0, 6, 2, [5, 6]);
    harness.set_controlled_input(0, FighterInput::default());
    harness.step();

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
        let position = source_position + direction * 180.0;
        world.entity_mut(dummy_entity).insert((
            Position(position),
            SpawnState {
                position,
                facing: 0.0,
            },
        ));
    }

    let aim = harness.aim_at_dummy(0);
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(aim), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| {
        let world = harness.server.world_mut();
        let mut sprays = world.query_filtered::<&ConeSprayState, With<ConeSpray>>();
        sprays.iter(world).next().is_some()
    });

    let initial_state = {
        let world = harness.server.world_mut();
        let mut sprays = world.query_filtered::<&ConeSprayState, With<ConeSpray>>();
        *sprays.iter(world).next().expect("server spray state")
    };
    assert_eq!(harness.server_projectile_count(), 0);
    harness.step_until(|harness| {
        let world = harness.clients[0].world_mut();
        let mut sprays = world.query_filtered::<&ConeSprayState, With<ConeSpray>>();
        sprays.iter(world).any(|state| *state == initial_state)
    });

    let initial_health = {
        let world = harness.server.world_mut();
        let mut dummy = world.query_filtered::<&CurrentHealth, With<TestDummy>>();
        dummy.single(world).expect("dummy health").0
    };
    harness.set_controlled_input(0, FighterInput::from_axes(Vec2::X, Some(-aim), 0));
    harness.step_until(|harness| {
        let world = harness.server.world_mut();
        let mut dummy = world.query_filtered::<&CurrentHealth, With<TestDummy>>();
        dummy
            .single(world)
            .is_ok_and(|health| health.0 < initial_health)
    });
    let health_after_first_pulse = {
        let world = harness.server.world_mut();
        let mut dummy = world.query_filtered::<&CurrentHealth, With<TestDummy>>();
        dummy
            .single(world)
            .expect("dummy health after first pulse")
            .0
    };
    harness.step_until(|harness| {
        let world = harness.server.world_mut();
        let mut dummy = world.query_filtered::<&CurrentHealth, With<TestDummy>>();
        dummy
            .single(world)
            .is_ok_and(|health| health.0 < health_after_first_pulse)
    });

    let current_state = {
        let world = harness.server.world_mut();
        let mut sprays = world.query_filtered::<&ConeSprayState, With<ConeSpray>>();
        *sprays.iter(world).next().expect("spray still active")
    };
    assert_eq!(current_state.origin, initial_state.origin);
    assert!((current_state.facing - initial_state.facing).abs() < f32::EPSILON);
    assert_eq!(harness.server_projectile_count(), 0);
}
