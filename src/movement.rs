//! Authoritative movement, greybox arena data, and input shaping.
#![allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    clippy::too_many_arguments
)]

use avian2d::prelude::*;
use bevy::prelude::*;
use core::time::Duration;
#[cfg(feature = "server")]
use lightyear::input::input_buffer::Compressed;
#[cfg(feature = "server")]
use lightyear::input::input_message::ActionStateSequence;
#[cfg(feature = "server")]
use lightyear::input::server::{InputValidationAppExt, authorize_controlled_targets};
#[cfg(feature = "server")]
use lightyear::prelude::ControlledBy;
#[cfg(feature = "server")]
use lightyear::prelude::input::native::NativeStateSequence;
use lightyear::prelude::input::native::{ActionState, NativeBuffer};
#[cfg(feature = "server")]
use lightyear::prelude::{LocalTimeline, MessageReceiver};

#[cfg(feature = "server")]
use crate::timing::SIMULATION_TICK;
use crate::{
    gameplay::GameplaySet,
    protocol::{Fighter, FighterInput, QuantizedAxis2},
    timing::SimulationTick,
};

pub const ARENA_MIN: Vec2 = Vec2::new(-800.0, -500.0);
pub const ARENA_MAX: Vec2 = Vec2::new(800.0, 500.0);
pub const CAMERA_VERTICAL_SPAN: f32 = 720.0;
pub const ARENA_WALL_THICKNESS: f32 = 48.0;

#[derive(Resource, Debug)]
struct AuthoritativeInputTrace {
    enabled: bool,
    last_inputs: Vec<(Entity, Vec2, u8)>,
}

impl FromWorld for AuthoritativeInputTrace {
    fn from_world(_world: &mut World) -> Self {
        Self {
            enabled: std::env::var("BRAWLER_INPUT_TRACE").as_deref() == Ok("1"),
            last_inputs: Vec::new(),
        }
    }
}

/// Immutable, code-authored greybox geometry shared by authoritative and client composition.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct GreyboxArenaDefinition {
    pub min: Vec2,
    pub max: Vec2,
    pub cover_centers: [Vec2; 2],
    pub cover_size: Vec2,
    pub spawn_x: [f32; 2],
    pub spawn_y: [f32; 4],
}

impl Default for GreyboxArenaDefinition {
    fn default() -> Self {
        Self {
            min: ARENA_MIN,
            max: ARENA_MAX,
            cover_centers: [Vec2::new(0.0, -220.0), Vec2::new(0.0, 220.0)],
            cover_size: Vec2::new(180.0, 120.0),
            spawn_x: [-620.0, 620.0],
            spawn_y: [-300.0, -100.0, 100.0, 300.0],
        }
    }
}

impl GreyboxArenaDefinition {
    #[must_use]
    pub fn spawn_slot(player_id: u64) -> u8 {
        u8::try_from(player_id.saturating_sub(1) % 8).expect("spawn slot modulo fits in u8")
    }

    #[must_use]
    pub fn spawn_position(self, player_id: u64) -> Vec2 {
        let slot = usize::from(Self::spawn_slot(player_id));
        Vec2::new(self.spawn_x[slot % 2], self.spawn_y[slot / 2])
    }

    #[must_use]
    pub fn perimeter_wall_shapes(self) -> [(Vec2, Vec2); 4] {
        let thickness = ARENA_WALL_THICKNESS;
        let width = self.max.x - self.min.x;
        let height = self.max.y - self.min.y;
        let center = (self.min + self.max) / 2.0;
        [
            (
                Vec2::new(self.min.x - thickness / 2.0, center.y),
                Vec2::new(thickness, height + thickness * 2.0),
            ),
            (
                Vec2::new(self.max.x + thickness / 2.0, center.y),
                Vec2::new(thickness, height + thickness * 2.0),
            ),
            (
                Vec2::new(center.x, self.min.y - thickness / 2.0),
                Vec2::new(width, thickness),
            ),
            (
                Vec2::new(center.x, self.max.y + thickness / 2.0),
                Vec2::new(width, thickness),
            ),
        ]
    }

    /// Return an in-bounds debug representation of the perimeter collision faces.
    ///
    /// The authoritative wall bodies intentionally sit outside the playable rectangle so a
    /// fighter can reach the exact clamped boundary. A camera following a fighter can therefore
    /// exclude the collider sprites entirely. Keep this presentation geometry inset from the
    /// collision face while deriving it from the same arena bounds.
    #[must_use]
    pub fn perimeter_visual_shapes(self) -> [(Vec2, Vec2); 4] {
        const VISUAL_THICKNESS: f32 = 24.0;
        let width = self.max.x - self.min.x;
        let height = self.max.y - self.min.y;
        let center = (self.min + self.max) / 2.0;
        [
            (
                Vec2::new(self.min.x + VISUAL_THICKNESS / 2.0, center.y),
                Vec2::new(VISUAL_THICKNESS, height),
            ),
            (
                Vec2::new(self.max.x - VISUAL_THICKNESS / 2.0, center.y),
                Vec2::new(VISUAL_THICKNESS, height),
            ),
            (
                Vec2::new(center.x, self.min.y + VISUAL_THICKNESS / 2.0),
                Vec2::new(width, VISUAL_THICKNESS),
            ),
            (
                Vec2::new(center.x, self.max.y - VISUAL_THICKNESS / 2.0),
                Vec2::new(width, VISUAL_THICKNESS),
            ),
        ]
    }

    /// Return the high-contrast inner edge for the in-bounds perimeter debug geometry.
    #[must_use]
    pub fn perimeter_visual_edge_shapes(self) -> [(Vec2, Vec2); 4] {
        const VISUAL_THICKNESS: f32 = 24.0;
        const EDGE_THICKNESS: f32 = 6.0;
        let edge_offset = VISUAL_THICKNESS - EDGE_THICKNESS / 2.0;
        let width = self.max.x - self.min.x;
        let height = self.max.y - self.min.y;
        let center = (self.min + self.max) / 2.0;
        [
            (
                Vec2::new(self.min.x + edge_offset, center.y),
                Vec2::new(EDGE_THICKNESS, height),
            ),
            (
                Vec2::new(self.max.x - edge_offset, center.y),
                Vec2::new(EDGE_THICKNESS, height),
            ),
            (
                Vec2::new(center.x, self.min.y + edge_offset),
                Vec2::new(width, EDGE_THICKNESS),
            ),
            (
                Vec2::new(center.x, self.max.y - edge_offset),
                Vec2::new(width, EDGE_THICKNESS),
            ),
        ]
    }

    #[must_use]
    pub fn cover_shapes(self) -> [(Vec2, Vec2); 2] {
        self.cover_centers.map(|center| (center, self.cover_size))
    }

    #[must_use]
    pub fn clamp_position(self, position: Vec2, radius: f32) -> Vec2 {
        position.clamp(
            self.min + Vec2::splat(radius),
            self.max - Vec2::splat(radius),
        )
    }
}

/// Fixed simulation values for the provisional fighter body.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct MovementTuning {
    pub speed: f32,
    pub radius: f32,
    pub spawn_facing: f32,
    pub stale_input_ticks: u64,
    pub move_iterations: usize,
    pub skin_width: f32,
}

impl Default for MovementTuning {
    fn default() -> Self {
        Self {
            speed: 320.0,
            radius: 24.0,
            spawn_facing: 0.0,
            stale_input_ticks: 12,
            move_iterations: 4,
            skin_width: 0.01,
        }
    }
}

/// Input shaping thresholds shared by controller, mouse, and focused tests.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct InputTuning {
    pub move_deadzone: f32,
    pub aim_deadzone: f32,
    pub aim_commit_threshold: f32,
    pub trigger_press: f32,
    pub trigger_release: f32,
    pub min_tick_delta: i64,
    pub max_tick_delta: i64,
    pub max_history_ticks: usize,
    pub input_rate: f32,
    pub input_burst: f32,
}

impl Default for InputTuning {
    fn default() -> Self {
        Self {
            move_deadzone: 0.20,
            aim_deadzone: 0.25,
            aim_commit_threshold: 0.35,
            trigger_press: 0.55,
            trigger_release: 0.45,
            min_tick_delta: -120,
            max_tick_delta: 16,
            max_history_ticks: 16,
            input_rate: 120.0,
            input_burst: 30.0,
        }
    }
}

/// Server-side input freshness used to turn a prolonged missing stream neutral.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputFreshness {
    pub last_fresh_tick: Option<u64>,
}

/// Per-connection guard state for the unordered native input channel.
#[cfg(feature = "server")]
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct InputValidationState {
    pub last_accepted_end_tick: Option<u32>,
    pub tokens: f32,
    pub last_refill_seconds: f32,
    pub ownership_rejections: u32,
    pub target_rejections: u32,
    pub malformed_rejections: u32,
    pub stale_or_reordered_rejections: u32,
    pub old_or_future_rejections: u32,
    pub rate_rejections: u32,
}

#[cfg(feature = "server")]
impl Default for InputValidationState {
    fn default() -> Self {
        Self {
            last_accepted_end_tick: None,
            tokens: InputTuning::default().input_burst,
            last_refill_seconds: 0.0,
            ownership_rejections: 0,
            target_rejections: 0,
            malformed_rejections: 0,
            stale_or_reordered_rejections: 0,
            old_or_future_rejections: 0,
            rate_rejections: 0,
        }
    }
}

#[cfg(feature = "server")]
fn input_history_len_is_valid(len: usize, tuning: InputTuning) -> bool {
    (1..=tuning.max_history_ticks).contains(&len)
}

#[cfg(feature = "server")]
fn input_target_is_entity(target: lightyear::input::input_message::InputTarget) -> bool {
    matches!(
        target,
        lightyear::input::input_message::InputTarget::Entity(_)
    )
}

#[cfg(feature = "server")]
fn input_end_tick_is_acceptable(
    end_tick: i64,
    server_tick: i64,
    last_accepted_end_tick: Option<u32>,
    tokens: f32,
    tuning: InputTuning,
) -> bool {
    end_tick >= server_tick + tuning.min_tick_delta
        && end_tick <= server_tick + tuning.max_tick_delta
        && last_accepted_end_tick.is_none_or(|last| end_tick > i64::from(last))
        && tokens >= 1.0
}

#[cfg(feature = "server")]
fn decoded_input_is_valid(input: FighterInput) -> bool {
    input.is_valid()
        && input.move_axis.to_vec2().length_squared() <= 1.0002
        && input
            .aim_update
            .is_none_or(|axis| axis.to_vec2().length_squared() <= 1.0002)
}

#[cfg(feature = "server")]
fn input_sequence_ends_with_present_state(
    states: impl Iterator<Item = Compressed<ActionState<FighterInput>>>,
) -> bool {
    let mut present = false;
    for state in states {
        match state {
            Compressed::Absent => present = false,
            Compressed::Input(_) => present = true,
            Compressed::SameAsPrecedent => {}
        }
    }
    present
}

/// Returns the newest remote tick whose resolved buffer value is present.
///
/// `InputBuffer::last_remote_tick` is a transport watermark, not a freshness
/// signal: Lightyear advances it even when a received state resolves to
/// `Compressed::Absent`. Only a present value (including a resolved
/// `SameAsPrecedent`) is evidence that the client supplied input for the tick.
fn latest_present_remote_tick(buffer: &NativeBuffer<FighterInput>) -> Option<u64> {
    let tick = buffer.last_remote_tick?;
    buffer.get(tick).map(|_| u64::from(tick.0))
}

/// Semantic marker for static greybox physics entities.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArenaWall;

/// Stable local marker for an arena spawn location.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpawnMarker(pub u8);

/// Typed collision-layer reservation for the first combat milestones.
pub const FIGHTER_LAYER: LayerMask = LayerMask(1 << 1);
pub const PROJECTILE_LAYER: LayerMask = LayerMask(1 << 2);
pub const INDESTRUCTIBLE_TERRAIN_LAYER: LayerMask = LayerMask(1 << 3);
pub const DESTRUCTIBLE_TERRAIN_LAYER: LayerMask = LayerMask(1 << 4);
pub const OBJECTIVE_LAYER: LayerMask = LayerMask(1 << 5);
pub const PICKUP_LAYER: LayerMask = LayerMask(1 << 6);
pub const HAZARD_LAYER: LayerMask = LayerMask(1 << 7);
pub const DEPLOYABLE_LAYER: LayerMask = LayerMask(1 << 8);

#[must_use]
pub fn fighter_collision_layers() -> CollisionLayers {
    CollisionLayers::new(
        FIGHTER_LAYER,
        INDESTRUCTIBLE_TERRAIN_LAYER | DESTRUCTIBLE_TERRAIN_LAYER,
    )
}

#[must_use]
pub fn terrain_collision_layers() -> CollisionLayers {
    CollisionLayers::new(
        INDESTRUCTIBLE_TERRAIN_LAYER,
        FIGHTER_LAYER | PROJECTILE_LAYER | DEPLOYABLE_LAYER,
    )
}

/// Apply a radial deadzone and remap the remaining magnitude to the full range.
#[must_use]
pub fn radial_deadzone(axis: Vec2, deadzone: f32) -> Vec2 {
    if !axis.is_finite() {
        return Vec2::ZERO;
    }
    let magnitude = axis.length();
    if magnitude <= deadzone || magnitude <= f32::EPSILON {
        Vec2::ZERO
    } else {
        axis / magnitude * ((magnitude - deadzone) / (1.0 - deadzone)).clamp(0.0, 1.0)
    }
}

/// Return a normalized facing update only when the post-deadzone aim is meaningful.
#[must_use]
pub fn committed_aim(axis: Vec2, tuning: InputTuning) -> Option<Vec2> {
    let remapped = radial_deadzone(axis, tuning.aim_deadzone);
    (remapped.length() >= tuning.aim_commit_threshold).then(|| remapped.normalize())
}

/// Hysteresis for an analog trigger represented as a held gameplay button.
#[must_use]
pub fn trigger_pressed(previous: bool, value: f32, tuning: InputTuning) -> bool {
    let value = if value.is_finite() { value } else { 0.0 };
    if previous {
        value >= tuning.trigger_release
    } else {
        value >= tuning.trigger_press
    }
}

/// Return the normalized movement axis used by the fixed simulation.
#[must_use]
pub fn decoded_move(input: QuantizedAxis2, tuning: InputTuning) -> Vec2 {
    radial_deadzone(input.to_vec2(), tuning.move_deadzone).clamp_length_max(1.0)
}

/// Return the exact movement/facing result before collision queries.
#[must_use]
pub fn desired_pose_step(
    position: Vec2,
    facing: f32,
    input: FighterInput,
    tuning: MovementTuning,
    input_tuning: InputTuning,
    delta: Duration,
) -> (Vec2, f32, Vec2) {
    let direction = decoded_move(input.move_axis, input_tuning);
    let aim = input
        .aim_update
        .and_then(|axis| committed_aim(axis.to_vec2(), input_tuning));
    let facing = aim.map_or(facing, |aim| aim.y.atan2(aim.x));
    let velocity = direction * tuning.speed;
    let position = position + velocity * delta.as_secs_f32();
    (position, facing, velocity)
}

#[must_use]
pub fn input_should_neutralize(
    current_tick: u64,
    last_fresh_tick: Option<u64>,
    limit: u64,
) -> bool {
    last_fresh_tick.is_none_or(|last| current_tick.saturating_sub(last) > limit)
}

#[must_use]
pub fn pose_is_valid(
    position: Vec2,
    facing: f32,
    arena: GreyboxArenaDefinition,
    radius: f32,
) -> bool {
    let min = arena.min + Vec2::splat(radius);
    let max = arena.max - Vec2::splat(radius);
    position.is_finite()
        && facing.is_finite()
        && position.x >= min.x
        && position.x <= max.x
        && position.y >= min.y
        && position.y <= max.y
}

/// Add the identical network/Avian integration used by the server and prediction-capable client.
pub struct AvianNetworkPlugin;

impl Plugin for AvianNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(lightyear::avian2d::plugin::LightyearAvianPlugin {
            replication_mode: lightyear::avian2d::plugin::AvianReplicationMode::Position {
                sync_to_transform: false,
            },
            register_physics_components: false,
            ..default()
        });
    }
}

/// Server-side collision and authoritative movement composition.
pub struct AuthoritativeMovementPlugin;

impl Plugin for AuthoritativeMovementPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GreyboxArenaDefinition>()
            .init_resource::<MovementTuning>()
            .init_resource::<InputTuning>()
            .init_resource::<AuthoritativeInputTrace>()
            .insert_resource(Gravity(Vec2::ZERO))
            .add_systems(Startup, spawn_greybox_arena)
            .add_systems(
                FixedUpdate,
                authoritative_movement.in_set(GameplaySet::Simulation),
            );

        #[cfg(feature = "server")]
        app.add_input_validator(authorize_controlled_targets::<NativeStateSequence<FighterInput>>)
            .add_input_validator(
                record_unauthorized_input_targets
                    .before(authorize_controlled_targets::<NativeStateSequence<FighterInput>>),
            )
            .add_input_validator(
                validate_fighter_input_messages
                    .after(authorize_controlled_targets::<NativeStateSequence<FighterInput>>),
            );

        // This is the only Avian physics schedule in the server world. The transform and
        // physics interpolation plugins stay disabled because Lightyear owns network pose history.
        app.add_plugins(
            PhysicsPlugins::default()
                .with_length_unit(100.0)
                .build()
                .disable::<PhysicsTransformPlugin>()
                .disable::<PhysicsInterpolationPlugin>(),
        );
    }
}

fn spawn_greybox_arena(mut commands: Commands, arena: Res<GreyboxArenaDefinition>) {
    for (position, size) in arena.perimeter_wall_shapes() {
        commands.spawn((
            ArenaWall,
            RigidBody::Static,
            Collider::rectangle(size.x, size.y),
            CollisionLayers::new(
                INDESTRUCTIBLE_TERRAIN_LAYER,
                FIGHTER_LAYER | PROJECTILE_LAYER | DEPLOYABLE_LAYER,
            ),
            Position::from_xy(position.x, position.y),
            Rotation::IDENTITY,
            Transform::from_translation(position.extend(0.0)),
        ));
    }
    for (position, size) in arena.cover_shapes() {
        commands.spawn((
            ArenaWall,
            RigidBody::Static,
            Collider::rectangle(size.x, size.y),
            terrain_collision_layers(),
            Position::from_xy(position.x, position.y),
            Rotation::IDENTITY,
            Transform::from_translation(position.extend(0.0)),
        ));
    }
    for (side, x) in arena.spawn_x.into_iter().enumerate() {
        for (row, y) in arena.spawn_y.into_iter().enumerate() {
            commands.spawn((
                SpawnMarker(
                    u8::try_from(side * arena.spawn_y.len() + row)
                        .expect("spawn marker fits in u8"),
                ),
                Position::from_xy(x, y),
            ));
        }
    }
}

fn authoritative_movement(
    mut commands: Commands,
    mut trace: ResMut<AuthoritativeInputTrace>,
    time: Res<Time<Fixed>>,
    tick: Res<SimulationTick>,
    arena: Res<GreyboxArenaDefinition>,
    tuning: Res<MovementTuning>,
    input_tuning: Res<InputTuning>,
    move_and_slide: MoveAndSlide,
    fighters: Query<
        (
            Entity,
            &Position,
            &Rotation,
            &Collider,
            &LinearVelocity,
            &InputFreshness,
            Option<&ActionState<FighterInput>>,
            Option<&NativeBuffer<FighterInput>>,
            Option<&crate::combat::Defeated>,
        ),
        With<Fighter>,
    >,
) {
    let config = MoveAndSlideConfig {
        move_and_slide_iterations: tuning.move_iterations,
        penetration_rejection_threshold: 2.0,
        skin_width: tuning.skin_width,
        ..default()
    };
    for (entity, position, rotation, collider, velocity, freshness, action, buffer, defeated) in
        &fighters
    {
        if defeated.is_some() {
            continue;
        }
        let previous_position = position.0;
        let mut position = *position;
        let mut rotation = *rotation;
        let mut velocity = *velocity;
        let mut freshness = *freshness;
        if let Some(remote_tick) = buffer.and_then(latest_present_remote_tick)
            && freshness
                .last_fresh_tick
                .is_none_or(|last| remote_tick > last)
        {
            freshness.last_fresh_tick = Some(remote_tick);
        }
        let stale =
            input_should_neutralize(tick.0, freshness.last_fresh_tick, tuning.stale_input_ticks);
        let input = action.map_or(FighterInput::default(), |action| action.0);
        let input = if !stale && input.is_valid() {
            input
        } else {
            FighterInput::default()
        };

        if let Some(aim) = input
            .aim_update
            .and_then(|axis| committed_aim(axis.to_vec2(), *input_tuning))
        {
            rotation = Rotation::radians(aim.y.atan2(aim.x));
        }
        let movement = decoded_move(input.move_axis, *input_tuning);
        let desired_velocity = movement * tuning.speed;
        let filter = SpatialQueryFilter::from_mask(
            INDESTRUCTIBLE_TERRAIN_LAYER | DESTRUCTIBLE_TERRAIN_LAYER,
        )
        .with_excluded_entities([entity]);
        let output = move_and_slide.move_and_slide(
            collider,
            position.0,
            rotation.as_radians(),
            desired_velocity,
            time.delta(),
            &config,
            &filter,
            |_| MoveAndSlideHitResponse::Accept,
        );
        position.0 = output.position;
        velocity.0 = output.projected_velocity;

        let facing = rotation.as_radians();
        if !pose_is_valid(position.0, facing, *arena, tuning.radius) {
            let repaired_position = if position.0.is_finite() {
                arena.clamp_position(position.0, tuning.radius)
            } else {
                arena.spawn_position(1)
            };
            position.0 = repaired_position;
            if !facing.is_finite() {
                rotation = Rotation::radians(tuning.spawn_facing);
            }
            warn!(?entity, "repaired invalid authoritative fighter pose");
        }
        if trace.enabled {
            let input_state = (input.move_axis.to_vec2(), input.gameplay_buttons);
            let last_input = trace
                .last_inputs
                .iter()
                .find(|(candidate, _, _)| *candidate == entity)
                .map(|(_, move_axis, buttons)| (*move_axis, *buttons));
            if last_input != Some(input_state) {
                info!(
                    tick = tick.0,
                    ?entity,
                    stale,
                    last_fresh_tick = ?freshness.last_fresh_tick,
                    move_axis = ?input.move_axis.to_vec2(),
                    position_before = ?previous_position,
                    position_after = ?position.0,
                    "live server authoritative input changed"
                );
                trace
                    .last_inputs
                    .retain(|(candidate, _, _)| *candidate != entity);
                trace
                    .last_inputs
                    .push((entity, input_state.0, input_state.1));
            }
        }
        commands
            .entity(entity)
            .insert((position, rotation, velocity, freshness));
    }
}

#[cfg(feature = "server")]
fn record_unauthorized_input_targets(
    mut receivers: Query<(
        Entity,
        &mut InputValidationState,
        &mut MessageReceiver<
            lightyear::input::input_message::InputMessage<NativeStateSequence<FighterInput>>,
        >,
    )>,
    controlled: Query<(Entity, &ControlledBy)>,
) {
    for (connection, mut state, mut receiver) in &mut receivers {
        receiver.retain_messages(|message| {
            for target in &message.inputs {
                if let lightyear::input::input_message::InputTarget::Entity(entity) = target.target
                {
                    let authorized = controlled.iter().any(|(controlled_entity, controlled)| {
                        controlled.owner == connection && controlled_entity == entity
                    });
                    if !authorized {
                        state.ownership_rejections = state.ownership_rejections.saturating_add(1);
                    }
                }
            }
            true
        });
    }
}

#[cfg(feature = "server")]
fn validate_fighter_input_messages(
    timeline: Res<LocalTimeline>,
    time: Res<Time<Real>>,
    input_tuning: Res<InputTuning>,
    mut receivers: Query<(
        &mut InputValidationState,
        &mut MessageReceiver<
            lightyear::input::input_message::InputMessage<NativeStateSequence<FighterInput>>,
        >,
    )>,
) {
    let now = time.elapsed_secs();
    let server_tick = i64::from(timeline.tick().0);
    for (mut state, mut receiver) in &mut receivers {
        let elapsed = (now - state.last_refill_seconds).max(0.0);
        state.tokens =
            (state.tokens + elapsed * input_tuning.input_rate).min(input_tuning.input_burst);
        state.last_refill_seconds = now;
        receiver.retain_messages(|message| {
            let Some(target) = message.inputs.first() else {
                state.target_rejections = state.target_rejections.saturating_add(1);
                return false;
            };
            if message.inputs.len() != 1 || !input_target_is_entity(target.target) {
                state.target_rejections = state.target_rejections.saturating_add(1);
                return false;
            }
            if !input_history_len_is_valid(target.states.len(), *input_tuning) {
                state.malformed_rejections = state.malformed_rejections.saturating_add(1);
                return false;
            }

            if !input_sequence_ends_with_present_state(
                target
                    .states
                    .clone()
                    .get_snapshots_from_message(SIMULATION_TICK),
            ) {
                state.malformed_rejections = state.malformed_rejections.saturating_add(1);
                return false;
            }

            let valid_states = target
                .states
                .clone()
                .get_snapshots_from_message(SIMULATION_TICK)
                .all(|state| match state {
                    Compressed::Input(state) => decoded_input_is_valid(state.0),
                    Compressed::Absent | Compressed::SameAsPrecedent => true,
                });
            if !valid_states {
                state.malformed_rejections = state.malformed_rejections.saturating_add(1);
                return false;
            }

            let end_tick = i64::from(message.end_tick.0);
            if !input_end_tick_is_acceptable(
                end_tick,
                server_tick,
                state.last_accepted_end_tick,
                state.tokens,
                *input_tuning,
            ) {
                if end_tick < server_tick + input_tuning.min_tick_delta
                    || end_tick > server_tick + input_tuning.max_tick_delta
                {
                    state.old_or_future_rejections =
                        state.old_or_future_rejections.saturating_add(1);
                } else if state
                    .last_accepted_end_tick
                    .is_some_and(|last| end_tick <= i64::from(last))
                {
                    state.stale_or_reordered_rejections =
                        state.stale_or_reordered_rejections.saturating_add(1);
                } else {
                    state.rate_rejections = state.rate_rejections.saturating_add(1);
                }
                return false;
            }

            state.tokens -= 1.0;
            state.last_accepted_end_tick = Some(message.end_tick.0);
            true
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radial_deadzone_remaps_and_clamps_diagonal_input() {
        assert_eq!(radial_deadzone(Vec2::splat(0.1), 0.2), Vec2::ZERO);
        let diagonal = radial_deadzone(Vec2::splat(1.0), 0.2);
        assert!(diagonal.length() <= 1.0 + f32::EPSILON);
        assert!((radial_deadzone(Vec2::new(0.6, 0.0), 0.2).x - 0.5).abs() < 1e-5);
    }

    #[test]
    fn movement_deadzone_is_applied_once_by_authoritative_decode() {
        let decoded = decoded_move(
            QuantizedAxis2::from_vec2(Vec2::new(0.6, 0.0)),
            InputTuning::default(),
        );
        assert!((decoded.x - 0.5).abs() < 0.01);
    }

    #[test]
    fn aim_threshold_preserves_last_valid_direction_by_returning_none() {
        let tuning = InputTuning::default();
        assert_eq!(committed_aim(Vec2::new(0.1, 0.0), tuning), None);
        assert_eq!(committed_aim(Vec2::new(1.0, 0.0), tuning), Some(Vec2::X));
    }

    #[test]
    fn trigger_hysteresis_does_not_chatter_between_thresholds() {
        let tuning = InputTuning::default();
        assert!(trigger_pressed(false, 0.56, tuning));
        assert!(trigger_pressed(true, 0.50, tuning));
        assert!(!trigger_pressed(true, 0.44, tuning));
    }

    #[test]
    fn known_fixed_input_moves_at_normalized_speed_and_keeps_facing_without_aim() {
        let tuning = MovementTuning::default();
        let input_tuning = InputTuning::default();
        let (position, facing, velocity) = desired_pose_step(
            Vec2::ZERO,
            0.7,
            FighterInput::from_axes(Vec2::splat(1.0), None, 0),
            tuning,
            input_tuning,
            Duration::from_secs_f32(1.0 / 60.0),
        );
        assert!((position.length() - tuning.speed / 60.0).abs() < 1e-4);
        assert!((facing - 0.7).abs() < f32::EPSILON);
        assert!((velocity.length() - tuning.speed).abs() < 1e-4);
    }

    #[test]
    fn missing_input_neutralizes_after_twelve_ticks() {
        assert!(!input_should_neutralize(12, Some(0), 12));
        assert!(input_should_neutralize(13, Some(0), 12));
        assert!(input_should_neutralize(1, None, 12));
    }

    #[test]
    fn absent_remote_input_does_not_refresh_freshness() {
        let input = FighterInput::from_axes(Vec2::X, None, 0);
        let mut buffer = NativeBuffer::<FighterInput>::default();
        buffer.set(lightyear::prelude::Tick(10), ActionState(input));
        buffer.last_remote_tick = Some(lightyear::prelude::Tick(10));
        assert_eq!(latest_present_remote_tick(&buffer), Some(10));

        buffer.set_empty(lightyear::prelude::Tick(11));
        buffer.last_remote_tick = Some(lightyear::prelude::Tick(11));
        assert_eq!(latest_present_remote_tick(&buffer), None);
    }

    #[test]
    fn camera_and_spawn_bounds_are_stable() {
        let arena = GreyboxArenaDefinition::default();
        assert_eq!(arena.spawn_position(1), Vec2::new(-620.0, -300.0));
        assert_eq!(arena.spawn_position(2), Vec2::new(620.0, -300.0));
        assert_eq!(arena.spawn_position(5), Vec2::new(-620.0, 100.0));
        assert_eq!(arena.spawn_position(8), Vec2::new(620.0, 300.0));
        assert_eq!(GreyboxArenaDefinition::spawn_slot(1), 0);
        assert_eq!(GreyboxArenaDefinition::spawn_slot(8), 7);
        let perimeter = arena.perimeter_wall_shapes();
        assert!((perimeter[0].0.x - (arena.min.x - ARENA_WALL_THICKNESS / 2.0)).abs() < 0.001);
        assert!((perimeter[1].0.x - (arena.max.x + ARENA_WALL_THICKNESS / 2.0)).abs() < 0.001);
        assert!((perimeter[2].0.y - (arena.min.y - ARENA_WALL_THICKNESS / 2.0)).abs() < 0.001);
        assert!((perimeter[3].0.y - (arena.max.y + ARENA_WALL_THICKNESS / 2.0)).abs() < 0.001);
        assert_eq!(
            arena.cover_shapes()[0],
            (Vec2::new(0.0, -220.0), Vec2::new(180.0, 120.0))
        );
        assert_eq!(
            arena.clamp_position(Vec2::new(9_000.0, -9_000.0), 24.0),
            Vec2::new(776.0, -476.0)
        );
    }

    #[test]
    fn perimeter_visual_shapes_are_in_bounds_and_follow_collision_faces() {
        let arena = GreyboxArenaDefinition::default();
        let visuals = arena.perimeter_visual_shapes();

        assert_eq!(
            visuals[0],
            (Vec2::new(-788.0, 0.0), Vec2::new(24.0, 1000.0))
        );
        assert_eq!(visuals[1], (Vec2::new(788.0, 0.0), Vec2::new(24.0, 1000.0)));
        assert_eq!(
            visuals[2],
            (Vec2::new(0.0, -488.0), Vec2::new(1600.0, 24.0))
        );
        assert_eq!(visuals[3], (Vec2::new(0.0, 488.0), Vec2::new(1600.0, 24.0)));

        for (position, size) in visuals {
            let min = position - size / 2.0;
            let max = position + size / 2.0;
            assert!(min.x >= arena.min.x);
            assert!(min.y >= arena.min.y);
            assert!(max.x <= arena.max.x);
            assert!(max.y <= arena.max.y);
        }

        let edges = arena.perimeter_visual_edge_shapes();
        assert_eq!(edges[0], (Vec2::new(-779.0, 0.0), Vec2::new(6.0, 1000.0)));
        assert_eq!(edges[1], (Vec2::new(779.0, 0.0), Vec2::new(6.0, 1000.0)));
        assert_eq!(edges[2], (Vec2::new(0.0, -479.0), Vec2::new(1600.0, 6.0)));
        assert_eq!(edges[3], (Vec2::new(0.0, 479.0), Vec2::new(1600.0, 6.0)));
    }

    #[test]
    fn pose_validation_uses_fighter_center_bounds() {
        let arena = GreyboxArenaDefinition::default();
        assert!(pose_is_valid(Vec2::new(776.0, 0.0), 0.0, arena, 24.0));
        assert!(!pose_is_valid(Vec2::new(800.0, 0.0), 0.0, arena, 24.0));
        assert!(!pose_is_valid(Vec2::new(0.0, -500.0), 0.0, arena, 24.0));
    }

    #[cfg(feature = "server")]
    #[test]
    fn input_watermark_rejects_invalid_order_future_and_rate_excess() {
        let tuning = InputTuning::default();
        assert!(input_history_len_is_valid(1, tuning));
        assert!(!input_history_len_is_valid(0, tuning));
        assert!(!input_history_len_is_valid(
            tuning.max_history_ticks + 1,
            tuning
        ));
        assert!(input_end_tick_is_acceptable(
            100,
            100,
            Some(99),
            1.0,
            tuning
        ));
        assert!(!input_end_tick_is_acceptable(
            99,
            100,
            Some(99),
            1.0,
            tuning
        ));
        assert!(!input_end_tick_is_acceptable(
            117,
            100,
            Some(99),
            1.0,
            tuning
        ));
        assert!(!input_end_tick_is_acceptable(
            100,
            100,
            Some(99),
            0.5,
            tuning
        ));
        assert!(input_target_is_entity(
            lightyear::input::input_message::InputTarget::Entity(
                Entity::from_raw_u32(1).expect("valid test entity index"),
            )
        ));
        assert!(!input_target_is_entity(
            lightyear::input::input_message::InputTarget::PreSpawned(1)
        ));
    }

    #[cfg(feature = "server")]
    #[test]
    fn malformed_input_bits_and_axes_are_rejected_without_client_masking() {
        let mut malformed = FighterInput::from_axes(Vec2::X, None, 0);
        malformed.gameplay_buttons = 0x80;
        assert!(!decoded_input_is_valid(malformed));

        let mut too_fast = FighterInput::from_axes(Vec2::X, None, 0);
        too_fast.move_axis = QuantizedAxis2 {
            x: QuantizedAxis2::MAX,
            y: QuantizedAxis2::MAX,
        };
        assert!(!decoded_input_is_valid(too_fast));
    }

    #[cfg(feature = "server")]
    #[test]
    fn absent_end_state_cannot_refresh_input_validation() {
        let input = FighterInput::from_axes(Vec2::X, None, 0);
        assert!(input_sequence_ends_with_present_state(
            [
                Compressed::Input(ActionState(input)),
                Compressed::SameAsPrecedent,
            ]
            .into_iter(),
        ));
        assert!(!input_sequence_ends_with_present_state(
            [Compressed::Input(ActionState(input)), Compressed::Absent,].into_iter(),
        ));
        assert!(!input_sequence_ends_with_present_state(
            [Compressed::Absent, Compressed::SameAsPrecedent].into_iter(),
        ));
    }
}
