//! Network integration scenarios extracted from the shared harness.

use super::*;

fn install_launcher_loadout(harness: &mut Harness) {
    harness.install_saved_brawler_loadout(0, 3, 2, [5, 6]);
}

#[test]
fn launcher_uses_bounded_focal_distance_instead_of_always_using_maximum_range() {
    let mut harness = Harness::new(1);
    harness.clients[0]
        .world_mut()
        .resource_mut::<ClientNetworkConfig>()
        .weapon_preset = Some(3);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.loadout_is_ready(0)
    });
    install_launcher_loadout(&mut harness);
    harness.set_controlled_input(0, FighterInput::default());
    harness.step();
    harness.set_controlled_input(
        0,
        FighterInput::from_axes_with_aim_distance(
            Vec2::ZERO,
            Some(Vec2::Y),
            Some(180.0),
            FighterInput::PRIMARY_FIRE,
        ),
    );
    harness.step_until(|harness| harness.server_projectile_count() > 0);

    let (travelled, flight_ticks) = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<
            (&brawler::combat::LobbedFlight, &ReplicatedAttackSource),
            With<Projectile>,
        >();
        let (flight, source) = query.iter(world).next().expect("server lobbed delivery");
        (
            flight
                .landing
                .as_vec2()
                .distance(source.attack.origin.as_vec2()),
            flight.lands_at_tick - flight.launched_at_tick,
        )
    };
    assert!(
        (travelled - 180.0).abs() <= 1.0,
        "requested 180-unit focal distance should be preserved, got {travelled}"
    );
    assert_eq!(flight_ticks, 16);
}

#[test]
fn launcher_explosion_does_not_damage_or_knock_back_its_owner() {
    let mut harness = Harness::new(1);
    harness.clients[0]
        .world_mut()
        .resource_mut::<ClientNetworkConfig>()
        .weapon_preset = Some(3);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.loadout_is_ready(0)
    });
    install_launcher_loadout(&mut harness);
    harness.set_controlled_input(0, FighterInput::default());
    harness.step();
    let player = harness.controlled_player_id(0);
    harness.set_controlled_input(
        0,
        FighterInput::from_axes_with_aim_distance(
            Vec2::ZERO,
            Some(Vec2::Y),
            Some(40.0),
            FighterInput::PRIMARY_FIRE,
        ),
    );
    harness.step_until(|harness| harness.server_projectile_count() > 0);
    harness.set_controlled_input(0, FighterInput::default());
    harness.step_until(|harness| harness.server_projectile_count() == 0);

    let (health, maximum_health, has_knockback) = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(
            &PlayerId,
            &CurrentHealth,
            &brawler::builds::ResolvedMatchLoadout,
            Option<&brawler::combat::ExternalMotion>,
        ), (With<Fighter>, Without<TestDummy>)>();
        let (_, health, loadout, motion) = query
            .iter(world)
            .find(|(candidate, _, _, _)| **candidate == player)
            .expect("server-owned launcher fighter");
        (
            health.0,
            loadout.fighter_stats.maximum_health,
            motion.is_some(),
        )
    };
    assert_eq!(health, maximum_health);
    assert!(!has_knockback);
    assert_eq!(
        harness
            .server
            .world()
            .resource::<WeaponTelemetry>()
            .self_damage_events
            .get(&WeaponPresetId(3))
            .copied()
            .unwrap_or(0),
        0
    );
}

#[test]
fn launcher_replication_preserves_flight_deadline_and_durable_slow_state() {
    let mut harness = Harness::new(1);
    harness.clients[0]
        .world_mut()
        .resource_mut::<ClientNetworkConfig>()
        .weapon_preset = Some(3);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.loadout_is_ready(0)
    });
    install_launcher_loadout(&mut harness);
    harness.set_controlled_input(0, FighterInput::default());
    harness.step();
    {
        let player = harness.controlled_player_id(0);
        let world = harness.server.world_mut();
        let mut query =
            world.query_filtered::<(Entity, &PlayerId), (With<Fighter>, Without<TestDummy>)>();
        let entity = query
            .iter(world)
            .find(|(_, candidate)| **candidate == player)
            .map(|(entity, _)| entity)
            .expect("launcher source fighter");
        world
            .entity_mut(entity)
            .insert(Position::from_xy(-500.0, -320.0));
    }
    harness.step();
    let aim = harness.aim_at_dummy(0);
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(aim), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| harness.server_projectile_count() > 0);
    let (server_deadline, server_flight) = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(&ProjectileDeadline, &brawler::combat::LobbedFlight), With<Projectile>>();
        let (deadline, flight) = query.iter(world).next().expect("server lobbed delivery");
        (*deadline, *flight)
    };
    assert_eq!(server_deadline.expires_at_tick, server_flight.lands_at_tick);
    harness.step_until(|harness| {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&ActiveEffects, With<TestDummy>>();
        query.iter(world).any(|effects| effects.slow.is_some())
    });
    let telemetry = harness.server.world().resource::<WeaponTelemetry>();
    assert!(
        telemetry
            .hostile_damage_events
            .get(&WeaponPresetId(3))
            .copied()
            .unwrap_or(0)
            > 0
    );
    harness.step_until(|harness| {
        let world = harness.clients[0].world_mut();
        let mut query = world.query_filtered::<(&NetworkEntityId, &ActiveEffects), With<Fighter>>();
        query.iter(world).any(|(network_id, effects)| {
            *network_id == DUMMY_NETWORK_ENTITY && effects.slow.is_some()
        })
    });
}
