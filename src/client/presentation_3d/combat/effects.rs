use super::super::*;
use super::common::GROUND_EFFECT_HEIGHT;
use crate::combat::client::DeduplicatedCombatCue;
use std::collections::{HashMap, VecDeque};

const MAX_EFFECTS: usize = 96;

#[derive(Component)]
pub(in super::super) struct CombatEffect3d {
    timer: Timer,
    expires_at_tick: Option<u64>,
    order: u64,
}

#[derive(Default)]
pub(in super::super) struct CombatEffectSequence(u64);

#[derive(Message)]
pub(in super::super) struct PendingCombatEffect {
    lifetime: Duration,
    expires_at_tick: Option<u64>,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    transform: Transform,
    label: &'static str,
}

impl PendingCombatEffect {
    fn spawn(&self, commands: &mut Commands, order: u64) -> Entity {
        commands
            .spawn((
                CombatEffect3d {
                    timer: Timer::new(self.lifetime, TimerMode::Once),
                    expires_at_tick: self.expires_at_tick,
                    order,
                },
                Mesh3d(self.mesh.clone()),
                MeshMaterial3d(self.material.clone()),
                NotShadowCaster,
                NotShadowReceiver,
                self.transform,
                Name::new(self.label),
            ))
            .id()
    }
}
#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "cue consumption resolves actor intents and one bounded effect transaction"
)]
pub(in super::super) fn consume_combat_cues(
    mut cues: MessageReader<DeduplicatedCombatCue>,
    mut pending_effects: MessageWriter<PendingCombatEffect>,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    settings: Option<Res<ClientShellSettings>>,
    owners: Query<(Entity, &NetworkEntityId), With<Fighter>>,
    mut visuals: Query<(Entity, &CombatVisualOwner, &mut V3FighterVisual)>,
    authoritative_ticks: Query<&AuthoritativeTick>,
) {
    let reduced = settings.is_some_and(|value| value.reduced_combat_effects);
    let current_tick = authoritative_ticks.iter().map(|tick| tick.0).max();
    let visuals_by_owner = visuals
        .iter()
        .map(|(visual, owner, _)| (owner.0, visual))
        .collect::<HashMap<_, _>>();
    let visuals_by_network_id = owners
        .iter()
        .filter_map(|(owner, network_id)| {
            visuals_by_owner
                .get(&owner)
                .map(|&visual| (network_id.0, visual))
        })
        .collect::<HashMap<_, _>>();
    for DeduplicatedCombatCue(cue) in cues.read() {
        if let crate::combat::CombatCue::AttackAccepted { source, .. } = cue
            && let Some(&visual_entity) = visuals_by_network_id.get(&source.0)
            && let Ok((_, _, mut visual)) = visuals.get_mut(visual_entity)
        {
            visual.shoot_seconds = 0.18;
        }
        let Some((position, material, scale)) = cue_effect(cue, &materials) else {
            continue;
        };
        let scan_pulse = matches!(cue, crate::combat::CombatCue::RevealScanActivated { .. });
        let (lifetime, expires_at_tick) = combat_effect_lifetime(cue, current_tick, reduced);
        let transform = if scan_pulse {
            Transform {
                translation: ground_position(position) + Vec3::Y * GROUND_EFFECT_HEIGHT,
                rotation: Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2),
                scale: Vec3::splat(scale),
            }
        } else {
            Transform::from_translation(ground_position(position) + Vec3::Y * (scale * 0.45))
                .with_scale(Vec3::splat(scale * if reduced { 0.65 } else { 1.0 }))
        };
        pending_effects.write(PendingCombatEffect {
            lifetime,
            expires_at_tick,
            mesh: if scan_pulse {
                primitives.area_ring.clone()
            } else {
                primitives.effect_sphere.clone()
            },
            material,
            transform,
            label: if scan_pulse {
                "V9 active Reveal Scan area"
            } else {
                "V3 bounded combat cue effect"
            },
        });
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    reason = "the focused Bevy presentation system reads bounded cue, map, palette, settings, and effect state"
)]
pub(in super::super) fn consume_world_object_cues(
    received: Option<ResMut<crate::map::ReceivedWorldObjectCues>>,
    mut pending_effects: MessageWriter<PendingCombatEffect>,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    settings: Option<Res<ClientShellSettings>>,
    map_states: Query<&crate::map::MapDynamicState, With<crate::map::MapRoot>>,
) {
    let Some(mut received) = received else {
        return;
    };
    let Ok(map_state) = map_states.single() else {
        received.0.clear();
        return;
    };
    let reduced = settings.is_some_and(|value| value.reduced_combat_effects);
    for cue in received.0.drain(..) {
        if cue.target().generation() != map_state.generation_id() {
            continue;
        }
        let (position, radius, exploded) = match cue {
            crate::map::WorldObjectCue::Damaged { position, .. } => {
                (position.as_vec2(), 11.0, false)
            }
            crate::map::WorldObjectCue::Exploded {
                position,
                radius_world_units,
                ..
            } => (position.as_vec2(), f32::from(radius_world_units), true),
        };
        pending_effects.write(PendingCombatEffect {
            lifetime: Duration::from_secs_f32(if reduced { 0.12 } else { 0.28 }),
            expires_at_tick: None,
            mesh: if exploded {
                primitives.area_ring.clone()
            } else {
                primitives.effect_sphere.clone()
            },
            material: materials.effect_damage.clone(),
            transform: if exploded {
                Transform {
                    translation: ground_position(position) + Vec3::Y * GROUND_EFFECT_HEIGHT,
                    rotation: Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2),
                    scale: Vec3::splat(radius),
                }
            } else {
                Transform::from_translation(ground_position(position) + Vec3::Y * 8.0)
                    .with_scale(Vec3::splat(if reduced { 7.0 } else { radius }))
            },
            label: if exploded {
                "V10 authoritative oil-barrel blast"
            } else {
                "V10 oil-barrel damage response"
            },
        });
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    reason = "the pickup cue presenter owns bounded green open, heal, and expiry feedback"
)]
pub(in super::super) fn consume_pickup_cues(
    received: Option<ResMut<crate::map::ReceivedPickupCues>>,
    mut pending_effects: MessageWriter<PendingCombatEffect>,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    settings: Option<Res<ClientShellSettings>>,
    map_states: Query<&crate::map::MapDynamicState, With<crate::map::MapRoot>>,
) {
    let Some(mut received) = received else { return };
    let Ok(map_state) = map_states.single() else {
        received.0.clear();
        return;
    };
    let reduced = settings.is_some_and(|value| value.reduced_combat_effects);
    for cue in received.0.drain(..) {
        let (identity, position, radius, ring, label) = match cue {
            crate::map::PickupCue::Spawned {
                identity, position, ..
            } => (
                identity,
                position.as_vec2(),
                22.0,
                true,
                "V10 chest restoration drop",
            ),
            crate::map::PickupCue::Collected {
                identity, position, ..
            } => (
                identity,
                position.as_vec2(),
                18.0,
                false,
                "V10 restoration collected",
            ),
            crate::map::PickupCue::Expired {
                identity, position, ..
            } => (
                identity,
                position.as_vec2(),
                12.0,
                true,
                "V10 restoration expired",
            ),
        };
        if identity.generation != map_state.generation_id() {
            continue;
        }
        pending_effects.write(PendingCombatEffect {
            lifetime: Duration::from_secs_f32(if reduced { 0.12 } else { 0.3 }),
            expires_at_tick: None,
            mesh: if ring {
                primitives.area_ring.clone()
            } else {
                primitives.effect_sphere.clone()
            },
            material: materials.pickup_glow.clone(),
            transform: if ring {
                Transform {
                    translation: ground_position(position) + Vec3::Y * GROUND_EFFECT_HEIGHT,
                    rotation: Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2),
                    scale: Vec3::splat(if reduced { radius * 0.65 } else { radius }),
                }
            } else {
                Transform::from_translation(ground_position(position) + Vec3::Y * 15.0)
                    .with_scale(Vec3::splat(if reduced { radius * 0.6 } else { radius }))
            },
            label,
        });
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    reason = "the focused presentation system validates and materializes bounded objective cues"
)]
pub(in super::super) fn consume_heist_objective_cues(
    mut received: MessageReader<crate::matchplay::ReceivedHeistObjectiveCue>,
    mut pending_effects: MessageWriter<PendingCombatEffect>,
    readiness: Res<hud::ClientHeistReadiness>,
    matches: Query<&MatchState, With<MatchRoot>>,
    safes: Query<&crate::map::DamageableTargetIdentity, With<crate::matchplay::HeistSafe>>,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    settings: Option<Res<ClientShellSettings>>,
) {
    let ready = matches!(*readiness, hud::ClientHeistReadiness::Ready);
    let match_id = matches.single().ok().map(|state| state.match_id);
    let reduced = settings.is_some_and(|value| value.reduced_combat_effects);
    for crate::matchplay::ReceivedHeistObjectiveCue(cue) in received.read() {
        if !ready
            || !matches!(
                cue.target,
                crate::map::DamageableTargetIdentity::HeistSafe {
                    match_id: cue_match,
                    ..
                } if Some(cue_match) == match_id
            )
            || !safes.iter().any(|identity| *identity == cue.target)
        {
            continue;
        }
        let (radius, duration, ring) = match cue.kind {
            crate::matchplay::HeistObjectiveCueKind::Damaged => (12.0, 0.24, false),
            crate::matchplay::HeistObjectiveCueKind::Critical => (30.0, 0.50, true),
            crate::matchplay::HeistObjectiveCueKind::Destroyed => (72.0, 0.85, true),
        };
        pending_effects.write(PendingCombatEffect {
            lifetime: Duration::from_secs_f32(if reduced { duration * 0.5 } else { duration }),
            expires_at_tick: None,
            mesh: if ring {
                primitives.area_ring.clone()
            } else {
                primitives.effect_sphere.clone()
            },
            material: materials.effect_damage.clone(),
            transform: Transform {
                translation: ground_position(cue.position.as_vec2())
                    + Vec3::Y * GROUND_EFFECT_HEIGHT,
                scale: if ring {
                    Vec3::new(radius, 1.0, radius)
                } else {
                    Vec3::splat(radius)
                },
                ..default()
            },
            label: match cue.kind {
                crate::matchplay::HeistObjectiveCueKind::Damaged => "Heist safe hit cue",
                crate::matchplay::HeistObjectiveCueKind::Critical => "Heist safe critical cue",
                crate::matchplay::HeistObjectiveCueKind::Destroyed => "Heist safe destroyed cue",
            },
        });
    }
}

pub(in super::super) fn materialize_combat_effects(
    mut commands: Commands,
    mut pending_effects: MessageReader<PendingCombatEffect>,
    effects: Query<(Entity, &CombatEffect3d)>,
    mut sequence: Local<CombatEffectSequence>,
) {
    let mut active = effects
        .iter()
        .map(|(entity, effect)| (effect.order, entity))
        .collect::<VecDeque<_>>();
    active
        .make_contiguous()
        .sort_unstable_by_key(|(order, entity)| (*order, entity.to_bits()));
    if let Some((maximum_order, _)) = active.back() {
        sequence.0 = sequence.0.max(*maximum_order);
    }
    while active.len() > MAX_EFFECTS {
        if let Some((_, oldest)) = active.pop_front() {
            commands.entity(oldest).try_despawn();
        }
    }

    for descriptor in pending_effects.read() {
        while active.len() >= MAX_EFFECTS {
            if let Some((_, oldest)) = active.pop_front() {
                commands.entity(oldest).try_despawn();
            }
        }
        sequence.0 = sequence.0.saturating_add(1);
        let entity = descriptor.spawn(&mut commands, sequence.0);
        active.push_back((sequence.0, entity));
    }
}

fn reveal_ring_remaining_duration(
    activated_at_tick: u64,
    expires_at_tick: u64,
    observed_tick: Option<u64>,
) -> Duration {
    let remaining_ticks = expires_at_tick.saturating_sub(
        observed_tick
            .unwrap_or(activated_at_tick)
            .max(activated_at_tick),
    );
    let whole_seconds = remaining_ticks / crate::timing::SIMULATION_TICK_HZ;
    let subsecond_ticks = u32::try_from(remaining_ticks % crate::timing::SIMULATION_TICK_HZ)
        .expect("subsecond tick remainder fits u32");
    Duration::from_secs(whole_seconds)
        .saturating_add(crate::timing::SIMULATION_TICK.saturating_mul(subsecond_ticks))
}

fn combat_effect_lifetime(
    cue: &crate::combat::CombatCue,
    current_tick: Option<u64>,
    reduced: bool,
) -> (Duration, Option<u64>) {
    if let crate::combat::CombatCue::RevealScanActivated {
        tick,
        expires_at_tick,
        ..
    } = cue
    {
        return (
            reveal_ring_remaining_duration(*tick, *expires_at_tick, current_tick),
            Some(*expires_at_tick),
        );
    }
    (
        Duration::from_secs_f32(if reduced { 0.10 } else { 0.18 }),
        None,
    )
}

fn cue_effect(
    cue: &crate::combat::CombatCue,
    materials: &Material3dAssets,
) -> Option<(Vec2, Handle<StandardMaterial>, f32)> {
    use crate::combat::CombatCue as C;
    match cue {
        C::AttackAccepted { position, .. } | C::SentryFired { position, .. } => {
            Some((position.as_vec2(), materials.effect_muzzle.clone(), 8.0))
        }
        C::DeliveryImpact { position, .. }
        | C::LobLanded { position, .. }
        | C::MeleeContact { position, .. }
        | C::DeployableRemoved { position, .. } => {
            Some((position.as_vec2(), materials.effect_impact.clone(), 14.0))
        }
        C::DamageApplied { position, .. }
        | C::EffectApplied { position, .. }
        | C::FighterDefeated { position, .. } => {
            Some((position.as_vec2(), materials.effect_damage.clone(), 12.0))
        }
        C::FighterReset { position, .. } => {
            Some((position.as_vec2(), materials.effect_muzzle.clone(), 16.0))
        }
        C::RevealScanActivated {
            center,
            radius_milliunits,
            ..
        }
        | C::ElementalFieldActivated {
            center,
            radius_milliunits,
            ..
        } => Some((
            center.as_vec2(),
            materials.scan_area.clone(),
            crate::builds::world_units_from_milliunits(*radius_milliunits).unwrap_or(0.0),
        )),
        C::DemolitionStrikeActivated {
            center,
            radius_milliunits,
            ..
        } => Some((
            center.as_vec2(),
            materials.demolition_area.clone(),
            crate::builds::world_units_from_milliunits(*radius_milliunits).unwrap_or(0.0),
        )),
        C::Muzzle { .. }
        | C::ConeSprayPulse { .. }
        | C::Impact { .. }
        | C::Damage { .. }
        | C::Defeat { .. }
        | C::Reset { .. }
        | C::SelfCloakActivated { .. }
        | C::SelfCloakEnded { .. }
        | C::ForcedRevealApplied { .. } => None,
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Time is a Bevy system resource parameter"
)]
pub(in super::super) fn cleanup_combat_effects(
    mut commands: Commands,
    time: Res<Time<Real>>,
    authoritative_ticks: Query<&AuthoritativeTick>,
    mut effects: Query<(Entity, &mut CombatEffect3d)>,
) {
    let current_tick = authoritative_ticks.iter().map(|tick| tick.0).max();
    for (entity, mut effect) in &mut effects {
        effect.timer.tick(time.delta());
        let authoritative_expiry = effect
            .expires_at_tick
            .zip(current_tick)
            .is_some_and(|(expires_at_tick, now)| now >= expires_at_tick);
        if authoritative_expiry || effect.timer.is_finished() {
            commands.entity(entity).try_despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pending_effect(label: &'static str, lifetime: Duration) -> PendingCombatEffect {
        PendingCombatEffect {
            lifetime,
            expires_at_tick: None,
            mesh: Handle::default(),
            material: Handle::default(),
            transform: Transform::default(),
            label,
        }
    }

    fn effect_allocation_app() -> App {
        let mut app = App::new();
        app.add_message::<PendingCombatEffect>()
            .add_systems(Update, materialize_combat_effects);
        app
    }

    #[test]
    fn effect_allocation_evicts_oldest_and_settles_at_capacity() {
        let mut app = effect_allocation_app();
        let existing = (0..MAX_EFFECTS + 2)
            .map(|order| {
                app.world_mut()
                    .spawn(CombatEffect3d {
                        timer: Timer::new(Duration::from_mins(1), TimerMode::Once),
                        expires_at_tick: None,
                        order: u64::try_from(order).expect("bounded effect order fits u64"),
                    })
                    .id()
            })
            .collect::<Vec<_>>();
        app.world_mut()
            .write_message(pending_effect("combat family", Duration::from_millis(180)));
        app.world_mut().write_message(pending_effect(
            "world-object family",
            Duration::from_millis(280),
        ));

        app.update();

        for evicted in &existing[..4] {
            assert!(app.world().get_entity(*evicted).is_err());
        }
        let world = app.world_mut();
        let mut effects = world.query::<&CombatEffect3d>();
        let orders = effects
            .iter(world)
            .map(|effect| effect.order)
            .collect::<Vec<_>>();
        assert_eq!(orders.len(), MAX_EFFECTS);
        assert_eq!(orders.iter().copied().min(), Some(4));
        assert_eq!(orders.iter().copied().max(), Some(99));
    }

    #[test]
    fn effect_allocation_preserves_cross_family_message_order() {
        let mut app = effect_allocation_app();
        for label in ["combat", "world-object", "pickup", "heist"] {
            app.world_mut()
                .write_message(pending_effect(label, Duration::from_millis(100)));
        }

        app.update();

        let world = app.world_mut();
        let mut effects = world.query::<(&CombatEffect3d, &Name)>();
        let mut ordered = effects
            .iter(world)
            .map(|(effect, name)| (effect.order, name.as_str()))
            .collect::<Vec<_>>();
        ordered.sort_unstable_by_key(|(order, _)| *order);
        assert_eq!(
            ordered
                .into_iter()
                .map(|(_, name)| name)
                .collect::<Vec<_>>(),
            ["combat", "world-object", "pickup", "heist"]
        );
    }

    #[test]
    fn reduced_effect_duration_survives_central_allocation() {
        let cue = crate::combat::CombatCue::Reset {
            event_id: crate::combat::CombatEventId(1),
            tick: 1,
            target: NetworkEntityId(1),
            position: Vec2::ZERO.into(),
        };
        let normal = combat_effect_lifetime(&cue, None, false).0;
        let reduced = combat_effect_lifetime(&cue, None, true).0;
        assert!(reduced < normal);

        let mut app = effect_allocation_app();
        app.world_mut()
            .write_message(pending_effect("reduced", reduced));
        app.update();
        let world = app.world_mut();
        let mut effects = world.query::<&CombatEffect3d>();
        assert_eq!(effects.single(world).unwrap().timer.duration(), reduced);
    }

    #[test]
    fn authoritative_effect_expiry_cleans_up_materialized_effect() {
        let mut app = App::new();
        app.add_message::<PendingCombatEffect>()
            .insert_resource(Time::<Real>::default())
            .add_systems(
                Update,
                (materialize_combat_effects, cleanup_combat_effects).chain(),
            );
        app.world_mut().spawn(AuthoritativeTick(40));
        let mut descriptor = pending_effect("authoritative", Duration::from_mins(1));
        descriptor.expires_at_tick = Some(40);
        app.world_mut().write_message(descriptor);

        app.update();
        let world = app.world_mut();
        let mut effects = world.query_filtered::<Entity, With<CombatEffect3d>>();
        assert_eq!(effects.iter(world).count(), 0);
    }

    #[test]
    fn reveal_area_lifetime_tracks_authoritative_effect_deadline() {
        assert_eq!(
            reveal_ring_remaining_duration(100, 400, Some(100)),
            Duration::from_secs(5)
        );
        assert_eq!(
            reveal_ring_remaining_duration(100, 400, Some(400)),
            Duration::ZERO
        );
    }
}
