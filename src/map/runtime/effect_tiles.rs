use avian2d::prelude::Position;
use bevy::ecs::schedule::ApplyDeferred;
use bevy::prelude::*;
use std::collections::BTreeMap;

use super::super::{EffectTileOccupancy, MapCell, MapDynamicState, MapRoot, ResolvedMap};

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
    let by_cell: BTreeMap<_, _> = map
        .effect_tiles
        .iter()
        .map(|tile| (tile.cell, tile))
        .collect();

    for (entity, position, current, defeated, active) in &fighters {
        let tile = (!defeated && active)
            .then(|| map_cell_at(&map, position.0))
            .flatten()
            .and_then(|cell| by_cell.get(&cell).copied());
        let Some(tile) = tile else {
            if current.is_some() {
                commands.entity(entity).remove::<EffectTileOccupancy>();
            }
            continue;
        };
        let kind = tile
            .behavior
            .kind()
            .expect("resolved effect tile has a kind");
        if current.is_some_and(|occupancy| {
            occupancy.generation == generation
                && occupancy.placement_id == tile.placement_id
                && occupancy.kind == kind
        }) {
            continue;
        }
        let next_pulse_at_tick = match tile.behavior {
            super::super::MapEffectTileBehavior::Damage { interval_ticks, .. } => {
                Some(tick.0.saturating_add(u64::from(interval_ticks)))
            }
            _ => None,
        };
        commands.entity(entity).insert(EffectTileOccupancy {
            generation,
            placement_id: tile.placement_id,
            kind,
            entered_at_tick: tick.0,
            next_pulse_at_tick,
        });
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
    let tick = world.resource::<crate::timing::SimulationTick>().0;
    let Some(map) = world.get_resource::<ResolvedMap>().cloned() else {
        return;
    };
    let behaviors: BTreeMap<_, _> = map
        .effect_tiles
        .iter()
        .map(|tile| (tile.placement_id, tile.behavior))
        .collect();
    let mut due: Vec<_> = world
        .query_filtered::<(Entity, &EffectTileOccupancy), (
            With<crate::protocol::Fighter>,
            With<crate::matchplay::ActiveCombatant>,
            Without<crate::combat::Defeated>,
        )>()
        .iter(world)
        .filter_map(|(entity, occupancy)| {
            let deadline = occupancy.next_pulse_at_tick?;
            let behavior = *behaviors.get(&occupancy.placement_id)?;
            (tick >= deadline).then_some((entity, *occupancy, behavior))
        })
        .collect();
    due.sort_by_key(|(_, occupancy, _)| occupancy.placement_id);
    for (entity, occupancy, behavior) in due {
        let super::super::MapEffectTileBehavior::Damage {
            damage,
            interval_ticks,
        } = behavior
        else {
            continue;
        };
        if let Some(mut current) = world.get_mut::<EffectTileOccupancy>(entity) {
            current.next_pulse_at_tick = Some(tick.saturating_add(u64::from(interval_ticks)));
        }
        crate::combat::environment::apply_neutral_environment_damage(
            world,
            crate::combat::environment::NeutralEnvironmentDamage {
                target: entity,
                generation: occupancy.generation,
                placement_id: occupancy.placement_id,
                damage,
                tick,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                catalog_schema_version: 7,
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
            effect_tiles: Vec::new(),
        };
        assert_eq!(
            map_cell_at(&map, dimensions.cell_center(MapCell::new(0, 0))),
            Some(MapCell::new(0, 0))
        );
        assert_eq!(map_cell_at(&map, dimensions.bounds().max), None);
    }

    #[test]
    fn damage_pulses_wait_reschedule_from_now_and_never_catch_up() {
        let mut app = App::new();
        app.init_resource::<crate::map::MapCatalogResource>()
            .init_resource::<crate::combat::NextCombatIds>()
            .init_resource::<crate::combat::CombatOutcomeFacts>()
            .init_resource::<crate::combat::CombatOutbox>()
            .init_resource::<crate::combat::CombatTelemetry>()
            .insert_resource(crate::timing::SimulationTick(60));
        let map = app
            .world()
            .resource::<crate::map::MapCatalogResource>()
            .0
            .resolve_preset(
                crate::map::FEATURE_YARD_WIPEOUT_PRESET,
                crate::map::MapInstanceId(3),
            )
            .unwrap();
        let tile = *map
            .effect_tiles
            .iter()
            .find(|tile| tile.behavior.kind() == Some(crate::map::EffectTileKind::Damage))
            .unwrap();
        let position = map.snapshot.dimensions.cell_center(tile.cell);
        app.insert_resource(map);
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
                    kind: crate::map::EffectTileKind::Damage,
                    entered_at_tick: 30,
                    next_pulse_at_tick: Some(60),
                },
            ))
            .id();

        apply_damage_tile_pulses(app.world_mut());
        assert_eq!(
            app.world().get::<crate::combat::CurrentHealth>(fighter),
            Some(&crate::combat::CurrentHealth(40))
        );
        assert_eq!(
            app.world()
                .get::<EffectTileOccupancy>(fighter)
                .and_then(|occupancy| occupancy.next_pulse_at_tick),
            Some(90)
        );

        app.world_mut()
            .resource_mut::<crate::timing::SimulationTick>()
            .0 = 200;
        apply_damage_tile_pulses(app.world_mut());
        assert_eq!(
            app.world().get::<crate::combat::CurrentHealth>(fighter),
            Some(&crate::combat::CurrentHealth(30)),
            "a delayed tick applies one pulse, not a catch-up burst"
        );
        assert_eq!(
            app.world()
                .get::<EffectTileOccupancy>(fighter)
                .and_then(|occupancy| occupancy.next_pulse_at_tick),
            Some(230)
        );
    }
}
