//! Windowed arena, fighter, camera, HUD, and pause presentation.

use super::{
    ClientFlow, ClientInputContext, ClientOverlay, ClientPresentation, MatchSettingsRequest,
    PauseOverlay, PendingLocalActions, ScoreboardOverlay, WindowCloseRequested, assets, audio, hud,
    presentation_3d, ui,
};
use bevy::prelude::*;

#[derive(Resource, Default)]
pub(super) struct MatchMenuState {
    focused: usize,
    scoreboard_latched: bool,
    was_open: bool,
}

mod match_menu;
use match_menu::{MatchMenuButton, update_match_menu_chrome};

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
#[cfg(test)]
pub(super) fn clamp_camera_center(
    position: Vec2,
    bounds: brawler::map::AxisAlignedMapRect,
    viewport: Vec2,
) -> Vec2 {
    let aspect = if viewport.y > 0.0 {
        viewport.x / viewport.y
    } else {
        16.0 / 9.0
    };
    let half_height = brawler::movement::CAMERA_VERTICAL_SPAN / 2.0;
    let half_width = half_height * aspect.max(0.0);
    let min = bounds.min + Vec2::new(half_width, half_height);
    let max = bounds.max - Vec2::new(half_width, half_height);
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

/// A finished tap or click, which dismisses the latched scoreboard.
#[derive(bevy::ecs::system::SystemParam)]
struct LatchedScoreboardDismiss<'w> {
    touches: Option<Res<'w, Touches>>,
    mouse: Option<Res<'w, ButtonInput<MouseButton>>>,
}

impl LatchedScoreboardDismiss<'_> {
    fn released(&self) -> bool {
        self.touches
            .as_ref()
            .is_some_and(|touches| touches.any_just_released())
            || self
                .mouse
                .as_ref()
                .is_some_and(|mouse| mouse.just_released(MouseButton::Left))
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn handle_match_menu_input(
    mut context: ResMut<ClientInputContext>,
    pending: Res<PendingLocalActions>,
    keyboard: Res<ButtonInput<KeyCode>>,
    pointer: LatchedScoreboardDismiss,
    gamepads: Query<&Gamepad>,
    mut menu: ResMut<MatchMenuState>,
    overlay: Option<ResMut<ClientOverlay>>,
    flow: Option<Res<State<ClientFlow>>>,
    settings_request: Option<ResMut<MatchSettingsRequest>>,
    buttons: Query<(
        &MatchMenuButton,
        Ref<Interaction>,
        &ui::press::ReleaseActivation,
        Option<&ui::surface::Surface>,
    )>,
    close_buttons: Query<&ui::press::ReleaseActivation, With<hud::ScoreboardCloseButton>>,
    mut feedback: MessageWriter<ui::feedback::UiFeedback>,
) {
    // Back, the visible close button, or (touch has no cancel key) a tap or click anywhere
    // leaves the latched scoreboard.
    let closed = close_buttons
        .iter()
        .any(ui::press::ReleaseActivation::activated);
    if menu.scoreboard_latched && (pending.cancel_pressed || closed || pointer.released()) {
        menu.scoreboard_latched = false;
        *context = ClientInputContext::Menu;
        // The pointer close button sounds through the press model; Esc, controller cancel and
        // platform Back back out audibly too (BRL-yrys).
        if pending.cancel_pressed && !closed {
            feedback.write(ui::feedback::UiFeedback::Back);
        }
    }
    let open = matches!(*context, ClientInputContext::Menu);
    if open && !menu.was_open {
        menu.focused = 0;
    }
    menu.was_open = open;
    if !open
        || menu.scoreboard_latched
        || overlay
            .as_ref()
            .is_some_and(|overlay| !matches!(overlay.as_ref(), ClientOverlay::None))
    {
        return;
    }
    let pad_pressed = |button| gamepads.iter().any(|pad| pad.just_pressed(button));
    let focus_before = menu.focused;
    if keyboard.any_just_pressed([KeyCode::ArrowDown, KeyCode::KeyS])
        || pad_pressed(GamepadButton::DPadDown)
    {
        menu.focused = (menu.focused + 1).min(3);
    }
    if keyboard.any_just_pressed([KeyCode::ArrowUp, KeyCode::KeyW])
        || pad_pressed(GamepadButton::DPadUp)
    {
        menu.focused = menu.focused.saturating_sub(1);
    }
    if menu.focused != focus_before {
        feedback.write(ui::feedback::UiFeedback::Moved);
    }
    let mut clicked = false;
    for (button, interaction, release, _) in &buttons {
        if interaction.is_changed()
            && matches!(*interaction, Interaction::Hovered | Interaction::Pressed)
        {
            menu.focused = button.0;
        }
        if release.activated() {
            menu.focused = button.0;
            clicked = true;
        }
    }
    let keyed = !clicked
        && (keyboard.any_just_pressed([KeyCode::Enter, KeyCode::Space])
            || pad_pressed(GamepadButton::South));
    if keyed {
        // Pointer releases already sound through the shared press model.
        let surface = buttons
            .iter()
            .find(|(button, ..)| button.0 == menu.focused)
            .and_then(|(.., surface)| surface.copied());
        feedback.write(ui::feedback::UiFeedback::Activated(surface));
    }
    if clicked || keyed {
        match menu.focused {
            0 => *context = ClientInputContext::Gameplay,
            1 => {
                if let Some(mut request) = settings_request {
                    request.0 = true;
                }
            }
            2 => menu.scoreboard_latched = true,
            3 => {
                if flow.is_some_and(|flow| *flow.get() == ClientFlow::Match)
                    && let Some(mut overlay) = overlay
                {
                    *overlay = ClientOverlay::LeaveConfirmation;
                }
            }
            _ => {}
        }
    }
}

/// Sounds the match menu opening and closing (BRL-yrys): Gameplay to Menu clicks, Menu to
/// Gameplay backs out, whichever pause request (keyboard pause binding, controller pause, platform
/// Back, HUD menu button) toggled it. Shell hand-offs such as Settings or the match ending are not
/// Gameplay/Menu transitions and stay silent. A menu row or button that resumes already sounded
/// itself; the UI audio producer's press spacing drops this same-gesture duplicate.
#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn publish_match_menu_toggle_feedback(
    context: Res<ClientInputContext>,
    mut previous: Local<Option<ClientInputContext>>,
    mut feedback: MessageWriter<ui::feedback::UiFeedback>,
) {
    let current = *context;
    match (previous.replace(current), current) {
        (Some(ClientInputContext::Gameplay), ClientInputContext::Menu) => {
            feedback.write(ui::feedback::UiFeedback::Activated(None));
        }
        (Some(ClientInputContext::Menu), ClientInputContext::Gameplay) => {
            feedback.write(ui::feedback::UiFeedback::Back);
        }
        _ => {}
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn update_pause_overlay(
    context: Res<ClientInputContext>,
    menu: Res<MatchMenuState>,
    overlay: Option<Res<ClientOverlay>>,
    mut overlays: Query<&mut Visibility, With<PauseOverlay>>,
) {
    let product_overlay_open =
        overlay.is_some_and(|overlay| !matches!(*overlay, ClientOverlay::None));
    for mut visibility in &mut overlays {
        *visibility = if matches!(*context, ClientInputContext::Menu)
            && !menu.scoreboard_latched
            && !product_overlay_open
        {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn update_client_hud(
    pending: Res<PendingLocalActions>,
    context: Res<ClientInputContext>,
    menu: Option<Res<MatchMenuState>>,
    mut scoreboard: Query<&mut Visibility, With<ScoreboardOverlay>>,
    mut close_buttons: Query<
        &mut Visibility,
        (With<hud::ScoreboardCloseButton>, Without<ScoreboardOverlay>),
    >,
) {
    let in_match = !matches!(*context, ClientInputContext::Shell);
    let latched = in_match && menu.as_deref().is_some_and(|menu| menu.scoreboard_latched);
    let shown = |visible: bool| {
        if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        }
    };
    for mut visibility in &mut scoreboard {
        visibility.set_if_neq(shown(latched || (in_match && pending.scoreboard_held)));
    }
    // A held scoreboard closes on release; only the latched one needs a visible close control.
    for mut visibility in &mut close_buttons {
        visibility.set_if_neq(shown(latched));
    }
}

/// Adds client-only window behavior and startup diagnostics.
pub struct ClientPresentationPlugin;

impl Plugin for ClientPresentationPlugin {
    fn build(&self, app: &mut App) {
        ui::press::add_release_activation(app);
        ui::touch_target::add_touch_targets(app);
        ui::feedback::add_ui_feedback(app);
        ui::fonts::add_fallback_fonts(app);
        app.init_resource::<ClientPresentation>()
            .init_resource::<MatchMenuState>()
            .add_systems(Startup, match_menu::spawn_pause_overlay)
            .add_systems(OnExit(ClientFlow::Match), close_match_menu)
            .add_systems(
                Update,
                (
                    exit_on_close_requested,
                    (
                        handle_match_menu_input,
                        publish_match_menu_toggle_feedback,
                        update_match_menu_chrome,
                        update_pause_overlay,
                    )
                        .chain(),
                    update_client_hud,
                ),
            )
            .add_plugins((
                assets::ClientAssetPlugin,
                audio::ClientAudioPlugin,
                hud::ClientReadinessHudPlugin,
            ));
        app.add_plugins(presentation_3d::WorldPresentationPlugin);
    }
}

/// The match menu and its latched scoreboard end with the match (BRL-nqnm); the flow's own
/// exit returns input to the shell, which hides the pause overlay.
fn close_match_menu(mut menu: ResMut<MatchMenuState>) {
    *menu = MatchMenuState::default();
}

fn exit_on_close_requested(
    mut close_requests: MessageReader<WindowCloseRequested>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if close_requests.read().next().is_some() {
        app_exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ClientPlayableGate, InputDeviceActivity, PrimaryWindow, apply_pause_request,
        sample_local_input, touch,
    };
    use bevy::input::touch::{TouchPhase, touch_screen_input_system};

    #[derive(Resource, Default)]
    struct RecordedUiFeedback(Vec<ui::feedback::UiFeedback>);

    fn record_ui_feedback(
        mut feedback: MessageReader<ui::feedback::UiFeedback>,
        mut recorded: ResMut<RecordedUiFeedback>,
    ) {
        recorded.0.extend(feedback.read().copied());
    }

    fn take_ui_feedback(app: &mut App) -> Vec<ui::feedback::UiFeedback> {
        std::mem::take(&mut app.world_mut().resource_mut::<RecordedUiFeedback>().0)
    }

    /// One frame's local pause request, applied the way the keyboard, controller, platform Back
    /// and HUD-button samplers apply it before `Update`.
    #[derive(Resource, Default)]
    struct PauseRequest(bool);

    fn apply_test_pause_request(
        mut request: ResMut<PauseRequest>,
        mut context: ResMut<ClientInputContext>,
        mut pending: ResMut<PendingLocalActions>,
    ) {
        let pressed = std::mem::take(&mut request.0);
        pending.cancel_pressed = pressed;
        apply_pause_request(&mut context, &mut pending, pressed);
    }

    #[test]
    fn pause_requests_sound_the_match_menu_opening_and_closing_once() {
        use ui::feedback::UiFeedback;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(ClientInputContext::Gameplay)
            .init_resource::<PendingLocalActions>()
            .init_resource::<MatchMenuState>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<PauseRequest>()
            .init_resource::<RecordedUiFeedback>()
            .add_plugins(ui::press::ReleaseActivationPlugin)
            .add_message::<UiFeedback>()
            .add_systems(PreUpdate, apply_test_pause_request)
            .add_systems(
                Update,
                (handle_match_menu_input, publish_match_menu_toggle_feedback).chain(),
            )
            .add_systems(Last, record_ui_feedback);
        let pause = |app: &mut App| {
            app.world_mut().resource_mut::<PauseRequest>().0 = true;
            app.update();
            take_ui_feedback(app)
        };
        app.update();
        assert_eq!(take_ui_feedback(&mut app), Vec::new(), "no sound at start");

        assert_eq!(pause(&mut app), vec![UiFeedback::Activated(None)]);
        assert_eq!(
            *app.world().resource::<ClientInputContext>(),
            ClientInputContext::Menu
        );
        app.update();
        assert_eq!(
            take_ui_feedback(&mut app),
            Vec::new(),
            "an open menu is silent"
        );
        assert_eq!(pause(&mut app), vec![UiFeedback::Back]);
        assert_eq!(
            *app.world().resource::<ClientInputContext>(),
            ClientInputContext::Gameplay
        );

        // Settings opened from the menu and its return are shell hand-offs, not pause toggles.
        assert_eq!(pause(&mut app), vec![UiFeedback::Activated(None)]);
        *app.world_mut().resource_mut::<ClientInputContext>() = ClientInputContext::Shell;
        app.update();
        assert_eq!(
            take_ui_feedback(&mut app),
            Vec::new(),
            "Menu to Shell is silent"
        );
        *app.world_mut().resource_mut::<ClientInputContext>() = ClientInputContext::Menu;
        app.update();
        assert_eq!(
            take_ui_feedback(&mut app),
            Vec::new(),
            "Shell to Menu is silent"
        );
        assert_eq!(pause(&mut app), vec![UiFeedback::Back]);

        // The pause binding closing the latched scoreboard returns to the open menu: one Back,
        // and the menu itself stays open.
        assert_eq!(pause(&mut app), vec![UiFeedback::Activated(None)]);
        app.world_mut()
            .resource_mut::<MatchMenuState>()
            .scoreboard_latched = true;
        assert_eq!(pause(&mut app), vec![UiFeedback::Back]);
        assert!(!app.world().resource::<MatchMenuState>().scoreboard_latched);
        assert_eq!(
            *app.world().resource::<ClientInputContext>(),
            ClientInputContext::Menu
        );
    }

    #[test]
    fn pointer_menu_actions_are_isolated_from_product_overlays() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(ClientInputContext::Menu)
            .insert_resource(ClientOverlay::LeaveConfirmation)
            .init_resource::<PendingLocalActions>()
            .init_resource::<MatchMenuState>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_plugins(ui::press::ReleaseActivationPlugin)
            .add_message::<ui::feedback::UiFeedback>()
            .add_systems(Update, handle_match_menu_input);
        let button = app
            .world_mut()
            .spawn((MatchMenuButton(0), Interaction::Pressed))
            .id();
        app.update();
        *app.world_mut().get_mut::<Interaction>(button).unwrap() = Interaction::Hovered;
        app.update();
        assert_eq!(
            *app.world().resource::<ClientInputContext>(),
            ClientInputContext::Menu
        );
        *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::None;
        *app.world_mut().get_mut::<Interaction>(button).unwrap() = Interaction::Hovered;
        app.update();
        assert_eq!(
            *app.world().resource::<ClientInputContext>(),
            ClientInputContext::Menu
        );
        *app.world_mut().get_mut::<Interaction>(button).unwrap() = Interaction::Pressed;
        app.update();
        assert_eq!(
            *app.world().resource::<ClientInputContext>(),
            ClientInputContext::Menu,
            "a held press only focuses; the action waits for release"
        );
        *app.world_mut().get_mut::<Interaction>(button).unwrap() = Interaction::Hovered;
        app.update();
        assert_eq!(
            *app.world().resource::<ClientInputContext>(),
            ClientInputContext::Gameplay
        );
    }

    #[test]
    fn menu_scoreboard_is_latched_and_cancel_returns_to_the_continuing_menu() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(ClientInputContext::Menu)
            .init_resource::<PendingLocalActions>()
            .init_resource::<MatchMenuState>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<ui::feedback::UiFeedback>()
            .add_systems(Update, handle_match_menu_input);

        for _ in 0..2 {
            let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keyboard.reset_all();
            keyboard.press(KeyCode::ArrowDown);
            app.update();
        }
        let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keyboard.reset_all();
        keyboard.press(KeyCode::Space);
        app.update();
        assert!(app.world().resource::<MatchMenuState>().scoreboard_latched);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        *app.world_mut().resource_mut::<ClientInputContext>() = ClientInputContext::Gameplay;
        app.world_mut()
            .resource_mut::<PendingLocalActions>()
            .cancel_pressed = true;
        app.update();
        assert!(!app.world().resource::<MatchMenuState>().scoreboard_latched);
        assert_eq!(
            *app.world().resource::<ClientInputContext>(),
            ClientInputContext::Menu
        );

        // Touch players have no cancel key: a finished tap leaves the latched scoreboard.
        app.world_mut()
            .resource_mut::<PendingLocalActions>()
            .cancel_pressed = false;
        app.world_mut()
            .resource_mut::<MatchMenuState>()
            .scoreboard_latched = true;
        app.init_resource::<Touches>()
            .add_message::<TouchInput>()
            .add_systems(PreUpdate, touch_screen_input_system);
        app.update();
        assert!(app.world().resource::<MatchMenuState>().scoreboard_latched);
        let window = app.world_mut().spawn_empty().id();
        for phase in [TouchPhase::Started, TouchPhase::Ended] {
            app.world_mut().write_message(TouchInput {
                phase,
                position: Vec2::new(10.0, 10.0),
                window,
                force: None,
                id: 1,
            });
        }
        app.update();
        assert!(!app.world().resource::<MatchMenuState>().scoreboard_latched);
    }

    #[test]
    fn the_latched_scoreboard_close_button_returns_to_the_menu_and_shows_only_while_latched() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(ClientInputContext::Gameplay)
            .init_resource::<PendingLocalActions>()
            .init_resource::<MatchMenuState>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_plugins(ui::press::ReleaseActivationPlugin)
            .add_message::<ui::feedback::UiFeedback>()
            .add_systems(Update, (handle_match_menu_input, update_client_hud).chain());
        app.world_mut()
            .resource_mut::<MatchMenuState>()
            .scoreboard_latched = true;
        let panel = app
            .world_mut()
            .spawn((ScoreboardOverlay, Visibility::Hidden))
            .id();
        let close = app
            .world_mut()
            .spawn((
                hud::ScoreboardCloseButton,
                Interaction::None,
                Visibility::Hidden,
                ChildOf(panel),
            ))
            .id();
        app.update();
        let visibility = |app: &App, entity| *app.world().get::<Visibility>(entity).unwrap();
        assert_eq!(visibility(&app, panel), Visibility::Inherited);
        assert_eq!(visibility(&app, close), Visibility::Inherited);

        *app.world_mut().get_mut::<Interaction>(close).unwrap() = Interaction::Pressed;
        app.update();
        assert!(
            app.world().resource::<MatchMenuState>().scoreboard_latched,
            "a held press waits for release"
        );
        *app.world_mut().get_mut::<Interaction>(close).unwrap() = Interaction::Hovered;
        app.update();
        assert!(!app.world().resource::<MatchMenuState>().scoreboard_latched);
        assert_eq!(
            *app.world().resource::<ClientInputContext>(),
            ClientInputContext::Menu
        );
        assert_eq!(visibility(&app, panel), Visibility::Hidden);
        assert_eq!(visibility(&app, close), Visibility::Hidden);

        // Holding the scoreboard key shows the panel without the latched close control.
        *app.world_mut().resource_mut::<ClientInputContext>() = ClientInputContext::Gameplay;
        app.world_mut()
            .resource_mut::<PendingLocalActions>()
            .scoreboard_held = true;
        app.update();
        assert_eq!(visibility(&app, panel), Visibility::Inherited);
        assert_eq!(visibility(&app, close), Visibility::Hidden);
    }

    /// The production chain around one HUD menu-button tap whose down and up land in the same
    /// frame (`adb shell input tap`, BRL-5hhp): real HUD and pause overlay layout, `bevy_ui` focus,
    /// the shared release tracker and its unseen-tap replay (BRL-wr0s), touch sampling in
    /// `BeforeFixedMainLoop`, and the match menu systems.
    #[test]
    fn a_same_frame_tap_on_the_hud_menu_button_opens_the_menu_and_keeps_it_open() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::time::TimeUpdateStrategy;
        use core::time::Duration;

        #[derive(Resource, Default)]
        struct FixedTicks(u32);

        let tick = brawler::timing::SIMULATION_TICK;
        // No fixed step on a fast frame, several on a slow one.
        for (step, slow) in [(Duration::ZERO, false), (tick * 3, true)] {
            let mut app =
                hud::screen_tests::hud_app(914, 411, brawler::map::WIPEOUT_MODE_DEFINITION);
            app.insert_resource(TimeUpdateStrategy::ManualDuration(step))
                .insert_resource(ClientPlayableGate(true))
                .init_resource::<MatchMenuState>()
                .init_resource::<InputDeviceActivity>()
                .init_resource::<FixedTicks>()
                .add_message::<ui::feedback::UiFeedback>()
                .add_plugins(ui::press::ReleaseActivationPlugin)
                .add_systems(
                    RunFixedMainLoop,
                    (sample_local_input, touch::apply_touch_controls)
                        .chain()
                        .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
                )
                .add_systems(FixedUpdate, |mut ticks: ResMut<FixedTicks>| ticks.0 += 1)
                .add_systems(
                    Update,
                    (
                        handle_match_menu_input,
                        update_match_menu_chrome,
                        update_pause_overlay,
                    )
                        .chain(),
                );
            app.world_mut()
                .run_system_once(match_menu::spawn_pause_overlay)
                .unwrap();
            for _ in 0..3 {
                app.update();
            }
            let context = |app: &App| *app.world().resource::<ClientInputContext>();
            assert_eq!(context(&app), ClientInputContext::Gameplay);
            let center = hud::screen_tests::menu_button_rect(&mut app).center();
            let window = app
                .world_mut()
                .query_filtered::<Entity, With<PrimaryWindow>>()
                .single(app.world())
                .unwrap();
            for phase in [TouchPhase::Started, TouchPhase::Ended] {
                app.world_mut().write_message(TouchInput {
                    phase,
                    position: center,
                    window,
                    force: None,
                    id: 1,
                });
            }
            let before = app.world().resource::<FixedTicks>().0;
            app.update();
            let ticks = app.world().resource::<FixedTicks>().0 - before;
            // The warm-up frames ran on real time, so the fixed accumulator may hold a partial tick.
            if slow {
                assert!(
                    ticks >= 2,
                    "a slow frame runs several fixed steps, got {ticks}"
                );
            } else {
                assert_eq!(ticks, 0, "a fast frame runs no fixed step");
            }
            assert_eq!(context(&app), ClientInputContext::Menu, "step {step:?}");
            for _ in 0..5 {
                app.update();
                assert_eq!(
                    context(&app),
                    ClientInputContext::Menu,
                    "the tap never closes the menu again (step {step:?})"
                );
            }
            let overlay_visible = app
                .world_mut()
                .query_filtered::<&InheritedVisibility, With<PauseOverlay>>()
                .single(app.world())
                .unwrap()
                .get();
            assert!(overlay_visible, "the pause overlay shows");
            assert_eq!(app.world().resource::<MatchMenuState>().focused, 0);

            // A second same-frame tap on the same spot, now under the open overlay, does not
            // reach a menu row or the hidden HUD button: the menu stays open.
            for phase in [TouchPhase::Started, TouchPhase::Ended] {
                app.world_mut().write_message(TouchInput {
                    phase,
                    position: center,
                    window,
                    force: None,
                    id: 2,
                });
            }
            for _ in 0..3 {
                app.update();
            }
            assert_eq!(context(&app), ClientInputContext::Menu);
        }
    }
}
