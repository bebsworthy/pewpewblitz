use crate::protocol::NetworkEntityId;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[must_use]
pub fn first_clear_sentry_placement(
    origin: Vec2,
    facing: Vec2,
    placement_offsets: &[u32],
    body_radius: f32,
    mut is_clear: impl FnMut(Vec2, f32) -> bool,
) -> Option<Vec2> {
    if !origin.is_finite() {
        return None;
    }
    let facing = facing.try_normalize()?;
    placement_offsets
        .iter()
        .filter_map(|offset| crate::builds::world_units_from_milliunits(*offset))
        .map(|offset| origin + facing * offset)
        .find(|candidate| is_clear(*candidate, body_radius))
}

#[must_use]
pub fn stable_sentry_target(
    candidates: impl IntoIterator<Item = (NetworkEntityId, f32, bool)>,
    acquisition_range: f32,
) -> Option<NetworkEntityId> {
    let mut candidates: Vec<_> = candidates
        .into_iter()
        .filter(|(_, distance_squared, visible)| {
            *visible
                && distance_squared.is_finite()
                && *distance_squared <= acquisition_range.powi(2)
        })
        .collect();
    candidates.sort_by(|left, right| {
        left.1
            .total_cmp(&right.1)
            .then_with(|| left.0.0.cmp(&right.0.0))
    });
    candidates.first().map(|candidate| candidate.0)
}

#[cfg(feature = "server")]
#[must_use]
pub(crate) fn stable_sentry_objective_target(
    candidates: impl IntoIterator<Item = (crate::map::DamageableTargetIdentity, f32, bool)>,
    acquisition_range: f32,
) -> Option<crate::map::DamageableTargetIdentity> {
    let mut candidates: Vec<_> = candidates
        .into_iter()
        .filter(|(_, distance_squared, visible)| {
            *visible
                && distance_squared.is_finite()
                && *distance_squared <= acquisition_range.powi(2)
        })
        .collect();
    candidates.sort_by(|left, right| {
        left.1
            .total_cmp(&right.1)
            .then_with(|| left.0.stable_order_key().cmp(&right.0.stable_order_key()))
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
#[derive(Component, Clone, Debug, PartialEq)]
pub(crate) struct ResolvedSentryTuning {
    placement_offsets_milliunits: [u32; 6],
    pub(crate) body_radius: f32,
    acquisition_range: f32,
    acquisition_interval_ticks: u64,
    fire_interval_ticks: u64,
    lifetime_ticks: u64,
    maximum_health: u16,
    charge_maximum: u16,
    recipe_fingerprint: crate::combat::WeaponRecipeFingerprint,
    presentation_profile_id: crate::combat::WeaponPresentationProfileId,
    recipe: crate::combat::WeaponRecipe,
}

#[cfg(feature = "server")]
fn resolve_sentry_tuning(
    ultimate: &crate::builds::ResolvedUltimate,
) -> Option<ResolvedSentryTuning> {
    let crate::builds::UltimateParameters::Sentry {
        placement_offsets_milliunits,
        body_radius_milliunits,
        acquisition_range_milliunits,
        acquisition_interval_ticks,
        fire_interval_ticks,
        lifetime_ticks,
        maximum_health,
        projectile_speed_milliunits,
        projectile_radius_milliunits,
        projectile_range_milliunits,
        projectile_lifetime_ticks,
        projectile_damage,
        presentation_profile_id,
    } = ultimate.parameters
    else {
        return None;
    };
    let acquisition_range =
        crate::builds::world_units_from_milliunits(acquisition_range_milliunits)?;
    let recipe = crate::combat::WeaponRecipe {
        economy: crate::combat::WeaponEconomy::Magazine {
            capacity: 1,
            refill_ticks: fire_interval_ticks,
        },
        fire_cooldown_ticks: fire_interval_ticks,
        firing: crate::combat::FiringPattern::Single,
        delivery: crate::combat::DeliveryMethod::Straight {
            speed: crate::builds::world_units_from_milliunits(projectile_speed_milliunits)?,
            radius: crate::builds::world_units_from_milliunits(projectile_radius_milliunits)?,
            range: crate::builds::world_units_from_milliunits(projectile_range_milliunits)?,
            lifetime_ticks: projectile_lifetime_ticks,
            muzzle_offset: 0.0,
        },
        payload_bundles: vec![crate::combat::PayloadBundleDefinition {
            target: crate::combat::TargetSelection::Direct,
            effects: vec![crate::combat::PayloadEffectDefinition::Damage {
                amount: projectile_damage,
                falloff: crate::combat::DamageFalloff::None,
                recipients: crate::combat::RecipientPolicy::Hostiles,
            }],
        }],
        world_effects: Vec::new(),
    };
    let bytes = postcard::to_allocvec(&(
        crate::combat::definitions::FINGERPRINT_FORMAT_VERSION,
        &recipe,
    ))
    .ok()?;
    let fingerprint = crate::content::fnv1a64(&bytes);
    Some(ResolvedSentryTuning {
        placement_offsets_milliunits,
        body_radius: crate::builds::world_units_from_milliunits(body_radius_milliunits)?,
        acquisition_range,
        acquisition_interval_ticks,
        fire_interval_ticks,
        lifetime_ticks,
        maximum_health,
        charge_maximum: ultimate.charge_policy.maximum,
        recipe_fingerprint: crate::combat::WeaponRecipeFingerprint(fingerprint.max(1)),
        presentation_profile_id: crate::combat::WeaponPresentationProfileId(
            presentation_profile_id,
        ),
        recipe,
    })
}

#[cfg(all(test, feature = "server"))]
pub(super) fn resolve_sentry_tuning_for_test(
    ultimate: &crate::builds::ResolvedUltimate,
) -> Option<ResolvedSentryTuning> {
    resolve_sentry_tuning(ultimate)
}

#[cfg(feature = "server")]
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SentryCleanupRequest {
    pub deployable_id: crate::builds::DeployableId,
    pub reason: crate::abilities::SentryCleanupReason,
    pub requested_at_tick: u64,
}

#[cfg(feature = "server")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SentryTarget {
    Fighter(NetworkEntityId),
    ModeObjective(crate::map::DamageableTargetIdentity),
}

#[cfg(feature = "server")]
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SentryRuntime {
    next_acquire_tick: u64,
    next_fire_tick: u64,
    target: Option<SentryTarget>,
}

#[cfg(feature = "server")]
impl SentryRuntime {
    pub(super) fn new(
        activated_at_tick: u64,
        acquisition_interval_ticks: u64,
        fire_interval_ticks: u64,
    ) -> Self {
        Self {
            next_acquire_tick: activated_at_tick.saturating_add(acquisition_interval_ticks),
            next_fire_tick: activated_at_tick.saturating_add(fire_interval_ticks),
            target: None,
        }
    }

    pub(super) fn begin_acquisition_if_due(&mut self, tick: u64, interval_ticks: u64) -> bool {
        if tick < self.next_acquire_tick {
            return false;
        }
        self.next_acquire_tick = tick.saturating_add(interval_ticks);
        true
    }

    pub(super) fn fire_is_due(&self, tick: u64) -> bool {
        tick >= self.next_fire_tick
    }

    pub(super) fn record_fire(&mut self, tick: u64, interval_ticks: u64) {
        self.next_fire_tick = tick.saturating_add(interval_ticks);
    }

    pub(super) fn target(&self) -> Option<SentryTarget> {
        self.target
    }

    pub(super) fn set_fighter_target(&mut self, target: Option<NetworkEntityId>) {
        self.target = target.map(SentryTarget::Fighter);
    }

    fn set_objective_target(&mut self, target: Option<crate::map::DamageableTargetIdentity>) {
        self.target = target.map(SentryTarget::ModeObjective);
    }

    fn clear_target(&mut self) {
        self.target = None;
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
#[derive(Clone, Copy, Debug)]
struct SentryOwnerLifecycleView {
    network_id: NetworkEntityId,
    defeated: bool,
    active: bool,
    controller_disconnected: bool,
}

#[cfg(feature = "server")]
#[allow(clippy::type_complexity)]
fn index_sentry_owner_lifecycle(
    owners: &Query<(
        &NetworkEntityId,
        Option<&crate::combat::Defeated>,
        Option<&crate::matchplay::ActiveCombatant>,
        Option<&lightyear::prelude::ControlledBy>,
    )>,
    disconnected: &Query<
        Entity,
        (
            With<lightyear::prelude::LinkOf>,
            With<lightyear::prelude::Disconnected>,
        ),
    >,
) -> Vec<SentryOwnerLifecycleView> {
    owners
        .iter()
        .map(
            |(network_id, defeated, active, controlled)| SentryOwnerLifecycleView {
                network_id: *network_id,
                defeated: defeated.is_some(),
                active: active.is_some(),
                controller_disconnected: controlled
                    .is_some_and(|controlled| disconnected.contains(controlled.owner)),
            },
        )
        .collect()
}

#[cfg(feature = "server")]
fn sentry_cleanup_reason(
    tick: u64,
    root: Option<&crate::matchplay::MatchState>,
    identity: SentryIdentity,
    deadline: SentryDeadline,
    destroyed: bool,
    owners: &[SentryOwnerLifecycleView],
) -> Option<crate::abilities::SentryCleanupReason> {
    if destroyed {
        return Some(crate::abilities::SentryCleanupReason::Destroyed);
    }
    if tick >= deadline.expires_at_tick {
        return Some(crate::abilities::SentryCleanupReason::Expired);
    }
    if root.is_some_and(|root| root.match_id != identity.match_id) {
        return Some(crate::abilities::SentryCleanupReason::MatchRestarted);
    }
    if root.is_some_and(|root| matches!(root.phase, crate::matchplay::MatchPhase::Completed { .. }))
    {
        return Some(crate::abilities::SentryCleanupReason::MatchCompleted);
    }
    let Some(owner) = owners
        .iter()
        .find(|owner| owner.network_id == identity.owner_network_id)
    else {
        return Some(crate::abilities::SentryCleanupReason::OwnerDisconnected);
    };
    if owner.controller_disconnected {
        Some(crate::abilities::SentryCleanupReason::OwnerDisconnected)
    } else if owner.defeated {
        Some(crate::abilities::SentryCleanupReason::OwnerDefeated)
    } else if !owner.active {
        Some(crate::abilities::SentryCleanupReason::OwnerDisconnected)
    } else {
        None
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
            (
                &mut crate::builds::AbilityState,
                &mut crate::combat::HealthRecoveryState,
            ),
            Option<&lightyear::prelude::input::native::ActionState<crate::protocol::FighterInput>>,
            Option<&mut crate::abilities::UltimateInputLatch>,
            Option<&crate::combat::Defeated>,
            Option<&crate::matchplay::ActiveCombatant>,
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
        (mut ability, mut health_recovery),
        action,
        latch,
        defeated,
        active,
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
            && !crate::movement::input_should_neutralize(
                tick.0,
                freshness.last_fresh_tick,
                crate::movement::AUTHORITATIVE_INPUT_STALE_TICKS,
            );
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
        } else if ability.charge != loadout.ultimate.charge_policy.maximum
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
        let Some(sentry_tuning) = resolve_sentry_tuning(&loadout.ultimate) else {
            continue;
        };
        let facing = Vec2::from_angle(rotation.as_radians());
        let mut excluded = Vec::with_capacity(1 + defeated_fighters.iter().len());
        excluded.push(owner);
        excluded.extend(defeated_fighters.iter());
        let filter = SpatialQueryFilter::from_mask(
            crate::movement::FIGHTER_LAYER
                | crate::movement::DEPLOYABLE_LAYER
                | crate::movement::STATIC_MAP_LAYER
                | crate::movement::DESTRUCTIBLE_MAP_LAYER
                | crate::movement::PLAYER_ONLY_MAP_LAYER,
        )
        .with_excluded_entities(excluded);
        let placement = first_clear_sentry_placement(
            position.0,
            facing,
            &sentry_tuning.placement_offsets_milliunits,
            sentry_tuning.body_radius,
            |candidate, radius| {
                bounds.0.contains_with_inset(candidate, radius)
                    && spatial_query
                        .shape_intersections(&Collider::circle(radius), candidate, 0.0, &filter)
                        .is_empty()
            },
        );
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
        let expires_at_tick = tick.0.saturating_add(sentry_tuning.lifetime_ticks);
        let identity = SentryIdentity {
            deployable_id,
            owner_player_id: *player,
            owner_network_id: *network_id,
            team_id: *team,
            ultimate_id: loadout.ultimate.id,
            match_id: participant.match_id,
        };
        commands
            .spawn((
                Sentry,
                identity,
                SentryDeadline { expires_at_tick },
                SentryRuntime::new(
                    tick.0,
                    sentry_tuning.acquisition_interval_ticks,
                    sentry_tuning.fire_interval_ticks,
                ),
                crate::combat::CurrentHealth(sentry_tuning.maximum_health),
                NetworkEntityId((1_u64 << 63) | deployable_id.0),
                crate::combat::TeamId(team.0),
                crate::matchplay::MatchMember(participant.match_id),
                avian2d::prelude::Position::from_xy(placement.x, placement.y),
                avian2d::prelude::Rotation::radians(rotation.as_radians()),
                Collider::circle(sentry_tuning.body_radius),
                Sensor,
                RigidBody::Static,
                CollisionLayers::new(
                    crate::movement::DEPLOYABLE_LAYER,
                    crate::movement::PROJECTILE_LAYER
                        | crate::movement::FIGHTER_LAYER
                        | crate::movement::DEPLOYABLE_LAYER,
                ),
                Replicate::to_clients(NetworkTarget::All),
            ))
            .insert(sentry_tuning);
        *ability = crate::builds::AbilityState {
            charge: 0,
            phase: crate::builds::AbilityPhase::Deployed {
                deployable_id,
                expires_at_tick,
            },
        };
        health_recovery.last_accepted_attack_tick = tick.0;
        health_recovery.recovery_remainder = 0;
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
    let owners = index_sentry_owner_lifecycle(&owners, &disconnected);
    for (identity, deadline, destroyed) in &sentries {
        let reason = sentry_cleanup_reason(
            tick.0,
            root,
            *identity,
            *deadline,
            destroyed.is_some(),
            &owners,
        );
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
fn coalesce_sentry_cleanup_requests(
    requests: impl IntoIterator<Item = SentryCleanupRequest>,
) -> std::collections::BTreeMap<crate::builds::DeployableId, SentryCleanupRequest> {
    let mut selected = std::collections::BTreeMap::new();
    for request in requests {
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
    selected
}

#[cfg(feature = "server")]
fn sentry_cleanup_lifetime(
    request: SentryCleanupRequest,
    deadline: Option<&SentryDeadline>,
    tuning: &ResolvedSentryTuning,
) -> u64 {
    deadline.map_or(0, |deadline| {
        let activated_at_tick = deadline
            .expires_at_tick
            .saturating_sub(tuning.lifetime_ticks);
        request
            .requested_at_tick
            .saturating_sub(activated_at_tick)
            .min(tuning.lifetime_ticks)
    })
}

#[cfg(feature = "server")]
#[allow(clippy::too_many_arguments)]
fn purge_sentry_owned_work(
    commands: &mut Commands,
    sentry_entity: Entity,
    identity: SentryIdentity,
    deliveries: &Query<(Entity, &crate::combat::ReplicatedAttackSource)>,
    pending_payloads: &mut Messages<crate::combat::PendingPayload>,
    pending_deliveries: &mut Messages<crate::combat::PendingDelivery>,
    melee_attacks: &mut Messages<crate::combat::MeleeAttack>,
) {
    // Match teardown may queue the same removal earlier in the schedule. Cleanup remains the
    // single ability transaction, but tolerates that external ownership teardown race.
    commands.entity(sentry_entity).try_despawn();
    despawn_sentry_deliveries(
        commands,
        identity.owner_network_id,
        identity.deployable_id,
        deliveries,
    );
    retain_non_sentry_messages(
        pending_payloads,
        identity.owner_network_id,
        identity.deployable_id,
        |message| message.source,
    );
    retain_non_sentry_messages(
        pending_deliveries,
        identity.owner_network_id,
        identity.deployable_id,
        |message| message.source,
    );
    retain_non_sentry_messages(
        melee_attacks,
        identity.owner_network_id,
        identity.deployable_id,
        |message| message.source,
    );
}

#[cfg(feature = "server")]
fn settle_sentry_owner(
    identity: SentryIdentity,
    tuning: &ResolvedSentryTuning,
    mut cleanup_position: Option<Vec2>,
    owners: &mut Query<(
        &NetworkEntityId,
        &avian2d::prelude::Position,
        &mut crate::builds::AbilityState,
    )>,
) -> Option<Vec2> {
    for (owner, owner_position, mut ability) in owners {
        if *owner == identity.owner_network_id
            && matches!(
                ability.phase,
                crate::builds::AbilityPhase::Deployed { deployable_id, .. }
                    if deployable_id == identity.deployable_id
            )
        {
            ability.phase =
                crate::abilities::settled_ability_phase(ability.charge, tuning.charge_maximum);
        }
        if *owner == identity.owner_network_id && cleanup_position.is_none() {
            cleanup_position = Some(owner_position.0);
        }
    }
    cleanup_position
}

#[cfg(feature = "server")]
#[allow(clippy::too_many_arguments)]
fn publish_sentry_cleanup(
    request: SentryCleanupRequest,
    identity: SentryIdentity,
    lifetime_ticks: u64,
    cleanup_position: Option<Vec2>,
    ids: &mut crate::combat::NextCombatIds,
    telemetry: &mut crate::abilities::AbilityTelemetry,
    combat_telemetry: &mut crate::combat::CombatTelemetry,
    outbox: &mut crate::combat::CombatOutbox,
) {
    telemetry.record(crate::abilities::AbilityTelemetryRecord {
        tick: request.requested_at_tick,
        owner_network_id: identity.owner_network_id,
        kind: crate::abilities::AbilityTelemetryKind::SentryCleanup {
            deployable_id: identity.deployable_id,
            reason: request.reason,
            lifetime_ticks,
        },
    });
    let Some(cleanup_position) = cleanup_position else {
        return;
    };
    let Some(event_id) = ids.allocate_event() else {
        return;
    };
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
            &ResolvedSentryTuning,
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
    let selected = coalesce_sentry_cleanup_requests(requests.read().copied());
    for request in selected.into_values() {
        let Some((entity, identity, deadline, position, tuning)) = sentries
            .iter()
            .find(|(_, identity, ..)| identity.deployable_id == request.deployable_id)
        else {
            continue;
        };
        let identity = *identity;
        let lifetime_ticks = sentry_cleanup_lifetime(request, deadline, tuning);
        purge_sentry_owned_work(
            &mut commands,
            entity,
            identity,
            &deliveries,
            &mut pending_payloads,
            &mut pending_deliveries,
            &mut melee_attacks,
        );
        let cleanup_position = settle_sentry_owner(
            identity,
            tuning,
            position.map(|position| position.0),
            &mut owners,
        );
        publish_sentry_cleanup(
            request,
            identity,
            lifetime_ticks,
            cleanup_position,
            &mut ids,
            &mut telemetry,
            &mut combat_telemetry,
            &mut outbox,
        );
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
#[derive(Clone, Copy, Debug)]
struct SentryOwnerView {
    visibility_position: Vec2,
    reveal_radius: f32,
    projectile_owner: Entity,
}

#[cfg(feature = "server")]
#[derive(Clone, Debug)]
struct SentryFighterTargetView {
    position: Vec2,
    network_id: NetworkEntityId,
    team_id: crate::combat::TeamId,
    targetable: bool,
    base_concealment: crate::concealment::ConcealmentSources,
    reveal_deadlines: Option<crate::concealment::ConcealmentRevealDeadlines>,
    ability: crate::builds::AbilityState,
    forced_reveals: Option<crate::concealment::ForcedRevealSources>,
    objective_carrier: bool,
}

#[cfg(feature = "server")]
#[derive(Clone, Copy, Debug)]
struct SentryObjectiveTargetView {
    entity: Entity,
    position: Vec2,
    identity: crate::map::DamageableTargetIdentity,
    safe: crate::matchplay::HeistSafe,
    health: u16,
    life: crate::map::DamageableLifeState,
}

#[cfg(feature = "server")]
#[derive(Clone, Copy, Debug)]
struct SentryFireTarget {
    position: Vec2,
    cue_target: Option<NetworkEntityId>,
}

#[cfg(feature = "server")]
#[derive(Clone, Debug)]
struct SentryFirePlan {
    owner_entity: Entity,
    direction: Vec2,
    cue: crate::combat::CombatCue,
    source: crate::combat::AttackSource,
    recipe: crate::combat::WeaponRecipe,
}

#[cfg(feature = "server")]
fn index_sentry_owners(
    owners: &Query<
        (
            Entity,
            &NetworkEntityId,
            &avian2d::prelude::Position,
            &crate::builds::ResolvedMatchLoadout,
        ),
        With<crate::protocol::Fighter>,
    >,
) -> std::collections::BTreeMap<NetworkEntityId, SentryOwnerView> {
    let mut indexed = std::collections::BTreeMap::new();
    for (entity, network_id, position, loadout) in owners {
        indexed
            .entry(*network_id)
            .and_modify(|view: &mut SentryOwnerView| view.projectile_owner = entity)
            .or_insert(SentryOwnerView {
                visibility_position: position.0,
                reveal_radius: loadout.fighter_stats.reveal_proximity_radius,
                projectile_owner: entity,
            });
    }
    indexed
}

#[cfg(feature = "server")]
#[allow(clippy::type_complexity)]
fn index_sentry_fighter_targets(
    fighters: &Query<
        (
            &avian2d::prelude::Position,
            &NetworkEntityId,
            &crate::combat::TeamId,
            Option<&crate::combat::Defeated>,
            Option<&crate::matchplay::ActiveCombatant>,
            Option<&crate::concealment::TerrainConcealmentMembership>,
            Option<&crate::concealment::AlliedConcealmentMemberships>,
            Option<&crate::concealment::ConcealmentRevealDeadlines>,
            &crate::builds::AbilityState,
            Option<&crate::concealment::ForcedRevealSources>,
            Has<crate::concealment::ObjectiveCarrier>,
        ),
        With<crate::protocol::Fighter>,
    >,
) -> Vec<SentryFighterTargetView> {
    fighters
        .iter()
        .map(
            |(
                position,
                network_id,
                team_id,
                defeated,
                active,
                terrain,
                field,
                reveal_deadlines,
                ability,
                forced_reveals,
                objective_carrier,
            )| SentryFighterTargetView {
                position: position.0,
                network_id: *network_id,
                team_id: *team_id,
                targetable: defeated.is_none() && active.is_some(),
                base_concealment: crate::concealment::ConcealmentSources {
                    terrain: terrain.is_some(),
                    self_cloak: false,
                    allied_field: field.is_some_and(|value| !value.0.is_empty()),
                },
                reveal_deadlines: reveal_deadlines.copied(),
                ability: *ability,
                forced_reveals: forced_reveals.cloned(),
                objective_carrier,
            },
        )
        .collect()
}

#[cfg(feature = "server")]
#[allow(clippy::type_complexity)]
fn index_sentry_objective_targets(
    objectives: &Query<
        (
            Entity,
            &avian2d::prelude::Position,
            &crate::map::DamageableTargetIdentity,
            &crate::matchplay::HeistSafe,
            &crate::combat::CurrentHealth,
            &crate::map::DamageableLifeState,
        ),
        With<crate::matchplay::HeistSafe>,
    >,
) -> Vec<SentryObjectiveTargetView> {
    objectives
        .iter()
        .map(
            |(entity, position, identity, safe, health, life)| SentryObjectiveTargetView {
                entity,
                position: position.0,
                identity: *identity,
                safe: *safe,
                health: health.0,
                life: *life,
            },
        )
        .collect()
}

#[cfg(feature = "server")]
fn sentry_observer_can_see(
    tick: u64,
    observer_team: crate::combat::TeamId,
    owner: SentryOwnerView,
    target: &SentryFighterTargetView,
) -> bool {
    crate::concealment::observer_can_see(crate::concealment::ObserverVisibilityInput {
        relation: crate::concealment::ObserverRelation::Enemy,
        observer_alive: true,
        concealment: if target.objective_carrier {
            crate::concealment::ConcealmentSources::NONE
        } else {
            crate::concealment::ConcealmentSources {
                terrain: target.base_concealment.terrain,
                self_cloak: matches!(target.ability.phase, crate::builds::AbilityPhase::Cloaked { expires_at_tick, .. } if tick < expires_at_tick),
                allied_field: target.base_concealment.allied_field,
            }
        },
        forced_revealed: target
            .forced_reveals
            .as_ref()
            .is_some_and(|sources| sources.active_for_team(observer_team, tick)),
        subject_reveal_locked: target
            .reveal_deadlines
            .is_some_and(|deadlines| crate::concealment::reveal_lock_active(tick, deadlines)),
        distance_squared: owner.visibility_position.distance_squared(target.position),
        reveal_radius: owner.reveal_radius,
    })
}

#[cfg(feature = "server")]
fn sentry_has_clear_line_of_sight(
    spatial_query: &avian2d::prelude::SpatialQuery,
    origin: Vec2,
    target: Vec2,
    excluded_entity: Option<Entity>,
) -> bool {
    use bevy::math::Dir2;

    let delta = target - origin;
    let Some(direction) = Dir2::new(delta.normalize_or_zero()).ok() else {
        return false;
    };
    let mut filter = avian2d::prelude::SpatialQueryFilter::from_mask(
        crate::movement::STATIC_MAP_LAYER | crate::movement::DESTRUCTIBLE_MAP_LAYER,
    );
    if let Some(entity) = excluded_entity {
        filter = filter.with_excluded_entities([entity]);
    }
    spatial_query
        .cast_ray(origin, direction, delta.length(), true, &filter)
        .is_none()
}

#[cfg(feature = "server")]
#[allow(clippy::too_many_arguments)]
fn select_sentry_target(
    tick: u64,
    sentry_position: Vec2,
    identity: SentryIdentity,
    acquisition_range: f32,
    owner: Option<SentryOwnerView>,
    fighters: &[SentryFighterTargetView],
    objectives: &[SentryObjectiveTargetView],
    spatial_query: &avian2d::prelude::SpatialQuery,
) -> Option<SentryTarget> {
    let fighter = stable_sentry_target(
        fighters.iter().filter_map(|target| {
            if target.team_id == identity.team_id || !target.targetable {
                return None;
            }
            let visible_to_owner = owner.is_some_and(|owner| {
                sentry_observer_can_see(tick, identity.team_id, owner, target)
            });
            if !visible_to_owner {
                return None;
            }
            let distance_squared = sentry_position.distance_squared(target.position);
            let clear_line_of_sight = sentry_has_clear_line_of_sight(
                spatial_query,
                sentry_position,
                target.position,
                None,
            );
            Some((target.network_id, distance_squared, clear_line_of_sight))
        }),
        acquisition_range,
    );
    if let Some(fighter) = fighter {
        return Some(SentryTarget::Fighter(fighter));
    }
    stable_sentry_objective_target(
        objectives.iter().filter_map(|target| {
            if target.safe.match_id != identity.match_id
                || target.safe.defending_team == identity.team_id
                || target.health == 0
                || !matches!(target.life, crate::map::DamageableLifeState::Live)
            {
                return None;
            }
            Some((
                target.identity,
                sentry_position.distance_squared(target.position),
                sentry_has_clear_line_of_sight(
                    spatial_query,
                    sentry_position,
                    target.position,
                    Some(target.entity),
                ),
            ))
        }),
        acquisition_range,
    )
    .map(SentryTarget::ModeObjective)
}

#[cfg(feature = "server")]
fn revalidate_sentry_target(
    tick: u64,
    identity: SentryIdentity,
    target: SentryTarget,
    owner: Option<SentryOwnerView>,
    fighters: &[SentryFighterTargetView],
    objectives: &[SentryObjectiveTargetView],
) -> Option<SentryFireTarget> {
    match target {
        SentryTarget::Fighter(network_id) => {
            let target = fighters
                .iter()
                .find(|target| target.network_id == network_id && target.targetable)?;
            owner
                .is_some_and(|owner| sentry_observer_can_see(tick, identity.team_id, owner, target))
                .then_some(SentryFireTarget {
                    position: target.position,
                    cue_target: Some(network_id),
                })
        }
        SentryTarget::ModeObjective(target_identity) => objectives
            .iter()
            .find(|target| {
                target.identity == target_identity
                    && target.safe.match_id == identity.match_id
                    && target.safe.defending_team != identity.team_id
                    && target.health > 0
                    && matches!(target.life, crate::map::DamageableLifeState::Live)
            })
            .map(|target| SentryFireTarget {
                position: target.position,
                cue_target: None,
            }),
    }
}

#[cfg(feature = "server")]
#[allow(clippy::too_many_arguments)]
fn plan_sentry_fire(
    tick: u64,
    sentry_position: Vec2,
    identity: SentryIdentity,
    target: SentryFireTarget,
    direction: Vec2,
    tuning: &ResolvedSentryTuning,
    owner: SentryOwnerView,
    attack_id: crate::combat::AttackId,
    fire_event_id: crate::combat::CombatEventId,
) -> SentryFirePlan {
    let source = crate::combat::AttackSource {
        kind: crate::combat::CombatSourceKind::Deployable {
            ultimate_id: identity.ultimate_id,
            deployable_id: identity.deployable_id,
        },
        attack_id,
        player_id: identity.owner_player_id,
        owner_network_entity_id: identity.owner_network_id,
        team_id: identity.team_id,
        recipe_fingerprint: tuning.recipe_fingerprint,
        presentation_profile_id: tuning.presentation_profile_id,
        legacy_compatibility: false,
        source_preset_id: None,
        origin: crate::combat::WorldPoint::from(sentry_position),
        facing: direction.y.atan2(direction.x),
    };
    SentryFirePlan {
        owner_entity: owner.projectile_owner,
        direction,
        cue: crate::combat::CombatCue::SentryFired {
            event_id: fire_event_id,
            tick,
            owner: identity.owner_network_id,
            deployable_id: identity.deployable_id,
            target: target.cue_target,
            position: crate::combat::WorldPoint::from(sentry_position),
            presentation_profile_id: tuning.presentation_profile_id,
        },
        source,
        recipe: tuning.recipe.clone(),
    }
}

#[cfg(feature = "server")]
#[allow(clippy::too_many_arguments)]
fn commit_sentry_fire(
    commands: &mut Commands,
    tick: u64,
    sentry_entity: Entity,
    identity: SentryIdentity,
    runtime: &mut SentryRuntime,
    tuning: &ResolvedSentryTuning,
    plan: SentryFirePlan,
    telemetry: &mut crate::abilities::AbilityTelemetry,
    combat_telemetry: &mut crate::combat::CombatTelemetry,
    outbox: &mut crate::combat::CombatOutbox,
) {
    use avian2d::prelude::CollisionLayers;
    use lightyear::prelude::{InterpolationTarget, NetworkTarget, Replicate};

    let crate::combat::DeliveryMethod::Straight {
        speed,
        radius,
        range,
        lifetime_ticks,
        ..
    } = plan.recipe.delivery
    else {
        return;
    };
    runtime.record_fire(tick, tuning.fire_interval_ticks);
    telemetry.record(crate::abilities::AbilityTelemetryRecord {
        tick,
        owner_network_id: identity.owner_network_id,
        kind: crate::abilities::AbilityTelemetryKind::SentryShot(identity.deployable_id),
    });
    combat_telemetry.record_cue(plan.cue.clone());
    outbox.0.push(plan.cue);
    commands.spawn((
        crate::combat::Projectile,
        crate::combat::ProjectileSource {
            shot_id: crate::combat::ShotId(plan.source.attack_id.0),
            player_id: identity.owner_player_id,
            owner_network_entity_id: identity.owner_network_id,
            team_id: identity.team_id,
            weapon_definition_id: crate::combat::WeaponDefinitionId(1),
        },
        crate::combat::ReplicatedAttackSource {
            attack: plan.source,
        },
        crate::combat::AttackDelivery {
            attack_id: plan.source.attack_id,
            delivery_index: 0,
        },
        crate::combat::ProjectileDeadline {
            expires_at_tick: tick.saturating_add(lifetime_ticks),
        },
        crate::combat::StraightFlight {
            origin: crate::combat::WorldPoint::from(plan.source.origin.as_vec2()),
            facing: plan.source.facing,
            speed,
            maximum_range: range,
            launched_at_tick: tick,
        },
        crate::combat::ProjectileBody::circle(radius),
        crate::combat::ComposedProjectileRuntime {
            owner_entity: plan.owner_entity,
            source_entity: sentry_entity,
            source: plan.source,
            delivery_index: 0,
            velocity: plan.direction * speed,
            travelled: 0.0,
            expires_at_tick: tick.saturating_add(lifetime_ticks),
            maximum_range: range,
            landing: None,
            recipe: plan.recipe,
        },
        avian2d::prelude::Position(plan.source.origin.as_vec2()),
        avian2d::prelude::Rotation::radians(plan.source.facing),
        crate::combat::ProjectileBody::circle(radius).collider(),
        CollisionLayers::new(
            crate::movement::PROJECTILE_LAYER,
            crate::movement::FIGHTER_LAYER
                | crate::movement::DEPLOYABLE_LAYER
                | crate::movement::STATIC_MAP_LAYER
                | crate::movement::DESTRUCTIBLE_MAP_LAYER,
        ),
        crate::matchplay::MatchMember(identity.match_id),
        Replicate::to_clients(NetworkTarget::All),
        InterpolationTarget::to_clients(NetworkTarget::All),
    ));
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
            &ResolvedSentryTuning,
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
            Option<&crate::concealment::TerrainConcealmentMembership>,
            Option<&crate::concealment::AlliedConcealmentMemberships>,
            Option<&crate::concealment::ConcealmentRevealDeadlines>,
            &crate::builds::AbilityState,
            Option<&crate::concealment::ForcedRevealSources>,
            Has<crate::concealment::ObjectiveCarrier>,
        ),
        With<crate::protocol::Fighter>,
    >,
    objectives: Query<
        (
            Entity,
            &avian2d::prelude::Position,
            &crate::map::DamageableTargetIdentity,
            &crate::matchplay::HeistSafe,
            &crate::combat::CurrentHealth,
            &crate::map::DamageableLifeState,
        ),
        With<crate::matchplay::HeistSafe>,
    >,
    owners: Query<
        (
            Entity,
            &NetworkEntityId,
            &avian2d::prelude::Position,
            &crate::builds::ResolvedMatchLoadout,
        ),
        With<crate::protocol::Fighter>,
    >,
) {
    let owner_index = index_sentry_owners(&owners);
    let fighter_targets = index_sentry_fighter_targets(&fighters);
    let objective_targets = index_sentry_objective_targets(&objectives);
    for (entity, position, identity, mut runtime, sentry_tuning) in &mut sentries {
        let owner = owner_index.get(&identity.owner_network_id).copied();
        if runtime.begin_acquisition_if_due(tick.0, sentry_tuning.acquisition_interval_ticks) {
            match select_sentry_target(
                tick.0,
                position.0,
                *identity,
                sentry_tuning.acquisition_range,
                owner,
                &fighter_targets,
                &objective_targets,
                &spatial_query,
            ) {
                Some(SentryTarget::Fighter(target)) => runtime.set_fighter_target(Some(target)),
                Some(SentryTarget::ModeObjective(target)) => {
                    runtime.set_objective_target(Some(target));
                }
                None => runtime.clear_target(),
            }
        }
        if !runtime.fire_is_due(tick.0) {
            continue;
        }
        let Some(target) = runtime.target() else {
            continue;
        };
        let Some(fire_target) = revalidate_sentry_target(
            tick.0,
            *identity,
            target,
            owner,
            &fighter_targets,
            &objective_targets,
        ) else {
            runtime.clear_target();
            continue;
        };
        let Some(direction) = (fire_target.position - position.0).try_normalize() else {
            continue;
        };
        let Some(attack_id) = ids.allocate_attack() else {
            continue;
        };
        for (owner_entity, owner_id, _, _) in &owners {
            if *owner_id == identity.owner_network_id {
                commands
                    .entity(owner_entity)
                    .remove::<crate::matchplay::SpawnProtection>();
            }
        }
        let Some(owner) = owner else {
            continue;
        };
        let Some(fire_event_id) = ids.allocate_event() else {
            continue;
        };
        let plan = plan_sentry_fire(
            tick.0,
            position.0,
            *identity,
            fire_target,
            direction,
            sentry_tuning,
            owner,
            attack_id,
            fire_event_id,
        );
        commit_sentry_fire(
            &mut commands,
            tick.0,
            entity,
            *identity,
            &mut runtime,
            sentry_tuning,
            plan,
            &mut telemetry,
            &mut combat_telemetry,
            &mut outbox,
        );
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
