//! Server-authoritative targeted Big Blob lob and deterministic six-way split.

use bevy::prelude::*;

#[derive(Component, Clone, Debug)]
pub(crate) struct BigBlobParentRuntime {
    owner_entity: Entity,
    source: crate::combat::AttackSource,
    parameters: crate::builds::UltimateParameters,
    match_member: crate::matchplay::MatchMember,
}

#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "the fixed six-element heading index is exactly representable as f32"
)]
pub(crate) fn big_blob_headings() -> [f32; 6] {
    core::array::from_fn(|index| index as f32 * core::f32::consts::TAU / 6.0)
}

#[allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::needless_pass_by_value,
    reason = "the targeted activation coordinator owns the complete authoritative input and spawn view"
)]
pub(crate) fn activate_big_blob(
    mut commands: Commands,
    tick: Res<crate::timing::SimulationTick>,
    bounds: Res<crate::map::PlayableBounds>,
    input_tuning: Res<crate::movement::InputTuning>,
    spatial_query: avian2d::prelude::SpatialQuery,
    mut ids: ResMut<crate::combat::NextCombatIds>,
    mut telemetry: ResMut<crate::abilities::AbilityTelemetry>,
    parents: Query<(), With<BigBlobParentRuntime>>,
    blobs: Query<&crate::combat::StickyBlobRuntime>,
    mut casters: Query<
        (
            Entity,
            &avian2d::prelude::Position,
            &avian2d::prelude::Rotation,
            &crate::builds::ResolvedMatchLoadout,
            &crate::protocol::PlayerId,
            &crate::protocol::NetworkEntityId,
            &crate::combat::TeamId,
            &crate::matchplay::MatchParticipant,
            &crate::movement::InputFreshness,
            &mut crate::builds::AbilityState,
            Option<&lightyear::prelude::input::native::ActionState<crate::protocol::FighterInput>>,
            Option<&mut crate::abilities::UltimateInputLatch>,
            Has<crate::combat::Defeated>,
            Has<crate::matchplay::ActiveCombatant>,
        ),
        With<crate::protocol::Fighter>,
    >,
) {
    use lightyear::prelude::{InterpolationTarget, NetworkTarget, Replicate};

    for (
        entity,
        position,
        rotation,
        loadout,
        player_id,
        network_id,
        team,
        participant,
        freshness,
        mut ability,
        action,
        latch,
        defeated,
        active,
    ) in &mut casters
    {
        if loadout.ultimate.kind != crate::builds::UltimateKind::BigBlob {
            continue;
        }
        let requested = action.is_some_and(|action| {
            action.0.is_valid()
                && action.0.gameplay_buttons & crate::protocol::FighterInput::ULTIMATE != 0
        });
        let was_held = latch.as_deref().is_some_and(|latch| latch.0);
        if let Some(mut latch) = latch {
            latch.0 = requested;
        } else {
            commands
                .entity(entity)
                .insert(crate::abilities::UltimateInputLatch(requested));
        }
        if !requested || was_held {
            continue;
        }
        telemetry.record(crate::abilities::AbilityTelemetryRecord {
            tick: tick.0,
            owner_network_id: *network_id,
            kind: crate::abilities::AbilityTelemetryKind::ActivationAttempt,
        });
        let crate::builds::UltimateParameters::BigBlob {
            maximum_range_milliunits,
            flight_ticks,
            visual_arc_height_milliunits,
            landing_clearance_milliunits,
            max_active_per_owner,
            ..
        } = loadout.ultimate.parameters
        else {
            continue;
        };
        let owner_blob_count = blobs
            .iter()
            .filter(|runtime| runtime.source.owner_network_entity_id == *network_id)
            .count();
        let rejection =
            if crate::movement::input_should_neutralize(tick.0, freshness.last_fresh_tick, 12) {
                Some(crate::abilities::AbilityRejectionReason::StaleInput)
            } else if defeated {
                Some(crate::abilities::AbilityRejectionReason::Defeated)
            } else if !active {
                Some(crate::abilities::AbilityRejectionReason::Inactive)
            } else if parents.iter().count() >= 16
                || owner_blob_count.saturating_add(6) > usize::from(max_active_per_owner)
            {
                Some(crate::abilities::AbilityRejectionReason::ActiveFieldCeiling)
            } else if ability.charge != crate::abilities::ULTIMATE_CHARGE_MAX
                || !matches!(ability.phase, crate::builds::AbilityPhase::Ready)
            {
                Some(crate::abilities::AbilityRejectionReason::NotCharged)
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
        let input = action.map(|action| action.0);
        let aim = input
            .and_then(|input| input.aim_update)
            .and_then(|axis| crate::movement::committed_aim(axis.to_vec2(), *input_tuning));
        let distance = input
            .and_then(|input| input.aim_distance)
            .map(crate::protocol::QuantizedAimDistance::to_world_units);
        let Some(maximum_range) =
            crate::builds::world_units_from_milliunits(maximum_range_milliunits)
        else {
            continue;
        };
        let Some(requested_landing) = super::targeted_ultimate_center(
            position.0,
            Vec2::from_angle(rotation.as_radians()),
            aim,
            distance,
            maximum_range,
            bounds.0,
        ) else {
            continue;
        };
        let Some(landing_clearance) =
            crate::builds::world_units_from_milliunits(landing_clearance_milliunits)
        else {
            continue;
        };
        let bounded_landing = requested_landing.clamp(
            bounds.0.min + Vec2::splat(landing_clearance),
            bounds.0.max - Vec2::splat(landing_clearance),
        );
        let landing_filter = avian2d::prelude::SpatialQueryFilter::from_mask(
            crate::movement::STATIC_MAP_LAYER | crate::movement::DESTRUCTIBLE_MAP_LAYER,
        );
        let Some(landing) = crate::combat::delivery::repaired_landing_point(
            position.0,
            bounded_landing,
            landing_clearance,
            |candidate| {
                spatial_query
                    .shape_intersections(
                        &avian2d::prelude::Collider::circle(landing_clearance),
                        candidate,
                        0.0,
                        &landing_filter,
                    )
                    .is_empty()
            },
        ) else {
            telemetry.record(crate::abilities::AbilityTelemetryRecord {
                tick: tick.0,
                owner_network_id: *network_id,
                kind: crate::abilities::AbilityTelemetryKind::ActivationRejected(
                    crate::abilities::AbilityRejectionReason::PlacementBlocked,
                ),
            });
            continue;
        };
        let Some(attack_id) = ids.allocate_attack() else {
            continue;
        };
        let Some(visual_arc_height) =
            crate::builds::world_units_from_milliunits(visual_arc_height_milliunits)
        else {
            continue;
        };
        let source = crate::combat::AttackSource {
            kind: crate::combat::CombatSourceKind::Ultimate {
                ultimate_id: loadout.ultimate.id,
            },
            attack_id,
            player_id: *player_id,
            owner_network_entity_id: *network_id,
            team_id: *team,
            recipe_fingerprint: crate::combat::WeaponRecipeFingerprint(0),
            presentation_profile_id: crate::combat::WeaponPresentationProfileId(5),
            legacy_compatibility: false,
            source_preset_id: None,
            origin: position.0.into(),
            facing: rotation.as_radians(),
        };
        let lands_at_tick = tick.0.saturating_add(flight_ticks);
        commands.spawn((
            crate::combat::Projectile,
            crate::combat::ProjectileSource {
                shot_id: crate::combat::ShotId(attack_id.0),
                player_id: *player_id,
                owner_network_entity_id: *network_id,
                team_id: *team,
                weapon_definition_id: crate::combat::WeaponDefinitionId(0),
            },
            crate::combat::ReplicatedAttackSource { attack: source },
            crate::combat::AttackDelivery {
                attack_id,
                delivery_index: 0,
            },
            crate::combat::ProjectileDeadline {
                expires_at_tick: lands_at_tick,
            },
            crate::combat::LobbedFlight {
                launch: position.0.into(),
                landing: landing.into(),
                launched_at_tick: tick.0,
                lands_at_tick,
                visual_arc_height,
            },
            BigBlobParentRuntime {
                owner_entity: entity,
                source,
                parameters: loadout.ultimate.parameters,
                match_member: crate::matchplay::MatchMember(participant.match_id),
            },
            avian2d::prelude::Position(position.0),
            avian2d::prelude::Rotation::radians(rotation.as_radians()),
            crate::matchplay::MatchMember(participant.match_id),
            Replicate::to_clients(NetworkTarget::All),
            InterpolationTarget::to_clients(NetworkTarget::All),
        ));
        *ability = crate::builds::AbilityState {
            charge: 0,
            phase: crate::builds::AbilityPhase::Charging,
        };
        commands
            .entity(entity)
            .remove::<crate::matchplay::SpawnProtection>();
    }
}

#[allow(
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    clippy::needless_pass_by_value,
    reason = "the split materializes one bounded authored child recipe and six deterministic deliveries"
)]
pub(crate) fn advance_big_blob_parents(
    mut commands: Commands,
    tick: Res<crate::timing::SimulationTick>,
    mut parents: Query<(
        Entity,
        &mut avian2d::prelude::Position,
        &crate::combat::LobbedFlight,
        &BigBlobParentRuntime,
        &crate::combat::ProjectileSource,
    )>,
) {
    use avian2d::prelude::{CollisionLayers, Position, Rotation};
    use lightyear::prelude::{InterpolationTarget, NetworkTarget, Replicate};

    for (entity, mut position, flight, runtime, projectile_source) in &mut parents {
        if tick.0 < flight.lands_at_tick {
            let progress = tick.0.saturating_sub(flight.launched_at_tick) as f32
                / flight
                    .lands_at_tick
                    .saturating_sub(flight.launched_at_tick)
                    .max(1) as f32;
            position.0 = flight
                .launch
                .as_vec2()
                .lerp(flight.landing.as_vec2(), progress.clamp(0.0, 1.0));
            continue;
        }
        let crate::builds::UltimateParameters::BigBlob {
            child_speed_milliunits,
            child_radius_milliunits,
            child_range_milliunits,
            child_lifetime_ticks,
            child_fuse_ticks,
            child_explosion_radius_milliunits,
            child_damage,
            max_active_per_owner,
            ..
        } = runtime.parameters
        else {
            commands.entity(entity).try_despawn();
            continue;
        };
        let Some(speed) = crate::builds::world_units_from_milliunits(child_speed_milliunits) else {
            commands.entity(entity).try_despawn();
            continue;
        };
        let Some(radius) = crate::builds::world_units_from_milliunits(child_radius_milliunits)
        else {
            commands.entity(entity).try_despawn();
            continue;
        };
        let Some(range) = crate::builds::world_units_from_milliunits(child_range_milliunits) else {
            commands.entity(entity).try_despawn();
            continue;
        };
        let Some(explosion_radius) =
            crate::builds::world_units_from_milliunits(child_explosion_radius_milliunits)
        else {
            commands.entity(entity).try_despawn();
            continue;
        };
        let landing = flight.landing.as_vec2();
        let recipe = crate::combat::WeaponRecipe {
            economy: crate::combat::WeaponEconomy::Charges {
                capacity: 1,
                recharge_ticks: 1,
            },
            fire_cooldown_ticks: 1,
            firing: crate::combat::FiringPattern::Single,
            delivery: crate::combat::DeliveryMethod::StickyStraight {
                speed,
                radius,
                range,
                lifetime_ticks: child_lifetime_ticks,
                muzzle_offset: 1.0,
                fuse_ticks: child_fuse_ticks,
                max_active_per_owner,
            },
            payload_bundles: vec![crate::combat::PayloadBundleDefinition {
                target: crate::combat::TargetSelection::Area {
                    radius: explosion_radius,
                    map_occlusion: false,
                    max_targets: 16,
                },
                effects: vec![crate::combat::PayloadEffectDefinition::Damage {
                    amount: child_damage,
                    falloff: crate::combat::DamageFalloff::None,
                    recipients: crate::combat::RecipientPolicy::Hostiles,
                }],
            }],
            world_effects: Vec::new(),
        };
        let body = crate::combat::ProjectileBody::circle(radius);
        for (delivery_index, heading) in big_blob_headings().into_iter().enumerate() {
            commands.spawn((
                crate::combat::Projectile,
                *projectile_source,
                crate::combat::ReplicatedAttackSource {
                    attack: runtime.source,
                },
                crate::combat::AttackDelivery {
                    attack_id: runtime.source.attack_id,
                    delivery_index: u8::try_from(delivery_index).unwrap_or(u8::MAX),
                },
                crate::combat::ProjectileDeadline {
                    expires_at_tick: tick.0.saturating_add(child_lifetime_ticks),
                },
                crate::combat::StraightFlight {
                    origin: landing.into(),
                    facing: heading,
                    speed,
                    maximum_range: range,
                    launched_at_tick: tick.0,
                },
                body,
                crate::combat::ComposedProjectileRuntime {
                    owner_entity: runtime.owner_entity,
                    source_entity: runtime.owner_entity,
                    source: runtime.source,
                    delivery_index: u8::try_from(delivery_index).unwrap_or(u8::MAX),
                    velocity: Vec2::from_angle(heading) * speed,
                    travelled: 0.0,
                    expires_at_tick: tick.0.saturating_add(child_lifetime_ticks),
                    maximum_range: range,
                    landing: None,
                    recipe: recipe.clone(),
                },
                Position(landing),
                Rotation::radians(heading),
                body.collider(),
                CollisionLayers::new(
                    crate::movement::PROJECTILE_LAYER,
                    crate::movement::FIGHTER_LAYER
                        | crate::movement::STATIC_MAP_LAYER
                        | crate::movement::DESTRUCTIBLE_MAP_LAYER,
                ),
                runtime.match_member,
                Replicate::to_clients(NetworkTarget::All),
                InterpolationTarget::to_clients(NetworkTarget::All),
            ));
        }
        commands.entity(entity).try_despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_form_fixed_sixty_degree_hexagon() {
        let headings = big_blob_headings();
        for pair in headings.windows(2) {
            assert!((pair[1] - pair[0] - core::f32::consts::TAU / 6.0).abs() < 0.000_01);
        }
    }
}
