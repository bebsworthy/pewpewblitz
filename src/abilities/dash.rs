use bevy::prelude::Vec2;

#[must_use]
pub fn bounded_dash_endpoint(
    origin: Vec2,
    direction: Vec2,
    clear_distance: f32,
    maximum_distance: f32,
) -> Option<Vec2> {
    if !origin.is_finite() || !clear_distance.is_finite() || !maximum_distance.is_finite() {
        return None;
    }
    let direction = direction.try_normalize()?;
    let distance = clear_distance.clamp(0.0, maximum_distance);
    (distance > 0.5).then_some(origin + direction * distance)
}

#[must_use]
pub fn stable_dash_contacts(
    segment_start: Vec2,
    segment_end: Vec2,
    contact_radius: f32,
    already_hit: &[crate::protocol::NetworkEntityId],
    candidates: impl IntoIterator<Item = (crate::protocol::NetworkEntityId, Vec2, bool)>,
    maximum_targets: u8,
) -> Vec<crate::protocol::NetworkEntityId> {
    let remaining = usize::from(maximum_targets).saturating_sub(already_hit.len());
    let mut candidates: Vec<_> = candidates
        .into_iter()
        .filter(|(id, position, eligible)| {
            *eligible
                && !already_hit.contains(id)
                && distance_to_segment(*position, segment_start, segment_end)
                    <= contact_radius * 2.0
        })
        .map(|(id, _, _)| id)
        .collect();
    candidates.sort_by_key(|id| id.0);
    candidates.dedup();
    candidates.truncate(remaining);
    candidates
}

#[cfg(feature = "server")]
fn stable_dash_contacts_with_radii(
    segment_start: Vec2,
    segment_end: Vec2,
    attacker_radius: f32,
    already_hit: &[crate::protocol::NetworkEntityId],
    maximum_targets: u8,
    candidates: impl IntoIterator<Item = (crate::protocol::NetworkEntityId, Vec2, f32, bool)>,
) -> Vec<crate::protocol::NetworkEntityId> {
    let remaining = usize::from(maximum_targets).saturating_sub(already_hit.len());
    let mut candidates: Vec<_> = candidates
        .into_iter()
        .filter(|(id, position, target_radius, eligible)| {
            *eligible
                && !already_hit.contains(id)
                && distance_to_segment(*position, segment_start, segment_end)
                    <= attacker_radius + *target_radius
        })
        .map(|(id, ..)| id)
        .collect();
    candidates.sort_by_key(|id| id.0);
    candidates.dedup();
    candidates.truncate(remaining);
    candidates
}

#[must_use]
pub fn dash_position(
    origin: Vec2,
    endpoint: Vec2,
    elapsed_ticks: u64,
    duration_ticks: u64,
) -> Vec2 {
    let elapsed =
        u16::try_from(elapsed_ticks.min(duration_ticks)).expect("validated dash duration fits u16");
    let duration = u16::try_from(duration_ticks).expect("validated dash duration fits u16");
    let fraction = f32::from(elapsed) / f32::from(duration);
    origin.lerp(endpoint, fraction)
}

#[cfg(feature = "server")]
#[derive(bevy::prelude::Component, Clone, Debug, PartialEq)]
pub(crate) struct DashRuntime {
    origin: Vec2,
    endpoint: Vec2,
    started_at_tick: u64,
    source: crate::combat::AttackSource,
    hit_targets: Vec<crate::protocol::NetworkEntityId>,
    tuning: ResolvedDashTuning,
}

#[cfg(feature = "server")]
impl DashRuntime {
    pub(crate) const fn charge_maximum(&self) -> u16 {
        self.tuning.charge_maximum
    }
}

#[cfg(feature = "server")]
#[derive(Clone, Copy, Debug, PartialEq)]
struct ResolvedDashTuning {
    charge_maximum: u16,
    maximum_distance: f32,
    duration_ticks: u64,
    damage: u16,
    knockback_speed: f32,
    knockback_duration_ticks: u64,
    maximum_targets: u8,
}

#[cfg(feature = "server")]
fn resolve_dash_tuning(
    parameters: crate::builds::UltimateParameters,
    charge_maximum: u16,
) -> Option<ResolvedDashTuning> {
    let crate::builds::UltimateParameters::Dash {
        maximum_distance_milliunits,
        duration_ticks,
        damage,
        knockback_speed_milliunits,
        knockback_duration_ticks,
        maximum_targets,
    } = parameters
    else {
        return None;
    };
    Some(ResolvedDashTuning {
        charge_maximum,
        maximum_distance: crate::builds::world_units_from_milliunits(maximum_distance_milliunits)?,
        duration_ticks,
        damage,
        knockback_speed: crate::builds::world_units_from_milliunits(knockback_speed_milliunits)?,
        knockback_duration_ticks,
        maximum_targets,
    })
}

#[cfg(feature = "server")]
#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
    clippy::type_complexity
)]
#[allow(
    clippy::too_many_arguments,
    reason = "the authoritative Bevy activation system coordinates input, collision, fighter state, telemetry, and one world-target request"
)]
pub(crate) fn activate_dash(
    mut commands: bevy::prelude::Commands,
    tick: bevy::prelude::Res<crate::timing::SimulationTick>,
    spatial_query: avian2d::prelude::SpatialQuery,
    mut ids: bevy::prelude::ResMut<crate::combat::NextCombatIds>,
    mut telemetry: bevy::prelude::ResMut<crate::abilities::AbilityTelemetry>,
    mut world_pending: bevy::prelude::ResMut<crate::map::PendingWorldTargetDamages>,
    mut objective_pending: bevy::prelude::ResMut<crate::matchplay::PendingModeObjectiveDamages>,
    objects: bevy::prelude::Query<
        (
            &crate::map::DamageableTargetIdentity,
            &crate::combat::CurrentHealth,
            &crate::map::DamageableLifeState,
        ),
        bevy::prelude::Or<(
            bevy::prelude::With<crate::map::DamageableWorldObject>,
            bevy::prelude::With<crate::matchplay::HeistSafe>,
        )>,
    >,
    mut fighters: bevy::prelude::Query<
        (
            bevy::prelude::Entity,
            &avian2d::prelude::Position,
            &avian2d::prelude::Rotation,
            &crate::builds::ResolvedUltimate,
            &crate::combat::ResolvedWeapon,
            &crate::builds::FighterBody,
            &crate::protocol::PlayerId,
            &crate::protocol::NetworkEntityId,
            &crate::combat::TeamId,
            &crate::movement::InputFreshness,
            (
                &mut crate::builds::AbilityState,
                &mut crate::combat::HealthRecoveryState,
            ),
            Option<&lightyear::prelude::input::native::ActionState<crate::protocol::FighterInput>>,
            Option<&crate::combat::Defeated>,
            Option<&crate::matchplay::ActiveCombatant>,
            Option<&mut crate::abilities::UltimateInputLatch>,
        ),
        bevy::prelude::With<crate::protocol::Fighter>,
    >,
) {
    use avian2d::prelude::{Collider, ShapeCastConfig, SpatialQueryFilter};
    use bevy::math::Dir2;
    for (
        entity,
        position,
        rotation,
        ultimate,
        weapon,
        body,
        player,
        network_id,
        team,
        freshness,
        (mut ability, mut health_recovery),
        action,
        defeated,
        active,
        latch,
    ) in &mut fighters
    {
        if ultimate.kind != crate::builds::UltimateKind::Dash {
            continue;
        }
        let was_held = latch.as_deref().is_some_and(|latch| latch.0);
        let request = super::activation::ultimate_request(action.map(|action| action.0), was_held);
        let requested = request.requested;
        let held = requested
            && !crate::movement::input_should_neutralize(
                tick.0,
                freshness.last_fresh_tick,
                crate::movement::AUTHORITATIVE_INPUT_STALE_TICKS,
            );
        if let Some(mut latch) = latch {
            latch.0 = requested;
        } else {
            commands
                .entity(entity)
                .insert(crate::abilities::UltimateInputLatch(requested));
        }
        if requested && !was_held {
            telemetry.record(crate::abilities::AbilityTelemetryRecord {
                tick: tick.0,
                owner_network_id: *network_id,
                kind: crate::abilities::AbilityTelemetryKind::ActivationAttempt,
            });
        }
        if !request.rising_edge {
            continue;
        }
        let before_readiness = matches!(
            ability.phase,
            crate::builds::AbilityPhase::Dashing { .. }
                | crate::builds::AbilityPhase::Deployed { .. }
        )
        .then_some(crate::abilities::AbilityRejectionReason::AlreadyExecuting);
        if let Err(reason) = super::activation::evaluate_activation_gate(
            super::activation::ActivationGateContext {
                input_fresh: held,
                defeated: defeated.is_some(),
                active: active.is_some(),
                state: *ability,
                maximum_charge: ultimate.charge_policy.maximum,
            },
            super::activation::ActivationRestrictions {
                before_readiness,
                after_readiness: None,
            },
        ) {
            telemetry.record(crate::abilities::AbilityTelemetryRecord {
                tick: tick.0,
                owner_network_id: *network_id,
                kind: crate::abilities::AbilityTelemetryKind::ActivationRejected(reason),
            });
            continue;
        }
        let direction = Vec2::from_angle(rotation.as_radians());
        let Some(dash_tuning) =
            resolve_dash_tuning(ultimate.parameters, ultimate.charge_policy.maximum)
        else {
            continue;
        };
        let Ok(direction_dir) = Dir2::new(direction) else {
            continue;
        };
        let filter = SpatialQueryFilter::from_mask(
            crate::movement::STATIC_MAP_LAYER
                | crate::movement::DESTRUCTIBLE_MAP_LAYER
                | crate::movement::PLAYER_ONLY_MAP_LAYER,
        )
        .with_excluded_entities([entity]);
        let collision = spatial_query.cast_shape(
            &Collider::circle(body.radius),
            position.0,
            rotation.as_radians(),
            direction_dir,
            &ShapeCastConfig::from_max_distance(dash_tuning.maximum_distance),
            &filter,
        );
        let distance = collision
            .as_ref()
            .map_or(dash_tuning.maximum_distance, |hit| hit.distance.max(0.0));
        let Some(endpoint) = bounded_dash_endpoint(
            position.0,
            direction,
            distance,
            dash_tuning.maximum_distance,
        ) else {
            telemetry.record(crate::abilities::AbilityTelemetryRecord {
                tick: tick.0,
                owner_network_id: *network_id,
                kind: crate::abilities::AbilityTelemetryKind::ActivationRejected(
                    crate::abilities::AbilityRejectionReason::ZeroLengthDash,
                ),
            });
            continue;
        };
        let Some(attack_id) = ids.allocate_attack() else {
            telemetry.record(crate::abilities::AbilityTelemetryRecord {
                tick: tick.0,
                owner_network_id: *network_id,
                kind: crate::abilities::AbilityTelemetryKind::ActivationRejected(
                    crate::abilities::AbilityRejectionReason::IdentifierExhausted,
                ),
            });
            continue;
        };
        let ends_at_tick = tick.0.saturating_add(dash_tuning.duration_ticks);
        *ability = crate::builds::AbilityState {
            charge: 0,
            phase: crate::builds::AbilityPhase::Dashing { ends_at_tick },
        };
        health_recovery.last_accepted_attack_tick = tick.0;
        health_recovery.recovery_remainder = 0;
        let source = crate::combat::AttackSource {
            kind: crate::combat::CombatSourceKind::Ultimate {
                ultimate_id: ultimate.id,
            },
            attack_id,
            player_id: *player,
            owner_network_entity_id: *network_id,
            team_id: *team,
            recipe_fingerprint: weapon.recipe_fingerprint,
            legacy_compatibility: false,
            source_preset_id: None,
            origin: crate::combat::WorldPoint::from(position.0),
            facing: rotation.as_radians(),
        };
        if let Some(hit) = collision
            && let Ok((identity, health, life)) = objects.get(hit.entity)
            && crate::map::object_is_live(*health, *life)
        {
            match *identity {
                crate::map::DamageableTargetIdentity::MapObject { .. } => {
                    world_pending.0.push(crate::map::PendingWorldTargetDamage {
                        target: *identity,
                        source,
                        attack_id,
                        requested_damage: dash_tuning.damage,
                        delivery_index: 0,
                        bundle_index: 0,
                        effect_index: 0,
                    });
                }
                crate::map::DamageableTargetIdentity::HeistSafe { .. } => {
                    objective_pending
                        .0
                        .push(crate::matchplay::PendingModeObjectiveDamage {
                            target: *identity,
                            source,
                            requested_damage: dash_tuning.damage,
                            delivery_index: 0,
                            bundle_index: 0,
                            effect_index: 0,
                        });
                }
            }
        }
        commands
            .entity(entity)
            .insert(DashRuntime {
                origin: position.0,
                endpoint,
                started_at_tick: tick.0,
                source,
                hit_targets: Vec::with_capacity(usize::from(dash_tuning.maximum_targets)),
                tuning: dash_tuning,
            })
            .remove::<crate::matchplay::SpawnProtection>();
        telemetry.record(crate::abilities::AbilityTelemetryRecord {
            tick: tick.0,
            owner_network_id: *network_id,
            kind: crate::abilities::AbilityTelemetryKind::DashAccepted,
        });
        telemetry.record(crate::abilities::AbilityTelemetryRecord {
            tick: tick.0,
            owner_network_id: *network_id,
            kind: crate::abilities::AbilityTelemetryKind::DashTravel {
                requested_distance_milli: distance_milli(dash_tuning.maximum_distance),
                actual_distance_milli: distance_milli(position.0.distance(endpoint)),
                map_collision_truncated: distance + 0.01 < dash_tuning.maximum_distance,
            },
        });
    }
}

#[cfg(feature = "server")]
#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
    clippy::type_complexity
)]
pub(crate) fn advance_dash(
    mut commands: bevy::prelude::Commands,
    tick: bevy::prelude::Res<crate::timing::SimulationTick>,
    bounds: bevy::prelude::Res<crate::map::PlayableBounds>,
    builds: bevy::prelude::Res<crate::builds::BuildCatalogResource>,
    mut telemetry: bevy::prelude::ResMut<crate::abilities::AbilityTelemetry>,
    mut payloads: bevy::prelude::MessageWriter<crate::combat::PendingPayload>,
    mut queries: bevy::prelude::ParamSet<(
        bevy::prelude::Query<(
            bevy::prelude::Entity,
            &mut avian2d::prelude::Position,
            &mut avian2d::prelude::LinearVelocity,
            &mut crate::builds::AbilityState,
            &mut DashRuntime,
            Option<&crate::combat::Defeated>,
            Option<&crate::matchplay::ActiveCombatant>,
        )>,
        bevy::prelude::Query<
            (
                bevy::prelude::Entity,
                &avian2d::prelude::Position,
                &crate::protocol::NetworkEntityId,
                &crate::combat::TeamId,
                Option<&crate::combat::Defeated>,
                Option<&crate::matchplay::ActiveCombatant>,
                Option<&crate::abilities::Sentry>,
                Option<&super::sentry::ResolvedSentryTuning>,
            ),
            bevy::prelude::Or<(
                bevy::prelude::With<crate::protocol::Fighter>,
                bevy::prelude::With<crate::abilities::Sentry>,
            )>,
        >,
    )>,
) {
    let mut targets: Vec<_> = queries
        .p1()
        .iter()
        .map(
            |(entity, position, network_id, team, defeated, active, sentry, sentry_tuning)| {
                (
                    entity,
                    position.0,
                    *network_id,
                    *team,
                    sentry_tuning.map_or(builds.0.fighter_body.radius, |value| value.body_radius),
                    defeated.is_some(),
                    active.is_some() || sentry.is_some(),
                )
            },
        )
        .collect();
    targets.sort_by_key(|target| target.2.0);
    for (entity, mut position, mut velocity, mut ability, mut dash, defeated, active) in
        &mut queries.p0()
    {
        if defeated.is_some() || active.is_none() {
            telemetry.record(crate::abilities::AbilityTelemetryRecord {
                tick: tick.0,
                owner_network_id: dash.source.owner_network_entity_id,
                kind: crate::abilities::AbilityTelemetryKind::DashInterrupted(
                    if defeated.is_some() {
                        crate::abilities::DashInterruptionReason::Defeated
                    } else {
                        crate::abilities::DashInterruptionReason::MatchInactive
                    },
                ),
            });
            ability.phase =
                crate::abilities::settled_ability_phase(ability.charge, dash.tuning.charge_maximum);
            velocity.0 = Vec2::ZERO;
            commands.entity(entity).remove::<DashRuntime>();
            continue;
        }
        let crate::builds::AbilityPhase::Dashing { ends_at_tick } = ability.phase else {
            velocity.0 = Vec2::ZERO;
            commands.entity(entity).remove::<DashRuntime>();
            continue;
        };
        let previous = position.0;
        let elapsed = tick
            .0
            .saturating_sub(dash.started_at_tick)
            .saturating_add(1);
        let next_position = dash_position(
            dash.origin,
            dash.endpoint,
            elapsed,
            dash.tuning.duration_ticks,
        );
        if !bounds
            .0
            .contains_with_inset(next_position, builds.0.fighter_body.radius)
        {
            telemetry.record(crate::abilities::AbilityTelemetryRecord {
                tick: tick.0,
                owner_network_id: dash.source.owner_network_entity_id,
                kind: crate::abilities::AbilityTelemetryKind::DashInterrupted(
                    crate::abilities::DashInterruptionReason::OutOfBounds,
                ),
            });
            ability.phase =
                crate::abilities::settled_ability_phase(ability.charge, dash.tuning.charge_maximum);
            velocity.0 = Vec2::ZERO;
            commands.entity(entity).remove::<DashRuntime>();
            continue;
        }
        position.0 = next_position;
        velocity.0 = Vec2::ZERO;
        let contacts = stable_dash_contacts_with_radii(
            previous,
            position.0,
            builds.0.fighter_body.radius,
            &dash.hit_targets,
            dash.tuning.maximum_targets,
            targets.iter().map(
                |(
                    target_entity,
                    target_position,
                    target_id,
                    target_team,
                    target_radius,
                    defeated,
                    active,
                )| {
                    (
                        *target_id,
                        *target_position,
                        *target_radius,
                        *target_entity != entity
                            && !*defeated
                            && *active
                            && *target_team != dash.source.team_id,
                    )
                },
            ),
        );
        for target_id in contacts {
            let Some((target_entity, target_position, _, _, _, _, _)) =
                targets.iter().find(|target| target.2 == target_id)
            else {
                continue;
            };
            dash.hit_targets.push(target_id);
            telemetry.record(crate::abilities::AbilityTelemetryRecord {
                tick: tick.0,
                owner_network_id: dash.source.owner_network_entity_id,
                kind: crate::abilities::AbilityTelemetryKind::DashContact,
            });
            payloads.write(crate::combat::PendingPayload {
                source: dash.source,
                delivery_index: 0,
                bundle_index: 0,
                target: *target_entity,
                target_network_id: target_id,
                position: *target_position,
                engagement_distance: target_position.distance(dash.origin),
                delivery_travel: position.0.distance(dash.origin),
                contact_fraction: 0.0,
                bundle: crate::combat::PayloadBundleDefinition {
                    target: crate::combat::TargetSelection::Direct,
                    effects: vec![
                        crate::combat::PayloadEffectDefinition::Damage {
                            amount: dash.tuning.damage,
                            falloff: crate::combat::DamageFalloff::None,
                            recipients: crate::combat::RecipientPolicy::Hostiles,
                        },
                        crate::combat::PayloadEffectDefinition::Knockback {
                            speed: dash.tuning.knockback_speed,
                            duration_ticks: dash.tuning.knockback_duration_ticks,
                            recipients: crate::combat::RecipientPolicy::Hostiles,
                        },
                    ],
                },
            });
        }
        if elapsed >= dash.tuning.duration_ticks || tick.0.saturating_add(1) >= ends_at_tick {
            ability.phase =
                crate::abilities::settled_ability_phase(ability.charge, dash.tuning.charge_maximum);
            commands.entity(entity).remove::<DashRuntime>();
        }
    }
}

#[cfg(feature = "server")]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn distance_milli(distance: f32) -> u32 {
    (distance.max(0.0) * 1_000.0).min(u32::MAX as f32).round() as u32
}

fn distance_to_segment(point: Vec2, start: Vec2, end: Vec2) -> f32 {
    let segment = end - start;
    if segment.length_squared() <= f32::EPSILON {
        return point.distance(start);
    }
    let t = ((point - start).dot(segment) / segment.length_squared()).clamp(0.0, 1.0);
    point.distance(start + segment * t)
}
