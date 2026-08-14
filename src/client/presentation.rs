//! Windowed arena, fighter, camera, HUD, and pause presentation.
#![allow(clippy::wildcard_imports)]

use super::*;

/// Client-only greybox visuals, camera follow, and pause feedback.
pub struct MovementPresentationPlugin;

impl Plugin for MovementPresentationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            (
                spawn_client_arena,
                spawn_client_camera,
                spawn_pause_overlay,
                spawn_client_hud,
            )
                .chain(),
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

pub(super) fn spawn_client_arena(mut commands: Commands, arena: Res<GreyboxArenaDefinition>) {
    let border_color = Color::srgb(0.08, 0.34, 0.58);
    let boundary_color = Color::srgb(0.40, 0.86, 1.0);
    let cover_color = Color::srgb(0.08, 0.34, 0.68);
    let cover_edge_color = Color::srgb(0.68, 0.92, 1.0);
    for (position, size) in arena.perimeter_visual_shapes() {
        commands.spawn((
            ArenaVisual,
            Sprite::from_color(border_color, size),
            Transform::from_translation(position.extend(-2.0)),
        ));
    }
    // The collision bodies remain outside the playable bounds. This in-bounds layer is
    // deliberately thick enough to survive a compact window and a camera at any arena edge;
    // only its inner edge is bright so the HUD remains readable when the camera reaches a wall.
    for (position, size) in arena.perimeter_visual_edge_shapes() {
        commands.spawn((
            ArenaVisual,
            Sprite::from_color(boundary_color, size),
            Transform::from_translation(position.extend(1.0)),
        ));
    }
    for (position, size) in arena.cover_shapes() {
        commands.spawn((
            ArenaVisual,
            Sprite::from_color(cover_color, size),
            // Keep blocker bodies above the arena markers/background so the complete cover,
            // rather than only its edge strip, remains visible in the window.
            Transform::from_translation(position.extend(2.0)),
        ));
        commands.spawn((
            ArenaVisual,
            Sprite::from_color(cover_edge_color, Vec2::new(size.x, 10.0)),
            Transform::from_translation(
                (position + Vec2::new(0.0, size.y / 2.0 - 5.0)).extend(3.0),
            ),
        ));
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
        Transform::from_xyz(0.0, 0.0, 1000.0),
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
            Visibility::Hidden,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("PAUSED\nEscape / Menu to resume"),
                TextFont::from_font_size(28.0),
                TextColor(Color::WHITE),
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
        Node {
            position_type: PositionType::Absolute,
            left: px(16.0),
            bottom: px(16.0),
            width: percent(52.0),
            ..default()
        },
    ));
    commands.spawn((
        InputStatusText,
        Text::new("Input: keyboard/mouse | gameplay"),
        TextFont::from_font_size(16.0),
        TextColor(Color::srgb(0.75, 0.9, 1.0)),
        TextLayout::new(Justify::Right, LineBreak::WordBoundary),
        Node {
            position_type: PositionType::Absolute,
            right: px(16.0),
            bottom: px(16.0),
            width: percent(42.0),
            ..default()
        },
    ));
    commands.spawn((
        CombatHudText,
        Text::new("Health ---   Pulse --/--   READY"),
        TextFont::from_font_size(20.0),
        TextColor(Color::srgb(1.0, 0.85, 0.35)),
        Node {
            position_type: PositionType::Absolute,
            left: px(16.0),
            top: px(16.0),
            ..default()
        },
    ));
    commands.spawn((
        WeaponSelectionText,
        Text::new("Select weapon: A/D or arrows • Space / South to confirm\nPulse Sidearm"),
        TextFont::from_font_size(22.0),
        TextColor(Color::srgb(0.85, 0.95, 1.0)),
        Visibility::Inherited,
        Node {
            position_type: PositionType::Absolute,
            left: percent(25.0),
            right: percent(25.0),
            top: percent(18.0),
            ..default()
        },
    ));
    commands.spawn((
        ScoreboardOverlay,
        Text::new("SCOREBOARD\nLocal fighter roster is authoritative"),
        TextFont::from_font_size(22.0),
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            right: px(24.0),
            top: px(24.0),
            ..default()
        },
        Visibility::Hidden,
    ));
}

fn ensure_fighter_visuals(
    mut commands: Commands,
    mut query: Query<
        (Entity, &PlayerId, &NetworkEntityId, Option<&mut Sprite>),
        (With<Fighter>, With<Remote>),
    >,
) {
    for (entity, player_id, network_id, sprite) in &mut query {
        if sprite.is_none() {
            if network_id.0 == 0 {
                commands.entity(entity).insert((
                    FighterVisual,
                    Sprite::from_color(Color::srgb(0.95, 0.25, 0.1), Vec2::new(52.0, 32.0)),
                ));
                continue;
            }
            commands.entity(entity).insert((
                FighterVisual,
                Sprite::from_color(fighter_color(*player_id), Vec2::new(48.0, 28.0)),
            ));
        }
    }
}

/// Keep render-only replicated fighters visually aligned with Lightyear's interpolated pose.
///
/// The client intentionally does not replicate a server `RigidBody`, so Avian's normal
/// `RigidBody -> Transform` writeback is not sufficient for every interpolated fighter.  The
/// replicated Position/Rotation pair is the canonical presentation pose in Position mode.
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

fn follow_controlled_camera(
    arena: Res<GreyboxArenaDefinition>,
    fighters: Query<&Position, (With<Fighter>, With<Controlled>, Without<ArenaCamera>)>,
    mut cameras: Query<(&Camera, &mut Transform), With<ArenaCamera>>,
) {
    let Some(position) = fighters.iter().next().map(|position| position.0) else {
        return;
    };
    for (camera, mut transform) in &mut cameras {
        let viewport = camera
            .logical_viewport_size()
            .filter(|size| size.x > 0.0 && size.y > 0.0)
            .unwrap_or(Vec2::new(16.0, 9.0));
        let center = clamp_camera_center(position, *arena, viewport);
        transform.translation.x = center.x;
        transform.translation.y = center.y;
    }
}

pub(super) fn clamp_camera_center(
    position: Vec2,
    arena: GreyboxArenaDefinition,
    viewport: Vec2,
) -> Vec2 {
    let aspect = if viewport.y > 0.0 {
        viewport.x / viewport.y
    } else {
        16.0 / 9.0
    };
    let half_height = CAMERA_VERTICAL_SPAN / 2.0;
    let half_width = half_height * aspect.max(0.0);
    let min = arena.min + Vec2::new(half_width, half_height);
    let max = arena.max - Vec2::new(half_width, half_height);
    Vec2::new(
        if min.x > max.x {
            f32::midpoint(arena.min.x, arena.max.x)
        } else {
            position.x.clamp(min.x, max.x)
        },
        if min.y > max.y {
            f32::midpoint(arena.min.y, arena.max.y)
        } else {
            position.y.clamp(min.y, max.y)
        },
    )
}

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
        let mode = if matches!(*context, ClientInputContext::Paused) {
            "paused"
        } else {
            "gameplay"
        };
        let connection = connection
            .iter()
            .next()
            .map_or("offline", |status| match status.phase {
                ClientJoinPhase::Connecting => "connecting",
                ClientJoinPhase::AwaitingOutcome => "handshaking",
                ClientJoinPhase::Active { .. } => "connected",
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
            .add_plugins(MovementPresentationPlugin);
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
