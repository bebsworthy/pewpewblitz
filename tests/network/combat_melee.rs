//! Authoritative melee emission and same-tick resolution scenarios.

use super::*;

#[test]
fn impact_blade_emits_without_a_projectile_and_damages_the_target() {
    let mut harness = Harness::new(1);
    harness.clients[0]
        .world_mut()
        .resource_mut::<ClientNetworkConfig>()
        .weapon_preset = Some(4);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.loadout_is_ready(0)
    });
    harness.install_saved_brawler_loadout(0, 4, 2, [5, 6]);
    harness.set_controlled_input(0, FighterInput::default());
    harness.step();

    let player_id = harness.controlled_player_id(0);
    let initial_health = {
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
        let mut dummy =
            world.query_filtered::<(Entity, &Position, &CurrentHealth), With<TestDummy>>();
        let (entity, position, health) = dummy.single(world).expect("test dummy");
        let direction = (position.0 - source_position).normalize_or(Vec2::X);
        let initial_health = health.0;
        let position = source_position + direction * 80.0;
        world.entity_mut(entity).insert((
            Position(position),
            SpawnState {
                position,
                facing: 0.0,
            },
        ));
        initial_health
    };

    let aim = harness.aim_at_dummy(0);
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(aim), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| {
        let world = harness.server.world_mut();
        let mut dummy = world.query_filtered::<&CurrentHealth, With<TestDummy>>();
        dummy
            .single(world)
            .is_ok_and(|health| health.0 < initial_health)
    });

    assert_eq!(harness.server_projectile_count(), 0);
    assert!(
        harness
            .server
            .world()
            .resource::<WeaponTelemetry>()
            .hostile_damage_events
            .get(&WeaponPresetId(4))
            .copied()
            .unwrap_or(0)
            > 0
    );
}
