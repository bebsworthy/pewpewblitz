//! Functional windowed product shell: title, one overlay, focus, settings draft, and errors.

use super::flow::ClientFlowSet;
use super::flow::ClientLocalLoadFailures;
use super::{
    ClientFlow, ClientInputContext, ClientOverlay, FlowError, FlowErrorAction, FlowErrorKind,
    InputCaptureConsumed, InputSettingsField, InputSettingsSelection, InputSettingsText,
    SessionPurpose,
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
    audio::{AudioSink, AudioSinkPlayback, GlobalVolume, SpatialAudioSink, Volume},
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
    window::{MonitorSelection, PresentMode, PrimaryWindow, WindowMode},
};

const ENTRANCE_SECONDS: f32 = 0.16;
const CREDITS: &str = "PewPew Blitz 0.1.0\n\nBuilt with Bevy 0.19 (MIT OR Apache-2.0).\nDefault Fira Mono font: Mozilla Foundation / Telefonica, SIL OFL 1.1.\nFighters and sounds: Kenney, CC0 1.0.\nFacility tiles: Murphy's Dad / HaywardMorihara, CC0 1.0.\n\nFull license texts ship in assets/licenses/.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ErrorReturn {
    Title,
    Settings,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum LocalSettingsErrorKind {
    #[default]
    Load,
    Validation,
    Save,
}

#[derive(Resource, Debug)]
struct ShellState {
    return_target: ErrorReturn,
    kind: LocalSettingsErrorKind,
    message: String,
}

impl Default for ShellState {
    fn default() -> Self {
        Self {
            return_target: ErrorReturn::Title,
            kind: LocalSettingsErrorKind::Load,
            message: String::new(),
        }
    }
}

#[derive(Resource, Clone, Copy, Debug, PartialEq)]
struct ShellSettingsDraft(ClientShellSettings);

#[derive(Resource, Default)]
struct PendingActions(Vec<ShellAction>);

#[derive(Resource, Default)]
struct NavigationLatch {
    x_ready: bool,
    y_ready: bool,
}

#[derive(Resource, Default)]
struct NavigationDirty(Option<ShellControlId>);

#[derive(Component)]
#[allow(
    dead_code,
    reason = "retained temporarily for V2 shell regression fixtures during V5 M01"
)]
struct TitleRoot;

#[derive(Component)]
struct OverlayRoot;

#[derive(Component)]
struct ShellScrollArea;

#[derive(Component)]
struct ShellSettingsText;

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
    FlowOwned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShellControlId {
    #[allow(
        dead_code,
        reason = "retained for V2 title regression fixtures during V5 M01"
    )]
    Play,
    #[allow(
        dead_code,
        reason = "retained for V2 title regression fixtures during V5 M01"
    )]
    Practice,
    Settings,
    Credits,
    #[allow(
        dead_code,
        reason = "retained for V2 title regression fixtures during V5 M01"
    )]
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
    ToggleReducedEffects,
    VolumeDown,
    VolumeUp,
    ToggleFocusMute,
    ToggleFullscreen,
    ToggleVsync,
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
    #[allow(
        dead_code,
        reason = "retained for V2 title regression fixtures during V5 M01"
    )]
    Play,
    #[allow(
        dead_code,
        reason = "retained for V2 title regression fixtures during V5 M01"
    )]
    Practice,
    #[allow(
        dead_code,
        reason = "retained for shell settings regression fixtures during V5 M01"
    )]
    OpenSettings,
    OpenMatchSettings,
    #[allow(
        dead_code,
        reason = "retained for V2 title regression fixtures during V5 M01"
    )]
    OpenCredits,
    #[allow(
        dead_code,
        reason = "retained for V2 title regression fixtures during V5 M01"
    )]
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
    ToggleReducedEffects,
    VolumeDown,
    VolumeUp,
    ToggleFocusMute,
    ToggleFullscreen,
    ToggleVsync,
    UiScaleUp,
    Reset,
    Apply,
    Cancel,
    Back,
    RetrySave,
    ContinueWithoutSaving,
}

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SettingsReturnTarget {
    #[default]
    Title,
    MatchMenu,
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
            .init_resource::<InputCaptureConsumed>()
            .init_resource::<super::MatchSettingsRequest>()
            .init_resource::<SettingsReturnTarget>()
            .init_resource::<SessionPurpose>()
            .add_systems(Startup, load_persistent_settings)
            .add_systems(OnEnter(ClientFlow::Dashboard), enter_dashboard_shell)
            .add_systems(
                Update,
                (
                    collect_navigation_input.in_set(ClientFlowSet::CollectFlowInput),
                    collect_pointer_actions.in_set(ClientFlowSet::CollectFlowInput),
                    collect_match_settings_request.in_set(ClientFlowSet::CollectFlowInput),
                    handle_shell_actions.in_set(ClientFlowSet::ResolveFlowAction),
                    present_flow_requested_overlay.in_set(ClientFlowSet::PresentFlow),
                    restore_match_menu_after_settings.in_set(ClientFlowSet::PresentFlow),
                    rebuild_navigation.in_set(ClientFlowSet::PresentFlow),
                    style_shell_buttons.in_set(ClientFlowSet::PresentFlow),
                    preview_shell_preferences.in_set(ClientFlowSet::PresentFlow),
                    apply_audio_preferences.in_set(ClientFlowSet::PresentFlow),
                    update_shell_settings_text.in_set(ClientFlowSet::PresentFlow),
                    animate_shell_entrance.in_set(ClientFlowSet::PresentFlow),
                    scroll_shell_panels.in_set(ClientFlowSet::PresentFlow),
                )
                    .chain(),
            )
            .add_systems(
                PostUpdate,
                (rebuild_navigation, keep_focused_control_visible)
                    .chain()
                    .after(UiSystems::Layout),
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
    mut failures: ResMut<ClientLocalLoadFailures>,
) {
    match load_settings(&path.0) {
        Ok(Some((loaded_input, loaded_shell))) => {
            *input = loaded_input;
            *shell = loaded_shell;
        }
        Ok(None) => {}
        Err(error) => {
            failures.settings_failed = true;
            state.return_target = ErrorReturn::Title;
            state.kind = LocalSettingsErrorKind::Load;
            state.message = error.to_string();
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn enter_dashboard_shell(mut context: ResMut<ClientInputContext>) {
    *context = ClientInputContext::Shell;
}

#[allow(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    reason = "flow-owned utility requests initialize the existing shell overlay drafts"
)]
fn present_flow_requested_overlay(
    mut commands: Commands,
    overlay: Res<ClientOverlay>,
    roots: Query<Entity, With<OverlayRoot>>,
    active_input: Res<ClientInputSettings>,
    active_shell: Res<ClientShellSettings>,
    input_draft: Option<Res<InputSettingsDraft>>,
    shell_draft: Option<Res<ShellSettingsDraft>>,
    mut selection: ResMut<InputSettingsSelection>,
    mut dirty: ResMut<NavigationDirty>,
    mut settings_return: ResMut<SettingsReturnTarget>,
) {
    if !roots.is_empty() {
        return;
    }
    match overlay.as_ref() {
        ClientOverlay::Settings => {
            *settings_return = SettingsReturnTarget::Title;
            if input_draft.is_none() {
                commands.insert_resource(InputSettingsDraft(*active_input));
            }
            if shell_draft.is_none() {
                commands.insert_resource(ShellSettingsDraft(*active_shell));
            }
            selection.listening = false;
            spawn_settings(&mut commands);
            dirty.0 = Some(ShellControlId::PreviousField);
        }
        ClientOverlay::Credits => {
            spawn_credits(&mut commands);
            dirty.0 = Some(ShellControlId::CreditsBack);
        }
        _ => {}
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

#[allow(
    dead_code,
    reason = "retained temporarily for V2 shell regression fixtures during V5 M01"
)]
fn spawn_title(commands: &mut Commands) {
    commands
        .spawn((
            TitleRoot,
            DespawnOnExit(ClientFlow::Dashboard),
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
                "PLAY",
                ShellControlId::Play,
                Some(ShellAction::Play),
                ShellLayer::Title,
            );
            spawn_button(
                root,
                "PRACTICE",
                ShellControlId::Practice,
                Some(ShellAction::Practice),
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
            ShellSettingsText,
            Text::new(""),
            TextFont::from_font_size(14.0),
            TextColor(Color::srgb(0.88, 0.92, 0.96)),
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
                spawn_settings_buttons(buttons);
            });
    });
}

fn spawn_settings_buttons(buttons: &mut ChildSpawnerCommands) {
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
        (
            "REDUCED EFFECTS",
            ShellControlId::ToggleReducedEffects,
            ShellAction::ToggleReducedEffects,
        ),
        (
            "VOLUME -",
            ShellControlId::VolumeDown,
            ShellAction::VolumeDown,
        ),
        ("VOLUME +", ShellControlId::VolumeUp, ShellAction::VolumeUp),
        (
            "MUTE UNFOCUSED",
            ShellControlId::ToggleFocusMute,
            ShellAction::ToggleFocusMute,
        ),
        (
            "FULLSCREEN",
            ShellControlId::ToggleFullscreen,
            ShellAction::ToggleFullscreen,
        ),
        (
            "VSYNC",
            ShellControlId::ToggleVsync,
            ShellAction::ToggleVsync,
        ),
        ("UI +", ShellControlId::UiScaleUp, ShellAction::UiScaleUp),
        ("RESET", ShellControlId::Reset, ShellAction::Reset),
        ("APPLY", ShellControlId::Apply, ShellAction::Apply),
        ("CANCEL", ShellControlId::Cancel, ShellAction::Cancel),
    ] {
        spawn_button(buttons, label, id, Some(action), ShellLayer::Settings);
    }
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

fn spawn_error(commands: &mut Commands, message: &str, kind: LocalSettingsErrorKind) {
    spawn_overlay(commands, "LOCAL SETTINGS ERROR", |panel| {
        let outcome = match kind {
            LocalSettingsErrorKind::Load => {
                "Safe defaults remain active. The existing file was not changed."
            }
            LocalSettingsErrorKind::Validation => {
                "The draft was retained and no values were applied."
            }
            LocalSettingsErrorKind::Save => {
                "Your changes remain active for this session. The existing file was not changed."
            }
        };
        panel.spawn((
            Text::new(format!("{message}\n\n{outcome}")),
            TextFont::from_font_size(16.0),
            TextColor(Color::srgb(1.0, 0.72, 0.48)),
            TextLayout::new(Justify::Center, LineBreak::WordBoundary),
            Node {
                width: percent(100),
                ..default()
            },
        ));
        if kind == LocalSettingsErrorKind::Save {
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

fn active_layer(overlay: &ClientOverlay) -> ShellLayer {
    match overlay {
        ClientOverlay::None => ShellLayer::Title,
        ClientOverlay::Settings => ShellLayer::Settings,
        ClientOverlay::Credits => ShellLayer::Credits,
        ClientOverlay::Error(_) => ShellLayer::Error,
        ClientOverlay::BuildEditor
        | ClientOverlay::Confirmation(_)
        | ClientOverlay::ChangeServerConfirmation
        | ClientOverlay::LeaveConfirmation => ShellLayer::FlowOwned,
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
    capture_consumed: Res<InputCaptureConsumed>,
    overlay: Res<ClientOverlay>,
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
    if stick.x.abs() < 0.35 {
        latch.x_ready = true;
    }
    let up = keyboard.any_just_pressed([KeyCode::ArrowUp, KeyCode::KeyW])
        || pad_pressed(GamepadButton::DPadUp)
        || (stick.y > 0.65 && stick.y.abs() >= stick.x.abs() && latch.y_ready);
    let down = keyboard.any_just_pressed([KeyCode::ArrowDown, KeyCode::KeyS])
        || pad_pressed(GamepadButton::DPadDown)
        || (stick.y < -0.65 && stick.y.abs() >= stick.x.abs() && latch.y_ready);
    let left = keyboard.any_just_pressed([KeyCode::ArrowLeft, KeyCode::KeyA])
        || pad_pressed(GamepadButton::DPadLeft)
        || (stick.x < -0.65 && stick.x.abs() > stick.y.abs() && latch.x_ready);
    let right = keyboard.any_just_pressed([KeyCode::ArrowRight, KeyCode::KeyD])
        || pad_pressed(GamepadButton::DPadRight)
        || (stick.x > 0.65 && stick.x.abs() > stick.y.abs() && latch.x_ready);
    if stick.y.abs() > 0.65 {
        latch.y_ready = false;
    }
    if stick.x.abs() > 0.65 {
        latch.x_ready = false;
    }
    if up {
        let _ = navigation.navigate(CompassOctant::North);
    }
    if down {
        let _ = navigation.navigate(CompassOctant::South);
    }
    if left {
        let _ = navigation.navigate(CompassOctant::West);
    }
    if right {
        let _ = navigation.navigate(CompassOctant::East);
    }

    let activate = keyboard.any_just_pressed([KeyCode::Enter, KeyCode::Space])
        || pad_pressed(GamepadButton::South);
    if activate
        && let Some(entity) = navigation.focus.get()
        && let Ok(button) = buttons.get(entity)
        && button.layer == active_layer(&overlay)
        && let Some(action) = button.action
        && !pending.0.contains(&action)
    {
        pending.0.push(action);
    }
    let back = !capture_consumed.0
        && (keyboard.just_pressed(KeyCode::Escape) || pad_pressed(GamepadButton::East));
    if back
        && !matches!(overlay.as_ref(), ClientOverlay::None)
        && !pending.0.contains(&ShellAction::Back)
    {
        pending.0.push(ShellAction::Back);
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn collect_pointer_actions(
    interactions: Query<(Entity, &Interaction, &ShellButton), Changed<Interaction>>,
    overlay: Res<ClientOverlay>,
    mut focus: ResMut<InputFocus>,
    mut pending: ResMut<PendingActions>,
) {
    for (entity, interaction, button) in &interactions {
        if button.layer != active_layer(&overlay) {
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

fn collect_match_settings_request(
    mut request: ResMut<super::MatchSettingsRequest>,
    mut pending: ResMut<PendingActions>,
) {
    if core::mem::take(&mut request.0) && !pending.0.contains(&ShellAction::OpenMatchSettings) {
        pending.0.push(ShellAction::OpenMatchSettings);
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
    flow_state: (
        ResMut<NextState<ClientFlow>>,
        ResMut<SessionPurpose>,
        ResMut<ClientOverlay>,
    ),
    mut context: ResMut<ClientInputContext>,
    mut settings_return: ResMut<SettingsReturnTarget>,
    mut exit: MessageWriter<AppExit>,
) {
    let (mut next_flow, mut purpose, mut client_overlay) = flow_state;
    let actions = core::mem::take(&mut pending.0);
    for action in actions {
        match action {
            ShellAction::Play => {
                *purpose = SessionPurpose::Multiplayer;
                *client_overlay = ClientOverlay::None;
                next_flow.set(ClientFlow::ServerSelect);
            }
            ShellAction::Practice => {
                *purpose = SessionPurpose::Practice;
                *client_overlay = ClientOverlay::None;
                next_flow.set(ClientFlow::ServerSelect);
            }
            ShellAction::OpenSettings | ShellAction::OpenMatchSettings => {
                *settings_return = if action == ShellAction::OpenMatchSettings {
                    *context = ClientInputContext::Shell;
                    SettingsReturnTarget::MatchMenu
                } else {
                    SettingsReturnTarget::Title
                };
                *client_overlay = ClientOverlay::Settings;
                commands.insert_resource(InputSettingsDraft(*active_input));
                commands.insert_resource(ShellSettingsDraft(*active_shell));
                selection.listening = false;
                despawn_overlays(&mut commands, &roots);
                spawn_settings(&mut commands);
                dirty.0 = Some(ShellControlId::PreviousField);
            }
            ShellAction::OpenCredits => {
                *client_overlay = ClientOverlay::Credits;
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
            ShellAction::ToggleReducedEffects => {
                if let Some(draft) = shell_draft.as_deref_mut() {
                    draft.0.reduced_combat_effects = !draft.0.reduced_combat_effects;
                }
            }
            ShellAction::VolumeDown | ShellAction::VolumeUp => {
                if let Some(draft) = shell_draft.as_deref_mut() {
                    draft.0.master_volume = if action == ShellAction::VolumeDown {
                        draft.0.master_volume.saturating_sub(10)
                    } else {
                        draft.0.master_volume.saturating_add(10).min(100)
                    };
                }
            }
            ShellAction::ToggleFocusMute => {
                if let Some(draft) = shell_draft.as_deref_mut() {
                    draft.0.mute_when_unfocused = !draft.0.mute_when_unfocused;
                }
            }
            ShellAction::ToggleFullscreen => {
                if let Some(draft) = shell_draft.as_deref_mut() {
                    draft.0.fullscreen = !draft.0.fullscreen;
                }
            }
            ShellAction::ToggleVsync => {
                if let Some(draft) = shell_draft.as_deref_mut() {
                    draft.0.vsync = !draft.0.vsync;
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
                    state.message = error;
                    state.return_target = ErrorReturn::Settings;
                    state.kind = LocalSettingsErrorKind::Validation;
                    *client_overlay = ClientOverlay::Error(FlowError {
                        kind: FlowErrorKind::Content,
                        message: state.message.clone(),
                        return_flow: ClientFlow::Dashboard,
                        actions: [Some(FlowErrorAction::ContinueWithoutSaving), None],
                    });
                    despawn_overlays(&mut commands, &roots);
                    spawn_error(&mut commands, &state.message, state.kind);
                    dirty.0 = Some(ShellControlId::ContinueWithoutSaving);
                    continue;
                }
                let mut applied = draft_input.0;
                applied.revision = active_input.revision.saturating_add(1);
                *active_input = applied;
                *active_shell = draft_shell.0;
                if let Err(error) = save_settings(&path.0, *active_input, *active_shell) {
                    state.message = error;
                    state.return_target = ErrorReturn::Settings;
                    state.kind = LocalSettingsErrorKind::Save;
                    *client_overlay = ClientOverlay::Error(FlowError {
                        kind: FlowErrorKind::Persistence,
                        message: state.message.clone(),
                        return_flow: ClientFlow::Dashboard,
                        actions: [
                            Some(FlowErrorAction::RetrySave),
                            Some(FlowErrorAction::ContinueWithoutSaving),
                        ],
                    });
                    despawn_overlays(&mut commands, &roots);
                    spawn_error(&mut commands, &state.message, state.kind);
                    dirty.0 = Some(ShellControlId::Retry);
                } else {
                    close_overlay(
                        &mut commands,
                        &mut state,
                        &mut client_overlay,
                        &roots,
                        &mut dirty,
                        ShellControlId::Settings,
                    );
                }
            }
            ShellAction::RetrySave => match save_settings(&path.0, *active_input, *active_shell) {
                Ok(()) => close_overlay(
                    &mut commands,
                    &mut state,
                    &mut client_overlay,
                    &roots,
                    &mut dirty,
                    ShellControlId::Settings,
                ),
                Err(error) => {
                    state.message = error;
                    state.kind = LocalSettingsErrorKind::Save;
                    despawn_overlays(&mut commands, &roots);
                    spawn_error(&mut commands, &state.message, state.kind);
                    dirty.0 = Some(ShellControlId::Retry);
                }
            },
            ShellAction::ContinueWithoutSaving => {
                if state.return_target == ErrorReturn::Settings
                    && state.kind == LocalSettingsErrorKind::Validation
                {
                    despawn_overlays(&mut commands, &roots);
                    *client_overlay = ClientOverlay::Settings;
                    spawn_settings(&mut commands);
                    dirty.0 = Some(ShellControlId::Apply);
                } else {
                    close_overlay(
                        &mut commands,
                        &mut state,
                        &mut client_overlay,
                        &roots,
                        &mut dirty,
                        ShellControlId::Settings,
                    );
                }
            }
            ShellAction::Cancel | ShellAction::Back => {
                if matches!(client_overlay.as_ref(), ClientOverlay::Error(_))
                    && state.return_target == ErrorReturn::Settings
                    && state.kind == LocalSettingsErrorKind::Validation
                {
                    despawn_overlays(&mut commands, &roots);
                    *client_overlay = ClientOverlay::Settings;
                    spawn_settings(&mut commands);
                    dirty.0 = Some(ShellControlId::Apply);
                    continue;
                }
                let focus = match client_overlay.as_ref() {
                    ClientOverlay::Credits => ShellControlId::Credits,
                    ClientOverlay::Settings | ClientOverlay::Error(_) => ShellControlId::Settings,
                    ClientOverlay::BuildEditor
                    | ClientOverlay::Confirmation(_)
                    | ClientOverlay::ChangeServerConfirmation
                    | ClientOverlay::LeaveConfirmation
                    | ClientOverlay::None => continue,
                };
                close_overlay(
                    &mut commands,
                    &mut state,
                    &mut client_overlay,
                    &roots,
                    &mut dirty,
                    focus,
                );
            }
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn restore_match_menu_after_settings(
    overlay: Res<ClientOverlay>,
    mut target: ResMut<SettingsReturnTarget>,
    mut context: ResMut<ClientInputContext>,
) {
    if *target == SettingsReturnTarget::MatchMenu && matches!(overlay.as_ref(), ClientOverlay::None)
    {
        *context = ClientInputContext::Menu;
        *target = SettingsReturnTarget::Title;
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
    overlay: &mut ClientOverlay,
    roots: &Query<Entity, With<OverlayRoot>>,
    dirty: &mut NavigationDirty,
    focus: ShellControlId,
) {
    despawn_overlays(commands, roots);
    commands.remove_resource::<InputSettingsDraft>();
    commands.remove_resource::<ShellSettingsDraft>();
    *overlay = ClientOverlay::None;
    state.kind = LocalSettingsErrorKind::Load;
    dirty.0 = Some(focus);
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn rebuild_navigation(
    overlay: Res<ClientOverlay>,
    mut dirty: ResMut<NavigationDirty>,
    buttons: Query<(Entity, &ShellButton, &UiGlobalTransform)>,
    mut map: ResMut<DirectionalNavigationMap>,
    mut focus: ResMut<InputFocus>,
) {
    let preferred = dirty.0.take();
    map.clear();
    let layer = active_layer(&overlay);
    let mut active: Vec<(Entity, ShellControlId, Vec2)> = buttons
        .iter()
        .filter_map(|(entity, button, transform)| {
            let (_, _, center) = transform.to_scale_angle_translation();
            (button.layer == layer && button.action.is_some())
                .then_some((entity, button.id, center))
        })
        .collect();
    active.sort_by_key(|(_, id, _)| *id as u8);
    add_spatial_navigation_edges(&mut map, &active);
    let current = focus.get();
    if let Some((entity, _, _)) = active
        .iter()
        .find(|(_, id, _)| preferred == Some(*id))
        .or_else(|| {
            active
                .iter()
                .find(|(entity, _, _)| current == Some(*entity))
        })
        .or_else(|| active.first())
        && current != Some(*entity)
    {
        focus.set(*entity, FocusCause::Navigated);
    }
}

fn add_spatial_navigation_edges(
    map: &mut DirectionalNavigationMap,
    active: &[(Entity, ShellControlId, Vec2)],
) {
    for (index, (entity, _, center)) in active.iter().enumerate() {
        for (direction, axis) in [
            (CompassOctant::North, Vec2::NEG_Y),
            (CompassOctant::South, Vec2::Y),
            (CompassOctant::West, Vec2::NEG_X),
            (CompassOctant::East, Vec2::X),
        ] {
            let candidate = active
                .iter()
                .enumerate()
                .filter(|(candidate_index, _)| *candidate_index != index)
                .filter_map(|(_, (candidate, id, candidate_center))| {
                    let delta = *candidate_center - *center;
                    let forward = delta.dot(axis);
                    (forward > 1.0).then_some((
                        *candidate,
                        delta.perp_dot(axis).abs() / forward,
                        delta.length_squared(),
                        *id as u8,
                    ))
                })
                .min_by(|left, right| {
                    left.1
                        .total_cmp(&right.1)
                        .then_with(|| left.2.total_cmp(&right.2))
                        .then_with(|| left.3.cmp(&right.3))
                })
                .map(|(candidate, _, _, _)| candidate)
                .or_else(|| stable_navigation_fallback(active, index, direction));
            if let Some(candidate) = candidate {
                map.add_edge(*entity, candidate, direction);
            }
        }
    }
}

fn stable_navigation_fallback(
    active: &[(Entity, ShellControlId, Vec2)],
    index: usize,
    direction: CompassOctant,
) -> Option<Entity> {
    if active.len() < 2 {
        return None;
    }
    let next = matches!(direction, CompassOctant::South | CompassOctant::East);
    let target = if next {
        (index + 1) % active.len()
    } else {
        (index + active.len() - 1) % active.len()
    };
    Some(active[target].0)
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
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let settings = draft.as_deref().map_or(*active, |draft| draft.0);
    scale.0 = settings.ui_scale;
    for mut window in &mut windows {
        window.mode = if settings.fullscreen {
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        } else {
            WindowMode::Windowed
        };
        window.present_mode = if settings.vsync {
            PresentMode::AutoVsync
        } else {
            PresentMode::AutoNoVsync
        };
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn update_shell_settings_text(
    active: Res<ClientShellSettings>,
    draft: Option<Res<ShellSettingsDraft>>,
    mut texts: Query<&mut Text, With<ShellSettingsText>>,
) {
    let settings = draft.as_deref().map_or(*active, |draft| draft.0);
    let on_off = |value| if value { "ON" } else { "OFF" };
    for mut text in &mut texts {
        text.0 = format!(
            "UI SCALE  {:.1}    REDUCED MOTION  {}    REDUCED EFFECTS  {}\nMASTER VOLUME  {}%    MUTE UNFOCUSED  {}\nDISPLAY  {}    VSYNC  {}",
            settings.ui_scale,
            on_off(settings.reduced_motion),
            on_off(settings.reduced_combat_effects),
            settings.master_volume,
            on_off(settings.mute_when_unfocused),
            if settings.fullscreen {
                "BORDERLESS FULLSCREEN"
            } else {
                "WINDOWED"
            },
            on_off(settings.vsync),
        );
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn apply_audio_preferences(
    settings: Res<ClientShellSettings>,
    windows: Query<&Window, With<PrimaryWindow>>,
    global: Option<ResMut<GlobalVolume>>,
    mut sinks: Query<&mut AudioSink>,
    mut spatial_sinks: Query<&mut SpatialAudioSink>,
) {
    let volume = Volume::Linear(f32::from(settings.master_volume) / 100.0);
    if let Some(mut global) = global {
        global.volume = volume;
    }
    let muted =
        settings.mute_when_unfocused && windows.iter().next().is_some_and(|window| !window.focused);
    for mut sink in &mut sinks {
        sink.set_volume(volume);
        if muted {
            sink.mute();
        } else {
            sink.unmute();
        }
    }
    for mut sink in &mut spatial_sinks {
        sink.set_volume(volume);
        if muted {
            sink.mute();
        } else {
            sink.unmute();
        }
    }
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
    use crate::client::ClientSettingsUiSet;
    use crate::client::settings::ui::adjust_input_settings_from_pause_keys;
    use bevy::input_focus::directional_navigation::NavNeighbor;

    fn spawn_legacy_title_fixture(mut commands: Commands) {
        spawn_title(&mut commands);
    }

    fn shell_test_app(path: std::path::PathBuf) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .add_message::<MouseWheel>()
            .add_message::<AppExit>()
            .insert_resource(ClientSettingsPath(path))
            .insert_resource(UiScale::default())
            .insert_resource(ButtonInput::<KeyCode>::default())
            .init_resource::<ClientInputContext>()
            .init_resource::<ClientInputSettings>()
            .init_resource::<InputSettingsSelection>()
            .init_state::<ClientFlow>()
            .init_resource::<ClientOverlay>()
            .init_resource::<ClientLocalLoadFailures>()
            .configure_sets(
                Update,
                (
                    ClientSettingsUiSet::Capture,
                    ClientSettingsUiSet::Shell,
                    ClientSettingsUiSet::Present,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                adjust_input_settings_from_pause_keys.in_set(ClientSettingsUiSet::Capture),
            )
            .add_plugins(ClientShellPlugin);
        app.add_systems(OnEnter(ClientFlow::Dashboard), spawn_legacy_title_fixture);
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::Dashboard);
        app
    }

    #[test]
    fn shell_startup_exposes_practice_and_overlay_replaces_navigation_root() {
        let path = std::env::temp_dir().join(format!(
            "brawler-m02-shell-missing-{}-settings.ron",
            std::process::id()
        ));
        let mut app = shell_test_app(path);

        app.update();
        assert_eq!(
            *app.world().resource::<ClientInputContext>(),
            ClientInputContext::Shell
        );
        let focus = app.world().resource::<InputFocus>().get().unwrap();
        assert_eq!(
            app.world().get::<ShellButton>(focus).unwrap().id,
            ShellControlId::Play
        );
        let practice_is_enabled = {
            let world = app.world_mut();
            let mut query = world.query::<&ShellButton>();
            query
                .iter(world)
                .find(|button| button.id == ShellControlId::Practice)
                .is_some_and(|button| button.action == Some(ShellAction::Practice))
        };
        assert!(practice_is_enabled);

        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::OpenSettings);
        app.update();
        assert_eq!(
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::Settings
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
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::None
        );
        assert!(path.is_file());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn controller_dpad_selects_practice_and_south_activates_once() {
        let path = std::env::temp_dir().join(format!(
            "brawler-m02-shell-controller-{}-settings.ron",
            std::process::id()
        ));
        let mut app = shell_test_app(path);
        app.update();
        let mut gamepad = Gamepad::default();
        gamepad.digital_mut().press(GamepadButton::DPadDown);
        gamepad.digital_mut().press(GamepadButton::South);
        app.world_mut().spawn(gamepad);

        app.update();
        assert_eq!(
            *app.world().resource::<SessionPurpose>(),
            SessionPurpose::Practice
        );
        assert_eq!(
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::None
        );
        let overlay_count = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<OverlayRoot>>();
            query.iter(world).count()
        };
        assert_eq!(overlay_count, 0);
    }

    #[test]
    fn spatial_navigation_uses_cardinal_layout_neighbors() {
        let mut world = World::new();
        let settings = world.spawn_empty().id();
        let credits = world.spawn_empty().id();
        let apply = world.spawn_empty().id();
        let cancel = world.spawn_empty().id();
        let active = [
            (settings, ShellControlId::Settings, Vec2::new(0.0, 0.0)),
            (credits, ShellControlId::Credits, Vec2::new(100.0, 0.0)),
            (apply, ShellControlId::Apply, Vec2::new(0.0, 100.0)),
            (cancel, ShellControlId::Cancel, Vec2::new(100.0, 100.0)),
        ];
        let mut map = DirectionalNavigationMap::default();
        add_spatial_navigation_edges(&mut map, &active);
        assert_eq!(
            map.get_neighbor(settings, CompassOctant::East),
            NavNeighbor::Set(credits)
        );
        assert_eq!(
            map.get_neighbor(settings, CompassOctant::South),
            NavNeighbor::Set(apply)
        );
        assert_eq!(
            map.get_neighbor(cancel, CompassOctant::West),
            NavNeighbor::Set(apply)
        );
        assert_eq!(
            map.get_neighbor(cancel, CompassOctant::North),
            NavNeighbor::Set(credits)
        );
    }

    #[test]
    fn horizontal_keyboard_and_left_stick_move_title_focus() {
        let keyboard_path = std::env::temp_dir().join(format!(
            "brawler-m02-shell-horizontal-keyboard-{}-settings.ron",
            std::process::id()
        ));
        let mut keyboard_app = shell_test_app(keyboard_path);
        keyboard_app.update();
        keyboard_app
            .world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowRight);
        keyboard_app.update();
        let focus = keyboard_app.world().resource::<InputFocus>().get().unwrap();
        assert_eq!(
            keyboard_app.world().get::<ShellButton>(focus).unwrap().id,
            ShellControlId::Practice
        );

        let stick_path = std::env::temp_dir().join(format!(
            "brawler-m02-shell-horizontal-stick-{}-settings.ron",
            std::process::id()
        ));
        let mut stick_app = shell_test_app(stick_path);
        stick_app.update();
        let mut gamepad = Gamepad::default();
        gamepad.analog_mut().set(GamepadAxis::LeftStickX, 1.0);
        stick_app.world_mut().spawn(gamepad);
        stick_app.update();
        let focus = stick_app.world().resource::<InputFocus>().get().unwrap();
        assert_eq!(
            stick_app.world().get::<ShellButton>(focus).unwrap().id,
            ShellControlId::Practice
        );
    }

    #[test]
    fn escape_and_controller_east_cancel_product_rebind_without_changing_the_draft() {
        let path = std::env::temp_dir().join(format!(
            "brawler-m02-shell-rebind-cancel-{}-settings.ron",
            std::process::id()
        ));
        let mut app = shell_test_app(path);
        app.update();
        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::OpenSettings);
        app.update();
        app.world_mut()
            .resource_mut::<InputSettingsSelection>()
            .field = InputSettingsField::Keyboard(crate::client::KeyboardAction::Ultimate);
        let original = app
            .world()
            .resource::<InputSettingsDraft>()
            .0
            .keyboard
            .ultimate;
        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::Rebind);
        app.update();
        assert!(app.world().resource::<InputSettingsSelection>().listening);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert!(!app.world().resource::<InputSettingsSelection>().listening);
        assert_eq!(
            app.world()
                .resource::<InputSettingsDraft>()
                .0
                .keyboard
                .ultimate,
            original
        );

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::Rebind);
        app.update();
        assert!(app.world().resource::<InputSettingsSelection>().listening);

        let mut gamepad = Gamepad::default();
        gamepad.digital_mut().press(GamepadButton::East);
        app.world_mut().spawn(gamepad);
        app.update();
        assert!(!app.world().resource::<InputSettingsSelection>().listening);
        assert_eq!(
            app.world()
                .resource::<InputSettingsDraft>()
                .0
                .keyboard
                .ultimate,
            original
        );
    }

    #[test]
    fn keyboard_b_is_a_valid_product_rebind() {
        let path = std::env::temp_dir().join(format!(
            "brawler-m02-shell-rebind-b-{}-settings.ron",
            std::process::id()
        ));
        let mut app = shell_test_app(path);
        app.update();
        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::OpenSettings);
        app.update();
        app.world_mut()
            .resource_mut::<InputSettingsSelection>()
            .field = InputSettingsField::Keyboard(crate::client::KeyboardAction::Ultimate);
        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::Rebind);
        app.update();
        assert!(app.world().resource::<InputSettingsSelection>().listening);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyB);
        app.update();
        assert!(!app.world().resource::<InputSettingsSelection>().listening);
        assert_eq!(
            app.world()
                .resource::<InputSettingsDraft>()
                .0
                .keyboard
                .ultimate,
            KeyCode::KeyB
        );
    }

    #[test]
    fn rebind_activation_press_is_not_captured_in_the_same_frame() {
        let path = std::env::temp_dir().join(format!(
            "brawler-m02-shell-rebind-order-{}-settings.ron",
            std::process::id()
        ));
        let mut app = shell_test_app(path);
        app.update();
        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::OpenSettings);
        app.update();
        app.world_mut()
            .resource_mut::<InputSettingsSelection>()
            .field = InputSettingsField::Gamepad(crate::client::GamepadAction::Ultimate);
        let rebind = {
            let world = app.world_mut();
            let mut query = world.query::<(Entity, &ShellButton)>();
            query
                .iter(world)
                .find_map(|(entity, button)| {
                    (button.id == ShellControlId::Rebind).then_some(entity)
                })
                .unwrap()
        };
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(rebind, FocusCause::Navigated);
        let original = app
            .world()
            .resource::<InputSettingsDraft>()
            .0
            .gamepad
            .ultimate;
        let mut gamepad = Gamepad::default();
        gamepad.digital_mut().press(GamepadButton::South);
        app.world_mut().spawn(gamepad);
        app.update();

        assert!(app.world().resource::<InputSettingsSelection>().listening);
        assert_eq!(
            app.world()
                .resource::<InputSettingsDraft>()
                .0
                .gamepad
                .ultimate,
            original
        );
    }

    #[test]
    fn save_failure_keeps_session_values_and_continue_closes_the_error() {
        let dir = std::env::temp_dir().join(format!(
            "brawler-m02-shell-save-recovery-{}",
            std::process::id()
        ));
        let blocker = dir.join("blocker");
        let path = blocker.join("settings.ron");
        let mut app = shell_test_app(path);
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
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&blocker, "blocks settings directory").unwrap();
        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::Apply);
        app.update();

        assert!(app.world().resource::<ClientInputSettings>().invert_aim_y);
        assert_eq!(
            app.world().resource::<ShellState>().kind,
            LocalSettingsErrorKind::Save
        );
        assert!(matches!(
            app.world().resource::<ClientOverlay>(),
            ClientOverlay::Error(_)
        ));
        let copy_is_accurate = {
            let world = app.world_mut();
            let mut query = world.query::<&Text>();
            query
                .iter(world)
                .any(|text| text.0.contains("remain active for this session"))
        };
        assert!(copy_is_accurate);

        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::RetrySave);
        app.update();
        assert!(matches!(
            app.world().resource::<ClientOverlay>(),
            ClientOverlay::Error(_)
        ));
        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::ContinueWithoutSaving);
        app.update();
        assert_eq!(
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::None
        );
        assert!(app.world().resource::<ClientInputSettings>().invert_aim_y);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn retry_save_closes_after_the_destination_is_repaired() {
        let dir = std::env::temp_dir().join(format!(
            "brawler-m02-shell-retry-recovery-{}",
            std::process::id()
        ));
        let blocker = dir.join("blocker");
        let path = blocker.join("settings.ron");
        let mut app = shell_test_app(path.clone());
        app.update();
        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::OpenSettings);
        app.update();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&blocker, "blocks settings directory").unwrap();
        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::Apply);
        app.update();
        std::fs::remove_file(&blocker).unwrap();
        std::fs::create_dir_all(&blocker).unwrap();
        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::RetrySave);
        app.update();
        assert_eq!(
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::None
        );
        assert!(path.is_file());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn validation_error_retains_the_unapplied_draft_and_returns_to_settings() {
        let path = std::env::temp_dir().join(format!(
            "brawler-m02-shell-validation-{}-settings.ron",
            std::process::id()
        ));
        let mut app = shell_test_app(path);
        app.update();
        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::OpenSettings);
        app.update();
        app.world_mut()
            .resource_mut::<ShellSettingsDraft>()
            .0
            .ui_scale = f32::NAN;
        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::Apply);
        app.update();
        assert_eq!(
            app.world().resource::<ShellState>().kind,
            LocalSettingsErrorKind::Validation
        );
        assert!(
            (app.world().resource::<ClientShellSettings>().ui_scale - 1.0).abs() < f32::EPSILON
        );
        assert!(
            app.world()
                .resource::<ShellSettingsDraft>()
                .0
                .ui_scale
                .is_nan()
        );

        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::ContinueWithoutSaving);
        app.update();
        assert_eq!(
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::Settings
        );
        assert!(app.world().contains_resource::<ShellSettingsDraft>());
    }

    #[test]
    fn active_layer_traps_focus_to_the_visible_overlay() {
        assert_eq!(active_layer(&ClientOverlay::None), ShellLayer::Title);
        assert_eq!(active_layer(&ClientOverlay::Settings), ShellLayer::Settings);
        assert_eq!(active_layer(&ClientOverlay::Credits), ShellLayer::Credits);
        assert_eq!(
            active_layer(&ClientOverlay::Error(FlowError {
                kind: FlowErrorKind::Connection,
                message: String::new(),
                return_flow: ClientFlow::Dashboard,
                actions: [Some(FlowErrorAction::Back), None],
            })),
            ShellLayer::Error
        );
    }

    #[test]
    fn entrance_finishes_at_identity_with_or_without_motion() {
        for reduced in [false, true] {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
                .insert_resource(ClientShellSettings {
                    ui_scale: 1.0,
                    reduced_motion: reduced,
                    ..ClientShellSettings::default()
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
