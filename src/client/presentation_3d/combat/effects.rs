use super::super::*;
use crate::client::vfx::{
    VfxLifetime, VfxMaterialKey, VfxProfile, VfxRegistry, VfxRendererFamily, VfxRequest,
};
use crate::combat::client::DeduplicatedCombatCue;
use std::collections::{HashMap, VecDeque};

const MAX_EFFECTS: usize = 96;

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

fn resolved_effect(
    request: &VfxRequest,
    profile: &VfxProfile,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
) -> PendingCombatEffect {
    let (lifetime, expires_at_tick) = match (profile.lifetime, request.deadline) {
        (VfxLifetime::AuthoritativeDeadline, Some(deadline)) => (
            reveal_ring_remaining_duration(
                deadline.activated_at_tick,
                deadline.expires_at_tick,
                deadline.observed_at_tick,
            ),
            Some(deadline.expires_at_tick),
        ),
        (VfxLifetime::Millis(millis), _) => (Duration::from_millis(u64::from(millis)), None),
        (VfxLifetime::AuthoritativeDeadline, None) => {
            unreachable!("the VFX registry rejects deadline profiles without a deadline")
        }
    };
    let transform = effect_transform(profile, request.position, request.authoritative_radius);
    PendingCombatEffect {
        lifetime,
        expires_at_tick,
        mesh: match profile.renderer {
            VfxRendererFamily::Sphere => primitives.effect_sphere.clone(),
            VfxRendererFamily::GroundRing => primitives.area_ring.clone(),
        },
        material: vfx_material(profile.material, materials),
        transform,
        label: request.label,
        profile_id: profile.id.clone(),
        concurrency_cap: profile.concurrency_cap,
    }
}

fn effect_transform(
    profile: &VfxProfile,
    position: Vec2,
    authoritative_radius: Option<f32>,
) -> Transform {
    let scale = profile
        .scale
        .resolve(authoritative_radius)
        .expect("validated VFX family supplies required authoritative radius");
    let height = profile
        .anchor
        .resolve_height(authoritative_radius)
        .expect("validated VFX family supplies required authoritative radius");
    match profile.renderer {
        VfxRendererFamily::GroundRing => Transform {
            translation: ground_position(position) + Vec3::Y * height,
            rotation: Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2),
            scale: Vec3::splat(scale),
        },
        VfxRendererFamily::Sphere => {
            Transform::from_translation(ground_position(position) + Vec3::Y * height)
                .with_scale(Vec3::splat(scale))
        }
    }
}

fn vfx_material(key: VfxMaterialKey, materials: &Material3dAssets) -> Handle<StandardMaterial> {
    match key {
        VfxMaterialKey::EffectMuzzle => materials.effect_muzzle.clone(),
        VfxMaterialKey::EffectImpact => materials.effect_impact.clone(),
        VfxMaterialKey::EffectDamage => materials.effect_damage.clone(),
        VfxMaterialKey::ScanArea => materials.scan_area.clone(),
        VfxMaterialKey::DemolitionArea => materials.demolition_area.clone(),
        VfxMaterialKey::PickupGlow => materials.pickup_glow.clone(),
    }
}
/// Keeps fighter animation feedback separate from semantic VFX production.
pub(in super::super) fn animate_attack_acceptance(
    mut cues: MessageReader<DeduplicatedCombatCue>,
    owners: Query<(Entity, &NetworkEntityId), With<Fighter>>,
    mut visuals: Query<(Entity, &CombatVisualOwner, &mut V3FighterVisual)>,
) {
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
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    reason = "the renderer adapter resolves one bounded request batch against retained render assets"
)]
pub(in super::super) fn resolve_vfx_requests(
    mut requests: MessageReader<VfxRequest>,
    mut pending_effects: MessageWriter<PendingCombatEffect>,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    registry: Res<VfxRegistry>,
    settings: Option<Res<ClientShellSettings>>,
) {
    let reduced = settings.is_some_and(|value| value.reduced_combat_effects);
    let ordered = ordered_vfx_requests(requests.read().copied());
    for request in &ordered {
        let Some(profile) = registry.resolve(request, reduced) else {
            continue;
        };
        pending_effects.write(resolved_effect(request, profile, &primitives, &materials));
    }
}

fn ordered_vfx_requests(requests: impl IntoIterator<Item = VfxRequest>) -> Vec<VfxRequest> {
    let mut ordered = requests.into_iter().collect::<Vec<_>>();
    ordered.sort_by_key(|request| (request.order, request.key));
    ordered
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
    use crate::client::vfx::{
        COMBAT_IMPACT_VFX, COMBAT_MUZZLE_VFX, COMBAT_RESET_VFX, COMBAT_VFX_PRODUCER_RANK,
        CombatVfxProducerPlugin, ELEMENTAL_FIELD_VFX, HEIST_DAMAGED_VFX, HEIST_VFX_PRODUCER_RANK,
        HeistVfxProducerPlugin, PICKUP_COLLECTED_VFX, PICKUP_VFX_PRODUCER_RANK,
        PickupVfxProducerPlugin, REVEAL_SCAN_VFX, VfxDeadline, VfxRegistryPlugin,
        VfxRequestCapabilities, VfxRequestKey, VfxRequestOrder, VfxRequestRegistration,
        WORLD_OBJECT_DAMAGED_VFX, WORLD_OBJECT_VFX_PRODUCER_RANK, WorldObjectVfxProducerPlugin,
    };

    fn registry() -> VfxRegistry {
        let mut app = App::new();
        app.add_plugins(VfxRegistryPlugin).add_plugins((
            CombatVfxProducerPlugin,
            WorldObjectVfxProducerPlugin,
            PickupVfxProducerPlugin,
            HeistVfxProducerPlugin,
        ));
        crate::test_app::finalize(&mut app);
        app.world().resource::<VfxRegistry>().clone()
    }

    fn profile(
        registry: &VfxRegistry,
        key: VfxRequestKey,
        reduced: bool,
        radius: Option<f32>,
        deadline: Option<VfxDeadline>,
    ) -> VfxProfile {
        let request = VfxRequest::try_new(
            key,
            VfxRequestOrder::new(registry.producer_rank(key).unwrap(), 1),
            Vec2::ZERO,
            radius,
            deadline,
            "renderer profile test",
        )
        .unwrap();
        registry.resolve(&request, reduced).unwrap().clone()
    }

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
    fn request_ordering_preserves_producer_rank_then_event_fifo() {
        let request = |key, rank, event_id, label| {
            VfxRequest::try_new(
                key,
                VfxRequestOrder::new(rank, event_id),
                Vec2::ZERO,
                None,
                None,
                label,
            )
            .unwrap()
        };
        let ordered = ordered_vfx_requests([
            request(HEIST_DAMAGED_VFX, HEIST_VFX_PRODUCER_RANK, 2, "heist"),
            request(
                COMBAT_IMPACT_VFX,
                COMBAT_VFX_PRODUCER_RANK,
                9,
                "combat impact",
            ),
            request(
                WORLD_OBJECT_DAMAGED_VFX,
                WORLD_OBJECT_VFX_PRODUCER_RANK,
                5,
                "world object",
            ),
            request(
                COMBAT_MUZZLE_VFX,
                COMBAT_VFX_PRODUCER_RANK,
                3,
                "combat muzzle",
            ),
            request(PICKUP_COLLECTED_VFX, PICKUP_VFX_PRODUCER_RANK, 1, "pickup"),
        ]);

        assert_eq!(
            ordered
                .iter()
                .map(|request| (request.key, request.order.event_id))
                .collect::<Vec<_>>(),
            [
                (COMBAT_MUZZLE_VFX, 3),
                (COMBAT_IMPACT_VFX, 9),
                (WORLD_OBJECT_DAMAGED_VFX, 5),
                (PICKUP_COLLECTED_VFX, 1),
                (HEIST_DAMAGED_VFX, 2),
            ]
        );
    }

    #[test]
    fn synthetic_request_mapping_materializes_through_existing_renderer_primitive() {
        const SYNTHETIC: VfxRequestKey = VfxRequestKey::new("synthetic.spark");
        const SYNTHETIC_RANK: u16 = 900;
        let registry = registry()
            .with_test_mapping(
                VfxRequestRegistration::new(
                    SYNTHETIC,
                    SYNTHETIC_RANK,
                    VfxRequestCapabilities::NONE,
                ),
                "impact",
            )
            .unwrap();
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<crate::builds::BuildCatalogResource>()
            .insert_resource(registry)
            .add_message::<VfxRequest>()
            .add_message::<PendingCombatEffect>()
            .add_systems(Startup, setup_3d_foundation)
            .add_systems(
                Update,
                (resolve_vfx_requests, materialize_combat_effects).chain(),
            );
        app.world_mut().write_message(
            VfxRequest::try_new(
                SYNTHETIC,
                VfxRequestOrder::new(SYNTHETIC_RANK, 1),
                Vec2::new(10.0, 20.0),
                None,
                None,
                "synthetic renderer extension",
            )
            .unwrap(),
        );

        app.update();

        let sphere = app
            .world()
            .resource::<Primitive3dAssets>()
            .effect_sphere
            .clone();
        let world = app.world_mut();
        let mut effects = world.query::<(&CombatEffect3d, &Name, &Mesh3d, &Transform)>();
        let (effect, name, mesh, transform) = effects.single(world).unwrap();
        assert_eq!(effect.profile_id, "impact");
        assert_eq!(name.as_str(), "synthetic renderer extension");
        assert_eq!(mesh.0, sphere);
        assert_eq!(transform.translation, Vec3::new(10.0, 6.3, -20.0));
        assert_eq!(transform.scale, Vec3::splat(14.0));
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
        let registry = registry();
        let normal = profile(&registry, COMBAT_RESET_VFX, false, None, None);
        let reduced = profile(&registry, COMBAT_RESET_VFX, true, None, None);
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
        let registry = registry();
        let normal = effect_transform(
            &profile(&registry, COMBAT_MUZZLE_VFX, false, None, None),
            Vec2::new(10.0, 20.0),
            None,
        );
        let reduced = effect_transform(
            &profile(&registry, COMBAT_MUZZLE_VFX, true, None, None),
            Vec2::new(10.0, 20.0),
            None,
        );

        assert_eq!(reduced.translation, normal.translation);
        assert_eq!(normal.scale, Vec3::splat(8.0));
        assert_eq!(reduced.scale, Vec3::splat(5.2));
    }

    #[test]
    fn renderer_family_owns_ground_ring_orientation_and_placement() {
        let registry = registry();
        let profile = profile(
            &registry,
            REVEAL_SCAN_VFX,
            false,
            Some(5.2),
            Some(VfxDeadline::new(1, 2, Some(1))),
        );
        let ring = effect_transform(&profile, Vec2::new(10.0, 20.0), Some(5.2));

        assert_eq!(
            ring.translation,
            ground_position(Vec2::new(10.0, 20.0)) + Vec3::Y * 2.5
        );
        assert_eq!(
            ring.rotation,
            Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2)
        );
        assert_eq!(ring.scale, Vec3::splat(5.2));
    }

    #[test]
    fn authored_fixed_and_authoritative_radius_geometry_reaches_runtime_transform() {
        let registry = registry();
        let fixed = effect_transform(
            &profile(&registry, WORLD_OBJECT_DAMAGED_VFX, true, None, None),
            Vec2::ZERO,
            None,
        );
        assert!((fixed.translation.y - 8.0).abs() < f32::EPSILON);
        assert_eq!(fixed.scale, Vec3::splat(7.0));

        let radius = 20.0;
        let area = effect_transform(
            &profile(&registry, ELEMENTAL_FIELD_VFX, true, Some(radius), None),
            Vec2::ZERO,
            Some(radius),
        );
        assert!((area.translation.y - radius * 0.45).abs() < f32::EPSILON);
        assert_eq!(area.scale, Vec3::splat(radius * 0.65));
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
