//! Touch-target floor for shell controls (BRL-q148).
//!
//! On a touch platform every shell control is at least [`MIN_TOUCH_TARGET_DP`] tall at every
//! UI scale. Owners keep authoring their desktop heights; a `PostUpdate` pass raises
//! `Node::min_height` to `48 / UiScale` UI pixels (one logical pixel is one dp on Android) and
//! restores the authored value when the policy is off, so desktop layouts never move.

use bevy::{
    prelude::*,
    ui::{UiScale, UiSystems},
};

/// Smallest comfortable touch target height in density-independent pixels.
pub(crate) const MIN_TOUCH_TARGET_DP: f32 = 48.0;

/// Whether the shell enforces the touch-target floor. The touch platform is the Android build;
/// a touch seen on a desktop window does not reflow the shell mid-session. Tests set it directly.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TouchTargetPolicy {
    pub(crate) enabled: bool,
}

/// The touch platform: the Android build (Pixel), the only shipped touch-first target.
const TOUCH_PLATFORM: bool = cfg!(target_os = "android");

impl Default for TouchTargetPolicy {
    fn default() -> Self {
        Self {
            enabled: TOUCH_PLATFORM,
        }
    }
}

/// Marks a shell control whose height is floored on touch platforms. It remembers the owner's
/// authored `min_height`, so owners may keep rewriting their node (responsive layouts) and the
/// floor follows.
#[derive(Component, Clone, Copy, Debug, Default)]
pub(crate) struct TouchTarget {
    authored: Option<Val>,
    applied: Option<Val>,
}

struct TouchTargetPlugin;

impl Plugin for TouchTargetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TouchTargetPolicy>().add_systems(
            PostUpdate,
            apply_touch_target_floor.before(UiSystems::Layout),
        );
    }
}

/// Installs the floor once for every shell owner that composes it.
pub(crate) fn add_touch_targets(app: &mut App) {
    if !app.is_plugin_added::<TouchTargetPlugin>() {
        app.add_plugins(TouchTargetPlugin);
    }
}

/// UI-pixel floor for the current scale, or `None` when the policy is off.
fn touch_floor_px(policy: TouchTargetPolicy, scale: f32) -> Option<f32> {
    policy
        .enabled
        .then(|| MIN_TOUCH_TARGET_DP / scale.max(0.01))
}

fn floored(authored: Val, floor: Option<f32>) -> Val {
    match (authored, floor) {
        (_, None) => authored,
        (Val::Px(value), Some(floor)) => Val::Px(value.max(floor)),
        (Val::Auto, Some(floor)) => Val::Px(floor),
        // Relative minimums stay owner-defined; shell controls author pixel or automatic heights.
        (other, Some(_)) => other,
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn apply_touch_target_floor(
    policy: Res<TouchTargetPolicy>,
    scale: Option<Res<UiScale>>,
    mut targets: Query<(&mut Node, &mut TouchTarget)>,
) {
    let floor = touch_floor_px(*policy, scale.map_or(1.0, |scale| scale.0));
    for (mut node, mut target) in &mut targets {
        if target.applied != Some(node.min_height) {
            target.authored = Some(node.min_height);
        }
        let desired = floored(target.authored.unwrap_or(node.min_height), floor);
        if node.min_height != desired {
            node.min_height = desired;
        }
        if target.applied != Some(desired) {
            target.applied = Some(desired);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(enabled: bool, scale: f32) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(UiScale(scale))
            .insert_resource(TouchTargetPolicy { enabled });
        add_touch_targets(&mut app);
        app
    }

    #[test]
    fn floor_raises_short_controls_and_keeps_taller_ones() {
        let mut app = app(true, 0.5);
        let short = app
            .world_mut()
            .spawn((
                TouchTarget::default(),
                Node {
                    min_height: px(40),
                    ..default()
                },
            ))
            .id();
        let tall = app
            .world_mut()
            .spawn((
                TouchTarget::default(),
                Node {
                    min_height: px(140),
                    ..default()
                },
            ))
            .id();
        let auto = app
            .world_mut()
            .spawn((TouchTarget::default(), Node::default()))
            .id();
        app.update();
        let height = |app: &App, entity| app.world().get::<Node>(entity).unwrap().min_height;
        assert_eq!(height(&app, short), px(96));
        assert_eq!(height(&app, tall), px(140));
        assert_eq!(height(&app, auto), px(96));

        // The floor follows the scale and an owner rewriting its authored height.
        app.world_mut().resource_mut::<UiScale>().0 = 1.0;
        app.world_mut().get_mut::<Node>(tall).unwrap().min_height = px(20);
        app.update();
        assert_eq!(height(&app, short), px(48));
        assert_eq!(height(&app, tall), px(48));

        // Turning the policy off restores every authored height.
        app.world_mut().resource_mut::<TouchTargetPolicy>().enabled = false;
        app.update();
        assert_eq!(height(&app, short), px(40));
        assert_eq!(height(&app, tall), px(20));
        assert_eq!(height(&app, auto), Val::Auto);
    }

    #[test]
    fn desktop_policy_leaves_authored_heights_untouched() {
        let mut app = app(false, 0.8);
        let entity = app
            .world_mut()
            .spawn((
                TouchTarget::default(),
                Node {
                    min_height: px(38),
                    ..default()
                },
            ))
            .id();
        app.update();
        assert_eq!(app.world().get::<Node>(entity).unwrap().min_height, px(38));
        assert_eq!(
            TouchTargetPolicy::default().enabled,
            cfg!(target_os = "android")
        );
    }
}
