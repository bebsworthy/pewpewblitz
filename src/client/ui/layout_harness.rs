//! Test-only headless UI layout harness (BRL-0196): real Bevy UI/text layout against a primary
//! window of a chosen logical size, so screen tests can assert reachability on small viewports.

use bevy::{
    asset::AssetPlugin,
    camera::{Camera, Camera2d, ComputedCameraValues, RenderTargetInfo},
    image::ImagePlugin,
    input::InputPlugin,
    prelude::*,
    ui::{OverflowAxis, UiScale},
    window::{PrimaryWindow, WindowResolution},
};

/// Adds layout-capable plugins and a camera targeting a `width`x`height` logical window.
pub(crate) fn add_layout(app: &mut App, width: u32, height: u32) {
    if !app.is_plugin_added::<InputPlugin>() {
        app.add_plugins(InputPlugin);
    }
    app.add_plugins((
        AssetPlugin::default(),
        ImagePlugin::default(),
        bevy::text::TextPlugin,
        bevy::ui::UiPlugin,
    ));
    // Headless tests have no render visibility pass; approximate it from `Visibility` so UI
    // focus and the reachability check treat hidden nodes as the renderer would.
    app.add_systems(First, approximate_visibility);
    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(width, height).with_scale_factor_override(1.0),
            ..default()
        },
        PrimaryWindow,
    ));
    app.world_mut().spawn((
        Camera2d,
        Camera {
            computed: ComputedCameraValues {
                target_info: Some(RenderTargetInfo {
                    physical_size: UVec2::new(width, height),
                    scale_factor: 1.0,
                }),
                ..default()
            },
            ..default()
        },
    ));
    app.init_asset::<TextureAtlasLayout>();
    if !app.world().contains_resource::<UiScale>() {
        app.init_resource::<UiScale>();
    }
}

fn approximate_visibility(
    mut nodes: Query<(Entity, &mut InheritedVisibility), With<Node>>,
    visibility: Query<&Visibility>,
    parents: Query<&ChildOf>,
) {
    for (entity, mut inherited) in &mut nodes {
        let hidden = std::iter::once(entity)
            .chain(parents.iter_ancestors(entity))
            .any(|node| visibility.get(node).is_ok_and(|v| *v == Visibility::Hidden));
        let next = if hidden {
            InheritedVisibility::HIDDEN
        } else {
            InheritedVisibility::VISIBLE
        };
        if *inherited != next {
            *inherited = next;
        }
    }
}

/// Sets the effective UI scale the shell would choose for the primary window at the default
/// UI-scale setting.
pub(crate) fn apply_viewport_fit(app: &mut App) {
    let world = app.world_mut();
    let mut windows = world.query_filtered::<&Window, With<PrimaryWindow>>();
    let height = windows.single(world).expect("primary window").height();
    world.resource_mut::<UiScale>().0 = super::viewport::viewport_fit(height);
}

/// Interactive nodes that can never be brought fully on screen: neither inside the window nor
/// inside a vertical scroll area that is itself inside the window.
pub(crate) fn unreachable_buttons(world: &mut World) -> Vec<String> {
    let window = {
        let mut windows = world.query_filtered::<&Window, With<PrimaryWindow>>();
        let window = windows.single(world).expect("primary window");
        Rect::from_corners(Vec2::ZERO, window.physical_size().as_vec2())
    };
    let mut buttons = world.query_filtered::<(Entity, &InheritedVisibility), With<Button>>();
    let candidates: Vec<_> = buttons
        .iter(world)
        .filter(|(_, visible)| visible.get())
        .map(|(entity, _)| entity)
        .collect();
    assert!(!candidates.is_empty(), "no visible controls to check");
    let contains = |outer: Rect, inner: Rect| {
        inner.min.x >= outer.min.x - 0.5
            && inner.max.x <= outer.max.x + 0.5
            && inner.min.y >= outer.min.y - 0.5
            && inner.max.y <= outer.max.y + 0.5
    };
    let mut failures = Vec::new();
    for entity in candidates {
        let Some(rect) = node_rect(world, entity) else {
            continue;
        };
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            continue;
        }
        let scroll_area = std::iter::successors(world.get::<ChildOf>(entity), |child_of| {
            world.get::<ChildOf>(child_of.parent())
        })
        .map(ChildOf::parent)
        .find(|ancestor| {
            world
                .get::<Node>(*ancestor)
                .is_some_and(|node| node.overflow.y == OverflowAxis::Scroll)
        });
        let reachable = match scroll_area.and_then(|area| node_rect(world, area)) {
            // Scrolling moves the control vertically only: it must fit the area's width and
            // height, and the area itself must be on screen.
            Some(area) => {
                contains(window, area)
                    && rect.min.x >= area.min.x - 0.5
                    && rect.max.x <= area.max.x + 0.5
                    && rect.height() <= area.height() + 0.5
            }
            None => contains(window, rect),
        };
        if !reachable {
            let label = world
                .get::<AccessibleLabel>(entity)
                .map_or_else(|| format!("{entity:?}"), |label| label.0.clone());
            failures.push(format!("{label} at {rect:?}"));
        }
    }
    failures
}

/// Physical-pixel rectangle of a laid-out node.
pub(crate) fn node_rect(world: &World, entity: Entity) -> Option<Rect> {
    let node = world.get::<ComputedNode>(entity)?;
    let transform = world.get::<UiGlobalTransform>(entity)?;
    let (_, _, center) = transform.to_scale_angle_translation();
    Some(Rect::from_center_size(center, node.size()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harness_lays_out_text_in_a_small_window() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        add_layout(&mut app, 914, 411);
        let root = app
            .world_mut()
            .spawn(Node {
                width: percent(100),
                height: percent(100),
                ..default()
            })
            .with_child(Text::new("hello"))
            .id();
        app.update();
        app.update();
        let rect = node_rect(app.world(), root).unwrap();
        assert!((rect.width() - 914.0).abs() < 0.5, "{rect:?}");
        let text = app.world().get::<Children>(root).unwrap()[0];
        assert!(node_rect(app.world(), text).unwrap().width() > 10.0);
    }
}
