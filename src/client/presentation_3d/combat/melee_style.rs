//! Bounded geometry cache for authored melee sweep angles.

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use std::collections::BTreeMap;

const MAX_MELEE_ARC_MESHES: usize = 64;
const ARC_SEGMENTS: u16 = 48;
const INNER_RADIUS_AT_CREST: f32 = 0.62;
const INNER_RADIUS_AT_TIP: f32 = 0.985;

#[derive(Resource, Default)]
pub(in crate::client) struct MeleeArcMeshCache {
    meshes: BTreeMap<u16, Handle<Mesh>>,
}

impl MeleeArcMeshCache {
    pub(in crate::client) fn resolve(
        &mut self,
        angle_degrees: f32,
        meshes: &mut Assets<Mesh>,
    ) -> Handle<Mesh> {
        let key = angle_key(angle_degrees);
        if let Some(handle) = self.meshes.get(&key) {
            return handle.clone();
        }
        if self.meshes.len() >= MAX_MELEE_ARC_MESHES {
            return self
                .meshes
                .range(..=key)
                .next_back()
                .or_else(|| self.meshes.range(key..).next())
                .map(|(_, handle)| handle.clone())
                .expect("a full melee mesh cache contains an entry");
        }
        let handle = meshes.add(melee_arc_mesh(f32::from(key) * 0.1));
        self.meshes.insert(key, handle.clone());
        handle
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.meshes.len()
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the validated 1..=180 degree angle is clamped and quantized to a bounded tenth-degree cache key"
)]
fn angle_key(angle_degrees: f32) -> u16 {
    (angle_degrees.clamp(1.0, 180.0) * 10.0).round() as u16
}

fn melee_arc_mesh(angle_degrees: f32) -> Mesh {
    let half_angle = angle_degrees.to_radians() * 0.5;
    let vertex_count = usize::from(ARC_SEGMENTS + 1) * 2;
    let mut positions = Vec::with_capacity(vertex_count);
    let mut uvs = Vec::with_capacity(vertex_count);
    for segment in 0..=ARC_SEGMENTS {
        let progress = f32::from(segment) / f32::from(ARC_SEGMENTS);
        let angle = (-half_angle).lerp(half_angle, progress);
        let direction = Vec2::from_angle(angle);
        let inner_radius = melee_slash_inner_radius(progress);
        positions.push([direction.x * inner_radius, direction.y * inner_radius, 0.0]);
        positions.push([direction.x, direction.y, 0.0]);
        uvs.push([progress, 0.0]);
        uvs.push([progress, 1.0]);
    }
    let normals = vec![[0.0, 0.0, 1.0]; positions.len()];
    let mut indices = Vec::with_capacity(usize::from(ARC_SEGMENTS) * 6);
    for segment in 0..ARC_SEGMENTS {
        let inner = segment * 2;
        let outer = inner + 1;
        let next_inner = inner + 2;
        let next_outer = inner + 3;
        indices.extend([inner, outer, next_outer, inner, next_outer, next_inner]);
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U16(indices));
    mesh
}

fn melee_slash_inner_radius(progress: f32) -> f32 {
    let distance_from_crest = (progress * 2.0 - 1.0).abs();
    INNER_RADIUS_AT_CREST
        + (INNER_RADIUS_AT_TIP - INNER_RADIUS_AT_CREST) * distance_from_crest.powf(1.65)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn melee_arc_cache_reuses_tenth_degree_geometry() {
        let mut cache = MeleeArcMeshCache::default();
        let mut meshes = Assets::<Mesh>::default();
        let first = cache.resolve(100.01, &mut meshes);
        let second = cache.resolve(100.04, &mut meshes);
        assert_eq!(first, second);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn melee_arc_cache_stays_bounded() {
        let mut cache = MeleeArcMeshCache::default();
        let mut meshes = Assets::<Mesh>::default();
        for angle in 1_u16..=80 {
            cache.resolve(f32::from(angle), &mut meshes);
        }
        assert_eq!(cache.len(), MAX_MELEE_ARC_MESHES);
    }

    #[test]
    fn melee_arc_tapers_to_sharp_tips_around_a_broad_cutting_crest() {
        assert!((melee_slash_inner_radius(0.5) - INNER_RADIUS_AT_CREST).abs() < f32::EPSILON);
        assert!((melee_slash_inner_radius(0.0) - INNER_RADIUS_AT_TIP).abs() < f32::EPSILON);
        assert!((melee_slash_inner_radius(1.0) - INNER_RADIUS_AT_TIP).abs() < f32::EPSILON);
        assert!(melee_slash_inner_radius(0.25) > INNER_RADIUS_AT_CREST);
        assert_eq!(
            melee_slash_inner_radius(0.25).to_bits(),
            melee_slash_inner_radius(0.75).to_bits()
        );
    }
}
