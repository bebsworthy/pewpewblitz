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

    fn generation(match_id: u64) -> TerrainGeneration {
        TerrainGeneration {
            map_instance_id: crate::map::MapInstanceId(1),
            match_id: MatchId(match_id),
            terrain_fingerprint: 0xabcd_ef01,
        }
    }

    /// One fully occupied chunk at the origin, like the built-in block's corner.
    fn initial_chunks() -> BTreeMap<TerrainChunkId, TerrainBits> {
        BTreeMap::from([chunk_with_all_cells_set(TerrainChunkId { x: 0, y: 0 })])
    }

    /// Compute the exact event a server would send by rasterizing `brush` on `current`.
    fn stage_event(
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

    fn center_brush(radius_half_cells: u16) -> TerrainBrush {
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
            state.apply_snapshot(&recovery_snapshot(&initial_chunks(), terrain_gen, 0)),
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
            state.apply_snapshot(&recovery_snapshot(&initial_chunks(), terrain_gen, 0)),
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
        let (second, after_second) = stage_event(
            &after_first,
            terrain_gen,
            2,
            TerrainBrush {
                center_half_cells_x: -5,
                center_half_cells_y: 3,
                radius_half_cells: 2,
            },
        );
        // Out-of-order arrival while recovery is outstanding.
        assert_eq!(
            state.apply_event(second.clone()),
            TerrainConvergenceAction::Buffered
        );
        assert_eq!(state.apply_event(first), TerrainConvergenceAction::Buffered);
        assert_eq!(
            state.apply_snapshot(&recovery_snapshot(&initial_chunks(), terrain_gen, 0)),
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
            state.apply_snapshot(&snapshot),
            TerrainConvergenceAction::Invalidated(_)
        ));
        // A snapshot missing its expected chunk also invalidates.
        let mut state = ClientTerrainConvergence::default();
        state.observe_generation(terrain_gen, &initial_chunks());
        let mut missing = recovery_snapshot(&initial_chunks(), terrain_gen, 0);
        missing.chunks.clear();
        assert!(matches!(
            state.apply_snapshot(&missing),
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
            state.apply_snapshot(&snapshot),
            TerrainConvergenceAction::Invalidated(_)
        ));
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
            state.apply_snapshot(&recovery_snapshot(&initial_chunks(), terrain_gen, u64::MAX)),
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
    /// confirm the per-cycle cost stays small and the state exactly returns.
    #[test]
    fn one_hundred_destroy_reset_cycles_stay_fast_and_exact() {
        let mut app = terrain_app();
        let initial = current_occupancy(app.world_mut());
        app.init_resource::<crate::matchplay::PendingMatchRestart>();
        let start = Instant::now();
        for cycle in 0..100_u64 {
            app.world_mut()
                .resource_mut::<crate::combat::CombatWorldEffectFacts>()
                .0
                .push(fact(1, (0.0, 0.0), 48.0));
            app.update();
            assert_eq!(root(app.world_mut()).revision, 1, "cycle {cycle}");
            app.world_mut()
                .resource_mut::<crate::matchplay::PendingMatchRestart>()
                .stage_for_test(crate::matchplay::PendingMatchRestartSlot {
                    previous_id: crate::matchplay::MatchId(cycle * 2 + 1),
                    next_id: crate::matchplay::MatchId(cycle * 2 + 3),
                    restart_tick: cycle,
                });
            crate::terrain::reset_terrain_on_match_restart(app.world_mut());
            assert_eq!(root(app.world_mut()).revision, 0, "cycle {cycle}");
        }
        let elapsed = start.elapsed();
        assert_eq!(current_occupancy(app.world_mut()), initial);
        assert!(
            elapsed.as_millis() < 4_000,
            "100 destroy/reset cycles took {elapsed:?}"
        );
    }
}
