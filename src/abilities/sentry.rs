use crate::protocol::NetworkEntityId;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub const SENTRY_PLACEMENT_OFFSETS: [u16; 6] = [96, 88, 80, 72, 64, 56];
pub const SENTRY_RADIUS: f32 = 20.0;
pub const SENTRY_ACQUISITION_RANGE: f32 = 480.0;
pub const SENTRY_ACQUISITION_INTERVAL_TICKS: u64 = 6;
pub const SENTRY_FIRE_INTERVAL_TICKS: u64 = 30;
pub const SENTRY_LIFETIME_TICKS: u64 = 720;
pub const SENTRY_MAXIMUM_HEALTH: u16 = 80;

#[must_use]
pub fn first_clear_sentry_placement(
    origin: Vec2,
    facing: Vec2,
    mut is_clear: impl FnMut(Vec2, f32) -> bool,
) -> Option<Vec2> {
    if !origin.is_finite() {
        return None;
    }
    let facing = facing.try_normalize()?;
    SENTRY_PLACEMENT_OFFSETS
        .into_iter()
        .map(|offset| origin + facing * f32::from(offset))
        .find(|candidate| is_clear(*candidate, SENTRY_RADIUS))
}

#[must_use]
pub fn stable_sentry_target(
    candidates: impl IntoIterator<Item = (NetworkEntityId, f32, bool)>,
) -> Option<NetworkEntityId> {
    let mut candidates: Vec<_> = candidates
        .into_iter()
        .filter(|(_, distance_squared, visible)| {
            *visible
                && distance_squared.is_finite()
                && *distance_squared <= SENTRY_ACQUISITION_RANGE.powi(2)
        })
        .collect();
    candidates.sort_by(|left, right| {
        left.1
            .total_cmp(&right.1)
            .then_with(|| left.0.0.cmp(&right.0.0))
    });
    candidates.first().map(|candidate| candidate.0)
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sentry;

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SentryIdentity {
    pub deployable_id: crate::builds::DeployableId,
    pub owner_player_id: crate::protocol::PlayerId,
    pub owner_network_id: NetworkEntityId,
    pub team_id: crate::combat::TeamId,
    pub ultimate_id: crate::builds::UltimateDefinitionId,
    pub match_id: crate::matchplay::MatchId,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SentryDeadline {
    pub expires_at_tick: u64,
}

#[cfg(feature = "server")]
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SentryCleanupRequest {
    pub deployable_id: crate::builds::DeployableId,
    pub reason: crate::abilities::SentryCleanupReason,
    pub requested_at_tick: u64,
}

#[cfg(feature = "server")]
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SentryRuntime {
    next_acquire_tick: u64,
    next_fire_tick: u64,
    target: Option<NetworkEntityId>,
}

#[cfg(feature = "server")]
impl SentryRuntime {
    pub(super) fn new(activated_at_tick: u64) -> Self {
        Self {
            next_acquire_tick: activated_at_tick.saturating_add(SENTRY_ACQUISITION_INTERVAL_TICKS),
            next_fire_tick: activated_at_tick.saturating_add(SENTRY_FIRE_INTERVAL_TICKS),
            target: None,
        }
    }

    pub(super) fn begin_acquisition_if_due(&mut self, tick: u64) -> bool {
        if tick < self.next_acquire_tick {
            return false;
        }
        self.next_acquire_tick = tick.saturating_add(SENTRY_ACQUISITION_INTERVAL_TICKS);
        true
    }

    pub(super) fn fire_is_due(&self, tick: u64) -> bool {
        tick >= self.next_fire_tick
    }

    pub(super) fn record_fire(&mut self, tick: u64) {
        self.next_fire_tick = tick.saturating_add(SENTRY_FIRE_INTERVAL_TICKS);
    }

    pub(super) fn target(&self) -> Option<NetworkEntityId> {
        self.target
    }

    pub(super) fn set_target(&mut self, target: Option<NetworkEntityId>) {
        self.target = target;
    }
}

#[cfg(feature = "server")]
#[derive(Resource, Debug)]
pub(crate) struct NextDeployableId(pub(super) u64);

#[cfg(feature = "server")]
impl Default for NextDeployableId {
    fn default() -> Self {
        Self(1)
    }
}

#[cfg(feature = "server")]
impl NextDeployableId {
    pub(super) fn allocate(&mut self) -> Option<crate::builds::DeployableId> {
        if self.0 == 0 {
            return None;
        }
        let id = crate::builds::DeployableId(self.0);
        self.0 = self.0.checked_add(1).unwrap_or(0);
        Some(id)
    }
}

#[cfg(feature = "server")]
#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity
)]
pub(crate) fn activate_sentry(
    mut commands: Commands,
    tick: Res<crate::timing::SimulationTick>,
    spatial_query: avian2d::prelude::SpatialQuery,
    bounds: Res<crate::map::PlayableBounds>,
    mut next_id: ResMut<NextDeployableId>,
    mut telemetry: ResMut<crate::abilities::AbilityTelemetry>,
    mut fighters: Query<
        (
            Entity,
            &avian2d::prelude::Position,
            &avian2d::prelude::Rotation,
            &crate::builds::ResolvedMatchLoadout,
            &crate::protocol::PlayerId,
            &NetworkEntityId,
            &crate::combat::TeamId,
            &crate::matchplay::MatchParticipant,
            &crate::movement::InputFreshness,
            &mut crate::builds::AbilityState,
            Option<&lightyear::prelude::input::native::ActionState<crate::protocol::FighterInput>>,
            Option<&mut crate::abilities::UltimateInputLatch>,
            Option<&crate::combat::Defeated>,
            Option<&crate::matchplay::ActiveCombatant>,
            Option<&crate::combat::AwaitingPostSelectionInput>,
        ),
        With<crate::protocol::Fighter>,
    >,
    existing: Query<&SentryIdentity, With<Sentry>>,
    defeated_fighters: Query<
        Entity,
        (
            With<crate::protocol::Fighter>,
            With<crate::combat::Defeated>,
        ),
    >,
) {
    use avian2d::prelude::{Collider, CollisionLayers, RigidBody, Sensor, SpatialQueryFilter};
    use lightyear::prelude::{NetworkTarget, Replicate};
    for (
        owner,
        position,
        rotation,
        loadout,
        player,
        network_id,
        team,
        participant,
        freshness,
        mut ability,
        action,
        latch,
        defeated,
        active,
        activation_barrier,
    ) in &mut fighters
    {
        if loadout.ultimate.kind != crate::builds::UltimateKind::Sentry {
            continue;
        }
        let requested = action.is_some_and(|action| {
            action.0.is_valid()
                && action.0.gameplay_buttons & crate::protocol::FighterInput::ULTIMATE != 0
        });
        let held = requested
            && activation_barrier.is_none()
            && !crate::movement::input_should_neutralize(tick.0, freshness.last_fresh_tick, 12);
        let was_held = latch.as_deref().is_some_and(|latch| latch.0);
        if let Some(mut latch) = latch {
            latch.0 = requested;
        } else {
            commands
                .entity(owner)
                .insert(crate::abilities::UltimateInputLatch(requested));
        }
        if requested && !was_held {
            telemetry.record(crate::abilities::AbilityTelemetryRecord {
                tick: tick.0,
                owner_network_id: *network_id,
                kind: crate::abilities::AbilityTelemetryKind::ActivationAttempt,
            });
        }
        if !requested || was_held {
            continue;
        }
        let rejection = if !held {
            Some(crate::abilities::AbilityRejectionReason::StaleInput)
        } else if defeated.is_some() {
            Some(crate::abilities::AbilityRejectionReason::Defeated)
        } else if active.is_none() {
            Some(crate::abilities::AbilityRejectionReason::Inactive)
        } else if matches!(
            ability.phase,
            crate::builds::AbilityPhase::Dashing { .. }
                | crate::builds::AbilityPhase::Deployed { .. }
        ) {
            Some(crate::abilities::AbilityRejectionReason::AlreadyExecuting)
        } else if ability.charge != crate::abilities::ULTIMATE_CHARGE_MAX
            || !matches!(ability.phase, crate::builds::AbilityPhase::Ready)
        {
            Some(crate::abilities::AbilityRejectionReason::NotCharged)
        } else if existing
            .iter()
            .any(|identity| identity.owner_network_id == *network_id)
        {
            Some(crate::abilities::AbilityRejectionReason::ExistingSentry)
        } else {
            None
        };
        if let Some(reason) = rejection {
            telemetry.record(crate::abilities::AbilityTelemetryRecord {
                tick: tick.0,
                owner_network_id: *network_id,
                kind: crate::abilities::AbilityTelemetryKind::ActivationRejected(reason),
            });
            continue;
        }
        let facing = Vec2::from_angle(rotation.as_radians());
        let mut excluded = Vec::with_capacity(1 + defeated_fighters.iter().len());
        excluded.push(owner);
        excluded.extend(defeated_fighters.iter());
        let filter = SpatialQueryFilter::from_mask(
            crate::movement::FIGHTER_LAYER
                | crate::movement::DEPLOYABLE_LAYER
                | crate::movement::INDESTRUCTIBLE_TERRAIN_LAYER
                | crate::movement::DESTRUCTIBLE_TERRAIN_LAYER,
        )
        .with_excluded_entities(excluded);
        let placement = first_clear_sentry_placement(position.0, facing, |candidate, radius| {
            bounds.0.contains_with_inset(candidate, radius)
                && spatial_query
                    .shape_intersections(&Collider::circle(radius), candidate, 0.0, &filter)
                    .is_empty()
        });
        let Some(placement) = placement else {
            telemetry.record(crate::abilities::AbilityTelemetryRecord {
                tick: tick.0,
                owner_network_id: *network_id,
                kind: crate::abilities::AbilityTelemetryKind::ActivationRejected(
                    crate::abilities::AbilityRejectionReason::PlacementBlocked,
                ),
            });
            continue;
        };
        let Some(deployable_id) = next_id.allocate() else {
            telemetry.record(crate::abilities::AbilityTelemetryRecord {
                tick: tick.0,
                owner_network_id: *network_id,
                kind: crate::abilities::AbilityTelemetryKind::ActivationRejected(
                    crate::abilities::AbilityRejectionReason::IdentifierExhausted,
                ),
            });
            continue;
        };
        let expires_at_tick = tick.0.saturating_add(SENTRY_LIFETIME_TICKS);
        let identity = SentryIdentity {
            deployable_id,
            owner_player_id: *player,
            owner_network_id: *network_id,
            team_id: *team,
            ultimate_id: loadout.ultimate.id,
            match_id: participant.match_id,
        };
        commands.spawn((
            Sentry,
            identity,
            SentryDeadline { expires_at_tick },
            SentryRuntime::new(tick.0),
            crate::combat::CurrentHealth(SENTRY_MAXIMUM_HEALTH),
            NetworkEntityId((1_u64 << 63) | deployable_id.0),
            crate::combat::TeamId(team.0),
            crate::matchplay::MatchMember(participant.match_id),
            avian2d::prelude::Position::from_xy(placement.x, placement.y),
            avian2d::prelude::Rotation::radians(rotation.as_radians()),
            Collider::circle(SENTRY_RADIUS),
            Sensor,
            RigidBody::Static,
            CollisionLayers::new(
                crate::movement::DEPLOYABLE_LAYER,
                crate::movement::PROJECTILE_LAYER
                    | crate::movement::FIGHTER_LAYER
                    | crate::movement::DEPLOYABLE_LAYER,
            ),
            Replicate::to_clients(NetworkTarget::All),
        ));
        *ability = crate::builds::AbilityState {
            charge: 0,
            phase: crate::builds::AbilityPhase::Deployed {
                deployable_id,
                expires_at_tick,
            },
        };
        commands
            .entity(owner)
            .remove::<crate::matchplay::SpawnProtection>();
        telemetry.record(crate::abilities::AbilityTelemetryRecord {
            tick: tick.0,
            owner_network_id: *network_id,
            kind: crate::abilities::AbilityTelemetryKind::SentryAccepted,
        });
        telemetry.record(crate::abilities::AbilityTelemetryRecord {
            tick: tick.0,
            owner_network_id: *network_id,
            kind: crate::abilities::AbilityTelemetryKind::SentrySpawned(deployable_id),
        });
    }
}

#[cfg(feature = "server")]
#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity
)]
pub(crate) fn request_sentry_lifecycle_cleanup(
    tick: Res<crate::timing::SimulationTick>,
    roots: Query<&crate::matchplay::MatchState, With<crate::matchplay::MatchRoot>>,
    sentries: Query<
        (
            &SentryIdentity,
            &SentryDeadline,
            Option<&crate::combat::Defeated>,
        ),
        With<Sentry>,
    >,
    owners: Query<(
        &NetworkEntityId,
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
    mut requests: MessageWriter<SentryCleanupRequest>,
) {
    let root = roots.single().ok();
    for (identity, deadline, destroyed) in &sentries {
        let reason = if destroyed.is_some() {
            Some(crate::abilities::SentryCleanupReason::Destroyed)
        } else if tick.0 >= deadline.expires_at_tick {
            Some(crate::abilities::SentryCleanupReason::Expired)
        } else if root.is_some_and(|root| root.match_id != identity.match_id) {
            Some(crate::abilities::SentryCleanupReason::MatchRestarted)
        } else if root.is_some_and(|root| {
            matches!(root.phase, crate::matchplay::MatchPhase::Completed { .. })
        }) {
            Some(crate::abilities::SentryCleanupReason::MatchCompleted)
        } else {
            match owners
                .iter()
                .find(|(owner, ..)| **owner == identity.owner_network_id)
            {
                Some((_, defeated, active, controlled)) => {
                    if controlled.is_some_and(|controlled| disconnected.contains(controlled.owner))
                    {
                        Some(crate::abilities::SentryCleanupReason::OwnerDisconnected)
                    } else if defeated.is_some() {
                        Some(crate::abilities::SentryCleanupReason::OwnerDefeated)
                    } else if active.is_none() {
                        Some(crate::abilities::SentryCleanupReason::OwnerDisconnected)
                    } else {
                        None
                    }
                }
                None => Some(crate::abilities::SentryCleanupReason::OwnerDisconnected),
            }
        };
        if let Some(reason) = reason {
            requests.write(SentryCleanupRequest {
                deployable_id: identity.deployable_id,
                reason,
                requested_at_tick: tick.0,
            });
        }
    }
}

#[cfg(feature = "server")]
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn cleanup_requested_sentries(
    mut commands: Commands,
    mut requests: MessageReader<SentryCleanupRequest>,
    mut ids: ResMut<crate::combat::NextCombatIds>,
    mut telemetry: ResMut<crate::abilities::AbilityTelemetry>,
    mut combat_telemetry: ResMut<crate::combat::CombatTelemetry>,
    mut outbox: ResMut<crate::combat::CombatOutbox>,
    sentries: Query<
        (
            Entity,
            &SentryIdentity,
            Option<&SentryDeadline>,
            Option<&avian2d::prelude::Position>,
        ),
        With<Sentry>,
    >,
    deliveries: Query<(Entity, &crate::combat::ReplicatedAttackSource)>,
    mut pending_payloads: ResMut<Messages<crate::combat::PendingPayload>>,
    mut pending_deliveries: ResMut<Messages<crate::combat::PendingDelivery>>,
    mut melee_attacks: ResMut<Messages<crate::combat::MeleeAttack>>,
    mut owners: Query<(
        &NetworkEntityId,
        &avian2d::prelude::Position,
        &mut crate::builds::AbilityState,
    )>,
) {
    let mut selected = std::collections::BTreeMap::new();
    for request in requests.read().copied() {
        selected
            .entry(request.deployable_id)
            .and_modify(|current: &mut SentryCleanupRequest| {
                if cleanup_reason_priority(request.reason) < cleanup_reason_priority(current.reason)
                {
                    *current = request;
                }
            })
            .or_insert(request);
    }
    for request in selected.into_values() {
        let Some((entity, identity, deadline, position)) = sentries
            .iter()
            .find(|(_, identity, _, _)| identity.deployable_id == request.deployable_id)
        else {
            continue;
        };
        let identity = *identity;
        let mut cleanup_position = position.map(|position| position.0);
        let lifetime_ticks = deadline.map_or(0, |deadline| {
            let activated_at_tick = deadline
                .expires_at_tick
                .saturating_sub(SENTRY_LIFETIME_TICKS);
            request
                .requested_at_tick
                .saturating_sub(activated_at_tick)
                .min(SENTRY_LIFETIME_TICKS)
        });
        // Match teardown may queue the same removal earlier in the schedule. Cleanup remains the
        // single ability transaction, but tolerates that external ownership teardown race.
        commands.entity(entity).try_despawn();
        despawn_sentry_deliveries(
            &mut commands,
            identity.owner_network_id,
            identity.deployable_id,
            &deliveries,
        );
        retain_non_sentry_messages(
            &mut pending_payloads,
            identity.owner_network_id,
            identity.deployable_id,
            |message| message.source,
        );
        retain_non_sentry_messages(
            &mut pending_deliveries,
            identity.owner_network_id,
            identity.deployable_id,
            |message| message.source,
        );
        retain_non_sentry_messages(
            &mut melee_attacks,
            identity.owner_network_id,
            identity.deployable_id,
            |message| message.source,
        );
        for (owner, owner_position, mut ability) in &mut owners {
            if *owner == identity.owner_network_id
                && matches!(
                    ability.phase,
                    crate::builds::AbilityPhase::Deployed { deployable_id, .. }
                        if deployable_id == identity.deployable_id
                )
            {
                ability.phase = crate::abilities::settled_ability_phase(ability.charge);
            }
            if *owner == identity.owner_network_id && cleanup_position.is_none() {
                cleanup_position = Some(owner_position.0);
            }
        }
        telemetry.record(crate::abilities::AbilityTelemetryRecord {
            tick: request.requested_at_tick,
            owner_network_id: identity.owner_network_id,
            kind: crate::abilities::AbilityTelemetryKind::SentryCleanup {
                deployable_id: identity.deployable_id,
                reason: request.reason,
                lifetime_ticks,
            },
        });
        if let Some(cleanup_position) = cleanup_position
            && let Some(event_id) = ids.allocate_event()
        {
            let cleanup_cue = crate::combat::CombatCue::DeployableRemoved {
                event_id,
                tick: request.requested_at_tick,
                owner: identity.owner_network_id,
                deployable_id: identity.deployable_id,
                position: crate::combat::WorldPoint::from(cleanup_position),
                reason: request.reason,
            };
            combat_telemetry.record_cue(cleanup_cue.clone());
            outbox.0.push(cleanup_cue);
        }
    }
}

#[cfg(feature = "server")]
fn retain_non_sentry_messages<M: Message>(
    messages: &mut Messages<M>,
    owner_network_id: NetworkEntityId,
    deployable_id: crate::builds::DeployableId,
    source: impl Fn(&M) -> crate::combat::AttackSource,
) {
    let retained: Vec<_> = messages
        .drain()
        .filter(|message| {
            let source = source(message);
            source.owner_network_entity_id != owner_network_id
                || !matches!(
                    source.kind,
                    crate::combat::CombatSourceKind::Deployable {
                        deployable_id: candidate,
                        ..
                    } if candidate == deployable_id
                )
        })
        .collect();
    messages.extend(retained);
}

#[cfg(feature = "server")]
const fn cleanup_reason_priority(reason: crate::abilities::SentryCleanupReason) -> u8 {
    match reason {
        crate::abilities::SentryCleanupReason::Destroyed => 0,
        crate::abilities::SentryCleanupReason::Expired => 1,
        crate::abilities::SentryCleanupReason::BuildReplaced => 2,
        crate::abilities::SentryCleanupReason::OwnerDefeated => 3,
        crate::abilities::SentryCleanupReason::OwnerDisconnected => 4,
        crate::abilities::SentryCleanupReason::MatchCompleted => 5,
        crate::abilities::SentryCleanupReason::MatchRestarted => 6,
    }
}

#[cfg(feature = "server")]
#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity
)]
pub(crate) fn tick_sentries(
    mut commands: Commands,
    tick: Res<crate::timing::SimulationTick>,
    spatial_query: avian2d::prelude::SpatialQuery,
    mut ids: ResMut<crate::combat::NextCombatIds>,
    mut telemetry: ResMut<crate::abilities::AbilityTelemetry>,
    mut combat_telemetry: ResMut<crate::combat::CombatTelemetry>,
    mut outbox: ResMut<crate::combat::CombatOutbox>,
    mut sentries: Query<
        (
            Entity,
            &avian2d::prelude::Position,
            &SentryIdentity,
            &mut SentryRuntime,
        ),
        With<Sentry>,
    >,
    fighters: Query<
        (
            &avian2d::prelude::Position,
            &NetworkEntityId,
            &crate::combat::TeamId,
            Option<&crate::combat::Defeated>,
            Option<&crate::matchplay::ActiveCombatant>,
        ),
        With<crate::protocol::Fighter>,
    >,
    mut owners: Query<(Entity, &NetworkEntityId), With<crate::protocol::Fighter>>,
) {
    use avian2d::prelude::{Collider, CollisionLayers};
    use bevy::math::Dir2;
    use lightyear::prelude::{InterpolationTarget, NetworkTarget, Replicate};
    for (entity, position, identity, mut runtime) in &mut sentries {
        if runtime.begin_acquisition_if_due(tick.0) {
            runtime.set_target(stable_sentry_target(fighters.iter().filter_map(
                |(target_position, target_id, team, defeated, active)| {
                    if *team == identity.team_id || defeated.is_some() || active.is_none() {
                        return None;
                    }
                    let delta = target_position.0 - position.0;
                    let distance_squared = delta.length_squared();
                    let visible =
                        Dir2::new(delta.normalize_or_zero())
                            .ok()
                            .is_some_and(|direction| {
                                spatial_query
                                    .cast_ray(
                                        position.0,
                                        direction,
                                        delta.length(),
                                        true,
                                        &avian2d::prelude::SpatialQueryFilter::from_mask(
                                            crate::movement::INDESTRUCTIBLE_TERRAIN_LAYER
                                                | crate::movement::DESTRUCTIBLE_TERRAIN_LAYER,
                                        ),
                                    )
                                    .is_none()
                            });
                    Some((*target_id, distance_squared, visible))
                },
            )));
        }
        if !runtime.fire_is_due(tick.0) {
            continue;
        }
        let Some(target_id) = runtime.target() else {
            continue;
        };
        let Some((target_position, _, _, _, _)) =
            fighters.iter().find(|(_, id, _, defeated, active)| {
                **id == target_id && defeated.is_none() && active.is_some()
            })
        else {
            continue;
        };
        let delta = target_position.0 - position.0;
        let Some(direction) = delta.try_normalize() else {
            continue;
        };
        let Some(attack_id) = ids.allocate_attack() else {
            continue;
        };
        let mut fighter_owner = None;
        for (owner_entity, owner_id) in &mut owners {
            if *owner_id == identity.owner_network_id {
                fighter_owner = Some(owner_entity);
                commands
                    .entity(owner_entity)
                    .remove::<crate::matchplay::SpawnProtection>();
            }
        }
        let Some(fighter_owner) = fighter_owner else {
            continue;
        };
        let Some(fire_event_id) = ids.allocate_event() else {
            continue;
        };
        runtime.record_fire(tick.0);
        telemetry.record(crate::abilities::AbilityTelemetryRecord {
            tick: tick.0,
            owner_network_id: identity.owner_network_id,
            kind: crate::abilities::AbilityTelemetryKind::SentryShot(identity.deployable_id),
        });
        let fire_cue = crate::combat::CombatCue::SentryFired {
            event_id: fire_event_id,
            tick: tick.0,
            owner: identity.owner_network_id,
            deployable_id: identity.deployable_id,
            target: target_id,
            position: crate::combat::WorldPoint::from(position.0),
            presentation_profile_id: crate::combat::WeaponPresentationProfileId(1),
        };
        combat_telemetry.record_cue(fire_cue.clone());
        outbox.0.push(fire_cue);
        let source = crate::combat::AttackSource {
            kind: crate::combat::CombatSourceKind::Deployable {
                ultimate_id: identity.ultimate_id,
                deployable_id: identity.deployable_id,
            },
            attack_id,
            player_id: identity.owner_player_id,
            owner_network_entity_id: identity.owner_network_id,
            team_id: identity.team_id,
            recipe_fingerprint: crate::combat::WeaponRecipeFingerprint(0),
            presentation_profile_id: crate::combat::WeaponPresentationProfileId(1),
            legacy_compatibility: false,
            source_preset_id: None,
            origin: crate::combat::WorldPoint::from(position.0),
            facing: direction.y.atan2(direction.x),
        };
        let recipe = crate::combat::WeaponRecipe {
            economy: crate::combat::WeaponEconomy::Magazine {
                capacity: 1,
                refill_ticks: SENTRY_FIRE_INTERVAL_TICKS,
            },
            fire_cooldown_ticks: SENTRY_FIRE_INTERVAL_TICKS,
            firing: crate::combat::FiringPattern::Single,
            delivery: crate::combat::DeliveryMethod::Straight {
                speed: 900.0,
                radius: 6.0,
                range: SENTRY_ACQUISITION_RANGE,
                lifetime_ticks: 32,
                muzzle_offset: 0.0,
            },
            payload_bundles: vec![crate::combat::PayloadBundleDefinition {
                target: crate::combat::TargetSelection::Direct,
                effects: vec![crate::combat::PayloadEffectDefinition::Damage {
                    amount: 10,
                    falloff: crate::combat::DamageFalloff::None,
                    recipients: crate::combat::RecipientPolicy::Hostiles,
                }],
            }],
            world_effects: Vec::new(),
        };
        commands.spawn((
            crate::combat::Projectile,
            crate::combat::ProjectileSource {
                shot_id: crate::combat::ShotId(attack_id.0),
                player_id: identity.owner_player_id,
                owner_network_entity_id: identity.owner_network_id,
                team_id: identity.team_id,
                weapon_definition_id: crate::combat::WeaponDefinitionId(1),
            },
            crate::combat::ReplicatedAttackSource { attack: source },
            crate::combat::AttackDelivery {
                attack_id,
                delivery_index: 0,
            },
            crate::combat::ProjectileDeadline {
                expires_at_tick: tick.0.saturating_add(32),
            },
            crate::combat::StraightFlight {
                origin: crate::combat::WorldPoint::from(position.0),
                facing: source.facing,
                speed: 900.0,
                maximum_range: SENTRY_ACQUISITION_RANGE,
                launched_at_tick: tick.0,
            },
            crate::combat::ComposedProjectileRuntime {
                owner_entity: fighter_owner,
                source_entity: entity,
                source,
                delivery_index: 0,
                velocity: direction * 900.0,
                travelled: 0.0,
                expires_at_tick: tick.0.saturating_add(32),
                maximum_range: 480.0,
                radius: 6.0,
                landing: None,
                recipe,
            },
            avian2d::prelude::Position::from_xy(position.x, position.y),
            avian2d::prelude::Rotation::radians(source.facing),
            Collider::circle(6.0),
            CollisionLayers::new(
                crate::movement::PROJECTILE_LAYER,
                crate::movement::FIGHTER_LAYER
                    | crate::movement::DEPLOYABLE_LAYER
                    | crate::movement::INDESTRUCTIBLE_TERRAIN_LAYER
                    | crate::movement::DESTRUCTIBLE_TERRAIN_LAYER,
            ),
            crate::matchplay::MatchMember(identity.match_id),
            Replicate::to_clients(NetworkTarget::All),
            InterpolationTarget::to_clients(NetworkTarget::All),
        ));
    }
}

#[cfg(feature = "server")]
pub(crate) fn despawn_sentry_deliveries(
    commands: &mut Commands,
    owner_network_id: NetworkEntityId,
    deployable_id: crate::builds::DeployableId,
    deliveries: &Query<(Entity, &crate::combat::ReplicatedAttackSource)>,
) {
    for (entity, source) in deliveries {
        if source.attack.owner_network_entity_id == owner_network_id
            && matches!(
                source.attack.kind,
                crate::combat::CombatSourceKind::Deployable {
                    deployable_id: candidate,
                    ..
                } if candidate == deployable_id
            )
        {
            commands.entity(entity).try_despawn();
        }
    }
}
