//! Windowed arena, fighter, camera, HUD, and pause presentation.
#![allow(clippy::wildcard_imports)]

use super::*;

#[derive(Resource, Default)]
pub(super) struct MatchMenuState {
    selected: usize,
    scoreboard_latched: bool,
    was_open: bool,
}

pub(super) fn spawn_pause_overlay(mut commands: Commands) {
    commands
        .spawn((
            PauseOverlay,
            Node {
                position_type: PositionType::Absolute,
                left: percent(25.0),
                right: percent(25.0),
                top: percent(40.0),
                bottom: percent(40.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.88)),
            GlobalZIndex(300),
            Visibility::Hidden,
        ))
        .with_children(|parent| {
            parent.spawn((
                MatchMenuText,
                Text::new(
                    "MATCH MENU — MATCH CONTINUES\n\nRESUME\nSETTINGS\nSCOREBOARD\nLEAVE MATCH",
                ),
                TextFont::from_font_size(28.0),
                TextColor(Color::WHITE),
            ));
        });
}

pub(super) fn spawn_client_hud(mut commands: Commands) {
    commands.spawn((
        CombatHudText,
        Text::new("HEALTH  --/--"),
        TextFont::from_font_size(19.0),
        TextColor(Color::WHITE),
        TextLayout::linebreak(LineBreak::WordBoundary),
        GlobalZIndex(100),
        Node {
            position_type: PositionType::Absolute,
            left: px(16.0),
            bottom: px(16.0),
            max_width: px(330.0),
            padding: UiRect::all(px(10.0)),
            border_radius: BorderRadius::all(px(7.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.015, 0.025, 0.04, 0.92)),
    ));
    commands.spawn((
        CombatAbilityHudText,
        Text::new("WEAPON  --/--\nULT --"),
        TextFont::from_font_size(18.0),
        TextColor(Color::WHITE),
        TextLayout::new(Justify::Right, LineBreak::WordBoundary),
        GlobalZIndex(100),
        Node {
            position_type: PositionType::Absolute,
            right: px(16.0),
            bottom: px(16.0),
            max_width: px(360.0),
            padding: UiRect::all(px(10.0)),
            border_radius: BorderRadius::all(px(7.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.015, 0.025, 0.04, 0.92)),
    ));
    commands.spawn((
        ScoreboardOverlay,
        Text::new("SCOREBOARD"),
        TextFont::from_font_size(20.0),
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

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn handle_match_menu_input(
    mut context: ResMut<ClientInputContext>,
    pending: Res<PendingLocalActions>,
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut menu: ResMut<MatchMenuState>,
    overlay: Option<ResMut<ClientOverlay>>,
    flow: Option<Res<State<ClientFlow>>>,
    settings_request: Option<ResMut<MatchSettingsRequest>>,
    mut texts: Query<&mut Text, With<MatchMenuText>>,
) {
    if menu.scoreboard_latched && pending.cancel_pressed {
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
    let activate = keyboard.any_just_pressed([KeyCode::Enter, KeyCode::Space])
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
    let rows = ["RESUME", "SETTINGS", "SCOREBOARD", "LEAVE MATCH"];
    let copy = rows
        .iter()
        .enumerate()
        .map(|(index, label)| {
            if index == menu.selected {
                format!("> {label}")
            } else {
                format!("  {label}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    for mut text in &mut texts {
        text.0 = format!("MATCH MENU — MATCH CONTINUES\n\n{copy}");
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
        app.init_resource::<ClientPresentation>()
            .init_resource::<MatchMenuState>()
            .add_systems(
                Update,
                (
                    exit_on_close_requested,
                    handle_match_menu_input,
                    update_pause_overlay,
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

    #[test]
    fn menu_scoreboard_is_latched_and_cancel_returns_to_the_continuing_menu() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(ClientInputContext::Menu)
            .init_resource::<PendingLocalActions>()
            .init_resource::<MatchMenuState>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, handle_match_menu_input);
        app.world_mut().spawn((MatchMenuText, Text::new("")));

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
    }
}
