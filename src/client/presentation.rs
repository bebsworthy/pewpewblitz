//! Windowed arena, fighter, camera, HUD, and pause presentation.
#![allow(clippy::wildcard_imports)]

use super::*;

/// Client-only greybox visuals, camera follow, and pause feedback.
pub struct MovementPresentationPlugin;

#[derive(Resource, Default)]
pub(super) struct MatchMenuState {
    selected: usize,
    scoreboard_latched: bool,
    was_open: bool,
}

impl Plugin for MovementPresentationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MatchMenuState>()
            .add_systems(
                Startup,
                (spawn_client_camera, spawn_pause_overlay, spawn_client_hud).chain(),
            )
            .add_systems(
                Update,
                (
                    ensure_fighter_visuals,
                    handle_match_menu_input,
                    update_pause_overlay,
                    update_client_hud,
                )
                    .chain(),
            )
            .add_systems(
                PostUpdate,
                (
                    write_interpolated_fighter_pose_to_transform,
                    follow_controlled_camera,
                )
                    .chain()
                    .after(InterpolationSystems::Interpolate)
                    .after(PhysicsSystems::Writeback)
                    .before(TransformSystems::Propagate),
            );
    }
}

fn spawn_client_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        ArenaCamera,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: CAMERA_VERTICAL_SPAN,
            },
            ..OrthographicProjection::default_2d()
        }),
        // Keep the camera at Bevy's standard 2D depth origin. The default projection's
        // -1000..1000 clip range then contains every arena presentation layer, including
        // floor/objective overlays at negative z and combat effects at positive z.
        Transform::default(),
    ));
    commands.spawn((
        Camera2d,
        IsDefaultUiCamera,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        // UI ignores render layers. Keeping this camera off layer 0 makes it a dedicated
        // overlay pass so arena sprites can never depth-sort over HUD nodes.
        RenderLayers::layer(31),
    ));
}

fn spawn_pause_overlay(mut commands: Commands) {
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

fn spawn_client_hud(mut commands: Commands) {
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
        BuildSelectionText,
        Text::new("Select weapon: A/D or arrows | Space / South to confirm\nPulse Sidearm"),
        TextFont::from_font_size(22.0),
        TextColor(Color::srgb(0.85, 0.95, 1.0)),
        GlobalZIndex(200),
        BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.88)),
        Visibility::Inherited,
        Node {
            position_type: PositionType::Absolute,
            left: percent(22.0),
            right: percent(22.0),
            top: percent(18.0),
            padding: UiRect::all(px(12.0)),
            ..default()
        },
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
fn ensure_fighter_visuals(
    mut commands: Commands,
    assets: Option<Res<ClientAssetHandles>>,
    mut query: Query<
        (
            Entity,
            &NetworkEntityId,
            Ref<crate::combat::TeamId>,
            Has<Controlled>,
            Option<&mut Sprite>,
        ),
        (With<Fighter>, With<Remote>),
    >,
) {
    for (entity, network_id, team, controlled, sprite) in &mut query {
        let needs_sprite = sprite.is_none();
        if needs_sprite || team.is_changed() {
            let replacement = fighter_sprite(*network_id, *team, assets.as_deref());
            if let Some(mut sprite) = sprite {
                *sprite = replacement;
                continue;
            }
            commands
                .entity(entity)
                .insert((FighterVisual, replacement))
                .with_children(|parent| {
                    parent.spawn((
                        Text2d::new(if controlled {
                            format!("T{}  YOU", team.0 + 1)
                        } else {
                            format!("T{}", team.0 + 1)
                        }),
                        TextFont::from_font_size(if controlled { 22.0 } else { 18.0 }),
                        TextColor(Color::WHITE),
                        Transform::from_xyz(0.0, 45.0, 2.0),
                    ));
                });
        }
    }
}

fn fighter_sprite(
    network_id: NetworkEntityId,
    team: crate::combat::TeamId,
    assets: Option<&ClientAssetHandles>,
) -> Sprite {
    if network_id.0 == 0 {
        return Sprite::from_color(Color::srgb(0.72, 0.76, 0.82), Vec2::new(52.0, 32.0));
    }
    if let Some(assets) = assets {
        let image = if team.0 == 1 {
            assets.team_red.clone()
        } else {
            assets.team_blue.clone()
        };
        let mut sprite = Sprite::from_image(image);
        sprite.custom_size = Some(Vec2::splat(52.0));
        return sprite;
    }
    let color = if team.0 == 1 {
        Color::srgb(1.0, 0.42, 0.12)
    } else {
        Color::srgb(0.12, 0.72, 0.96)
    };
    Sprite::from_color(color, Vec2::new(48.0, 30.0))
}

/// Keep render-only replicated fighters visually aligned with Lightyear's interpolated pose.
///
/// The client intentionally does not replicate a server `RigidBody`, so Avian's normal
/// `RigidBody -> Transform` writeback is not sufficient for every interpolated fighter.  The
/// replicated Position/Rotation pair is the canonical presentation pose in Position mode.
#[allow(
    clippy::type_complexity,
    reason = "the query declares this system's complete world view inline at its schedule boundary"
)]
pub(super) fn write_interpolated_fighter_pose_to_transform(
    trace: Option<ResMut<LiveInputTrace>>,
    fighters: Query<(Entity, &Position, &Rotation, &mut Transform), (With<Fighter>, With<Remote>)>,
) {
    let mut trace = trace.filter(|trace| trace.enabled);
    for (entity, position, rotation, mut transform) in fighters {
        transform.translation.x = position.0.x;
        transform.translation.y = position.0.y;
        transform.rotation = Quat::from_rotation_z(rotation.as_radians());
        if let Some(trace) = trace.as_mut() {
            let last_position = trace
                .last_presented
                .iter()
                .find(|(candidate, _)| *candidate == entity)
                .map(|(_, position)| *position);
            if last_position.is_none_or(|last| last.distance(position.0) >= 32.0) {
                info!(
                    ?entity,
                    replicated_position = ?position.0,
                    visible_translation = ?transform.translation.truncate(),
                    "live client presented fighter pose"
                );
                trace
                    .last_presented
                    .retain(|(candidate, _)| *candidate != entity);
                trace.last_presented.push((entity, position.0));
            }
        }
    }
}

#[allow(
    clippy::type_complexity,
    reason = "the query declares this system's complete world view inline at its schedule boundary"
)]
fn follow_controlled_camera(
    map: Option<Res<crate::map::PresentedMap>>,
    fighters: Query<&Position, (With<Fighter>, With<Controlled>, Without<ArenaCamera>)>,
    mut cameras: Query<(&Camera, &mut Transform), With<ArenaCamera>>,
) {
    let Some(map) = map else {
        return;
    };
    let Some(position) = fighters.iter().next().map(|position| position.0) else {
        return;
    };
    for (camera, mut transform) in &mut cameras {
        let viewport = camera
            .logical_viewport_size()
            .filter(|size| size.x > 0.0 && size.y > 0.0)
            .unwrap_or(Vec2::new(16.0, 9.0));
        let center = clamp_camera_center(position, map.camera_bounds, viewport);
        transform.translation.x = center.x;
        transform.translation.y = center.y;
    }
}

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
    let half_height = CAMERA_VERTICAL_SPAN / 2.0;
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
            .add_systems(Update, exit_on_close_requested)
            .add_plugins((
                crate::map::MapPresentationPlugin,
                assets::ClientAssetPlugin,
                audio::ClientAudioPlugin,
                hud::ClientReadinessHudPlugin,
                MovementPresentationPlugin,
            ));
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
    fn arena_camera_clip_range_contains_all_presentation_layers() {
        const MIN_PRESENTATION_Z: f32 = -10.0;
        const MAX_PRESENTATION_Z: f32 = 39.0;

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(Startup, spawn_client_camera);
        app.update();

        let world = app.world_mut();
        let mut cameras = world.query_filtered::<(&Transform, &Projection), With<ArenaCamera>>();
        let (transform, projection) = cameras.single(world).unwrap();
        let Projection::Orthographic(projection) = projection else {
            panic!("arena camera must remain orthographic")
        };
        let nearest_visible_z = transform.translation.z - projection.far;
        let furthest_visible_z = transform.translation.z - projection.near;

        assert!(transform.translation.z.abs() <= f32::EPSILON);
        assert!(nearest_visible_z <= MIN_PRESENTATION_Z);
        assert!(furthest_visible_z >= MAX_PRESENTATION_Z);
    }

    #[test]
    fn fighter_visual_waits_for_team_and_refreshes_when_team_changes() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(Update, ensure_fighter_visuals);
        let fighter = app
            .world_mut()
            .spawn((Fighter, Remote, NetworkEntityId(7)))
            .id();

        app.update();
        assert!(app.world().get::<Sprite>(fighter).is_none());

        app.world_mut()
            .entity_mut(fighter)
            .insert(crate::combat::TeamId(1));
        app.update();
        assert_eq!(
            app.world().get::<Sprite>(fighter).unwrap().color,
            Color::srgb(1.0, 0.42, 0.12)
        );

        app.world_mut()
            .entity_mut(fighter)
            .insert(crate::combat::TeamId(0));
        app.update();
        assert_eq!(
            app.world().get::<Sprite>(fighter).unwrap().color,
            Color::srgb(0.12, 0.72, 0.96)
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
