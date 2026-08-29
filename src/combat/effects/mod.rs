//! Deterministic immediate effect policies.
//!
//! The composed-payload pipeline is organized as four ordered stages: collection and
//! deterministic ordering (`application::collect_composed_batch`), event planning and
//! reservation (`application::plan_composed_events` plus `planning`), per-record
//! application (`application::apply_composed_records`), and deferred commit
//! (`application::commit_composed_batch`). The system below only sequences the stages
//! and owns no rules itself.

#[allow(clippy::wildcard_imports)]
#[cfg(feature = "server")]
use super::*;
use super::{ActiveEffects, AttackId, ExternalMotion, NetworkEntityId, SlowEffect};
use bevy::prelude::Vec2;

#[cfg(feature = "server")]
mod application;
#[cfg(feature = "server")]
mod planning;
mod runtime;
#[cfg(test)]
mod tests;

#[cfg(feature = "server")]
use application::{
    AppliedComposedState, BatchView, ComposedBatch, apply_composed_records, collect_composed_batch,
    commit_composed_batch, plan_composed_events,
};
#[cfg(feature = "server")]
use planning::{abort_composed_event_batch, resolve_pending_deliveries};
#[cfg(feature = "server")]
pub(crate) use planning::{
    finish_attack_delivery, flush_completed_attack_telemetry, payload_target_visible,
};
#[cfg(feature = "server")]
pub(crate) use runtime::{
    apply_cold_contribution, apply_resistance, refresh_damage_over_time, requested_damage,
};
/// The bounded authoritative transaction outputs produced by payload resolution, grouped
/// to keep the scheduling system within the engine's system-parameter budget.
#[cfg(feature = "server")]
#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct CombatTransactionState<'w> {
    legacy_telemetry: ResMut<'w, CombatTelemetry>,
    outbox: ResMut<'w, CombatOutbox>,
    world_effect_facts: ResMut<'w, CombatWorldEffectFacts>,
    outcome_facts: ResMut<'w, CombatOutcomeFacts>,
}

/// Target, owner, passive, and match-state queries used by payload resolution, grouped so
/// the stage functions can share one coherent view of the same fixed-tick world. Field
/// access discipline mirrors the original single system: disjoint `ParamSet` members are
/// borrowed per stage, never across stages that conflict.
#[cfg(feature = "server")]
#[allow(
    clippy::type_complexity,
    reason = "each field is the complete world view one payload stage needs; factoring them into aliases would hide the stage contracts"
)]
#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct CombatTargetState<'w, 's> {
    targets: ParamSet<
        'w,
        's,
        (
            Query<
                'w,
                's,
                (
                    &'static NetworkEntityId,
                    &'static TeamId,
                    &'static mut CurrentHealth,
                    Option<&'static mut ActiveEffects>,
                    Option<&'static ExternalMotion>,
                    Option<&'static Defeated>,
                    Option<&'static lightyear::prelude::ControlledBy>,
                    Option<&'static TestDummy>,
                ),
                Or<(With<Fighter>, With<crate::abilities::Sentry>)>,
            >,
            Query<
                'w,
                's,
                (
                    Entity,
                    &'static NetworkEntityId,
                    &'static TeamId,
                    &'static CurrentHealth,
                    Option<&'static Defeated>,
                    Option<&'static lightyear::prelude::ControlledBy>,
                ),
                Or<(With<Fighter>, With<crate::abilities::Sentry>)>,
            >,
        ),
    >,
    owners: Query<
        'w,
        's,
        (
            &'static NetworkEntityId,
            Option<&'static lightyear::prelude::ControlledBy>,
        ),
        With<Fighter>,
    >,
    passive_access: ParamSet<
        'w,
        's,
        (
            Query<'w, 's, &'static crate::builds::ResolvedMatchLoadout>,
            Query<
                'w,
                's,
                (
                    &'static NetworkEntityId,
                    &'static crate::builds::ResolvedMatchLoadout,
                ),
                With<Fighter>,
            >,
        ),
    >,
    sentry_targets: Query<'w, 's, (), With<crate::abilities::Sentry>>,
    disconnected: Query<'w, 's, Entity, (With<LinkOf>, With<lightyear::prelude::Disconnected>)>,
    match_access: ParamSet<
        'w,
        's,
        (
            Query<'w, 's, (), With<crate::matchplay::MatchParticipant>>,
            Query<'w, 's, (), With<crate::matchplay::ActiveCombatant>>,
            Query<'w, 's, (), With<crate::matchplay::SpawnProtection>>,
        ),
    >,
}

#[cfg(feature = "server")]
#[allow(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    reason = "the stage coordinator hands each stage the fixed-tick state it owns; the parameter list is the pipeline's explicit data flow, and every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn resolve_composed_payloads(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    mut ids: ResMut<NextCombatIds>,
    mut trackers: ResMut<ActiveAttackTrackers>,
    mut payloads: MessageReader<PendingPayload>,
    mut deliveries: MessageReader<PendingDelivery>,
    mut gameplay_telemetry: AbilityWeaponTelemetry,
    mut transaction: CombatTransactionState,
    mut combat: CombatTargetState,
) {
    // Stage one: collect and deterministically order this tick's payloads and deliveries.
    let batch = collect_composed_batch(&mut combat, payloads.read(), deliveries.read());

    // Stage two: dry-run the complete batch against a target snapshot, then reserve every
    // event ID the batch can consume. Exhaustion aborts the whole batch; an earlier target
    // must never be partially committed while a later record fails to reserve its IDs.
    let Some(reserved) = plan_composed_events(
        &mut combat,
        &mut ids,
        &batch,
        &mut gameplay_telemetry.weapon,
    ) else {
        abort_composed_event_batch(
            &mut commands,
            &mut trackers,
            &batch.deliveries,
            &batch.records,
        );
        return;
    };
    let mut reserved_events = reserved.into_iter();

    // Stage three: resolve deliveries, then apply every payload record against live state.
    let ComposedBatch {
        disconnected,
        connected_owners,
        close_quarters_owners,
        records,
        deliveries,
    } = batch;
    let mut resolved_delivery_keys = resolve_pending_deliveries(
        &mut commands,
        deliveries,
        &connected_owners,
        &mut reserved_events,
        &mut trackers,
        &mut gameplay_telemetry.weapon,
        &mut transaction.legacy_telemetry,
        &mut transaction.outbox,
        &mut transaction.world_effect_facts,
    );
    let view = BatchView {
        disconnected: &disconnected,
        connected_owners: &connected_owners,
        close_quarters_owners: &close_quarters_owners,
        records: &records,
    };
    let mut applied = AppliedComposedState::default();
    apply_composed_records(
        &mut commands,
        tick.0,
        &view,
        &mut combat,
        &mut reserved_events,
        &mut trackers,
        &mut gameplay_telemetry,
        &mut transaction,
        &mut applied,
        &mut resolved_delivery_keys,
    );

    // Stage four: commit accumulated effects, deferred cues, and tracker completion.
    commit_composed_batch(
        &mut commands,
        &mut trackers,
        &mut transaction,
        applied,
        resolved_delivery_keys,
    );
}
