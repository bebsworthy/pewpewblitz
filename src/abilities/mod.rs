//! Server-authoritative ultimate and passive runtime rules.

#[cfg(feature = "server")]
mod big_blob;
mod charge;
mod concealment_field;
mod dash;
#[cfg(feature = "server")]
mod demolition;
#[cfg(feature = "server")]
mod elemental_field;
mod passives;
mod reveal_scan;
mod self_cloak;
mod sentry;
mod telemetry;

pub use charge::apply_charge;
#[cfg(any(feature = "server", test))]
pub(crate) use charge::settled_ability_phase;
#[cfg(feature = "server")]
pub(crate) use dash::DashRuntime;
pub use dash::{bounded_dash_endpoint, dash_position, stable_dash_contacts};
#[cfg(feature = "server")]
pub(crate) use passives::apply_close_quarters_scale;
pub use passives::{apply_close_quarters_damage, apply_quick_cycle_ticks, apply_tenacity_ticks};
pub use reveal_scan::targeted_ultimate_center;
#[cfg(feature = "server")]
pub(crate) use self_cloak::UltimateGeneration;
pub use sentry::{
    Sentry, SentryDeadline, SentryIdentity, first_clear_sentry_placement, stable_sentry_target,
};
#[cfg(feature = "server")]
pub(crate) use sentry::{
    SentryCleanupRequest, cleanup_requested_sentries, request_sentry_lifecycle_cleanup,
};
pub use telemetry::{
    AbilityRejectionReason, AbilityTelemetry, AbilityTelemetryKind, AbilityTelemetryRecord,
    ConcealmentFieldCleanupReason, DashInterruptionReason, SentryCleanupReason,
    SentryTelemetryAggregate,
};

#[cfg(feature = "server")]
use bevy::ecs::schedule::ScheduleLabel;
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

/// Mode- and transport-neutral lifecycle snapshot consumed by owned ability behaviors.
#[cfg(feature = "server")]
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AbilityCleanupFacts {
    pub tick: u64,
    pub match_id: Option<crate::matchplay::MatchId>,
    pub match_completed: bool,
    pub owners: Vec<AbilityOwnerLifecycleFact>,
}

#[cfg(feature = "server")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AbilityOwnerLifecycleFact {
    pub network_id: crate::protocol::NetworkEntityId,
    pub defeated: bool,
    pub active: bool,
    pub controller_disconnected: bool,
}

#[cfg(feature = "server")]
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct AbilityCleanupSchedule;

#[cfg(feature = "server")]
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct AbilityPendingCleanupSchedule;

#[cfg(feature = "server")]
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum AbilityCleanupSet {
    PublishFacts,
    RequestBehaviors,
    ApplyBehaviors,
}

#[cfg(feature = "server")]
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum AbilityMovementSet {
    Cleanup,
    Advance,
}

#[cfg(feature = "server")]
struct AbilityCorePlugin;

#[cfg(feature = "server")]
impl Plugin for AbilityCorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AbilityTelemetry>()
            .init_resource::<telemetry::AbilityOutcomeObservationState>()
            .init_resource::<AbilityCleanupFacts>()
            .init_schedule(AbilityCleanupSchedule)
            .init_schedule(AbilityPendingCleanupSchedule)
            .configure_sets(
                AbilityCleanupSchedule,
                (
                    AbilityCleanupSet::PublishFacts,
                    AbilityCleanupSet::RequestBehaviors,
                    AbilityCleanupSet::ApplyBehaviors,
                )
                    .chain(),
            )
            .add_systems(
                AbilityCleanupSchedule,
                publish_ability_cleanup_facts.in_set(AbilityCleanupSet::PublishFacts),
            )
            .add_systems(
                AbilityCleanupSchedule,
                run_pending_ability_cleanup.in_set(AbilityCleanupSet::ApplyBehaviors),
            )
            .add_systems(
                FixedUpdate,
                suppress_frozen_actions.in_set(AbilitySet::Activation),
            );
    }
}

#[cfg(feature = "server")]
struct DashAbilityPlugin;

#[cfg(feature = "server")]
impl Plugin for DashAbilityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            dash::activate_dash
                .after(suppress_frozen_actions)
                .in_set(AbilitySet::Activation),
        )
        .add_systems(
            FixedUpdate,
            dash::advance_dash.in_set(AbilityMovementSet::Advance),
        );
    }
}

#[cfg(feature = "server")]
struct SentryAbilityPlugin;

#[cfg(feature = "server")]
impl Plugin for SentryAbilityPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<sentry::NextDeployableId>()
            .add_message::<SentryCleanupRequest>()
            .add_systems(
                FixedUpdate,
                sentry::activate_sentry
                    .after(dash::activate_dash)
                    .in_set(AbilitySet::Activation),
            )
            .add_systems(
                AbilityCleanupSchedule,
                request_sentry_lifecycle_cleanup.in_set(AbilityCleanupSet::RequestBehaviors),
            )
            .add_systems(
                AbilityPendingCleanupSchedule,
                (cleanup_requested_sentries, ApplyDeferred).chain(),
            )
            .add_systems(
                FixedUpdate,
                sentry::tick_sentries
                    .after(dash::advance_dash)
                    .in_set(AbilityMovementSet::Advance),
            );
    }
}

#[cfg(feature = "server")]
struct StealthAbilityPlugin;

#[cfg(feature = "server")]
impl Plugin for StealthAbilityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                self_cloak::activate_self_cloak.after(sentry::activate_sentry),
                reveal_scan::activate_reveal_scan.after(self_cloak::activate_self_cloak),
                concealment_field::activate_concealment_field
                    .after(reveal_scan::activate_reveal_scan),
            )
                .in_set(AbilitySet::Activation),
        )
        .add_systems(
            FixedUpdate,
            (concealment_field::cleanup_concealment_fields, ApplyDeferred)
                .chain()
                .after(run_ability_cleanup)
                .in_set(AbilityMovementSet::Cleanup),
        )
        .add_systems(
            FixedPostUpdate,
            (concealment_field::cleanup_concealment_fields, ApplyDeferred)
                .chain()
                .after(crate::combat::CombatSet::Lifecycle)
                .before(crate::concealment::ConcealmentSet::ResolveSources),
        );
    }
}

#[cfg(feature = "server")]
struct ImpactAbilityPlugin;

#[cfg(feature = "server")]
impl Plugin for ImpactAbilityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                demolition::activate_demolition_strike
                    .after(concealment_field::activate_concealment_field),
                elemental_field::activate_elemental_field
                    .after(demolition::activate_demolition_strike),
                big_blob::activate_big_blob.after(elemental_field::activate_elemental_field),
                ApplyDeferred.after(big_blob::activate_big_blob),
            )
                .in_set(AbilitySet::Activation),
        )
        .add_systems(
            FixedUpdate,
            big_blob::advance_big_blob_parents
                .after(sentry::tick_sentries)
                .in_set(AbilityMovementSet::Advance),
        );
    }
}

#[cfg(feature = "server")]
struct AbilityOutcomePlugin;

#[cfg(feature = "server")]
impl Plugin for AbilityOutcomePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedPostUpdate,
            (
                charge::observe_primary_damage_charge,
                passives::observe_passive_triggers,
                self_cloak::resolve_self_cloak_lifecycle,
                telemetry::observe_ability_outcomes,
            )
                .chain()
                .in_set(AbilitySet::ObserveOutcomes),
        );
    }
}

#[cfg(feature = "server")]
pub struct ServerAbilityPlugin;

#[cfg(feature = "server")]
impl Plugin for ServerAbilityPlugin {
    fn build(&self, app: &mut App) {
        configure_ability_schedule(app);
        app.add_plugins((
            AbilityCorePlugin,
            DashAbilityPlugin,
            SentryAbilityPlugin,
            StealthAbilityPlugin,
            ImpactAbilityPlugin,
            AbilityOutcomePlugin,
        ))
        .add_systems(
            FixedUpdate,
            run_ability_cleanup.in_set(AbilityMovementSet::Cleanup),
        );
    }
}

#[cfg(feature = "server")]
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Bevy system parameters are schedule-injected by value"
)]
fn publish_ability_cleanup_facts(
    tick: Res<crate::timing::SimulationTick>,
    roots: Query<&crate::matchplay::MatchState, With<crate::matchplay::MatchRoot>>,
    owners: Query<(
        &crate::protocol::NetworkEntityId,
        Option<&crate::combat::Defeated>,
        Option<&crate::matchplay::ActiveCombatant>,
        Option<&lightyear::prelude::ControlledBy>,
    )>,
    disconnected: Query<
        Entity,
        (
            With<lightyear::prelude::LinkOf>,
            With<lightyear::prelude::Disconnected>,
        ),
    >,
    mut facts: ResMut<AbilityCleanupFacts>,
) {
    let root = roots.single().ok();
    facts.tick = tick.0;
    facts.match_id = root.map(|root| root.match_id);
    facts.match_completed = root
        .is_some_and(|root| matches!(root.phase, crate::matchplay::MatchPhase::Completed { .. }));
    facts.owners.clear();
    facts.owners.extend(
        owners.iter().map(
            |(network_id, defeated, active, controlled)| AbilityOwnerLifecycleFact {
                network_id: *network_id,
                defeated: defeated.is_some(),
                active: active.is_some(),
                controller_disconnected: controlled
                    .is_some_and(|controlled| disconnected.contains(controlled.owner)),
            },
        ),
    );
}

/// Runs every registered ability cleanup behavior at the caller's existing lifecycle boundary.
#[cfg(feature = "server")]
pub(crate) fn run_ability_cleanup(world: &mut World) {
    world.run_schedule(AbilityCleanupSchedule);
}

/// Applies cleanup requests already emitted at another authoritative lifecycle point.
#[cfg(feature = "server")]
pub(crate) fn run_pending_ability_cleanup(world: &mut World) {
    world.run_schedule(AbilityPendingCleanupSchedule);
}

#[cfg(feature = "server")]
#[allow(clippy::needless_pass_by_value)]
fn suppress_frozen_actions(
    tick: Res<crate::timing::SimulationTick>,
    mut fighters: Query<(
        &crate::combat::ActiveEffects,
        &mut lightyear::prelude::input::native::ActionState<crate::protocol::FighterInput>,
    )>,
) {
    for (effects, mut action) in &mut fighters {
        if effects.is_frozen(tick.0) {
            action.0.gameplay_buttons &= !(crate::protocol::FighterInput::PRIMARY_FIRE
                | crate::protocol::FighterInput::ULTIMATE);
        }
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
        FixedUpdate,
        (AbilityMovementSet::Cleanup, AbilityMovementSet::Advance)
            .chain()
            .in_set(AbilitySet::Movement),
    )
    .configure_sets(
        FixedPostUpdate,
        AbilitySet::ObserveOutcomes
            .in_set(crate::gameplay::AuthoritativePhase::Effects)
            .after(crate::combat::CombatSet::Damage)
            .before(crate::matchplay::MatchSet::Outcomes),
    );
}

#[cfg(test)]
mod tests;
