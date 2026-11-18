//! Standalone development surface; never installed in the player application.

use super::{
    controls::{self, ControlState, ControlValue, spawn_control},
    scroll::{clamp_scroll_offset, normalized_wheel_delta, offset_keeping_interval_visible},
    surface::Surface,
    theme,
};
use bevy::input::mouse::MouseWheel;

mod affordance_board;
use bevy::{
    prelude::*,
    ui::{InteractionDisabled, ScrollPosition, UiScale, UiSystems},
    window::PrimaryWindow,
};

/// Builds the native component review tool without gameplay, networking or settings persistence.
pub fn build_ui_gallery_app() -> App {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: format!("{}/assets", crate::REPOSITORY_ROOT),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "PewPew Blitz — UI Gallery".into(),
                    // BRL-0196: the Pixel 8a landscape logical viewport.
                    resolution: if let Some(size) = gallery_size_override() {
                        size.into()
                    } else if std::env::var_os("BRAWLER_UI_GALLERY_PHONE").is_some() {
                        (914, 411).into()
                    } else if std::env::var_os("BRAWLER_UI_GALLERY_COMPACT").is_some() {
                        (900, 600).into()
                    } else {
                        (1440, 900).into()
                    },
                    ..default()
                }),
                ..default()
            }),
    )
    .insert_resource(ClearColor(theme::SHELL_BG_BOTTOM))
    .init_resource::<GalleryFocus>()
    .add_plugins((
        controls::UiControlsPlugin,
        super::surface::SurfacePlugin,
        super::press::ReleaseActivationPlugin,
    ))
    .add_systems(
        Startup,
        (
            setup.run_if(not(resource_exists::<GalleryBoard>)),
            setup_board.run_if(resource_exists::<GalleryBoard>),
        ),
    )
    .add_systems(
        Update,
        (
            affordance_board::cycle_affordance,
            fit_gallery_viewport,
            interact,
            scroll_gallery,
            resize_gallery,
            sync_motion_demo,
            (replay_burst_demo, hold_burst_peak_demo).after(interact),
        ),
    )
    .add_systems(PreUpdate, hold_press_demo.after(UiSystems::Focus))
    .add_systems(PostUpdate, reveal_focus.after(UiSystems::Layout));
    let board = std::env::var_os("BRAWLER_UI_GALLERY_AFFORDANCE_BOARD").is_some();
    if board {
        app.insert_resource(GalleryBoard);
    }
    if let Some(path) = std::env::var_os("BRAWLER_UI_GALLERY_CAPTURE") {
        app.insert_resource(GalleryCapture {
            path: path.into(),
            frame: 0,
            single: board,
        })
        .add_systems(Last, capture_gallery);
    }
    app
}

/// `BRAWLER_UI_GALLERY_SIZE=WIDTHxHEIGHT` opens the gallery at an exact logical size.
fn gallery_size_override() -> Option<(u32, u32)> {
    let value = std::env::var("BRAWLER_UI_GALLERY_SIZE").ok()?;
    let (width, height) = value.split_once('x')?;
    Some((width.parse().ok()?, height.parse().ok()?))
}

/// The gallery shows the BRL-fe7t focus/selection affordance board instead of the component
/// matrix (`BRAWLER_UI_GALLERY_AFFORDANCE_BOARD=1`).
#[derive(Resource)]
struct GalleryBoard;

/// Applies the shell's small-viewport fit, as the player client does (BRL-0196).
fn fit_gallery_viewport(windows: Query<&Window, With<PrimaryWindow>>, mut scale: ResMut<UiScale>) {
    let fit = windows
        .iter()
        .next()
        .map_or(1.0, |window| super::viewport::viewport_fit(window.height()));
    if (scale.0 - fit).abs() > f32::EPSILON {
        scale.0 = fit;
    }
}

/// Development evidence: save the settled top of the gallery to `BRAWLER_UI_GALLERY_CAPTURE`,
/// then a scrolled middle (`<stem>-middle.png`) and bottom (`<stem>-bottom.png`) beside it, then
/// exit.
#[derive(Resource)]
struct GalleryCapture {
    path: std::path::PathBuf,
    frame: u32,
    /// Save only the top (the affordance board fits one screen).
    single: bool,
}

const GALLERY_CAPTURE_FRAME: u32 = 120;

fn capture_gallery(
    mut commands: Commands,
    mut capture: ResMut<GalleryCapture>,
    mut roots: Query<&mut ScrollPosition, With<GalleryRoot>>,
    mut exit: MessageWriter<AppExit>,
) {
    use bevy::render::view::screenshot::{Screenshot, save_to_disk};
    capture.frame += 1;
    let sibling = |suffix: &str| {
        capture.path.with_file_name(format!(
            "{}-{suffix}.png",
            capture
                .path
                .file_stem()
                .map_or("gallery".into(), |stem| stem.to_string_lossy())
        ))
    };
    let mut scroll_to = |offset: f32| {
        for mut scroll in &mut roots {
            scroll.0.y = offset;
        }
    };
    match capture.frame.checked_sub(GALLERY_CAPTURE_FRAME) {
        Some(0) => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(capture.path.clone()));
        }
        Some(30) if capture.single => {
            exit.write(AppExit::Success);
        }
        // The middle shows the held-press and reward-burst references.
        Some(10) if !capture.single => scroll_to(GALLERY_MIDDLE_SCROLL),
        Some(40) => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(sibling("middle")));
        }
        Some(50) => scroll_to(10_000.0),
        Some(80) => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(sibling("bottom")));
        }
        Some(110) => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}

/// Logical scroll offset for the middle evidence capture.
const GALLERY_MIDDLE_SCROLL: f32 = 700.0;

/// Reference-only state: keeps one example at the peak of its reward burst.
#[derive(Component)]
struct BurstPeakDemo;

fn hold_burst_peak_demo(mut commands: Commands, held: Query<Entity, With<BurstPeakDemo>>) {
    for entity in &held {
        commands
            .entity(entity)
            .insert(super::surface::SurfaceBurst::AT_PEAK);
    }
}

/// The interactive burst example replays the production burst on each activation.
#[derive(Component)]
struct BurstDemo;

#[allow(clippy::needless_pass_by_value, reason = "isolated gallery input")]
fn replay_burst_demo(
    mut commands: Commands,
    demos: Query<(Entity, &super::press::ReleaseActivation), With<BurstDemo>>,
    keys: Res<ButtonInput<KeyCode>>,
    focus: Res<GalleryFocus>,
    items: Query<&GalleryItem, With<BurstDemo>>,
) {
    let keyed = keys.just_pressed(KeyCode::Enter) && items.iter().any(|item| item.0 == focus.index);
    for (entity, release) in &demos {
        if release.activated() || keyed {
            commands
                .entity(entity)
                .insert(super::surface::SurfaceBurst::default());
        }
    }
}

/// Reference-only state: keeps one example visibly held, as if under a pointer.
#[derive(Component)]
struct HeldPressDemo;

fn hold_press_demo(mut held: Query<&mut Interaction, With<HeldPressDemo>>) {
    for mut interaction in &mut held {
        interaction.set_if_neq(Interaction::Pressed);
    }
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
#[require(super::press::ReleaseActivation)]
struct GalleryItem(usize);

/// Camera and the fixed backdrop with the shared motif layer (edges layout) behind the scrolling
/// content; returns the gallery's display font.
fn spawn_backdrop(commands: &mut Commands, assets: &AssetServer) -> TextFont {
    commands.spawn(Camera2d);
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        theme::shell_background(),
        super::motifs::ShellBackdrop(super::motifs::MotifLayout::Edges),
        GlobalZIndex(-1),
    ));
    TextFont {
        font: assets.load(theme::FONT_PATH).into(),
        font_size: FontSize::Px(24.0),
        ..default()
    }
}

#[allow(clippy::needless_pass_by_value, reason = "Bevy gallery setup")]
fn setup_board(mut commands: Commands, assets: Res<AssetServer>) {
    let font = spawn_backdrop(&mut commands, &assets);
    commands
        .spawn((
            GalleryRoot,
            ScrollPosition::default(),
            Node {
                width: percent(100),
                height: percent(100),
                overflow: Overflow::scroll_y(),
                ..default()
            },
        ))
        .with_children(|root| affordance_board::spawn(root, &font));
}

#[allow(clippy::needless_pass_by_value, reason = "Bevy gallery setup")]
fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    let font = spawn_backdrop(&mut commands, &assets);
    commands.spawn((GalleryRoot, ScrollPosition::default(), Node {
        width: percent(100), height: percent(100), padding: UiRect::all(px(32)),
        flex_direction: FlexDirection::Column, row_gap: px(24), overflow: Overflow::scroll_y(), ..default()
    })).with_children(|root| {
        root.spawn((Text::new("UI GALLERY"), TextFont { font_size: FontSize::Px(42.0), ..font.clone() }, TextColor(theme::TEXT)));
        root.spawn((Text::new("Development preview · Up/Down or D-pad: focus · Left/Right: adjust · Enter/South: activate"),
            TextFont { font_size: FontSize::Px(18.0), ..font.clone() }, TextColor(theme::TEXT)));
        root.spawn((GalleryColumns, Node { width: percent(100), column_gap: px(32), row_gap: px(32), flex_shrink: 0.0, ..default() })).with_children(|columns| {
            columns.spawn((GalleryColumn, column())).with_children(|left| {
                heading(left, &font, "INTERACTIVE CONTROLS");
                let controls = [
                    ("Measure stick drift", ControlValue::Action),
                    ("Accessibility", ControlValue::Tab),
                    ("Reduced motion", ControlValue::Toggle(false)),
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
                    (Surface::Primary, "PLAY", 96.),
                    (Surface::Secondary, "PRACTICE", 72.),
                    (Surface::Card, "BRAWLER CARD\nPulse Sidearm", 220.),
                    (Surface::Panel, "WEAPON OVERVIEW\nFour attachment slots", 150.),
                    (Surface::Quiet, "RENAME", 64.),
                    (Surface::Card, "REWARD BURST\nActivate to replay", 120.),
                ].into_iter().enumerate() {
                    let entity=surface_example(left,&font,role,label,height);
                    left.commands().entity(entity).insert((GalleryItem(offset+5),ControlValue::Action,ControlState::default()));
                    if role == Surface::Primary {
                        // The Dashboard Play treatment: breathing and periodic shine.
                        left.commands().entity(entity).insert(super::surface::PrimaryIdleMotion);
                    }
                    if label.starts_with("REWARD BURST") {
                        left.commands().entity(entity).insert(BurstDemo);
                    }
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
                    ("Focused play action", ControlState{focused:true,..default()}, false),
                    ("Disabled action", ControlState::default(), true),
                    ("Inactive selected card", ControlState{selected:true,focused:true,inactive:true}, false),
                    ("Held press (squash, lip collapse)", ControlState::default(), false),
                    ("Reward burst at peak (pop, shine, flash)", ControlState::default(), false),
                ] {
                    let entity=surface_example(right,&font,Surface::Primary,label,100.);
                    right.commands().entity(entity).insert(state);
                    if disabled { right.commands().entity(entity).insert(InteractionDisabled); }
                    if label.starts_with("Held press") {
                        right.commands().entity(entity).insert(HeldPressDemo);
                    }
                    if label.starts_with("Reward burst") {
                        right.commands().entity(entity).insert(BurstPeakDemo);
                    }
                }
            });
        });
        icon_set(root, &font, &assets);
    });
}

/// The BRL-0157 icon set and BRL-1wms mode pennants at shipped sizes on the surfaces that host
/// them.
fn icon_set(parent: &mut ChildSpawnerCommands, font: &TextFont, assets: &AssetServer) {
    heading(parent, font, "ICON SET");
    let framed: Vec<_> = super::icons::BADGES
        .into_iter()
        .chain(super::icons::PARTS)
        .collect();
    for (role, size) in [
        (Surface::Primary, 36.0),
        (Surface::Secondary, 28.0),
        (Surface::Dark, 22.0),
    ] {
        icon_strip(parent, assets, role, &super::icons::PICTOGRAMS, size, size);
    }
    icon_strip(parent, assets, Surface::Card, &framed, 120.0, 100.0);
    // Mode pennants at their Dashboard (Panel) and game-selection (Card) sizes (BRL-1wms).
    let pennants = &super::icons::MODE_PENNANTS;
    icon_strip(parent, assets, Surface::Panel, pennants, 64.0, 80.0);
    icon_strip(parent, assets, Surface::Card, pennants, 140.0, 174.0);
}

fn icon_strip(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    role: Surface,
    names: &[&str],
    width: f32,
    height: f32,
) {
    parent
        .spawn((
            role,
            super::surface::SurfaceState::default(),
            Node {
                width: percent(100),
                flex_shrink: 0.,
                flex_wrap: FlexWrap::Wrap,
                column_gap: px(14),
                row_gap: px(12),
                padding: UiRect::axes(px(32), px(20)),
                ..default()
            },
        ))
        .with_children(|strip| {
            for name in names {
                strip.spawn((
                    ImageNode::new(assets.load(super::icons::path(name))),
                    Node {
                        width: px(width),
                        height: px(height),
                        ..default()
                    },
                ));
            }
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
    parent.spawn((Text::new(label), font.clone(), TextColor(theme::ACCENT)));
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "isolated gallery input, no product actions"
)]
fn interact(
    keys: Res<ButtonInput<KeyCode>>,
    pointer: super::slider::SliderPointer,
    pads: Query<&Gamepad>,
    mut focus: ResMut<GalleryFocus>,
    mut controls: Query<(
        &GalleryItem,
        &Interaction,
        &super::press::ReleaseActivation,
        &mut ControlState,
        &mut ControlValue,
    )>,
) {
    for adjust in pointer.adjustments() {
        if let Ok((item, _, _, _, mut value)) = controls.get_mut(adjust.owner)
            && let ControlValue::Slider { fraction, label } = &mut *value
        {
            *fraction = adjust.fraction;
            *label = format!("{:.0}%", 80.0 + *fraction * 40.0);
            focus.index = item.0;
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
    for (item, interaction, _, _, _) in &mut controls {
        if *interaction == Interaction::Pressed {
            focus.index = item.0;
        }
    }
    for (item, _, release, mut state, mut value) in &mut controls {
        state.focused = item.0 == focus.index;
        if !state.focused {
            continue;
        }
        let clicked = release.activated();
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

/// The demo Reduced motion switch drives the production surface motion policy.
fn sync_motion_demo(
    values: Query<(&GalleryItem, &ControlValue)>,
    mut policy: ResMut<super::surface::SurfaceMotionPolicy>,
) {
    for (item, value) in &values {
        if item.0 == REDUCED_MOTION_ITEM
            && let ControlValue::Toggle(reduced) = value
            && policy.reduced != *reduced
        {
            policy.reduced = *reduced;
        }
    }
}

const REDUCED_MOTION_ITEM: usize = 2;

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
            TextColor(theme::TEXT),
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
    use super::super::controls::SliderTrack;
    use super::*;
    use bevy::ui::RelativeCursorPosition;

    #[test]
    fn short_track_click_adjusts_without_a_held_frame() {
        let mut app = App::new();
        let mut mouse = ButtonInput::<MouseButton>::default();
        mouse.press(MouseButton::Left);
        mouse.release(MouseButton::Left);
        app.insert_resource(mouse)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<GalleryFocus>()
            .init_resource::<super::super::slider::SliderTouch>()
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
