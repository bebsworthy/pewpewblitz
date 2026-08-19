//! Windowed arena, fighter, camera, HUD, and pause presentation.
#![allow(clippy::wildcard_imports)]

use super::*;

/// Client-only greybox visuals, camera follow, and pause feedback.
pub struct MovementPresentationPlugin;

impl Plugin for MovementPresentationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            (spawn_client_camera, spawn_pause_overlay, spawn_client_hud).chain(),
        )
        .add_systems(
            Update,
            (
                ensure_fighter_visuals,
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
                Text::new("PAUSED\nEscape / Menu to resume"),
                TextFont::from_font_size(28.0),
                TextColor(Color::WHITE),
            ));
            parent.spawn((
                InputSettingsText,
                Text::new(""),
                TextFont::from_font_size(14.0),
                TextColor(Color::srgb(0.75, 0.9, 1.0)),
                TextLayout::linebreak(LineBreak::WordBoundary),
                Node {
                    margin: UiRect::all(px(4.0)),
                    ..default()
                },
            ));
        });
}

fn spawn_client_hud(mut commands: Commands) {
    commands.spawn((
        ControlsText,
        Text::new("WASD / left stick: move   Mouse / right stick: aim\nQ: active item   E: ultimate   Space: interact   Tab: scoreboard   Esc: pause/cancel"),
        TextFont::from_font_size(16.0),
        TextColor(Color::WHITE),
        TextLayout::linebreak(LineBreak::WordBoundary),
        GlobalZIndex(100),
        Node {
            position_type: PositionType::Absolute,
            left: px(16.0),
            bottom: px(32.0),
            width: percent(52.0),
            ..default()
        },
    ));
    commands.spawn((
        InputStatusText,
        Text::new("Input: keyboard/mouse | gameplay"),
        TextFont::from_font_size(16.0),
        TextColor(Color::srgb(0.75, 0.9, 1.0)),
        TextLayout::new(Justify::Left, LineBreak::WordBoundary),
        GlobalZIndex(100),
        Node {
            position_type: PositionType::Absolute,
            right: px(16.0),
            bottom: px(32.0),
            width: percent(42.0),
            ..default()
        },
    ));
    commands.spawn((
        CombatHudText,
        Text::new("Health ---   Pulse --/--   READY"),
        TextFont::from_font_size(20.0),
        TextColor(Color::srgb(1.0, 0.85, 0.35)),
        TextLayout::linebreak(LineBreak::WordBoundary),
        GlobalZIndex(100),
        Node {
            position_type: PositionType::Absolute,
            left: px(16.0),
            right: px(16.0),
            top: px(16.0),
            ..default()
        },
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
        Text::new("SCOREBOARD\nLocal fighter roster is authoritative"),
        TextFont::from_font_size(22.0),
        TextColor(Color::WHITE),
        GlobalZIndex(250),
        Node {
            position_type: PositionType::Absolute,
            right: px(24.0),
            top: px(24.0),
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
                        Text2d::new("^"),
                        TextFont::from_font_size(if controlled { 22.0 } else { 18.0 }),
                        TextColor(Color::WHITE),
                        Transform::from_xyz(0.0, 30.0, 2.0),
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
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn update_pause_overlay(
    context: Res<ClientInputContext>,
    mut overlays: Query<&mut Visibility, With<PauseOverlay>>,
) {
    for mut visibility in &mut overlays {
        *visibility = if matches!(*context, ClientInputContext::Paused) {
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
    connection: Query<&ClientJoinStatus, With<Client>>,
    mut status: Query<&mut Text, With<InputStatusText>>,
    mut scoreboard: Query<&mut Visibility, With<ScoreboardOverlay>>,
) {
    if pending.is_changed() || context.is_changed() {
        let device = match pending.active_device {
            ActiveInputDevice::KeyboardMouse => "keyboard/mouse",
            ActiveInputDevice::Gamepad(_) => "gamepad",
        };
        let mode = match *context {
            ClientInputContext::Gameplay => "gameplay",
            ClientInputContext::Paused => "paused",
            ClientInputContext::Shell => "shell",
        };
        let connection = connection
            .iter()
            .next()
            .map_or("offline", |status| match status.phase {
                ClientJoinPhase::Connecting => "connecting",
                ClientJoinPhase::AwaitingOutcome => "handshaking",
                ClientJoinPhase::Active { .. } => "connected",
                ClientJoinPhase::LobbyActive { .. } => "lobby",
                ClientJoinPhase::Rejected(_) => "rejected",
                ClientJoinPhase::Disconnected => "disconnected",
            });
        let mut actions = String::new();
        for (bit, label) in [
            (ACTION_PRIMARY_FIRE, "fire"),
            (ACTION_ACTIVE_ITEM, "item"),
            (ACTION_ULTIMATE, "ultimate"),
            (ACTION_INTERACT, "interact"),
            (ACTION_CANCEL, "cancel"),
            (ACTION_PAUSE, "pause"),
            (ACTION_SCOREBOARD, "scoreboard"),
        ] {
            if pending.action_indicator & bit != 0 {
                if !actions.is_empty() {
                    actions.push(',');
                }
                actions.push_str(label);
            }
        }
        if actions.is_empty() {
            actions.push_str("none");
        }
        for mut text in &mut status {
            **text =
                format!("Connection: {connection}\nInput: {device} | {mode}\nActions: {actions}");
        }
    }
    for mut visibility in &mut scoreboard {
        *visibility = if pending.scoreboard_held {
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
}
