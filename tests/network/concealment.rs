//! Observer-specific concealment security scenarios.

use super::*;

fn public_projection_count(app: &mut App) -> usize {
    let mut query = app
        .world_mut()
        .query::<&brawler::matchplay::PublicParticipantState>();
    query.iter(app.world()).count()
}

#[test]
fn distant_grass_occupant_is_absent_but_public_roster_and_reveals_converge() {
    let mut harness = Harness::new_tidal_garden(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.selection_is_complete(0)
            && harness.selection_is_complete(1)
            && public_projection_count(&mut harness.clients[0]) == 2
            && public_projection_count(&mut harness.clients[1]) == 2
    });

    let grass_center = {
        let map = harness.server.world().resource::<ResolvedMap>();
        let catalog = harness.server.world().resource::<MapCatalogResource>();
        let placement = map
            .snapshot
            .placements
            .iter()
            .find(|placement| placement.asset_id == brawler::map::TALL_GRASS_ASSET)
            .unwrap();
        let asset = catalog.0.asset(placement.asset_id).unwrap();
        brawler::map::placement_world_center(map.snapshot.dimensions, asset, placement)
    };
    let (observer, subject, subject_id) = {
        let mut query = harness.server.world_mut().query_filtered::<(
            Entity,
            &NetworkEntityId,
            &lightyear::prelude::ControlledBy,
        ), With<Fighter>>();
        let mut values: Vec<_> = query
            .iter(harness.server.world())
            .map(|(entity, id, controlled)| (controlled.owner, entity, *id))
            .collect();
        values.sort_by_key(|(owner, _, _)| owner.index());
        let observer = values
            .iter()
            .find(|(owner, _, _)| *owner == harness.server_links[0])
            .unwrap()
            .1;
        let (_, subject, subject_id) = values
            .iter()
            .find(|(owner, _, _)| *owner == harness.server_links[1])
            .copied()
            .unwrap();
        (observer, subject, subject_id)
    };
    let attack_tick = harness.server.world().resource::<SimulationTick>().0;
    harness
        .server
        .world_mut()
        .entity_mut(subject)
        .insert(Position::from_xy(grass_center.x, grass_center.y));
    harness
        .server
        .world_mut()
        .entity_mut(observer)
        .insert(Position::from_xy(grass_center.x + 320.0, grass_center.y));

    harness.step_until(|harness| {
        harness.client_ids(0).len() == 1
            && harness.client_ids(1).len() == 2
            && public_projection_count(&mut harness.clients[0]) == 2
    });
    assert!(
        !harness
            .client_ids(0)
            .iter()
            .any(|(_, id)| *id == subject_id)
    );

    harness
        .server
        .world_mut()
        .entity_mut(observer)
        .insert(Position::from_xy(grass_center.x + 160.0, grass_center.y));
    harness.step_until(|harness| harness.client_ids(0).len() == 2);

    harness
        .server
        .world_mut()
        .entity_mut(observer)
        .insert(Position::from_xy(grass_center.x + 320.0, grass_center.y));
    harness.step_until(|harness| harness.client_ids(0).len() == 1);
    harness
        .server
        .world_mut()
        .resource_mut::<CombatOutbox>()
        .0
        .push(CombatCue::AttackAccepted {
            event_id: CombatEventId(90_001),
            tick: attack_tick,
            attack_id: AttackId(90_001),
            source: subject_id,
            position: WorldPoint::from(grass_center),
            weapon_definition_id: brawler::combat::WeaponDefinitionId(1),
            presentation_profile_id: brawler::combat::WeaponPresentationProfileId(1),
        });
    harness.step_until(|harness| harness.client_ids(0).len() == 2);
    assert_eq!(public_projection_count(&mut harness.clients[0]), 2);

    harness
        .server
        .world_mut()
        .entity_mut(subject)
        .remove::<brawler::concealment::ConcealmentRevealDeadlines>();
    harness.step_until(|harness| harness.client_ids(0).len() == 1);
    let target_team = *harness
        .server
        .world()
        .entity(subject)
        .get::<TeamId>()
        .unwrap();
    let damage_fact = |amount| CombatOutcomeFact {
        event_id: CombatEventId(90_002 + u64::from(amount)),
        tick: 0,
        attack_id: AttackId(90_002),
        source_kind: CombatSourceKind::PrimaryWeapon,
        source_player: None,
        source_network_id: None,
        source_team: None,
        target_network_id: subject_id,
        target_kind: brawler::combat::CombatTargetKind::Fighter,
        target_team,
        preset_id: None,
        recipe_fingerprint: None,
        position: WorldPoint::from(grass_center),
        engagement_distance: 320.0,
        kind: CombatOutcomeKind::Damage { amount },
    };
    harness
        .server
        .world_mut()
        .resource_mut::<CombatOutcomeFacts>()
        .0
        .push(damage_fact(0));
    for _ in 0..5 {
        harness.step();
    }
    assert_eq!(harness.client_ids(0).len(), 1);
    harness
        .server
        .world_mut()
        .resource_mut::<CombatOutcomeFacts>()
        .0
        .push(damage_fact(1));
    harness.step_until(|harness| harness.client_ids(0).len() == 2);
}
