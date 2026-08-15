//! Server-authoritative ultimate and passive runtime rules.

mod charge;
mod dash;
mod passives;
mod sentry;
mod telemetry;

#[cfg(any(feature = "server", test))]
pub(crate) use charge::settled_ability_phase;
pub use charge::{ULTIMATE_CHARGE_MAX, apply_charge};
#[cfg(feature = "server")]
pub(crate) use dash::DashRuntime;
pub use dash::{
    DASH_DURATION_TICKS, DASH_MAX_DISTANCE, bounded_dash_endpoint, dash_position,
    stable_dash_contacts,
};
#[cfg(feature = "server")]
pub(crate) use passives::apply_close_quarters_scale;
pub use passives::{
    ADRENAL_DURATION_TICKS, ADRENAL_REARM_TICKS, apply_close_quarters_damage,
    apply_quick_cycle_ticks, apply_tenacity_ticks,
};
pub use sentry::{
    SENTRY_ACQUISITION_INTERVAL_TICKS, SENTRY_ACQUISITION_RANGE, SENTRY_FIRE_INTERVAL_TICKS,
    SENTRY_LIFETIME_TICKS, SENTRY_MAXIMUM_HEALTH, SENTRY_PLACEMENT_OFFSETS, SENTRY_RADIUS, Sentry,
    SentryDeadline, SentryIdentity, first_clear_sentry_placement, stable_sentry_target,
};
#[cfg(feature = "server")]
pub(crate) use sentry::{
    SentryCleanupRequest, cleanup_requested_sentries, request_sentry_lifecycle_cleanup,
};
pub use telemetry::{
    AbilityRejectionReason, AbilityTelemetry, AbilityTelemetryKind, AbilityTelemetryRecord,
    DashInterruptionReason, SentryCleanupReason, SentryTelemetryAggregate,
};

#[cfg(feature = "server")]
use bevy::prelude::*;

#[cfg(feature = "server")]
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct UltimateInputLatch(pub bool);

#[cfg(feature = "server")]
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum AbilitySet {
    Activation,
    Movement,
    ObserveOutcomes,
}

#[cfg(feature = "server")]
pub struct ServerAbilityPlugin;

#[cfg(feature = "server")]
impl Plugin for ServerAbilityPlugin {
    fn build(&self, app: &mut App) {
        configure_ability_schedule(app);
        app.init_resource::<sentry::NextDeployableId>()
            .init_resource::<AbilityTelemetry>()
            .init_resource::<telemetry::AbilityOutcomeObservationState>()
            .add_message::<SentryCleanupRequest>()
            .add_systems(
                FixedUpdate,
                (dash::activate_dash, sentry::activate_sentry, ApplyDeferred)
                    .chain()
                    .in_set(AbilitySet::Activation),
            )
            .add_systems(
                FixedUpdate,
                (
                    request_sentry_lifecycle_cleanup,
                    cleanup_requested_sentries,
                    ApplyDeferred,
                    dash::advance_dash,
                    sentry::tick_sentries,
                )
                    .chain()
                    .in_set(AbilitySet::Movement),
            )
            .add_systems(
                FixedPostUpdate,
                (
                    charge::observe_primary_damage_charge,
                    passives::observe_passive_triggers,
                    telemetry::observe_ability_outcomes,
                )
                    .chain()
                    .in_set(AbilitySet::ObserveOutcomes),
            );
    }
}

#[cfg(feature = "server")]
fn configure_ability_schedule(app: &mut App) {
    app.configure_sets(
        FixedUpdate,
        (AbilitySet::Activation, AbilitySet::Movement)
            .chain()
            .after(crate::gameplay::GameplaySet::Input)
            .before(crate::gameplay::GameplaySet::Simulation),
    )
    .configure_sets(
        FixedPostUpdate,
        AbilitySet::ObserveOutcomes
            .after(crate::combat::CombatSet::Damage)
            .before(crate::matchplay::MatchSet::Outcomes),
    );
}

#[cfg(test)]
mod tests;
