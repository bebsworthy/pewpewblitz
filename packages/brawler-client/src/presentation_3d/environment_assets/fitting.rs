use super::MapVisualFitting;
use bevy::{
    camera::primitives::Aabb,
    math::Affine3A,
    prelude::{ChildOf, Entity, Mesh3d, Quat, Transform, Vec2, Vec3, World},
    world_serialization::WorldAsset,
};
use std::collections::BTreeSet;

const EXACT_ASPECT_TOLERANCE: f32 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SceneBounds {
    pub(super) min: Vec3,
    pub(super) max: Vec3,
}

impl SceneBounds {
    fn dimensions(self) -> Vec3 {
        self.max - self.min
    }

    fn validate(self) -> Result<Self, String> {
        let dimensions = self.dimensions();
        if !self.min.is_finite()
            || !self.max.is_finite()
            || !dimensions.is_finite()
            || dimensions.cmple(Vec3::ZERO).any()
        {
            return Err("imported scene has empty or non-finite bounds".to_string());
        }
        Ok(self)
    }
}

pub(super) fn world_asset_scene_bounds(asset: &WorldAsset) -> Result<SceneBounds, String> {
    let mut minimum = Vec3::splat(f32::INFINITY);
    let mut maximum = Vec3::splat(f32::NEG_INFINITY);
    let mut found = false;
    for entity in asset.world.iter_entities() {
        if !entity.contains::<Mesh3d>() {
            continue;
        }
        let Some(aabb) = entity.get::<Aabb>() else {
            return Err("imported scene mesh is missing its intrinsic AABB".to_string());
        };
        let transform = entity_world_affine(&asset.world, entity.id())?;
        for corner in aabb_corners(*aabb) {
            let point = transform.transform_point3(corner);
            minimum = minimum.min(point);
            maximum = maximum.max(point);
            found = true;
        }
    }
    if !found {
        return Err("imported scene contains no bounded mesh".to_string());
    }
    SceneBounds {
        min: minimum,
        max: maximum,
    }
    .validate()
}

fn entity_world_affine(world: &World, entity: Entity) -> Result<Affine3A, String> {
    let mut current = entity;
    let mut chain = Vec::new();
    let mut visited = BTreeSet::new();
    loop {
        if !visited.insert(current) {
            return Err("imported scene hierarchy contains a cycle".to_string());
        }
        let transform = world.get::<Transform>(current).copied().unwrap_or_default();
        chain.push(transform.compute_affine());
        let Some(parent) = world.get::<ChildOf>(current) else {
            break;
        };
        current = parent.parent();
    }
    Ok(chain
        .into_iter()
        .rev()
        .fold(Affine3A::IDENTITY, |global, local| global * local))
}

fn aabb_corners(aabb: Aabb) -> [Vec3; 8] {
    let min = Vec3::from(aabb.min());
    let max = Vec3::from(aabb.max());
    [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(min.x, max.y, max.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(max.x, max.y, min.z),
        max,
    ]
}

pub(super) fn fit_scene_to_footprint(
    bounds: SceneBounds,
    fitting: MapVisualFitting,
    fill: f32,
    footprint_world: Vec2,
    yaw_radians: f32,
    vertical_offset: f32,
) -> Result<Transform, String> {
    let bounds = bounds.validate()?;
    if !fill.is_finite()
        || !(0.0..=1.0).contains(&fill)
        || fill == 0.0
        || !footprint_world.is_finite()
        || footprint_world.cmple(Vec2::ZERO).any()
        || !yaw_radians.is_finite()
        || !vertical_offset.is_finite()
    {
        return Err("invalid imported-scene fitting inputs".to_string());
    }
    if fitting == MapVisualFitting::Tiled {
        return Err("imported tiled fitting is not supported".to_string());
    }

    let rotation = Quat::from_rotation_y(yaw_radians);
    let mut rotated_min = Vec3::splat(f32::INFINITY);
    let mut rotated_max = Vec3::splat(f32::NEG_INFINITY);
    for corner in bounds_corners(bounds) {
        let point = rotation * corner;
        rotated_min = rotated_min.min(point);
        rotated_max = rotated_max.max(point);
    }
    let dimensions = rotated_max - rotated_min;
    let scale_x = footprint_world.x / dimensions.x;
    let scale_z = footprint_world.y / dimensions.z;
    if fitting == MapVisualFitting::Exact {
        let relative_delta = (scale_x - scale_z).abs() / scale_x.max(scale_z);
        if relative_delta > EXACT_ASPECT_TOLERANCE || (fill - 1.0).abs() > f32::EPSILON {
            return Err("exact imported scene does not match the authoritative aspect".to_string());
        }
    }
    let uniform_scale = scale_x.min(scale_z) * fill;
    let rotated_center = (rotated_min + rotated_max) * 0.5;
    Ok(Transform {
        translation: Vec3::new(
            -rotated_center.x * uniform_scale,
            -rotated_min.y * uniform_scale + vertical_offset,
            -rotated_center.z * uniform_scale,
        ),
        rotation,
        scale: Vec3::splat(uniform_scale),
    })
}

fn bounds_corners(bounds: SceneBounds) -> [Vec3; 8] {
    let min = bounds.min;
    let max = bounds.max;
    [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(min.x, max.y, max.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(max.x, max.y, min.z),
        max,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hierarchy_bounds_include_nested_transforms_and_off_center_meshes() {
        let mut world = World::new();
        world
            .spawn(Transform::from_translation(Vec3::new(3.0, 4.0, 5.0)))
            .with_children(|parent| {
                parent.spawn((
                    Transform::from_scale(Vec3::new(2.0, 3.0, 4.0)),
                    Mesh3d::default(),
                    Aabb::from_min_max(Vec3::new(-1.0, 0.0, -0.5), Vec3::new(1.0, 2.0, 0.5)),
                ));
            });
        let bounds = world_asset_scene_bounds(&WorldAsset::new(world)).unwrap();

        assert!(bounds.min.abs_diff_eq(Vec3::new(1.0, 4.0, 3.0), 1e-5));
        assert!(bounds.max.abs_diff_eq(Vec3::new(5.0, 10.0, 7.0), 1e-5));
    }

    #[test]
    fn hierarchy_bounds_compose_parent_rotation_before_mesh_bounds() {
        let mut world = World::new();
        world
            .spawn(
                Transform::from_translation(Vec3::new(4.0, 0.0, 0.0))
                    .with_rotation(Quat::from_rotation_y(core::f32::consts::FRAC_PI_2)),
            )
            .with_children(|parent| {
                parent.spawn((
                    Mesh3d::default(),
                    Aabb::from_min_max(Vec3::ZERO, Vec3::new(2.0, 1.0, 1.0)),
                ));
            });
        let bounds = world_asset_scene_bounds(&WorldAsset::new(world)).unwrap();

        assert!(bounds.min.abs_diff_eq(Vec3::new(4.0, 0.0, -2.0), 1e-5));
        assert!(bounds.max.abs_diff_eq(Vec3::new(5.0, 1.0, 0.0), 1e-5));
    }

    #[test]
    fn exact_block_fits_one_cell_and_grounds_centered_pivot() {
        let transform = fit_scene_to_footprint(
            SceneBounds {
                min: Vec3::splat(-1.0),
                max: Vec3::splat(1.0),
            },
            MapVisualFitting::Exact,
            1.0,
            Vec2::splat(32.0),
            0.0,
            0.0,
        )
        .unwrap();

        assert!(transform.scale.abs_diff_eq(Vec3::splat(16.0), 1e-5));
        assert!(transform.translation.abs_diff_eq(Vec3::Y * 16.0, 1e-5));
    }

    #[test]
    fn contained_scene_is_recentred_grounded_and_cannot_overflow() {
        let transform = fit_scene_to_footprint(
            SceneBounds {
                min: Vec3::new(-0.25, -0.5, -1.0),
                max: Vec3::new(0.75, 1.5, 1.0),
            },
            MapVisualFitting::Contained,
            0.75,
            Vec2::splat(32.0),
            0.0,
            2.0,
        )
        .unwrap();

        assert!(transform.scale.abs_diff_eq(Vec3::splat(12.0), 1e-5));
        assert!(
            transform
                .translation
                .abs_diff_eq(Vec3::new(-3.0, 8.0, 0.0), 1e-5)
        );
    }

    #[test]
    fn exact_fit_rejects_mismatched_aspect_and_imported_tiling() {
        let bounds = SceneBounds {
            min: Vec3::new(-1.0, 0.0, -0.5),
            max: Vec3::new(1.0, 1.0, 0.5),
        };
        assert!(
            fit_scene_to_footprint(
                bounds,
                MapVisualFitting::Exact,
                1.0,
                Vec2::splat(32.0),
                0.0,
                0.0
            )
            .is_err()
        );
        assert!(
            fit_scene_to_footprint(
                bounds,
                MapVisualFitting::Tiled,
                1.0,
                Vec2::splat(32.0),
                0.0,
                0.0
            )
            .is_err()
        );
    }

    #[test]
    fn exact_fit_accounts_for_profile_yaw_before_matching_the_footprint() {
        let transform = fit_scene_to_footprint(
            SceneBounds {
                min: Vec3::new(-0.5, 0.0, -1.0),
                max: Vec3::new(0.5, 1.0, 1.0),
            },
            MapVisualFitting::Exact,
            1.0,
            Vec2::new(64.0, 32.0),
            core::f32::consts::FRAC_PI_2,
            0.0,
        )
        .unwrap();

        assert!(transform.scale.abs_diff_eq(Vec3::splat(32.0), 1e-5));
        assert!(transform.translation.abs_diff_eq(Vec3::ZERO, 1e-5));
    }

    #[test]
    fn empty_scene_and_non_finite_inputs_fail_closed() {
        assert!(world_asset_scene_bounds(&WorldAsset::new(World::new())).is_err());
        assert!(
            fit_scene_to_footprint(
                SceneBounds {
                    min: Vec3::ZERO,
                    max: Vec3::ONE,
                },
                MapVisualFitting::Contained,
                f32::NAN,
                Vec2::splat(32.0),
                0.0,
                0.0,
            )
            .is_err()
        );
    }
}
