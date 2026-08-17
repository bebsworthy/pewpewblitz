//! Network integration scenarios extracted from the shared harness.

use super::*;

#[test]
fn build_selection_resolves_distinct_primary_weapons_and_spawns_spread_deliveries() {
    let mut harness = Harness::new(2);
    harness.clients[0]
        .world_mut()
        .resource_mut::<ClientNetworkConfig>()
        .build_preset = Some(2);
    harness.clients[1]
        .world_mut()
        .resource_mut::<ClientNetworkConfig>()
        .build_preset = Some(4);

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
                .query_filtered::<(), (With<Fighter>, With<Controlled>, With<SelectingBuild>)>();
            query.iter(world).next().is_none()
        })
    });

    let world = harness.server.world_mut();
    let mut query = world.query_filtered::<(
        &PlayerId,
        &brawler::builds::SelectedBuild,
        &brawler::builds::ResolvedMatchLoadout,
        &WeaponState,
    ), With<Fighter>>();
    let mut selections: Vec<_> = query
        .iter(world)
        .filter(|(player, _, _, _)| player.0 != 0)
        .map(|(player, build, loadout, state)| (player.0, build, loadout, state))
        .collect();
    selections.sort_by_key(|(_, _, loadout, _)| loadout.primary_weapon.source_preset_id);
    assert_eq!(selections.len(), 2);
    assert_eq!(
        selections[0].2.primary_weapon.source_preset_id,
        Some(WeaponPresetId(2))
    );
    assert_eq!(
        selections[1].2.primary_weapon.source_preset_id,
        Some(WeaponPresetId(4))
    );
    assert_eq!(selections[0].2.primary_weapon.recipe.economy.capacity(), 4);
    assert_eq!(selections[1].2.primary_weapon.recipe.economy.capacity(), 3);
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
#[allow(clippy::too_many_lines)]
fn selection_channel_is_connection_scoped_idempotent_and_strictly_ordered() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| harness.client_is_active(0) && harness.selection_is_complete(0));
    let accepted = harness
        .server
        .world()
        .get::<ServerSession>(harness.server_links[0])
        .and_then(|session| session.last_selection_response)
        .filter(|outcome| outcome.decision == BuildSelectionDecision::Accepted)
        .expect("automatic accepted selection outcome");
    let accepted_request = harness
        .server
        .world()
        .get::<ServerSession>(harness.server_links[0])
        .and_then(|session| session.last_selection_request)
        .expect("accepted selection request");
    harness.send_build_selection(0, accepted_request);
    harness.send_build_selection(0, accepted_request);
    harness.step();
    let selected = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(
            &SelectedBrawlerBuild,
            &brawler::builds::ResolvedMatchLoadout,
        ), (With<Fighter>, Without<TestDummy>)>();
        query
            .iter(world)
            .find(|(build, _)| Some(**build) == accepted.accepted_identity)
            .map(|(build, loadout)| (*build, loadout.primary_weapon.recipe_fingerprint))
            .expect("accepted selection")
    };
    let duplicate = harness
        .server
        .world()
        .get::<ServerSession>(harness.server_links[0])
        .and_then(|session| session.last_selection_response)
        .expect("duplicate outcome");
    assert_eq!(duplicate, accepted);
    assert_eq!(Some(selected.0), accepted.accepted_identity);
    assert_eq!(
        harness
            .server
            .world()
            .resource::<WeaponTelemetry>()
            .selection_records
            .len(),
        1
    );

    harness.send_build_selection(
        0,
        BuildSelectionRequest {
            request_id: accepted.request_id.saturating_sub(1),
            match_id: accepted.match_id,
            selection: BuildSelection::Preset(BuildPresetId(4)),
        },
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .get::<ServerSession>(harness.server_links[0])
            .and_then(|session| session.last_selection_response)
            .is_some_and(|outcome| outcome.decision == BuildSelectionDecision::Stale)
    });
    let stale = harness
        .server
        .world()
        .get::<ServerSession>(harness.server_links[0])
        .and_then(|session| session.last_selection_response)
        .expect("stale outcome");
    assert_eq!(stale.decision, BuildSelectionDecision::Stale);

    harness.send_build_selection(
        0,
        BuildSelectionRequest {
            request_id: accepted.request_id.saturating_add(1),
            match_id: accepted.match_id,
            selection: BuildSelection::Preset(BuildPresetId(4)),
        },
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .get::<ServerSession>(harness.server_links[0])
            .and_then(|session| session.last_selection_response)
            .is_some_and(|outcome| outcome.decision == BuildSelectionDecision::Accepted)
    });
    let replacement = harness
        .server
        .world()
        .get::<ServerSession>(harness.server_links[0])
        .and_then(|session| session.last_selection_response)
        .expect("replacement outcome");
    assert_eq!(replacement.decision, BuildSelectionDecision::Accepted);
    let final_build = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&SelectedBrawlerBuild, With<Fighter>>();
        query
            .iter(world)
            .find(|build| Some(**build) == replacement.accepted_identity)
            .copied()
            .expect("replacement selection is installed")
    };
    assert_ne!(final_build, selected.0);

    // Prove that an unknown build preset cannot mutate the accepted replacement.
    let fighter_entity = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<Entity, (With<Fighter>, Without<TestDummy>)>();
        query.iter(world).next().expect("player fighter")
    };
    harness.send_build_selection(
        0,
        BuildSelectionRequest {
            request_id: accepted.request_id.saturating_add(2),
            match_id: accepted.match_id,
            selection: BuildSelection::Preset(BuildPresetId(999)),
        },
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .get::<ServerSession>(harness.server_links[0])
            .and_then(|session| session.last_selection_response)
            .is_some_and(|outcome| outcome.decision == BuildSelectionDecision::UnknownId)
    });
    let unknown = harness
        .server
        .world()
        .get::<ServerSession>(harness.server_links[0])
        .and_then(|session| session.last_selection_response)
        .expect("unknown-preset outcome");
    assert_eq!(unknown.decision, BuildSelectionDecision::UnknownId);
    let unchanged_build = harness
        .server
        .world()
        .get::<SelectedBrawlerBuild>(fighter_entity)
        .copied()
        .expect("accepted build remains authoritative");
    assert_eq!(unchanged_build, final_build);
}
