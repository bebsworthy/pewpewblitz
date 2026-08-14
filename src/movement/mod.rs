//! Shared movement rules and server-authoritative Avian integration.
#![allow(clippy::needless_pass_by_value, clippy::type_complexity)]

use avian2d::prelude::*;
use bevy::prelude::*;
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
    combat::AuthoritativePose,
    gameplay::GameplaySet,
    protocol::{Fighter, FighterInput},
    timing::SimulationTick,
};

mod arena;
mod input;
pub use arena::*;
#[cfg(feature = "server")]
pub use input::InputValidationState;
use input::latest_present_remote_tick;
pub use input::{
    InputFreshness, InputTuning, committed_aim, decoded_move, desired_pose_step,
    input_should_neutralize, radial_deadzone, trigger_pressed,
};
#[cfg(feature = "server")]
use input::{
    decoded_input_is_valid, input_end_tick_is_acceptable, input_history_len_is_valid,
    input_sequence_ends_with_present_state, input_target_is_entity,
};

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

#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
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
            Option<&crate::combat::SelectingWeapon>,
            Option<&crate::combat::AwaitingPostSelectionInput>,
            Option<&crate::combat::ActiveEffects>,
            Option<&crate::combat::ExternalMotion>,
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
    for (
        entity,
        position,
        rotation,
        collider,
        velocity,
        freshness,
        action,
        buffer,
        defeated,
        selecting,
        activation_barrier,
        active_effects,
        external_motion,
    ) in &fighters
    {
        if defeated.is_some() || selecting.is_some() {
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

        let activation_ready = activation_barrier.is_none_or(|barrier| {
            freshness
                .last_fresh_tick
                .is_some_and(|fresh_tick| fresh_tick > barrier.accepted_at_tick)
        });
        if activation_ready
            && let Some(aim) = input
                .aim_update
                .and_then(|axis| committed_aim(axis.to_vec2(), *input_tuning))
        {
            rotation = Rotation::radians(aim.y.atan2(aim.x));
        }
        let movement = decoded_move(input.move_axis, *input_tuning);
        let movement_multiplier = active_effects
            .and_then(|effects| effects.slow)
            .filter(|slow| tick.0 < slow.expires_at_tick)
            .map_or(1.0, |slow| {
                f32::from(slow.movement_multiplier_milli) / 1000.0
            });
        let desired_velocity = if activation_ready {
            movement * tuning.speed * movement_multiplier
                + external_motion
                    .filter(|motion| tick.0 < motion.expires_at_tick)
                    .map_or(Vec2::ZERO, |motion| motion.velocity)
        } else {
            Vec2::ZERO
        };
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
        commands.entity(entity).insert((
            position,
            rotation,
            velocity,
            freshness,
            AuthoritativePose {
                position: crate::combat::WorldPoint::from(position.0),
                facing,
                tick: tick.0,
            },
        ));
        if activation_barrier.is_some_and(|barrier| {
            freshness
                .last_fresh_tick
                .is_some_and(|fresh_tick| fresh_tick > barrier.accepted_at_tick)
        }) {
            commands
                .entity(entity)
                .remove::<crate::combat::AwaitingPostSelectionInput>();
        }
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
mod tests;
