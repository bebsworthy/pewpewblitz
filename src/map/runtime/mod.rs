//! Authoritative map runtime schedule composition and public API.

use super::{MapDynamicGeneration, MapDynamicResetEvent, MapMutationEvent, MapPlacementId};
use crate::combat::CombatWorldEffectFacts;
use bevy::prelude::*;

const MAX_MAP_DYNAMIC_OUTBOX_EVENTS: usize = 256;
const MAX_RECOVERY_RESPONSES_PER_GENERATION: u8 = 4;

#[derive(Resource, Default)]
struct MapDynamicOutbox {
    mutations: Vec<MapMutationEvent>,
    reset: Option<MapDynamicResetEvent>,
}

/// Bounded process-lifetime evidence for map destruction and recovery traffic.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct MapDynamicTelemetry {
    pub destruction_requests: u64,
    pub destruction_applied: u64,
    pub destruction_no_ops: u64,
    pub placements_changed: u64,
    pub demolition_requests: u64,
    pub demolition_applied: u64,
    pub demolition_no_ops: u64,
    pub demolition_placements_changed: u64,
    pub recovery_requests: u64,
    pub recovery_responses: u64,
    pub recovery_rejections: u64,
    recovery_admission: std::collections::BTreeMap<Entity, (MapDynamicGeneration, u8)>,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DestructibleMapCollider {
    pub placement_id: MapPlacementId,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
struct PlayerOnlyMapCollider;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MapRuntimeSet {
    ApplyDestruction,
    Publish,
}

pub fn register_map_runtime(app: &mut App) {
    app.add_plugins((
        terminal_reactions::TerminalReactionRegistryPlugin,
        object_authority::ExplosionTerminalReactionPlugin,
        object_authority::RestorationPickupTerminalReactionPlugin,
    ))
    .init_resource::<CombatWorldEffectFacts>()
    .init_resource::<super::PendingWorldTargetDamages>()
    .init_resource::<super::WorldTargetDamageFacts>()
    .init_resource::<super::WorldObjectExplosionFacts>()
    .init_resource::<super::WorldObjectOutbox>()
    .init_resource::<super::WorldObjectTelemetry>()
    .init_resource::<MapDynamicOutbox>()
    .init_resource::<MapDynamicTelemetry>()
    .configure_sets(
        FixedPostUpdate,
        (
            MapRuntimeSet::ApplyDestruction
                .after(crate::abilities::AbilitySet::ObserveOutcomes)
                .after(crate::combat::CombatSet::Damage),
            MapRuntimeSet::Publish.after(MapRuntimeSet::ApplyDestruction),
        )
            .in_set(crate::gameplay::AuthoritativePhase::Environment)
            .before(crate::matchplay::MatchSet::ModeRules),
    )
    .add_systems(
        FixedPostUpdate,
        apply_map_destruction.in_set(MapRuntimeSet::ApplyDestruction),
    )
    .add_systems(
        FixedPostUpdate,
        process_world_target_damage.in_set(crate::combat::CombatDamageSet::WorldTargets),
    )
    .add_systems(
        FixedPostUpdate,
        effect_tiles::apply_damage_tile_pulses
            .in_set(crate::combat::CombatDamageSet::EnvironmentReactions),
    )
    .add_systems(
        FixedPostUpdate,
        publish_map_dynamic_traffic.in_set(MapRuntimeSet::Publish),
    )
    .add_systems(
        FixedPostUpdate,
        send_world_object_cues
            .in_set(crate::combat::CombatSet::TelemetryAndCues)
            .after(crate::concealment::ConcealmentSet::DecideObservers),
    )
    .add_systems(
        FixedPostUpdate,
        clear_world_object_tick_facts
            .in_set(crate::combat::CombatSet::Finalize)
            .before(crate::gameplay::advance_simulation_tick),
    )
    .add_systems(Update, receive_map_recovery_requests);
    super::pickups::register_pickup_runtime(app);
    effect_tiles::register(app);
    crate::matchplay::register_environment_reset_system(app, reset_map_on_match_restart);
}

mod dynamics;
pub(super) mod effect_tiles;
mod installation;
pub(crate) mod object_authority;
pub(crate) mod terminal_reactions;
#[cfg(test)]
mod tests;

pub use installation::install_resolved_map;

use dynamics::{
    apply_map_destruction, publish_map_dynamic_traffic, receive_map_recovery_requests,
    reset_map_on_match_restart,
};
use object_authority::{
    clear_world_object_tick_facts, process_world_target_damage, send_world_object_cues,
};
