//! Authoritative attack economy and delivery orchestration.

#[allow(clippy::wildcard_imports)]
#[cfg(feature = "server")]
use super::*;

#[cfg(feature = "server")]
pub(crate) const MAX_ACTIVE_CONE_SPRAYS: usize = 32;

#[cfg(feature = "server")]
#[must_use]
pub(crate) fn cone_spray_timing(
    emitted_at_tick: u64,
    propagation_speed: f32,
    reach: f32,
    linger_ticks: u64,
    pulse_interval_ticks: u64,
) -> (u64, u64, u8) {
    let fill_ticks = ((reach * crate::timing::SIMULATION_TICK_HZ as f32) / propagation_speed)
        .ceil()
        .max(1.0) as u64;
    let full_at_tick = emitted_at_tick.saturating_add(fill_ticks);
    let expires_at_tick = full_at_tick.saturating_add(linger_ticks);
    let pulse_count = expires_at_tick.saturating_sub(emitted_at_tick) / pulse_interval_ticks;
    (
        full_at_tick,
        expires_at_tick,
        u8::try_from(pulse_count).unwrap_or(u8::MAX),
    )
}

#[cfg(feature = "server")]
pub(super) fn advance_composed_weapon_state(
    state: &mut WeaponState,
    recipe: &WeaponRecipe,
    tick: u64,
) -> bool {
    match state.phase {
        WeaponPhase::Cooldown { ready_at_tick } if tick >= ready_at_tick => {
            state.phase = WeaponPhase::Ready;
        }
        _ => {}
    }
    let Some(recovery) = state.ammo_recovery else {
        return false;
    };
    if tick < recovery.ready_at_tick {
        return false;
    }
    state.ammo = state.ammo.saturating_add(1).min(recipe.economy.capacity());
    state.ammo_recovery = None;
    true
}

#[cfg(feature = "server")]
fn delivery_angles(facing: f32, firing: FiringPattern) -> Vec<f32> {
    spread_angles(
        facing,
        match firing {
            FiringPattern::Single => 1,
            FiringPattern::Spread { delivery_count, .. } => delivery_count,
        },
        match firing {
            FiringPattern::Single => 0.0,
            FiringPattern::Spread {
                total_angle_degrees,
                ..
            } => total_angle_degrees,
        },
    )
}

#[cfg(feature = "server")]
fn requested_lob_distance(
    maximum_distance: f32,
    aim_distance: Option<crate::protocol::QuantizedAimDistance>,
) -> f32 {
    aim_distance.map_or(maximum_distance, |requested| {
        requested.to_world_units().clamp(0.0, maximum_distance)
    })
}

#[cfg(feature = "server")]
const MINIMUM_LOB_FLIGHT_TICKS: u64 = 6;

/// Scale lob time by the authoritative landing distance while retaining a short presentation and
/// replication floor. `max_flight_ticks` is the authored duration at maximum range.
#[cfg(feature = "server")]
fn resolved_lob_flight_ticks(
    maximum_distance: f32,
    landing_distance: f32,
    max_flight_ticks: u64,
) -> u64 {
    let proportional = ((landing_distance.clamp(0.0, maximum_distance) / maximum_distance)
        * max_flight_ticks as f32)
        .ceil() as u64;
    proportional.clamp(
        MINIMUM_LOB_FLIGHT_TICKS.min(max_flight_ticks),
        max_flight_ticks,
    )
}

#[cfg(feature = "server")]
fn resolved_lob_landing(
    origin: Vec2,
    facing: f32,
    aim_distance: Option<crate::protocol::QuantizedAimDistance>,
    recipe: &WeaponRecipe,
    bounds: &crate::map::PlayableBounds,
    spatial_query: &avian2d::prelude::SpatialQuery,
) -> Option<Vec2> {
    let (DeliveryMethod::Lobbed {
        distance,
        landing_clearance_radius,
        ..
    }
    | DeliveryMethod::Splash {
        distance,
        landing_clearance_radius,
        ..
    }) = recipe.delivery
    else {
        return None;
    };
    let requested_distance = requested_lob_distance(distance, aim_distance);
    let desired = origin + Vec2::from_angle(facing) * requested_distance;
    let bounded = desired.clamp(
        bounds.0.min + Vec2::splat(landing_clearance_radius),
        bounds.0.max - Vec2::splat(landing_clearance_radius),
    );
    let map_filter =
        avian2d::prelude::SpatialQueryFilter::from_mask(STATIC_MAP_LAYER | DESTRUCTIBLE_MAP_LAYER);
    delivery::repaired_landing_point(origin, bounded, landing_clearance_radius, |candidate| {
        spatial_query
            .shape_intersections(
                &Collider::circle(landing_clearance_radius),
                candidate,
                0.0,
                &map_filter,
            )
            .is_empty()
    })
}

#[cfg(feature = "server")]
#[derive(Clone, Copy)]
struct BlockedStraightDelivery {
    delivery_index: u8,
    target: Option<crate::map::DamageableTargetIdentity>,
    point: Vec2,
    normal: Vec2,
}

#[cfg(feature = "server")]
type DamageableMuzzleTargetQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static crate::map::DamageableTargetIdentity,
        &'static CurrentHealth,
        &'static crate::map::DamageableLifeState,
    ),
    Or<(
        With<crate::map::DamageableWorldObject>,
        With<crate::matchplay::HeistSafe>,
    )>,
>;

#[cfg(feature = "server")]
#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct MuzzleContactState<'w, 's> {
    spatial_query: avian2d::prelude::SpatialQuery<'w, 's>,
    objects: DamageableMuzzleTargetQuery<'w, 's>,
    world_pending: ResMut<'w, crate::map::PendingWorldTargetDamages>,
    objective_pending: ResMut<'w, crate::matchplay::PendingModeObjectiveDamages>,
    sticky_blobs: Query<'w, 's, &'static StickyBlobRuntime>,
    cone_sprays: Query<'w, 's, (), With<ConeSprayRuntime>>,
    splashes: Query<'w, 's, &'static PersistentSplashRuntime>,
    projectiles: Query<'w, 's, &'static ComposedProjectileRuntime>,
}

#[cfg(feature = "server")]
#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct AttackCommitBuffers<'w> {
    accepted_attacks: ResMut<'w, AcceptedAttackFacts>,
    outbox: ResMut<'w, CombatOutbox>,
}

#[cfg(feature = "server")]
fn blocked_straight_deliveries(
    origin: Vec2,
    facing: f32,
    recipe: &WeaponRecipe,
    spatial_query: &avian2d::prelude::SpatialQuery,
    objects: &DamageableMuzzleTargetQuery<'_, '_>,
) -> Vec<BlockedStraightDelivery> {
    let (DeliveryMethod::Straight {
        radius,
        muzzle_offset,
        ..
    }
    | DeliveryMethod::StickyStraight {
        radius,
        muzzle_offset,
        ..
    }) = recipe.delivery
    else {
        return Vec::new();
    };
    let body = ProjectileBody::circle(radius);
    delivery_angles(facing, recipe.firing)
        .into_iter()
        .enumerate()
        .filter_map(|(index, angle)| {
            let muzzle = muzzle_position(origin, angle, muzzle_offset);
            map_muzzle_contact(origin, muzzle, body, spatial_query).map(
                |(entity, point, normal)| BlockedStraightDelivery {
                    delivery_index: u8::try_from(index).unwrap_or(u8::MAX),
                    target: objects
                        .get(entity)
                        .ok()
                        .filter(|(_, health, life)| crate::map::object_is_live(**health, **life))
                        .map(|(identity, _, _)| *identity),
                    point,
                    normal,
                },
            )
        })
        .collect()
}

#[cfg(feature = "server")]
fn queue_blocked_world_target_damage(
    target: crate::map::DamageableTargetIdentity,
    point: Vec2,
    source: AttackSource,
    delivery_index: u8,
    recipe: &WeaponRecipe,
    world_pending: &mut crate::map::PendingWorldTargetDamages,
    objective_pending: &mut crate::matchplay::PendingModeObjectiveDamages,
) {
    for (bundle_index, bundle) in recipe
        .payload_bundles
        .iter()
        .enumerate()
        .filter(|(_, bundle)| matches!(bundle.target, TargetSelection::Direct))
    {
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
                    target,
                    source,
                    attack_id: source.attack_id,
                    requested_damage: effects::requested_damage(
                        amount,
                        falloff,
                        0.0,
                        1.0,
                        false,
                        source.origin.as_vec2().distance(point),
                    ),
                    delivery_index,
                    bundle_index: u8::try_from(bundle_index).unwrap_or(u8::MAX),
                    effect_index: u8::try_from(effect_index).unwrap_or(u8::MAX),
                },
            );
        }
    }
}

#[cfg(feature = "server")]
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
fn emit_attack_deliveries(
    commands: &mut Commands,
    tick: u64,
    owner_entity: Entity,
    origin: Vec2,
    facing: f32,
    resolved: &ResolvedWeapon,
    source: AttackSource,
    source_component: ProjectileSource,
    weapon_id: WeaponDefinitionId,
    blocked_deliveries: &[BlockedStraightDelivery],
    reserved_events: &[CombatEventId],
    blocked_event_cursor: &mut usize,
    legacy_telemetry: &mut CombatTelemetry,
    outbox: &mut CombatOutbox,
    melee: &mut MessageWriter<MeleeAttack>,
    world_pending: &mut crate::map::PendingWorldTargetDamages,
    objective_pending: &mut crate::matchplay::PendingModeObjectiveDamages,
    lob_landing: Option<Vec2>,
    match_member: Option<crate::matchplay::MatchMember>,
) -> u64 {
    let recipe = &resolved.recipe;
    let attack_id = source.attack_id;
    let network_id = &source.owner_network_entity_id;
    let legacy_compatibility = source.legacy_compatibility;
    let entity = owner_entity;
    let mut emitted_deliveries = 0_u64;
    match recipe.delivery {
        DeliveryMethod::Straight {
            speed,
            radius,
            range,
            lifetime_ticks,
            muzzle_offset,
        }
        | DeliveryMethod::StickyStraight {
            speed,
            radius,
            range,
            lifetime_ticks,
            muzzle_offset,
            ..
        } => {
            let body = ProjectileBody::circle(radius);
            debug_assert!(body.shape.is_valid(), "validated straight projectile body");
            let angles = delivery_angles(facing, recipe.firing);
            for (delivery_index, angle) in angles.into_iter().enumerate() {
                let delivery_index = u8::try_from(delivery_index).unwrap_or(u8::MAX);
                let muzzle = muzzle_position(origin, angle, muzzle_offset);
                if let Some(blocked) = blocked_deliveries
                    .iter()
                    .find(|blocked| blocked.delivery_index == delivery_index)
                {
                    let point = blocked.point;
                    let normal = blocked.normal;
                    if matches!(recipe.delivery, DeliveryMethod::StickyStraight { .. }) {
                        let sticky_entity = commands
                            .spawn((
                                source_component,
                                ReplicatedAttackSource { attack: source },
                                AttackDelivery {
                                    attack_id,
                                    delivery_index,
                                },
                                Position(point),
                                Rotation::radians(angle),
                                Replicate::to_clients(NetworkTarget::All),
                                InterpolationTarget::to_clients(NetworkTarget::All),
                            ))
                            .id();
                        let runtime = ComposedProjectileRuntime {
                            owner_entity: entity,
                            source_entity: entity,
                            source,
                            delivery_index,
                            velocity: Vec2::from_angle(angle) * speed,
                            travelled: origin.distance(point),
                            expires_at_tick: tick.saturating_add(lifetime_ticks),
                            maximum_range: range,
                            landing: None,
                            recipe: recipe.clone(),
                        };
                        let _ = sticky::arm_projectile(
                            commands,
                            sticky_entity,
                            runtime,
                            point,
                            None,
                            StickyBlobKind::Primary,
                            tick,
                        );
                        if let Some(match_member) = match_member {
                            commands.entity(sticky_entity).insert(match_member);
                        }
                        emitted_deliveries = emitted_deliveries.saturating_add(1);
                        continue;
                    }
                    if let Some(target) = blocked.target {
                        queue_blocked_world_target_damage(
                            target,
                            point,
                            source,
                            delivery_index,
                            recipe,
                            world_pending,
                            objective_pending,
                        );
                    }
                    let impact_event_id = reserved_events[*blocked_event_cursor];
                    *blocked_event_cursor += 1;
                    let impact_cue = CombatCue::DeliveryImpact {
                        event_id: impact_event_id,
                        tick,
                        attack_id,
                        delivery_index,
                        source: *network_id,
                        weapon_definition_id: weapon_id,
                        presentation_profile_id: resolved.presentation_profile_id,
                        target: None,
                        position: WorldPoint::from(point),
                        normal: WorldPoint::from(normal),
                        distance_band: distance_band(origin.distance(point)),
                    };
                    legacy_telemetry.record_cue(impact_cue.clone());
                    outbox.0.push(impact_cue);
                    if legacy_compatibility {
                        let legacy_event = reserved_events[*blocked_event_cursor];
                        *blocked_event_cursor += 1;
                        let legacy_cue = CombatCue::Impact {
                            event_id: legacy_event,
                            tick,
                            source: *network_id,
                            shot_id: ShotId(attack_id.0),
                            weapon_definition_id: weapon_id,
                            target: None,
                            position: WorldPoint::from(point),
                            normal: WorldPoint::from(normal),
                            distance_band: distance_band(origin.distance(point)),
                        };
                        legacy_telemetry.record_cue(legacy_cue.clone());
                        legacy_telemetry.record(CombatLogRecord::Hit {
                            tick,
                            event_id: legacy_event,
                            shot_id: ShotId(attack_id.0),
                            source: *network_id,
                            target: None,
                            weapon: weapon_id,
                            position: WorldPoint::from(point),
                            distance: origin.distance(point),
                            band: distance_band(origin.distance(point)),
                        });
                        outbox.0.push(legacy_cue);
                    }
                    emitted_deliveries = emitted_deliveries.saturating_add(1);
                    continue;
                }
                let mut projectile = commands.spawn((
                    Projectile,
                    source_component,
                    ReplicatedAttackSource { attack: source },
                    AttackDelivery {
                        attack_id,
                        delivery_index,
                    },
                    ProjectileDeadline {
                        expires_at_tick: tick.saturating_add(lifetime_ticks),
                    },
                    StraightFlight {
                        origin: WorldPoint::from(muzzle),
                        facing: angle,
                        speed,
                        maximum_range: range,
                        launched_at_tick: tick,
                    },
                    body,
                    ComposedProjectileRuntime {
                        owner_entity: entity,
                        source_entity: entity,
                        source,
                        delivery_index,
                        velocity: Vec2::from_angle(angle) * speed,
                        travelled: 0.0,
                        expires_at_tick: tick.saturating_add(lifetime_ticks),
                        maximum_range: range,
                        landing: None,
                        recipe: recipe.clone(),
                    },
                    Position::from_xy(muzzle.x, muzzle.y),
                    Rotation::radians(angle),
                    body.collider(),
                    CollisionLayers::new(
                        PROJECTILE_LAYER,
                        FIGHTER_LAYER | STATIC_MAP_LAYER | DESTRUCTIBLE_MAP_LAYER,
                    ),
                    Replicate::to_clients(NetworkTarget::All),
                    InterpolationTarget::to_clients(NetworkTarget::All),
                ));
                if let Some(match_member) = match_member {
                    projectile.insert(match_member);
                }
                emitted_deliveries = emitted_deliveries.saturating_add(1);
            }
        }
        DeliveryMethod::Lobbed {
            distance,
            max_flight_ticks,
            visual_arc_height,
            landing_clearance_radius: _,
            muzzle_offset,
        } => {
            let landing = lob_landing.expect("validated lob landing must exist");
            let launch = muzzle_position(origin, facing, muzzle_offset);
            let flight_ticks =
                resolved_lob_flight_ticks(distance, origin.distance(landing), max_flight_ticks);
            let mut projectile = commands.spawn((
                Projectile,
                source_component,
                ReplicatedAttackSource { attack: source },
                AttackDelivery {
                    attack_id,
                    delivery_index: 0,
                },
                ProjectileDeadline {
                    expires_at_tick: tick.saturating_add(flight_ticks),
                },
                LobbedFlight {
                    launch: WorldPoint::from(launch),
                    landing: WorldPoint::from(landing),
                    launched_at_tick: tick,
                    lands_at_tick: tick.saturating_add(flight_ticks),
                    visual_arc_height,
                },
                ComposedProjectileRuntime {
                    owner_entity: entity,
                    source_entity: entity,
                    source,
                    delivery_index: 0,
                    velocity: Vec2::ZERO,
                    travelled: 0.0,
                    expires_at_tick: tick.saturating_add(flight_ticks),
                    maximum_range: distance,
                    landing: Some(landing),
                    recipe: recipe.clone(),
                },
                Position::from_xy(launch.x, launch.y),
                Rotation::radians(facing),
                Replicate::to_clients(NetworkTarget::All),
                InterpolationTarget::to_clients(NetworkTarget::All),
            ));
            if let Some(match_member) = match_member {
                projectile.insert(match_member);
            }
            emitted_deliveries = 1;
        }
        DeliveryMethod::Splash {
            distance,
            max_flight_ticks,
            visual_arc_height,
            muzzle_offset,
            duration_ticks,
            pulse_interval_ticks,
            ..
        } => {
            let landing = lob_landing.expect("validated Splash landing must exist");
            let launch = muzzle_position(origin, facing, muzzle_offset);
            let flight_ticks =
                resolved_lob_flight_ticks(distance, origin.distance(landing), max_flight_ticks);
            let mut projectile = commands.spawn((
                Projectile,
                source_component,
                ReplicatedAttackSource { attack: source },
                AttackDelivery {
                    attack_id,
                    delivery_index: 0,
                },
                ProjectileDeadline {
                    expires_at_tick: tick.saturating_add(flight_ticks),
                },
                LobbedFlight {
                    launch: WorldPoint::from(launch),
                    landing: WorldPoint::from(landing),
                    launched_at_tick: tick,
                    lands_at_tick: tick.saturating_add(flight_ticks),
                    visual_arc_height,
                },
                ComposedProjectileRuntime {
                    owner_entity: entity,
                    source_entity: entity,
                    source,
                    delivery_index: 0,
                    velocity: Vec2::ZERO,
                    travelled: 0.0,
                    expires_at_tick: tick.saturating_add(flight_ticks),
                    maximum_range: distance,
                    landing: Some(landing),
                    recipe: recipe.clone(),
                },
                Position::from_xy(launch.x, launch.y),
                Rotation::radians(facing),
                Replicate::to_clients(NetworkTarget::All),
                InterpolationTarget::to_clients(NetworkTarget::All),
            ));
            if let Some(match_member) = match_member {
                projectile.insert(match_member);
            }
            let (_, pulse_count) = splash::splash_timing(
                tick.saturating_add(flight_ticks),
                duration_ticks,
                pulse_interval_ticks,
            );
            emitted_deliveries = u64::from(pulse_count).saturating_add(1);
        }
        DeliveryMethod::MeleeArc { .. } => {
            melee.write(MeleeAttack {
                source,
                origin,
                facing,
                tick,
                recipe: recipe.clone(),
            });
            emitted_deliveries = 1;
        }
        DeliveryMethod::ConeSpray {
            propagation_speed,
            reach,
            angle_degrees,
            linger_ticks,
            pulse_interval_ticks,
            map_occlusion,
            max_targets,
        } => {
            let (full_at_tick, expires_at_tick, pulse_count) = cone_spray_timing(
                tick,
                propagation_speed,
                reach,
                linger_ticks,
                pulse_interval_ticks,
            );
            let state = ConeSprayState {
                origin: WorldPoint::from(origin),
                facing,
                propagation_speed,
                maximum_reach: reach,
                angle_degrees,
                emitted_at_tick: tick,
                full_at_tick,
                expires_at_tick,
                pulse_interval_ticks,
                map_occlusion,
                max_targets,
            };
            let mut spray = commands.spawn((
                ConeSpray,
                state,
                ReplicatedAttackSource { attack: source },
                AttackDelivery {
                    attack_id,
                    delivery_index: 0,
                },
                ConeSprayRuntime {
                    owner_entity: entity,
                    source,
                    recipe: recipe.clone(),
                    next_pulse_tick: tick.saturating_add(pulse_interval_ticks),
                    next_delivery_index: 0,
                    match_id: match_member.map(|member| member.0),
                },
                Replicate::to_clients(NetworkTarget::All),
            ));
            if let Some(match_member) = match_member {
                spray.insert(match_member);
            }
            emitted_deliveries = u64::from(pulse_count);
        }
    }
    emitted_deliveries
}

#[cfg(feature = "server")]
struct AcceptedAttackRecord<'a> {
    tick: u64,
    source: AttackSource,
    weapon_definition_id: WeaponDefinitionId,
    event_id: CombatEventId,
    legacy_muzzle_event: Option<CombatEventId>,
    origin: Vec2,
    facing: f32,
    ammo_after: u8,
    blocked_delivery_count: usize,
    emitted_deliveries: u64,
    recipe: &'a WeaponRecipe,
}

#[cfg(feature = "server")]
#[allow(
    clippy::needless_pass_by_value,
    reason = "the record is a small copied fact bundle owned by this recording step"
)]
#[allow(clippy::too_many_lines)]
fn record_accepted_attack(
    record: AcceptedAttackRecord<'_>,
    evidence_enabled: bool,
    trackers: &mut ActiveAttackTrackers,
    telemetry: &mut WeaponTelemetry,
    legacy_telemetry: &mut CombatTelemetry,
    accepted_attacks: &mut AcceptedAttackFacts,
    outbox: &mut CombatOutbox,
) {
    let AcceptedAttackRecord {
        tick,
        source,
        weapon_definition_id,
        event_id,
        legacy_muzzle_event,
        origin,
        facing,
        ammo_after,
        blocked_delivery_count,
        emitted_deliveries,
        recipe,
    } = record;
    let attack_id = source.attack_id;
    let preset_id = source
        .source_preset_id
        .unwrap_or(WeaponPresetId(weapon_definition_id.0));
    telemetry.record_emitted_deliveries(preset_id, source.recipe_fingerprint, emitted_deliveries);
    if emitted_deliveries > 0 {
        if trackers.active.len() < server::MAX_ACTIVE_ATTACK_TRACKERS {
            trackers.active.insert(
                attack_id,
                ActiveAttackTracker {
                    source,
                    expected_deliveries: u8::try_from(emitted_deliveries).unwrap_or(u8::MAX),
                    resolved_deliveries: 0,
                    had_hostile_contact: false,
                },
            );
        } else {
            telemetry.tracker_drops = telemetry.tracker_drops.saturating_add(1);
        }
    }
    for _ in 0..blocked_delivery_count {
        finish_attack_delivery(trackers, attack_id);
    }
    telemetry.record_accepted_attack(preset_id, source.recipe_fingerprint);
    legacy_telemetry.accepted_shots = legacy_telemetry.accepted_shots.saturating_add(1);
    let recorded = accepted_attacks.record(AcceptedAttackFact {
        event_id,
        tick,
        attack_id,
        source_network_id: source.owner_network_entity_id,
    });
    debug_assert!(recorded, "attack fact capacity was checked before commit");
    let accepted_cue = CombatCue::AttackAccepted {
        event_id,
        tick,
        attack_id,
        source: source.owner_network_entity_id,
        position: WorldPoint::from(origin),
        weapon_definition_id,
        presentation_profile_id: source.presentation_profile_id,
    };
    legacy_telemetry.record_cue(accepted_cue.clone());
    outbox.0.push(accepted_cue);
    if evidence_enabled {
        if legacy_telemetry.accepted_shot_timestamps.len() < MAX_COMBAT_EVIDENCE_EVENTS {
            legacy_telemetry
                .accepted_shot_timestamps
                .push((ShotId(attack_id.0), unix_epoch_micros()));
        } else {
            legacy_telemetry.dropped_accepted_shot_timestamps = legacy_telemetry
                .dropped_accepted_shot_timestamps
                .saturating_add(1);
        }
    }
    if let Some(muzzle_event) = legacy_muzzle_event {
        let muzzle = muzzle_position(
            origin,
            facing,
            match recipe.delivery {
                DeliveryMethod::Straight { muzzle_offset, .. }
                | DeliveryMethod::StickyStraight { muzzle_offset, .. }
                | DeliveryMethod::Lobbed { muzzle_offset, .. }
                | DeliveryMethod::Splash { muzzle_offset, .. } => muzzle_offset,
                DeliveryMethod::MeleeArc { .. } | DeliveryMethod::ConeSpray { .. } => 0.0,
            },
        );
        legacy_telemetry.record(CombatLogRecord::Shot {
            event_id: muzzle_event,
            tick,
            shot_id: ShotId(attack_id.0),
            source: source.owner_network_entity_id,
            weapon: weapon_definition_id,
            muzzle_position: WorldPoint::from(muzzle),
            ammo_after,
        });
        let muzzle_cue = CombatCue::Muzzle {
            event_id: muzzle_event,
            tick,
            source: source.owner_network_entity_id,
            shot_id: ShotId(attack_id.0),
            weapon_definition_id,
            position: WorldPoint::from(muzzle),
        };
        legacy_telemetry.record_cue(muzzle_cue.clone());
        outbox.0.push(muzzle_cue);
    }
    telemetry.record(WeaponTelemetryRecord {
        tick,
        event_id,
        attack_id,
        preset_id,
        recipe_fingerprint: source.recipe_fingerprint,
        delivery_index: None,
        source: source.owner_network_entity_id,
        target: None,
        position: WorldPoint::from(origin),
        requested_value: 0,
        applied_value: 0,
        engagement_distance: 0.0,
        delivery_travel: 0.0,
        hostile_contact: false,
        effect: None,
        resulting_health: None,
        resulting_effects: None,
        resulting_motion: None,
        outcome: WeaponTelemetryOutcome::AttackAccepted,
    });
}

#[cfg(feature = "server")]
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
#[allow(
    clippy::type_complexity,
    reason = "the query declares this system's complete world view inline at its schedule boundary"
)]
pub(super) fn authoritative_composed_fire(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    bounds: Res<crate::map::PlayableBounds>,
    mut muzzle_contacts: MuzzleContactState,
    disconnected: Query<Entity, (With<LinkOf>, With<lightyear::prelude::Disconnected>)>,
    mut ids: ResMut<NextCombatIds>,
    mut gameplay_telemetry: AbilityWeaponTelemetry,
    mut legacy_telemetry: ResMut<CombatTelemetry>,
    evidence: Res<CombatEvidenceMode>,
    mut trackers: ResMut<ActiveAttackTrackers>,
    mut commit_buffers: AttackCommitBuffers,
    mut melee: MessageWriter<MeleeAttack>,
    active_combatants: Query<(), With<crate::matchplay::ActiveCombatant>>,
    dashing: Query<(), With<crate::abilities::DashRuntime>>,
    mut passive_states: Query<&mut crate::builds::PassiveRuntimeState>,
    query: Query<
        (
            (Entity, &Position, &Rotation),
            &crate::builds::SelectedBuild,
            &crate::builds::ResolvedMatchLoadout,
            &TeamId,
            &PlayerId,
            &NetworkEntityId,
            Option<&lightyear::prelude::ControlledBy>,
            &crate::movement::InputFreshness,
            (&mut WeaponState, &mut HealthRecoveryState),
            Option<&ActionState<FighterInput>>,
            Option<&Defeated>,
            Option<&crate::matchplay::MatchParticipant>,
        ),
        With<Fighter>,
    >,
) {
    let disconnected: HashSet<_> = disconnected.iter().collect();
    for (
        (entity, position, rotation),
        _build_identity,
        loadout,
        team,
        player_id,
        network_id,
        controlled_by,
        freshness,
        (mut state, mut health_recovery),
        action,
        defeated,
        match_participant,
    ) in query
    {
        if defeated.is_some()
            || (match_participant.is_some() && !active_combatants.contains(entity))
        {
            continue;
        }
        let resolved = &loadout.primary_weapon;
        let recipe = &resolved.recipe;
        advance_composed_weapon_state(&mut state, recipe, tick.0);
        ensure_ammo_recovery(
            entity,
            *network_id,
            tick.0,
            &mut state,
            recipe,
            &mut passive_states,
            &mut gameplay_telemetry.ability,
        );
        if dashing.contains(entity)
            || controlled_by.is_some_and(|controlled| disconnected.contains(&controlled.owner))
        {
            continue;
        }
        let input = action.map_or(FighterInput::default(), |value| value.0);
        let targeted_ultimate_requested = input.gameplay_buttons & FighterInput::ULTIMATE != 0
            && loadout.ultimate.kind.activation_style()
                == crate::builds::UltimateActivationStyle::Targeted;
        let held = !input_should_neutralize(tick.0, freshness.last_fresh_tick, 12)
            && input.is_valid()
            && !targeted_ultimate_requested
            && input.gameplay_buttons & FighterInput::PRIMARY_FIRE != 0;
        if !held || !matches!(state.phase, WeaponPhase::Ready) {
            continue;
        }
        if state.ammo == 0 {
            continue;
        }
        if !commit_buffers.accepted_attacks.has_capacity() {
            continue;
        }
        let origin = position.0;
        let facing = rotation.as_radians();
        let lob_landing = resolved_lob_landing(
            origin,
            facing,
            input.aim_distance,
            recipe,
            &bounds,
            &muzzle_contacts.spatial_query,
        );
        if matches!(
            recipe.delivery,
            DeliveryMethod::Lobbed { .. } | DeliveryMethod::Splash { .. }
        ) && lob_landing.is_none()
        {
            continue;
        }
        let legacy_compatibility = legacy_compatibility_recipe(recipe);
        let blocked_deliveries = blocked_straight_deliveries(
            origin,
            facing,
            recipe,
            &muzzle_contacts.spatial_query,
            &muzzle_contacts.objects,
        );
        let sticky_delivery = matches!(recipe.delivery, DeliveryMethod::StickyStraight { .. });
        if matches!(recipe.delivery, DeliveryMethod::ConeSpray { .. })
            && muzzle_contacts.cone_sprays.iter().count() >= MAX_ACTIVE_CONE_SPRAYS
        {
            continue;
        }
        if let DeliveryMethod::Splash {
            max_active_per_owner,
            ..
        } = recipe.delivery
        {
            let active_for_owner = muzzle_contacts
                .splashes
                .iter()
                .filter(|splash| splash.source.owner_network_entity_id == *network_id)
                .count()
                + muzzle_contacts
                    .projectiles
                    .iter()
                    .filter(|projectile| {
                        projectile.source.owner_network_entity_id == *network_id
                            && matches!(projectile.recipe.delivery, DeliveryMethod::Splash { .. })
                    })
                    .count();
            let active_total = muzzle_contacts.splashes.iter().count()
                + muzzle_contacts
                    .projectiles
                    .iter()
                    .filter(|projectile| {
                        matches!(projectile.recipe.delivery, DeliveryMethod::Splash { .. })
                    })
                    .count();
            if active_for_owner >= usize::from(max_active_per_owner)
                || active_total >= splash::MAX_ACTIVE_PERSISTENT_SPLASHES
            {
                continue;
            }
        }
        if sticky_delivery
            && !blocked_deliveries.is_empty()
            && let DeliveryMethod::StickyStraight {
                max_active_per_owner,
                ..
            } = recipe.delivery
        {
            let owner_active = muzzle_contacts
                .sticky_blobs
                .iter()
                .filter(|blob| blob.source.owner_network_entity_id == *network_id)
                .count();
            if owner_active >= usize::from(max_active_per_owner)
                || muzzle_contacts.sticky_blobs.iter().count() >= sticky::MAX_ACTIVE_STICKY_BLOBS
            {
                continue;
            }
        }
        let per_blocked_delivery_events = if sticky_delivery {
            0
        } else if legacy_compatibility {
            2
        } else {
            1
        };
        let event_count = 1
            + usize::from(legacy_compatibility)
            + blocked_deliveries.len() * per_blocked_delivery_events;
        let Some((attack_id, reserved_events)) =
            server::reserve_attack_and_events(&mut ids, event_count)
        else {
            continue;
        };
        let event_id = reserved_events[0];
        commands
            .entity(entity)
            .remove::<crate::matchplay::SpawnProtection>();
        let legacy_muzzle_event = if legacy_compatibility {
            Some(reserved_events[1])
        } else {
            None
        };
        let mut blocked_event_cursor = 1 + usize::from(legacy_compatibility);
        state.ammo = state.ammo.saturating_sub(1);
        state.phase = WeaponPhase::Cooldown {
            ready_at_tick: tick.0.saturating_add(recipe.fire_cooldown_ticks),
        };
        ensure_ammo_recovery(
            entity,
            *network_id,
            tick.0,
            &mut state,
            recipe,
            &mut passive_states,
            &mut gameplay_telemetry.ability,
        );
        health_recovery.last_accepted_attack_tick = tick.0;
        health_recovery.recovery_remainder = 0;
        let preset_id = resolved.source_preset_id;
        let source = AttackSource {
            kind: CombatSourceKind::PrimaryWeapon,
            attack_id,
            player_id: *player_id,
            owner_network_entity_id: *network_id,
            team_id: *team,
            recipe_fingerprint: resolved.recipe_fingerprint,
            presentation_profile_id: resolved.presentation_profile_id,
            legacy_compatibility,
            source_preset_id: preset_id,
            origin: WorldPoint::from(origin),
            facing,
        };
        let weapon_id = resolved
            .source_preset_id
            .map_or(WeaponDefinitionId(1), |id| WeaponDefinitionId(id.0));
        let source_component = ProjectileSource {
            shot_id: ShotId(attack_id.0),
            player_id: *player_id,
            owner_network_entity_id: *network_id,
            team_id: *team,
            weapon_definition_id: weapon_id,
        };
        let emitted_deliveries = emit_attack_deliveries(
            &mut commands,
            tick.0,
            entity,
            origin,
            facing,
            resolved,
            source,
            source_component,
            weapon_id,
            &blocked_deliveries,
            &reserved_events,
            &mut blocked_event_cursor,
            &mut legacy_telemetry,
            &mut commit_buffers.outbox,
            &mut melee,
            &mut muzzle_contacts.world_pending,
            &mut muzzle_contacts.objective_pending,
            lob_landing,
            match_participant
                .map(|participant| crate::matchplay::MatchMember(participant.match_id)),
        );
        record_accepted_attack(
            AcceptedAttackRecord {
                tick: tick.0,
                source,
                weapon_definition_id: weapon_id,
                event_id,
                legacy_muzzle_event,
                origin,
                facing,
                ammo_after: state.ammo,
                blocked_delivery_count: if sticky_delivery {
                    0
                } else {
                    blocked_deliveries.len()
                },
                emitted_deliveries,
                recipe,
            },
            evidence.enabled,
            &mut trackers,
            &mut gameplay_telemetry.weapon,
            &mut legacy_telemetry,
            &mut commit_buffers.accepted_attacks,
            &mut commit_buffers.outbox,
        );
    }
}

#[cfg(feature = "server")]
fn consume_quick_cycle(
    entity: Entity,
    owner_network_id: NetworkEntityId,
    tick: u64,
    base_ticks: u64,
    states: &mut Query<&mut crate::builds::PassiveRuntimeState>,
    telemetry: &mut crate::abilities::AbilityTelemetry,
) -> u64 {
    let Ok(mut state) = states.get_mut(entity) else {
        return base_ticks;
    };
    let ticks = consume_quick_cycle_state(&mut state, base_ticks);
    if ticks < base_ticks {
        telemetry.record(crate::abilities::AbilityTelemetryRecord {
            tick,
            owner_network_id,
            kind: crate::abilities::AbilityTelemetryKind::PassiveModified {
                passive_id: crate::builds::PassiveDefinitionId(5),
                amount: u16::try_from(base_ticks.saturating_sub(ticks)).unwrap_or(u16::MAX),
            },
        });
    }
    ticks
}

#[cfg(feature = "server")]
fn ensure_ammo_recovery(
    entity: Entity,
    owner_network_id: NetworkEntityId,
    tick: u64,
    state: &mut WeaponState,
    recipe: &WeaponRecipe,
    passive_states: &mut Query<&mut crate::builds::PassiveRuntimeState>,
    telemetry: &mut crate::abilities::AbilityTelemetry,
) {
    if state.ammo >= recipe.economy.capacity() || state.ammo_recovery.is_some() {
        return;
    }
    let duration = consume_quick_cycle(
        entity,
        owner_network_id,
        tick,
        recipe.economy.refill_ticks(),
        passive_states,
        telemetry,
    );
    state.ammo_recovery = Some(AmmoRecovery {
        started_at_tick: tick,
        ready_at_tick: tick.saturating_add(duration),
    });
}

#[cfg(feature = "server")]
fn consume_quick_cycle_state(
    state: &mut crate::builds::PassiveRuntimeState,
    base_ticks: u64,
) -> u64 {
    if !state.quick_cycle_primed {
        return base_ticks;
    }
    state.quick_cycle_primed = false;
    crate::abilities::apply_quick_cycle_ticks(base_ticks)
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;
    #[test]
    fn quick_cycle_is_consumed_once_for_magazine_and_charge_refills() {
        for economy in [
            WeaponEconomy::Magazine {
                capacity: 6,
                refill_ticks: 60,
            },
            WeaponEconomy::Charges {
                capacity: 2,
                recharge_ticks: 60,
            },
        ] {
            let mut state = crate::builds::PassiveRuntimeState {
                quick_cycle_primed: true,
                ..Default::default()
            };
            assert_eq!(
                consume_quick_cycle_state(&mut state, economy.refill_ticks()),
                36
            );
            assert!(!state.quick_cycle_primed);
            assert_eq!(
                consume_quick_cycle_state(&mut state, economy.refill_ticks()),
                60
            );
        }
    }

    #[test]
    fn lob_focal_distance_is_bounded_by_the_authored_maximum() {
        use crate::protocol::QuantizedAimDistance;

        assert!((requested_lob_distance(520.0, None) - 520.0).abs() < f32::EPSILON);
        assert!(
            (requested_lob_distance(520.0, Some(QuantizedAimDistance(180))) - 180.0).abs()
                < f32::EPSILON
        );
        assert!(
            (requested_lob_distance(520.0, Some(QuantizedAimDistance(900))) - 520.0).abs()
                < f32::EPSILON
        );
    }

    #[test]
    fn lob_flight_time_scales_with_landing_distance_and_keeps_a_short_floor() {
        assert_eq!(resolved_lob_flight_ticks(520.0, 520.0, 45), 45);
        assert_eq!(resolved_lob_flight_ticks(520.0, 260.0, 45), 23);
        assert_eq!(resolved_lob_flight_ticks(520.0, 1.0, 45), 6);
        assert_eq!(resolved_lob_flight_ticks(520.0, 0.0, 45), 6);
        assert_eq!(resolved_lob_flight_ticks(520.0, 900.0, 45), 45);
    }

    #[test]
    fn cone_spray_timing_includes_fill_and_linger_pulses() {
        let (full_at, expires_at, pulses) = cone_spray_timing(100, 480.0, 240.0, 30, 10);
        assert_eq!(full_at, 130);
        assert_eq!(expires_at, 160);
        assert_eq!(pulses, 6);
    }
}
