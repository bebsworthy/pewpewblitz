//! Standalone development surface; never installed in the player application.

use super::{
    controls::{self, ControlState, ControlValue, SliderTrack, spawn_control},
    scroll::{clamp_scroll_offset, normalized_wheel_delta, offset_keeping_interval_visible},
    surface::Surface,
    theme,
};
use bevy::input::mouse::MouseWheel;
use bevy::{
    prelude::*,
    ui::{InteractionDisabled, RelativeCursorPosition, ScrollPosition, UiSystems},
};

/// Builds the native component review tool without gameplay, networking or settings persistence.
pub fn build_ui_gallery_app() -> App {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: format!("{}/assets", env!("CARGO_MANIFEST_DIR")),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "PewPew Blitz — UI Gallery".into(),
                    resolution: if std::env::var_os("BRAWLER_UI_GALLERY_COMPACT").is_some() {
                        (900, 600).into()
                    } else {
                        (1440, 900).into()
                    },
                    ..default()
                }),
                ..default()
            }),
    )
    .insert_resource(ClearColor(theme::CREAM))
    .init_resource::<GalleryFocus>()
    .add_plugins((controls::UiControlsPlugin, super::surface::SurfacePlugin))
    .add_systems(Startup, setup)
    .add_systems(Update, (interact, scroll_gallery, resize_gallery))
    .add_systems(PostUpdate, reveal_focus.after(UiSystems::Layout));
    app
}

#[derive(Resource, Default)]
struct GalleryFocus {
    index: usize,
    stick_held: bool,
}

#[derive(Component)]
struct GalleryRoot;

#[derive(Component)]
struct GalleryColumns;

#[derive(Component)]
struct GalleryColumn;

#[derive(Component)]
struct GalleryItem(usize);

#[allow(clippy::needless_pass_by_value, reason = "Bevy gallery setup")]
fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);
    let font = TextFont {
        font: assets.load(theme::FONT_PATH).into(),
        font_size: FontSize::Px(24.0),
        ..default()
    };
    commands.spawn((GalleryRoot, ScrollPosition::default(), Node {
        width: percent(100), height: percent(100), padding: UiRect::all(px(32)),
        flex_direction: FlexDirection::Column, row_gap: px(24), overflow: Overflow::scroll_y(), ..default()
    }, BackgroundColor(theme::CREAM))).with_children(|root| {
        root.spawn((Text::new("UI GALLERY"), TextFont { font_size: FontSize::Px(42.0), ..font.clone() }, TextColor(theme::INK)));
        root.spawn((Text::new("Development preview · Up/Down or D-pad: focus · Left/Right: adjust · Enter/South: activate"),
            TextFont { font_size: FontSize::Px(18.0), ..font.clone() }, TextColor(theme::INK)));
        root.spawn((GalleryColumns, Node { width: percent(100), column_gap: px(32), row_gap: px(32), flex_shrink: 0.0, ..default() })).with_children(|columns| {
            columns.spawn((GalleryColumn, column())).with_children(|left| {
                heading(left, &font, "INTERACTIVE CONTROLS");
                let controls = [
                    ("Measure stick drift", ControlValue::Action),
                    ("Accessibility", ControlValue::Tab),
                    ("Reduced motion", ControlValue::Toggle(true)),
                    ("UI scale", ControlValue::Slider { fraction: 0.5, label: "100%".into() }),
                    ("Active item", ControlValue::Binding("Q".into())),
                ];
                for (index, (label, value)) in controls.into_iter().enumerate() {
                    let entity = spawn_control(left, &font, label, value);
                    left.commands().entity(entity).insert(GalleryItem(index));
                }
                heading(left, &font, "SHELL COMPONENTS");
                for (offset, (role, label, height)) in [
                    (Surface::Back, "BACK TO BRAWLERS", 64.),
                    (Surface::Primary, "SELECT FOR PLAY", 96.),
                    (Surface::Secondary, "PRACTICE", 72.),
                    (Surface::Card, "BRAWLER CARD\nPulse Sidearm", 220.),
                    (Surface::Panel, "WEAPON OVERVIEW\nFour attachment slots", 150.),
                    (Surface::Quiet, "RENAME", 64.),
                ].into_iter().enumerate() {
                    let entity=surface_example(left,&font,role,label,height);
                    left.commands().entity(entity).insert((GalleryItem(offset+5),ControlValue::Action,ControlState::default()));
                }
            });
            columns.spawn((GalleryColumn, column())).with_children(|right| {
                heading(right, &font, "STATE REFERENCE");
                spawn_control(right, &font, "Normal", ControlValue::Action);
                let selected = spawn_control(right, &font, "Selected category", ControlValue::Tab);
                right.commands().entity(selected).insert(ControlState { selected: true, ..default() });
                let focused = spawn_control(right, &font, "Navigation focus", ControlValue::Action);
                right.commands().entity(focused).insert(ControlState { focused: true, ..default() });
                let disabled = spawn_control(right, &font, "Disabled", ControlValue::Action);
                right.commands().entity(disabled).insert(InteractionDisabled);
                let inactive = spawn_control(right, &font, "Inactive surface", ControlValue::Tab);
                right.commands().entity(inactive).insert(ControlState { selected: true, focused: true, inactive: true });
                heading(right, &font, "SHARED SURFACE STATES");
                for (label, state, disabled) in [
                    ("Selected and focused", ControlState{selected:true,focused:true,..default()}, false),
                    ("Focused orange action", ControlState{focused:true,..default()}, false),
                    ("Disabled action", ControlState::default(), true),
                    ("Inactive selected card", ControlState{selected:true,focused:true,inactive:true}, false),
                ] {
                    let entity=surface_example(right,&font,Surface::Primary,label,100.);
                    right.commands().entity(entity).insert(state);
                    if disabled { right.commands().entity(entity).insert(InteractionDisabled); }
                }
            });
        });
    });
}

fn column() -> Node {
    Node {
        width: percent(50),
        min_width: px(0),
        flex_direction: FlexDirection::Column,
        row_gap: px(16),
        ..default()
    }
}

fn heading(parent: &mut ChildSpawnerCommands, font: &TextFont, label: &str) {
    parent.spawn((Text::new(label), font.clone(), TextColor(theme::TEAL)));
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "isolated gallery input, no product actions"
)]
fn interact(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    tracks: Query<(&SliderTrack, &RelativeCursorPosition)>,
    pads: Query<&Gamepad>,
    mut focus: ResMut<GalleryFocus>,
    mut controls: Query<(
        &GalleryItem,
        Ref<Interaction>,
        &mut ControlState,
        &mut ControlValue,
    )>,
) {
    if mouse.pressed(MouseButton::Left)
        || mouse.just_pressed(MouseButton::Left)
        || mouse.just_released(MouseButton::Left)
    {
        for (track, cursor) in &tracks {
            if let Some(position) = controls::slider_fraction(cursor)
                && let Ok((item, _, _, mut value)) = controls.get_mut(track.0)
                && let ControlValue::Slider { fraction, label } = &mut *value
            {
                *fraction = position;
                *label = format!("{:.0}%", 80.0 + *fraction * 40.0);
                focus.index = item.0;
            }
        }
    }
    let pressed = |button| pads.iter().any(|pad| pad.just_pressed(button));
    let stick = pads
        .iter()
        .map(Gamepad::left_stick)
        .find(|v| v.length() > 0.6)
        .unwrap_or(Vec2::ZERO);
    let new_stick = !focus.stick_held;
    focus.stick_held = stick != Vec2::ZERO;
    if keys.just_pressed(KeyCode::ArrowDown)
        || pressed(GamepadButton::DPadDown)
        || (new_stick && stick.y < -0.6)
    {
        focus.index = (focus.index + 1) % controls.iter().count().max(1);
    }
    if keys.just_pressed(KeyCode::ArrowUp)
        || pressed(GamepadButton::DPadUp)
        || (new_stick && stick.y > 0.6)
    {
        let count = controls.iter().count().max(1);
        focus.index = (focus.index + count - 1) % count;
    }
    let direction =
        i8::from(keys.just_pressed(KeyCode::ArrowRight) || pressed(GamepadButton::DPadRight))
            - i8::from(keys.just_pressed(KeyCode::ArrowLeft) || pressed(GamepadButton::DPadLeft));
    let activate = keys.just_pressed(KeyCode::Enter) || pressed(GamepadButton::South);
    for (item, interaction, _, _) in &mut controls {
        if *interaction == Interaction::Pressed {
            focus.index = item.0;
        }
    }
    for (item, interaction, mut state, mut value) in &mut controls {
        state.focused = item.0 == focus.index;
        if !state.focused {
            continue;
        }
        let clicked = interaction.is_changed() && *interaction == Interaction::Pressed;
        if direction != 0 || activate || clicked {
            match &mut *value {
                ControlValue::Toggle(on) if direction != 0 || activate || clicked => *on = !*on,
                ControlValue::Tab if activate || clicked => state.selected = !state.selected,
                ControlValue::Slider { fraction, label } if direction != 0 => {
                    *fraction = (*fraction + f32::from(direction) * 0.1).clamp(0.0, 1.0);
                    *label = format!("{:.0}%", 80.0 + *fraction * 40.0);
                }
                _ => {}
            }
        }
    }
}

#[allow(clippy::needless_pass_by_value, reason = "gallery responsive layout")]
fn resize_gallery(
    windows: Query<&Window>,
    mut groups: Query<&mut Node, (With<GalleryColumns>, Without<GalleryColumn>)>,
    mut columns: Query<&mut Node, (With<GalleryColumn>, Without<GalleryColumns>)>,
) {
    let narrow = windows
        .iter()
        .next()
        .is_some_and(|window| window.width() < 1000.0);
    for mut node in &mut groups {
        node.flex_direction = if narrow {
            FlexDirection::Column
        } else {
            FlexDirection::Row
        };
    }
    for mut node in &mut columns {
        node.width = percent(if narrow { 100 } else { 50 });
        node.flex_shrink = if narrow { 0.0 } else { 1.0 };
    }
}

#[allow(clippy::needless_pass_by_value, reason = "Bevy gallery scrolling")]
fn scroll_gallery(
    mut wheel: MessageReader<MouseWheel>,
    mut roots: Query<(&ComputedNode, &mut ScrollPosition), With<GalleryRoot>>,
) {
    let delta = normalized_wheel_delta(wheel.read(), 24.0);
    for (node, mut position) in &mut roots {
        position.0.y = clamp_scroll_offset(
            position.0.y - delta,
            node.content_size().y,
            node.size().y,
            node.inverse_scale_factor(),
        );
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "gallery observes completed Bevy layout"
)]
fn reveal_focus(
    focus: Res<GalleryFocus>,
    buttons: Query<(&GalleryItem, &ComputedNode, &UiGlobalTransform)>,
    mut roots: Query<(&ComputedNode, &UiGlobalTransform, &mut ScrollPosition), With<GalleryRoot>>,
    mut previous: Local<Option<(usize, UVec2)>>,
) {
    let Some(size) = roots
        .iter_mut()
        .next()
        .map(|(node, _, _)| node.size().as_uvec2())
    else {
        return;
    };
    let key = (focus.index, size);
    if *previous == Some(key) {
        return;
    }
    let Some((_, node, transform)) = buttons
        .iter()
        .find(|(item, node, _)| item.0 == focus.index && !node.is_empty())
    else {
        return;
    };
    *previous = Some(key);
    let (_, _, center) = transform.to_scale_angle_translation();
    for (root, transform, mut scroll) in &mut roots {
        let (_, _, root_center) = transform.to_scale_angle_translation();
        let offset = offset_keeping_interval_visible(
            scroll.0.y,
            root_center.y - root.size().y * 0.5..root_center.y + root.size().y * 0.5,
            center.y - node.size().y * 0.5..center.y + node.size().y * 0.5,
            root.inverse_scale_factor(),
        );
        scroll.0.y = clamp_scroll_offset(
            offset,
            root.content_size().y,
            root.size().y,
            root.inverse_scale_factor(),
        );
    }
}

fn surface_example(
    parent: &mut ChildSpawnerCommands,
    font: &TextFont,
    role: Surface,
    label: &str,
    height: f32,
) -> Entity {
    parent
        .spawn((
            Button,
            role,
            super::surface::SurfaceState::default(),
            Node {
                width: percent(100),
                height: px(height),
                flex_shrink: 0.,
                padding: UiRect::all(px(32)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_child((
            Text::new(label),
            font.clone(),
            TextColor(theme::INK),
            TextLayout::justify(Justify::Center),
            Node {
                width: percent(100),
                ..default()
            },
        ))
        .id()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_track_click_adjusts_without_a_held_frame() {
        let mut app = App::new();
        let mut mouse = ButtonInput::<MouseButton>::default();
        mouse.press(MouseButton::Left);
        mouse.release(MouseButton::Left);
        app.insert_resource(mouse)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<GalleryFocus>()
            .add_systems(Update, interact);
        let owner = app
            .world_mut()
            .spawn((
                GalleryItem(3),
                Interaction::None,
                ControlState::default(),
                ControlValue::Slider {
                    fraction: 0.5,
                    label: "100%".into(),
                },
            ))
            .id();
        app.world_mut().spawn((
            SliderTrack(owner),
            RelativeCursorPosition {
                cursor_over: true,
                normalized: Some(Vec2::new(0.25, 0.0)),
            },
        ));
        app.update();
        let ControlValue::Slider { fraction, label } =
            app.world().get::<ControlValue>(owner).unwrap()
        else {
            panic!("slider missing")
        };
        assert!((*fraction - 0.75).abs() < f32::EPSILON);
        assert_eq!(label, "110%");
        assert_eq!(app.world().resource::<GalleryFocus>().index, 3);
    }
}
