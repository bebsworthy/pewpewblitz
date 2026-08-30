//! Routed authoritative Sticky Blomb attachment, replication, and fuse behavior.

use super::*;

#[test]
fn sticky_blomb_attaches_replicates_and_detonates_from_the_authoritative_fuse() {
    let mut harness = Harness::new(1);
    harness.clients[0]
        .world_mut()
        .resource_mut::<ClientNetworkConfig>()
        .weapon_preset = Some(5);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.loadout_is_ready(0)
    });
    harness.install_saved_brawler_loadout(0, 5, 2, [5, 6]);
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

    let initial_health = {
        let world = harness.server.world_mut();
        let mut dummy = world.query_filtered::<&CurrentHealth, With<TestDummy>>();
        dummy.single(world).expect("dummy health").0
    };
    let aim = harness.aim_at_dummy(0);
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(aim), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| harness.server_projectile_count() > 0);
    harness.set_controlled_input(0, FighterInput::default());
    harness.step_until(|harness| {
        let world = harness.server.world_mut();
        let mut blobs = world.query::<&StickyBlobState>();
        blobs
            .iter(world)
            .any(|state| state.attached_to == Some(DUMMY_NETWORK_ENTITY))
    });

    let authoritative_state = {
        let world = harness.server.world_mut();
        let mut blobs = world.query::<&StickyBlobState>();
        *blobs
            .iter(world)
            .find(|state| state.attached_to == Some(DUMMY_NETWORK_ENTITY))
            .expect("authoritative attached sticky")
    };
    assert!(authoritative_state.detonates_at_tick > authoritative_state.armed_at_tick);
    harness.step_until(|harness| {
        let world = harness.clients[0].world_mut();
        let mut blobs = world.query::<&StickyBlobState>();
        blobs.iter(world).any(|state| *state == authoritative_state)
    });

    harness.step_until(|harness| {
        let world = harness.server.world_mut();
        let mut dummy = world.query_filtered::<&CurrentHealth, With<TestDummy>>();
        dummy
            .single(world)
            .is_ok_and(|health| health.0 < initial_health)
    });
    assert!(harness.server_simulation_tick() >= authoritative_state.detonates_at_tick);
}
