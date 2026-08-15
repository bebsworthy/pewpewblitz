//! Shared match state and server-authoritative Wipeout composition.

#[cfg(feature = "server")]
mod lifecycle;
mod model;
#[cfg(feature = "server")]
mod server;
mod telemetry;
mod wipeout;

#[cfg(feature = "server")]
pub use lifecycle::AuthoritativeFighterLifecyclePlugin;
#[cfg(feature = "server")]
pub(crate) use lifecycle::{
    FighterLifecycleConfig, FighterReset, complete_fighter_lifecycle, fighter_runtime_values,
    reset_fighter_runtime,
};
pub use model::{
    ActiveCombatant, MatchId, MatchMember, MatchParticipant, MatchPhase, MatchResult, MatchRoot,
    MatchState, RespawnState, SpawnProtection,
};
#[cfg(feature = "server")]
pub use server::WipeoutPlugin;
#[cfg(all(feature = "server", test))]
pub(crate) use server::{credited_defeat_team, increment_score};
#[cfg(any(feature = "server", test))]
pub(crate) use telemetry::MatchTelemetryContext;
pub use telemetry::{
    MatchOutcomeDiagnostics, MatchParticipantSummary, MatchSummary, MatchTelemetry,
};
pub use wipeout::{
    SpawnCandidate, WIPEOUT_RULES_REVISION, WipeoutRules, assigned_team, complete_phase,
    score_result, select_spawn, timeout_result,
};

#[cfg(feature = "server")]
#[derive(bevy::prelude::SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum MatchSet {
    Lifecycle,
    FighterLifecycle,
    Outcomes,
}

#[cfg(feature = "server")]
fn configure_match_schedule(app: &mut bevy::prelude::App) {
    use bevy::prelude::*;

    app.configure_sets(
        FixedUpdate,
        (MatchSet::Lifecycle, MatchSet::FighterLifecycle).chain(),
    );
    app.configure_sets(
        FixedPostUpdate,
        MatchSet::Outcomes
            .after(crate::combat::CombatSet::Damage)
            .before(crate::combat::CombatSet::Lifecycle),
    );
}

#[cfg(test)]
mod tests;
