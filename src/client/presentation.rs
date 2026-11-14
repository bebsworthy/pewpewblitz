//! Windowed arena, fighter, camera, HUD, and pause presentation.
#![allow(clippy::wildcard_imports)]

use super::*;

#[derive(Resource, Default)]
pub(super) struct MatchMenuState {
    selected: usize,
    scoreboard_latched: bool,
    was_open: bool,
}

mod match_menu;
use match_menu::{MatchMenuButton, update_match_menu_chrome};

/// Spawns the on-demand scoreboard panel; the persistent HUD belongs to `hud`.
pub(super) fn spawn_client_hud(mut commands: Commands, font: &Handle<Font>) {
    commands.spawn((
        ScoreboardOverlay,
        Text::new("SCOREBOARD"),
        ui::theme::hud_text(font, 20.0),
        TextColor(Color::WHITE),
        TextLayout::new(Justify::Left, LineBreak::WordBoundary),
        BackgroundColor(Color::srgba(0.015, 0.025, 0.04, 0.96)),
        GlobalZIndex(250),
        Node {
            position_type: PositionType::Absolute,
            left: percent(22.0),
            right: percent(22.0),
            top: percent(20.0),
            padding: UiRect::all(px(18.0)),
            border_radius: BorderRadius::all(px(9.0)),
            ..default()
        },
        Visibility::Hidden,
    ));
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
#[cfg(test)]
pub(super) fn clamp_camera_center(
    position: Vec2,
    bounds: crate::map::AxisAlignedMapRect,
    viewport: Vec2,
) -> Vec2 {
    let aspect = if viewport.y > 0.0 {
        viewport.x / viewport.y
    } else {
        16.0 / 9.0
    };
    let half_height = crate::movement::CAMERA_VERTICAL_SPAN / 2.0;
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
    )>,
) {
    // Touch has no cancel key: a tap or click anywhere also leaves the latched scoreboard.
    if menu.scoreboard_latched && (pending.cancel_pressed || pointer.released()) {
        menu.scoreboard_latched = false;
        *context = ClientInputContext::Menu;
    }
    let open = matches!(*context, ClientInputContext::Menu);
    if open && !menu.was_open {
        menu.selected = 0;
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
    if keyboard.any_just_pressed([KeyCode::ArrowDown, KeyCode::KeyS])
        || pad_pressed(GamepadButton::DPadDown)
    {
        menu.selected = (menu.selected + 1).min(3);
    }
    if keyboard.any_just_pressed([KeyCode::ArrowUp, KeyCode::KeyW])
        || pad_pressed(GamepadButton::DPadUp)
    {
        menu.selected = menu.selected.saturating_sub(1);
    }
    let mut clicked = false;
    for (button, interaction, release) in &buttons {
        if interaction.is_changed()
            && matches!(*interaction, Interaction::Hovered | Interaction::Pressed)
        {
            menu.selected = button.0;
        }
        if release.activated() {
            menu.selected = button.0;
            clicked = true;
        }
    }
    let activate = clicked
        || keyboard.any_just_pressed([KeyCode::Enter, KeyCode::Space])
        || pad_pressed(GamepadButton::South);
    if activate {
        match menu.selected {
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
) {
    for mut visibility in &mut scoreboard {
        *visibility = if !matches!(*context, ClientInputContext::Shell)
            && (pending.scoreboard_held
                || menu.as_deref().is_some_and(|menu| menu.scoreboard_latched))
        {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Adds client-only window behavior and startup diagnostics.
pub struct ClientPresentationPlugin;

impl Plugin for ClientPresentationPlugin {
    fn build(&self, app: &mut App) {
        ui::press::add_release_activation(app);
        app.init_resource::<ClientPresentation>()
            .init_resource::<MatchMenuState>()
            .add_systems(Startup, match_menu::spawn_pause_overlay)
            .add_systems(
                Update,
                (
                    exit_on_close_requested,
                    (
                        handle_match_menu_input,
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
    use bevy::input::touch::{TouchPhase, touch_screen_input_system};

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
}
