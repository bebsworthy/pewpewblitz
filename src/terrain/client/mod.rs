//! Client terrain composition: convergence state ownership, plugin schedule, and the
//! terrain readiness input gate. Recovery/wire logic lives in `recovery`; images,
//! sprites, and debris live in `presentation`.
#![allow(
    clippy::needless_pass_by_value,
    reason = "system parameters follow the sibling client modules' shared-resource style"
)]

pub(crate) mod presentation;
pub(crate) mod recovery;

pub use presentation::{TERRAIN_PRESENTATION_Z, TerrainChunkVisual, paint_chunk_pixels};
use recovery::{
    clear_terrain_convergence_on_disconnect, derive_expected_client_terrain,
    drive_terrain_wire_convergence,
};

use super::TerrainCorePlugin;
use super::model::TerrainGeneration;
use super::telemetry::TerrainTelemetry;
use crate::map::{InitialTerrainLayout, MapInstanceId};
use crate::matchplay::MatchId;
use bevy::prelude::*;

/// User-facing terrain synchronization state derived from the pure convergence phase.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub enum ClientTerrainReadiness {
    #[default]
    WaitingForMap,
    SyncingTerrain,
    RecoveringTerrain,
    Ready,
    Invalid(String),
}

/// One locally derived expectation from the replicated map and match state.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ExpectedClientTerrain {
    pub(super) generation: TerrainGeneration,
    pub(super) layout: InitialTerrainLayout,
    pub(super) derived_from: (MapInstanceId, MatchId),
}

/// Derivation cache so layout resolution runs only when the replicated pair changes.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub(crate) enum ExpectedClientTerrainSlot {
    #[default]
    Waiting,
    Failed(String),
    Derived(ExpectedClientTerrain),
}

pub struct ClientTerrainPlugin;

impl Plugin for ClientTerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TerrainCorePlugin)
            .init_resource::<ClientTerrainReadiness>()
            .init_resource::<ExpectedClientTerrainSlot>()
            .init_resource::<TerrainTelemetry>()
            .add_systems(
                Update,
                (
                    derive_expected_client_terrain,
                    drive_terrain_wire_convergence,
                    clear_terrain_convergence_on_disconnect,
                    presentation::update_terrain_visuals,
                    presentation::spawn_terrain_debris,
                    presentation::expire_terrain_debris,
                )
                    .chain(),
            )
            .add_systems(PostUpdate, gate_inputs_on_terrain_readiness);
    }
}

/// Terrain is the fourth readiness observation: inputs stay suppressed until convergence
/// reports the matching generation committed. Runs after the Update-stage readiness
/// writers so the clamp is authoritative for the next sampled frame.
fn gate_inputs_on_terrain_readiness(
    readiness: Res<ClientTerrainReadiness>,
    config: Option<Res<crate::config::ClientNetworkConfig>>,
    joins: Query<&crate::client::ClientJoinStatus>,
    mut playable: ResMut<crate::client::ClientPlayableGate>,
    mut suppressed: Local<bool>,
) {
    if !matches!(&*readiness, ClientTerrainReadiness::Ready) {
        *suppressed = true;
        playable.0 = false;
        return;
    }
    if !*suppressed {
        return;
    }
    *suppressed = false;
    // The windowed composition recomputes the full playable formula every Update; the
    // headless composition has no asset writer, so this gate restores it directly once
    // an accepted client's terrain has converged.
    let headless = config.is_none_or(|config| config.headless);
    if headless
        && joins
            .iter()
            .any(|status| matches!(status.phase, crate::client::ClientJoinPhase::Active { .. }))
    {
        playable.0 = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matchplay::MatchPhase;
    use crate::matchplay::MatchRoot as MatchRootMarker;
    use crate::terrain::grid as terrain_grid;
    use crate::terrain::model::{
        TERRAIN_CHUNK_SIDE_CELLS, TerrainBits, TerrainChunkId, TerrainDestructionEvent,
    };
    use crate::terrain::network::ClientTerrainConvergence;
    use crate::terrain::network::TerrainConvergenceAction;
    use bevy::image::{Image, ImageSampler};
    use bevy::render::render_resource::TextureFormat;
    use presentation::chunk_image;
    use presentation::{TERRAIN_EDGE_PIXEL, TERRAIN_FILL_PIXEL};
    use std::collections::BTreeMap;

    fn full_chunk(id: TerrainChunkId) -> (TerrainChunkId, TerrainBits) {
        let mut bits = TerrainBits::default();
        for local_y in 0..TERRAIN_CHUNK_SIDE_CELLS {
            for local_x in 0..TERRAIN_CHUNK_SIDE_CELLS {
                bits.set(local_x, local_y);
            }
        }
        (id, bits)
    }

    /// Row-major pixel lookup matching the painted top-down image rows.
    fn pixel(data: &[u8], local_x: u32, local_y: u32) -> [u8; 4] {
        let row = TERRAIN_CHUNK_SIDE_CELLS - 1 - local_y;
        let index = ((row * TERRAIN_CHUNK_SIDE_CELLS + local_x) * 4) as usize;
        [
            data[index],
            data[index + 1],
            data[index + 2],
            data[index + 3],
        ]
    }

    #[test]
    fn intact_chunk_paints_fill_interior_and_edge_rim() {
        let (_, bits) = full_chunk(TerrainChunkId { x: 0, y: 0 });
        let data = paint_chunk_pixels(&bits, None, None, None, None);
        assert_eq!(pixel(&data, 16, 16), TERRAIN_FILL_PIXEL, "interior fills");
        // Every boundary cell borders the unallocated outside, so it rims.
        assert_eq!(pixel(&data, 0, 0), TERRAIN_EDGE_PIXEL);
        assert_eq!(pixel(&data, 31, 16), TERRAIN_EDGE_PIXEL);
    }

    #[test]
    fn cross_seam_edges_follow_the_neighbor_occupancy() {
        let (_, west_bits) = full_chunk(TerrainChunkId { x: 0, y: 0 });
        let (_, east_bits) = full_chunk(TerrainChunkId { x: 1, y: 0 });
        let data = paint_chunk_pixels(&west_bits, None, Some(&east_bits), None, None);
        assert_eq!(
            pixel(&data, 31, 16),
            TERRAIN_FILL_PIXEL,
            "a solid east neighbor leaves the seam cell as interior"
        );
        let mut carved = east_bits;
        carved.clear(0, 16);
        let data = paint_chunk_pixels(&west_bits, None, Some(&carved), None, None);
        assert_eq!(
            pixel(&data, 31, 16),
            TERRAIN_EDGE_PIXEL,
            "erasing the neighbor cell rims the seam cell"
        );
    }

    #[test]
    fn empty_chunk_and_erased_crater_paint_transparent_holes() {
        let data = paint_chunk_pixels(&TerrainBits::default(), None, None, None, None);
        assert!(data.iter().all(|byte| *byte == 0));
        let (_, bits) = full_chunk(TerrainChunkId { x: 0, y: 0 });
        let mut chunks = BTreeMap::from([full_chunk(TerrainChunkId { x: 0, y: 0 })]);
        let brush = crate::terrain::TerrainBrush {
            center_half_cells_x: 1,
            center_half_cells_y: 1,
            radius_half_cells: 2,
        };
        let _ = terrain_grid::apply_brush(&mut chunks, brush);
        let data = paint_chunk_pixels(
            &chunks[&TerrainChunkId { x: 0, y: 0 }],
            None,
            None,
            None,
            None,
        );
        assert_eq!(
            pixel(&data, 0, 0),
            [0, 0, 0, 0],
            "the crater core is a hole"
        );
        let _ = bits;
    }

    #[test]
    fn chunk_images_are_tiny_nearest_sampled_rgba8_quads() {
        let image = chunk_image(vec![
            7_u8;
            (TERRAIN_CHUNK_SIDE_CELLS * TERRAIN_CHUNK_SIDE_CELLS * 4)
                as usize
        ]);
        assert_eq!(
            image.texture_descriptor.size.width,
            TERRAIN_CHUNK_SIDE_CELLS
        );
        assert_eq!(
            image.texture_descriptor.size.height,
            TERRAIN_CHUNK_SIDE_CELLS
        );
        assert!(matches!(
            image.texture_descriptor.format,
            TextureFormat::Rgba8UnormSrgb
        ));
        assert!(
            matches!(&image.sampler, ImageSampler::Descriptor(descriptor)
            if descriptor.mag_filter == bevy::image::ImageFilterMode::Nearest)
        );
    }

    /// A client app whose replicated map and match state drive the same three-chunk
    /// expectation the convergence machine commits for these tests.
    fn visual_app() -> (
        App,
        TerrainGeneration,
        BTreeMap<TerrainChunkId, TerrainBits>,
    ) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<Image>>()
            .insert_resource(crate::client::ClientPlayableGate(true))
            .add_plugins(ClientTerrainPlugin);

        let resolved = crate::map::MapContentCatalog::embedded()
            .expect("embedded map catalog")
            .resolve_preset(
                crate::map::MapPresetId(1),
                MapInstanceId(1),
                &crate::map::MapLayoutRequirements::wipeout(),
            )
            .expect("preset resolves");
        let snapshot = resolved.snapshot;
        let rules_revision = snapshot.recipe_schema_version;
        let mode_definition_id = snapshot.mode_definition_id;
        let layout = crate::map::resolve_initial_terrain(
            snapshot.playable_bounds,
            &snapshot.geometry,
            &snapshot.regions,
            &snapshot.spawn_points,
            &snapshot.mode_anchors,
            crate::map::EngineMapLimits::default(),
        )
        .expect("preset terrain layout resolves");
        let generation = TerrainGeneration {
            map_instance_id: MapInstanceId(1),
            match_id: MatchId(1),
            terrain_fingerprint: layout.terrain_fingerprint,
        };
        let world = app.world_mut();
        // A bare client marker keeps the disconnect-clear system from treating this
        // single-app fixture as an already-disconnected peer.
        world.spawn(lightyear::prelude::client::Client);
        world.spawn((
            crate::map::MapRoot,
            snapshot.identity.instance_id,
            snapshot.identity,
            snapshot,
        ));
        world.spawn((
            MatchRootMarker,
            crate::matchplay::MatchState {
                match_id: MatchId(1),
                mode_definition_id,
                phase: MatchPhase::Waiting,
                rules_revision,
            },
        ));
        // Commit an authoritative snapshot for the exact derived generation so the
        // presentation works from committed occupancy like a recovered client.
        {
            let mut convergence = world.resource_mut::<ClientTerrainConvergence>();
            assert!(matches!(
                convergence.observe_generation(generation, &layout.chunks),
                TerrainConvergenceAction::RequestRecovery(_)
            ));
            convergence.mark_request_sent();
            assert_eq!(
                convergence.apply_snapshot(
                    &terrain_grid::recovery_snapshot(&layout.chunks, generation, 0),
                    &layout.chunks
                ),
                TerrainConvergenceAction::Applied
            );
            convergence.take_dirty();
        }
        (app, generation, layout.chunks)
    }

    #[test]
    fn visuals_spawn_one_sprite_per_expected_chunk_with_terrain_depth() {
        let (mut app, _, layout_chunks) = visual_app();
        app.update();
        let world = app.world_mut();
        let mut visuals = world.query::<(&TerrainChunkVisual, &Transform, &Sprite)>();
        let mut count = 0;
        for (visual, transform, sprite) in visuals.iter(world) {
            count += 1;
            assert_eq!(
                sprite.custom_size,
                Some(Vec2::splat(crate::terrain::TERRAIN_CHUNK_SIDE_WORLD))
            );
            assert!((transform.translation.z - TERRAIN_PRESENTATION_Z).abs() <= f32::EPSILON);
            let min = terrain_grid::chunk_min_world(visual.chunk);
            assert_eq!(
                transform.translation.truncate(),
                min + Vec2::splat(crate::terrain::TERRAIN_CHUNK_SIDE_WORLD * 0.5)
            );
        }
        assert_eq!(count, layout_chunks.len());
        // Ready terrain leaves the pre-set playable gate untouched.
        assert!(world.resource::<crate::client::ClientPlayableGate>().0);
    }

    #[test]
    fn applied_brush_repaints_the_chunk_its_neighbors_and_spawns_debris() {
        let (mut app, generation, layout_chunks) = visual_app();
        app.update();
        // A small brush at the origin erases chunk (0,0)'s west-boundary cells, so the
        // (-1,0) neighbor must repaint its seam rim even though its bits never changed.
        let brush = crate::terrain::TerrainBrush {
            center_half_cells_x: 1,
            center_half_cells_y: 1,
            radius_half_cells: 2,
        };
        let event = {
            let mut touched = layout_chunks.clone();
            let outcome = terrain_grid::apply_brush(&mut touched, brush);
            TerrainDestructionEvent {
                generation,
                revision: 1,
                source_attack_id: crate::combat::AttackId(1),
                source_delivery_index: 0,
                brush,
                affected_chunks: outcome.affected_chunks,
                erased_cells: outcome.erased_cells,
            }
        };
        {
            let mut convergence = app.world_mut().resource_mut::<ClientTerrainConvergence>();
            assert_eq!(
                convergence.apply_event(event),
                TerrainConvergenceAction::Applied
            );
        }
        app.update();
        let world = app.world_mut();
        // The seam-facing west neighbor repainted too: its x=31 rim follows the crater.
        let mut visuals = world.query_filtered::<&TerrainChunkVisual, ()>();
        let west = visuals
            .iter(world)
            .find(|visual| visual.chunk == TerrainChunkId { x: -1, y: 0 })
            .expect("west neighbor visual");
        let images = world.resource::<Assets<Image>>();
        let image = images.get(&west.image).expect("west image");
        // The erased seam cell is now a hole; the occupied cell one further west rims it,
        // proving the unchanged neighbor chunk repainted from the new occupancy.
        let rim = pixel(image.data.as_deref().unwrap_or_default(), 30, 0);
        assert_eq!(rim, TERRAIN_EDGE_PIXEL);
        // One cosmetic burst for the committed brush.
        let mut debris = world.query::<&presentation::TerrainDebris>();
        assert_eq!(debris.iter(world).count(), 1);
    }
}
