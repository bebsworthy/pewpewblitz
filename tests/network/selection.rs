//! Network integration scenarios extracted from the shared harness.

use super::*;

#[test]
fn milestone_five_selection_resolves_distinct_presets_and_spawns_spread_deliveries() {
    let mut harness = Harness::new(2);
    harness.clients[0]
        .world_mut()
        .resource_mut::<ClientNetworkConfig>()
        .weapon_preset = Some(2);
    harness.clients[1]
        .world_mut()
        .resource_mut::<ClientNetworkConfig>()
        .weapon_preset = Some(4);

    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
    });
    for _ in 0..60 {
        harness.step();
    }
    harness.step_until(|harness| {
        (0..2).all(|index| {
            let world = harness.clients[index].world_mut();
            let mut query = world
                .query_filtered::<(), (With<Fighter>, With<Controlled>, With<SelectingWeapon>)>();
            query.iter(world).next().is_none()
        })
    });

    let world = harness.server.world_mut();
    let mut query = world.query_filtered::<(&PlayerId, &SelectedBuild, &ResolvedWeapon, &WeaponState), With<Fighter>>();
    let mut selections: Vec<_> = query
        .iter(world)
        .filter(|(player, _, _, _)| player.0 != 0)
        .map(|(player, build, resolved, state)| (player.0, build, resolved, state))
        .collect();
    selections.sort_by_key(|(_, build, _, _)| build.source_preset_id);
    assert_eq!(selections.len(), 2);
    assert_eq!(selections[0].1.source_preset_id, Some(WeaponPresetId(2)));
    assert_eq!(selections[1].1.source_preset_id, Some(WeaponPresetId(4)));
    assert_eq!(selections[0].2.recipe.economy.capacity(), 4);
    assert_eq!(selections[1].2.recipe.economy.capacity(), 3);
    assert_eq!(selections[0].3.ammo, 4);
    assert_eq!(selections[1].3.ammo, 3);

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
    assert_eq!(deliveries.iter(world).count(), 7);
}

#[test]
fn selection_channel_is_connection_scoped_idempotent_and_strictly_ordered() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| harness.client_is_active(0) && harness.selection_is_complete(0));
    let accepted = harness
        .server
        .world()
        .get::<ServerSession>(harness.server_links[0])
        .and_then(|session| session.last_selection_response)
        .filter(|outcome| outcome.decision == WeaponSelectionDecision::Accepted)
        .expect("automatic accepted selection outcome");
    let accepted_request = harness
        .server
        .world()
        .get::<ServerSession>(harness.server_links[0])
        .and_then(|session| session.last_selection_request)
        .expect("accepted selection request");
    harness.send_weapon_selection(0, accepted_request);
    harness.send_weapon_selection(0, accepted_request);
    harness.step();
    let selected = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(&SelectedBuild, &ResolvedWeapon), With<Fighter>>();
        query
            .iter(world)
            .find(|(build, _)| build.source_preset_id == accepted.accepted_preset_id)
            .map(|(build, resolved)| (*build, resolved.recipe_fingerprint))
            .expect("accepted selection")
    };
    let duplicate = harness
        .server
        .world()
        .get::<ServerSession>(harness.server_links[0])
        .and_then(|session| session.last_selection_response)
        .expect("duplicate outcome");
    assert_eq!(duplicate, accepted);
    assert_eq!(selected.0.source_preset_id, accepted.accepted_preset_id);
    assert_eq!(
        harness
            .server
            .world()
            .resource::<WeaponTelemetry>()
            .selection_records
            .len(),
        1
    );

    harness.send_weapon_selection(
        0,
        WeaponSelectionRequest {
            request_id: accepted.request_id.saturating_sub(1),
            preset_id: WeaponPresetId(4),
        },
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .get::<ServerSession>(harness.server_links[0])
            .and_then(|session| session.last_selection_response)
            .is_some_and(|outcome| outcome.decision == WeaponSelectionDecision::StaleRequest)
    });
    let stale = harness
        .server
        .world()
        .get::<ServerSession>(harness.server_links[0])
        .and_then(|session| session.last_selection_response)
        .expect("stale outcome");
    assert_eq!(stale.decision, WeaponSelectionDecision::StaleRequest);

    harness.send_weapon_selection(
        0,
        WeaponSelectionRequest {
            request_id: accepted.request_id.saturating_add(1),
            preset_id: WeaponPresetId(4),
        },
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .get::<ServerSession>(harness.server_links[0])
            .and_then(|session| session.last_selection_response)
            .is_some_and(|outcome| outcome.decision == WeaponSelectionDecision::NotSelecting)
    });
    let not_selecting = harness
        .server
        .world()
        .get::<ServerSession>(harness.server_links[0])
        .and_then(|session| session.last_selection_response)
        .expect("not-selecting outcome");
    assert_eq!(
        not_selecting.decision,
        WeaponSelectionDecision::NotSelecting
    );
    let final_build = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&SelectedBuild, With<Fighter>>();
        query
            .iter(world)
            .find(|build| build.source_preset_id == accepted.accepted_preset_id)
            .copied()
            .expect("selection cannot be switched")
    };
    assert_eq!(final_build, selected.0);

    // Re-entering selection here isolates the registered request path from the automatic
    // first-selection helper and proves that an unknown preset cannot mutate the accepted build.
    let fighter_entity = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<Entity, (With<Fighter>, Without<TestDummy>)>();
        query.iter(world).next().expect("player fighter")
    };
    harness
        .server
        .world_mut()
        .entity_mut(fighter_entity)
        .insert(SelectingWeapon);
    harness.send_weapon_selection(
        0,
        WeaponSelectionRequest {
            request_id: accepted.request_id.saturating_add(2),
            preset_id: WeaponPresetId(999),
        },
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .get::<ServerSession>(harness.server_links[0])
            .and_then(|session| session.last_selection_response)
            .is_some_and(|outcome| outcome.decision == WeaponSelectionDecision::UnknownPreset)
    });
    let unknown = harness
        .server
        .world()
        .get::<ServerSession>(harness.server_links[0])
        .and_then(|session| session.last_selection_response)
        .expect("unknown-preset outcome");
    assert_eq!(unknown.decision, WeaponSelectionDecision::UnknownPreset);
    let unchanged_build = harness
        .server
        .world()
        .get::<SelectedBuild>(fighter_entity)
        .copied()
        .expect("accepted build remains authoritative");
    assert_eq!(unchanged_build, final_build);
}
