//! Shared movement rules and server-authoritative Avian integration.

use avian2d::prelude::*;
use bevy::prelude::*;

mod arena;
#[cfg(feature = "server")]
mod authority;
mod input;
pub use arena::*;
#[cfg(feature = "server")]
pub use input::InputValidationState;
#[cfg(feature = "server")]
pub(crate) use input::decoded_input_is_valid;
pub use input::{
    InputFreshness, InputTuning, active_slow_multiplier, adrenaline_multiplier, committed_aim,
    decoded_move, desired_pose_step, input_should_neutralize, latest_present_remote_tick,
    radial_deadzone, trigger_pressed,
};

/// Canonical circular fighter footprint used by movement, combat, map clearance, and presentation.
pub const STANDARD_FIGHTER_RADIUS: f32 = 14.0;

/// Live input-trace switch and last-seen inputs. Read only by the server-gated coordinator.
#[derive(Resource, Debug)]
#[cfg_attr(not(feature = "server"), allow(dead_code))]
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
            radius: STANDARD_FIGHTER_RADIUS,
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
        app.init_resource::<MovementTuning>()
            .init_resource::<InputTuning>()
            .init_resource::<AuthoritativeInputTrace>()
            .insert_resource(Gravity(Vec2::ZERO));
        #[cfg(feature = "server")]
        {
            app.add_systems(
                FixedUpdate,
                authority::authoritative_movement.in_set(crate::gameplay::GameplaySet::Simulation),
            );
            authority::install_input_validators(app);
        }

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

#[cfg(test)]
mod tests;
