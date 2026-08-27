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
    let mut harness = Harness::new_feature_yard(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.loadout_is_ready(0)
            && harness.loadout_is_ready(1)
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
        .insert(Position::from_xy(grass_center.x, grass_center.y + 320.0));

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
        .insert(Position::from_xy(grass_center.x, grass_center.y + 160.0));
    harness.step_until(|harness| harness.client_ids(0).len() == 2);

    harness
        .server
        .world_mut()
        .entity_mut(observer)
        .insert(Position::from_xy(grass_center.x, grass_center.y + 320.0));
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

#[test]
fn self_cloak_ignores_proximity_and_team_scan_reveals_then_rehides() {
    let mut harness = Harness::new_feature_yard(2);
    harness.clients[0]
        .world_mut()
        .resource_mut::<ClientNetworkConfig>()
        .weapon_preset = Some(2);
    harness.clients[1]
        .world_mut()
        .resource_mut::<ClientNetworkConfig>()
        .weapon_preset = Some(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.loadout_is_ready(0)
            && harness.loadout_is_ready(1)
    });
    harness.install_saved_brawler_loadout(0, 2, 4, [5, 6]);
    harness.install_saved_brawler_loadout(1, 1, 3, [4, 6]);
    let (observer, subject, subject_id) = {
        let mut query = harness.server.world_mut().query_filtered::<(
            Entity,
            &NetworkEntityId,
            &TeamId,
            &lightyear::prelude::ControlledBy,
        ), With<Fighter>>();
        let values: Vec<_> = query
            .iter(harness.server.world())
            .map(|(entity, id, team, controlled)| (controlled.owner, entity, *id, *team))
            .collect();
        let observer = values
            .iter()
            .find(|value| value.0 == harness.server_links[0])
            .unwrap();
        let subject = values
            .iter()
            .find(|value| value.0 == harness.server_links[1])
            .unwrap();
        (observer.1, subject.1, subject.2)
    };
    harness
        .server
        .world_mut()
        .entity_mut(observer)
        .insert((
            Position::from_xy(0.0, 0.0),
            brawler::builds::AbilityState {
                charge: 1_000,
                phase: brawler::builds::AbilityPhase::Ready,
            },
        ))
        .remove::<brawler::matchplay::SpawnProtection>();
    harness.server.world_mut().entity_mut(subject).insert((
        Position::from_xy(0.0, 0.0),
        brawler::builds::AbilityState {
            charge: 1_000,
            phase: brawler::builds::AbilityPhase::Ready,
        },
    ));
    harness
        .server
        .world_mut()
        .entity_mut(subject)
        .remove::<brawler::matchplay::SpawnProtection>();
    harness.set_controlled_input(0, FighterInput::default());
    harness.set_controlled_input(1, FighterInput::default());
    harness.step();
    harness.set_controlled_input(
        1,
        FighterInput::from_axes(Vec2::ZERO, None, FighterInput::ULTIMATE),
    );
    harness.step_until(|harness| {
        matches!(
            harness
                .server
                .world()
                .get::<brawler::builds::AbilityState>(subject)
                .map(|ability| ability.phase),
            Some(brawler::builds::AbilityPhase::Cloaked { .. })
        )
    });
    let cloak_expiry = match harness
        .server
        .world()
        .get::<brawler::builds::AbilityState>(subject)
        .unwrap()
        .phase
    {
        brawler::builds::AbilityPhase::Cloaked {
            expires_at_tick, ..
        } => expires_at_tick,
        phase => panic!("expected active cloak, got {phase:?}"),
    };
    harness.set_controlled_input(1, FighterInput::default());
    harness.step_until(|harness| {
        !harness
            .client_ids(0)
            .iter()
            .any(|(_, id)| *id == subject_id)
    });
    harness.set_controlled_input(
        0,
        FighterInput::from_axes_with_aim_distance(
            Vec2::ZERO,
            Some(Vec2::X),
            Some(0.0),
            FighterInput::ULTIMATE | FighterInput::PRIMARY_FIRE,
        ),
    );
    let observer_ammo = harness
        .server
        .world()
        .get::<WeaponState>(observer)
        .expect("scanner weapon state")
        .ammo;
    harness.step_until(|harness| {
        harness
            .client_ids(0)
            .iter()
            .any(|(_, id)| *id == subject_id)
    });
    assert_eq!(
        harness
            .server
            .world()
            .get::<WeaponState>(observer)
            .expect("scanner weapon state after confirmation")
            .ammo,
        observer_ammo,
        "Reveal Scan confirmation must not also fire the primary weapon"
    );
    harness.set_controlled_input(0, FighterInput::default());
    let forced_reveal_expiry = harness
        .server
        .world()
        .get::<brawler::concealment::ForcedRevealSources>(subject)
        .and_then(|sources| sources.0.first())
        .map(|source| source.expires_at_tick)
        .expect("accepted scan installs its durable reveal source");
    assert!(
        forced_reveal_expiry < cloak_expiry,
        "scan reveal must expire while the accepted cloak remains active"
    );
    while harness.server.world().resource::<SimulationTick>().0 < forced_reveal_expiry + 10 {
        harness.step();
    }
    let final_tick = harness.server.world().resource::<SimulationTick>().0;
    let final_phase = harness
        .server
        .world()
        .get::<brawler::builds::AbilityState>(subject)
        .map(|ability| ability.phase);
    let final_sources = harness
        .server
        .world()
        .get::<brawler::concealment::ForcedRevealSources>(subject)
        .cloned();
    let visible_ids = harness.client_ids(0);
    assert!(
        !visible_ids.iter().any(|(_, id)| *id == subject_id),
        "subject stayed visible at tick {final_tick}; cloak={final_phase:?} sources={final_sources:?} ids={visible_ids:?}"
    );
}

#[test]
fn concealment_field_is_public_hides_at_range_and_confirmation_does_not_fire() {
    let mut harness = Harness::new_feature_yard(2);
    harness.clients[1]
        .world_mut()
        .resource_mut::<ClientNetworkConfig>()
        .weapon_preset = Some(3);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.loadout_is_ready(0)
            && harness.loadout_is_ready(1)
    });
    harness.install_saved_brawler_loadout(1, 3, 5, [5, 6]);
    let (observer, caster, caster_id) = {
        let mut query = harness.server.world_mut().query_filtered::<(
            Entity,
            &NetworkEntityId,
            &lightyear::prelude::ControlledBy,
        ), With<Fighter>>();
        let values: Vec<_> = query
            .iter(harness.server.world())
            .map(|(entity, id, controlled)| (controlled.owner, entity, *id))
            .collect();
        let observer = values
            .iter()
            .find(|value| value.0 == harness.server_links[0])
            .unwrap();
        let caster = values
            .iter()
            .find(|value| value.0 == harness.server_links[1])
            .unwrap();
        (observer.1, caster.1, caster.2)
    };
    harness
        .server
        .world_mut()
        .entity_mut(observer)
        .insert(Position::from_xy(320.0, 0.0));
    harness
        .server
        .world_mut()
        .entity_mut(caster)
        .insert((
            Position::from_xy(0.0, 0.0),
            brawler::builds::AbilityState {
                charge: 1_000,
                phase: brawler::builds::AbilityPhase::Ready,
            },
        ))
        .remove::<brawler::matchplay::SpawnProtection>();
    harness.set_controlled_input(0, FighterInput::default());
    harness.set_controlled_input(1, FighterInput::default());
    harness.step();
    let caster_ammo = harness
        .server
        .world()
        .get::<WeaponState>(caster)
        .expect("caster weapon state")
        .ammo;
    harness.set_controlled_input(
        1,
        FighterInput::from_axes_with_aim_distance(
            Vec2::ZERO,
            Some(Vec2::X),
            Some(0.0),
            FighterInput::ULTIMATE | FighterInput::PRIMARY_FIRE,
        ),
    );
    harness.step_until(|harness| {
        matches!(
            harness
                .server
                .world()
                .get::<brawler::builds::AbilityState>(caster)
                .map(|ability| ability.phase),
            Some(brawler::builds::AbilityPhase::FieldActive { .. })
        )
    });
    assert_eq!(
        harness
            .server
            .world()
            .get::<WeaponState>(caster)
            .expect("caster weapon after confirmation")
            .ammo,
        caster_ammo,
        "Concealment Field confirmation must not also fire the primary weapon"
    );
    harness.set_controlled_input(1, FighterInput::default());
    harness.step_until(|harness| {
        let public_fields = {
            let mut query = harness.clients[0]
                .world_mut()
                .query::<&brawler::concealment::ConcealmentFieldState>();
            query.iter(harness.clients[0].world()).count()
        };
        public_fields == 1 && !harness.client_ids(0).iter().any(|(_, id)| *id == caster_id)
    });

    harness
        .server
        .world_mut()
        .entity_mut(observer)
        .insert(Position::from_xy(120.0, 0.0));
    harness.step_until(|harness| harness.client_ids(0).iter().any(|(_, id)| *id == caster_id));

    harness
        .server
        .world_mut()
        .entity_mut(caster)
        .insert(Defeated {
            event_id: CombatEventId(99_001),
        });
    harness.step_until(|harness| {
        let mut query = harness.clients[0]
            .world_mut()
            .query::<&brawler::concealment::ConcealmentFieldState>();
        query.iter(harness.clients[0].world()).count() == 0
    });
}
