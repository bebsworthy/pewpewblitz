//! Gentle sway of concealing foliage (BRL-0153): a pivot at the cell's ground centre tilts the
//! imported bush a few degrees with a per-cell phase, so the whole bush bends about its base.

use super::*;

/// Peak tilt; small enough that the bush never leaves its authoritative cell.
const SWAY_RADIANS: f32 = 0.045;
/// Cycles per second of the slow primary bend.
const SWAY_HZ: f32 = 0.32;

/// Pivot between an imported foliage root and its scene. Its rotation is owned by
/// [`sway_foliage`]; the parent keeps the placement rotation.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(in crate::client::presentation_3d) struct FoliageSway {
    phase: f32,
}

impl FoliageSway {
    /// Neighbouring cells get different phases so a patch of bushes ripples instead of pulsing.
    pub(in crate::client::presentation_3d) fn at(center: Vec2) -> Self {
        Self {
            phase: (center.x * 0.071 + center.y * 0.043).rem_euclid(core::f32::consts::TAU),
        }
    }

    fn rotation(self, seconds: f32) -> Quat {
        let angle = seconds * SWAY_HZ * core::f32::consts::TAU + self.phase;
        Quat::from_rotation_x(angle.sin() * SWAY_RADIANS)
            * Quat::from_rotation_z((angle * 0.73 + 1.1).sin() * SWAY_RADIANS * 0.6)
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are owned by the scheduling runtime"
)]
pub(in crate::client::presentation_3d) fn sway_foliage(
    time: Res<Time>,
    settings: Option<Res<ClientShellSettings>>,
    quality: Option<Res<render_quality::ActiveRenderQuality>>,
    mut pivots: Query<(&FoliageSway, &mut Transform)>,
) {
    let still = settings.is_some_and(|settings| settings.reduced_combat_effects)
        || !render_quality::foliage_sway_enabled(quality.as_deref());
    let seconds = time.elapsed_secs();
    for (sway, mut transform) in &mut pivots {
        let rotation = if still {
            Quat::IDENTITY
        } else {
            sway.rotation(seconds)
        };
        // A still bush stays unchanged, so it costs no transform propagation.
        if transform.rotation != rotation {
            transform.rotation = rotation;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sway_stays_a_small_bend_and_differs_between_neighbours() {
        let a = FoliageSway::at(Vec2::new(16.0, 16.0));
        let b = FoliageSway::at(Vec2::new(48.0, 16.0));
        assert!((a.phase - b.phase).abs() > 0.5);
        for step in 0..200_u16 {
            let rotation = a.rotation(f32::from(step) * 0.05);
            let tilted_up = rotation * Vec3::Y;
            assert!(tilted_up.angle_between(Vec3::Y) <= SWAY_RADIANS * 1.7);
        }
    }

    #[test]
    fn reduced_effects_hold_foliage_still() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .insert_resource(ClientShellSettings {
                reduced_combat_effects: true,
                ..default()
            })
            .add_systems(Update, sway_foliage);
        let pivot = app
            .world_mut()
            .spawn((
                FoliageSway::at(Vec2::splat(40.0)),
                Transform::from_rotation(Quat::from_rotation_x(0.3)),
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Transform>(pivot).unwrap().rotation,
            Quat::IDENTITY
        );
    }
}
