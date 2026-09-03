use avian2d::prelude::Position;
use bevy::ecs::schedule::ApplyDeferred;
use bevy::prelude::*;

use super::super::{
    EffectTileOccupancy, MapCell, MapDynamicState, MapRoot, ResolvedMap,
    effect_tiles::EffectTilePulseState,
};

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct EffectTileOccupancySet;

pub(super) fn register(app: &mut App) {
    app.configure_sets(
        FixedUpdate,
        EffectTileOccupancySet
            .after(crate::gameplay::GameplaySet::Input)
            .before(crate::gameplay::GameplaySet::Simulation),
    )
    .add_systems(
        FixedUpdate,
        (resolve_effect_tile_occupancy, ApplyDeferred)
            .chain()
            .in_set(EffectTileOccupancySet),
    );
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "validated finite in-bounds world coordinates are quantized to bounded map cells"
)]
fn map_cell_at(map: &ResolvedMap, position: Vec2) -> Option<MapCell> {
    if !position.is_finite() {
        return None;
    }
    let dimensions = map.snapshot.dimensions;
    let local = position - dimensions.bounds().min;
    if local.x < 0.0 || local.y < 0.0 {
        return None;
    }
    let cell = MapCell::new(
        (local.x / super::super::MAP_CELL_SIZE_WORLD).floor() as u16,
        (local.y / super::super::MAP_CELL_SIZE_WORLD).floor() as u16,
    );
    dimensions.contains(cell).then_some(cell)
}

#[allow(clippy::type_complexity)]
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy owns injected system parameters at the schedule boundary"
)]
fn resolve_effect_tile_occupancy(
    mut commands: Commands,
    tick: Res<crate::timing::SimulationTick>,
    map: Option<Res<ResolvedMap>>,
    map_state: Query<&MapDynamicState, With<MapRoot>>,
    fighters: Query<
        (
            Entity,
            &Position,
            Option<&EffectTileOccupancy>,
            Has<crate::combat::Defeated>,
            Has<crate::matchplay::ActiveCombatant>,
        ),
        With<crate::protocol::Fighter>,
    >,
) {
    let (Some(map), Ok(map_state)) = (map, map_state.single()) else {
        return;
    };
    let generation = map_state.generation_id();
    for (entity, position, current, defeated, active) in &fighters {
        let tile = (!defeated && active)
            .then(|| map_cell_at(&map, position.0))
            .flatten()
            .and_then(|cell| map.effect_tiles_by_cell.get(&cell));
        let Some(tile) = tile else {
            if current.is_some() {
                commands
                    .entity(entity)
                    .remove::<(EffectTileOccupancy, EffectTilePulseState)>();
            }
            continue;
        };
        if current.is_some_and(|occupancy| {
            occupancy.generation == generation
                && occupancy.placement_id == tile.placement_id
                && occupancy.definition_id == tile.definition_id
                && occupancy.visual_profile_id == tile.visual_profile_id
                && occupancy.capabilities == tile.capabilities
        }) {
            continue;
        }
        let mut fighter = commands.entity(entity);
        fighter.insert(EffectTileOccupancy {
            generation,
            placement_id: tile.placement_id,
            definition_id: tile.definition_id,
            visual_profile_id: tile.visual_profile_id,
            capabilities: tile.capabilities,
            entered_at_tick: tick.0,
        });
        if let Some(effect) = tile.capabilities.periodic_damage {
            fighter.insert(EffectTilePulseState {
                next_pulse_at_tick: tick.0.saturating_add(u64::from(effect.interval_ticks)),
            });
        } else {
            fighter.remove::<EffectTilePulseState>();
        }
    }
}

pub(crate) fn apply_damage_tile_pulses(world: &mut World) {
    let active = world
        .query::<&crate::matchplay::MatchState>()
        .iter(world)
        .any(|state| matches!(state.phase, crate::matchplay::MatchPhase::Active { .. }));
    if !active {
        return;
    }
    if world.get_resource::<ResolvedMap>().is_none() {
        return;
    }
    let active_generation = {
        let mut roots = world.query_filtered::<&MapDynamicState, With<MapRoot>>();
        let mut roots = roots.iter(world);
        let Some(state) = roots.next() else {
            return;
        };
        let generation = state.generation_id();
        if roots.next().is_some() {
            return;
        }
        generation
    };
    let tick = world.resource::<crate::timing::SimulationTick>().0;
    let mut due: Vec<_> = world
        .query_filtered::<(
            Entity,
            &crate::protocol::NetworkEntityId,
            &EffectTileOccupancy,
            &EffectTilePulseState,
        ), (
            With<crate::protocol::Fighter>,
            With<crate::matchplay::ActiveCombatant>,
            Without<crate::combat::Defeated>,
        )>()
        .iter(world)
        .filter_map(|(entity, network_id, occupancy, pulse)| {
            if occupancy.generation != active_generation {
                return None;
            }
            (tick >= pulse.next_pulse_at_tick).then_some((entity, *network_id, *occupancy))
        })
        .collect();
    due.sort_by_key(|(_, network_id, occupancy)| {
        let effect = occupancy
            .capabilities
            .periodic_damage
            .expect("only periodic-damage occupancies have deadlines");
        (
            occupancy.placement_id,
            effect.damage,
            effect.interval_ticks,
            *network_id,
        )
    });
    let mut first = 0;
    while first < due.len() {
        let occupancy = due[first].2;
        let Some(effect) = occupancy.capabilities.periodic_damage else {
            first += 1;
            continue;
        };
        let mut end = first + 1;
        while end < due.len()
            && due[end].2.generation == occupancy.generation
            && due[end].2.placement_id == occupancy.placement_id
            && due[end].2.capabilities.periodic_damage == Some(effect)
        {
            end += 1;
        }
        let targets: Vec<_> = due[first..end]
            .iter()
            .map(|(entity, _, _)| *entity)
            .collect();
        match crate::combat::environment::apply_environment_damage_batch(
            world,
            crate::combat::environment::EnvironmentDamageBatch {
                targets: &targets,
                generation: occupancy.generation,
                placement_id: occupancy.placement_id,
                damage: effect.damage,
                tick,
                origin: None,
                attack: crate::combat::environment::EnvironmentAttack::Neutral,
                protection:
                    crate::combat::environment::EnvironmentProtection::RespectSpawnProtection,
            },
        ) {
            Ok(_) => {
                for entity in targets {
                    if let Some(mut current) = world.get_mut::<EffectTilePulseState>(entity) {
                        current.next_pulse_at_tick =
                            tick.saturating_add(u64::from(effect.interval_ticks));
                    }
                }
            }
            Err(error) => error!(?error, "damage-tile combat transaction failed"),
        }
        first = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn occupancy_retains_tuned_behavior_and_schedules_its_full_first_interval() {
        let mut app = App::new();
        app.insert_resource(crate::timing::SimulationTick(30));
        let mut map = crate::map::MapCatalogResource::from_world(app.world_mut())
            .0
            .resolve_preset(
                crate::map::FEATURE_YARD_WIPEOUT_PRESET,
                crate::map::MapInstanceId(3),
            )
            .unwrap();
        let tile = map
            .effect_tiles
            .iter_mut()
            .find(|tile| tile.definition_id == crate::map::EffectTileDefinitionId(3))
            .unwrap();
        tile.capabilities.periodic_damage = Some(crate::map::PeriodicDamageTileEffect {
            damage: 7,
            interval_ticks: 45,
        });
        map.effect_tiles_by_cell.insert(tile.cell, *tile);
        let position = map.snapshot.dimensions.cell_center(tile.cell);
        app.insert_resource(map);
        app.world_mut().spawn((
            MapRoot,
            MapDynamicState {
                map_instance_id: crate::map::MapInstanceId(3),
                generation: 1,
                revision: 0,
                terminal_states: Vec::new(),
            },
        ));
        let fighter = app
            .world_mut()
            .spawn((
                crate::protocol::Fighter,
                crate::matchplay::ActiveCombatant,
                Position(position),
            ))
            .id();
        app.add_systems(
            Update,
            (resolve_effect_tile_occupancy, ApplyDeferred).chain(),
        );

        app.update();

        let occupancy = app.world().get::<EffectTileOccupancy>(fighter).unwrap();
        assert_eq!(
            occupancy.capabilities.periodic_damage,
            Some(crate::map::PeriodicDamageTileEffect {
                damage: 7,
                interval_ticks: 45,
            })
        );
        assert_eq!(occupancy.entered_at_tick, 30);
        assert_eq!(
            app.world()
                .get::<EffectTilePulseState>(fighter)
                .map(|pulse| pulse.next_pulse_at_tick),
            Some(75)
        );
    }

    #[test]
    fn cell_lookup_uses_half_open_bounds() {
        let dimensions = crate::map::MapDimensions {
            width: 2,
            height: 2,
        };
        let map = ResolvedMap {
            snapshot: crate::map::ResolvedMapSnapshot {
                identity: crate::map::ResolvedMapIdentity {
                    instance_id: crate::map::MapInstanceId(1),
                    source_preset_id: None,
                    recipe_id: crate::map::MapRecipeId(1),
                    recipe_revision: 1,
                    recipe_fingerprint: crate::map::MapRecipeFingerprint(1),
                },
                catalog_schema_version: 8,
                recipe_schema_version: 5,
                presentation_theme_id: crate::map::MapPresentationThemeId(1),
                mode_definition_id: crate::map::WIPEOUT_MODE_DEFINITION,
                dimensions,
                default_surface_asset_id: crate::map::GROUND_ASSET,
                placements: Vec::new(),
                mode_anchors: Vec::new(),
            },
            spawn_points_by_team: BTreeMap::new(),
            static_colliders: Vec::new(),
            dynamic_placements: Vec::new(),
            player_only_surface_rects: Vec::new(),
            objective_zone: None,
            heist_safes: Vec::new(),
            zombie_ingress: Vec::new(),
            effect_tiles: Vec::new(),
            effect_tiles_by_cell: BTreeMap::new(),
        };
        assert_eq!(
            map_cell_at(&map, dimensions.cell_center(MapCell::new(0, 0))),
            Some(MapCell::new(0, 0))
        );
        assert_eq!(map_cell_at(&map, dimensions.bounds().max), None);
        assert_eq!(map_cell_at(&map, Vec2::new(f32::NAN, 1.0)), None);
    }

    fn damage_pulse_fixture() -> (App, Entity, Entity) {
        let mut app = App::new();
        app.init_resource::<crate::map::MapCatalogResource>()
            .init_resource::<crate::combat::NextCombatIds>()
            .init_resource::<crate::combat::CombatOutcomeFacts>()
            .init_resource::<crate::combat::CombatOutbox>()
            .init_resource::<crate::combat::CombatTelemetry>()
            .insert_resource(crate::timing::SimulationTick(75));
        let mut map = app
            .world()
            .resource::<crate::map::MapCatalogResource>()
            .0
            .resolve_preset(
                crate::map::FEATURE_YARD_WIPEOUT_PRESET,
                crate::map::MapInstanceId(3),
            )
            .unwrap();
        let tile = map
            .effect_tiles
            .iter_mut()
            .find(|tile| tile.definition_id == crate::map::EffectTileDefinitionId(3))
            .unwrap();
        tile.capabilities.periodic_damage = Some(crate::map::PeriodicDamageTileEffect {
            damage: 7,
            interval_ticks: 45,
        });
        let tile = *tile;
        map.effect_tiles_by_cell.insert(tile.cell, tile);
        let position = map.snapshot.dimensions.cell_center(tile.cell);
        app.insert_resource(map);
        let map_root = app
            .world_mut()
            .spawn((
                MapRoot,
                MapDynamicState {
                    map_instance_id: crate::map::MapInstanceId(3),
                    generation: 1,
                    revision: 0,
                    terminal_states: Vec::new(),
                },
            ))
            .id();
        app.world_mut().spawn(crate::matchplay::MatchState {
            match_id: crate::matchplay::MatchId(1),
            mode_definition_id: crate::map::WIPEOUT_MODE_DEFINITION,
            phase: crate::matchplay::MatchPhase::Active { ends_at_tick: 999 },
            rules_revision: 1,
        });
        let fighter = app
            .world_mut()
            .spawn((
                crate::protocol::Fighter,
                crate::matchplay::ActiveCombatant,
                Position(position),
                crate::combat::CurrentHealth(50),
                crate::combat::TeamId(0),
                crate::protocol::NetworkEntityId(10),
                EffectTileOccupancy {
                    generation: crate::map::MapDynamicGeneration {
                        map_instance_id: crate::map::MapInstanceId(3),
                        generation: 1,
                    },
                    placement_id: tile.placement_id,
                    definition_id: tile.definition_id,
                    visual_profile_id: tile.visual_profile_id,
                    capabilities: tile.capabilities,
                    entered_at_tick: 30,
                },
                EffectTilePulseState {
                    next_pulse_at_tick: 75,
                },
            ))
            .id();
        (app, map_root, fighter)
    }

    #[test]
    fn damage_pulses_wait_reschedule_from_now_and_never_catch_up() {
        let (mut app, map_root, fighter) = damage_pulse_fixture();

        apply_damage_tile_pulses(app.world_mut());
        assert_eq!(
            app.world().get::<crate::combat::CurrentHealth>(fighter),
            Some(&crate::combat::CurrentHealth(43))
        );
        assert_eq!(
            app.world()
                .get::<EffectTilePulseState>(fighter)
                .map(|pulse| pulse.next_pulse_at_tick),
            Some(120)
        );

        app.world_mut()
            .resource_mut::<crate::timing::SimulationTick>()
            .0 = 200;
        apply_damage_tile_pulses(app.world_mut());
        assert_eq!(
            app.world().get::<crate::combat::CurrentHealth>(fighter),
            Some(&crate::combat::CurrentHealth(36)),
            "a delayed tick applies one pulse, not a catch-up burst"
        );
        assert_eq!(
            app.world()
                .get::<EffectTilePulseState>(fighter)
                .map(|pulse| pulse.next_pulse_at_tick),
            Some(245)
        );

        app.world_mut()
            .get_mut::<MapDynamicState>(map_root)
            .unwrap()
            .generation = 2;
        apply_damage_tile_pulses(app.world_mut());
        assert_eq!(
            app.world().get::<crate::combat::CurrentHealth>(fighter),
            Some(&crate::combat::CurrentHealth(36)),
            "stale-generation occupancy cannot apply damage"
        );

        app.world_mut().despawn(map_root);
        apply_damage_tile_pulses(app.world_mut());
        assert_eq!(
            app.world().get::<crate::combat::CurrentHealth>(fighter),
            Some(&crate::combat::CurrentHealth(36)),
            "occupancy cannot apply damage without a live map root"
        );
    }

    #[test]
    fn fighters_due_on_one_tile_share_one_atomic_environment_attack() {
        let (mut app, _, first) = damage_pulse_fixture();
        let occupancy = *app.world().get::<EffectTileOccupancy>(first).unwrap();
        let position = *app.world().get::<Position>(first).unwrap();
        let second = app
            .world_mut()
            .spawn((
                crate::protocol::Fighter,
                crate::matchplay::ActiveCombatant,
                position,
                crate::combat::CurrentHealth(50),
                crate::combat::TeamId(1),
                crate::protocol::NetworkEntityId(11),
                occupancy,
                EffectTilePulseState {
                    next_pulse_at_tick: 75,
                },
            ))
            .id();

        apply_damage_tile_pulses(app.world_mut());

        assert_eq!(
            app.world().get::<crate::combat::CurrentHealth>(first),
            Some(&crate::combat::CurrentHealth(43))
        );
        assert_eq!(
            app.world().get::<crate::combat::CurrentHealth>(second),
            Some(&crate::combat::CurrentHealth(43))
        );
        assert_eq!(
            app.world()
                .resource::<crate::combat::NextCombatIds>()
                .next_attack_id,
            2,
            "one placement pulse owns one shared combat transaction"
        );
    }
}
