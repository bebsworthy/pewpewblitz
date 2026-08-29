//! Authoritative persistent-splash replication and pulse scenarios.

use super::*;

#[test]
fn splash_lands_as_a_replicated_stationary_area_with_repeated_server_damage() {
    let mut harness = Harness::new(1);
    harness.clients[0]
        .world_mut()
        .resource_mut::<ClientNetworkConfig>()
        .weapon_preset = Some(7);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.loadout_is_ready(0)
    });
    harness.install_saved_brawler_loadout(0, 7, 2, [5, 6]);
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
        FighterInput::from_axes_with_aim_distance(
            Vec2::ZERO,
            Some(aim),
            Some(180.0),
            FighterInput::PRIMARY_FIRE,
        ),
    );
    harness.step_until(|harness| harness.server_projectile_count() > 0);
    harness.set_controlled_input(0, FighterInput::default());
    harness.step_until(|harness| {
        let world = harness.server.world_mut();
        let mut areas = world.query_filtered::<&PersistentSplashState, With<PersistentSplash>>();
        areas.iter(world).next().is_some()
    });

    let initial_state = {
        let world = harness.server.world_mut();
        let mut areas = world.query_filtered::<&PersistentSplashState, With<PersistentSplash>>();
        *areas.iter(world).next().expect("server splash state")
    };
    assert_eq!(harness.server_projectile_count(), 0);
    harness.step_until(|harness| {
        let world = harness.clients[0].world_mut();
        let mut areas = world.query_filtered::<&PersistentSplashState, With<PersistentSplash>>();
        areas.iter(world).any(|state| *state == initial_state)
    });

    let health_after_landing = dummy_health(&mut harness);
    harness.step_until(|harness| dummy_health(harness) < health_after_landing);
    let health_after_next_pulse = dummy_health(&mut harness);
    harness.step_until(|harness| dummy_health(harness) < health_after_next_pulse);

    let current_state = {
        let world = harness.server.world_mut();
        let mut areas = world.query_filtered::<&PersistentSplashState, With<PersistentSplash>>();
        *areas.iter(world).next().expect("splash remains active")
    };
    assert_eq!(current_state.center, initial_state.center);
    assert!((current_state.facing - initial_state.facing).abs() < f32::EPSILON);
    assert!(current_state.next_pulse_tick > initial_state.next_pulse_tick);
}

fn dummy_health(harness: &mut Harness) -> u16 {
    let world = harness.server.world_mut();
    let mut dummy = world.query_filtered::<&CurrentHealth, With<TestDummy>>();
    dummy.single(world).expect("dummy health").0
}
