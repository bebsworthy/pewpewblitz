//! Functional windowed product shell: title, one overlay, focus, settings draft, and errors.

use super::{
    ClientInputContext, InputSettingsField, InputSettingsSelection, InputSettingsText,
    settings::{
        ClientInputSettings,
        persistence::{
            ClientSettingsPath, ClientShellSettings, MAX_UI_SCALE, MIN_UI_SCALE, load_settings,
            save_settings,
        },
        ui::InputSettingsDraft,
    },
};
use bevy::{
    input::mouse::{MouseScrollUnit, MouseWheel},
    input_focus::{
        FocusCause, InputFocus, InputFocusVisible,
        directional_navigation::{
            DirectionalNavigation, DirectionalNavigationMap, DirectionalNavigationPlugin,
        },
    },
    math::CompassOctant,
    prelude::*,
    ui::{Overflow, ScrollPosition, UiScale, UiSystems, UiTransform, Val2},
};

const ENTRANCE_SECONDS: f32 = 0.16;
const CREDITS: &str = "Brawler 0.1.0\n\nBuilt with Bevy 0.19 (MIT OR Apache-2.0).\nDefault Fira Mono font: Mozilla Foundation / Telefonica, SIL OFL 1.1.\nFighters and sounds: Kenney, CC0 1.0.\nFacility tiles: Murphy's Dad / HaywardMorihara, CC0 1.0.\n\nFull license texts ship in assets/licenses/.";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShellOverlay {
    #[default]
    None,
    Settings,
    Credits,
    LocalError,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ErrorReturn {
    Title,
    Settings,
}

#[derive(Resource, Debug)]
struct ShellState {
    overlay: ShellOverlay,
    error_return: ErrorReturn,
    error_message: String,
    settings_applied_before_error: bool,
}

impl Default for ShellState {
    fn default() -> Self {
        Self {
            overlay: ShellOverlay::None,
            error_return: ErrorReturn::Title,
            error_message: String::new(),
            settings_applied_before_error: false,
        }
    }
}

#[derive(Resource, Clone, Copy, Debug, PartialEq)]
struct ShellSettingsDraft(ClientShellSettings);

#[derive(Resource, Default)]
struct PendingActions(Vec<ShellAction>);

#[derive(Resource, Default)]
struct NavigationLatch {
    y_ready: bool,
}

#[derive(Resource, Default)]
struct NavigationDirty(Option<ShellControlId>);

#[derive(Component)]
struct TitleRoot;

#[derive(Component)]
struct OverlayRoot;

#[derive(Component)]
struct ShellScrollArea;

#[derive(Component)]
struct EntranceAnimation {
    elapsed: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShellLayer {
    Title,
    Settings,
    Credits,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShellControlId {
    Play,
    Practice,
    Settings,
    Credits,
    Quit,
    PreviousField,
    NextField,
    Decrease,
    Increase,
    Rebind,
    ToggleMoveY,
    ToggleAimY,
    UiScaleDown,
    ToggleReducedMotion,
    UiScaleUp,
    Reset,
    Apply,
    Cancel,
    CreditsBack,
    Retry,
    ContinueWithoutSaving,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShellAction {
    OpenSettings,
    OpenCredits,
    Quit,
    PreviousField,
    NextField,
    Decrease,
    Increase,
    Rebind,
    ToggleMoveY,
    ToggleAimY,
    UiScaleDown,
    ToggleReducedMotion,
    UiScaleUp,
    Reset,
    Apply,
    Cancel,
    Back,
    RetrySave,
    ContinueWithoutSaving,
}

#[derive(Component)]
struct ShellButton {
    id: ShellControlId,
    action: Option<ShellAction>,
    layer: ShellLayer,
}

/// Installed only by normal windowed startup. Explicit automation keeps its established arena
/// presentation and connection path without composing product-shell state.
pub struct ClientShellPlugin;

impl Plugin for ClientShellPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(DirectionalNavigationPlugin)
            .init_resource::<InputFocus>()
            .insert_resource(InputFocusVisible(true))
            .init_resource::<DirectionalNavigationMap>()
            .init_resource::<ShellState>()
            .init_resource::<ClientShellSettings>()
            .init_resource::<ClientSettingsPath>()
            .init_resource::<PendingActions>()
            .init_resource::<NavigationLatch>()
            .init_resource::<NavigationDirty>()
            .add_systems(
                Startup,
                (
                    load_persistent_settings,
                    spawn_initial_shell,
                    rebuild_navigation,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    collect_navigation_input,
                    collect_pointer_actions,
                    handle_shell_actions,
                    rebuild_navigation,
                    style_shell_buttons,
                    preview_shell_preferences,
                    animate_shell_entrance,
                    scroll_shell_panels,
                )
                    .chain(),
            )
            .add_systems(
                PostUpdate,
                keep_focused_control_visible.after(UiSystems::Layout),
            );
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn load_persistent_settings(
    path: Res<ClientSettingsPath>,
    mut input: ResMut<ClientInputSettings>,
    mut shell: ResMut<ClientShellSettings>,
    mut state: ResMut<ShellState>,
) {
    match load_settings(&path.0) {
        Ok(Some((loaded_input, loaded_shell))) => {
            *input = loaded_input;
            *shell = loaded_shell;
        }
        Ok(None) => {}
        Err(error) => {
            state.overlay = ShellOverlay::LocalError;
            state.error_return = ErrorReturn::Title;
            state.error_message = error.to_string();
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn spawn_initial_shell(
    mut commands: Commands,
    state: Res<ShellState>,
    mut context: ResMut<ClientInputContext>,
    mut dirty: ResMut<NavigationDirty>,
) {
    *context = ClientInputContext::Paused;
    spawn_title(&mut commands);
    if state.overlay == ShellOverlay::LocalError {
        spawn_error(&mut commands, &state.error_message, false);
        dirty.0 = Some(ShellControlId::ContinueWithoutSaving);
    } else {
        dirty.0 = Some(ShellControlId::Settings);
    }
}

fn root_node() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(0),
        right: px(0),
        top: px(0),
        bottom: px(0),
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        padding: UiRect::all(px(24)),
        ..default()
    }
}

fn panel_node() -> Node {
    Node {
        width: percent(92),
        max_width: px(760),
        max_height: percent(90),
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        row_gap: px(10),
        padding: UiRect::all(px(20)),
        overflow: Overflow::scroll_y(),
        border_radius: BorderRadius::all(px(12)),
        ..default()
    }
}

fn spawn_title(commands: &mut Commands) {
    commands
        .spawn((
            TitleRoot,
            root_node(),
            BackgroundColor(Color::srgb(0.025, 0.04, 0.07)),
            GlobalZIndex(400),
            EntranceAnimation { elapsed: 0.0 },
            UiTransform::from_translation(Val2::new(px(0), px(18))),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("BRAWLER"),
                TextFont::from_font_size(56.0),
                TextColor(Color::srgb(0.25, 0.9, 1.0)),
            ));
            root.spawn((
                Text::new("AUTHOR YOUR FIGHTER. OWN THE ARENA."),
                TextFont::from_font_size(16.0),
                TextColor(Color::srgb(0.7, 0.78, 0.86)),
                Node {
                    margin: UiRect::bottom(px(18)),
                    ..default()
                },
            ));
            spawn_button(
                root,
                "PLAY - COMING IN M03",
                ShellControlId::Play,
                None,
                ShellLayer::Title,
            );
            spawn_button(
                root,
                "PRACTICE - COMING IN M08",
                ShellControlId::Practice,
                None,
                ShellLayer::Title,
            );
            spawn_button(
                root,
                "SETTINGS",
                ShellControlId::Settings,
                Some(ShellAction::OpenSettings),
                ShellLayer::Title,
            );
            spawn_button(
                root,
                "CREDITS",
                ShellControlId::Credits,
                Some(ShellAction::OpenCredits),
                ShellLayer::Title,
            );
            spawn_button(
                root,
                "QUIT",
                ShellControlId::Quit,
                Some(ShellAction::Quit),
                ShellLayer::Title,
            );
        });
}

fn spawn_overlay(
    commands: &mut Commands,
    title: &str,
    build: impl FnOnce(&mut ChildSpawnerCommands),
) {
    commands
        .spawn((
            OverlayRoot,
            root_node(),
            BackgroundColor(Color::srgba(0.005, 0.01, 0.02, 0.94)),
            GlobalZIndex(500),
            EntranceAnimation { elapsed: 0.0 },
            UiTransform::from_translation(Val2::new(px(0), px(14))),
        ))
        .with_children(|root| {
            root.spawn((
                ShellScrollArea,
                ScrollPosition::default(),
                panel_node(),
                BackgroundColor(Color::srgb(0.055, 0.08, 0.12)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new(title),
                    TextFont::from_font_size(32.0),
                    TextColor(Color::srgb(0.25, 0.9, 1.0)),
                ));
                build(panel);
            });
        });
}

fn spawn_settings(commands: &mut Commands) {
    spawn_overlay(commands, "SETTINGS", |panel| {
        panel.spawn((
            InputSettingsText,
            Text::new("Loading settings…"),
            TextFont::from_font_size(13.0),
            TextColor(Color::srgb(0.8, 0.9, 1.0)),
            TextLayout::linebreak(LineBreak::WordBoundary),
            Node {
                width: percent(100),
                ..default()
            },
        ));
        panel.spawn((
            Text::new(
                "Select a row, adjust it, or start rebind capture. East/Escape cancels capture.",
            ),
            TextFont::from_font_size(13.0),
            TextColor(Color::srgb(0.65, 0.72, 0.8)),
        ));
        panel
            .spawn(Node {
                width: percent(100),
                display: Display::Flex,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::Center,
                column_gap: px(8),
                row_gap: px(8),
                ..default()
            })
            .with_children(|buttons| {
                for (label, id, action) in [
                    (
                        "PREVIOUS FIELD",
                        ShellControlId::PreviousField,
                        ShellAction::PreviousField,
                    ),
                    (
                        "NEXT FIELD",
                        ShellControlId::NextField,
                        ShellAction::NextField,
                    ),
                    ("- VALUE", ShellControlId::Decrease, ShellAction::Decrease),
                    ("+ VALUE", ShellControlId::Increase, ShellAction::Increase),
                    ("REBIND", ShellControlId::Rebind, ShellAction::Rebind),
                    (
                        "MOVE Y",
                        ShellControlId::ToggleMoveY,
                        ShellAction::ToggleMoveY,
                    ),
                    ("AIM Y", ShellControlId::ToggleAimY, ShellAction::ToggleAimY),
                    (
                        "UI -",
                        ShellControlId::UiScaleDown,
                        ShellAction::UiScaleDown,
                    ),
                    (
                        "REDUCED MOTION",
                        ShellControlId::ToggleReducedMotion,
                        ShellAction::ToggleReducedMotion,
                    ),
                    ("UI +", ShellControlId::UiScaleUp, ShellAction::UiScaleUp),
                    ("RESET", ShellControlId::Reset, ShellAction::Reset),
                    ("APPLY", ShellControlId::Apply, ShellAction::Apply),
                    ("CANCEL", ShellControlId::Cancel, ShellAction::Cancel),
                ] {
                    spawn_button(buttons, label, id, Some(action), ShellLayer::Settings);
                }
            });
    });
}

fn spawn_credits(commands: &mut Commands) {
    spawn_overlay(commands, "CREDITS", |panel| {
        panel.spawn((
            Text::new(CREDITS),
            TextFont::from_font_size(16.0),
            TextColor(Color::srgb(0.85, 0.9, 0.96)),
            TextLayout::new(Justify::Center, LineBreak::WordBoundary),
            Node {
                width: percent(100),
                ..default()
            },
        ));
        spawn_button(
            panel,
            "BACK",
            ShellControlId::CreditsBack,
            Some(ShellAction::Back),
            ShellLayer::Credits,
        );
    });
}

fn spawn_error(commands: &mut Commands, message: &str, retry: bool) {
    spawn_overlay(commands, "LOCAL SETTINGS ERROR", |panel| {
        panel.spawn((
            Text::new(format!(
                "{message}\n\nSafe defaults remain active. The existing file was not changed."
            )),
            TextFont::from_font_size(16.0),
            TextColor(Color::srgb(1.0, 0.72, 0.48)),
            TextLayout::new(Justify::Center, LineBreak::WordBoundary),
            Node {
                width: percent(100),
                ..default()
            },
        ));
        if retry {
            spawn_button(
                panel,
                "RETRY SAVE",
                ShellControlId::Retry,
                Some(ShellAction::RetrySave),
                ShellLayer::Error,
            );
        }
        spawn_button(
            panel,
            "CONTINUE WITHOUT SAVING",
            ShellControlId::ContinueWithoutSaving,
            Some(ShellAction::ContinueWithoutSaving),
            ShellLayer::Error,
        );
    });
}

fn spawn_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    id: ShellControlId,
    action: Option<ShellAction>,
    layer: ShellLayer,
) {
    parent
        .spawn((
            Button,
            ShellButton { id, action, layer },
            Node {
                min_width: px(250),
                min_height: px(38),
                padding: UiRect::axes(px(16), px(8)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.09, 0.14, 0.2)),
            Outline::new(px(2), px(2), Color::NONE),
        ))
        .with_child((
            Text::new(label),
            TextFont::from_font_size(15.0),
            TextColor(if action.is_some() {
                Color::WHITE
            } else {
                Color::srgb(0.42, 0.48, 0.54)
            }),
        ));
}

fn active_layer(overlay: ShellOverlay) -> ShellLayer {
    match overlay {
        ShellOverlay::None => ShellLayer::Title,
        ShellOverlay::Settings => ShellLayer::Settings,
        ShellOverlay::Credits => ShellLayer::Credits,
        ShellOverlay::LocalError => ShellLayer::Error,
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "the input system declares its complete bounded shell view at the schedule boundary"
)]
fn collect_navigation_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    selection: Res<InputSettingsSelection>,
    state: Res<ShellState>,
    mut latch: ResMut<NavigationLatch>,
    mut navigation: DirectionalNavigation,
    buttons: Query<&ShellButton>,
    mut pending: ResMut<PendingActions>,
) {
    if selection.listening {
        return;
    }
    let pad_pressed = |button| gamepads.iter().any(|pad| pad.just_pressed(button));
    let stick = gamepads
        .iter()
        .map(Gamepad::left_stick)
        .fold(Vec2::ZERO, |best, sample| {
            if sample.length_squared() > best.length_squared() {
                sample
            } else {
                best
            }
        });
    if stick.y.abs() < 0.35 {
        latch.y_ready = true;
    }
    let up = keyboard.any_just_pressed([KeyCode::ArrowUp, KeyCode::KeyW])
        || pad_pressed(GamepadButton::DPadUp)
        || (stick.y > 0.65 && latch.y_ready);
    let down = keyboard.any_just_pressed([KeyCode::ArrowDown, KeyCode::KeyS])
        || pad_pressed(GamepadButton::DPadDown)
        || (stick.y < -0.65 && latch.y_ready);
    if stick.y.abs() > 0.65 {
        latch.y_ready = false;
    }
    if up {
        let _ = navigation.navigate(CompassOctant::North);
    }
    if down {
        let _ = navigation.navigate(CompassOctant::South);
    }

    let activate = keyboard.any_just_pressed([KeyCode::Enter, KeyCode::Space])
        || pad_pressed(GamepadButton::South);
    if activate
        && let Some(entity) = navigation.focus.get()
        && let Ok(button) = buttons.get(entity)
        && button.layer == active_layer(state.overlay)
        && let Some(action) = button.action
        && !pending.0.contains(&action)
    {
        pending.0.push(action);
    }
    let back = keyboard.just_pressed(KeyCode::Escape) || pad_pressed(GamepadButton::East);
    if back && state.overlay != ShellOverlay::None && !pending.0.contains(&ShellAction::Back) {
        pending.0.push(ShellAction::Back);
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn collect_pointer_actions(
    interactions: Query<(Entity, &Interaction, &ShellButton), Changed<Interaction>>,
    state: Res<ShellState>,
    mut focus: ResMut<InputFocus>,
    mut pending: ResMut<PendingActions>,
) {
    for (entity, interaction, button) in &interactions {
        if button.layer != active_layer(state.overlay) {
            continue;
        }
        if matches!(interaction, Interaction::Hovered | Interaction::Pressed)
            && button.action.is_some()
        {
            focus.set(entity, FocusCause::Pressed);
        }
        if *interaction == Interaction::Pressed
            && let Some(action) = button.action
            && !pending.0.contains(&action)
        {
            pending.0.push(action);
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the specification keeps the small ShellAction transition owner together; extracted UI builders and persistence helpers carry the algorithms"
)]
fn handle_shell_actions(
    mut commands: Commands,
    mut pending: ResMut<PendingActions>,
    mut state: ResMut<ShellState>,
    roots: Query<Entity, With<OverlayRoot>>,
    mut input_draft: Option<ResMut<InputSettingsDraft>>,
    mut shell_draft: Option<ResMut<ShellSettingsDraft>>,
    mut selection: ResMut<InputSettingsSelection>,
    path: Res<ClientSettingsPath>,
    mut active_input: ResMut<ClientInputSettings>,
    mut active_shell: ResMut<ClientShellSettings>,
    mut dirty: ResMut<NavigationDirty>,
    mut exit: MessageWriter<AppExit>,
) {
    let actions = core::mem::take(&mut pending.0);
    for action in actions {
        match action {
            ShellAction::OpenSettings => {
                state.overlay = ShellOverlay::Settings;
                commands.insert_resource(InputSettingsDraft(*active_input));
                commands.insert_resource(ShellSettingsDraft(*active_shell));
                selection.listening = false;
                despawn_overlays(&mut commands, &roots);
                spawn_settings(&mut commands);
                dirty.0 = Some(ShellControlId::PreviousField);
            }
            ShellAction::OpenCredits => {
                state.overlay = ShellOverlay::Credits;
                despawn_overlays(&mut commands, &roots);
                spawn_credits(&mut commands);
                dirty.0 = Some(ShellControlId::CreditsBack);
            }
            ShellAction::Quit => {
                exit.write(AppExit::Success);
            }
            ShellAction::PreviousField => selection.field = selection.field.previous(),
            ShellAction::NextField => selection.field = selection.field.next(),
            ShellAction::Decrease | ShellAction::Increase => {
                if let Some(draft) = input_draft.as_deref_mut()
                    && let InputSettingsField::Calibration(field) = selection.field
                {
                    draft.0.adjust_calibration(
                        field,
                        if action == ShellAction::Decrease {
                            -0.05
                        } else {
                            0.05
                        },
                    );
                }
            }
            ShellAction::Rebind => {
                if selection.field.is_rebindable() {
                    selection.listening = true;
                }
            }
            ShellAction::ToggleMoveY => {
                if let Some(draft) = input_draft.as_deref_mut() {
                    draft.0.toggle_inversion(false);
                }
            }
            ShellAction::ToggleAimY => {
                if let Some(draft) = input_draft.as_deref_mut() {
                    draft.0.toggle_inversion(true);
                }
            }
            ShellAction::UiScaleDown | ShellAction::UiScaleUp => {
                if let Some(draft) = shell_draft.as_deref_mut() {
                    let step = if action == ShellAction::UiScaleDown {
                        -0.1
                    } else {
                        0.1
                    };
                    draft.0.ui_scale = ((draft.0.ui_scale + step) * 10.0).round() / 10.0;
                    draft.0.ui_scale = draft.0.ui_scale.clamp(MIN_UI_SCALE, MAX_UI_SCALE);
                }
            }
            ShellAction::ToggleReducedMotion => {
                if let Some(draft) = shell_draft.as_deref_mut() {
                    draft.0.reduced_motion = !draft.0.reduced_motion;
                }
            }
            ShellAction::Reset => {
                if let Some(draft) = input_draft.as_deref_mut() {
                    draft.0.reset_to_default();
                }
                if let Some(draft) = shell_draft.as_deref_mut() {
                    draft.0 = ClientShellSettings::default();
                }
                selection.listening = false;
            }
            ShellAction::Apply => {
                let (Some(draft_input), Some(draft_shell)) =
                    (input_draft.as_deref(), shell_draft.as_deref())
                else {
                    continue;
                };
                if let Err(error) = draft_input
                    .0
                    .validate()
                    .and_then(|()| draft_shell.0.validate())
                {
                    state.error_message = error;
                    state.error_return = ErrorReturn::Settings;
                    state.overlay = ShellOverlay::LocalError;
                    state.settings_applied_before_error = false;
                    despawn_overlays(&mut commands, &roots);
                    spawn_error(&mut commands, &state.error_message, false);
                    dirty.0 = Some(ShellControlId::ContinueWithoutSaving);
                    continue;
                }
                let mut applied = draft_input.0;
                applied.revision = active_input.revision.saturating_add(1);
                *active_input = applied;
                *active_shell = draft_shell.0;
                if let Err(error) = save_settings(&path.0, *active_input, *active_shell) {
                    state.error_message = error;
                    state.error_return = ErrorReturn::Settings;
                    state.overlay = ShellOverlay::LocalError;
                    state.settings_applied_before_error = true;
                    despawn_overlays(&mut commands, &roots);
                    spawn_error(&mut commands, &state.error_message, true);
                    dirty.0 = Some(ShellControlId::Retry);
                } else {
                    close_overlay(
                        &mut commands,
                        &mut state,
                        &roots,
                        &mut dirty,
                        ShellControlId::Settings,
                    );
                }
            }
            ShellAction::RetrySave => {
                if save_settings(&path.0, *active_input, *active_shell).is_ok() {
                    close_overlay(
                        &mut commands,
                        &mut state,
                        &roots,
                        &mut dirty,
                        ShellControlId::Settings,
                    );
                }
            }
            ShellAction::ContinueWithoutSaving => {
                if state.error_return == ErrorReturn::Settings
                    && !state.settings_applied_before_error
                {
                    despawn_overlays(&mut commands, &roots);
                    state.overlay = ShellOverlay::Settings;
                    spawn_settings(&mut commands);
                    dirty.0 = Some(ShellControlId::Apply);
                } else {
                    close_overlay(
                        &mut commands,
                        &mut state,
                        &roots,
                        &mut dirty,
                        ShellControlId::Settings,
                    );
                }
            }
            ShellAction::Cancel | ShellAction::Back => {
                if state.overlay == ShellOverlay::LocalError
                    && state.error_return == ErrorReturn::Settings
                    && !state.settings_applied_before_error
                {
                    despawn_overlays(&mut commands, &roots);
                    state.overlay = ShellOverlay::Settings;
                    spawn_settings(&mut commands);
                    dirty.0 = Some(ShellControlId::Apply);
                    continue;
                }
                let focus = match state.overlay {
                    ShellOverlay::Credits => ShellControlId::Credits,
                    ShellOverlay::Settings | ShellOverlay::LocalError => ShellControlId::Settings,
                    ShellOverlay::None => continue,
                };
                close_overlay(&mut commands, &mut state, &roots, &mut dirty, focus);
            }
        }
    }
}

fn despawn_overlays(commands: &mut Commands, roots: &Query<Entity, With<OverlayRoot>>) {
    for entity in roots {
        commands.entity(entity).despawn();
    }
}

fn close_overlay(
    commands: &mut Commands,
    state: &mut ShellState,
    roots: &Query<Entity, With<OverlayRoot>>,
    dirty: &mut NavigationDirty,
    focus: ShellControlId,
) {
    despawn_overlays(commands, roots);
    commands.remove_resource::<InputSettingsDraft>();
    commands.remove_resource::<ShellSettingsDraft>();
    state.overlay = ShellOverlay::None;
    state.settings_applied_before_error = false;
    dirty.0 = Some(focus);
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn rebuild_navigation(
    state: Res<ShellState>,
    mut dirty: ResMut<NavigationDirty>,
    buttons: Query<(Entity, &ShellButton)>,
    mut map: ResMut<DirectionalNavigationMap>,
    mut focus: ResMut<InputFocus>,
) {
    let Some(preferred) = dirty.0.take() else {
        return;
    };
    map.clear();
    let layer = active_layer(state.overlay);
    let mut active: Vec<(Entity, ShellControlId)> = buttons
        .iter()
        .filter_map(|(entity, button)| {
            (button.layer == layer && button.action.is_some()).then_some((entity, button.id))
        })
        .collect();
    active.sort_by_key(|(_, id)| *id as u8);
    let entities: Vec<Entity> = active.iter().map(|(entity, _)| *entity).collect();
    map.add_looping_edges(&entities, CompassOctant::South);
    if let Some((entity, _)) = active
        .iter()
        .find(|(_, id)| *id == preferred)
        .or_else(|| active.first())
    {
        focus.set(*entity, FocusCause::Navigated);
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn style_shell_buttons(
    focus: Res<InputFocus>,
    mut buttons: Query<(
        Entity,
        &Interaction,
        &ShellButton,
        &mut BackgroundColor,
        &mut Outline,
    )>,
) {
    for (entity, interaction, button, mut background, mut outline) in &mut buttons {
        let disabled = button.action.is_none();
        let focused = focus.get() == Some(entity);
        background.0 = if disabled {
            Color::srgb(0.055, 0.07, 0.09)
        } else if *interaction == Interaction::Pressed {
            Color::srgb(0.08, 0.48, 0.58)
        } else if focused || *interaction == Interaction::Hovered {
            Color::srgb(0.12, 0.32, 0.42)
        } else {
            Color::srgb(0.09, 0.14, 0.2)
        };
        outline.color = if focused { Color::WHITE } else { Color::NONE };
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn preview_shell_preferences(
    active: Res<ClientShellSettings>,
    draft: Option<Res<ShellSettingsDraft>>,
    mut scale: ResMut<UiScale>,
) {
    scale.0 = draft
        .as_deref()
        .map_or(active.ui_scale, |draft| draft.0.ui_scale);
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn animate_shell_entrance(
    time: Res<Time<Real>>,
    active: Res<ClientShellSettings>,
    draft: Option<Res<ShellSettingsDraft>>,
    mut roots: Query<(&mut EntranceAnimation, &mut UiTransform)>,
) {
    let reduced = draft
        .as_deref()
        .map_or(active.reduced_motion, |draft| draft.0.reduced_motion);
    for (mut animation, mut transform) in &mut roots {
        animation.elapsed = if reduced {
            ENTRANCE_SECONDS
        } else {
            (animation.elapsed + time.delta_secs()).min(ENTRANCE_SECONDS)
        };
        let progress = animation.elapsed / ENTRANCE_SECONDS;
        let eased = 1.0 - (1.0 - progress).powi(3);
        transform.translation = Val2::new(px(0), px(18.0 * (1.0 - eased)));
        transform.scale = Vec2::splat(0.985 + 0.015 * eased);
    }
}

fn scroll_shell_panels(
    mut wheel: MessageReader<MouseWheel>,
    mut panels: Query<&mut ScrollPosition, With<ShellScrollArea>>,
) {
    let delta = wheel
        .read()
        .map(|event| match event.unit {
            MouseScrollUnit::Line => event.y * 24.0,
            MouseScrollUnit::Pixel => event.y,
        })
        .sum::<f32>();
    if delta.abs() <= f32::EPSILON {
        return;
    }
    for mut position in &mut panels {
        position.0.y = (position.0.y - delta).max(0.0);
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn keep_focused_control_visible(
    focus: Res<InputFocus>,
    buttons: Query<(&ComputedNode, &UiGlobalTransform), With<ShellButton>>,
    mut panels: Query<
        (&ComputedNode, &UiGlobalTransform, &mut ScrollPosition),
        With<ShellScrollArea>,
    >,
) {
    let Some(focused) = focus.get() else {
        return;
    };
    let Ok((button_node, button_transform)) = buttons.get(focused) else {
        return;
    };
    if button_node.is_empty() {
        return;
    }
    let (_, _, button_center) = button_transform.to_scale_angle_translation();
    let button_half_height = button_node.size().y * 0.5;
    for (panel_node, panel_transform, mut scroll) in &mut panels {
        if panel_node.is_empty() {
            continue;
        }
        let (_, _, panel_center) = panel_transform.to_scale_angle_translation();
        let panel_half_height = panel_node.size().y * 0.5;
        let visible_min = panel_center.y - panel_half_height + 8.0;
        let visible_max = panel_center.y + panel_half_height - 8.0;
        let button_min = button_center.y - button_half_height;
        let button_max = button_center.y + button_half_height;
        if button_max > visible_max {
            scroll.0.y += button_max - visible_max;
        } else if button_min < visible_min {
            scroll.0.y = (scroll.0.y - (visible_min - button_min)).max(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell_test_app(path: std::path::PathBuf) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<MouseWheel>()
            .add_message::<AppExit>()
            .insert_resource(ClientSettingsPath(path))
            .insert_resource(UiScale::default())
            .insert_resource(ButtonInput::<KeyCode>::default())
            .init_resource::<ClientInputContext>()
            .init_resource::<ClientInputSettings>()
            .init_resource::<InputSettingsSelection>()
            .add_plugins(ClientShellPlugin);
        app
    }

    #[test]
    fn shell_startup_focus_skips_disabled_entries_and_overlay_replaces_navigation_root() {
        let path = std::env::temp_dir().join(format!(
            "brawler-m02-shell-missing-{}-settings.ron",
            std::process::id()
        ));
        let mut app = shell_test_app(path);

        app.update();
        let focus = app.world().resource::<InputFocus>().get().unwrap();
        assert_eq!(
            app.world().get::<ShellButton>(focus).unwrap().id,
            ShellControlId::Settings
        );
        let enabled_title_buttons_skip_disabled = {
            let world = app.world_mut();
            let mut query = world.query::<&ShellButton>();
            query
                .iter(world)
                .filter(|button| button.layer == ShellLayer::Title && button.action.is_some())
                .all(|button| !matches!(button.id, ShellControlId::Play | ShellControlId::Practice))
        };
        assert!(enabled_title_buttons_skip_disabled);

        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::OpenSettings);
        app.update();
        assert_eq!(
            app.world().resource::<ShellState>().overlay,
            ShellOverlay::Settings
        );
        let focus = app.world().resource::<InputFocus>().get().unwrap();
        assert_eq!(
            app.world().get::<ShellButton>(focus).unwrap().layer,
            ShellLayer::Settings
        );
    }

    #[test]
    fn settings_draft_cancel_reset_and_apply_have_distinct_commit_behavior() {
        let dir =
            std::env::temp_dir().join(format!("brawler-m02-shell-draft-{}", std::process::id()));
        let path = dir.join("settings.ron");
        let mut app = shell_test_app(path.clone());
        app.update();

        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::OpenSettings);
        app.update();
        app.world_mut()
            .resource_mut::<InputSettingsDraft>()
            .0
            .invert_aim_y = true;
        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::Cancel);
        app.update();
        assert!(!app.world().resource::<ClientInputSettings>().invert_aim_y);

        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::OpenSettings);
        app.update();
        app.world_mut()
            .resource_mut::<InputSettingsDraft>()
            .0
            .invert_move_y = true;
        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::Reset);
        app.update();
        assert_eq!(
            app.world().resource::<InputSettingsDraft>().0,
            ClientInputSettings {
                revision: 1,
                ..ClientInputSettings::default()
            }
        );

        app.world_mut()
            .resource_mut::<InputSettingsDraft>()
            .0
            .invert_aim_y = true;
        app.world_mut()
            .resource_mut::<ShellSettingsDraft>()
            .0
            .reduced_motion = true;
        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::Apply);
        app.update();
        assert!(app.world().resource::<ClientInputSettings>().invert_aim_y);
        assert!(app.world().resource::<ClientShellSettings>().reduced_motion);
        assert_eq!(
            app.world().resource::<ShellState>().overlay,
            ShellOverlay::None
        );
        assert!(path.is_file());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn controller_dpad_skips_disabled_title_entries_and_south_activates_once() {
        let path = std::env::temp_dir().join(format!(
            "brawler-m02-shell-controller-{}-settings.ron",
            std::process::id()
        ));
        let mut app = shell_test_app(path);
        let mut gamepad = Gamepad::default();
        gamepad.digital_mut().press(GamepadButton::DPadDown);
        gamepad.digital_mut().press(GamepadButton::South);
        app.world_mut().spawn(gamepad);

        app.update();
        assert_eq!(
            app.world().resource::<ShellState>().overlay,
            ShellOverlay::Credits
        );
        let overlay_count = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<OverlayRoot>>();
            query.iter(world).count()
        };
        assert_eq!(overlay_count, 1);
    }

    #[test]
    fn active_layer_traps_focus_to_the_visible_overlay() {
        assert_eq!(active_layer(ShellOverlay::None), ShellLayer::Title);
        assert_eq!(active_layer(ShellOverlay::Settings), ShellLayer::Settings);
        assert_eq!(active_layer(ShellOverlay::Credits), ShellLayer::Credits);
        assert_eq!(active_layer(ShellOverlay::LocalError), ShellLayer::Error);
    }

    #[test]
    fn entrance_finishes_at_identity_with_or_without_motion() {
        for reduced in [false, true] {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
                .insert_resource(ClientShellSettings {
                    ui_scale: 1.0,
                    reduced_motion: reduced,
                })
                .add_systems(Update, animate_shell_entrance);
            app.world_mut().spawn((
                EntranceAnimation {
                    elapsed: ENTRANCE_SECONDS,
                },
                UiTransform::from_xy(px(0), px(9)),
            ));
            app.update();
            let world = app.world_mut();
            let mut query = world.query::<&UiTransform>();
            let transform = query.single(world).unwrap();
            assert_eq!(transform.translation, Val2::ZERO);
            assert_eq!(transform.scale, Vec2::ONE);
        }
    }
}
