use super::super::*;
use super::common::GROUND_EFFECT_HEIGHT;
use super::vfx_catalog::{VfxCatalog, VfxCueFamily, VfxLifetime, VfxRendererFamily};
use crate::combat::client::DeduplicatedCombatCue;
use std::collections::{HashMap, VecDeque};

const MAX_EFFECTS: usize = 96;
const SPHERE_CENTER_HEIGHT_FRACTION: f32 = 0.45;

#[derive(Component)]
pub(in super::super) struct CombatEffect3d {
    timer: Timer,
    expires_at_tick: Option<u64>,
    order: u64,
    profile_id: String,
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
    profile_id: String,
    concurrency_cap: usize,
}

impl PendingCombatEffect {
    fn spawn(&self, commands: &mut Commands, order: u64) -> Entity {
        commands
            .spawn((
                CombatEffect3d {
                    timer: Timer::new(self.lifetime, TimerMode::Once),
                    expires_at_tick: self.expires_at_tick,
                    order,
                    profile_id: self.profile_id.clone(),
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

#[derive(Clone, Copy)]
enum EffectAnchor {
    Center,
    RaisedFixed(f32),
}

#[derive(Clone, Copy)]
struct CatalogEffectRequest {
    family: VfxCueFamily,
    reduced: bool,
    position: Vec2,
    base_scale: f32,
    anchor: EffectAnchor,
    deadline: Option<(u64, u64, Option<u64>)>,
    label: &'static str,
}

fn catalog_effect(
    request: CatalogEffectRequest,
    catalog: &VfxCatalog,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
) -> PendingCombatEffect {
    let profile =
        catalog.resolve_for_request(request.family, request.reduced, request.deadline.is_some());
    let scale = request.base_scale * profile.scale_multiplier;
    let (lifetime, expires_at_tick) = match (profile.lifetime, request.deadline) {
        (VfxLifetime::AuthoritativeDeadline, Some((activated, expires, observed))) => (
            reveal_ring_remaining_duration(activated, expires, observed),
            Some(expires),
        ),
        (VfxLifetime::Millis(millis), _) => (Duration::from_millis(u64::from(millis)), None),
        (VfxLifetime::AuthoritativeDeadline, None) => {
            unreachable!("validated VFX resolution falls back when no deadline is available")
        }
    };
    let transform = effect_transform(
        profile.renderer,
        request.position,
        request.base_scale,
        scale,
        request.anchor,
    );
    PendingCombatEffect {
        lifetime,
        expires_at_tick,
        mesh: match profile.renderer {
            VfxRendererFamily::Sphere => primitives.effect_sphere.clone(),
            VfxRendererFamily::GroundRing => primitives.area_ring.clone(),
        },
        material: vfx_material(&profile.material, materials),
        transform,
        label: request.label,
        profile_id: profile.id.clone(),
        concurrency_cap: profile.concurrency_cap,
    }
}

fn effect_transform(
    renderer: VfxRendererFamily,
    position: Vec2,
    base_scale: f32,
    rendered_scale: f32,
    anchor: EffectAnchor,
) -> Transform {
    match renderer {
        VfxRendererFamily::GroundRing => Transform {
            translation: ground_position(position) + Vec3::Y * GROUND_EFFECT_HEIGHT,
            rotation: Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2),
            scale: Vec3::splat(rendered_scale),
        },
        VfxRendererFamily::Sphere => {
            let height = match anchor {
                EffectAnchor::Center => base_scale * SPHERE_CENTER_HEIGHT_FRACTION,
                EffectAnchor::RaisedFixed(height) => height,
            };
            Transform::from_translation(ground_position(position) + Vec3::Y * height)
                .with_scale(Vec3::splat(rendered_scale))
        }
    }
}

fn vfx_material(key: &str, materials: &Material3dAssets) -> Handle<StandardMaterial> {
    match key {
        "effect_muzzle" => materials.effect_muzzle.clone(),
        "effect_damage" => materials.effect_damage.clone(),
        "scan_area" => materials.scan_area.clone(),
        "demolition_area" => materials.demolition_area.clone(),
        "pickup_glow" => materials.pickup_glow.clone(),
        _ => materials.effect_impact.clone(),
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
    catalog: Res<VfxCatalog>,
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
        let Some((family, position, scale)) = cue_effect(cue) else {
            continue;
        };
        let scan_pulse = matches!(cue, crate::combat::CombatCue::RevealScanActivated { .. });
        let deadline = if let crate::combat::CombatCue::RevealScanActivated {
            tick,
            expires_at_tick,
            ..
        } = cue
        {
            Some((*tick, *expires_at_tick, current_tick))
        } else {
            None
        };
        pending_effects.write(catalog_effect(
            CatalogEffectRequest {
                family,
                reduced,
                position,
                base_scale: scale,
                anchor: EffectAnchor::Center,
                deadline,
                label: if scan_pulse {
                    "V9 active Reveal Scan area"
                } else {
                    "V3 bounded combat cue effect"
                },
            },
            &catalog,
            &primitives,
            &materials,
        ));
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
    catalog: Res<VfxCatalog>,
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
        pending_effects.write(catalog_effect(
            CatalogEffectRequest {
                family: if exploded {
                    VfxCueFamily::WorldObjectExploded
                } else {
                    VfxCueFamily::WorldObjectDamaged
                },
                reduced,
                position,
                base_scale: radius,
                anchor: if exploded {
                    EffectAnchor::Center
                } else {
                    EffectAnchor::RaisedFixed(8.0)
                },
                deadline: None,
                label: if exploded {
                    "V10 authoritative oil-barrel blast"
                } else {
                    "V10 oil-barrel damage response"
                },
            },
            &catalog,
            &primitives,
            &materials,
        ));
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
    catalog: Res<VfxCatalog>,
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
        let (identity, position, radius, family, label) = match cue {
            crate::map::PickupCue::Spawned {
                identity, position, ..
            } => (
                identity,
                position.as_vec2(),
                22.0,
                VfxCueFamily::PickupSpawned,
                "V10 chest restoration drop",
            ),
            crate::map::PickupCue::Collected {
                identity, position, ..
            } => (
                identity,
                position.as_vec2(),
                18.0,
                VfxCueFamily::PickupCollected,
                "V10 restoration collected",
            ),
            crate::map::PickupCue::Expired {
                identity, position, ..
            } => (
                identity,
                position.as_vec2(),
                12.0,
                VfxCueFamily::PickupExpired,
                "V10 restoration expired",
            ),
        };
        if identity.generation != map_state.generation_id() {
            continue;
        }
        let ring = !matches!(family, VfxCueFamily::PickupCollected);
        pending_effects.write(catalog_effect(
            CatalogEffectRequest {
                family,
                reduced,
                position,
                base_scale: radius,
                anchor: if ring {
                    EffectAnchor::Center
                } else {
                    EffectAnchor::RaisedFixed(15.0)
                },
                deadline: None,
                label,
            },
            &catalog,
            &primitives,
            &materials,
        ));
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
    catalog: Res<VfxCatalog>,
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
        let (radius, family) = match cue.kind {
            crate::matchplay::HeistObjectiveCueKind::Damaged => (12.0, VfxCueFamily::HeistDamaged),
            crate::matchplay::HeistObjectiveCueKind::Critical => {
                (30.0, VfxCueFamily::HeistCritical)
            }
            crate::matchplay::HeistObjectiveCueKind::Destroyed => {
                (72.0, VfxCueFamily::HeistDestroyed)
            }
        };
        pending_effects.write(catalog_effect(
            CatalogEffectRequest {
                family,
                reduced,
                position: cue.position.as_vec2(),
                base_scale: radius,
                anchor: EffectAnchor::Center,
                deadline: None,
                label: match cue.kind {
                    crate::matchplay::HeistObjectiveCueKind::Damaged => "Heist safe hit cue",
                    crate::matchplay::HeistObjectiveCueKind::Critical => "Heist safe critical cue",
                    crate::matchplay::HeistObjectiveCueKind::Destroyed => {
                        "Heist safe destroyed cue"
                    }
                },
            },
            &catalog,
            &primitives,
            &materials,
        ));
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
        .map(|(entity, effect)| (effect.order, entity, effect.profile_id.clone()))
        .collect::<VecDeque<_>>();
    active
        .make_contiguous()
        .sort_unstable_by_key(|(order, entity, _)| (*order, entity.to_bits()));
    if let Some((maximum_order, ..)) = active.back() {
        sequence.0 = sequence.0.max(*maximum_order);
    }
    while active.len() > MAX_EFFECTS {
        if let Some((_, oldest, _)) = active.pop_front() {
            commands.entity(oldest).try_despawn();
        }
    }

    for descriptor in pending_effects.read() {
        while active
            .iter()
            .filter(|(_, _, profile)| profile == &descriptor.profile_id)
            .count()
            >= descriptor.concurrency_cap
        {
            let Some(index) = active
                .iter()
                .position(|(_, _, profile)| profile == &descriptor.profile_id)
            else {
                break;
            };
            if let Some((_, oldest, _)) = active.remove(index) {
                commands.entity(oldest).try_despawn();
            }
        }
        while active.len() >= MAX_EFFECTS {
            if let Some((_, oldest, _)) = active.pop_front() {
                commands.entity(oldest).try_despawn();
            }
        }
        sequence.0 = sequence.0.saturating_add(1);
        let entity = descriptor.spawn(&mut commands, sequence.0);
        active.push_back((sequence.0, entity, descriptor.profile_id.clone()));
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

fn cue_effect(cue: &crate::combat::CombatCue) -> Option<(VfxCueFamily, Vec2, f32)> {
    use crate::combat::CombatCue as C;
    match cue {
        C::AttackAccepted { position, .. } | C::SentryFired { position, .. } => {
            Some((VfxCueFamily::CombatMuzzle, position.as_vec2(), 8.0))
        }
        C::DeliveryImpact { position, .. }
        | C::LobLanded { position, .. }
        | C::MeleeContact { position, .. }
        | C::DeployableRemoved { position, .. } => {
            Some((VfxCueFamily::CombatImpact, position.as_vec2(), 14.0))
        }
        C::DamageApplied { position, .. }
        | C::EffectApplied { position, .. }
        | C::FighterDefeated { position, .. } => {
            Some((VfxCueFamily::CombatDamage, position.as_vec2(), 12.0))
        }
        C::FighterReset { position, .. } => {
            Some((VfxCueFamily::CombatReset, position.as_vec2(), 16.0))
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
            if matches!(cue, C::RevealScanActivated { .. }) {
                VfxCueFamily::RevealScan
            } else {
                VfxCueFamily::ElementalField
            },
            center.as_vec2(),
            crate::builds::world_units_from_milliunits(*radius_milliunits).unwrap_or(0.0),
        )),
        C::DemolitionStrikeActivated {
            center,
            radius_milliunits,
            ..
        } => Some((
            VfxCueFamily::DemolitionStrike,
            center.as_vec2(),
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
            profile_id: label.to_string(),
            concurrency_cap: MAX_EFFECTS,
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
                        profile_id: "existing".to_string(),
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
    fn effect_allocation_enforces_the_catalog_profile_cap() {
        let mut app = effect_allocation_app();
        for label in ["first", "second", "third"] {
            let mut effect = pending_effect(label, Duration::from_millis(100));
            effect.profile_id = "capped-family".to_string();
            effect.concurrency_cap = 2;
            app.world_mut().write_message(effect);
        }

        app.update();

        let world = app.world_mut();
        let mut effects = world.query::<(&CombatEffect3d, &Name)>();
        let mut retained = effects
            .iter(world)
            .map(|(effect, name)| (effect.order, name.as_str()))
            .collect::<Vec<_>>();
        retained.sort_unstable_by_key(|(order, _)| *order);
        assert_eq!(
            retained
                .into_iter()
                .map(|(_, name)| name)
                .collect::<Vec<_>>(),
            ["second", "third"]
        );
    }

    #[test]
    fn reduced_effect_duration_survives_central_allocation() {
        let catalog = VfxCatalog::embedded().unwrap();
        let normal = catalog.resolve(VfxCueFamily::CombatReset, false);
        let reduced = catalog.resolve(VfxCueFamily::CombatReset, true);
        let VfxLifetime::Millis(normal) = normal.lifetime else {
            panic!("reset VFX uses a fixed lifetime")
        };
        let VfxLifetime::Millis(reduced) = reduced.lifetime else {
            panic!("reduced reset VFX uses a fixed lifetime")
        };
        let normal = Duration::from_millis(u64::from(normal));
        let reduced = Duration::from_millis(u64::from(reduced));
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
    fn reduced_sphere_scale_preserves_the_semantic_center_height() {
        let normal = effect_transform(
            VfxRendererFamily::Sphere,
            Vec2::new(10.0, 20.0),
            8.0,
            8.0,
            EffectAnchor::Center,
        );
        let reduced = effect_transform(
            VfxRendererFamily::Sphere,
            Vec2::new(10.0, 20.0),
            8.0,
            5.2,
            EffectAnchor::Center,
        );

        assert_eq!(reduced.translation, normal.translation);
        assert_eq!(normal.scale, Vec3::splat(8.0));
        assert_eq!(reduced.scale, Vec3::splat(5.2));
    }

    #[test]
    fn renderer_family_owns_ground_ring_orientation_and_placement() {
        let ring = effect_transform(
            VfxRendererFamily::GroundRing,
            Vec2::new(10.0, 20.0),
            8.0,
            5.2,
            EffectAnchor::RaisedFixed(99.0),
        );

        assert_eq!(
            ring.translation,
            ground_position(Vec2::new(10.0, 20.0)) + Vec3::Y * GROUND_EFFECT_HEIGHT
        );
        assert_eq!(
            ring.rotation,
            Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2)
        );
        assert_eq!(ring.scale, Vec3::splat(5.2));
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
