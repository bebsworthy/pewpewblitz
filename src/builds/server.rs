//! Waiting-phase build-selection transaction: request resolution, install, and response.
//!
//! This is build/session authority, not endpoint composition: the system resets input
//! epochs, resolves a server-owned candidate, cleans deployables and transients, installs
//! the loadout and runtime state, records telemetry, and responds idempotently.

use crate::combat::{ActiveEffects, HealthRecoveryState, WeaponCatalogResource, WeaponState};
use crate::matchplay::{MatchParticipant, MatchPhase, MatchRoot, MatchState};
use crate::protocol::{
    BuildSelectionDecision, BuildSelectionOutcome, BuildSelectionRequest, FighterInput,
    NetworkEntityId, SessionChannel,
};
use crate::server::ServerSession;
use avian2d::prelude::CollisionLayers;
use bevy::prelude::*;
use lightyear::prelude::Disconnected;
use lightyear::prelude::input::native::{ActionState, NativeBuffer};
use lightyear::prelude::{ControlledBy, MessageReceiver, MessageSender};
use std::collections::HashSet;

use super::{BuildCatalogResource, BuildResolutionError, BuildTelemetry, resolve_build_recipe};

/// Resolve a bounded build request against server-owned catalogs and the current waiting
/// match. One system by design: the transaction is atomic within the fixed Update step.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the multi-receiver session query and multi-field fighter install query are Bevy system parameters owned by the schedule runtime, and the transaction is deliberately atomic in one fixed Update step"
)]
pub fn process_build_selection(
    mut commands: Commands,
    builds: Res<BuildCatalogResource>,
    weapons: Res<WeaponCatalogResource>,
    definitions: Res<crate::combat::FighterDefinitions>,
    tick: Res<crate::timing::SimulationTick>,
    mut telemetry: ResMut<crate::combat::WeaponTelemetry>,
    mut build_telemetry: ResMut<BuildTelemetry>,
    match_root: Query<&MatchState, With<MatchRoot>>,
    sentries: Query<&crate::abilities::SentryIdentity, With<crate::abilities::Sentry>>,
    mut sentry_cleanup_requests: MessageWriter<crate::abilities::SentryCleanupRequest>,
    mut sessions: Query<(
        Entity,
        &mut MessageReceiver<BuildSelectionRequest>,
        &mut MessageSender<BuildSelectionOutcome>,
        &mut ServerSession,
        Has<Disconnected>,
    )>,
    mut fighter_query: Query<
        (
            Entity,
            &ControlledBy,
            &crate::combat::FighterDefinitionId,
            &NetworkEntityId,
            &MatchParticipant,
            Option<&mut NativeBuffer<FighterInput>>,
            Option<&mut ActionState<FighterInput>>,
            Option<&mut crate::movement::InputFreshness>,
        ),
        With<crate::protocol::Fighter>,
    >,
) {
    let mut accepted_fighters_this_tick = HashSet::new();
    let Ok(match_state) = match_root.single() else {
        return;
    };
    for (connection, mut receiver, mut sender, mut session, disconnected) in &mut sessions {
        if disconnected {
            receiver.receive().for_each(drop);
            continue;
        }
        let requests: Vec<_> = receiver.receive().collect();
        for request in requests {
            if session
                .last_selection_request
                .is_some_and(|previous| request.request_id < previous.request_id)
            {
                let outcome = BuildSelectionOutcome {
                    request_id: request.request_id,
                    match_id: request.match_id,
                    decision: BuildSelectionDecision::Stale,
                    accepted_identity: None,
                    accepted_total_points: None,
                };
                session.last_selection_response = Some(outcome);
                sender.send::<SessionChannel>(outcome);
                continue;
            }
            if session
                .last_selection_request
                .is_some_and(|previous| request.request_id == previous.request_id)
            {
                if let Some(outcome) = session.last_selection_outcome {
                    session.last_selection_response = Some(outcome);
                    sender.send::<SessionChannel>(outcome);
                }
                continue;
            }

            let fighter = fighter_query
                .iter_mut()
                .find(|(_, controlled, _, _, _, _, _, _)| controlled.owner == connection);
            let outcome = if let Some((
                fighter_entity,
                _,
                fighter_definition_id,
                fighter_network_id,
                participant,
                mut input_buffer,
                mut action,
                mut input_freshness,
            )) = fighter
            {
                if request.match_id != match_state.match_id
                    || participant.match_id != match_state.match_id
                {
                    BuildSelectionOutcome {
                        request_id: request.request_id,
                        match_id: request.match_id,
                        decision: BuildSelectionDecision::WrongMatch,
                        accepted_identity: None,
                        accepted_total_points: None,
                    }
                } else if !matches!(match_state.phase, MatchPhase::Waiting) {
                    BuildSelectionOutcome {
                        request_id: request.request_id,
                        match_id: request.match_id,
                        decision: BuildSelectionDecision::WrongPhase,
                        accepted_identity: None,
                        accepted_total_points: None,
                    }
                } else if participant.ready {
                    BuildSelectionOutcome {
                        request_id: request.request_id,
                        match_id: request.match_id,
                        decision: BuildSelectionDecision::ReadyLocked,
                        accepted_identity: None,
                        accepted_total_points: None,
                    }
                } else if accepted_fighters_this_tick.contains(&fighter_entity) {
                    BuildSelectionOutcome {
                        request_id: request.request_id,
                        match_id: request.match_id,
                        decision: BuildSelectionDecision::Stale,
                        accepted_identity: None,
                        accepted_total_points: None,
                    }
                } else {
                    let (recipe, source_preset) = match request.selection {
                        crate::protocol::BuildSelection::Preset(id) => builds
                            .0
                            .preset(id)
                            .map_or((None, Some(id)), |preset| (Some(preset.recipe), Some(id))),
                        crate::protocol::BuildSelection::Custom(recipe) => (Some(recipe), None),
                    };
                    let resolved =
                        recipe
                            .ok_or(BuildResolutionError::UnknownId)
                            .and_then(|recipe| {
                                let fighter = definitions
                                    .get(*fighter_definition_id)
                                    .ok_or(BuildResolutionError::ResolutionFailed)?;
                                resolve_build_recipe(
                                    &builds.0,
                                    &weapons.0,
                                    fighter,
                                    recipe,
                                    source_preset,
                                )
                            });
                    match resolved {
                        Ok(resolved) => {
                            accepted_fighters_this_tick.insert(fighter_entity);
                            // Selection acceptance is a hard input epoch boundary. Discard every
                            // buffered native state, the currently applied action, and its cached
                            // watermark so a packet sent before acceptance, including one carrying
                            // a future tick, cannot satisfy the post-selection freshness barrier.
                            if let Some(buffer) = input_buffer.as_mut() {
                                **buffer = NativeBuffer::default();
                            }
                            if let Some(action) = action.as_mut() {
                                **action = ActionState::default();
                            }
                            if let Some(input_freshness) = input_freshness.as_mut() {
                                **input_freshness = crate::movement::InputFreshness::default();
                            }
                            let capacity = resolved.primary_weapon.recipe.economy.capacity();
                            let legacy_preset = resolved.primary_weapon.source_preset_id;
                            for identity in &sentries {
                                if identity.owner_network_id == *fighter_network_id {
                                    sentry_cleanup_requests.write(
                                        crate::abilities::SentryCleanupRequest {
                                            deployable_id: identity.deployable_id,
                                            reason: crate::abilities::SentryCleanupReason::BuildReplaced,
                                            requested_at_tick: tick.0,
                                        },
                                    );
                                }
                            }
                            commands
                                .entity(fighter_entity)
                                .insert((
                                    resolved.identity,
                                    resolved.clone(),
                                    super::AbilityState::default(),
                                    super::PassiveRuntimeState::default(),
                                    crate::combat::CurrentHealth(
                                        resolved.fighter_stats.maximum_health,
                                    ),
                                    WeaponState::ready(capacity),
                                    HealthRecoveryState::starting_at(tick.0),
                                    ActiveEffects::default(),
                                    crate::combat::AwaitingPostSelectionInput {
                                        accepted_at_tick: tick.0,
                                    },
                                    CollisionLayers::new(
                                        crate::movement::FIGHTER_LAYER,
                                        crate::movement::STATIC_MAP_LAYER
                                            | crate::movement::DESTRUCTIBLE_MAP_LAYER,
                                    ),
                                ))
                                .remove::<crate::combat::SelectingBuild>()
                                .remove::<crate::abilities::DashRuntime>()
                                .remove::<crate::abilities::UltimateInputLatch>()
                                .remove::<crate::abilities::UltimateGeneration>()
                                .remove::<crate::concealment::ForcedRevealSources>();
                            if let Some(preset_id) = legacy_preset {
                                telemetry.record_selection(
                                    preset_id,
                                    resolved.primary_weapon.recipe_fingerprint,
                                    tick.0,
                                    request.request_id,
                                );
                            }
                            build_telemetry.record(super::BuildSelectionTelemetryRecord {
                                tick: tick.0,
                                request_id: request.request_id,
                                owner_network_id: *fighter_network_id,
                                identity: resolved.identity,
                                total_points: resolved.total_points,
                                weapon_fingerprint: resolved.primary_weapon.recipe_fingerprint,
                                ultimate_id: resolved.ultimate.id,
                                passive_ids: resolved.passives.map(|passive| passive.id),
                            });
                            let _ = fighter_definition_id;
                            BuildSelectionOutcome {
                                request_id: request.request_id,
                                match_id: request.match_id,
                                decision: BuildSelectionDecision::Accepted,
                                accepted_identity: Some(resolved.identity),
                                accepted_total_points: Some(resolved.total_points),
                            }
                        }
                        Err(error) => {
                            warn!(?error, "build selection resolution failed");
                            BuildSelectionOutcome {
                                request_id: request.request_id,
                                match_id: request.match_id,
                                decision: match error {
                                    BuildResolutionError::UnknownId => {
                                        BuildSelectionDecision::UnknownId
                                    }
                                    BuildResolutionError::InvalidCombination => {
                                        BuildSelectionDecision::InvalidCombination
                                    }
                                    BuildResolutionError::OverBudget => {
                                        BuildSelectionDecision::OverBudget
                                    }
                                    BuildResolutionError::CandidateTooLarge => {
                                        BuildSelectionDecision::CandidateTooLarge
                                    }
                                    BuildResolutionError::ResolutionFailed => {
                                        BuildSelectionDecision::ResolutionFailed
                                    }
                                },
                                accepted_identity: None,
                                accepted_total_points: None,
                            }
                        }
                    }
                }
            } else {
                BuildSelectionOutcome {
                    request_id: request.request_id,
                    match_id: request.match_id,
                    decision: BuildSelectionDecision::ResolutionFailed,
                    accepted_identity: None,
                    accepted_total_points: None,
                }
            };
            session.last_selection_request = Some(request);
            session.last_selection_outcome = Some(outcome);
            session.last_selection_response = Some(outcome);
            sender.send::<SessionChannel>(outcome);
        }
    }
}
