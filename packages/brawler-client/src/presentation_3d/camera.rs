//! Per-axis map/viewport framing, the arena camera writer, and ground-plane cursor projection.

use super::camera_rig::{
    ArenaCameraShake, CameraRig, CameraRigCatalog, CameraRigFrameInput, weapon_reach,
};
use super::combat::{OVERHEAD_AMMO_HEIGHT, OVERHEAD_WORLD_HEIGHT};
use super::{
    ArenaCamera, ClientShellSettings, Controlled, DEFAULT_GAMEPLAY_VIEWPORT,
    DEFAULT_GAMEPLAY_WINDOW_ASPECT, Fighter, MapEditorPreview, PendingLocalActions, Position,
    PrimaryWindow, TargetedUltimateInput, ground_point, ground_position, hud,
    targeted_ultimate_range,
};
use bevy::light::{CascadeShadowConfig, CascadeShadowConfigBuilder};
use bevy::prelude::*;

/// Vertical field of view at the reference (default native) aspect and every narrower one.
pub(super) const CAMERA_VERTICAL_FOV_RADIANS: f32 = 30.0_f32.to_radians();
pub(super) const CAMERA_ELEVATION_RADIANS: f32 = 55.0_f32.to_radians();
/// Keeps approximately twelve map cells visible vertically (BRL-0148).
pub(super) const CAMERA_DISTANCE: f32 = 566.0;

/// UI-unit gap between the top HUD score plate and the local fighter's overhead (BRL-fwtg).
const TOP_HUD_OVERHEAD_GAP: f32 = 6.0;

/// Margin past the farthest visible ground so the top screen edge never sits on the cascade's
/// hard cutoff (camera shake is at most a few units; one map cell is ample).
const SHADOW_FAR_MARGIN_WORLD: f32 = brawler::map::MAP_CELL_SIZE_WORLD;

/// View-space depth of the farthest visible gameplay ground at the framing distance. Wider
/// viewports narrow the vertical field of view, so the reference vertical view is the maximum.
fn farthest_visible_ground_depth() -> f32 {
    let half_fov = CAMERA_VERTICAL_FOV_RADIANS * 0.5;
    let camera_height = CAMERA_DISTANCE * CAMERA_ELEVATION_RADIANS.sin();
    // The top screen edge's ray descends at `elevation - half_fov`; its view-space depth is the
    // ray length projected on the view axis.
    camera_height / (CAMERA_ELEVATION_RADIANS - half_fov).sin() * half_fov.cos()
}

/// The arena light's cascade layout for the gameplay view (BRL-0180). Bevy's default (4 cascades
/// ending 150 units from the camera) never reached the ground, which starts roughly 480 view
/// units away. One cascade spanning the whole visible depth keeps texels well under one world
/// unit at the default 2048 map size, because the far-plane diagonal sets the cascade size and
/// the minimum distance does not; the near bound stays at Bevy's default so nothing between the
/// camera and the ground loses its shadow.
pub(super) fn arena_shadow_cascades() -> CascadeShadowConfig {
    CascadeShadowConfigBuilder {
        num_cascades: 1,
        maximum_distance: farthest_visible_ground_depth() + SHADOW_FAR_MARGIN_WORLD,
        ..default()
    }
    .build()
}

/// Vertical field of view for a viewport aspect. Narrower viewports keep the reference vertical
/// view and see less width; wider ones keep the reference horizontal view and see less depth, so
/// no window shape sees more of the arena than the reference aspect on either axis.
pub(super) fn camera_vertical_fov(aspect: f32) -> f32 {
    let aspect = valid_aspect(aspect);
    if aspect <= DEFAULT_GAMEPLAY_WINDOW_ASPECT {
        CAMERA_VERTICAL_FOV_RADIANS
    } else {
        2.0 * ((CAMERA_VERTICAL_FOV_RADIANS * 0.5).tan() * DEFAULT_GAMEPLAY_WINDOW_ASPECT / aspect)
            .atan()
    }
}

/// Intersect a viewport ray with the sole gameplay ground plane (`Y = 0`).
pub(crate) fn cursor_ground_point(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    cursor: Vec2,
) -> Option<Vec2> {
    let ray = camera.viewport_to_world(camera_transform, cursor).ok()?;
    ray.plane_intersection_point(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))
        .map(ground_point)
}

/// Presentation inputs of the camera rig: frame time, rig state and tuning, the local aim
/// sample, and the motion/effects accessibility settings.
#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct CameraRigParams<'w> {
    time: Res<'w, Time>,
    rig: ResMut<'w, CameraRig>,
    catalog: Res<'w, CameraRigCatalog>,
    pending: Option<Res<'w, PendingLocalActions>>,
    settings: Option<Res<'w, ClientShellSettings>>,
    ui_scale: Option<Res<'w, UiScale>>,
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "the camera follows the one controlled replicated fighter"
)]
pub(super) fn follow_3d_camera(
    map: Option<Res<brawler::map::PresentedMap>>,
    editor: Option<Res<MapEditorPreview>>,
    fighters: Query<
        (&Position, Option<&brawler::builds::ResolvedMatchLoadout>),
        (With<Fighter>, With<Controlled>, Without<ArenaCamera>),
    >,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<
        (
            &mut Transform,
            &mut Projection,
            Option<&mut ArenaCameraShake>,
        ),
        With<ArenaCamera>,
    >,
    mut rig: CameraRigParams,
) {
    let viewport = windows
        .iter()
        .next()
        .map_or(DEFAULT_GAMEPLAY_VIEWPORT, |window| {
            Vec2::new(window.width(), window.height())
        });
    let fov = camera_vertical_fov(viewport_aspect(viewport));
    for (_, mut projection, _) in &mut cameras {
        if let Projection::Perspective(perspective) = projection.bypass_change_detection()
            && (perspective.fov - fov).abs() > f32::EPSILON
        {
            perspective.fov = fov;
            projection.set_changed();
        }
    }
    if let Some(editor) = editor {
        let bounds = editor.snapshot.dimensions.bounds();
        let footprint = conservative_ground_footprint(viewport_aspect(viewport));
        let fit_scale = (bounds.size().x / footprint.size().x)
            .max(bounds.size().y / footprint.size().y)
            .max(0.25)
            * 1.12;
        let distance = CAMERA_DISTANCE * fit_scale * editor.camera_zoom.clamp(0.5, 3.0);
        let center = editor.camera_center;
        let target = ground_position(center);
        let offset = camera_offset() * (distance / CAMERA_DISTANCE);
        // The editor camera is direct; leaving the editor snaps the gameplay rig.
        rig.rig.reset();
        for (mut transform, _, shake) in &mut cameras {
            *transform = Transform::from_translation(target + offset).looking_at(target, Vec3::Y);
            if let Some(mut shake) = shake {
                shake.0 = Vec3::ZERO;
            }
        }
        return;
    }
    let controlled = fighters.iter().next();
    let (bounds, fighter) = map.as_ref().map_or_else(
        || {
            let neutral = brawler::map::AxisAlignedMapRect {
                min: Vec2::ZERO,
                max: Vec2::ZERO,
            };
            (neutral, None)
        },
        |map| {
            (
                map.camera_bounds,
                controlled.map(|(position, _)| position.0),
            )
        },
    );
    let settings = rig.settings.as_deref().copied().unwrap_or_default();
    let aim = rig.pending.as_deref().and_then(|pending| {
        pending
            .aim_axis
            .map(|direction| (direction, pending.aim_distance))
    });
    let reach = controlled
        .and_then(|(_, loadout)| loadout)
        .zip(rig.pending.as_deref())
        .map(|(loadout, pending)| aim_reach(loadout, pending.targeted_ultimate));
    let dt = rig.time.delta_secs();
    let catalog = *rig.catalog;
    let frame = rig.rig.step(
        &catalog,
        CameraRigFrameInput {
            fighter,
            aim,
            reach,
            bounds,
            viewport,
            dt,
            reduced_motion: settings.reduced_motion,
            reduced_effects: settings.reduced_combat_effects,
            shake_scale: settings.camera_shake_scale(),
            ui_scale: rig.ui_scale.as_deref().map_or(1.0, |scale| scale.0),
        },
    );
    let target = ground_position(frame.center);
    let shake = ground_position(frame.shake);
    let offset = camera_offset();
    for (mut transform, _, shake_component) in &mut cameras {
        *transform = Transform::from_translation(target + offset + shake)
            .looking_at(target + shake, Vec3::Y);
        if let Some(mut shake_component) = shake_component {
            shake_component.0 = shake;
        }
    }
}

/// Reach of what the local aim commands: an armed targeted ultimate's placement range, otherwise
/// the primary weapon's reach.
fn aim_reach(
    loadout: &brawler::builds::ResolvedMatchLoadout,
    targeted_ultimate: TargetedUltimateInput,
) -> f32 {
    targeted_ultimate
        .is_targeting(loadout.ultimate.id)
        .then(|| targeted_ultimate_range(loadout))
        .flatten()
        .unwrap_or_else(|| weapon_reach(&loadout.primary_weapon.recipe))
}

/// Camera position relative to its ground look target at the framing distance.
pub(super) fn camera_offset() -> Vec3 {
    Vec3::new(
        0.0,
        CAMERA_ELEVATION_RADIANS.sin(),
        CAMERA_ELEVATION_RADIANS.cos(),
    ) * CAMERA_DISTANCE
}

pub(super) fn viewport_aspect(viewport: Vec2) -> f32 {
    if viewport.is_finite() && viewport.x > 0.0 && viewport.y > 0.0 {
        viewport.x / viewport.y
    } else {
        DEFAULT_GAMEPLAY_WINDOW_ASPECT
    }
}

fn valid_aspect(aspect: f32) -> f32 {
    if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        DEFAULT_GAMEPLAY_WINDOW_ASPECT
    }
}

pub(super) fn clamp_3d_camera_center(
    position: Vec2,
    bounds: brawler::map::AxisAlignedMapRect,
    viewport: Vec2,
) -> Vec2 {
    let footprint = conservative_ground_footprint(viewport_aspect(viewport));
    let min = bounds.min - footprint.min;
    let max = bounds.max - footprint.max;
    Vec2::new(
        if min.x > max.x {
            f32::midpoint(bounds.min.x, bounds.max.x)
        } else {
            position.x.clamp(min.x, max.x)
        },
        if min.y > max.y {
            f32::midpoint(bounds.min.y, bounds.max.y)
        } else {
            position.y.clamp(min.y, max.y)
        },
    )
}

pub(super) fn conservative_ground_footprint(aspect: f32) -> brawler::map::AxisAlignedMapRect {
    let view = ground_view(aspect);
    // A tilted perspective view is a trapezoid on the ground. Axis-fit decisions must use its
    // narrow near edge so a map judged to fit cannot be clipped lower in the viewport.
    brawler::map::AxisAlignedMapRect {
        min: Vec2::new(-view.near_half_width, view.near_y),
        max: Vec2::new(view.near_half_width, view.far_y),
    }
}

/// The visible ground relative to the look target: a trapezoid whose rows run from `near_y`
/// (bottom screen edge, negative) to `far_y` (top screen edge), widening linearly from
/// `near_half_width` to `far_half_width`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct GroundView {
    pub(super) near_y: f32,
    pub(super) far_y: f32,
    pub(super) near_half_width: f32,
    pub(super) far_half_width: f32,
}

impl GroundView {
    /// Increase of the visible half width per unit of ground depth.
    fn width_slope(self) -> f32 {
        (self.far_half_width - self.near_half_width) / (self.far_y - self.near_y)
    }

    /// Visible half width on the ground row `y` units above the look target.
    pub(super) fn half_width_at(self, y: f32) -> f32 {
        self.near_half_width + (y - self.near_y) * self.width_slope()
    }

    /// Whether a ground offset from the look target lies at least `margin` inside every edge.
    #[cfg(test)]
    pub(super) fn contains(self, offset: Vec2, margin: f32) -> bool {
        offset.y >= self.near_y + margin
            && offset.y <= self.far_y - margin
            && offset.x.abs() <= self.half_width_at(offset.y) - margin
    }

    /// Distance from the look target to the edge shrunk by `margin` along a unit direction
    /// (zero when the look target itself is inside the margin).
    pub(super) fn ray_extent(self, direction: Vec2, margin: f32) -> f32 {
        let near = (self.near_y + margin).min(0.0);
        let far = (self.far_y - margin).max(0.0);
        let half_width = (self.half_width_at(0.0) - margin).max(0.0);
        let mut extent = f32::INFINITY;
        if direction.y > f32::EPSILON {
            extent = extent.min(far / direction.y);
        } else if direction.y < -f32::EPSILON {
            extent = extent.min(near / direction.y);
        }
        // `|t * dx| <= half_width + slope * t * dy` along the ray.
        let closing = direction.x.abs() - self.width_slope() * direction.y;
        if closing > f32::EPSILON {
            extent = extent.min(half_width / closing);
        }
        extent.max(0.0)
    }

    /// Move a ground offset from the look target to the nearest point at least `margin` inside
    /// the view: depth first, then width on that row.
    pub(super) fn clamp(self, offset: Vec2, margin: f32) -> Vec2 {
        let y = offset.y.clamp(
            (self.near_y + margin).min(0.0),
            (self.far_y - margin).max(0.0),
        );
        let half_width = (self.half_width_at(y) - margin).max(0.0);
        Vec2::new(offset.x.clamp(-half_width, half_width), y)
    }
}

/// Farthest ground offset ahead of the look target (toward the top of the screen) at which the
/// local fighter's overhead still sits below the top-center HUD score plate (BRL-fwtg). The band
/// is the plate's bottom edge, a small gap, and the overhead's own height, all UI units scaled by
/// the effective `UiScale` (the phone viewport fit times the player's UI-scale setting).
pub(super) fn top_hud_fighter_limit(viewport: Vec2, ui_scale: f32) -> f32 {
    let height = if viewport.is_finite() && viewport.y > 0.0 {
        viewport.y
    } else {
        DEFAULT_GAMEPLAY_VIEWPORT.y
    };
    let scale = if ui_scale.is_finite() && ui_scale > 0.0 {
        ui_scale
    } else {
        1.0
    };
    let band = (hud::HUD_PLATE_BOTTOM + TOP_HUD_OVERHEAD_GAP + OVERHEAD_AMMO_HEIGHT) * scale;
    let sin_elevation = CAMERA_ELEVATION_RADIANS.sin();
    let cos_elevation = CAMERA_ELEVATION_RADIANS.cos();
    let vertical_tangent = (camera_vertical_fov(viewport_aspect(viewport)) * 0.5).tan();
    // A point `y` ahead of the look target and `h` above the ground sits `h cos + y sin` above
    // the view axis at depth `D - h sin + y cos`; its screen row is that ratio over the tangent.
    // Solve for the `y` whose overhead anchor lands `band` pixels below the top edge.
    let limit = (1.0 - 2.0 * band / height) * vertical_tangent;
    let anchor = OVERHEAD_WORLD_HEIGHT;
    (limit * (CAMERA_DISTANCE - anchor * sin_elevation) - anchor * cos_elevation)
        / (sin_elevation - limit * cos_elevation)
}

pub(super) fn ground_view(aspect: f32) -> GroundView {
    let aspect = valid_aspect(aspect);
    let sin_elevation = CAMERA_ELEVATION_RADIANS.sin();
    let cos_elevation = CAMERA_ELEVATION_RADIANS.cos();
    let camera_height = CAMERA_DISTANCE * sin_elevation;
    let camera_ground_distance = CAMERA_DISTANCE * cos_elevation;
    let vertical_tangent = (camera_vertical_fov(aspect) * 0.5).tan();
    let horizontal_tangent = vertical_tangent * aspect;
    let far_parameter = camera_height / (sin_elevation - vertical_tangent * cos_elevation);
    let near_parameter = camera_height / (sin_elevation + vertical_tangent * cos_elevation);
    let far_y =
        far_parameter * (cos_elevation + vertical_tangent * sin_elevation) - camera_ground_distance;
    let near_y = near_parameter * (cos_elevation - vertical_tangent * sin_elevation)
        - camera_ground_distance;

    GroundView {
        near_y,
        far_y,
        near_half_width: near_parameter * horizontal_tangent,
        far_half_width: far_parameter * horizontal_tangent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui;

    const EPSILON: f32 = 0.02;

    fn centered_map(width_cells: f32, height_cells: f32) -> brawler::map::AxisAlignedMapRect {
        let half_size =
            Vec2::new(width_cells, height_cells) * brawler::map::MAP_CELL_SIZE_WORLD * 0.5;
        brawler::map::AxisAlignedMapRect {
            min: -half_size,
            max: half_size,
        }
    }

    fn visible_ground_bounds(center: Vec2, viewport: Vec2) -> brawler::map::AxisAlignedMapRect {
        let footprint = conservative_ground_footprint(viewport.x / viewport.y);
        brawler::map::AxisAlignedMapRect {
            min: center + footprint.min,
            max: center + footprint.max,
        }
    }

    fn assert_approximately(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < EPSILON,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn reference_view_matches_the_twelve_cell_target() {
        let footprint = conservative_ground_footprint(DEFAULT_GAMEPLAY_WINDOW_ASPECT);
        let visible_cells = footprint.size() / brawler::map::MAP_CELL_SIZE_WORLD;

        assert!((visible_cells.y - 12.0).abs() < 0.02, "{visible_cells}");
        assert!((visible_cells.x - 17.64).abs() < 0.02, "{visible_cells}");
        // The fighter sits nearer the bottom edge: ~4.9 cells behind, ~7.1 cells ahead.
        assert!((footprint.min.y / brawler::map::MAP_CELL_SIZE_WORLD + 4.87).abs() < 0.02);
        assert!((footprint.max.y / brawler::map::MAP_CELL_SIZE_WORLD - 7.12).abs() < 0.02);
    }

    #[test]
    fn arena_shadow_cascade_covers_the_visible_ground() {
        // Measure the far ground edge from the footprint and the real camera pose rather than
        // reusing the closed form under test.
        let camera = Transform::from_translation(camera_offset()).looking_at(Vec3::ZERO, Vec3::Y);
        let forward = camera.forward().as_vec3();
        let depth = |ground: Vec2| (ground_position(ground) - camera.translation).dot(forward);
        let reference = conservative_ground_footprint(DEFAULT_GAMEPLAY_WINDOW_ASPECT);
        let far_depth = depth(Vec2::new(0.0, reference.max.y));
        let near_depth = depth(Vec2::new(0.0, reference.min.y));
        assert_approximately(farthest_visible_ground_depth(), far_depth);
        // BRL-0148 framing: the ground spans roughly 475-700 view units.
        assert!((470.0..490.0).contains(&near_depth), "{near_depth}");
        assert!((690.0..705.0).contains(&far_depth), "{far_depth}");

        let cascades = arena_shadow_cascades();
        assert_eq!(cascades.bounds.len(), 1);
        assert!(cascades.minimum_distance < near_depth);
        for aspect in [
            4.0 / 3.0,
            16.0 / 9.0,
            DEFAULT_GAMEPLAY_WINDOW_ASPECT,
            32.0 / 9.0,
        ] {
            let footprint = conservative_ground_footprint(aspect);
            assert!(
                cascades.bounds[0] > depth(Vec2::new(0.0, footprint.max.y)),
                "{aspect}"
            );
        }
    }

    #[test]
    fn top_hud_limit_puts_the_local_overhead_just_below_the_score_plate() {
        // Project through Bevy's own camera pose and perspective rather than the closed form.
        let camera = Transform::from_translation(camera_offset()).looking_at(Vec3::ZERO, Vec3::Y);
        let view_from_world = camera.to_matrix().inverse();
        for viewport in [
            Vec2::new(1280.0, 720.0),
            Vec2::new(914.0, 411.0),
            Vec2::new(868.0, 411.0),
            Vec2::new(1920.0, 1080.0),
            Vec2::new(2560.0, 720.0),
        ] {
            let aspect = viewport_aspect(viewport);
            let clip_from_view =
                Mat4::perspective_rh(camera_vertical_fov(aspect), aspect, 1.0, 5_000.0);
            let fit = ui::viewport::viewport_fit(viewport.y);
            for setting in [0.8, 1.0, 1.4] {
                let scale = fit * setting;
                let limit = top_hud_fighter_limit(viewport, scale);
                let anchor =
                    ground_position(Vec2::new(0.0, limit)) + Vec3::Y * OVERHEAD_WORLD_HEIGHT;
                let ndc = (clip_from_view * view_from_world).project_point3(anchor);
                let overhead_top = (1.0 - ndc.y) * 0.5 * viewport.y - OVERHEAD_AMMO_HEIGHT * scale;
                let plate_bottom = hud::HUD_PLATE_BOTTOM * scale;
                assert_approximately(overhead_top, plate_bottom + TOP_HUD_OVERHEAD_GAP * scale);
                // The band costs real view depth but leaves an idle fighter (at the look target)
                // clear at the default scale, so open-field framing never needs it.
                let view = ground_view(aspect);
                assert!(limit < view.far_y, "{viewport} {setting}");
                if (setting - 1.0).abs() < f32::EPSILON {
                    assert!(limit > 0.0, "{viewport}: {limit}");
                }
            }
        }
        // A larger UI reserves more of the screen.
        let phone = Vec2::new(914.0, 411.0);
        assert!(top_hud_fighter_limit(phone, 0.9) < top_hud_fighter_limit(phone, 0.6));
        assert!(top_hud_fighter_limit(phone, f32::NAN).is_finite());
    }

    #[test]
    fn standard_pulse_range_fits_beside_the_fighter_at_the_reference_aspect() {
        // At the look target the view is wider than the trapezoid's conservative near edge.
        let half_width_at_target = CAMERA_DISTANCE
            * (camera_vertical_fov(DEFAULT_GAMEPLAY_WINDOW_ASPECT) * 0.5).tan()
            * DEFAULT_GAMEPLAY_WINDOW_ASPECT;
        // Standard Pulse Sidearm reach (`content/catalogs/weapons.ron`).
        assert!(half_width_at_target >= 320.0, "{half_width_at_target}");
    }

    #[test]
    fn no_viewport_shape_sees_more_than_the_reference_aspect() {
        let reference = conservative_ground_footprint(DEFAULT_GAMEPLAY_WINDOW_ASPECT);
        let reference_horizontal =
            (CAMERA_VERTICAL_FOV_RADIANS * 0.5).tan() * DEFAULT_GAMEPLAY_WINDOW_ASPECT;
        for aspect in [4.0 / 3.0, 16.0 / 9.0, 21.0 / 9.0, 32.0 / 9.0, 8.0] {
            let fov = camera_vertical_fov(aspect);
            let horizontal = (fov * 0.5).tan() * aspect;
            let footprint = conservative_ground_footprint(aspect);
            assert!(fov <= CAMERA_VERTICAL_FOV_RADIANS + 1e-6);
            assert!(horizontal <= reference_horizontal + 1e-5, "{aspect}");
            assert!(footprint.min.y >= reference.min.y - EPSILON, "{aspect}");
            assert!(footprint.max.y <= reference.max.y + EPSILON, "{aspect}");
            if aspect < DEFAULT_GAMEPLAY_WINDOW_ASPECT {
                // Narrower windows keep the reference depth and lose width.
                assert_approximately(footprint.min.y, reference.min.y);
                assert_approximately(footprint.max.y, reference.max.y);
                assert!(footprint.size().x < reference.size().x);
            } else {
                // Wider windows keep the reference width at the look target and lose depth.
                assert!((horizontal - reference_horizontal).abs() < 1e-5, "{aspect}");
                assert!(footprint.size().y < reference.size().y);
            }
        }
        assert!(
            (camera_vertical_fov(f32::NAN) - camera_vertical_fov(DEFAULT_GAMEPLAY_WINDOW_ASPECT))
                .abs()
                < 1e-6
        );
    }

    #[test]
    fn narrow_map_centers_width_and_follows_height() {
        let bounds = centered_map(16.0, 37.0);
        let viewport = DEFAULT_GAMEPLAY_VIEWPORT;
        let player = Vec2::new(150.0, 300.0);
        let center = clamp_3d_camera_center(player, bounds, viewport);
        let visible = visible_ground_bounds(center, viewport);

        assert_approximately(center.x, bounds.center().x);
        assert_approximately(center.y, player.y);
        assert!(visible.min.x <= bounds.min.x);
        assert!(visible.max.x >= bounds.max.x);
    }

    #[test]
    fn production_map_follows_both_axes_and_clamps_at_the_perimeter() {
        let bounds = centered_map(25.0, 37.0);
        let viewport = DEFAULT_GAMEPLAY_VIEWPORT;
        let near_center = Vec2::new(60.0, 100.0);
        assert_eq!(
            clamp_3d_camera_center(near_center, bounds, viewport),
            near_center
        );

        let corner = clamp_3d_camera_center(bounds.max, bounds, viewport);
        let visible = visible_ground_bounds(corner, viewport);
        assert_approximately(visible.max.x, bounds.max.x);
        assert_approximately(visible.max.y, bounds.max.y);
    }

    #[test]
    fn larger_map_follows_the_player_on_both_axes() {
        let bounds = centered_map(50.0, 50.0);
        let viewport = DEFAULT_GAMEPLAY_VIEWPORT;
        let player = Vec2::new(300.0, 300.0);
        let center = clamp_3d_camera_center(player, bounds, viewport);

        assert_eq!(center, player);
    }

    #[test]
    fn player_remains_inside_view_when_follow_is_clamped_at_every_edge() {
        let bounds = centered_map(50.0, 50.0);
        let viewport = DEFAULT_GAMEPLAY_VIEWPORT;

        for player in [
            Vec2::new(bounds.min.x, 0.0),
            Vec2::new(bounds.max.x, 0.0),
            Vec2::new(0.0, bounds.min.y),
            Vec2::new(0.0, bounds.max.y),
        ] {
            let center = clamp_3d_camera_center(player, bounds, viewport);
            let visible = visible_ground_bounds(center, viewport);
            assert!(player.x >= visible.min.x - EPSILON);
            assert!(player.x <= visible.max.x + EPSILON);
            assert!(player.y >= visible.min.y - EPSILON);
            assert!(player.y <= visible.max.y + EPSILON);
        }
    }

    #[test]
    fn resizing_recomputes_fit_vs_follow_per_axis() {
        let bounds = centered_map(16.0, 37.0);
        let player = Vec2::new(20.0, 0.0);
        let reference = clamp_3d_camera_center(player, bounds, DEFAULT_GAMEPLAY_VIEWPORT);
        let narrow = clamp_3d_camera_center(player, bounds, Vec2::new(16.0, 9.0));

        assert_approximately(reference.x, bounds.center().x);
        assert_approximately(narrow.x, player.x);
    }
}
