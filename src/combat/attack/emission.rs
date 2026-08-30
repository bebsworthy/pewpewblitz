//! Deterministic delivery-family emission for accepted primary attacks.

#[allow(clippy::wildcard_imports)]
use super::*;

struct DeliveryEmissionRequest<'a> {
    tick: u64,
    owner_entity: Entity,
    origin: Vec2,
    facing: f32,
    recipe: &'a WeaponRecipe,
    source: AttackSource,
    source_component: ProjectileSource,
    weapon_id: WeaponDefinitionId,
    blocked_deliveries: &'a [BlockedStraightDelivery],
    reserved_events: &'a [CombatEventId],
    lob_landing: Option<Vec2>,
    match_member: Option<crate::matchplay::MatchMember>,
    minimum_lob_flight_ticks: u64,
}

#[allow(
    clippy::too_many_arguments,
    reason = "the accepted-attack transaction supplies distinct Bevy command and message sinks"
)]
pub(super) fn emit_attack_deliveries(
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
    minimum_lob_flight_ticks: u64,
) -> u64 {
    let request = DeliveryEmissionRequest {
        tick,
        owner_entity,
        origin,
        facing,
        recipe: &resolved.recipe,
        source,
        source_component,
        weapon_id,
        blocked_deliveries,
        reserved_events,
        lob_landing,
        match_member,
        minimum_lob_flight_ticks,
    };
    match request.recipe.delivery {
        DeliveryMethod::Straight { .. } | DeliveryMethod::StickyStraight { .. } => {
            emit_straight_or_sticky(
                commands,
                &request,
                blocked_event_cursor,
                legacy_telemetry,
                outbox,
                world_pending,
                objective_pending,
            )
        }
        DeliveryMethod::Lobbed { .. } => emit_lobbed(commands, &request),
        DeliveryMethod::Splash { .. } => emit_splash(commands, &request),
        DeliveryMethod::MeleeArc { .. } => emit_melee(&request, melee),
        DeliveryMethod::ConeSpray { .. } => emit_cone_spray(commands, &request),
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "blocked contact publishes to authoritative damage, cue, and telemetry sinks"
)]
fn emit_straight_or_sticky(
    commands: &mut Commands,
    request: &DeliveryEmissionRequest<'_>,
    blocked_event_cursor: &mut usize,
    legacy_telemetry: &mut CombatTelemetry,
    outbox: &mut CombatOutbox,
    world_pending: &mut crate::map::PendingWorldTargetDamages,
    objective_pending: &mut crate::matchplay::PendingModeObjectiveDamages,
) -> u64 {
    let (DeliveryMethod::Straight {
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
    }) = request.recipe.delivery
    else {
        unreachable!("straight-family helper requires a straight delivery");
    };
    let body = ProjectileBody::circle(radius);
    debug_assert!(body.shape.is_valid(), "validated straight projectile body");
    let mut emitted_deliveries = 0_u64;
    for (delivery_index, angle) in delivery_angles(request.facing, request.recipe.firing)
        .into_iter()
        .enumerate()
    {
        let delivery_index = u8::try_from(delivery_index).unwrap_or(u8::MAX);
        let muzzle = muzzle_position(request.origin, angle, muzzle_offset);
        if let Some(blocked) = request
            .blocked_deliveries
            .iter()
            .find(|blocked| blocked.delivery_index == delivery_index)
        {
            if matches!(
                request.recipe.delivery,
                DeliveryMethod::StickyStraight { .. }
            ) {
                arm_blocked_sticky(
                    commands,
                    request,
                    *blocked,
                    delivery_index,
                    angle,
                    speed,
                    range,
                    lifetime_ticks,
                );
            } else {
                publish_blocked_straight_contact(
                    request,
                    *blocked,
                    delivery_index,
                    blocked_event_cursor,
                    legacy_telemetry,
                    outbox,
                    world_pending,
                    objective_pending,
                );
            }
        } else {
            spawn_straight_projectile(
                commands,
                request,
                body,
                delivery_index,
                angle,
                muzzle,
                speed,
                range,
                lifetime_ticks,
            );
        }
        emitted_deliveries = emitted_deliveries.saturating_add(1);
    }
    emitted_deliveries
}

#[allow(clippy::too_many_arguments)]
fn arm_blocked_sticky(
    commands: &mut Commands,
    request: &DeliveryEmissionRequest<'_>,
    blocked: BlockedStraightDelivery,
    delivery_index: u8,
    angle: f32,
    speed: f32,
    range: f32,
    lifetime_ticks: u64,
) {
    let sticky_entity = commands
        .spawn((
            request.source_component,
            ReplicatedAttackSource {
                attack: request.source,
            },
            AttackDelivery {
                attack_id: request.source.attack_id,
                delivery_index,
            },
            Position(blocked.point),
            Rotation::radians(angle),
            Replicate::to_clients(NetworkTarget::All),
            InterpolationTarget::to_clients(NetworkTarget::All),
        ))
        .id();
    let runtime = ComposedProjectileRuntime {
        owner_entity: request.owner_entity,
        source_entity: request.owner_entity,
        source: request.source,
        delivery_index,
        velocity: Vec2::from_angle(angle) * speed,
        travelled: request.origin.distance(blocked.point),
        expires_at_tick: request.tick.saturating_add(lifetime_ticks),
        maximum_range: range,
        landing: None,
        recipe: request.recipe.clone(),
    };
    let _ = sticky::arm_projectile(
        commands,
        sticky_entity,
        runtime,
        blocked.point,
        None,
        StickyBlobKind::Primary,
        request.tick,
    );
    if let Some(match_member) = request.match_member {
        commands.entity(sticky_entity).insert(match_member);
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "contact publication preserves the existing ordered authoritative sinks"
)]
fn publish_blocked_straight_contact(
    request: &DeliveryEmissionRequest<'_>,
    blocked: BlockedStraightDelivery,
    delivery_index: u8,
    blocked_event_cursor: &mut usize,
    legacy_telemetry: &mut CombatTelemetry,
    outbox: &mut CombatOutbox,
    world_pending: &mut crate::map::PendingWorldTargetDamages,
    objective_pending: &mut crate::matchplay::PendingModeObjectiveDamages,
) {
    if let Some(target) = blocked.target {
        queue_blocked_world_target_damage(
            target,
            blocked.point,
            request.source,
            delivery_index,
            request.recipe,
            world_pending,
            objective_pending,
        );
    }
    let impact_event_id = request.reserved_events[*blocked_event_cursor];
    *blocked_event_cursor += 1;
    let distance = request.origin.distance(blocked.point);
    let impact_cue = CombatCue::DeliveryImpact {
        event_id: impact_event_id,
        tick: request.tick,
        attack_id: request.source.attack_id,
        delivery_index,
        source: request.source.owner_network_entity_id,
        weapon_definition_id: request.weapon_id,
        target: None,
        position: WorldPoint::from(blocked.point),
        normal: WorldPoint::from(blocked.normal),
        distance_band: distance_band(distance),
    };
    legacy_telemetry.record_cue(impact_cue.clone());
    outbox.0.push(impact_cue);
    if request.source.legacy_compatibility {
        publish_legacy_blocked_impact(
            request,
            blocked,
            distance,
            blocked_event_cursor,
            legacy_telemetry,
            outbox,
        );
    }
}

fn publish_legacy_blocked_impact(
    request: &DeliveryEmissionRequest<'_>,
    blocked: BlockedStraightDelivery,
    distance: f32,
    blocked_event_cursor: &mut usize,
    legacy_telemetry: &mut CombatTelemetry,
    outbox: &mut CombatOutbox,
) {
    let event_id = request.reserved_events[*blocked_event_cursor];
    *blocked_event_cursor += 1;
    let cue = CombatCue::Impact {
        event_id,
        tick: request.tick,
        source: request.source.owner_network_entity_id,
        shot_id: ShotId(request.source.attack_id.0),
        weapon_definition_id: request.weapon_id,
        target: None,
        position: WorldPoint::from(blocked.point),
        normal: WorldPoint::from(blocked.normal),
        distance_band: distance_band(distance),
    };
    legacy_telemetry.record_cue(cue.clone());
    legacy_telemetry.record(CombatLogRecord::Hit {
        tick: request.tick,
        event_id,
        shot_id: ShotId(request.source.attack_id.0),
        source: request.source.owner_network_entity_id,
        target: None,
        weapon: request.weapon_id,
        position: WorldPoint::from(blocked.point),
        distance,
        band: distance_band(distance),
    });
    outbox.0.push(cue);
}

#[allow(clippy::too_many_arguments)]
fn spawn_straight_projectile(
    commands: &mut Commands,
    request: &DeliveryEmissionRequest<'_>,
    body: ProjectileBody,
    delivery_index: u8,
    angle: f32,
    muzzle: Vec2,
    speed: f32,
    range: f32,
    lifetime_ticks: u64,
) {
    let mut projectile = commands.spawn((
        Projectile,
        request.source_component,
        ReplicatedAttackSource {
            attack: request.source,
        },
        AttackDelivery {
            attack_id: request.source.attack_id,
            delivery_index,
        },
        ProjectileDeadline {
            expires_at_tick: request.tick.saturating_add(lifetime_ticks),
        },
        StraightFlight {
            origin: WorldPoint::from(muzzle),
            facing: angle,
            speed,
            maximum_range: range,
            launched_at_tick: request.tick,
        },
        body,
        ComposedProjectileRuntime {
            owner_entity: request.owner_entity,
            source_entity: request.owner_entity,
            source: request.source,
            delivery_index,
            velocity: Vec2::from_angle(angle) * speed,
            travelled: 0.0,
            expires_at_tick: request.tick.saturating_add(lifetime_ticks),
            maximum_range: range,
            landing: None,
            recipe: request.recipe.clone(),
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
    if let Some(match_member) = request.match_member {
        projectile.insert(match_member);
    }
}

fn emit_lobbed(commands: &mut Commands, request: &DeliveryEmissionRequest<'_>) -> u64 {
    let DeliveryMethod::Lobbed {
        distance,
        max_flight_ticks,
        visual_arc_height,
        muzzle_offset,
        ..
    } = request.recipe.delivery
    else {
        unreachable!("lobbed helper requires a lobbed delivery");
    };
    spawn_lobbed_projectile(
        commands,
        request,
        request
            .lob_landing
            .expect("validated lob landing must exist"),
        distance,
        max_flight_ticks,
        visual_arc_height,
        muzzle_offset,
    );
    1
}

fn emit_splash(commands: &mut Commands, request: &DeliveryEmissionRequest<'_>) -> u64 {
    let DeliveryMethod::Splash {
        distance,
        max_flight_ticks,
        visual_arc_height,
        muzzle_offset,
        duration_ticks,
        pulse_interval_ticks,
        ..
    } = request.recipe.delivery
    else {
        unreachable!("splash helper requires a splash delivery");
    };
    let flight_ticks = spawn_lobbed_projectile(
        commands,
        request,
        request
            .lob_landing
            .expect("validated Splash landing must exist"),
        distance,
        max_flight_ticks,
        visual_arc_height,
        muzzle_offset,
    );
    let (_, pulse_count) = splash::splash_timing(
        request.tick.saturating_add(flight_ticks),
        duration_ticks,
        pulse_interval_ticks,
    );
    u64::from(pulse_count).saturating_add(1)
}

#[allow(clippy::too_many_arguments)]
fn spawn_lobbed_projectile(
    commands: &mut Commands,
    request: &DeliveryEmissionRequest<'_>,
    landing: Vec2,
    distance: f32,
    max_flight_ticks: u64,
    visual_arc_height: f32,
    muzzle_offset: f32,
) -> u64 {
    let launch = muzzle_position(request.origin, request.facing, muzzle_offset);
    let flight_ticks = resolved_lob_flight_ticks(
        distance,
        request.origin.distance(landing),
        request.minimum_lob_flight_ticks,
        max_flight_ticks,
    );
    let mut projectile = commands.spawn((
        Projectile,
        request.source_component,
        ReplicatedAttackSource {
            attack: request.source,
        },
        AttackDelivery {
            attack_id: request.source.attack_id,
            delivery_index: 0,
        },
        ProjectileDeadline {
            expires_at_tick: request.tick.saturating_add(flight_ticks),
        },
        LobbedFlight {
            launch: WorldPoint::from(launch),
            landing: WorldPoint::from(landing),
            launched_at_tick: request.tick,
            lands_at_tick: request.tick.saturating_add(flight_ticks),
            visual_arc_height,
        },
        ComposedProjectileRuntime {
            owner_entity: request.owner_entity,
            source_entity: request.owner_entity,
            source: request.source,
            delivery_index: 0,
            velocity: Vec2::ZERO,
            travelled: 0.0,
            expires_at_tick: request.tick.saturating_add(flight_ticks),
            maximum_range: distance,
            landing: Some(landing),
            recipe: request.recipe.clone(),
        },
        Position::from_xy(launch.x, launch.y),
        Rotation::radians(request.facing),
        Replicate::to_clients(NetworkTarget::All),
        InterpolationTarget::to_clients(NetworkTarget::All),
    ));
    if let Some(match_member) = request.match_member {
        projectile.insert(match_member);
    }
    flight_ticks
}

fn emit_melee(
    request: &DeliveryEmissionRequest<'_>,
    melee: &mut MessageWriter<MeleeAttack>,
) -> u64 {
    melee.write(MeleeAttack {
        source: request.source,
        origin: request.origin,
        facing: request.facing,
        tick: request.tick,
        recipe: request.recipe.clone(),
    });
    1
}

fn emit_cone_spray(commands: &mut Commands, request: &DeliveryEmissionRequest<'_>) -> u64 {
    let DeliveryMethod::ConeSpray {
        propagation_speed,
        reach,
        angle_degrees,
        linger_ticks,
        pulse_interval_ticks,
        map_occlusion,
        max_targets,
    } = request.recipe.delivery
    else {
        unreachable!("cone-spray helper requires a cone-spray delivery");
    };
    let (full_at_tick, expires_at_tick, pulse_count) = cone_spray_timing(
        request.tick,
        propagation_speed,
        reach,
        linger_ticks,
        pulse_interval_ticks,
    );
    let state = ConeSprayState {
        origin: WorldPoint::from(request.origin),
        facing: request.facing,
        propagation_speed,
        maximum_reach: reach,
        angle_degrees,
        emitted_at_tick: request.tick,
        full_at_tick,
        expires_at_tick,
        pulse_interval_ticks,
        map_occlusion,
        max_targets,
    };
    let mut spray = commands.spawn((
        ConeSpray,
        state,
        ReplicatedAttackSource {
            attack: request.source,
        },
        AttackDelivery {
            attack_id: request.source.attack_id,
            delivery_index: 0,
        },
        ConeSprayRuntime {
            owner_entity: request.owner_entity,
            source: request.source,
            recipe: request.recipe.clone(),
            next_pulse_tick: request.tick.saturating_add(pulse_interval_ticks),
            next_delivery_index: 0,
            match_id: request.match_member.map(|member| member.0),
        },
        Replicate::to_clients(NetworkTarget::All),
    ));
    if let Some(match_member) = request.match_member {
        spray.insert(match_member);
    }
    u64::from(pulse_count)
}

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
                        None,
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
