//! Server-authoritative combat lifecycle, spatial targeting, and cue publication.
#![allow(clippy::wildcard_imports)]

use super::*;

/// M04's legacy cue/log shape is a compatibility adapter for the original single straight
/// direct-damage recipe. It is selected from the resolved recipe, never from a preset ID, and
/// does not participate in acceptance, collision, damage, or telemetry aggregation decisions.
#[cfg(feature = "server")]
pub(super) fn legacy_compatibility_recipe(recipe: &WeaponRecipe) -> bool {
    matches!(recipe.firing, FiringPattern::Single)
        && matches!(
            recipe.delivery,
            DeliveryMethod::Straight { .. } | DeliveryMethod::StickyStraight { .. }
        )
        && matches!(
            recipe.payload_bundles.as_slice(),
            [PayloadBundleDefinition {
                target: TargetSelection::Direct,
                effects
            }] if matches!(
                effects.as_slice(),
                [PayloadEffectDefinition::Damage {
                    falloff: DamageFalloff::None,
                    recipients: RecipientPolicy::Hostiles,
                    ..
                }]
            )
        )
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn validate_definitions(
    fighters: Res<FighterDefinitions>,
    weapons: Res<WeaponDefinitions>,
) {
    fighters
        .validate(&weapons)
        .expect("code-authored fighter definitions must be valid");
    weapons
        .validate(&fighters)
        .expect("code-authored weapon definitions must be valid");
}

#[cfg(feature = "server")]
#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn spawn_test_dummy(
    mut commands: Commands,
    catalog: Res<WeaponCatalogResource>,
    map_spawn: Res<TestDummyFixture>,
    fighters: Res<FighterDefinitions>,
    weapons: Res<WeaponDefinitions>,
) {
    if fighters.get(STANDARD_FIGHTER_DEFINITION).is_none()
        || weapons.get(PULSE_SIDEARM_DEFINITION).is_none()
    {
        return;
    }
    let Some(fighter) = fighters.get(STANDARD_FIGHTER_DEFINITION) else {
        return;
    };
    let position = map_spawn.position;
    let spawn_facing = map_spawn.facing;
    let body_radius = fighter.body_radius;
    let (fighter_definition, team, health, weapon) =
        default_fighter_runtime(NEUTRAL_TEAM, &fighters, &weapons);
    let build_catalog =
        crate::builds::BuildCatalog::embedded().expect("embedded build catalog is valid");
    let loadout = crate::builds::resolve_saved_brawler_recipe(
        &build_catalog,
        &catalog.0,
        fighter,
        crate::profiles::FighterProfileId(1),
        crate::profiles::WeaponBaseId(1),
        crate::builds::UltimateDefinitionId(1),
        [
            crate::builds::PassiveDefinitionId(3),
            crate::builds::PassiveDefinitionId(4),
        ],
    )
    .expect("dummy saved-brawler loadout resolves");
    let dummy = commands
        .spawn((
            Fighter,
            crate::movement::InputFreshness::default(),
            PlayerId(0),
            DUMMY_NETWORK_ENTITY,
            crate::protocol::PlaceholderState { spawn_slot: 255 },
            fighter_definition,
            team,
            health,
            weapon,
            Position::from_xy(position.x, position.y),
            Rotation::radians(spawn_facing),
            SpawnState {
                position,
                facing: spawn_facing,
            },
            LinearVelocity::default(),
            AngularVelocity::default(),
        ))
        .id();
    commands.entity(dummy).insert((
        loadout.identity,
        loadout,
        AuthoritativeTick::default(),
        Collider::circle(body_radius),
        RigidBody::Kinematic,
        CustomPositionIntegration,
        fighter_collision_layers(),
        Replicate::to_clients(NetworkTarget::All),
        InterpolationTarget::to_clients(NetworkTarget::All),
        TestDummy,
    ));
}

/// Marks the reserved stationary hostile practice target.
#[cfg(feature = "server")]
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TestDummy;

/// Explicit opt-in fixture; production Wipeout composition never inserts it.
#[cfg(feature = "server")]
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct TestDummyFixture {
    pub position: Vec2,
    pub facing: f32,
}

#[cfg(feature = "server")]
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TestDummyResetDeadline(pub u64);

#[cfg(feature = "server")]
#[allow(clippy::too_many_arguments)]
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime; the query declares this system's complete world view inline at its schedule boundary"
)]
pub(super) fn reset_due_fighters(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    fighters: Res<FighterDefinitions>,
    mut telemetry: ResMut<CombatTelemetry>,
    mut ids: ResMut<NextCombatIds>,
    mut outbox: ResMut<CombatOutbox>,
    query: Query<
        (
            Entity,
            &NetworkEntityId,
            &FighterDefinitionId,
            &crate::builds::ResolvedMatchLoadout,
            &TestDummyResetDeadline,
            &SpawnState,
        ),
        With<TestDummy>,
    >,
) {
    for (entity, network_id, fighter_id, loadout, deadline, spawn) in &query {
        if !reset_is_due(tick.0, deadline.0) {
            continue;
        }
        let Some(fighter) = fighters.get(*fighter_id) else {
            continue;
        };
        let (capacity, refill_ticks) = (
            loadout.primary_weapon.recipe.economy.capacity(),
            loadout.primary_weapon.recipe.economy.refill_ticks(),
        );
        if capacity == 0 || refill_ticks == 0 {
            continue;
        }
        let Some(event_id) = ids.allocate_event() else {
            continue;
        };
        let position = spawn.position;
        commands
            .entity(entity)
            .insert((
                CurrentHealth(fighter.maximum_health),
                WeaponState {
                    ammo: capacity,
                    phase: WeaponPhase::Ready,
                    ammo_recovery: None,
                },
                HealthRecoveryState::starting_at(tick.0),
                Position::from_xy(position.x, position.y),
                Rotation::radians(spawn.facing),
                fighter_collision_layers(),
            ))
            .remove::<Defeated>()
            .remove::<TestDummyResetDeadline>()
            .remove::<ExternalMotion>()
            .remove::<KnockbackFeedback>()
            .insert(ActiveEffects::default());
        telemetry.record(CombatLogRecord::Reset {
            tick: tick.0,
            event_id,
            target: *network_id,
            position: WorldPoint::from(position),
        });
        info!(
            tick = tick.0,
            event_id = event_id.0,
            target = network_id.0,
            position = ?position,
            "authoritative fighter reset"
        );
        let cue = CombatCue::Reset {
            event_id,
            tick: tick.0,
            target: *network_id,
            position: WorldPoint::from(position),
        };
        telemetry.record_cue(cue.clone());
        outbox.0.push(cue);
        let _ = fighter;
    }
}

#[cfg(feature = "server")]
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime; the query declares this system's complete world view inline at its schedule boundary"
)]
pub(super) fn expire_runtime_effects(
    tick: Res<SimulationTick>,
    mut commands: Commands,
    mut fighters: Query<
        (
            Entity,
            &mut ActiveEffects,
            Option<&ExternalMotion>,
            Option<&KnockbackFeedback>,
            Option<&Defeated>,
        ),
        Or<(With<Fighter>, With<crate::abilities::Sentry>)>,
    >,
) {
    for (entity, mut effects, external_motion, knockback, defeated) in &mut fighters {
        if defeated.is_some() {
            effects.slow = None;
            if external_motion.is_some() {
                commands
                    .entity(entity)
                    .remove::<ExternalMotion>()
                    .remove::<KnockbackFeedback>();
            }
            continue;
        }
        if effects
            .slow
            .is_some_and(|slow| tick.0 >= slow.expires_at_tick)
        {
            effects.slow = None;
        }
        if external_motion.is_some_and(|motion| tick.0 >= motion.expires_at_tick) {
            commands
                .entity(entity)
                .remove::<ExternalMotion>()
                .remove::<KnockbackFeedback>();
        } else if knockback.is_some_and(|feedback| tick.0 >= feedback.expires_at_tick) {
            commands.entity(entity).remove::<KnockbackFeedback>();
        }
    }
}

#[cfg(feature = "server")]
pub(super) fn payload_can_affect_target(
    bundle: &PayloadBundleDefinition,
    source: AttackSource,
    target_team: TeamId,
    target_network_id: NetworkEntityId,
) -> bool {
    bundle.effects.iter().any(|effect| {
        let recipients = match *effect {
            PayloadEffectDefinition::Damage { recipients, .. }
            | PayloadEffectDefinition::Knockback { recipients, .. }
            | PayloadEffectDefinition::Slow { recipients, .. }
            | PayloadEffectDefinition::Cold { recipients, .. }
            | PayloadEffectDefinition::DamageOverTime { recipients, .. }
            | PayloadEffectDefinition::Heal { recipients, .. } => recipients,
        };
        if target_network_id == source.owner_network_entity_id {
            matches!(
                recipients,
                RecipientPolicy::HostilesAndOwner { .. } | RecipientPolicy::AlliesAndOwner
            )
        } else if teams_are_hostile(source.team_id, target_team) {
            matches!(
                recipients,
                RecipientPolicy::Hostiles | RecipientPolicy::HostilesAndOwner { .. }
            )
        } else {
            matches!(
                recipients,
                RecipientPolicy::Allies | RecipientPolicy::AlliesAndOwner
            )
        }
    })
}

#[cfg(feature = "server")]
pub(super) fn area_line_of_sight_clear(
    origin: Vec2,
    target: Vec2,
    spatial_query: &avian2d::prelude::SpatialQuery,
) -> bool {
    let delta = target - origin;
    let distance = delta.length();
    let Some(direction) = Dir2::new(delta.normalize_or_zero()).ok() else {
        return true;
    };
    let filter =
        avian2d::prelude::SpatialQueryFilter::from_mask(STATIC_MAP_LAYER | DESTRUCTIBLE_MAP_LAYER);
    spatial_query
        .cast_ray(origin, direction, distance.max(0.0), true, &filter)
        .is_none()
}

#[cfg(feature = "server")]
pub(super) fn area_line_of_sight_clear_excluding(
    origin: Vec2,
    target: Vec2,
    excluded: Entity,
    spatial_query: &avian2d::prelude::SpatialQuery,
) -> bool {
    let delta = target - origin;
    let distance = delta.length();
    let Some(direction) = Dir2::new(delta.normalize_or_zero()).ok() else {
        return true;
    };
    let filter =
        avian2d::prelude::SpatialQueryFilter::from_mask(STATIC_MAP_LAYER | DESTRUCTIBLE_MAP_LAYER)
            .with_excluded_entities([excluded]);
    spatial_query
        .cast_ray(origin, direction, distance.max(0.0), true, &filter)
        .is_none()
}

#[cfg(feature = "server")]
pub(super) fn map_muzzle_contact(
    origin: Vec2,
    muzzle: Vec2,
    body: ProjectileBody,
    spatial_query: &avian2d::prelude::SpatialQuery,
) -> Option<(Entity, Vec2, Vec2)> {
    let delta = muzzle - origin;
    let distance = delta.length();
    let direction = Dir2::new(delta.normalize_or_zero()).ok()?;
    let filter =
        avian2d::prelude::SpatialQueryFilter::from_mask(STATIC_MAP_LAYER | DESTRUCTIBLE_MAP_LAYER);
    spatial_query
        .cast_shape_predicate(
            &body.collider(),
            origin,
            0.0,
            direction,
            &avian2d::prelude::ShapeCastConfig::from_max_distance(distance),
            &filter,
            &|_| true,
        )
        .map(|hit| (hit.entity, hit.point2, hit.normal1))
}

#[cfg(feature = "server")]
#[allow(clippy::too_many_arguments)]
#[allow(
    clippy::type_complexity,
    reason = "the query parameter is the shared area-targeting view, declared inline where the payload plan is built"
)]
#[allow(
    clippy::too_many_lines,
    reason = "the bounded area planner keeps the shared combatant/object target budget and stable ordering visible in one transaction"
)]
pub(super) fn queue_area_payloads(
    landing: Vec2,
    source: AttackSource,
    delivery_index: u8,
    recipe: &WeaponRecipe,
    fighters: &Query<
        (
            Entity,
            &Position,
            &TeamId,
            &NetworkEntityId,
            Option<&Defeated>,
            Option<&lightyear::prelude::ControlledBy>,
        ),
        Or<(With<Fighter>, With<crate::abilities::Sentry>)>,
    >,
    objects: &Query<
        (
            Entity,
            &Position,
            &crate::map::DamageableTargetIdentity,
            &CurrentHealth,
            &crate::map::DamageableLifeState,
        ),
        Or<(
            With<crate::map::DamageableWorldObject>,
            With<crate::matchplay::HeistSafe>,
        )>,
    >,
    disconnected: &HashSet<Entity>,
    spatial_query: &avian2d::prelude::SpatialQuery,
    pending: &mut MessageWriter<PendingPayload>,
    world_pending: &mut ResMut<crate::map::PendingWorldTargetDamages>,
    objective_pending: &mut ResMut<crate::matchplay::PendingModeObjectiveDamages>,
) -> usize {
    let mut queued = 0;
    let fighter_filter = avian2d::prelude::SpatialQueryFilter::from_mask(
        FIGHTER_LAYER | crate::movement::DEPLOYABLE_LAYER,
    );
    for (bundle_index, bundle) in recipe
        .payload_bundles
        .iter()
        .enumerate()
        .filter(|(_, bundle)| matches!(bundle.target, TargetSelection::Area { .. }))
    {
        let TargetSelection::Area {
            radius,
            map_occlusion,
            max_targets,
        } = bundle.target
        else {
            continue;
        };
        let candidate_entities = spatial_query.shape_intersections(
            &Collider::circle(radius),
            landing,
            0.0,
            &fighter_filter,
        );
        let mut candidates: Vec<_> = candidate_entities
            .into_iter()
            .filter_map(|entity| fighters.get(entity).ok().map(|data| (entity, data)))
            .collect();
        candidates.sort_by_key(|(_, (_, _, _, network_id, _, _))| network_id.0);
        let mut collected = 0_u8;
        for (target, (_, position, team, network_id, defeated, controlled)) in candidates {
            if collected >= max_targets {
                break;
            }
            if defeated.is_some()
                || controlled.is_some_and(|controlled| disconnected.contains(&controlled.owner))
                || (map_occlusion && !area_line_of_sight_clear(landing, position.0, spatial_query))
                || !payload_can_affect_target(bundle, source, *team, *network_id)
            {
                continue;
            }
            pending.write(PendingPayload {
                source,
                delivery_index,
                bundle_index: u8::try_from(bundle_index).unwrap_or(u8::MAX),
                target,
                target_network_id: *network_id,
                position: landing,
                engagement_distance: source.origin.as_vec2().distance(position.0),
                delivery_travel: lob_launch_point(source, recipe).distance(landing),
                contact_fraction: 1.0,
                bundle: bundle.clone(),
            });
            queued += 1;
            collected = collected.saturating_add(1);
        }
        let mut object_candidates: Vec<_> = objects
            .iter()
            .filter(|(_, position, _, health, life)| {
                crate::map::object_is_live(**health, **life)
                    && position.0.distance_squared(landing) <= radius * radius
            })
            .collect();
        object_candidates.sort_by_key(|(_, _, identity, ..)| identity.stable_order_key());
        for (entity, position, identity, _, _) in object_candidates {
            if collected >= max_targets {
                break;
            }
            if map_occlusion
                && !area_line_of_sight_clear_excluding(landing, position.0, entity, spatial_query)
            {
                continue;
            }
            for (effect_index, effect) in bundle.effects.iter().enumerate() {
                let PayloadEffectDefinition::Damage {
                    amount, falloff, ..
                } = *effect
                else {
                    continue;
                };
                delivery::queue_damageable_target(
                    world_pending,
                    objective_pending,
                    crate::map::PendingWorldTargetDamage {
                        target: *identity,
                        source,
                        attack_id: source.attack_id,
                        requested_damage: effects::requested_damage(
                            amount,
                            falloff,
                            lob_launch_point(source, recipe).distance(landing),
                            1.0,
                            false,
                            source.origin.as_vec2().distance(position.0),
                        ),
                        delivery_index,
                        bundle_index: u8::try_from(bundle_index).unwrap_or(u8::MAX),
                        effect_index: u8::try_from(effect_index).unwrap_or(u8::MAX),
                    },
                );
            }
            collected = collected.saturating_add(1);
            queued += 1;
        }
    }
    queued
}

#[cfg(feature = "server")]
pub(super) fn lob_launch_point(source: AttackSource, recipe: &WeaponRecipe) -> Vec2 {
    let muzzle_offset = match recipe.delivery {
        DeliveryMethod::Lobbed { muzzle_offset, .. } => muzzle_offset,
        _ => 0.0,
    };
    muzzle_position(source.origin.as_vec2(), source.facing, muzzle_offset)
}

#[cfg(feature = "server")]
pub(super) fn record_delivery_termination(
    ids: &mut NextCombatIds,
    telemetry: &mut WeaponTelemetry,
    tick: u64,
    runtime: &ComposedProjectileRuntime,
    position: Vec2,
    outcome: WeaponTelemetryOutcome,
) {
    let Some(event_id) = ids.allocate_event() else {
        return;
    };
    telemetry.record(WeaponTelemetryRecord {
        tick,
        event_id,
        attack_id: runtime.source.attack_id,
        preset_id: runtime.source.source_preset_id.unwrap_or(WeaponPresetId(0)),
        recipe_fingerprint: runtime.source.recipe_fingerprint,
        delivery_index: Some(runtime.delivery_index),
        source: runtime.source.owner_network_entity_id,
        target: None,
        position: WorldPoint::from(position),
        requested_value: 0,
        applied_value: 0,
        engagement_distance: 0.0,
        delivery_travel: runtime.travelled,
        hostile_contact: false,
        effect: None,
        resulting_health: None,
        resulting_effects: None,
        resulting_motion: None,
        outcome,
    });
}

#[cfg(feature = "server")]
#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn publish_authoritative_tick(
    tick: Res<SimulationTick>,
    mut fighters: Query<&mut AuthoritativeTick, With<Fighter>>,
) {
    for mut authoritative_tick in &mut fighters {
        authoritative_tick.0 = tick.0;
    }
}

#[cfg(feature = "server")]
#[allow(clippy::too_many_arguments)]
#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn cleanup_disconnected_projectiles(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    mut ids: ResMut<NextCombatIds>,
    mut trackers: ResMut<ActiveAttackTrackers>,
    mut telemetry: ResMut<WeaponTelemetry>,
    disconnected: Query<Entity, (With<LinkOf>, With<lightyear::prelude::Disconnected>)>,
    fighters: Query<(Entity, Option<&lightyear::prelude::ControlledBy>), With<Fighter>>,
    projectiles: Query<(Entity, &Position, &ComposedProjectileRuntime)>,
) {
    let disconnected: HashSet<_> = disconnected.iter().collect();
    let mut fighter_entities = HashSet::new();
    let mut disconnected_fighters = HashSet::new();
    for (fighter, controlled) in &fighters {
        fighter_entities.insert(fighter);
        if controlled.is_some_and(|controlled| disconnected.contains(&controlled.owner)) {
            disconnected_fighters.insert(fighter);
        }
    }
    for (entity, position, composed) in &projectiles {
        let owner_entity = composed.owner_entity;
        if disconnected_fighters.contains(&owner_entity)
            || !fighter_entities.contains(&owner_entity)
        {
            record_delivery_termination(
                &mut ids,
                &mut telemetry,
                tick.0,
                composed,
                position.0,
                WeaponTelemetryOutcome::DeliveryCancelled,
            );
            finish_attack_delivery(&mut trackers, composed.source.attack_id);
            commands.entity(entity).try_despawn();
        }
    }
}

#[cfg(feature = "server")]
#[allow(clippy::needless_pass_by_value, clippy::type_complexity)]
pub(super) fn send_combat_cues(
    mut outbox: ResMut<CombatOutbox>,
    mut telemetry: ResMut<CombatTelemetry>,
    visibility: Res<crate::concealment::ObserverVisibilityCache>,
    fighters: Query<
        (Entity, &NetworkEntityId),
        (
            With<Fighter>,
            With<crate::builds::AbilityState>,
            With<crate::matchplay::ActiveCombatant>,
            With<Replicate>,
        ),
    >,
    mut senders: Query<(Entity, &mut lightyear::prelude::MessageSender<CombatCue>), With<LinkOf>>,
) {
    // Deferred effect cues can be created after a later target's damage cue. Keep the retained
    // process evidence in the same event order as the wire batch sent to clients.
    telemetry
        .cues
        .sort_by_key(|cue| combat_cue_key(cue).event_id.0);
    let mut cues = std::mem::take(&mut outbox.0);
    cues.sort_by_key(|cue| combat_cue_key(cue).event_id.0);
    let fighter_entities: HashMap<_, _> = fighters
        .iter()
        .map(|(entity, network_id)| (network_id.0, entity))
        .collect();
    for (connection, mut sender) in &mut senders {
        for cue in &cues {
            if combat_cue_subjects(cue).into_iter().all(|subject| {
                fighter_entities
                    .get(&subject.0)
                    .is_none_or(|entity| visibility.permits(connection, *entity))
            }) {
                sender.send::<crate::protocol::CombatChannel>(cue.clone());
            }
        }
    }
}

#[cfg(feature = "server")]
fn damage_source_fighter(source: DamageSource) -> Option<NetworkEntityId> {
    match source {
        DamageSource::PlayerWeapon { fighter_id, .. }
        | DamageSource::Ultimate { fighter_id, .. }
        | DamageSource::Deployable { fighter_id, .. } => Some(fighter_id),
        DamageSource::Environment {
            initiating_fighter, ..
        } => initiating_fighter,
    }
}

#[cfg(feature = "server")]
fn combat_cue_subjects(cue: &CombatCue) -> Vec<NetworkEntityId> {
    match cue {
        CombatCue::AttackAccepted { source, .. }
        | CombatCue::DeliveryImpact { source, .. }
        | CombatCue::LobLanded { source, .. }
        | CombatCue::Muzzle { source, .. }
        | CombatCue::Impact { source, .. }
        | CombatCue::DemolitionStrikeActivated { source, .. }
        | CombatCue::ElementalFieldActivated { source, .. } => vec![*source],
        CombatCue::MeleeContact { source, target, .. } => vec![*source, *target],
        CombatCue::DamageApplied { source, target, .. }
        | CombatCue::EffectApplied { source, target, .. }
        | CombatCue::Damage { source, target, .. } => damage_source_fighter(*source)
            .map_or_else(|| vec![*target], |source| vec![source, *target]),
        CombatCue::FighterDefeated { source, target, .. }
        | CombatCue::Defeat { source, target, .. } => source
            .and_then(damage_source_fighter)
            .map_or_else(|| vec![*target], |source| vec![source, *target]),
        CombatCue::FighterReset { target, .. }
        | CombatCue::Reset { target, .. }
        | CombatCue::ForcedRevealApplied { target, .. } => vec![*target],
        CombatCue::SentryFired { owner, target, .. } => {
            target.map_or_else(|| vec![*owner], |target| vec![*owner, target])
        }
        CombatCue::DeployableRemoved { owner, .. } => vec![*owner],
        CombatCue::SelfCloakActivated { source, .. } | CombatCue::SelfCloakEnded { source, .. } => {
            vec![*source]
        }
        CombatCue::RevealScanActivated { .. } => Vec::new(),
    }
}

#[cfg(feature = "server")]
#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn emit_combat_summary(
    mut summary_logged: ResMut<CombatSummaryLogged>,
    telemetry: Res<CombatTelemetry>,
    weapon_telemetry: Res<WeaponTelemetry>,
    stopped: Query<(), (With<NetcodeServer>, With<Stopped>)>,
) {
    if summary_logged.0 || stopped.iter().next().is_none() {
        return;
    }
    summary_logged.0 = true;
    let hit_rate_basis_points = telemetry
        .hostile_fighter_hits
        .saturating_mul(10_000)
        .checked_div(telemetry.accepted_shots)
        .unwrap_or(0);
    info!(
        shots = telemetry.accepted_shots,
        hostile_hits = telemetry.hostile_fighter_hits,
        hit_rate_basis_points,
        applied_damage = telemetry.applied_damage,
        defeats = telemetry.defeats,
        close_hits = telemetry.close_hits,
        mid_hits = telemetry.mid_hits,
        long_hits = telemetry.long_hits,
        dropped_cues = telemetry.dropped_cues,
        dropped_records = telemetry.dropped_records,
        dropped_accepted_shot_timestamps = telemetry.dropped_accepted_shot_timestamps,
        weapon_dropped_records = weapon_telemetry.dropped_records,
        weapon_dropped_aggregate_entries = weapon_telemetry.dropped_aggregate_entries,
        "combat telemetry summary"
    );
}
