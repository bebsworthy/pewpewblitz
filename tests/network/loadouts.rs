//! Network integration scenarios for server-resolved direct-diagnostic loadouts.

use super::*;

#[test]
fn direct_admission_resolves_distinct_weapon_bases_and_spawns_expected_deliveries() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
    });
    harness.step_until_within(240, "both clients' input timeline sync", |harness| {
        harness.client_input_synced(0) && harness.client_input_synced(1)
    });

    let world = harness.server.world_mut();
    let mut query = world.query_filtered::<(
        &PlayerId,
        &brawler::builds::ResolvedMatchLoadout,
        &WeaponState,
    ), With<Fighter>>();
    let mut loadouts: Vec<_> = query
        .iter(world)
        .filter(|(player, _, _)| player.0 != 0)
        .map(|(player, loadout, state)| (player.0, loadout, state))
        .collect();
    loadouts.sort_by_key(|(_, loadout, _)| loadout.primary_weapon.source_preset_id);
    assert_eq!(loadouts.len(), 2);
    assert_eq!(
        loadouts[0].1.primary_weapon.source_preset_id,
        Some(WeaponPresetId(1))
    );
    assert_eq!(
        loadouts[1].1.primary_weapon.source_preset_id,
        Some(WeaponPresetId(2))
    );
    assert_eq!(loadouts[0].2.ammo, 4);
    assert_eq!(loadouts[1].2.ammo, 3);

    for index in 0..2 {
        harness.set_controlled_input(
            index,
            FighterInput::from_axes(Vec2::ZERO, Some(Vec2::X), FighterInput::PRIMARY_FIRE),
        );
    }
    // Both opening volleys land once the fire input reaches the server; how many steps that
    // takes follows the input delay and link RTT, so wait for the deliveries themselves.
    harness.step_until_within(30, "both opening volleys", |harness| {
        server_projectile_deliveries(harness) >= 6
    });
    assert_eq!(server_projectile_deliveries(&mut harness), 6);
}

fn server_projectile_deliveries(harness: &mut Harness) -> usize {
    let world = harness.server.world_mut();
    let mut deliveries = world.query_filtered::<&AttackDelivery, With<Projectile>>();
    deliveries.iter(world).count()
}
