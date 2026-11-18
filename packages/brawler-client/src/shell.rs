//! Functional windowed product overlays: focus, settings draft, credits, and errors.

use super::flow::ClientFlowSet;
use super::flow::ClientLocalLoadFailures;
use super::ui::feedback::UiFeedback;
use super::{
    ClientFlow, ClientInputContext, ClientOverlay, InputCaptureConsumed, InputSettingsSelection,
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
    window::{MonitorSelection, PresentMode, PrimaryWindow, WindowMode},
};

mod controls;
mod credits;
mod settings_screen;

const ENTRANCE_SECONDS: f32 = 0.16;

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
struct OverlayRoot;

#[derive(Component)]
struct ShellScrollArea;

#[derive(Component)]
struct EntranceAnimation {
    elapsed: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShellLayer {
    Settings,
    Credits,
    Error,
    FlowOwned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ShellControlId {
    Settings,
    Credits,
    DisplayCategory,
    AudioCategory,
    AccessibilityCategory,
    ControlsCategory,
    RetryPreferences,
    ToggleReducedMotion,
    ToggleReducedEffects,
    VolumeUp,
    EffectsVolumeUp,
    MusicVolumeUp,
    CameraShakeUp,
    ToggleFocusMute,
    ToggleFullscreen,
    ToggleVsync,
    GraphicsQuality,
    UiScaleUp,
    ResetPreferences,
    UndoPreferences,
    KeepDisplay,
    RevertDisplay,
    Cancel,
    CreditsBack,
    Input(controls::InputControl),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShellAction {
    DisplayCategory,
    AudioCategory,
    AccessibilityCategory,
    ControlsCategory,
    RetryPreferences,
    OpenMatchSettings,
    UiScaleDown,
    UiScaleUp,
    ToggleReducedMotion,
    ToggleReducedEffects,
    VolumeDown,
    VolumeUp,
    EffectsVolumeDown,
    EffectsVolumeUp,
    MusicVolumeDown,
    MusicVolumeUp,
    CameraShakeDown,
    CameraShakeUp,
    ToggleFocusMute,
    ToggleFullscreen,
    ToggleVsync,
    CycleGraphicsQuality,
    ResetPreferences,
    UndoPreferences,
    KeepDisplay,
    RevertDisplay,
    Input(controls::InputControl),
    InputAdjust(controls::InputControl, i8),
    Back,
}

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SettingsReturnTarget {
    #[default]
    ProductFlow,
    MatchMenu,
}

#[derive(Component)]
#[require(
    super::ui::press::ReleaseActivation,
    super::ui::touch_target::TouchTarget
)]
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
        super::ui::press::add_release_activation(app);
        super::ui::touch_target::add_touch_targets(app);
        super::back::add_back_input(app);
        super::ui::feedback::add_ui_feedback(app);
        app.add_plugins((
            DirectionalNavigationPlugin,
            super::ui::controls::UiControlsPlugin,
        ))
        .init_resource::<settings_screen::SettingsPage>()
        .init_resource::<controls::ControlsState>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<InputFocus>()
        .insert_resource(InputFocusVisible(true))
        .init_resource::<DirectionalNavigationMap>()
        .init_resource::<ClientShellSettings>()
        .init_resource::<ClientSettingsPath>()
        .init_resource::<PendingActions>()
        .init_resource::<NavigationLatch>()
        .init_resource::<NavigationDirty>()
        .init_resource::<InputCaptureConsumed>()
        .init_resource::<super::MatchSettingsRequest>()
        .init_resource::<SettingsReturnTarget>()
        .add_systems(Startup, load_persistent_settings)
        .add_systems(OnEnter(ClientFlow::Dashboard), enter_dashboard_shell)
        .add_systems(OnExit(ClientFlow::Match), forget_match_settings_return)
        .add_systems(
            Update,
            (
                (
                    controls::capture_input
                        .after(super::settings::ui::adjust_input_settings_from_pause_keys),
                    collect_navigation_input,
                    collect_pointer_actions,
                    settings_screen::pointer_sliders,
                    controls::pointer_sliders,
                    collect_match_settings_request,
                )
                    .chain()
                    .in_set(ClientFlowSet::CollectFlowInput),
                (
                    controls::actions,
                    settings_screen::choose_category,
                    settings_screen::preference_actions,
                    settings_screen::slider_step_feedback,
                    settings_screen::autosave,
                    handle_shell_actions,
                )
                    .chain()
                    .in_set(ClientFlowSet::ResolveFlowAction),
                (
                    controls::sample_sticks,
                    settings_screen::responsive,
                    settings_screen::refresh,
                    close_externally_cleared_overlay,
                    present_flow_requested_overlay,
                    restore_match_menu_after_settings,
                    rebuild_navigation,
                    style_shell_buttons,
                    settings_screen::present_retry,
                    settings_screen::present,
                    controls::present_values,
                    controls::present_plots,
                    preview_shell_preferences,
                    animate_shell_entrance,
                    scroll_shell_panels,
                )
                    .chain()
                    .in_set(ClientFlowSet::PresentFlow),
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
            warn!("Unable to load local settings: {error}");
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

/// Leaving the match ends its menu, so Settings closed later must not reopen it.
fn forget_match_settings_return(mut target: ResMut<SettingsReturnTarget>) {
    *target = SettingsReturnTarget::ProductFlow;
}

/// BRL-nqnm: the flow may replace or clear the overlay under the shell (match completion,
/// a connection error). The shell then closes its Settings or Credits root exactly as Back
/// does; `settings_screen::autosave` has already committed the draft earlier in the frame.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn close_externally_cleared_overlay(
    mut commands: Commands,
    overlay: Res<ClientOverlay>,
    roots: Query<Entity, With<OverlayRoot>>,
) {
    if roots.is_empty()
        || matches!(
            overlay.as_ref(),
            ClientOverlay::Settings | ClientOverlay::Credits
        )
    {
        return;
    }
    discard_overlays(&mut commands, &roots);
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
            *settings_return = SettingsReturnTarget::ProductFlow;
            if input_draft.is_none() {
                commands.insert_resource(InputSettingsDraft(*active_input));
            }
            if shell_draft.is_none() {
                commands.insert_resource(ShellSettingsDraft(*active_shell));
            }
            selection.listening = false;
            spawn_settings(&mut commands);
            dirty.0 = Some(ShellControlId::DisplayCategory);
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

fn spawn_overlay(
    commands: &mut Commands,
    title: &str,
    build: impl FnOnce(&mut ChildSpawnerCommands),
) {
    commands
        .spawn((
            OverlayRoot,
            root_node(),
            BackgroundColor(super::ui::theme::INK.with_alpha(0.94)),
            GlobalZIndex(500),
            EntranceAnimation { elapsed: 0.0 },
            UiTransform::from_translation(Val2::new(px(0), px(14))),
        ))
        .with_children(|root| {
            root.spawn((
                ShellScrollArea,
                ScrollPosition::default(),
                panel_node(),
                BackgroundColor(super::ui::theme::PANEL),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new(title),
                    TextFont::from_font_size(32.0),
                    TextColor(super::ui::theme::ACCENT),
                ));
                build(panel);
            });
        });
}

fn spawn_settings(commands: &mut Commands) {
    settings_screen::spawn(commands);
}

fn spawn_credits(commands: &mut Commands) {
    spawn_overlay(commands, "CREDITS", |panel| {
        panel.spawn((
            Text::new(credits::credits_text()),
            TextFont::from_font_size(16.0),
            TextColor(super::ui::theme::TEXT),
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

                ..default()
            },
        ))
        .with_child((
            Text::new(label),
            TextFont::from_font_size(15.0),
            TextColor(if action.is_some() {
                super::ui::theme::TEXT
            } else {
                super::ui::theme::TEXT.with_alpha(0.45)
            }),
        ));
}

fn active_layer(overlay: &ClientOverlay) -> ShellLayer {
    match overlay {
        ClientOverlay::Settings => ShellLayer::Settings,
        ClientOverlay::Credits => ShellLayer::Credits,
        ClientOverlay::Error(_) => ShellLayer::Error,
        ClientOverlay::None
        | ClientOverlay::DashboardMenu
        | ClientOverlay::BrawlerList
        | ClientOverlay::BrawlerDetails(_)
        | ClientOverlay::BrawlerCreation
        | ClientOverlay::BrawlerEditor
        | ClientOverlay::WeaponEquipment
        | ClientOverlay::DeleteBrawlerConfirmation(_)
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
    platform_back: Res<super::BackInput>,
    gamepads: Query<&Gamepad>,
    selection: Res<InputSettingsSelection>,
    capture_consumed: Res<InputCaptureConsumed>,
    overlay: Res<ClientOverlay>,
    mut latch: ResMut<NavigationLatch>,
    mut navigation: DirectionalNavigation,
    buttons: Query<(Entity, &ShellButton)>,
    page: Res<settings_screen::SettingsPage>,
    controls_state: Res<controls::ControlsState>,
    mut pending: ResMut<PendingActions>,
    surfaces: Query<&super::ui::surface::Surface>,
    mut feedback: MessageWriter<UiFeedback>,
) {
    if selection.listening || capture_consumed.0 {
        return;
    }
    let pad_pressed = |button| gamepads.iter().any(|pad| pad.just_pressed(button));
    let stick = strongest_stick(&gamepads);
    let stick = if *overlay == ClientOverlay::Settings && controls_state.testing_sticks() {
        Vec2::ZERO
    } else {
        stick
    };
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
    if *overlay == ClientOverlay::Settings
        && (keyboard.just_pressed(KeyCode::Tab)
            || pad_pressed(GamepadButton::LeftTrigger)
            || pad_pressed(GamepadButton::RightTrigger))
    {
        let before = navigation.focus.get();
        transfer_settings_focus(
            &keyboard,
            pad_pressed(GamepadButton::RightTrigger),
            page.category.control().0,
            &buttons,
            &controls_state,
            &mut navigation.focus,
        );
        if navigation.focus.get() != before {
            feedback.write(UiFeedback::Moved);
        }
    }
    for (pressed, octant) in [(up, CompassOctant::North), (down, CompassOctant::South)] {
        navigate_with_feedback(pressed, &mut navigation, octant, &mut feedback);
    }
    let slider = navigation
        .focus
        .get()
        .and_then(|entity| buttons.get(entity).ok())
        .filter(|(_, button)| button.layer == active_layer(&overlay))
        .and_then(|(_, button)| settings_screen::slider_actions(button.id));
    if let Some((decrease, increase)) = slider {
        if left {
            pending.0.push(decrease);
        }
        if right {
            pending.0.push(increase);
        }
    } else {
        for (pressed, octant) in [(left, CompassOctant::West), (right, CompassOctant::East)] {
            navigate_with_feedback(pressed, &mut navigation, octant, &mut feedback);
        }
    }

    let activate = keyboard.any_just_pressed([KeyCode::Enter, KeyCode::Space])
        || pad_pressed(GamepadButton::South);
    if activate
        && slider.is_none()
        && let Some(entity) = navigation.focus.get()
        && let Ok((_, button)) = buttons.get(entity)
        && button.layer == active_layer(&overlay)
        && controls::allows(button.id, &controls_state)
        && let Some(action) = button.action
        && !pending.0.contains(&action)
    {
        pending.0.push(action);
        feedback.write(UiFeedback::Activated(surfaces.get(entity).ok().copied()));
    }
    let back = !capture_consumed.0
        && (keyboard.just_pressed(KeyCode::Escape)
            || pad_pressed(GamepadButton::East)
            || platform_back.just_pressed());
    if back
        && !matches!(overlay.as_ref(), ClientOverlay::None)
        && !pending.0.contains(&ShellAction::Back)
    {
        pending.0.push(ShellAction::Back);
        // Flow-owned overlays publish their own Back feedback where the flow resolves it.
        if active_layer(&overlay) != ShellLayer::FlowOwned {
            feedback.write(UiFeedback::Back);
        }
    }
}

/// Moves focus one step when `pressed`; only a move that lands on another control ticks.
fn navigate_with_feedback(
    pressed: bool,
    navigation: &mut DirectionalNavigation,
    octant: CompassOctant,
    feedback: &mut MessageWriter<UiFeedback>,
) {
    if pressed && navigation.navigate(octant).is_ok() {
        feedback.write(UiFeedback::Moved);
    }
}

fn strongest_stick(gamepads: &Query<&Gamepad>) -> Vec2 {
    gamepads
        .iter()
        .map(Gamepad::left_stick)
        .fold(Vec2::ZERO, |best, sample| {
            if sample.length_squared() > best.length_squared() {
                sample
            } else {
                best
            }
        })
}

fn transfer_settings_focus(
    keyboard: &ButtonInput<KeyCode>,
    right_bumper: bool,
    category: ShellControlId,
    buttons: &Query<(Entity, &ShellButton)>,
    state: &controls::ControlsState,
    focus: &mut InputFocus,
) {
    let on_category = focus
        .get()
        .and_then(|entity| buttons.get(entity).ok())
        .is_some_and(|(_, button)| button.id == category);
    let enter_content = right_bumper || (keyboard.just_pressed(KeyCode::Tab) && on_category);
    let target = buttons
        .iter()
        .filter(|(_, button)| {
            button.layer == ShellLayer::Settings
                && button.action.is_some()
                && controls::allows(button.id, state)
        })
        .filter(|(_, button)| {
            if enter_content {
                !matches!(
                    button.id,
                    ShellControlId::DisplayCategory
                        | ShellControlId::AudioCategory
                        | ShellControlId::AccessibilityCategory
                        | ShellControlId::ControlsCategory
                        | ShellControlId::Cancel
                )
            } else {
                button.id == category
            }
        })
        .min_by_key(|(_, button)| button.id)
        .map(|(entity, _)| entity);
    if let Some(entity) = target {
        focus.set(entity, FocusCause::Navigated);
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn collect_pointer_actions(
    interactions: Query<(
        Entity,
        Ref<Interaction>,
        &super::ui::press::ReleaseActivation,
        &ShellButton,
    )>,
    overlay: Res<ClientOverlay>,
    controls_state: Res<controls::ControlsState>,
    consumed: Res<InputCaptureConsumed>,
    mut focus: ResMut<InputFocus>,
    mut pending: ResMut<PendingActions>,
) {
    if consumed.0 {
        return;
    }
    for (entity, interaction, release, button) in &interactions {
        if button.layer != active_layer(&overlay) || !controls::allows(button.id, &controls_state) {
            continue;
        }
        if interaction.is_changed()
            && matches!(*interaction, Interaction::Hovered | Interaction::Pressed)
            && button.action.is_some()
        {
            focus.set(entity, FocusCause::Pressed);
        }
        // Sliders adjust while held; every other shell action fires on an undragged release.
        if release.activated()
            && settings_screen::slider_actions(button.id).is_none()
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
    reason = "Shell lifecycle boundary"
)]
fn handle_shell_actions(
    mut commands: Commands,
    mut pending: ResMut<PendingActions>,
    roots: Query<Entity, With<OverlayRoot>>,
    active_input: Res<ClientInputSettings>,
    active_shell: Res<ClientShellSettings>,
    mut dirty: ResMut<NavigationDirty>,
    mut client_overlay: ResMut<ClientOverlay>,
    mut context: ResMut<ClientInputContext>,
    mut settings_return: ResMut<SettingsReturnTarget>,
) {
    for action in core::mem::take(&mut pending.0) {
        match action {
            ShellAction::OpenMatchSettings => {
                *context = ClientInputContext::Shell;
                *settings_return = SettingsReturnTarget::MatchMenu;
                *client_overlay = ClientOverlay::Settings;
                commands.insert_resource(InputSettingsDraft(*active_input));
                commands.insert_resource(ShellSettingsDraft(*active_shell));
                despawn_overlays(&mut commands, &roots);
                spawn_settings(&mut commands);
                dirty.0 = Some(ShellControlId::DisplayCategory);
            }
            ShellAction::Back
                if matches!(
                    *client_overlay,
                    ClientOverlay::Settings | ClientOverlay::Credits
                ) =>
            {
                let focus = if *client_overlay == ClientOverlay::Credits {
                    ShellControlId::Credits
                } else {
                    ShellControlId::Settings
                };
                close_overlay(
                    &mut commands,
                    &mut client_overlay,
                    &roots,
                    &mut dirty,
                    focus,
                );
            }
            _ => {}
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn restore_match_menu_after_settings(
    overlay: Res<ClientOverlay>,
    flow: Res<State<ClientFlow>>,
    next_flow: Res<NextState<ClientFlow>>,
    mut target: ResMut<SettingsReturnTarget>,
    mut context: ResMut<ClientInputContext>,
) {
    if *target == SettingsReturnTarget::MatchMenu && matches!(overlay.as_ref(), ClientOverlay::None)
    {
        // A match that is ending under Settings has no menu to return to (BRL-nqnm).
        let leaving_match = match next_flow.as_ref() {
            NextState::Pending(next) | NextState::PendingIfNeq(next) => *next != ClientFlow::Match,
            NextState::Unchanged => false,
        };
        if *flow.get() == ClientFlow::Match && !leaving_match {
            *context = ClientInputContext::Menu;
        }
        *target = SettingsReturnTarget::ProductFlow;
    }
}

fn despawn_overlays(commands: &mut Commands, roots: &Query<Entity, With<OverlayRoot>>) {
    for entity in roots {
        commands.entity(entity).despawn();
    }
}

/// Despawns the shell overlay roots and their settings drafts.
fn discard_overlays(commands: &mut Commands, roots: &Query<Entity, With<OverlayRoot>>) {
    despawn_overlays(commands, roots);
    commands.remove_resource::<InputSettingsDraft>();
    commands.remove_resource::<ShellSettingsDraft>();
}

fn close_overlay(
    commands: &mut Commands,
    overlay: &mut ClientOverlay,
    roots: &Query<Entity, With<OverlayRoot>>,
    dirty: &mut NavigationDirty,
    focus: ShellControlId,
) {
    discard_overlays(commands, roots);
    *overlay = ClientOverlay::None;
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
    controls_state: Res<controls::ControlsState>,
    mut focus: ResMut<InputFocus>,
) {
    let preferred = dirty.0.take();
    map.clear();
    let layer = active_layer(&overlay);
    let mut active: Vec<(Entity, ShellControlId, Vec2)> = buttons
        .iter()
        .filter_map(|(entity, button, transform)| {
            let (_, _, center) = transform.to_scale_angle_translation();
            (button.layer == layer
                && button.action.is_some()
                && controls::allows(button.id, &controls_state))
            .then_some((entity, button.id, center))
        })
        .collect();
    active.sort_by_key(|(_, id, _)| *id);
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
                        *id,
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
    mut commands: Commands,
    focus: Res<InputFocus>,
    overlay: Res<ClientOverlay>,
    mut buttons: Query<
        (
            Entity,
            &ShellButton,
            Option<&mut super::ui::surface::SurfaceState>,
        ),
        Without<super::ui::controls::ControlValue>,
    >,
) {
    use super::ui::surface::{Surface, SurfaceState};
    for (entity, button, state) in &mut buttons {
        let next = SurfaceState {
            focused: focus.get() == Some(entity),
            inactive: button.layer != active_layer(&overlay),
            disabled: button.action.is_none(),
            ..default()
        };
        if let Some(mut state) = state {
            *state = next;
        } else {
            commands.entity(entity).insert((
                if button.action == Some(ShellAction::Back) {
                    Surface::Back
                } else {
                    Surface::Action
                },
                next,
            ));
        }
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
    let fit = windows.iter().next().map_or(1.0, |window| {
        super::ui::viewport::viewport_fit(window.height())
    });
    let next_scale = settings.ui_scale * fit;
    if (scale.0 - next_scale).abs() > f32::EPSILON {
        scale.0 = next_scale;
    }
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
    mut panels: Query<
        (
            &ComputedNode,
            Option<&bevy::ui::RelativeCursorPosition>,
            &mut ScrollPosition,
        ),
        With<ShellScrollArea>,
    >,
) {
    let delta = wheel
        .read()
        .map(|event| match event.unit {
            MouseScrollUnit::Line => event.y * 32.0,
            MouseScrollUnit::Pixel => event.y,
        })
        .sum::<f32>();
    if delta.abs() <= f32::EPSILON {
        return;
    }
    for (node, cursor, mut position) in &mut panels {
        if cursor.is_some_and(|cursor| !cursor.cursor_over()) {
            continue;
        }
        let maximum =
            ((node.content_size().y - node.size().y) * node.inverse_scale_factor()).max(0.0);
        position.0.y = (position.0.y - delta).clamp(0.0, maximum);
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn keep_focused_control_visible(
    focus: Res<InputFocus>,
    buttons: Query<(&ComputedNode, &UiGlobalTransform), With<ShellButton>>,
    parents: Query<&ChildOf>,
    mut panels: Query<
        (
            Entity,
            &ComputedNode,
            &UiGlobalTransform,
            &mut ScrollPosition,
        ),
        With<ShellScrollArea>,
    >,
    mut revealed: Local<Option<Entity>>,
) {
    let Some(focused) = focus.get() else {
        *revealed = None;
        return;
    };
    // Reveal once per focus change: re-clamping every frame would fight touch drag-scrolling,
    // which may legitimately move the focused row out of view (BRL-0196).
    if *revealed == Some(focused) {
        return;
    }
    let Ok((button_node, button_transform)) = buttons.get(focused) else {
        return;
    };
    if button_node.is_empty() {
        return;
    }
    *revealed = Some(focused);
    let (_, _, button_center) = button_transform.to_scale_angle_translation();
    let button_half_height = button_node.size().y * 0.5;
    for (panel_entity, panel_node, panel_transform, mut scroll) in &mut panels {
        if !parents
            .iter_ancestors(focused)
            .any(|parent| parent == panel_entity)
        {
            continue;
        }
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

/// Lets flow tests compose the real shell over the product flow (BRL-nqnm).
#[cfg(test)]
pub(crate) mod flow_test_support {
    use super::*;
    use crate::ClientSettingsUiSet;
    use crate::settings::ui::adjust_input_settings_from_pause_keys;

    /// Adds the shell and the resources a windowed app gives it; the caller owns `ClientFlow`.
    pub(crate) fn add_shell(app: &mut App, path: std::path::PathBuf) {
        app.add_message::<MouseWheel>()
            .add_message::<AppExit>()
            .insert_resource(ClientSettingsPath(path))
            .insert_resource(UiScale::default())
            .init_resource::<ClientInputContext>()
            .init_resource::<ClientInputSettings>()
            .init_resource::<InputSettingsSelection>()
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
    }

    pub(crate) fn overlay_root_count(world: &mut World) -> usize {
        let mut query = world.query_filtered::<Entity, With<OverlayRoot>>();
        query.iter(world).count()
    }

    pub(crate) fn has_settings_drafts(world: &World) -> bool {
        world.contains_resource::<InputSettingsDraft>()
            || world.contains_resource::<ShellSettingsDraft>()
    }

    pub(crate) fn returns_to_match_menu(world: &World) -> bool {
        *world.resource::<SettingsReturnTarget>() == SettingsReturnTarget::MatchMenu
    }

    /// Shell buttons of the topmost interactive layer for the current overlay.
    pub(crate) fn active_shell_buttons(world: &mut World) -> usize {
        let layer = active_layer(world.resource::<ClientOverlay>());
        let mut query = world.query::<&ShellButton>();
        query
            .iter(world)
            .filter(|button| button.layer == layer && button.action.is_some())
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FlowError, FlowErrorAction, FlowErrorKind};
    use bevy::input_focus::directional_navigation::NavNeighbor;

    pub(super) fn shell_test_app(path: std::path::PathBuf) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .insert_resource(ButtonInput::<KeyCode>::default())
            .init_state::<ClientFlow>();
        flow_test_support::add_shell(&mut app, path);
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::Dashboard);
        app
    }

    pub(super) fn request_settings(app: &mut App) {
        *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::Settings;
        app.update();
    }

    #[test]
    fn settings_and_controls_fit_a_landscape_phone() {
        let path = std::env::temp_dir().join(format!(
            "brawler-brl0196-shell-phone-{}-settings.ron",
            std::process::id()
        ));
        let mut app = shell_test_app(path);
        crate::ui::layout_harness::add_layout(&mut app, 868, 411);
        app.update();
        request_settings(&mut app);
        for category in [
            None,
            Some(ShellAction::AudioCategory),
            Some(ShellAction::AccessibilityCategory),
            Some(ShellAction::ControlsCategory),
        ] {
            if let Some(category) = category {
                app.world_mut()
                    .resource_mut::<PendingActions>()
                    .0
                    .push(category);
            }
            for _ in 0..3 {
                app.update();
            }
            // The shell applies the viewport fit itself from the primary window height.
            assert!((app.world().resource::<UiScale>().0 - 411.0 / 640.0).abs() < 1e-3);
            let unreachable = crate::ui::layout_harness::unreachable_buttons(app.world_mut());
            assert!(
                unreachable.is_empty(),
                "{category:?}: unreachable controls: {unreachable:?}"
            );
        }

        // A touch drag may move the focused row out of view; the focus reveal must not snap the
        // panel back on later frames.
        let mut areas = app
            .world_mut()
            .query_filtered::<(Entity, &ComputedNode), With<ShellScrollArea>>();
        let area = areas
            .iter(app.world())
            .find(|(_, node)| node.content_size().y > node.size().y + 1.0)
            .map(|(entity, _)| entity)
            .expect("the Controls page overflows the phone viewport");
        let mut buttons = app
            .world_mut()
            .query_filtered::<Entity, With<ShellButton>>();
        let first_row = buttons
            .iter(app.world())
            .find(|button| {
                std::iter::successors(Some(*button), |entity| {
                    app.world().get::<ChildOf>(*entity).map(ChildOf::parent)
                })
                .any(|ancestor| ancestor == area)
            })
            .expect("a row inside the scroll area");
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(first_row, FocusCause::Pressed);
        app.update();
        app.update();
        app.world_mut().get_mut::<ScrollPosition>(area).unwrap().0.y = 400.0;
        app.update();
        app.update();
        let scroll = app.world().get::<ScrollPosition>(area).unwrap().0.y;
        assert!((scroll - 400.0).abs() < f32::EPSILON, "{scroll}");
    }

    /// BRL-q148: with the touch policy on, every Settings row, tab and binding is at least 48 dp
    /// tall on a landscape phone at the default and the smallest UI scale, and stays reachable.
    #[test]
    fn settings_controls_are_48_dp_touch_targets_on_a_landscape_phone() {
        use crate::ui::{layout_harness, touch_target};
        for ui_scale in [1.0, MIN_UI_SCALE] {
            let path = std::env::temp_dir().join(format!(
                "brawler-brlq148-touch-{}-{}-settings.ron",
                (ui_scale * 100.0).round(),
                std::process::id()
            ));
            let mut app = shell_test_app(path);
            layout_harness::add_layout(&mut app, 868, 411);
            app.insert_resource(touch_target::TouchTargetPolicy { enabled: true });
            app.update();
            app.world_mut()
                .resource_mut::<ClientShellSettings>()
                .ui_scale = ui_scale;
            request_settings(&mut app);
            for category in [
                None,
                Some(ShellAction::AudioCategory),
                Some(ShellAction::AccessibilityCategory),
                Some(ShellAction::ControlsCategory),
            ] {
                if let Some(category) = category {
                    app.world_mut()
                        .resource_mut::<PendingActions>()
                        .0
                        .push(category);
                }
                for _ in 0..3 {
                    app.update();
                }
                let scale = app.world().resource::<UiScale>().0;
                assert!((scale - ui_scale * 411.0 / 640.0).abs() < 1e-3, "{scale}");
                let screen = format!("{category:?} at {:.0}%", ui_scale * 100.0);
                let unreachable = layout_harness::unreachable_buttons(app.world_mut());
                assert!(
                    unreachable.is_empty(),
                    "{screen}: unreachable controls: {unreachable:?}"
                );
                let short = layout_harness::short_buttons(
                    app.world_mut(),
                    touch_target::MIN_TOUCH_TARGET_DP,
                );
                assert!(
                    short.is_empty(),
                    "{screen}: controls under 48 dp: {short:?}"
                );
                let overlapping = layout_harness::overlapping_buttons(app.world_mut(), &[]);
                assert!(
                    overlapping.is_empty(),
                    "{screen}: raised controls overlap: {overlapping:?}"
                );
            }
            *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::Credits;
            for _ in 0..4 {
                app.update();
            }
            assert!(layout_harness::unreachable_buttons(app.world_mut()).is_empty());
            let short =
                layout_harness::short_buttons(app.world_mut(), touch_target::MIN_TOUCH_TARGET_DP);
            assert!(short.is_empty(), "Credits: controls under 48 dp: {short:?}");
        }
    }

    /// BRL-wpr8: the longer Credits text keeps BACK reachable on a landscape phone and desktop.
    #[test]
    fn credits_back_is_reachable_on_phone_and_desktop() {
        for (width, height) in [(868, 411), (1280, 720)] {
            let path = std::env::temp_dir().join(format!(
                "brawler-brlwpr8-credits-{width}-{}-settings.ron",
                std::process::id()
            ));
            let mut app = shell_test_app(path);
            crate::ui::layout_harness::add_layout(&mut app, width, height);
            app.update();
            *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::Credits;
            for _ in 0..4 {
                app.update();
            }
            let mut texts = app.world_mut().query::<&Text>();
            assert!(
                texts
                    .iter(app.world())
                    .any(|text| text.0.contains("THIRD-PARTY ASSETS")),
                "Credits text spawned"
            );
            let unreachable = crate::ui::layout_harness::unreachable_buttons(app.world_mut());
            assert!(
                unreachable.is_empty(),
                "{width}x{height}: unreachable controls: {unreachable:?}"
            );
        }
    }

    /// BRL-0201: on the phone layout a finger drag and a tap set a slider; saving waits for the
    /// finger to lift.
    #[test]
    fn phone_touch_drag_and_tap_set_sliders_and_save_on_release() {
        use crate::ui::{controls::SliderTrack, layout_harness};
        use bevy::input::touch::TouchPhase;
        let dir =
            std::env::temp_dir().join(format!("brawler-brl0201-touch-{}", std::process::id()));
        let path = dir.join("settings.ron");
        let mut app = shell_test_app(path.clone());
        layout_harness::add_layout(&mut app, 868, 411);
        app.update();
        request_settings(&mut app);
        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::AudioCategory);
        for _ in 0..4 {
            app.update();
        }
        let track = |app: &mut App, id: ShellControlId| {
            let mut tracks = app.world_mut().query::<(Entity, &SliderTrack)>();
            let entity = tracks
                .iter(app.world())
                .find(|(_, track)| {
                    app.world()
                        .get::<ShellButton>(track.0)
                        .is_some_and(|button| button.id == id)
                })
                .map(|(entity, _)| entity)
                .expect("slider track");
            layout_harness::node_rect(app.world(), entity).expect("laid-out track")
        };
        let along = |rect: Rect, fraction: f32| {
            Vec2::new(rect.min.x + rect.width() * fraction, rect.center().y)
        };
        app.init_resource::<RecordedUiFeedback>()
            .add_systems(Last, record_ui_feedback);
        let master = track(&mut app, ShellControlId::VolumeUp);
        layout_harness::touch(&mut app, TouchPhase::Started, along(master, 0.9));
        layout_harness::touch(&mut app, TouchPhase::Moved, along(master, 0.6));
        layout_harness::touch(&mut app, TouchPhase::Moved, along(master, 0.3));
        assert_eq!(
            app.world().resource::<ClientShellSettings>().master_volume,
            30
        );
        assert!(!path.exists(), "saving waits for the finger to lift");
        // BRL-ek8p: a touch drag steps the slider audibly, like keyboard steps.
        assert!(
            take_ui_feedback(&mut app).contains(&UiFeedback::Stepped),
            "the touch drag publishes slider steps"
        );
        layout_harness::touch(&mut app, TouchPhase::Ended, along(master, 0.3));
        app.update();
        assert!(path.exists());
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("master_volume: 30"), "{saved}");

        let music = track(&mut app, ShellControlId::MusicVolumeUp);
        layout_harness::tap(&mut app, along(music, 0.5));
        assert_eq!(
            app.world().resource::<ClientShellSettings>().music_volume,
            50
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// BRL-0198: the platform Back request closes Settings and Credits like Esc.
    #[test]
    fn platform_back_closes_settings_and_credits() {
        let path = std::env::temp_dir().join(format!(
            "brawler-brl0198-shell-back-{}-settings.ron",
            std::process::id()
        ));
        let mut app = shell_test_app(path);
        app.update();
        for overlay in [ClientOverlay::Settings, ClientOverlay::Credits] {
            *app.world_mut().resource_mut::<ClientOverlay>() = overlay.clone();
            app.update();
            app.update();
            app.world_mut().resource_mut::<crate::BackInput>().request();
            app.update();
            app.update();
            assert_eq!(
                *app.world().resource::<ClientOverlay>(),
                ClientOverlay::None,
                "{overlay:?} stayed open"
            );
        }
    }

    #[test]
    fn dashboard_overlay_request_builds_settings_navigation() {
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
        request_settings(&mut app);
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
    fn preference_toggle_autosaves_without_closing_settings() {
        let dir =
            std::env::temp_dir().join(format!("brawler-shell-autosave-{}", std::process::id()));
        let path = dir.join("settings.ron");
        let mut app = shell_test_app(path.clone());
        app.update();
        request_settings(&mut app);
        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::ToggleReducedMotion);
        app.update();
        assert!(app.world().resource::<ClientShellSettings>().reduced_motion);
        assert_eq!(
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::Settings
        );
        assert!(load_settings(&path).unwrap().unwrap().1.reduced_motion);
        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::Back);
        app.update();
        assert!(app.world().resource::<ClientShellSettings>().reduced_motion);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn preference_save_failure_is_inline_and_retry_preserves_the_page() {
        let dir =
            std::env::temp_dir().join(format!("brawler-shell-inline-save-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let blocker = dir.join("blocker");
        std::fs::write(&blocker, "blocked").unwrap();
        let path = blocker.join("settings.ron");
        let mut app = shell_test_app(path.clone());
        app.update();
        request_settings(&mut app);
        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::ToggleReducedMotion);
        app.update();
        app.update();
        assert_eq!(
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::Settings
        );
        assert!(app.world().resource::<ClientShellSettings>().reduced_motion);
        let mut texts = app.world_mut().query::<&Text>();
        assert!(
            texts
                .iter(app.world())
                .any(|text| text.0.contains("could not be saved"))
        );
        std::fs::remove_file(&blocker).unwrap();
        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::RetryPreferences);
        app.update();
        assert!(load_settings(&path).unwrap().unwrap().1.reduced_motion);
        assert_eq!(
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::Settings
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn gamepad_adjusts_slider_without_moving_focus() {
        let dir = std::env::temp_dir().join(format!("brawler-shell-slider-{}", std::process::id()));
        let mut app = shell_test_app(dir.join("settings.ron"));
        app.update();
        request_settings(&mut app);
        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::AccessibilityCategory);
        app.update();
        let mut buttons = app.world_mut().query::<(Entity, &ShellButton)>();
        let slider = buttons
            .iter(app.world())
            .find(|(_, button)| button.id == ShellControlId::UiScaleUp)
            .unwrap()
            .0;
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(slider, FocusCause::Navigated);
        let mut gamepad = Gamepad::default();
        gamepad.digital_mut().press(GamepadButton::DPadRight);
        app.world_mut().spawn(gamepad);
        app.update();
        let focused = app.world().resource::<InputFocus>().get().unwrap();
        assert_eq!(
            app.world().get::<ShellButton>(focused).unwrap().id,
            ShellControlId::UiScaleUp
        );
        assert!((app.world().resource::<ClientShellSettings>().ui_scale - 1.1).abs() < 0.001);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[derive(Resource, Default)]
    struct RecordedUiFeedback(Vec<UiFeedback>);

    fn record_ui_feedback(
        mut feedback: MessageReader<UiFeedback>,
        mut recorded: ResMut<RecordedUiFeedback>,
    ) {
        recorded.0.extend(feedback.read().copied());
    }

    fn take_ui_feedback(app: &mut App) -> Vec<UiFeedback> {
        std::mem::take(&mut app.world_mut().resource_mut::<RecordedUiFeedback>().0)
    }

    fn tap_key(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keyboard.release(key);
        keyboard.clear();
        app.update();
    }

    /// BRL-ek8p: keyboard slider steps, focus moves, activation and Back each publish one
    /// presentation fact; a step that cannot change the value and an edge move stay silent.
    #[test]
    fn settings_keyboard_input_publishes_step_move_activation_and_back_feedback() {
        let dir =
            std::env::temp_dir().join(format!("brawler-brlek8p-shell-{}", std::process::id()));
        let mut app = shell_test_app(dir.join("settings.ron"));
        app.init_resource::<RecordedUiFeedback>()
            .add_systems(Last, record_ui_feedback);
        app.update();
        request_settings(&mut app);
        app.world_mut()
            .resource_mut::<PendingActions>()
            .0
            .push(ShellAction::AudioCategory);
        app.update();
        app.update();
        let mut buttons = app.world_mut().query::<(Entity, &ShellButton)>();
        let master = buttons
            .iter(app.world())
            .find(|(_, button)| button.id == ShellControlId::VolumeUp)
            .unwrap()
            .0;
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(master, FocusCause::Navigated);
        app.update();
        take_ui_feedback(&mut app);

        assert_eq!(
            app.world().resource::<ClientShellSettings>().master_volume,
            100
        );
        tap_key(&mut app, KeyCode::ArrowRight);
        assert_eq!(take_ui_feedback(&mut app), Vec::new(), "already at maximum");
        tap_key(&mut app, KeyCode::ArrowLeft);
        assert_eq!(
            app.world().resource::<ClientShellSettings>().master_volume,
            90
        );
        assert_eq!(take_ui_feedback(&mut app), vec![UiFeedback::Stepped]);

        tap_key(&mut app, KeyCode::ArrowDown);
        assert_ne!(app.world().resource::<InputFocus>().get(), Some(master));
        assert_eq!(take_ui_feedback(&mut app), vec![UiFeedback::Moved]);

        // Sliders adjust instead of activating; a toggle row activates.
        let mut buttons = app.world_mut().query::<(Entity, &ShellButton)>();
        let focused = buttons
            .iter(app.world())
            .find(|(_, button)| button.id == ShellControlId::ToggleFocusMute)
            .unwrap()
            .0;
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(focused, FocusCause::Navigated);
        app.update();
        take_ui_feedback(&mut app);
        let surface = app
            .world()
            .get::<crate::ui::surface::Surface>(focused)
            .copied();
        tap_key(&mut app, KeyCode::Enter);
        assert_eq!(
            take_ui_feedback(&mut app),
            vec![UiFeedback::Activated(surface)]
        );

        tap_key(&mut app, KeyCode::Escape);
        assert_eq!(
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::None
        );
        assert_eq!(take_ui_feedback(&mut app), vec![UiFeedback::Back]);
        let _ = std::fs::remove_dir_all(dir);
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
            (apply, ShellControlId::ToggleVsync, Vec2::new(0.0, 100.0)),
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
    fn active_layer_traps_focus_to_the_visible_overlay() {
        assert_eq!(active_layer(&ClientOverlay::None), ShellLayer::FlowOwned);
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
