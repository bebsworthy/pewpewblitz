//! Shared match state and server-authoritative common lifecycle plus installed mode rules.

mod hot_zone;
#[cfg(feature = "server")]
mod lifecycle;
mod model;
#[cfg(feature = "server")]
mod server;
mod telemetry;
mod wipeout;

#[cfg(feature = "server")]
pub use hot_zone::HotZoneModePlugin;
#[cfg(feature = "server")]
pub use hot_zone::hot_zone_setup_for_composition;
pub use hot_zone::{
    HOT_ZONE_NEAR_COMBAT_EXPANSION, HOT_ZONE_RULES_REVISION, HotZoneRules, HotZoneState,
    HotZoneStatus, HotZoneSummary, hot_zone_rules_for_profile,
};
#[cfg(feature = "server")]
pub use lifecycle::AuthoritativeFighterLifecyclePlugin;
#[cfg(feature = "server")]
pub(crate) use lifecycle::{
    FighterLifecycleConfig, FighterReset, complete_fighter_lifecycle, fighter_runtime_values,
    reset_fighter_runtime,
};
pub use model::{
    ActiveCombatant, MatchClock, MatchId, MatchMember, MatchParticipant, MatchPhase, MatchResult,
    MatchRoot, MatchState, RespawnState, SpawnProtection,
};
#[cfg(feature = "server")]
pub use server::{
    AuthoritativeMatchPlugin, MATCH_LIFECYCLE_RULES_REVISION, MatchLifecycleRules, MatchModeSetup,
    SpawnCandidate, assigned_team, select_spawn,
};
#[cfg(feature = "server")]
pub(crate) use server::{
    ConnectedMatchRoster, ModeOutcomeCause, ModeRuleOutcome, PendingModeRuleOutcome,
    clear_combat_facts, offer_mode_rule_outcome, prepare_mode_rule_facts, record_match_telemetry,
};
#[cfg(any(feature = "server", test))]
pub(crate) use telemetry::MatchTelemetryContext;
pub use telemetry::{
    MatchOutcomeDiagnostics, MatchParticipantSummary, MatchSummary, MatchTelemetry, ModeSummary,
    WipeoutSummary,
};
#[cfg(feature = "server")]
pub use wipeout::WipeoutModePlugin;
#[cfg(all(feature = "server", test))]
pub(crate) use wipeout::credited_defeat_team;
pub use wipeout::{
    WIPEOUT_RULES_REVISION, WipeoutRules, WipeoutState, score_result, timeout_result,
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
            MatchRestartSet::Commit,
        )
            .chain()
            .in_set(MatchSet::Lifecycle),
    );
    app.configure_sets(
        FixedPostUpdate,
        MatchSet::ModeRules
            .after(crate::abilities::AbilitySet::ObserveOutcomes)
            .after(crate::combat::CombatSet::Damage),
    );
    app.configure_sets(
        FixedPostUpdate,
        MatchSet::Outcomes
            .after(MatchSet::ModeRules)
            .before(crate::combat::CombatSet::Lifecycle),
    );
}

#[cfg(test)]
mod tests;
