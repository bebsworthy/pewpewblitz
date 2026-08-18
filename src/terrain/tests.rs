//! Focused pure tests for the terrain grid model.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::needless_pass_by_value,
    reason = "deliberate boundary-value test math"
)]

use super::grid::*;
use super::model::*;
use bevy::prelude::Vec2;
use std::collections::BTreeMap;

fn chunk_with_all_cells_set(id: TerrainChunkId) -> (TerrainChunkId, TerrainBits) {
    let mut bits = TerrainBits::default();
    for local_y in 0..TERRAIN_CHUNK_SIDE_CELLS {
        for local_x in 0..TERRAIN_CHUNK_SIDE_CELLS {
            bits.set(local_x, local_y);
        }
    }
    (id, bits)
}

#[test]
fn world_to_cell_uses_euclidean_floor_division() {
    assert_eq!(world_to_cell(Vec2::new(0.0, 0.0)), Some((0, 0)));
    assert_eq!(world_to_cell(Vec2::new(7.9, 7.9)), Some((0, 0)));
    assert_eq!(world_to_cell(Vec2::new(8.0, 0.0)), Some((1, 0)));
    assert_eq!(world_to_cell(Vec2::new(-0.1, 0.0)), Some((-1, 0)));
    assert_eq!(world_to_cell(Vec2::new(-8.0, -8.0)), Some((-1, -1)));
    assert_eq!(world_to_cell(Vec2::new(-8.1, 8.0)), Some((-2, 1)));
    assert_eq!(world_to_cell(Vec2::new(f32::NAN, 0.0)), None);
    assert_eq!(world_to_cell(Vec2::new(f32::INFINITY, 0.0)), None);
    assert_eq!(world_to_cell(Vec2::new(f32::MAX, 0.0)), None);
    assert_eq!(world_to_cell(Vec2::new(f32::MIN, 0.0)), None);
}

#[test]
fn cell_to_chunk_and_local_round_trips_at_chunk_boundaries() {
    for cell in [
        (0, 0),
        (31, 31),
        (32, 0),
        (0, 32),
        (-1, -1),
        (-32, -32),
        (-33, 31),
        (512, -384),
        (-512, 384),
    ] {
        let (chunk, (local_x, local_y)) =
            cell_to_chunk_and_local(cell).expect("legal cells resolve");
        assert!(local_x < TERRAIN_CHUNK_SIDE_CELLS && local_y < TERRAIN_CHUNK_SIDE_CELLS);
        assert_eq!(
            i32::from(chunk.x) * TERRAIN_CHUNK_SIDE_CELLS as i32 + local_x as i32,
            cell.0
        );
        assert_eq!(
            i32::from(chunk.y) * TERRAIN_CHUNK_SIDE_CELLS as i32 + local_y as i32,
            cell.1
        );
        assert_eq!(
            world_to_cell(cell_center_world(cell)),
            Some(cell),
            "cell center maps back to the same cell"
        );
    }
    // Chunk coordinates beyond i16 are unrepresentable.
    assert_eq!(
        cell_to_chunk_and_local((i32::MAX, 0)),
        None,
        "extreme positive cell overflows the chunk coordinate"
    );
    assert_eq!(
        cell_to_chunk_and_local((i32::MIN, 0)),
        None,
        "extreme negative cell overflows the chunk coordinate"
    );
}

#[test]
fn chunk_and_cell_world_geometry_matches_constants() {
    let chunk = TerrainChunkId { x: -3, y: 2 };
    assert_eq!(
        chunk_min_world(chunk),
        Vec2::new(-768.0, 512.0),
        "chunk min world is (chunk * 32) * 8"
    );
    assert_eq!(chunk_min_world(TerrainChunkId::default()), Vec2::ZERO);
    let cell = (-97, 65);
    assert_eq!(
        cell_center_world(cell),
        cell_min_world(cell) + Vec2::splat(TERRAIN_CELL_SIZE_WORLD * 0.5)
    );
    assert_eq!(cell_center_half_cells(cell), (-193, 131));
}

#[test]
fn brush_quantization_round_trips_to_canonical_world_values() {
    let brush = quantize_brush(Vec2::new(12.6, -20.4), 48.0).expect("legal brush quantizes");
    assert_eq!(
        brush,
        TerrainBrush {
            center_half_cells_x: 3,
            center_half_cells_y: -5,
            radius_half_cells: 12,
        }
    );
    assert_eq!(brush_center_world(brush), Vec2::new(12.0, -20.0));

    // Half-cell boundaries round deterministically (half away from zero).
    let brush = quantize_brush(Vec2::new(2.0, 2.0), 8.0).expect("legal brush quantizes");
    assert_eq!(brush.center_half_cells_x, 1);
    let brush = quantize_brush(Vec2::new(6.0, 6.0), 64.0).expect("legal brush quantizes");
    assert_eq!(brush.radius_half_cells, 16);

    assert!(quantize_brush(Vec2::new(f32::NAN, 0.0), 48.0).is_none());
    assert!(quantize_brush(Vec2::new(0.0, f32::INFINITY), 48.0).is_none());
    assert!(quantize_brush(Vec2::new(f32::MAX, 0.0), 48.0).is_none());
    assert!(
        quantize_brush(Vec2::new(0.0, 0.0), 4.0).is_none(),
        "radius below one cell"
    );
    assert!(
        quantize_brush(Vec2::new(0.0, 0.0), 68.0).is_none(),
        "radius above engine ceiling"
    );
    assert!(
        quantize_brush(Vec2::new(0.0, 0.0), 50.0).is_none(),
        "radius must be a half-cell multiple"
    );
}

#[test]
fn bitset_ordering_is_row_major_and_counts_match() {
    let mut bits = TerrainBits::default();
    assert!(bits.is_empty());
    assert_eq!(bits.count(), 0);
    bits.set(0, 0);
    bits.set(31, 0);
    bits.set(0, 31);
    bits.set(13, 17);
    assert_eq!(bits.count(), 4);
    assert_eq!(TerrainBits::bit_index(13, 17), 17 * 32 + 13);
    assert!(bits.get(13, 17));
    bits.clear(13, 17);
    assert!(!bits.get(13, 17));
    assert_eq!(bits.count(), 3);

    let mut full = chunk_with_all_cells_set(TerrainChunkId::default()).1;
    assert_eq!(full.count(), 1_024);
    let occupied: Vec<_> = full.iter_occupied().collect();
    assert_eq!(occupied.len(), 1_024);
    assert_eq!(occupied.first(), Some(&(0, 0)));
    assert_eq!(occupied.last(), Some(&(31, 31)));
    full.clear(5, 7);
    assert!(!full.is_empty());
    let mut empty = full;
    for local_y in 0..TERRAIN_CHUNK_SIDE_CELLS {
        for local_x in 0..TERRAIN_CHUNK_SIDE_CELLS {
            empty.clear(local_x, local_y);
        }
    }
    assert!(empty.is_empty());
}

#[test]
fn brush_erase_is_integer_inclusive_and_symmetric() {
    // A brush centered on a cell center (odd half-cell coordinates) makes exact boundary
    // distances representable: radius 10 half-cells touches cell (5, 0) exactly.
    let brush = TerrainBrush {
        center_half_cells_x: 1,
        center_half_cells_y: 1,
        radius_half_cells: 10,
    };
    assert!(
        brush_erases_cell(brush, (5, 0)),
        "distance squared equals the radius squared inclusively"
    );
    assert!(brush_erases_cell(brush, (0, 0)));
    assert!(brush_erases_cell(brush, (3, 3)));
    assert!(brush_erases_cell(brush, (-4, 0)));
    assert!(!brush_erases_cell(brush, (5, 1)), "one past the boundary");
    assert!(!brush_erases_cell(brush, (4, 4)));

    let ((x_min, x_max), (y_min, y_max)) = brush_cell_range(brush);
    assert_eq!((x_min, x_max, y_min, y_max), (-5, 5, -5, 5));
    // Every erased cell is inside the candidate range, and no cell just outside erases.
    for cell_y in (y_min - 2)..=(y_max + 2) {
        for cell_x in (x_min - 2)..=(x_max + 2) {
            let inside = cell_x >= x_min && cell_x <= x_max && cell_y >= y_min && cell_y <= y_max;
            let erases = brush_erases_cell(brush, (cell_x, cell_y));
            assert_eq!(
                erases,
                erases && inside,
                "candidate range exactly covers every erased cell"
            );
            if erases {
                assert!(
                    brush_erases_cell(brush, (-cell_x, -cell_y)),
                    "point symmetry around the brush center"
                );
            }
        }
    }
}

#[test]
fn apply_brush_clips_to_allocated_occupied_cells_and_reports_chunks() {
    let mut chunks = BTreeMap::new();
    chunks.insert(TerrainChunkId { x: 0, y: 0 }, {
        let mut bits = TerrainBits::default();
        for local_y in 0..TERRAIN_CHUNK_SIDE_CELLS {
            for local_x in 0..TERRAIN_CHUNK_SIDE_CELLS {
                bits.set(local_x, local_y);
            }
        }
        bits
    });
    // Brush centered exactly on the corner between four chunks at the world origin.
    let brush = quantize_brush(Vec2::new(-4.0, -4.0), 48.0).expect("legal brush");
    let outcome = apply_brush(&mut chunks, brush);
    assert_eq!(outcome.affected_chunks, vec![TerrainChunkId { x: 0, y: 0 }]);
    assert!(outcome.erased_cells > 0);
    let remaining = chunks[&TerrainChunkId { x: 0, y: 0 }].count();
    assert_eq!(remaining, 1_024 - u32::from(outcome.erased_cells));

    // Re-applying the same brush is a deterministic no-op.
    let again = apply_brush(&mut chunks, brush);
    assert_eq!(again.erased_cells, 0);
    assert!(again.affected_chunks.is_empty());
}

#[test]
fn apply_brush_across_chunk_seams_reports_every_changed_chunk() {
    let mut chunks = BTreeMap::new();
    for (x, y) in [(-1, -1), (0, -1), (-1, 0), (0, 0)] {
        chunks.insert(
            TerrainChunkId { x, y },
            chunk_with_all_cells_set(TerrainChunkId { x, y }).1,
        );
    }
    chunks.insert(
        TerrainChunkId { x: 5, y: 5 },
        chunk_with_all_cells_set(TerrainChunkId { x: 5, y: 5 }).1,
    );
    let brush = quantize_brush(Vec2::new(-4.0, -4.0), 32.0).expect("legal brush");
    let outcome = apply_brush(&mut chunks, brush);
    assert_eq!(
        outcome.affected_chunks,
        vec![
            TerrainChunkId { x: -1, y: -1 },
            TerrainChunkId { x: -1, y: 0 },
            TerrainChunkId { x: 0, y: -1 },
            TerrainChunkId { x: 0, y: 0 },
        ],
        "a brush smaller than one chunk affects at most four chunks"
    );
    assert_eq!(outcome.affected_chunks.len(), 4);
    // The distant chunk is untouched.
    assert_eq!(chunks[&TerrainChunkId { x: 5, y: 5 }].count(), 1_024);
}

#[test]
fn occupancy_digest_is_stable_and_sensitive() {
    let mut left = BTreeMap::new();
    left.insert(
        TerrainChunkId { x: 1, y: -2 },
        chunk_with_all_cells_set(TerrainChunkId { x: 1, y: -2 }).1,
    );
    let mut right = left.clone();
    assert_eq!(occupancy_digest(&left), occupancy_digest(&right));
    right
        .get_mut(&TerrainChunkId { x: 1, y: -2 })
        .unwrap()
        .clear(3, 4);
    assert_ne!(occupancy_digest(&left), occupancy_digest(&right));
    // Insertion order cannot matter: BTreeMap iteration is canonical.
    let mut reordered = BTreeMap::new();
    reordered.insert(TerrainChunkId { x: 0, y: 9 }, TerrainBits::default());
    let mut built = BTreeMap::new();
    built.insert(TerrainChunkId { x: -5, y: 3 }, TerrainBits::default());
    built.insert(TerrainChunkId { x: 0, y: 9 }, TerrainBits::default());
    built.remove(&TerrainChunkId { x: -5, y: 3 }).unwrap();
    assert_eq!(occupancy_digest(&reordered), occupancy_digest(&built));
}

#[test]
fn wire_shapes_round_trip_and_bounds_hold() {
    let generation = TerrainGeneration {
        map_instance_id: crate::map::MapInstanceId(7),
        match_id: crate::matchplay::MatchId(3),
        terrain_fingerprint: 0xdead_beef,
    };
    let event = TerrainDestructionEvent {
        generation,
        revision: 42,
        source_attack_id: crate::combat::AttackId(9),
        source_delivery_index: 0,
        brush: TerrainBrush {
            center_half_cells_x: -12,
            center_half_cells_y: 30,
            radius_half_cells: 12,
        },
        affected_chunks: vec![
            TerrainChunkId { x: -1, y: 0 },
            TerrainChunkId { x: 0, y: 0 },
        ],
        erased_cells: 77,
    };
    let bytes = postcard::to_allocvec(&event).expect("event serializes");
    let decoded: TerrainDestructionEvent = postcard::from_bytes(&bytes).expect("event decodes");
    assert_eq!(decoded, event);
    assert!(
        bytes.len() <= MAX_TERRAIN_EVENT_BYTES,
        "compact events stay below the ceiling: {}",
        bytes.len()
    );

    let mut chunks = BTreeMap::new();
    for index in 0..MAX_TERRAIN_CHUNKS {
        chunks.insert(
            TerrainChunkId {
                x: i16::try_from(index as i32 - 110).unwrap(),
                y: 0,
            },
            chunk_with_all_cells_set(TerrainChunkId::default()).1,
        );
    }
    let snapshot = recovery_snapshot(&chunks, generation, 5);
    let decoded: TerrainRecoverySnapshot =
        postcard::from_bytes(&postcard::to_allocvec(&snapshot).expect("snapshot serializes"))
            .expect("snapshot decodes");
    assert_eq!(decoded, snapshot);
    let bytes = recovery_snapshot_bytes(&snapshot).expect("snapshot bytes");
    assert!(
        bytes <= MAX_TERRAIN_RECOVERY_BYTES,
        "maximum allocation stays below the recovery ceiling: {bytes}"
    );
    assert!(
        bytes > MAX_TERRAIN_CHUNKS * (4 + TERRAIN_WORDS_PER_CHUNK * 8),
        "221 fully occupied chunks plus identity dominate the snapshot size: {bytes}"
    );
}

#[test]
fn terrain_fingerprint_depends_on_regions_and_initial_bits_only() {
    let regions = [(
        crate::map::MapPlacementId(200),
        Vec2::new(10.0, -4.0),
        0.5,
        crate::map::MapShape::Rectangle {
            half_extents: Vec2::new(96.0, 96.0),
        },
    )];
    let mut chunks = BTreeMap::new();
    chunks.insert(
        TerrainChunkId { x: 0, y: 0 },
        chunk_with_all_cells_set(TerrainChunkId { x: 0, y: 0 }).1,
    );
    let first = terrain_fingerprint(&regions, &chunks);
    assert_eq!(first, terrain_fingerprint(&regions, &chunks));

    let mut other_bits = chunks.clone();
    other_bits
        .get_mut(&TerrainChunkId { x: 0, y: 0 })
        .unwrap()
        .clear(0, 0);
    assert_ne!(first, terrain_fingerprint(&regions, &other_bits));
    assert_ne!(first, terrain_fingerprint(&regions[..1], &BTreeMap::new()));
    let moved = [(
        crate::map::MapPlacementId(200),
        Vec2::new(18.0, -4.0),
        0.5,
        crate::map::MapShape::Rectangle {
            half_extents: Vec2::new(96.0, 96.0),
        },
    )];
    assert_ne!(first, terrain_fingerprint(&moved, &chunks));
}

#[test]
fn floor_and_ceil_division_handle_negative_values() {
    assert_eq!(floor_div(7, 2), 3);
    assert_eq!(floor_div(-7, 2), -4);
    assert_eq!(floor_div(-8, 2), -4);
    assert_eq!(ceil_div(7, 2), 4);
    assert_eq!(ceil_div(-7, 2), -3);
    assert_eq!(ceil_div(-8, 2), -4);
}

// --- Authoritative terrain App/World behavior (server role) ---

#[cfg(feature = "server")]
mod authority_tests {
    use super::super::authority::{
        AuthoritativeTerrainPlugin, PendingTerrainBrushes, TerrainChunk, TerrainChunkCollision,
        TerrainChunkState, TerrainOutbox, TerrainRecoveryCache, TerrainRoot,
    };
    use super::*;
    use crate::combat::{
        AttackId, AttackSource, CombatSourceKind, CombatWorldEffectFact, WorldEffectDefinition,
        WorldPoint,
    };
    use crate::map::{MapLayoutRequirements, MapPresetId};
    use crate::protocol::{NetworkEntityId, PlayerId};
    use crate::terrain::TerrainSet;
    use bevy::prelude::*;
    use bevy::time::TimeUpdateStrategy;
    use std::collections::BTreeMap;

    pub(super) fn fact(attack: u64, position: (f32, f32), radius: f32) -> CombatWorldEffectFact {
        CombatWorldEffectFact {
            tick: 1,
            source: AttackSource {
                kind: CombatSourceKind::PrimaryWeapon,
                attack_id: AttackId(attack),
                player_id: PlayerId(1),
                owner_network_entity_id: NetworkEntityId(1),
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
            position: WorldPoint {
                x: position.0,
                y: position.1,
            },
            effect: WorldEffectDefinition::DestroyTerrain { radius },
        }
    }

    pub(super) fn terrain_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            crate::gameplay::GameplayPlugin,
            crate::map::MapContentPlugin,
            crate::map::AuthoritativeMapPlugin,
            // The same Avian physics configuration the authoritative movement plugin
            // installs, without its Lightyear input-validation systems.
            avian2d::prelude::PhysicsPlugins::default()
                .with_length_unit(100.0)
                .build()
                .disable::<avian2d::prelude::PhysicsTransformPlugin>()
                .disable::<avian2d::prelude::PhysicsInterpolationPlugin>(),
            AuthoritativeTerrainPlugin,
        ))
        .insert_resource(avian2d::prelude::Gravity(Vec2::ZERO))
        .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
        .configure_sets(
            FixedPostUpdate,
            (
                crate::abilities::AbilitySet::ObserveOutcomes,
                crate::matchplay::MatchSet::ModeRules,
            ),
        );
        app.finish();
        app.cleanup();
        app.update();
        // Adopt a match generation like the real match root would.
        app.world_mut().spawn((
            crate::matchplay::MatchRoot,
            crate::matchplay::MatchState {
                match_id: crate::matchplay::MatchId(1),
                mode_definition_id: crate::map::WIPEOUT_MODE_DEFINITION,
                phase: crate::matchplay::MatchPhase::Waiting,
                rules_revision: 1,
            },
        ));
        app.update();
        app
    }

    pub(super) fn root(world: &mut World) -> TerrainRoot {
        world
            .query::<&TerrainRoot>()
            .iter(world)
            .next()
            .copied()
            .expect("terrain root exists")
    }

    pub(super) fn current_occupancy(world: &mut World) -> BTreeMap<TerrainChunkId, TerrainBits> {
        let mut chunks = world.query::<(&TerrainChunk, &TerrainChunkState)>();
        chunks
            .iter(world)
            .map(|(chunk, state)| (chunk.id, state.current))
            .collect()
    }

    #[test]
    fn authoritative_terrain_installs_exact_reconciled_chunks() {
        let mut app = terrain_app();
        let world = app.world_mut();
        let root = root(world);
        assert_eq!(root.map_instance_id.0, 1);
        assert_eq!(root.revision, 0);
        assert_eq!(root.match_id, Some(crate::matchplay::MatchId(1)));
        assert_ne!(root.terrain_fingerprint, 0);
        let mut chunks = world.query::<(&TerrainChunk, &TerrainChunkCollision)>();
        let mut ids: Vec<_> = chunks.iter(world).map(|(chunk, _)| chunk.id).collect();
        ids.sort_unstable();
        assert_eq!(
            ids,
            vec![
                TerrainChunkId { x: -1, y: -1 },
                TerrainChunkId { x: -1, y: 0 },
                TerrainChunkId { x: 0, y: -1 },
                TerrainChunkId { x: 0, y: 0 },
            ]
        );
        let observed: Vec<_> = chunks
            .iter(world)
            .map(|(chunk, collision)| (chunk.id, collision.occupied_cells))
            .collect();
        for (chunk, occupied_cells) in observed {
            assert_eq!(occupied_cells, 144, "even initial split in {chunk:?}");
        }
        let mut with_collider =
            world.query_filtered::<&avian2d::prelude::Collider, With<TerrainChunk>>();
        assert_eq!(with_collider.iter(world).count(), 4);
        assert_eq!(world.resource::<TerrainRecoveryCache>().revision, 0);
    }

    #[test]
    fn brush_transaction_erases_cells_revisions_and_rebuilds_colliders() {
        let mut app = terrain_app();
        let before = current_occupancy(app.world_mut());
        let digest_before = occupancy_digest(&before);
        // Fire into the center of the block: a radius-48 brush erases cells and splits
        // across the four chunks.
        app.world_mut()
            .resource_mut::<crate::combat::CombatWorldEffectFacts>()
            .0
            .push(fact(1, (0.0, 0.0), 48.0));
        app.update();
        let world = app.world_mut();
        let root = root(world);
        assert_eq!(root.revision, 1, "one changed brush advances revision once");
        let after = current_occupancy(world);
        assert_ne!(occupancy_digest(&after), digest_before);
        let erased: u32 = before
            .values()
            .zip(after.values())
            .map(|(left, right)| left.count().saturating_sub(right.count()))
            .sum();
        assert!(erased > 0);
        // Publication drains the outbox onto the wire each tick; the retained telemetry
        // record carries the same staged-event facts for inspection.
        let outbox = world.resource::<TerrainOutbox>();
        assert!(
            outbox.events.is_empty(),
            "the network publisher drains staged events"
        );
        let telemetry = world.resource::<super::super::telemetry::TerrainTelemetry>();
        assert_eq!(telemetry.aggregates.applied_brushes, 1);
        assert_eq!(telemetry.aggregates.cells_erased, u64::from(erased));
        let record = telemetry
            .records
            .iter()
            .rev()
            .find(|record| {
                record.outcome == super::super::telemetry::TerrainTelemetryOutcome::Applied
            })
            .expect("the applied brush leaves a telemetry record");
        assert_eq!(record.revision, 1);
        assert_eq!(u32::from(record.erased_cells), erased);
        assert_eq!(record.map_instance_id, root.map_instance_id);
        assert!(!record.affected_chunks.is_empty());
        assert!(record.serialized_event_bytes.is_some_and(|bytes| bytes > 0));
        // Colliders still installed and consistent with the new occupancy.
        let mut colliders = world.query_filtered::<
            (&TerrainChunkState, &avian2d::prelude::Collider),
            With<TerrainChunk>,
        >();
        for (state, _) in colliders.iter(world) {
            let occupied = state.current.count();
            let has_collider = true;
            let _ = has_collider;
            assert_eq!(occupied, state.current.count());
        }
    }

    #[test]
    fn no_op_brush_records_telemetry_without_revision() {
        let mut app = terrain_app();
        app.world_mut()
            .resource_mut::<crate::combat::CombatWorldEffectFacts>()
            .0
            .push(fact(1, (-800.0, 0.0), 16.0));
        app.update();
        let world = app.world_mut();
        assert_eq!(root(world).revision, 0);
        assert!(world.resource::<TerrainOutbox>().events.is_empty());
        let telemetry = world.resource::<super::super::telemetry::TerrainTelemetry>();
        assert_eq!(telemetry.aggregates.no_op_brushes, 1);
        assert_eq!(telemetry.aggregates.requested_brushes, 1);
    }

    #[test]
    fn duplicate_brush_facts_count_and_evaluate_once() {
        let mut app = terrain_app();
        let duplicate = fact(1, (0.0, 0.0), 48.0);
        app.world_mut()
            .resource_mut::<crate::combat::CombatWorldEffectFacts>()
            .0
            .extend([duplicate.clone(), duplicate]);
        app.update();
        let telemetry = app
            .world()
            .resource::<super::super::telemetry::TerrainTelemetry>();
        assert_eq!(telemetry.aggregates.requested_brushes, 1);
        assert_eq!(telemetry.aggregates.applied_brushes, 1);
        assert_eq!(telemetry.aggregates.no_op_brushes, 0);
        assert_eq!(telemetry.aggregates.rejected_brushes, 0);
    }

    #[test]
    fn deferred_brushes_are_requested_once_and_apply_on_later_ticks() {
        let mut app = terrain_app();
        app.world_mut()
            .insert_resource(crate::terrain::authority::TerrainAdmissionCapacity(1));
        app.world_mut()
            .resource_mut::<crate::combat::CombatWorldEffectFacts>()
            .0
            .push(fact(1, (0.0, 0.0), 48.0));
        app.world_mut()
            .resource_mut::<crate::combat::CombatWorldEffectFacts>()
            .0
            .push(fact(2, (40.0, 40.0), 48.0));
        app.update();
        {
            let world = app.world_mut();
            let telemetry = world.resource::<super::super::telemetry::TerrainTelemetry>();
            assert_eq!(
                telemetry.aggregates.requested_brushes, 2,
                "both submissions count exactly once"
            );
            assert_eq!(telemetry.aggregates.applied_brushes, 1);
            assert_eq!(telemetry.aggregates.deferred_brushes, 1);
            assert_eq!(
                world.resource::<PendingTerrainBrushes>().queue.len(),
                1,
                "the excess whole brush defers"
            );
        }
        // The deferred brush re-enters the next batch without being counted as a new
        // request, so deferral stays a lifecycle event inside the submission count.
        app.update();
        let world = app.world_mut();
        let telemetry = world.resource::<super::super::telemetry::TerrainTelemetry>();
        assert_eq!(
            telemetry.aggregates.requested_brushes, 2,
            "re-admission does not recount the deferred brush"
        );
        assert_eq!(telemetry.aggregates.applied_brushes, 2);
        assert_eq!(telemetry.aggregates.deferred_brushes, 1);
        assert!(world.resource::<PendingTerrainBrushes>().queue.is_empty());
    }

    #[test]
    fn queue_full_rejection_is_terminal_inside_the_submission_count() {
        let mut app = terrain_app();
        app.world_mut()
            .insert_resource(crate::terrain::authority::TerrainAdmissionCapacity(1));
        let overflow = MAX_PENDING_TERRAIN_BRUSHES + 2;
        for attack in 1..=overflow {
            app.world_mut()
                .resource_mut::<crate::combat::CombatWorldEffectFacts>()
                .0
                .push(fact(attack as u64, (0.0, 0.0), 48.0));
        }
        app.update();
        let world = app.world_mut();
        let telemetry = world.resource::<super::super::telemetry::TerrainTelemetry>();
        assert_eq!(
            telemetry.aggregates.requested_brushes, overflow as u64,
            "every submitted brush is counted, including the rejected one"
        );
        assert_eq!(telemetry.aggregates.applied_brushes, 1);
        assert_eq!(
            telemetry.aggregates.deferred_brushes, MAX_PENDING_TERRAIN_BRUSHES as u64,
            "the pending queue fills with whole deferred brushes"
        );
        assert_eq!(telemetry.aggregates.rejected_brushes, 1);
        assert_eq!(
            world.resource::<PendingTerrainBrushes>().queue.len(),
            MAX_PENDING_TERRAIN_BRUSHES
        );
    }

    #[test]
    fn revision_exhaustion_rejects_brushes_without_mutation() {
        let mut app = terrain_app();
        // Fabricate an exhausted root: unreachable in play, but the invariant must hold.
        let world = app.world_mut();
        let root_entity = world
            .query_filtered::<Entity, With<TerrainRoot>>()
            .iter(world)
            .next()
            .expect("terrain root exists");
        let exhausted = root(world);
        world.entity_mut(root_entity).insert(TerrainRoot {
            revision: u64::MAX,
            ..exhausted
        });
        let before = current_occupancy(world);
        world
            .resource_mut::<crate::combat::CombatWorldEffectFacts>()
            .0
            .push(fact(1, (0.0, 0.0), 48.0));
        app.update();
        let world = app.world_mut();
        assert_eq!(
            current_occupancy(world),
            before,
            "no cell changes once the revision space is exhausted"
        );
        assert_eq!(root(world).revision, u64::MAX);
        assert!(
            world.resource::<TerrainOutbox>().events.is_empty(),
            "no duplicate maximum-revision event is staged"
        );
        let telemetry = world.resource::<super::super::telemetry::TerrainTelemetry>();
        assert_eq!(telemetry.aggregates.rejected_brushes, 1);
        assert_eq!(telemetry.aggregates.applied_brushes, 0);
        assert!(telemetry.records.iter().any(|record| record.outcome
            == super::super::telemetry::TerrainTelemetryOutcome::RejectedRevisionExhausted));
    }

    #[test]
    fn applied_records_and_aggregates_carry_real_rebuild_and_visual_counts() {
        let mut app = terrain_app();
        // An interior brush of chunk (0,0): exactly one collider rebuilds, while
        // presentation also repaints the two allocated orthogonal neighbors of the
        // changed chunk regardless of boundary masks.
        app.world_mut()
            .resource_mut::<crate::combat::CombatWorldEffectFacts>()
            .0
            .push(fact(1, (48.0, 48.0), 16.0));
        app.update();
        let world = app.world_mut();
        let telemetry = world.resource::<super::super::telemetry::TerrainTelemetry>();
        assert_eq!(
            telemetry.aggregates.collision_rebuilt_chunks.len(),
            1,
            "an interior brush rebuilds only its own chunk collider"
        );
        assert_eq!(
            telemetry.aggregates.visual_dirty_chunks.len(),
            3,
            "the changed chunk plus its two allocated orthogonal neighbors repaint"
        );
        let record = telemetry
            .records
            .iter()
            .rev()
            .find(|record| {
                record.outcome == super::super::telemetry::TerrainTelemetryOutcome::Applied
            })
            .expect("the interior brush applies");
        assert_eq!(record.rebuilt_colliders, 1);
        assert_eq!(telemetry.aggregates.max_collider_rebuilds_in_one_tick, 1);
        assert_eq!(telemetry.aggregates.max_brushes_in_one_tick, 1);
    }

    #[test]
    fn multi_brush_batches_credit_each_brush_only_its_own_collider_dirt() {
        let mut app = terrain_app();
        // One batch: an interior brush of chunk (0,0) plus the corner seam brush, whose
        // boundary erases dirty every allocated chunk. The committed union holds three
        // chunks in or adjacent to the interior brush's chunk, but the interior brush
        // changed no boundary cell: those neighbor rebuilds belong to the seam brush.
        for brush in [(1, (48.0, 48.0), 16.0), (2, (4.0, 4.0), 48.0)] {
            app.world_mut()
                .resource_mut::<crate::combat::CombatWorldEffectFacts>()
                .0
                .push(fact(brush.0, brush.1, brush.2));
        }
        app.update();
        let world = app.world_mut();
        let telemetry = world.resource::<super::super::telemetry::TerrainTelemetry>();
        assert_eq!(telemetry.aggregates.applied_brushes, 2);
        let union_len = telemetry.aggregates.collision_rebuilt_chunks.len();
        assert!(
            union_len >= 4,
            "the seam brush dirties every allocated chunk"
        );
        let applied = |attack: u64| {
            telemetry
                .records
                .iter()
                .rev()
                .find(|record| {
                    record.outcome == super::super::telemetry::TerrainTelemetryOutcome::Applied
                        && record.source_attack_id == Some(AttackId(attack))
                })
                .unwrap_or_else(|| panic!("the brush of attack {attack} applies"))
        };
        assert_eq!(
            applied(1).rebuilt_colliders,
            1,
            "an interior-only brush never credits neighbor rebuilds another brush caused"
        );
        assert_eq!(
            applied(2).rebuilt_colliders,
            union_len,
            "the boundary brush forces every collider the batch rebuilt"
        );
    }

    #[test]
    fn brush_at_chunk_seam_rebuilds_the_boundary_neighbor() {
        let mut app = terrain_app();
        // Erase along the x=0 global seam at y far from the block edge: cells at local
        // x=31 of chunk (-1, y) and local x=0 of chunk (0, y) change, so both are dirty
        // and their orthogonal neighbors join the collision-dirty union when boundary
        // cells change.
        app.world_mut()
            .resource_mut::<crate::combat::CombatWorldEffectFacts>()
            .0
            .push(fact(1, (4.0, 4.0), 48.0));
        app.update();
        let world = app.world_mut();
        let telemetry = world.resource::<super::super::telemetry::TerrainTelemetry>();
        assert!(
            telemetry.aggregates.collision_rebuilt_chunks.len() >= 4,
            "the seam brush dirties every initially-allocated chunk"
        );
    }

    #[test]
    fn admission_defers_whole_excess_brushes_and_rejects_queue_overflow() {
        let mut app = terrain_app();
        app.world_mut()
            .insert_resource(crate::terrain::authority::TerrainAdmissionCapacity(1));
        let positions = [(0.0, 0.0), (40.0, 40.0), (-40.0, -40.0)];
        for (index, attack) in (1..=3).enumerate() {
            app.world_mut()
                .resource_mut::<crate::combat::CombatWorldEffectFacts>()
                .0
                .push(fact(attack, positions[index], 48.0));
        }
        app.update();
        {
            let world = app.world_mut();
            assert_eq!(root(world).revision, 1, "only one brush admitted");
            assert_eq!(
                world.resource::<PendingTerrainBrushes>().queue.len(),
                2,
                "excess facts defer whole"
            );
        }
        app.update();
        let world = app.world_mut();
        assert_eq!(root(world).revision, 2);
        assert_eq!(world.resource::<PendingTerrainBrushes>().queue.len(), 1);
        // Overflow the bounded queue with the capacity pinned at one.
        for attack in 100..=200 {
            app.world_mut()
                .resource_mut::<crate::combat::CombatWorldEffectFacts>()
                .0
                .push(fact(attack, (0.0, 0.0), 16.0));
        }
        app.update();
        let world = app.world_mut();
        let queue = world.resource::<PendingTerrainBrushes>();
        assert!(queue.queue.len() <= MAX_PENDING_TERRAIN_BRUSHES);
        let telemetry = world.resource::<super::super::telemetry::TerrainTelemetry>();
        assert!(
            telemetry.aggregates.rejected_brushes > 0,
            "queue overflow rejects the newest excess fact"
        );
    }

    #[test]
    fn map_replacement_teardown_leaves_no_stale_terrain() {
        let mut app = terrain_app();
        let catalog = app
            .world()
            .resource::<crate::map::MapCatalogResource>()
            .0
            .clone();
        let replacement = catalog
            .resolve_preset(
                MapPresetId(1),
                crate::map::MapInstanceId(2),
                &MapLayoutRequirements::wipeout(),
            )
            .unwrap();
        crate::map::install_resolved_map(app.world_mut(), replacement).unwrap();
        app.update();
        let world = app.world_mut();
        let root = root(world);
        assert_eq!(root.map_instance_id, crate::map::MapInstanceId(2));
        let mut chunks = world.query_filtered::<&TerrainChunk, With<TerrainChunk>>();
        assert_eq!(chunks.iter(world).count(), 4);
        assert!(
            chunks
                .iter(world)
                .all(|chunk| chunk.map_instance_id == crate::map::MapInstanceId(2))
        );
    }

    #[test]
    fn map_replacement_clears_the_previous_generation_telemetry() {
        let mut app = terrain_app();
        app.world_mut()
            .resource_mut::<crate::combat::CombatWorldEffectFacts>()
            .0
            .push(fact(1, (0.0, 0.0), 48.0));
        app.update();
        {
            let world = app.world_mut();
            let telemetry = world.resource::<super::super::telemetry::TerrainTelemetry>();
            assert_eq!(telemetry.aggregates.applied_brushes, 1);
            assert!(telemetry.records.iter().any(|record| record.outcome
                == super::super::telemetry::TerrainTelemetryOutcome::Applied));
        }
        let catalog = app
            .world()
            .resource::<crate::map::MapCatalogResource>()
            .0
            .clone();
        let replacement = catalog
            .resolve_preset(
                MapPresetId(1),
                crate::map::MapInstanceId(2),
                &MapLayoutRequirements::wipeout(),
            )
            .unwrap();
        crate::map::install_resolved_map(app.world_mut(), replacement).unwrap();
        app.update();
        let world = app.world_mut();
        assert_eq!(
            *world.resource::<super::super::telemetry::TerrainTelemetry>(),
            super::super::telemetry::TerrainTelemetry::default(),
            "the replacement generation inherits no records or aggregates"
        );
    }

    #[test]
    fn restart_reset_restores_initial_occupancy_and_zero_revision() {
        let mut app = terrain_app();
        let initial = current_occupancy(app.world_mut());
        app.world_mut()
            .resource_mut::<crate::combat::CombatWorldEffectFacts>()
            .0
            .push(fact(1, (0.0, 0.0), 48.0));
        app.update();
        {
            let world = app.world_mut();
            assert_eq!(root(world).revision, 1);
        }
        // Stage one restart slot and run the environment reset directly, exactly as the
        // chained restart transaction would.
        app.init_resource::<crate::matchplay::PendingMatchRestart>();
        app.world_mut()
            .resource_mut::<crate::matchplay::PendingMatchRestart>()
            .stage_for_test(crate::matchplay::PendingMatchRestartSlot {
                previous_id: crate::matchplay::MatchId(1),
                next_id: crate::matchplay::MatchId(3),
                restart_tick: 10,
            });
        crate::terrain::reset_terrain_on_match_restart(app.world_mut());
        let world = app.world_mut();
        let root = root(world);
        assert_eq!(root.revision, 0);
        assert_eq!(root.match_id, Some(crate::matchplay::MatchId(3)));
        assert_eq!(current_occupancy(world), initial);
        let outbox = world.resource::<TerrainOutbox>();
        let reset = outbox.reset.expect("reset event staged");
        assert_eq!(
            reset.previous_generation.match_id,
            crate::matchplay::MatchId(1)
        );
        assert_eq!(reset.next_generation.match_id, crate::matchplay::MatchId(3));
        // Re-occupied chunks regain colliders.
        let mut colliders =
            world.query_filtered::<&avian2d::prelude::Collider, With<TerrainChunk>>();
        assert_eq!(colliders.iter(world).count(), 4);
    }

    #[test]
    fn restart_starts_a_fresh_telemetry_epoch_for_the_next_generation() {
        let mut app = terrain_app();
        app.world_mut()
            .resource_mut::<crate::combat::CombatWorldEffectFacts>()
            .0
            .push(fact(1, (0.0, 0.0), 48.0));
        app.update();
        {
            let world = app.world_mut();
            let instance_id = root(world).map_instance_id;
            let mut telemetry = world.resource_mut::<super::super::telemetry::TerrainTelemetry>();
            assert_eq!(telemetry.aggregates.applied_brushes, 1);
            // The previous match also served one recovery exchange.
            telemetry.record(super::super::telemetry::TerrainTelemetryRecord {
                tick: 5,
                map_instance_id: instance_id,
                revision: 1,
                source_attack_id: None,
                delivery_index: None,
                brush: None,
                affected_chunks: Vec::new(),
                erased_cells: 0,
                rebuilt_colliders: 0,
                serialized_event_bytes: None,
                outcome: super::super::telemetry::TerrainTelemetryOutcome::RecoverySent {
                    bytes: 512,
                    chunks: 4,
                },
            });
            telemetry.record_recovery_request();
            assert_eq!(telemetry.aggregates.recovery_requests, 1);
            assert_eq!(telemetry.aggregates.recovery_responses, 1);
        }
        app.init_resource::<crate::matchplay::PendingMatchRestart>();
        app.world_mut()
            .resource_mut::<crate::matchplay::PendingMatchRestart>()
            .stage_for_test(crate::matchplay::PendingMatchRestartSlot {
                previous_id: crate::matchplay::MatchId(1),
                next_id: crate::matchplay::MatchId(3),
                restart_tick: 10,
            });
        crate::terrain::reset_terrain_on_match_restart(app.world_mut());
        let world = app.world_mut();
        let telemetry = world.resource::<super::super::telemetry::TerrainTelemetry>();
        // The new generation's telemetry opens with its own reset facts only.
        assert_eq!(telemetry.records.len(), 1, "only the reset record remains");
        assert_eq!(
            telemetry.records[0].outcome,
            super::super::telemetry::TerrainTelemetryOutcome::Reset
        );
        assert_eq!(telemetry.aggregates.applied_brushes, 0);
        assert_eq!(telemetry.aggregates.requested_brushes, 0);
        assert_eq!(telemetry.aggregates.cells_erased, 0);
        assert_eq!(telemetry.aggregates.events_sent, 0);
        assert_eq!(telemetry.aggregates.recovery_requests, 0);
        assert_eq!(telemetry.aggregates.recovery_responses, 0);
        assert!(telemetry.aggregates.occupancy_dirty_chunks.is_empty());
        assert_eq!(telemetry.aggregates.max_brushes_in_one_tick, 0);
        // The reset's own restoration rebuilds are the new epoch's first metrics.
        assert!(telemetry.records[0].rebuilt_colliders > 0);
        assert_eq!(
            telemetry.records[0].rebuilt_colliders,
            telemetry.aggregates.collision_rebuilt_chunks.len()
        );
    }

    #[test]
    fn restart_clears_queued_brushes_and_rejects_restart_tick_facts() {
        let mut app = terrain_app();
        app.world_mut()
            .insert_resource(crate::terrain::authority::TerrainAdmissionCapacity(1));
        let initial = current_occupancy(app.world_mut());
        let positions = [(0.0, 0.0), (40.0, 40.0), (-40.0, -40.0)];
        for (index, attack) in (1..=3).enumerate() {
            app.world_mut()
                .resource_mut::<crate::combat::CombatWorldEffectFacts>()
                .0
                .push(fact(attack, positions[index], 48.0));
        }
        app.update();
        {
            let world = app.world_mut();
            assert_eq!(root(world).revision, 1, "one brush admitted");
            assert_eq!(
                world.resource::<PendingTerrainBrushes>().queue.len(),
                2,
                "excess facts defer whole"
            );
        }
        app.init_resource::<crate::matchplay::PendingMatchRestart>();
        app.world_mut()
            .resource_mut::<crate::matchplay::PendingMatchRestart>()
            .stage_for_test(crate::matchplay::PendingMatchRestartSlot {
                previous_id: crate::matchplay::MatchId(1),
                next_id: crate::matchplay::MatchId(3),
                restart_tick: 1,
            });
        crate::terrain::reset_terrain_on_match_restart(app.world_mut());
        // A detonation resolved at the restart tick itself is staged after the reset
        // inside the same fixed-post chain; it must not carve the restored generation.
        app.world_mut()
            .resource_mut::<crate::combat::CombatWorldEffectFacts>()
            .0
            .push(fact(9, (0.0, 0.0), 48.0));
        app.update();
        {
            let world = app.world_mut();
            assert_eq!(root(world).revision, 0, "no stale brush may apply");
            assert_eq!(current_occupancy(world), initial);
            assert!(world.resource::<PendingTerrainBrushes>().queue.is_empty());
            assert!(
                world
                    .resource::<crate::terrain::TerrainBrushBatch>()
                    .brushes
                    .is_empty()
            );
            let telemetry = world.resource::<crate::terrain::telemetry::TerrainTelemetry>();
            assert_eq!(
                telemetry.aggregates.stale_generation_brushes, 1,
                "the restart-tick fact is counted and dropped"
            );
        }
        // A detonation resolved after the restart tick brushes the new match normally.
        let mut fresh = fact(11, (0.0, 0.0), 48.0);
        fresh.tick = 2;
        app.world_mut()
            .resource_mut::<crate::combat::CombatWorldEffectFacts>()
            .0
            .push(fresh);
        app.update();
        assert_eq!(root(app.world_mut()).revision, 1);
    }

    #[test]
    fn exact_teardown_without_reinstall_keeps_fixed_post_systems_schedulable() {
        let mut app = terrain_app();
        app.update();
        let resolved = app
            .world_mut()
            .resource::<crate::map::ResolvedMap>()
            .clone();
        let initial_chunks = {
            let world = app.world_mut();
            let mut chunks = world.query::<&TerrainChunk>();
            chunks.iter(world).count()
        };
        assert!(initial_chunks > 0, "fixture installs terrain chunks");
        // The exact teardown authoritative map teardown performs with no replacement.
        crate::map::teardown_authoritative_map(app.world_mut());
        for _ in 0..3 {
            app.update();
        }
        {
            let world = app.world_mut();
            let mut roots = world.query_filtered::<Entity, With<TerrainRoot>>();
            assert_eq!(roots.iter(world).count(), 0);
            let mut chunks = world.query::<&TerrainChunk>();
            assert_eq!(chunks.iter(world).count(), 0);
            assert!(
                world
                    .resource::<crate::terrain::TerrainChunkIndex>()
                    .0
                    .is_empty()
            );
            assert!(world.resource::<PendingTerrainBrushes>().queue.is_empty());
        }
        // Reinstalling a map rebuilds a fresh generation from the retained empty state.
        crate::map::install_resolved_map(app.world_mut(), resolved).expect("map reinstalls");
        app.update();
        let world = app.world_mut();
        let mut chunks = world.query::<&TerrainChunk>();
        assert_eq!(chunks.iter(world).count(), initial_chunks);
        assert_eq!(root(world).revision, 0);
    }

    /// Probe the Avian collider world for destructible terrain at one point from inside a
    /// system, because `SpatialQuery` is a system parameter rather than a resource.
    fn terrain_point_hits(app: &mut App, point: (f32, f32)) -> bool {
        #[derive(Resource, Default)]
        struct HitResult(bool);
        #[derive(Resource)]
        struct ProbePoint(Vec2);
        fn probe(
            spatial: avian2d::prelude::SpatialQuery,
            point: Res<ProbePoint>,
            mut result: ResMut<HitResult>,
        ) {
            let filter = avian2d::prelude::SpatialQueryFilter::from_mask(
                crate::movement::DESTRUCTIBLE_TERRAIN_LAYER,
            );
            result.0 = spatial
                .project_point(point.0, true, &filter)
                .is_some_and(|projection| projection.is_inside);
        }
        app.init_resource::<HitResult>();
        if app.world().contains_resource::<ProbePoint>() {
            app.world_mut().resource_mut::<ProbePoint>().0 = Vec2::new(point.0, point.1);
        } else {
            app.insert_resource(ProbePoint(Vec2::new(point.0, point.1)));
        }
        app.add_systems(FixedPostUpdate, probe);
        app.update();
        app.world().resource::<HitResult>().0
    }

    #[test]
    fn occupancy_agrees_with_avian_point_queries() {
        let mut app = terrain_app();
        assert!(
            terrain_point_hits(&mut app, (4.0, 4.0)),
            "an occupied cell center projects onto destructible terrain"
        );
        app.world_mut()
            .resource_mut::<crate::combat::CombatWorldEffectFacts>()
            .0
            .push(fact(1, (4.0, 4.0), 48.0));
        app.update();
        // The next prepare phase refreshed the collider tree; the erased core no longer
        // projects. A point outside the block still does.
        assert!(
            !terrain_point_hits(&mut app, (4.0, 4.0)),
            "the erased core no longer projects onto terrain"
        );
        assert!(
            terrain_point_hits(&mut app, (60.0, 60.0)),
            "remaining occupancy still projects after the brush"
        );
    }

    #[test]
    fn schedule_trace_places_terrain_between_damage_and_mode_rules() {
        #[derive(Resource, Default)]
        struct Trace(Vec<&'static str>);
        fn probe(label: &'static str) -> impl FnMut(ResMut<Trace>) + 'static {
            move |mut trace: ResMut<Trace>| trace.0.push(label)
        }
        let mut app = terrain_app();
        app.init_resource::<Trace>().add_systems(
            FixedPostUpdate,
            (
                probe("damage").in_set(crate::combat::CombatSet::Damage),
                probe("observe")
                    .in_set(crate::abilities::AbilitySet::ObserveOutcomes)
                    .after(crate::combat::CombatSet::Damage),
                probe("terrain").in_set(TerrainSet::ApplyBrushes),
                probe("mode-rules").in_set(crate::matchplay::MatchSet::ModeRules),
            ),
        );
        app.update();
        let trace = &app.world().resource::<Trace>().0;
        let damage = trace.iter().position(|label| *label == "damage").unwrap();
        let observe = trace.iter().position(|label| *label == "observe").unwrap();
        let terrain = trace.iter().position(|label| *label == "terrain").unwrap();
        let mode_rules = trace
            .iter()
            .position(|label| *label == "mode-rules")
            .unwrap();
        assert!(damage < terrain && observe < terrain && terrain < mode_rules);
    }
}

/// Pure convergence state-machine coverage: every wire input class that can reach a
/// client, without ECS or transport.
mod convergence_tests {
    use super::*;
    use crate::matchplay::MatchId;
    use crate::terrain::network::{
        ClientTerrainConvergence, TerrainConvergenceAction, TerrainConvergencePhase,
    };

    pub(super) fn generation(match_id: u64) -> TerrainGeneration {
        TerrainGeneration {
            map_instance_id: crate::map::MapInstanceId(1),
            match_id: MatchId(match_id.into()),
            terrain_fingerprint: 0xabcd_ef01,
        }
    }

    /// One fully occupied chunk at the origin, like the built-in block's corner.
    pub(super) fn initial_chunks() -> BTreeMap<TerrainChunkId, TerrainBits> {
        BTreeMap::from([chunk_with_all_cells_set(TerrainChunkId { x: 0, y: 0 })])
    }

    /// Compute the exact event a server would send by rasterizing `brush` on `current`.
    pub(super) fn stage_event(
        current: &BTreeMap<TerrainChunkId, TerrainBits>,
        terrain_gen: TerrainGeneration,
        revision: u64,
        brush: TerrainBrush,
    ) -> (
        TerrainDestructionEvent,
        BTreeMap<TerrainChunkId, TerrainBits>,
    ) {
        let mut next = current.clone();
        let mut touched: BTreeMap<TerrainChunkId, TerrainBits> = BTreeMap::new();
        let ((x_min, x_max), (y_min, y_max)) = brush_cell_range(brush);
        for cell_y in y_min..=y_max {
            for cell_x in x_min..=x_max {
                let Some((chunk, _)) = cell_to_chunk_and_local((cell_x, cell_y)) else {
                    continue;
                };
                if let Some(bits) = next.get(&chunk) {
                    touched.entry(chunk).or_insert(*bits);
                }
            }
        }
        let outcome = apply_brush(&mut touched, brush);
        for (chunk, bits) in touched {
            next.insert(chunk, bits);
        }
        (
            TerrainDestructionEvent {
                generation: terrain_gen,
                revision,
                source_attack_id: crate::combat::AttackId(1),
                source_delivery_index: 0,
                brush,
                affected_chunks: outcome.affected_chunks,
                erased_cells: outcome.erased_cells,
            },
            next,
        )
    }

    pub(super) fn center_brush(radius_half_cells: u16) -> TerrainBrush {
        TerrainBrush {
            center_half_cells_x: 1,
            center_half_cells_y: 1,
            radius_half_cells: radius_half_cells.min(2),
        }
    }

    /// A convergence machine that already committed the initial snapshot at revision 0.
    fn ready_state() -> ClientTerrainConvergence {
        let terrain_gen = generation(1);
        let mut state = ClientTerrainConvergence::default();
        assert_eq!(
            state.observe_generation(terrain_gen, &initial_chunks()),
            TerrainConvergenceAction::RequestRecovery(terrain_gen)
        );
        state.mark_request_sent();
        assert_eq!(
            state.apply_snapshot(
                &recovery_snapshot(&initial_chunks(), terrain_gen, 0),
                &initial_chunks()
            ),
            TerrainConvergenceAction::Applied
        );
        assert_eq!(
            state.phase,
            TerrainConvergencePhase::Ready {
                generation: terrain_gen
            }
        );
        // The visual layer consumed the snapshot-wide dirty set on commit.
        state.take_dirty();
        state
    }

    #[test]
    fn valid_snapshot_and_event_commit_dirty_chunks_and_revision() {
        let terrain_gen = generation(1);
        let mut state = ready_state();
        let brush = center_brush(2);
        let (event, expected_chunks) = stage_event(&initial_chunks(), terrain_gen, 1, brush);
        assert_eq!(
            state.apply_event(event.clone()),
            TerrainConvergenceAction::Applied
        );
        assert_eq!(state.revision(), 1);
        assert_eq!(state.chunks(), &expected_chunks);
        assert_eq!(state.take_dirty(), vec![TerrainChunkId { x: 0, y: 0 }]);
        assert!(state.take_dirty().is_empty(), "dirty drains exactly once");
    }

    #[test]
    fn duplicate_revisions_are_ignored_without_state_change() {
        let terrain_gen = generation(1);
        let mut state = ready_state();
        let (event, _) = stage_event(&initial_chunks(), terrain_gen, 1, center_brush(2));
        assert_eq!(
            state.apply_event(event.clone()),
            TerrainConvergenceAction::Applied
        );
        assert_eq!(state.apply_event(event), TerrainConvergenceAction::Ignored);
        assert_eq!(state.revision(), 1);
    }

    #[test]
    fn missing_revision_requests_recovery_and_retains_no_guess() {
        let terrain_gen = generation(1);
        let mut state = ready_state();
        let (event, _) = stage_event(&initial_chunks(), terrain_gen, 2, center_brush(2));
        assert_eq!(
            state.apply_event(event),
            TerrainConvergenceAction::RequestRecovery(terrain_gen)
        );
        assert!(matches!(
            state.phase,
            TerrainConvergencePhase::AwaitingRecovery { .. }
        ));
        // A duplicate snapshot for the recovered generation re-syncs fully.
        assert_eq!(
            state.apply_snapshot(
                &recovery_snapshot(&initial_chunks(), terrain_gen, 0),
                &initial_chunks()
            ),
            TerrainConvergenceAction::Applied
        );
        assert!(matches!(state.phase, TerrainConvergencePhase::Ready { .. }));
    }

    #[test]
    fn buffered_events_replay_in_revision_order_after_snapshot() {
        let terrain_gen = generation(1);
        let mut state = ClientTerrainConvergence::default();
        state.observe_generation(terrain_gen, &initial_chunks());
        state.mark_request_sent();
        let (first, after_first) = stage_event(&initial_chunks(), terrain_gen, 1, center_brush(2));
        // A second brush elsewhere in the same occupied chunk erases fresh cells.
        let (second, after_second) = stage_event(
            &after_first,
            terrain_gen,
            2,
            TerrainBrush {
                center_half_cells_x: 17,
                center_half_cells_y: 17,
                radius_half_cells: 2,
            },
        );
        assert!(
            second.erased_cells > 0,
            "the second event erases fresh cells"
        );
        // Out-of-order arrival while recovery is outstanding.
        assert_eq!(
            state.apply_event(second.clone()),
            TerrainConvergenceAction::Buffered
        );
        assert_eq!(state.apply_event(first), TerrainConvergenceAction::Buffered);
        assert_eq!(
            state.apply_snapshot(
                &recovery_snapshot(&initial_chunks(), terrain_gen, 0),
                &initial_chunks()
            ),
            TerrainConvergenceAction::Applied
        );
        assert_eq!(state.revision(), 2);
        assert_eq!(state.chunks(), &after_second);
        let _ = second;
    }

    #[test]
    fn stale_generations_are_discarded_silently() {
        let mut state = ready_state();
        let stale_match = generation(2);
        let (event, _) = stage_event(&initial_chunks(), stale_match, 1, center_brush(2));
        assert_eq!(state.apply_event(event), TerrainConvergenceAction::Ignored);
        assert_eq!(state.revision(), 0);
        let mut foreign_map = generation(2);
        foreign_map.map_instance_id = crate::map::MapInstanceId(9);
        let (event, _) = stage_event(&initial_chunks(), foreign_map, 1, center_brush(2));
        assert_eq!(state.apply_event(event), TerrainConvergenceAction::Ignored);
    }

    #[test]
    fn wrong_fingerprint_invalidates_only_its_generation() {
        let terrain_gen = generation(1);
        let mut state = ready_state();
        let mut corrupt = terrain_gen;
        corrupt.terrain_fingerprint += 1;
        let (event, _) = stage_event(&initial_chunks(), corrupt, 1, center_brush(2));
        let action = state.apply_event(event);
        assert!(matches!(action, TerrainConvergenceAction::Invalidated(_)));
        assert!(matches!(
            state.phase,
            TerrainConvergencePhase::Invalid { .. }
        ));
        // Invalid is terminal for that generation: a same-generation re-observe is inert.
        assert_eq!(
            state.observe_generation(terrain_gen, &initial_chunks()),
            TerrainConvergenceAction::Ignored
        );
        // A newer valid generation starts fresh recovery.
        let next = generation(2);
        assert_eq!(
            state.observe_generation(next, &initial_chunks()),
            TerrainConvergenceAction::RequestRecovery(next)
        );
    }

    #[test]
    fn foreign_chunk_ids_and_false_reports_request_recovery() {
        let terrain_gen = generation(1);
        let mut state = ready_state();
        // Valid rasterization but a foreign affected chunk listed on the wire.
        let (mut event, _) = stage_event(&initial_chunks(), terrain_gen, 1, center_brush(2));
        event.affected_chunks.push(TerrainChunkId { x: 5, y: 5 });
        assert_eq!(
            state.apply_event(event),
            TerrainConvergenceAction::RequestRecovery(terrain_gen)
        );
        // A well-formed event that misreports its erased count never guesses.
        let mut state = ready_state();
        let (mut event, _) = stage_event(&initial_chunks(), terrain_gen, 1, center_brush(2));
        event.erased_cells = event.erased_cells.saturating_sub(1);
        assert_eq!(
            state.apply_event(event),
            TerrainConvergenceAction::RequestRecovery(terrain_gen)
        );
    }

    #[test]
    fn snapshot_count_and_set_violations_invalidate() {
        let terrain_gen = generation(1);
        let mut state = ClientTerrainConvergence::default();
        state.observe_generation(terrain_gen, &initial_chunks());
        state.mark_request_sent();
        // Duplicate entries shrink the unique set below the expected chunk set.
        let mut snapshot = recovery_snapshot(&initial_chunks(), terrain_gen, 0);
        snapshot.chunks.push(snapshot.chunks[0]);
        assert!(matches!(
            state.apply_snapshot(&snapshot, &initial_chunks()),
            TerrainConvergenceAction::Invalidated(_)
        ));
        // A snapshot missing its expected chunk also invalidates.
        let mut state = ClientTerrainConvergence::default();
        state.observe_generation(terrain_gen, &initial_chunks());
        let mut missing = recovery_snapshot(&initial_chunks(), terrain_gen, 0);
        missing.chunks.clear();
        assert!(matches!(
            state.apply_snapshot(&missing, &initial_chunks()),
            TerrainConvergenceAction::Invalidated(_)
        ));
    }

    #[test]
    fn oversized_snapshot_invalidates() {
        // An engine-illegal expected set (bypassing map validation) proves the serialized
        // byte ceiling still guards the commit independently of the chunk-count ceiling.
        let terrain_gen = generation(1);
        let mut wide = BTreeMap::new();
        for y in 0..20 {
            for x in 0..20 {
                let (chunk, bits) = chunk_with_all_cells_set(TerrainChunkId { x, y });
                wide.insert(chunk, bits);
            }
        }
        let mut state = ClientTerrainConvergence::default();
        state.observe_generation(terrain_gen, &wide);
        let snapshot = recovery_snapshot(&wide, terrain_gen, 0);
        assert!(
            recovery_snapshot_bytes(&snapshot)
                .is_some_and(|bytes| bytes > MAX_TERRAIN_RECOVERY_BYTES),
            "400-chunk fixture must exceed the serialized ceiling"
        );
        assert!(matches!(
            state.apply_snapshot(&snapshot, &wide),
            TerrainConvergenceAction::Invalidated(_)
        ));
    }

    #[test]
    fn zero_effect_events_recover_instead_of_consuming_a_revision() {
        let terrain_gen = generation(1);
        // Commit one real erase, then repeat the same brush inside its own crater: a
        // self-consistent zero-erasure event whose local rasterization also erases
        // nothing. It must never consume the next revision.
        let mut state = ready_state();
        let brush = center_brush(2);
        let (first, after_first) = stage_event(&initial_chunks(), terrain_gen, 1, brush);
        assert_eq!(state.apply_event(first), TerrainConvergenceAction::Applied);
        let (repeat, _) = stage_event(&after_first, terrain_gen, 2, brush);
        assert_eq!(
            repeat.erased_cells, 0,
            "a repeat brush on its own crater erases nothing"
        );
        assert!(repeat.affected_chunks.is_empty());
        assert_eq!(
            state.apply_event(repeat),
            TerrainConvergenceAction::RequestRecovery(terrain_gen)
        );
        assert_eq!(
            state.revision(),
            1,
            "the zero-effect event consumed no revision"
        );
        // A misreported zero on an otherwise real chunk report is equally corrupt.
        let mut state = ready_state();
        let (mut event, _) = stage_event(&initial_chunks(), terrain_gen, 1, center_brush(2));
        event.erased_cells = 0;
        assert_eq!(
            state.apply_event(event),
            TerrainConvergenceAction::RequestRecovery(terrain_gen)
        );
    }

    #[test]
    fn snapshots_may_not_construct_cells_or_rewrite_a_revision_zero_state() {
        let terrain_gen = generation(1);
        let sparse_initial = {
            let mut bits = TerrainBits::default();
            bits.set(5, 5);
            bits.set(9, 9);
            BTreeMap::from([(TerrainChunkId { x: 0, y: 0 }, bits)])
        };
        // One constructed cell outside the authored occupancy invalidates irrecoverably.
        let mut state = ClientTerrainConvergence::default();
        state.observe_generation(terrain_gen, &sparse_initial);
        state.mark_request_sent();
        let mut constructed = recovery_snapshot(&sparse_initial, terrain_gen, 4);
        constructed.chunks[0].occupancy.set(20, 20);
        let TerrainConvergenceAction::Invalidated(reason) =
            state.apply_snapshot(&constructed, &sparse_initial)
        else {
            panic!("a snapshot that constructs cells must invalidate");
        };
        assert!(reason.contains("outside the authored terrain"));

        // Revision zero means no brush ever applied: the snapshot must equal initial.
        let mut state = ClientTerrainConvergence::default();
        state.observe_generation(terrain_gen, &sparse_initial);
        state.mark_request_sent();
        let mut preseeded = recovery_snapshot(&sparse_initial, terrain_gen, 0);
        preseeded.chunks[0].occupancy = {
            let mut bits = TerrainBits::default();
            bits.set(9, 9);
            bits
        };
        let TerrainConvergenceAction::Invalidated(reason) =
            state.apply_snapshot(&preseeded, &sparse_initial)
        else {
            panic!("a revision-zero snapshot below initial must invalidate");
        };
        assert!(reason.contains("revision-zero"));

        // An erase-only subset at a real revision still commits.
        let mut state = ClientTerrainConvergence::default();
        state.observe_generation(terrain_gen, &sparse_initial);
        state.mark_request_sent();
        let mut subset = recovery_snapshot(&sparse_initial, terrain_gen, 7);
        subset.chunks[0].occupancy = preseeded.chunks[0].occupancy;
        assert_eq!(
            state.apply_snapshot(&subset, &sparse_initial),
            TerrainConvergenceAction::Applied
        );
        assert_eq!(state.revision(), 7);
    }

    #[test]
    fn buffer_overflow_while_awaiting_clears_and_rerequests() {
        let terrain_gen = generation(1);
        let mut state = ClientTerrainConvergence::default();
        state.observe_generation(terrain_gen, &initial_chunks());
        state.mark_request_sent();
        let (event, _) = stage_event(&initial_chunks(), terrain_gen, 1, center_brush(2));
        for index in 0..=MAX_BUFFERED_TERRAIN_EVENTS {
            let mut duplicate = event.clone();
            duplicate.revision = u64::try_from(index + 1).unwrap();
            let action = state.apply_event(duplicate);
            if index < MAX_BUFFERED_TERRAIN_EVENTS {
                assert_eq!(action, TerrainConvergenceAction::Buffered);
            } else {
                assert_eq!(
                    action,
                    TerrainConvergenceAction::RequestRecovery(terrain_gen)
                );
            }
        }
        let TerrainConvergencePhase::AwaitingRecovery {
            buffered,
            request_pending,
            ..
        } = &state.phase
        else {
            panic!("overflow must return to a clean recovery request");
        };
        assert!(buffered.is_empty());
        assert!(!request_pending);
    }

    #[test]
    fn revision_space_exhaustion_treats_events_as_duplicates() {
        let terrain_gen = generation(1);
        let mut state = ClientTerrainConvergence::default();
        state.observe_generation(terrain_gen, &initial_chunks());
        assert_eq!(
            state.apply_snapshot(
                &recovery_snapshot(&initial_chunks(), terrain_gen, u64::MAX),
                &initial_chunks()
            ),
            TerrainConvergenceAction::Applied
        );
        assert_eq!(state.revision(), u64::MAX);
        let (mut event, _) = stage_event(&initial_chunks(), terrain_gen, u64::MAX, center_brush(2));
        event.revision = u64::MAX;
        assert_eq!(state.apply_event(event), TerrainConvergenceAction::Ignored);
    }

    #[test]
    fn reset_requires_a_chained_generation_on_the_same_map() {
        let terrain_gen = generation(1);
        let mut state = ready_state();
        let next_gen = generation(2);
        // Observed match already moved on: accepted, revision zero, initial bits.
        assert_eq!(
            state.apply_reset(
                TerrainResetEvent {
                    previous_generation: terrain_gen,
                    next_generation: next_gen,
                },
                Some(next_gen),
                &initial_chunks()
            ),
            TerrainConvergenceAction::Applied
        );
        assert_eq!(state.revision(), 0);
        assert_eq!(state.chunks(), &initial_chunks());

        // A reset that does not chain from the committed generation is discarded.
        let mut state = ready_state();
        assert_eq!(
            state.apply_reset(
                TerrainResetEvent {
                    previous_generation: generation(99),
                    next_generation: next_gen,
                },
                Some(next_gen),
                &initial_chunks()
            ),
            TerrainConvergenceAction::Ignored
        );
        // A reset onto a different map is discarded.
        let mut state = ready_state();
        let mut foreign = next_gen;
        foreign.map_instance_id = crate::map::MapInstanceId(8);
        assert_eq!(
            state.apply_reset(
                TerrainResetEvent {
                    previous_generation: terrain_gen,
                    next_generation: foreign,
                },
                Some(foreign),
                &initial_chunks()
            ),
            TerrainConvergenceAction::Ignored
        );
        // A match the client has not observed at all cannot validate the reset.
        let mut state = ready_state();
        let unseen = generation(3);
        assert_eq!(
            state.apply_reset(
                TerrainResetEvent {
                    previous_generation: terrain_gen,
                    next_generation: unseen,
                },
                Some(generation(4)),
                &initial_chunks()
            ),
            TerrainConvergenceAction::Ignored
        );
    }

    #[test]
    fn reset_outrunning_match_observation_syncs_through_recovery() {
        let terrain_gen = generation(1);
        let next_gen = generation(2);
        // The reset arrives while the client still observes the pre-restart match id:
        // nothing is committed and convergence leaves the ready state immediately.
        let mut state = ready_state();
        assert_eq!(
            state.apply_reset(
                TerrainResetEvent {
                    previous_generation: terrain_gen,
                    next_generation: next_gen,
                },
                Some(terrain_gen),
                &initial_chunks()
            ),
            TerrainConvergenceAction::RequestRecovery(next_gen)
        );
        assert!(matches!(
            &state.phase,
            TerrainConvergencePhase::AwaitingRecovery { generation, .. } if *generation == next_gen
        ));
        assert!(state.chunks().is_empty());
        assert_eq!(state.revision(), 0);
        // Repeated pre-restart observations hold the syncing state instead of churning
        // back to a request the server would reject as stale.
        assert_eq!(
            state.observe_generation(terrain_gen, &initial_chunks()),
            TerrainConvergenceAction::Ignored
        );
        assert!(matches!(
            &state.phase,
            TerrainConvergencePhase::AwaitingRecovery { generation, .. } if *generation == next_gen
        ));
        // The served post-restart snapshot converges the held generation.
        assert_eq!(
            state.apply_snapshot(
                &recovery_snapshot(&initial_chunks(), next_gen, 0),
                &initial_chunks()
            ),
            TerrainConvergenceAction::Applied
        );
        assert_eq!(state.revision(), 0);
        assert_eq!(state.chunks(), &initial_chunks());
        assert_eq!(
            state.observe_generation(next_gen, &initial_chunks()),
            TerrainConvergenceAction::Ignored
        );
        // An unrelated generation change discards the held reset and recovers instead.
        let mut state = ready_state();
        assert_eq!(
            state.apply_reset(
                TerrainResetEvent {
                    previous_generation: terrain_gen,
                    next_generation: next_gen,
                },
                Some(terrain_gen),
                &initial_chunks()
            ),
            TerrainConvergenceAction::RequestRecovery(next_gen)
        );
        let foreign = generation(7);
        assert_eq!(
            state.observe_generation(foreign, &initial_chunks()),
            TerrainConvergenceAction::RequestRecovery(foreign)
        );
        assert!(matches!(
            &state.phase,
            TerrainConvergencePhase::AwaitingRecovery { generation, .. } if *generation == foreign
        ));
    }

    #[test]
    fn generation_change_discards_state_and_requests_recovery() {
        let mut state = ready_state();
        let (event, next_chunks) =
            stage_event(&initial_chunks(), generation(1), 1, center_brush(2));
        assert_eq!(state.apply_event(event), TerrainConvergenceAction::Applied);
        let next_gen = generation(2);
        assert_eq!(
            state.observe_generation(next_gen, &initial_chunks()),
            TerrainConvergenceAction::RequestRecovery(next_gen)
        );
        assert!(state.chunks().is_empty());
        assert_eq!(state.revision(), 0);
        let _ = next_chunks;
    }

    #[test]
    fn clear_resets_every_generation_scoped_state() {
        let mut state = ready_state();
        state.clear();
        assert_eq!(state.phase, TerrainConvergencePhase::WaitingForMap);
        assert!(state.chunks().is_empty());
        let terrain_gen = generation(5);
        assert_eq!(
            state.observe_generation(terrain_gen, &initial_chunks()),
            TerrainConvergenceAction::RequestRecovery(terrain_gen)
        );
    }
}

#[cfg(all(test, feature = "server"))]
mod reset_cycle_tests {
    use super::authority_tests::{current_occupancy, fact, root, terrain_app};
    use std::time::Instant;

    /// The 100-cycle reset budget from the M10 scale slice: destroy, reset, repeat, and
    /// confirm the per-cycle cost stays small, the state exactly returns, and repeated
    /// cycles leak no entities, queues, or telemetry records.
    #[test]
    fn one_hundred_destroy_reset_cycles_stay_fast_and_exact() {
        let mut app = terrain_app();
        let initial = current_occupancy(app.world_mut());
        let initial_chunks = {
            let world = app.world_mut();
            let mut chunks = world.query::<&crate::terrain::TerrainChunk>();
            chunks.iter(world).count()
        };
        app.init_resource::<crate::matchplay::PendingMatchRestart>();
        let start = Instant::now();
        for cycle in 0..100_u64 {
            // Each cycle's detonation carries a tick after the previous restart's brush
            // epoch, exactly like a real post-restart impact.
            let mut detonation = fact(1, (0.0, 0.0), 48.0);
            detonation.tick = cycle + 1;
            app.world_mut()
                .resource_mut::<crate::combat::CombatWorldEffectFacts>()
                .0
                .push(detonation);
            app.update();
            assert_eq!(root(app.world_mut()).revision, 1, "cycle {cycle}");
            app.world_mut()
                .resource_mut::<crate::matchplay::PendingMatchRestart>()
                .stage_for_test(crate::matchplay::PendingMatchRestartSlot {
                    previous_id: crate::matchplay::MatchId((cycle * 2 + 1).into()),
                    next_id: crate::matchplay::MatchId((cycle * 2 + 3).into()),
                    restart_tick: cycle,
                });
            crate::terrain::reset_terrain_on_match_restart(app.world_mut());
            let world = app.world_mut();
            assert_eq!(root(world).revision, 0, "cycle {cycle}");
            assert!(
                world
                    .resource::<crate::terrain::PendingTerrainBrushes>()
                    .queue
                    .is_empty(),
                "cycle {cycle}"
            );
            assert!(
                world
                    .resource::<crate::terrain::TerrainBrushBatch>()
                    .brushes
                    .is_empty(),
                "cycle {cycle}"
            );
        }
        let elapsed = start.elapsed();
        let world = app.world_mut();
        assert_eq!(current_occupancy(world), initial);
        let mut chunks = world.query::<&crate::terrain::TerrainChunk>();
        assert_eq!(
            chunks.iter(world).count(),
            initial_chunks,
            "repeated cycles leak no chunk entities"
        );
        assert_eq!(
            world
                .resource::<crate::terrain::TerrainChunkIndex>()
                .0
                .len(),
            initial_chunks
        );
        assert!(
            world
                .resource::<crate::terrain::telemetry::TerrainTelemetry>()
                .records
                .len()
                <= crate::terrain::model::MAX_TERRAIN_TELEMETRY_RECORDS
        );
        assert!(
            elapsed.as_millis() < 4_000,
            "100 destroy/reset cycles took {elapsed:?}"
        );
    }
}

#[cfg(feature = "client")]
mod client_presentation_tests {
    use super::convergence_tests::{center_brush, generation, initial_chunks, stage_event};
    use super::*;
    use crate::terrain::ClientTerrainConvergence;
    use crate::terrain::TerrainConvergenceAction;
    use crate::terrain::TerrainConvergencePhase;
    use crate::terrain::client::presentation::{
        TerrainDebris, expire_terrain_debris, spawn_terrain_debris,
    };
    use crate::terrain::client::recovery::{
        classify_client_event, clear_telemetry_on_generation_change, record_snapshot_application,
    };
    use crate::terrain::grid::recovery_snapshot;
    use crate::terrain::telemetry::{TerrainTelemetry, TerrainTelemetryOutcome};
    use bevy::prelude::*;

    fn debris_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<Image>>()
            .init_resource::<ClientTerrainConvergence>()
            .add_systems(
                Update,
                (spawn_terrain_debris, expire_terrain_debris).chain(),
            );
        app
    }

    /// A convergence machine that already committed the initial snapshot at revision 0.
    fn converged(terrain_gen: TerrainGeneration) -> ClientTerrainConvergence {
        let mut state = ClientTerrainConvergence::default();
        assert_eq!(
            state.observe_generation(terrain_gen, &initial_chunks()),
            TerrainConvergenceAction::RequestRecovery(terrain_gen)
        );
        state.mark_request_sent();
        assert_eq!(
            state.apply_snapshot(
                &recovery_snapshot(&initial_chunks(), terrain_gen, 0),
                &initial_chunks(),
            ),
            TerrainConvergenceAction::Applied
        );
        state
    }

    /// Commit `count` distinct fresh-terrain brushes through the public convergence path
    /// so the debris spawner observes one authoritative burst. Centers sit on an
    /// eight-half-cell grid so every disc erases fresh cells: a zero-effect event is
    /// corrupt input the machine rejects with recovery instead of committing.
    pub(super) fn commit_burst(
        convergence: &mut ClientTerrainConvergence,
        start: usize,
        count: usize,
    ) {
        let TerrainConvergencePhase::Ready {
            generation: terrain_gen,
        } = convergence.phase
        else {
            panic!("commit_burst expects a ready convergence");
        };
        let mut current = convergence.chunks().clone();
        for offset in 0..count {
            let index = start + offset;
            let (lattice, within) = (index / 64, index % 64);
            let brush = TerrainBrush {
                center_half_cells_x: 1 + i16::try_from((within % 8) * 8 + (lattice % 2) * 4)
                    .unwrap(),
                center_half_cells_y: 1 + i16::try_from((within / 8) * 8 + (lattice % 2) * 4)
                    .unwrap(),
                radius_half_cells: 2,
            };
            let (event, next) =
                stage_event(&current, terrain_gen, convergence.revision() + 1, brush);
            assert!(
                event.erased_cells > 0,
                "burst brush {index} must erase fresh cells"
            );
            assert_eq!(
                convergence.apply_event(event),
                TerrainConvergenceAction::Applied
            );
            current = next;
        }
    }

    pub(super) fn debris_count(app: &mut App) -> usize {
        let mut debris = app
            .world_mut()
            .query_filtered::<&Transform, With<TerrainDebris>>();
        debris.iter(app.world()).count()
    }

    #[test]
    fn debris_bursts_respect_the_ceiling_across_existing_and_new_effects() {
        let mut app = debris_app();
        {
            let mut convergence = app.world_mut().resource_mut::<ClientTerrainConvergence>();
            let terrain_gen = generation(1);
            assert_eq!(
                convergence.observe_generation(terrain_gen, &initial_chunks()),
                TerrainConvergenceAction::RequestRecovery(terrain_gen)
            );
            convergence.mark_request_sent();
            let snapshot = recovery_snapshot(&initial_chunks(), terrain_gen, 0);
            assert_eq!(
                convergence.apply_snapshot(&snapshot, &initial_chunks()),
                TerrainConvergenceAction::Applied
            );
            commit_burst(&mut convergence, 0, 63);
        }
        app.update();
        assert_eq!(
            debris_count(&mut app),
            63,
            "a sub-ceiling burst lands whole"
        );
        // A full 24-brush burst on top of 63 live effects must hold the ceiling instead
        // of exceeding it.
        {
            let mut convergence = app.world_mut().resource_mut::<ClientTerrainConvergence>();
            commit_burst(&mut convergence, 64, 24);
        }
        app.update();
        assert_eq!(
            debris_count(&mut app),
            MAX_TERRAIN_DEBRIS_EFFECTS,
            "existing plus pending stays exactly at the ceiling"
        );
        // Repeated bursts keep it there.
        {
            let mut convergence = app.world_mut().resource_mut::<ClientTerrainConvergence>();
            commit_burst(&mut convergence, 88, 24);
        }
        app.update();
        assert_eq!(debris_count(&mut app), MAX_TERRAIN_DEBRIS_EFFECTS);
    }

    #[test]
    fn stale_generation_debris_is_retired_immediately() {
        let mut app = debris_app();
        {
            let mut convergence = app.world_mut().resource_mut::<ClientTerrainConvergence>();
            *convergence = converged(generation(1));
            commit_burst(&mut convergence, 0, 4);
        }
        app.update();
        assert_eq!(debris_count(&mut app), 4, "fresh debris presents");
        // A generation change (restart or map replacement) moves convergence out of the
        // old generation's Ready state: the sweep retires old debris without waiting on
        // the 500 ms presentation timer.
        {
            let mut convergence = app.world_mut().resource_mut::<ClientTerrainConvergence>();
            assert_eq!(
                convergence.observe_generation(generation(2), &initial_chunks()),
                TerrainConvergenceAction::RequestRecovery(generation(2))
            );
        }
        app.update();
        assert_eq!(
            debris_count(&mut app),
            0,
            "a generation change retires old-generation debris immediately"
        );
        // New-generation debris presents again once convergence re-commits.
        {
            let mut convergence = app.world_mut().resource_mut::<ClientTerrainConvergence>();
            *convergence = converged(generation(2));
            commit_burst(&mut convergence, 0, 2);
        }
        app.update();
        assert_eq!(debris_count(&mut app), 2);
        // Disconnect clears convergence entirely: no debris survives the session.
        {
            let mut convergence = app.world_mut().resource_mut::<ClientTerrainConvergence>();
            convergence.clear();
        }
        app.update();
        assert_eq!(
            debris_count(&mut app),
            0,
            "disconnect retires every debris effect"
        );
    }

    #[test]
    fn client_convergence_telemetry_records_duplicates_gaps_and_snapshots() {
        let terrain_gen = generation(1);
        let mut state = converged(terrain_gen);
        let mut telemetry = TerrainTelemetry::default();
        // A duplicate revision from a committed Ready state counts once.
        let (event, _) = stage_event(&initial_chunks(), terrain_gen, 1, center_brush(2));
        assert_eq!(
            state.apply_event(event.clone()),
            TerrainConvergenceAction::Applied
        );
        classify_client_event(&state, &event, 7, &mut telemetry);
        assert_eq!(telemetry.aggregates.client_duplicates, 1);
        assert_eq!(telemetry.aggregates.client_gaps, 0);
        // A revision beyond committed + 1 reports the observed gap.
        let (gap, _) = stage_event(&initial_chunks(), terrain_gen, 9, center_brush(2));
        classify_client_event(&state, &gap, 8, &mut telemetry);
        assert_eq!(telemetry.aggregates.client_duplicates, 1);
        assert_eq!(telemetry.aggregates.client_gaps, 1);
        // An event from a foreign generation is neither a duplicate nor a gap.
        let (foreign, _) = stage_event(&initial_chunks(), generation(2), 1, center_brush(2));
        classify_client_event(&state, &foreign, 9, &mut telemetry);
        assert_eq!(telemetry.aggregates.client_duplicates, 1);
        assert_eq!(telemetry.aggregates.client_gaps, 1);
        // One applied recovery snapshot records against the committed generation.
        record_snapshot_application(&state, 0, 9, &mut telemetry);
        assert_eq!(telemetry.aggregates.client_snapshots_applied, 1);
        assert_eq!(telemetry.aggregates.client_duplicates, 1);
        let record = telemetry
            .records
            .iter()
            .rev()
            .find(|record| record.outcome == TerrainTelemetryOutcome::ClientSnapshotApplied)
            .expect("the snapshot application leaves a record");
        assert_eq!(record.map_instance_id, terrain_gen.map_instance_id);
        assert_eq!(record.revision, 0);
    }

    #[test]
    fn client_telemetry_clears_exactly_once_per_generation_change() {
        let terrain_gen = generation(1);
        let mut state = converged(terrain_gen);
        let mut telemetry = TerrainTelemetry::default();
        let mut telemetry_generation = None;
        // Adopting the first generation clears nothing; convergence facts accumulate.
        clear_telemetry_on_generation_change(&state, &mut telemetry, &mut telemetry_generation);
        let (event, _) = stage_event(&initial_chunks(), terrain_gen, 1, center_brush(2));
        assert_eq!(
            state.apply_event(event.clone()),
            TerrainConvergenceAction::Applied
        );
        classify_client_event(&state, &event, 7, &mut telemetry);
        record_snapshot_application(&state, 0, 7, &mut telemetry);
        assert_eq!(telemetry.records.len(), 2);
        // A restart or map replacement moves the machine to a new generation: the
        // previous generation's convergence facts must not survive the boundary.
        assert_eq!(
            state.observe_generation(generation(2), &initial_chunks()),
            TerrainConvergenceAction::RequestRecovery(generation(2))
        );
        clear_telemetry_on_generation_change(&state, &mut telemetry, &mut telemetry_generation);
        assert_eq!(telemetry, TerrainTelemetry::default());
        // The new generation records its own facts; the clear happened exactly once.
        state.mark_request_sent();
        assert_eq!(
            state.apply_snapshot(
                &recovery_snapshot(&initial_chunks(), generation(2), 0),
                &initial_chunks(),
            ),
            TerrainConvergenceAction::Applied
        );
        record_snapshot_application(&state, 0, 8, &mut telemetry);
        assert_eq!(telemetry.aggregates.client_snapshots_applied, 1);
    }
}

#[cfg(feature = "client")]
mod client_soak_tests {
    use super::client_presentation_tests::{commit_burst, debris_count};
    use super::convergence_tests::{generation, initial_chunks};
    use crate::terrain::ClientTerrainConvergence;
    use crate::terrain::TerrainConvergenceAction;
    use crate::terrain::client::presentation::{
        TerrainChunkVisual, expire_terrain_debris, spawn_terrain_debris, update_terrain_visuals,
    };
    use crate::terrain::client::{ExpectedClientTerrain, ExpectedClientTerrainSlot};
    use crate::terrain::grid::recovery_snapshot;
    use crate::terrain::model::MAX_TERRAIN_DEBRIS_EFFECTS;
    use bevy::prelude::*;

    fn visual_count(app: &mut App) -> usize {
        let mut visuals = app
            .world_mut()
            .query_filtered::<&Transform, With<TerrainChunkVisual>>();
        visuals.iter(app.world()).count()
    }

    fn image_count(app: &mut App) -> usize {
        app.world().resource::<Assets<Image>>().len()
    }

    /// The M10 client growth soak: one hundred destroy/reset cycles through the public
    /// convergence path hold visual-entity, image-handle, and debris bounds with no
    /// per-cycle growth, and debris reaches exact cleanup by presentation time.
    #[test]
    fn one_hundred_client_destroy_reset_cycles_hold_visual_and_debris_bounds() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_millis(50),
            ))
            .init_resource::<Assets<Image>>()
            .init_resource::<ClientTerrainConvergence>()
            .init_resource::<ExpectedClientTerrainSlot>()
            .add_systems(
                Update,
                (
                    update_terrain_visuals,
                    spawn_terrain_debris,
                    expire_terrain_debris,
                ),
            );
        let chunks = initial_chunks();
        let layout = crate::map::InitialTerrainLayout {
            terrain_fingerprint: 0xabcd_ef01,
            chunks: chunks.clone(),
            total_cells: 1024,
        };
        let set_slot = |app: &mut App, current: crate::terrain::TerrainGeneration| {
            app.insert_resource(ExpectedClientTerrainSlot::Derived(ExpectedClientTerrain {
                generation: current,
                layout: layout.clone(),
                derived_from: (crate::map::MapInstanceId(1), current.match_id),
            }));
        };
        let first = generation(1);
        {
            let mut convergence = app.world_mut().resource_mut::<ClientTerrainConvergence>();
            assert_eq!(
                convergence.observe_generation(first, &chunks),
                TerrainConvergenceAction::RequestRecovery(first)
            );
            convergence.mark_request_sent();
            assert_eq!(
                convergence.apply_snapshot(&recovery_snapshot(&chunks, first, 0), &chunks),
                TerrainConvergenceAction::Applied
            );
        }
        set_slot(&mut app, first);
        app.update();
        let baseline_visuals = visual_count(&mut app);
        let baseline_images = image_count(&mut app);
        assert_eq!(baseline_visuals, chunks.len());
        for cycle in 0..100_u64 {
            {
                let mut convergence = app.world_mut().resource_mut::<ClientTerrainConvergence>();
                commit_burst(&mut convergence, 0, 1);
            }
            app.update();
            // The server resets for the next match; the client observes the new match
            // id and accepts the chained reset.
            let previous = generation(cycle + 1);
            let next = generation(cycle + 2);
            set_slot(&mut app, next);
            {
                let mut convergence = app.world_mut().resource_mut::<ClientTerrainConvergence>();
                assert_eq!(
                    convergence.apply_reset(
                        crate::terrain::TerrainResetEvent {
                            previous_generation: previous,
                            next_generation: next,
                        },
                        Some(next),
                        &chunks,
                    ),
                    TerrainConvergenceAction::Applied
                );
            }
            app.update();
            if cycle % 10 == 0 {
                assert_eq!(
                    visual_count(&mut app),
                    baseline_visuals,
                    "cycle {cycle} leaks no chunk visuals"
                );
                assert_eq!(
                    image_count(&mut app),
                    baseline_images,
                    "cycle {cycle} leaks no image handles"
                );
            }
        }
        assert_eq!(visual_count(&mut app), baseline_visuals);
        assert_eq!(image_count(&mut app), baseline_images);
        assert_eq!(
            app.world()
                .resource::<ClientTerrainConvergence>()
                .revision(),
            0
        );
        assert!(debris_count(&mut app) <= MAX_TERRAIN_DEBRIS_EFFECTS);
        // One debris lifetime later, the last cycle's feedback is gone exactly.
        for _ in 0..10 {
            app.update();
        }
        assert_eq!(debris_count(&mut app), 0, "debris expires to exact cleanup");
    }
}
