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
    for _ in 0..60 {
        harness.step();
    }

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
    for _ in 0..3 {
        harness.step();
    }
    let world = harness.server.world_mut();
    let mut deliveries = world.query_filtered::<&AttackDelivery, With<Projectile>>();
    assert_eq!(deliveries.iter(world).count(), 6);
}
