//! Shared match state and server-authoritative common lifecycle plus installed mode rules.

#[cfg(feature = "server")]
mod lifecycle;
mod model;
mod objective_results;
mod turf_hold;
mod vault_break;
pub use objective_results::ObjectiveResult;
#[cfg(feature = "client")]
pub use objective_results::ObjectiveTotals;
#[cfg(feature = "server")]
mod server;
mod shard_rush;
mod spawns;
mod takedown;
mod telemetry;
mod zombie;

#[cfg(feature = "server")]
pub use lifecycle::AuthoritativeFighterLifecyclePlugin;
#[cfg(feature = "server")]
pub(crate) use lifecycle::{
    FighterLifecycleConfig, FighterReset, complete_fighter_lifecycle, reset_fighter_runtime,
};
#[cfg(feature = "server")]
pub(crate) use model::BotObjectiveView;
pub use model::{
    ActiveCombatant, FighterDisplayName, MatchClock, MatchId, MatchMember, MatchParticipant,
    MatchPhase, MatchResult, MatchRoot, MatchState, ParticipantControl, PublicParticipantState,
    PublicParticipantStatus, ResolvedMatchCapacity, RespawnState, SpawnProtection,
    TeamSlotCapacity,
};
#[cfg(feature = "server")]
pub use server::{
    AuthoritativeMatchPlugin, MATCH_LIFECYCLE_RULES_REVISION, MatchLifecycleRules, MatchModeSetup,
};
#[cfg(feature = "server")]
pub(crate) use server::{
    ConnectedMatchRoster, ModeOutcomeCause, ModeRuleOutcome, PendingMatchRestart,
    PendingModeRuleOutcome, clear_combat_facts, initialize_match_root, offer_mode_rule_outcome,
    prepare_mode_rule_facts, record_match_telemetry,
};
#[cfg(feature = "balance-lab")]
pub(crate) use server::{
    NextMatchId, PendingMatchRestartSlot, RestartBuildPolicy, prepare_match_restart,
};
#[cfg(feature = "server")]
pub(crate) use shard_rush::ShardRushTelemetry;
pub use shard_rush::{
    MAX_SHARD_RUSH_LIVE_CEILING, MAX_SHARD_RUSH_TARGET, SHARD_BODY_RADIUS, SHARD_NETWORK_NAMESPACE,
    SHARD_PICKUP_RADIUS, SHARD_RUSH_RULES_REVISION, Shard, ShardCarrier, ShardIdentity,
    ShardOrigin, ShardRushCountdown, ShardRushPolicy, ShardRushRules, ShardRushState,
    ShardRushSummary, leading_team as shard_rush_leading_team, shard_rush_rules_for_profile,
    timeout_result as shard_rush_timeout_result,
};
#[cfg(feature = "server")]
pub use shard_rush::{ShardRushModePlugin, ShardRushSource};
pub use spawns::{SpawnCandidate, assigned_team, select_spawn};
#[cfg(feature = "server")]
pub use takedown::TakedownModePlugin;
pub use takedown::{
    TAKEDOWN_RULES_REVISION, TakedownPlayerStats, TakedownRules, TakedownState, score_result,
    timeout_result,
};
#[cfg(all(feature = "server", test))]
pub(crate) use takedown::{credited_defeat_team, credited_defeat_team_with_recent};
#[cfg(feature = "server")]
pub(crate) use telemetry::MatchTelemetryContext;
pub use telemetry::{
    MatchOutcomeDiagnostics, MatchParticipantSummary, MatchSummary, MatchTelemetry, ModeSummary,
    TakedownSummary,
};
#[cfg(feature = "server")]
pub(crate) use turf_hold::ResolvedObjectiveZone;
#[cfg(feature = "server")]
pub use turf_hold::TurfHoldModePlugin;
#[cfg(feature = "server")]
pub use turf_hold::turf_hold_setup_for_composition;
pub use turf_hold::{
    TURF_HOLD_NEAR_COMBAT_EXPANSION, TURF_HOLD_RULES_REVISION, TurfHoldRules, TurfHoldState,
    TurfHoldStatus, TurfHoldSummary, turf_hold_rules_for_profile,
};
#[cfg(feature = "client")]
pub use vault_break::ReceivedVaultBreakObjectiveCue;
pub use vault_break::{
    MAX_VAULT_BREAK_OBJECTIVE_CUES, VAULT_BREAK_RULES_REVISION, VAULT_BREAK_SAFE_COUNT,
    VAULT_BREAK_SAFE_HALF_EXTENTS, VaultBreakCompletion, VaultBreakHealthComparison,
    VaultBreakObjectiveCue, VaultBreakObjectiveCueKind, VaultBreakRules, VaultBreakSafe,
    VaultBreakSafeIdentity, VaultBreakState, VaultBreakSummary, destroyed_safe_result,
    objective_cue_kind, remaining_health_comparison, timeout_result as vault_break_timeout_result,
};
#[cfg(feature = "server")]
pub use vault_break::{
    PendingModeObjectiveDamage, PendingModeObjectiveDamages, VaultBreakModePlugin,
    VaultBreakTelemetry,
};
#[allow(
    unused_imports,
    reason = "client-only role builds retain the shared HostileNpc source conversion contract"
)]
pub(crate) use zombie::ZOMBIE_DEFINITION;
#[cfg(feature = "server")]
pub use zombie::ZombieModePlugin;
#[cfg(feature = "server")]
pub(crate) use zombie::ZombieTelemetry;
pub use zombie::{
    BoostGrantScope, BoostPickup, BoostPickupAvailableAtTick, BoostPickupIdentity,
    BoostPickupSource, BoostStacks, HostileNpc, HostileNpcDefinitionId, HostileNpcIdentity,
    ZOMBIE_RULES_REVISION, ZombieModeState, ZombiePhase, ZombieRules, ZombieSummary,
    effective_maximum_health, scale_boosted_value,
};

#[cfg(feature = "server")]
#[derive(bevy::prelude::SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum MatchSet {
    /// Common roster/phase/restart transaction, before any gameplay.
    Lifecycle,
    /// Installed mode deadline check at or after the active `ends_at_tick`.
    DeadlineRules,
    /// Common forfeit precedence and deadline outcome commitment, before fighter lifecycle.
    PreGameOutcomes,
    /// Common respawn/reset/protection fighter lifecycle.
    FighterLifecycle,
    /// Installed mode post-damage scoring/progress inside the fixed-post transaction.
    ModeRules,
    /// Common outcome consumption, defeat respawns, telemetry, fact clear, cleanup, summary.
    Outcomes,
}

/// The explicitly chained restart transaction inside `MatchSet::Lifecycle`: common prepare,
/// installed-mode in-place reset, common commit. No observer, deferred flush, or replication
/// extraction may run between these sets.
#[cfg(feature = "server")]
#[derive(bevy::prelude::SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum MatchRestartSet {
    Prepare,
    ModeReset,
    EnvironmentReset,
    Commit,
}

#[cfg(feature = "server")]
pub(crate) fn configure_match_schedule(app: &mut bevy::prelude::App) {
    use bevy::prelude::*;

    app.configure_sets(
        FixedUpdate,
        (
            MatchSet::Lifecycle,
            MatchSet::DeadlineRules,
            MatchSet::PreGameOutcomes,
            MatchSet::FighterLifecycle,
        )
            .chain()
            .in_set(crate::gameplay::GameplaySet::Lifecycle),
    );
    app.configure_sets(
        FixedUpdate,
        (
            MatchRestartSet::Prepare,
            MatchRestartSet::ModeReset,
            MatchRestartSet::EnvironmentReset,
            MatchRestartSet::Commit,
        )
            .chain()
            .in_set(MatchSet::Lifecycle),
    );
    app.configure_sets(
        FixedPostUpdate,
        MatchSet::ModeRules
            .in_set(crate::gameplay::AuthoritativePhase::Objectives)
            .after(crate::abilities::AbilitySet::ObserveOutcomes)
            .after(crate::combat::CombatSet::Damage),
    );
    app.configure_sets(
        FixedPostUpdate,
        MatchSet::Outcomes
            .in_set(crate::gameplay::AuthoritativePhase::Objectives)
            .after(MatchSet::ModeRules)
            .before(crate::combat::CombatSet::Lifecycle),
    );
}

/// Register one environment-reset system into the common restart transaction between
/// mode reset and commit. Map dynamics reset synchronously here so no
/// downstream system observes a new match with old environment state.
#[cfg(feature = "server")]
pub(crate) fn register_environment_reset_system<S, Marker>(app: &mut bevy::prelude::App, system: S)
where
    S: bevy::ecs::system::IntoSystem<(), (), Marker>,
{
    use bevy::ecs::schedule::IntoScheduleConfigs as _;
    app.add_systems(
        bevy::prelude::FixedUpdate,
        system.in_set(MatchRestartSet::EnvironmentReset),
    );
}

#[cfg(feature = "server")]
#[cfg(test)]
mod tests;
