use super::*;
use super::{
    dynamics::{recovery_request_is_admitted, restore_map},
    installation::spawn_dynamic_collider,
};
use crate::combat::{
    AttackId, AttackSource, CombatSourceKind, CombatWorldEffectFact, WorldEffectDefinition,
    WorldPoint,
};
use crate::map::{
    MapAssetId, MapCatalogResource, MapDynamicState, MapInstanceId, MapPlacementOutcome,
    MapPlacementTransition, MapRoot, placement_world_center,
};
use crate::movement::{ArenaWall, destructible_map_collision_layers};
use avian2d::prelude::{Collider, Position, RigidBody, Rotation};

#[test]
fn invalid_installation_preserves_the_existing_map_root() {
    let mut app = App::new();
    app.init_resource::<MapCatalogResource>();
    let existing_root = app.world_mut().spawn(MapRoot).id();
    let mut resolved = app
        .world()
        .resource::<MapCatalogResource>()
        .0
        .resolve_preset(super::super::FEATURE_YARD_WIPEOUT_PRESET, MapInstanceId(99))
        .unwrap();
    resolved.dynamic_placements[0].asset_id = MapAssetId(u16::MAX);

    assert_eq!(
        install_resolved_map(app.world_mut(), resolved),
        Err("resolved dynamic asset disappeared".to_string())
    );
    assert!(app.world().get_entity(existing_root).is_ok());
}

fn test_attack_source() -> AttackSource {
    AttackSource {
        kind: CombatSourceKind::PrimaryWeapon,
        attack_id: AttackId(41),
        player_id: crate::protocol::PlayerId(7),
        owner_network_entity_id: crate::protocol::NetworkEntityId(70),
        team_id: crate::combat::TeamId(0),
        recipe_fingerprint: crate::combat::WeaponRecipeFingerprint::default(),
        presentation_profile_id: crate::combat::WeaponPresentationProfileId(3),
        legacy_compatibility: false,
        source_preset_id: None,
        origin: WorldPoint { x: 0.0, y: 0.0 },
        facing: 0.0,
    }
}

fn barrel_test_app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(avian2d::prelude::PhysicsPlugins::default())
        .init_resource::<CombatWorldEffectFacts>()
        .init_resource::<MapDynamicOutbox>()
        .init_resource::<MapDynamicTelemetry>()
        .init_resource::<MapCatalogResource>()
        .init_resource::<super::super::PendingWorldTargetDamages>()
        .init_resource::<super::super::WorldTargetDamageFacts>()
        .init_resource::<super::super::WorldObjectExplosionFacts>()
        .init_resource::<super::super::WorldObjectOutbox>()
        .init_resource::<super::super::WorldObjectTelemetry>()
        .init_resource::<super::super::PickupLifecycleFacts>()
        .init_resource::<super::super::PickupOutbox>()
        .init_resource::<super::super::PickupTelemetry>()
        .init_resource::<crate::combat::CombatOutcomeFacts>()
        .init_resource::<crate::combat::CombatOutbox>()
        .init_resource::<crate::combat::NextCombatIds>()
        .insert_resource(crate::timing::SimulationTick(9));
    object_authority::register_terminal_reactions(&mut app);
    let resolved = app
        .world()
        .resource::<MapCatalogResource>()
        .0
        .resolve_preset(super::super::FEATURE_YARD_WIPEOUT_PRESET, MapInstanceId(11))
        .unwrap();
    let snapshot = resolved.snapshot.clone();
    let root = app
        .world_mut()
        .spawn((
            MapRoot,
            snapshot.clone(),
            MapDynamicState {
                map_instance_id: MapInstanceId(11),
                generation: 1,
                revision: 0,
                terminal_states: Vec::new(),
            },
        ))
        .id();
    let catalog = app.world().resource::<MapCatalogResource>().0.clone();
    for placement in &resolved.dynamic_placements {
        spawn_dynamic_collider(
            app.world_mut(),
            MapInstanceId(11),
            1,
            &snapshot,
            catalog.asset(placement.asset_id).unwrap(),
            placement,
        );
    }
    app.world_mut().spawn(crate::matchplay::MatchState {
        match_id: crate::matchplay::MatchId(1),
        mode_definition_id: snapshot.mode_definition_id,
        phase: crate::matchplay::MatchPhase::Active { ends_at_tick: 999 },
        rules_revision: 1,
    });
    (app, root)
}

fn barrel_identity(app: &mut App, placement_id: u32) -> super::super::DamageableTargetIdentity {
    let world = app.world_mut();
    let mut query = world.query::<&super::super::DamageableTargetIdentity>();
    *query
        .iter(world)
        .find(|identity| identity.placement_id() == MapPlacementId(placement_id))
        .unwrap()
}

fn barrel_health(app: &mut App, placement_id: u32) -> Option<u16> {
    let world = app.world_mut();
    let mut query = world.query::<(
        &super::super::DamageableTargetIdentity,
        &crate::combat::CurrentHealth,
    )>();
    query
        .iter(world)
        .find(|(identity, _)| identity.placement_id() == MapPlacementId(placement_id))
        .map(|(_, health)| health.0)
}

fn destruction_fact(position: Vec2, radius: f32) -> CombatWorldEffectFact {
    CombatWorldEffectFact {
        tick: 1,
        source: crate::combat::CombatWorldEffectSource::Weapon {
            attack: AttackSource {
                kind: CombatSourceKind::PrimaryWeapon,
                attack_id: AttackId(1),
                player_id: crate::protocol::PlayerId(1),
                owner_network_entity_id: crate::protocol::NetworkEntityId(1),
                team_id: crate::combat::TeamId(0),
                recipe_fingerprint: crate::combat::WeaponRecipeFingerprint::default(),
                presentation_profile_id: crate::combat::WeaponPresentationProfileId(3),
                legacy_compatibility: false,
                source_preset_id: None,
                origin: WorldPoint { x: 0.0, y: 0.0 },
                facing: 0.0,
            },
            delivery_index: 0,
            effect_index: 0,
        },
        position: WorldPoint {
            x: position.x,
            y: position.y,
        },
        effect: WorldEffectDefinition::DestroyMap { radius },
    }
}

fn demolition_fact(position: Vec2, radius: f32, event_id: u64) -> CombatWorldEffectFact {
    CombatWorldEffectFact {
        tick: 2,
        source: crate::combat::CombatWorldEffectSource::Ultimate {
            event_id: crate::combat::CombatEventId(event_id),
            owner_network_entity_id: crate::protocol::NetworkEntityId(1),
            ultimate_id: crate::builds::UltimateDefinitionId(6),
        },
        position: position.into(),
        effect: WorldEffectDefinition::DestroyMap { radius },
    }
}

#[test]
fn recovery_admission_rejects_inactive_stale_and_rate_exhausted_requests() {
    let current = MapDynamicGeneration {
        map_instance_id: MapInstanceId(2),
        generation: 3,
    };
    assert!(recovery_request_is_admitted(true, current, current, 0));
    assert!(!recovery_request_is_admitted(false, current, current, 0));
    assert!(!recovery_request_is_admitted(
        true,
        MapDynamicGeneration {
            generation: 2,
            ..current
        },
        current,
        0,
    ));
    assert!(!recovery_request_is_admitted(
        true,
        current,
        current,
        MAX_RECOVERY_RESPONSES_PER_GENERATION,
    ));
}

#[test]
fn radius_brush_removes_whole_grid_cells_and_restart_restores_them() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<CombatWorldEffectFacts>()
        .init_resource::<MapDynamicOutbox>()
        .init_resource::<MapDynamicTelemetry>()
        .init_resource::<MapCatalogResource>();
    let resolved = app
        .world()
        .resource::<MapCatalogResource>()
        .0
        .resolve_preset(super::super::FEATURE_YARD_WIPEOUT_PRESET, MapInstanceId(1))
        .unwrap();
    // Install directly without Lightyear replication in this focused rule test.
    let snapshot = resolved.snapshot.clone();
    let root = app
        .world_mut()
        .spawn((
            MapRoot,
            snapshot.clone(),
            MapDynamicState {
                map_instance_id: MapInstanceId(1),
                generation: 1,
                revision: 0,
                terminal_states: Vec::new(),
            },
        ))
        .id();
    let catalog = app.world().resource::<MapCatalogResource>().0.clone();
    for placement in &resolved.dynamic_placements {
        let asset = catalog.asset(placement.asset_id).unwrap();
        spawn_dynamic_collider(
            app.world_mut(),
            MapInstanceId(1),
            1,
            &snapshot,
            asset,
            placement,
        );
    }
    let target = resolved
        .dynamic_placements
        .iter()
        .find(|placement| placement.placement_id == MapPlacementId(220))
        .unwrap();
    let target_asset = catalog.asset(target.asset_id).unwrap();
    let target_center = placement_world_center(snapshot.dimensions, target_asset, target);
    app.world_mut()
        .resource_mut::<CombatWorldEffectFacts>()
        .0
        .push(destruction_fact(target_center, 1.0));
    apply_map_destruction(app.world_mut());
    let state = app.world().get::<MapDynamicState>(root).unwrap();
    assert!(state.revision > 0);
    assert!(!state.terminal_states.is_empty());
    assert_eq!(state.terminal_states.len(), 1);
    assert!(
        state
            .terminal_states
            .iter()
            .all(|transition| transition.outcome == MapPlacementOutcome::Removed)
    );
    let removed_count = state.terminal_states.len();

    restore_map(app.world_mut());
    let state = app.world().get::<MapDynamicState>(root).unwrap();
    assert_eq!(state.generation, 2);
    assert_eq!(state.revision, 0);
    assert!(state.terminal_states.is_empty());
    let collider_count = {
        let world = app.world_mut();
        let mut query = world.query::<&DestructibleMapCollider>();
        query.iter(world).count()
    };
    assert_eq!(collider_count, 8);
    assert!(removed_count > 0);
    let telemetry = app.world().resource::<MapDynamicTelemetry>();
    assert_eq!(telemetry.destruction_requests, 1);
    assert_eq!(telemetry.destruction_applied, 1);
    assert_eq!(telemetry.placements_changed, removed_count as u64);
    assert_eq!(telemetry.demolition_requests, 0);

    app.world_mut()
        .resource_mut::<CombatWorldEffectFacts>()
        .0
        .push(demolition_fact(target_center, 1.0, 2));
    apply_map_destruction(app.world_mut());
    let revision_after_demolition = app.world().get::<MapDynamicState>(root).unwrap().revision;
    assert_eq!(revision_after_demolition, 1);
    app.world_mut()
        .resource_mut::<CombatWorldEffectFacts>()
        .0
        .push(demolition_fact(target_center, 1.0, 3));
    apply_map_destruction(app.world_mut());
    let state = app.world().get::<MapDynamicState>(root).unwrap();
    assert_eq!(state.revision, revision_after_demolition);
    let telemetry = app.world().resource::<MapDynamicTelemetry>();
    assert_eq!(telemetry.demolition_requests, 2);
    assert_eq!(telemetry.demolition_applied, 1);
    assert_eq!(telemetry.demolition_no_ops, 1);
    assert_eq!(telemetry.demolition_placements_changed, 1);
}

#[test]
fn one_hit_replaces_an_entire_rotated_barrier_and_restart_restores_it() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<CombatWorldEffectFacts>()
        .init_resource::<MapDynamicOutbox>()
        .init_resource::<MapDynamicTelemetry>()
        .init_resource::<MapCatalogResource>();
    let catalog = app.world().resource::<MapCatalogResource>().0.clone();
    let resolved = catalog
        .resolve_preset(super::super::FEATURE_YARD_WIPEOUT_PRESET, MapInstanceId(2))
        .unwrap();
    let snapshot = resolved.snapshot.clone();
    let target = resolved
        .dynamic_placements
        .iter()
        .find(|placement| placement.placement_id == MapPlacementId(200))
        .unwrap()
        .clone();
    let target_asset = catalog.asset(target.asset_id).unwrap();
    let target_center = placement_world_center(snapshot.dimensions, target_asset, &target);
    let root = app
        .world_mut()
        .spawn((
            MapRoot,
            snapshot.clone(),
            MapDynamicState {
                map_instance_id: MapInstanceId(2),
                generation: 1,
                revision: 0,
                terminal_states: Vec::new(),
            },
        ))
        .id();
    for placement in &resolved.dynamic_placements {
        spawn_dynamic_collider(
            app.world_mut(),
            MapInstanceId(2),
            1,
            &snapshot,
            catalog.asset(placement.asset_id).unwrap(),
            placement,
        );
    }
    app.world_mut()
        .resource_mut::<CombatWorldEffectFacts>()
        .0
        .push(destruction_fact(target_center, 1.0));
    apply_map_destruction(app.world_mut());

    let state = app.world().get::<MapDynamicState>(root).unwrap();
    assert_eq!(
        state.terminal_states,
        vec![MapPlacementTransition {
            placement_id: MapPlacementId(200),
            outcome: MapPlacementOutcome::ReplacedWith(super::super::RUBBLE_ASSET),
        }]
    );
    let collider_count = {
        let world = app.world_mut();
        let mut query = world.query::<&DestructibleMapCollider>();
        query.iter(world).count()
    };
    assert_eq!(collider_count, 7);

    restore_map(app.world_mut());
    assert!(
        app.world()
            .get::<MapDynamicState>(root)
            .unwrap()
            .terminal_states
            .is_empty()
    );
    let restored_count = {
        let world = app.world_mut();
        let mut query = world.query::<&DestructibleMapCollider>();
        query.iter(world).count()
    };
    assert_eq!(restored_count, 8);
}

#[test]
fn barrel_damage_explodes_once_chains_and_restart_restores_a_new_generation() {
    let (mut app, root) = barrel_test_app();
    let target = barrel_identity(&mut app, 240);
    let source = test_attack_source();
    app.world_mut()
        .resource_mut::<super::super::PendingWorldTargetDamages>()
        .0
        .push(super::super::PendingWorldTargetDamage {
            target,
            source,
            attack_id: source.attack_id,
            requested_damage: 60,
            delivery_index: 0,
            bundle_index: 0,
            effect_index: 0,
        });

    process_world_target_damage(app.world_mut());

    assert_eq!(barrel_health(&mut app, 240), None);
    assert_eq!(barrel_health(&mut app, 241), Some(25));
    let state = app.world().get::<MapDynamicState>(root).unwrap();
    assert_eq!(state.revision, 1);
    assert_eq!(
        state.terminal_states,
        vec![MapPlacementTransition {
            placement_id: MapPlacementId(240),
            outcome: MapPlacementOutcome::ReplacedWith(super::super::BARREL_WOOD_DEBRIS_ASSET,),
        }]
    );
    assert_eq!(
        app.world()
            .resource::<super::super::WorldObjectExplosionFacts>()
            .0
            .len(),
        1
    );
    assert_eq!(
        app.world()
            .resource::<super::super::WorldTargetDamageFacts>()
            .0
            .len(),
        3
    );
    let destroyed_collider_exists = {
        let world = app.world_mut();
        let mut colliders = world.query::<&DestructibleMapCollider>();
        colliders
            .iter(world)
            .any(|collider| collider.placement_id == MapPlacementId(101))
    };
    assert!(
        !destroyed_collider_exists,
        "the debris replacement is visual-only and nonblocking"
    );

    app.world_mut()
        .resource_mut::<super::super::PendingWorldTargetDamages>()
        .0
        .push(super::super::PendingWorldTargetDamage {
            target,
            source,
            attack_id: source.attack_id,
            requested_damage: 60,
            delivery_index: 0,
            bundle_index: 0,
            effect_index: 0,
        });
    process_world_target_damage(app.world_mut());
    assert_eq!(
        app.world()
            .resource::<super::super::WorldObjectExplosionFacts>()
            .0
            .len(),
        1,
        "the stale terminal identity cannot explode twice"
    );

    restore_map(app.world_mut());
    let state = app.world().get::<MapDynamicState>(root).unwrap();
    assert_eq!(state.generation, 2);
    assert_eq!(state.revision, 0);
    assert!(state.terminal_states.is_empty());
    let world = app.world_mut();
    let mut query = world.query::<(
        &super::super::DamageableTargetIdentity,
        &crate::combat::CurrentHealth,
    )>();
    let restored: Vec<_> = query
        .iter(world)
        .map(|(identity, health)| (identity.generation().generation, health.0))
        .collect();
    assert_eq!(restored.len(), 6);
    assert_eq!(
        restored.iter().filter(|entry| **entry == (2, 60)).count(),
        4
    );
    assert_eq!(
        restored.iter().filter(|entry| **entry == (2, 80)).count(),
        2
    );
}

#[test]
fn treasure_chest_commits_one_removed_state_and_one_generation_derived_pickup() {
    let (mut app, root) = barrel_test_app();
    let target = barrel_identity(&mut app, 260);
    let source = AttackSource {
        kind: CombatSourceKind::PrimaryWeapon,
        attack_id: AttackId(71),
        player_id: crate::protocol::PlayerId(1),
        owner_network_entity_id: crate::protocol::NetworkEntityId(1),
        team_id: crate::combat::TeamId(0),
        recipe_fingerprint: crate::combat::WeaponRecipeFingerprint::default(),
        presentation_profile_id: crate::combat::WeaponPresentationProfileId(3),
        legacy_compatibility: false,
        source_preset_id: None,
        origin: WorldPoint { x: 0.0, y: 0.0 },
        facing: 0.0,
    };
    for _ in 0..2 {
        app.world_mut()
            .resource_mut::<super::super::PendingWorldTargetDamages>()
            .0
            .push(super::super::PendingWorldTargetDamage {
                target,
                source,
                attack_id: source.attack_id,
                requested_damage: 80,
                delivery_index: 0,
                bundle_index: 0,
                effect_index: 0,
            });
        process_world_target_damage(app.world_mut());
    }

    assert_eq!(barrel_health(&mut app, 260), None);
    let state = app.world().get::<MapDynamicState>(root).unwrap();
    assert!(state.terminal_states.contains(&MapPlacementTransition {
        placement_id: MapPlacementId(260),
        outcome: MapPlacementOutcome::Removed,
    }));
    let world = app.world_mut();
    let pickups = world
        .query::<&super::super::RestorationPickupIdentity>()
        .iter(world)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(pickups.len(), 1);
    assert_eq!(
        pickups[0],
        super::super::RestorationPickupIdentity {
            generation: target.generation(),
            source_placement_id: MapPlacementId(260),
        }
    );
}

#[test]
fn barrel_explosion_respects_authoritative_map_occlusion() {
    let (mut app, _) = barrel_test_app();
    let target = barrel_identity(&mut app, 240);
    let source_position = {
        let world = app.world_mut();
        let mut query = world.query::<(&super::super::DamageableTargetIdentity, &Position)>();
        query
            .iter(world)
            .find(|(identity, _)| **identity == target)
            .unwrap()
            .1
            .0
    };
    let chained_position = {
        let chained = barrel_identity(&mut app, 241);
        let world = app.world_mut();
        let mut query = world.query::<(&super::super::DamageableTargetIdentity, &Position)>();
        query
            .iter(world)
            .find(|(identity, _)| **identity == chained)
            .unwrap()
            .1
            .0
    };
    let midpoint = (source_position + chained_position) * 0.5;
    app.world_mut().spawn((
        ArenaWall,
        RigidBody::Static,
        Collider::rectangle(16.0, 64.0),
        destructible_map_collision_layers(),
        Position::from_xy(midpoint.x, midpoint.y),
        Rotation::default(),
    ));
    app.update();
    let source = test_attack_source();
    app.world_mut()
        .resource_mut::<super::super::PendingWorldTargetDamages>()
        .0
        .push(super::super::PendingWorldTargetDamage {
            target,
            source,
            attack_id: source.attack_id,
            requested_damage: 60,
            delivery_index: 0,
            bundle_index: 0,
            effect_index: 0,
        });

    process_world_target_damage(app.world_mut());

    assert_eq!(barrel_health(&mut app, 241), Some(60));
}

#[test]
fn barrel_explosion_damages_combatants_as_environment_without_object_outcome_leakage() {
    let (mut app, _) = barrel_test_app();
    let target = barrel_identity(&mut app, 240);
    let position = {
        let world = app.world_mut();
        let mut query = world.query::<(&super::super::DamageableTargetIdentity, &Position)>();
        query
            .iter(world)
            .find(|(identity, _)| **identity == target)
            .unwrap()
            .1
            .0
    };
    app.world_mut().spawn((
        crate::protocol::Fighter,
        crate::protocol::PlayerId(7),
        crate::protocol::NetworkEntityId(70),
        crate::combat::TeamId(0),
        crate::combat::CurrentHealth(100),
        crate::matchplay::MatchMember(crate::matchplay::MatchId(1)),
        crate::matchplay::ActiveCombatant,
        Position::from_xy(position.x - 256.0, position.y),
    ));
    let fighter = app
        .world_mut()
        .spawn((
            crate::protocol::Fighter,
            crate::protocol::NetworkEntityId(88),
            crate::combat::TeamId(1),
            crate::combat::CurrentHealth(100),
            Position::from_xy(position.x, position.y + 48.0),
        ))
        .id();
    app.update();
    let source = test_attack_source();
    app.world_mut()
        .resource_mut::<super::super::PendingWorldTargetDamages>()
        .0
        .push(super::super::PendingWorldTargetDamage {
            target,
            source,
            attack_id: source.attack_id,
            requested_damage: 60,
            delivery_index: 0,
            bundle_index: 0,
            effect_index: 0,
        });

    process_world_target_damage(app.world_mut());

    assert_eq!(
        app.world()
            .get::<crate::combat::CurrentHealth>(fighter)
            .unwrap()
            .0,
        65
    );
    let outcomes = &app
        .world()
        .resource::<crate::combat::CombatOutcomeFacts>()
        .0;
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].source_kind, CombatSourceKind::Environment);
    assert_eq!(outcomes[0].source_team, Some(crate::combat::TeamId(0)));
    assert_eq!(
        app.world()
            .resource::<super::super::WorldTargetDamageFacts>()
            .0
            .len(),
        3,
        "the primary barrel, chained barrel, and nearby chest use the object-fact channel"
    );
}
