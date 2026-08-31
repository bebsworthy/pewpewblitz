use super::super::*;
use crate::client::vfx::{
    CombatVisualSignature, ResolvedVfxRecipe, VfxDeadline, VfxLifetime, VfxMaterialKey,
    VfxMeshLayer, VfxReadabilityClass, VfxRegistry, VfxRendererFamily, VfxRequest,
    VfxSignatureCache, VfxTimeline, vfx_counter_sample, vfx_presentation_variant,
};
use crate::combat::client::DeduplicatedCombatCue;
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};

const MAX_EFFECTS: usize = 96;

#[derive(Component)]
pub(in crate::client) struct CombatEffect3d {
    timer: Timer,
    expires_at_tick: Option<u64>,
    order: u64,
    recipe_id: String,
}

impl CombatEffect3d {
    pub(in crate::client) fn advance_and_is_finished(
        &mut self,
        delta: Duration,
        current_tick: Option<u64>,
    ) -> bool {
        self.timer.tick(delta);
        let authoritative_expiry = self
            .expires_at_tick
            .zip(current_tick)
            .is_some_and(|(expires_at_tick, now)| now >= expires_at_tick);
        authoritative_expiry || self.timer.is_finished()
    }
}

#[derive(Default)]
pub(in crate::client) struct CombatEffectSequence(u64);

#[derive(Message)]
pub(in crate::client) struct PendingCombatEffect {
    lifetime: Duration,
    expires_at_tick: Option<u64>,
    layers: Vec<PendingCombatEffectLayer>,
    label: &'static str,
    recipe_id: String,
    concurrency_cap: usize,
    signature: CombatVisualSignature,
}

struct PendingCombatEffectLayer {
    id: String,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    transform: Transform,
    timeline: VfxTimeline,
    duration: Duration,
    casts_shadows: bool,
    receives_shadows: bool,
    readability: VfxReadabilityClass,
    variant_sample: u64,
}

#[derive(Component)]
pub(in crate::client) struct CombatEffectLayer3d {
    root: Entity,
    layer_index: u8,
    layer_id: String,
    timeline: VfxTimeline,
    duration: Duration,
    base_scale: Vec3,
    readability: VfxReadabilityClass,
    variant_sample: u64,
}

impl PendingCombatEffect {
    fn spawn(&self, commands: &mut Commands, order: u64) -> Entity {
        let root = commands
            .spawn((
                CombatEffect3d {
                    timer: Timer::new(self.lifetime, TimerMode::Once),
                    expires_at_tick: self.expires_at_tick,
                    order,
                    recipe_id: self.recipe_id.clone(),
                },
                self.signature,
                Transform::default(),
                Visibility::default(),
                Name::new(self.label),
            ))
            .id();
        commands.entity(root).with_children(|parent| {
            for (index, layer) in self.layers.iter().enumerate() {
                let initial_scale = layer
                    .timeline
                    .scale_at(Duration::ZERO, layer.duration)
                    .unwrap_or(0.0);
                let mut transform = layer.transform;
                let base_scale = transform.scale;
                transform.scale *= initial_scale;
                let mut child = parent.spawn((
                    CombatEffectLayer3d {
                        root,
                        layer_index: u8::try_from(index)
                            .expect("validated recipe layer count fits u8"),
                        layer_id: layer.id.clone(),
                        timeline: layer.timeline,
                        duration: layer.duration,
                        base_scale,
                        readability: layer.readability,
                        variant_sample: layer.variant_sample,
                    },
                    Mesh3d(layer.mesh.clone()),
                    MeshMaterial3d(layer.material.clone()),
                    transform,
                    if layer.timeline.delay_millis == 0 {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    },
                    Name::new(format!("{} / {}", self.label, layer.id)),
                ));
                if !layer.casts_shadows {
                    child.insert(NotShadowCaster);
                }
                if !layer.receives_shadows {
                    child.insert(NotShadowReceiver);
                }
            }
        });
        root
    }
}

fn resolved_effect(
    request: &VfxRequest,
    recipe: &ResolvedVfxRecipe,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
) -> PendingCombatEffect {
    let mut lifetime = Duration::ZERO;
    let mut expires_at_tick = None;
    let layers = recipe
        .layers
        .iter()
        .enumerate()
        .map(|(occurrence, layer)| {
            let (duration, authoritative_expiry) = resolved_layer_duration(layer, request.deadline);
            lifetime = lifetime.max(
                Duration::from_millis(u64::from(layer.timeline.delay_millis))
                    .saturating_add(duration),
            );
            expires_at_tick = expires_at_tick.or(authoritative_expiry);
            PendingCombatEffectLayer {
                id: layer.id.clone(),
                mesh: match layer.renderer {
                    VfxRendererFamily::Sphere => primitives.effect_sphere.clone(),
                    VfxRendererFamily::GroundRing => primitives.area_ring.clone(),
                },
                material: vfx_material(layer.material, materials),
                transform: effect_layer_transform(
                    layer,
                    request.position,
                    request.authoritative_radius,
                ),
                timeline: layer.timeline,
                duration,
                casts_shadows: layer.casts_shadows,
                receives_shadows: layer.receives_shadows,
                readability: layer.readability,
                variant_sample: vfx_counter_sample(
                    request.order.event_id,
                    &recipe.id,
                    &layer.id,
                    u8::try_from(occurrence).expect("validated recipe layer count fits u8"),
                    0,
                ),
            }
        })
        .collect();
    PendingCombatEffect {
        lifetime,
        expires_at_tick,
        layers,
        label: request.label,
        recipe_id: recipe.id.clone(),
        concurrency_cap: recipe.concurrency_cap,
        signature: request.signature,
    }
}

fn resolved_layer_duration(
    layer: &VfxMeshLayer,
    deadline: Option<VfxDeadline>,
) -> (Duration, Option<u64>) {
    match (layer.timeline.duration, deadline) {
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
            unreachable!("the VFX registry rejects deadline recipes without a deadline")
        }
    }
}

fn effect_layer_transform(
    layer: &VfxMeshLayer,
    position: Vec2,
    authoritative_radius: Option<f32>,
) -> Transform {
    let scale = layer
        .scale
        .resolve(authoritative_radius)
        .expect("validated VFX layer supplies required authoritative radius");
    let height = layer
        .anchor
        .resolve_height(authoritative_radius)
        .expect("validated VFX layer supplies required authoritative radius");
    match layer.renderer {
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

pub(in crate::client) fn animate_combat_effect_layers(
    effects: Query<&CombatEffect3d>,
    mut layers: Query<(&CombatEffectLayer3d, &mut Transform, &mut Visibility)>,
) {
    for (layer, mut transform, mut visibility) in &mut layers {
        let Ok(effect) = effects.get(layer.root) else {
            continue;
        };
        if let Some(scale) = layer
            .timeline
            .scale_at(effect.timer.elapsed(), layer.duration)
        {
            *visibility = Visibility::Inherited;
            transform.scale = layer.base_scale * scale;
        } else {
            *visibility = Visibility::Hidden;
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
        VfxMaterialKey::ColdBright => materials.elemental_cold_bright.clone(),
        VfxMaterialKey::ColdSoft => materials.elemental_cold.clone(),
        VfxMaterialKey::FireBright => materials.elemental_fire_bright.clone(),
        VfxMaterialKey::FireSoft => materials.elemental_fire.clone(),
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
pub(in crate::client) fn resolve_vfx_requests(
    mut requests: MessageReader<VfxRequest>,
    mut pending_effects: MessageWriter<PendingCombatEffect>,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    registry: Res<VfxRegistry>,
    settings: Option<Res<ClientShellSettings>>,
    fallback: Option<Res<ImportedWorldFallbackPolicy>>,
) {
    let reduced = settings.is_some_and(|value| value.reduced_combat_effects);
    let primitive = fallback
        .as_deref()
        .is_some_and(|policy| matches!(policy, ImportedWorldFallbackPolicy::ForcePrimitive));
    let variant = vfx_presentation_variant(reduced, primitive);
    let ordered = ordered_vfx_requests(requests.read().copied());
    for request in &ordered {
        let mut request = *request;
        request.signature = request.signature.with_variant(variant);
        let Some(recipe) = registry.resolve(&request, reduced) else {
            continue;
        };
        pending_effects.write(resolved_effect(&request, &recipe, &primitives, &materials));
    }
}

fn ordered_vfx_requests(requests: impl IntoIterator<Item = VfxRequest>) -> Vec<VfxRequest> {
    let mut ordered = requests.into_iter().collect::<Vec<_>>();
    ordered.sort_by_key(|request| (request.order, request.key));
    ordered
}

pub(in crate::client) fn materialize_combat_effects(
    mut commands: Commands,
    mut pending_effects: MessageReader<PendingCombatEffect>,
    effects: Query<(Entity, &CombatEffect3d)>,
    mut sequence: Local<CombatEffectSequence>,
) {
    let mut active = effects
        .iter()
        .map(|(entity, effect)| (effect.order, entity, effect.recipe_id.clone()))
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
            .filter(|(_, _, recipe)| recipe == &descriptor.recipe_id)
            .count()
            >= descriptor.concurrency_cap
        {
            let Some(index) = active
                .iter()
                .position(|(_, _, recipe)| recipe == &descriptor.recipe_id)
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
        active.push_back((sequence.0, entity, descriptor.recipe_id.clone()));
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
pub(in crate::client) fn cleanup_combat_effects(
    mut commands: Commands,
    time: Res<Time<Real>>,
    authoritative_ticks: Query<&AuthoritativeTick>,
    mut effects: Query<(Entity, &mut CombatEffect3d)>,
) {
    let current_tick = authoritative_ticks.iter().map(|tick| tick.0).max();
    for (entity, mut effect) in &mut effects {
        if effect.advance_and_is_finished(time.delta(), current_tick) {
            commands.entity(entity).try_despawn();
        }
    }
}

/// Collects bounded presentation-owned VFX state for a screenshot evidence pair.
///
/// The snapshot intentionally reports renderer ownership rather than gameplay outcomes. Durable
/// gameplay facts (projectiles, fields, fighters, and authoritative ticks) are captured by the
/// ordinary evidence path alongside this renderer view.
#[allow(
    clippy::too_many_lines,
    reason = "one evidence snapshot keeps the transient, durable-signature, registry, and cache views atomic"
)]
pub(in crate::client) fn collect_vfx_evidence_snapshot(world: &mut World) -> Value {
    let mut layer_query = world.query::<&CombatEffectLayer3d>();
    let mut layers_by_root = HashMap::<Entity, Vec<(u8, Value)>>::new();
    for layer in layer_query.iter(world) {
        layers_by_root.entry(layer.root).or_default().push((
            layer.layer_index,
            json!({
                "index": layer.layer_index,
                "id": layer.layer_id,
                "readability": layer.readability,
                "variant_sample": layer.variant_sample,
            }),
        ));
    }
    let mut transient_query = world.query::<(
        Entity,
        &CombatEffect3d,
        &CombatVisualSignature,
        Option<&Name>,
    )>();
    let mut transient_effects = transient_query
        .iter(world)
        .map(|(entity, effect, signature, name)| {
            json!({
                "entity": entity.to_bits(),
                "recipe_id": effect.recipe_id,
                "signature": signature,
                "layers": layers_by_root.remove(&entity).map(|mut layers| {
                    layers.sort_unstable_by_key(|(index, _)| *index);
                    layers.into_iter().map(|(_, layer)| layer).collect::<Vec<_>>()
                }).unwrap_or_default(),
                "order": effect.order,
                "label": name.map(Name::as_str),
                "elapsed_seconds": effect.timer.elapsed_secs(),
                "duration_seconds": effect.timer.duration().as_secs_f32(),
                "authoritative_expires_at_tick": effect.expires_at_tick,
            })
        })
        .collect::<Vec<_>>();
    transient_effects.sort_by_key(|effect| effect["order"].as_u64().unwrap_or(u64::MAX));

    let reduced_effects = world
        .get_resource::<ClientShellSettings>()
        .is_some_and(|settings| settings.reduced_combat_effects);
    let primitive_world = world
        .get_resource::<ImportedWorldFallbackPolicy>()
        .is_some_and(|policy| matches!(policy, ImportedWorldFallbackPolicy::ForcePrimitive));
    let mesh_assets = world.get_resource::<Assets<Mesh>>().map_or(0, Assets::len);
    let material_assets = world
        .get_resource::<Assets<StandardMaterial>>()
        .map_or(0, Assets::len);
    let fire_area_material_assets = world
        .get_resource::<Assets<super::elemental_style::FireAreaMaterial>>()
        .map_or(0, Assets::len);
    let image_assets = world.get_resource::<Assets<Image>>().map_or(0, Assets::len);
    let signature_cache = world
        .get_resource::<VfxSignatureCache>()
        .map(VfxSignatureCache::diagnostics);
    let mut signature_query = world.query::<(
        Entity,
        &CombatVisualSignature,
        Option<&CombatVisualOwner>,
        Option<&Name>,
    )>();
    let mut signatures = signature_query
        .iter(world)
        .map(|(entity, signature, owner, name)| {
            json!({
                "entity": entity.to_bits(),
                "owner": owner.map(|owner| owner.0.to_bits()),
                "name": name.map(Name::as_str),
                "signature": signature,
            })
        })
        .collect::<Vec<_>>();
    signatures.sort_by_key(|entry| entry["entity"].as_u64().unwrap_or(u64::MAX));
    let elemental_fields =
        serialized_component_snapshots::<crate::combat::ElementalFieldState>(world);
    let concealment_fields =
        serialized_component_snapshots::<crate::concealment::ConcealmentFieldState>(world);
    let persistent_splashes =
        serialized_component_snapshots::<crate::combat::PersistentSplashState>(world);
    let cone_sprays = serialized_component_snapshots::<crate::combat::ConeSprayState>(world);
    let sticky_blobs = sticky_blob_snapshots(world);

    json!({
        "catalog_fingerprint": vfx::embedded_vfx_catalog_fingerprint(),
        "reduced_effects": reduced_effects,
        "primitive_world": primitive_world,
        "active": {
            "visual_roots": component_count::<CombatVisualOwner>(world),
            "transient_roots": transient_effects.len(),
            "projectile_roots": component_count::<V3ProjectileVisual>(world),
            "sentry_roots": component_count::<super::entities::SentryVisual3d>(world),
            "concealment_field_roots": component_count::<super::entities::ConcealmentFieldVisual3d>(world),
            "elemental_field_roots": component_count::<super::entities::ElementalFieldVisual3d>(world),
            "persistent_splash_roots": component_count::<super::entities::PersistentSplashVisual3d>(world),
            "sticky_blob_roots": component_count::<super::entities::StickyBlobVisual3d>(world),
            "cone_spray_roots": component_count::<super::cone_spray::ConeSprayVisual3d>(world),
            "status_roots": component_count::<super::fighter_feedback::StatusVisual3d>(world),
            "dash_trail_roots": component_count::<super::fighter_feedback::DashTrailVisual3d>(world),
            "elemental_motion_accents": component_count::<super::elemental_style::ElementalMotionAccent3d>(world),
            "animated_fire_areas": component_count::<super::elemental_style::AnimatedFireArea3d>(world),
            "fire_area_underlays": component_count::<super::elemental_style::FireAreaUnderlay3d>(world),
        },
        "assets": {
            "meshes": mesh_assets,
            "standard_materials": material_assets,
            "fire_area_materials": fire_area_material_assets,
            "images": image_assets,
        },
        "signature_cache": signature_cache,
        "signatures": signatures,
        "transient_effects": transient_effects,
        "persistent_facts": {
            "elemental_fields": elemental_fields,
            "concealment_fields": concealment_fields,
            "persistent_splashes": persistent_splashes,
            "cone_sprays": cone_sprays,
            "sticky_blobs": sticky_blobs,
        },
    })
}

fn component_count<T: Component>(world: &mut World) -> usize {
    world
        .query_filtered::<Entity, With<T>>()
        .iter(world)
        .count()
}

fn serialized_component_snapshots<T>(world: &mut World) -> Vec<Value>
where
    T: Component + serde::Serialize,
{
    let mut query = world.query::<(Entity, &T)>();
    let mut snapshots = query
        .iter(world)
        .map(|(entity, state)| {
            json!({
                "entity": entity.to_bits(),
                "state": state,
            })
        })
        .collect::<Vec<_>>();
    snapshots.sort_by_key(|snapshot| snapshot["entity"].as_u64().unwrap_or(u64::MAX));
    snapshots
}

fn sticky_blob_snapshots(world: &mut World) -> Vec<Value> {
    let mut query = world.query::<(
        Entity,
        &crate::combat::StickyBlobState,
        Option<&Position>,
        Option<&crate::combat::ReplicatedAttackSource>,
    )>();
    let mut snapshots = query
        .iter(world)
        .map(|(entity, state, position, source)| {
            json!({
                "entity": entity.to_bits(),
                "position": position.map(|position| [position.x, position.y]),
                "state": state,
                "source": source,
            })
        })
        .collect::<Vec<_>>();
    snapshots.sort_by_key(|snapshot| snapshot["entity"].as_u64().unwrap_or(u64::MAX));
    snapshots
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

    fn recipe(
        registry: &VfxRegistry,
        key: VfxRequestKey,
        reduced: bool,
        radius: Option<f32>,
        deadline: Option<VfxDeadline>,
    ) -> ResolvedVfxRecipe {
        let request = VfxRequest::try_new(
            key,
            VfxRequestOrder::new(registry.producer_rank(key).unwrap(), 1),
            Vec2::ZERO,
            radius,
            deadline,
            "renderer recipe test",
        )
        .unwrap();
        registry.resolve(&request, reduced).unwrap().clone()
    }

    fn pending_effect(label: &'static str, lifetime: Duration) -> PendingCombatEffect {
        PendingCombatEffect {
            lifetime,
            expires_at_tick: None,
            layers: Vec::new(),
            label,
            recipe_id: label.to_string(),
            concurrency_cap: MAX_EFFECTS,
            signature: CombatVisualSignature::neutral_point(0, Vec2::ZERO),
        }
    }

    fn pending_layer(layer: &VfxMeshLayer) -> PendingCombatEffectLayer {
        PendingCombatEffectLayer {
            id: layer.id.clone(),
            mesh: Handle::default(),
            material: Handle::default(),
            transform: effect_layer_transform(layer, Vec2::ZERO, None),
            timeline: layer.timeline,
            duration: match layer.timeline.duration {
                VfxLifetime::Millis(millis) => Duration::from_millis(u64::from(millis)),
                VfxLifetime::AuthoritativeDeadline => {
                    panic!("test layer requires a fixed duration")
                }
            },
            casts_shadows: layer.casts_shadows,
            receives_shadows: layer.receives_shadows,
            readability: layer.readability,
            variant_sample: 7,
        }
    }

    #[test]
    fn evidence_snapshot_names_catalog_variant_and_active_lifecycle_owners() {
        let mut world = World::new();
        world.insert_resource(ClientShellSettings {
            reduced_combat_effects: true,
            ..Default::default()
        });
        world.insert_resource(ImportedWorldFallbackPolicy::ForcePrimitive);
        world.spawn((
            CombatEffect3d {
                timer: Timer::new(Duration::from_millis(180), TimerMode::Once),
                expires_at_tick: None,
                order: 7,
                recipe_id: "impact".to_string(),
            },
            CombatVisualSignature::neutral_point(7, Vec2::ZERO)
                .with_variant(vfx::VfxPresentationVariant::ReducedPrimitive),
            Name::new("Impact evidence fixture"),
        ));
        let owner = world.spawn_empty().id();
        world.spawn((
            CombatVisualOwner(owner),
            super::super::entities::ElementalFieldVisual3d,
            CombatVisualSignature::neutral_point(11, Vec2::new(2.0, 3.0)),
        ));
        world.spawn(super::super::elemental_style::AnimatedFireArea3d);
        world.spawn(super::super::elemental_style::FireAreaUnderlay3d);
        world.spawn(crate::combat::ElementalFieldState {
            id: crate::combat::ElementalFieldId(11),
            kind: crate::combat::ElementalFieldKind::Fire,
            owner_network_entity_id: NetworkEntityId(12),
            team_id: crate::combat::TeamId(1),
            center: crate::combat::WorldPoint { x: 2.0, y: 3.0 },
            radius_milliunits: 64_000,
            activated_at_tick: 20,
            next_pulse_tick: 30,
            expires_at_tick: 80,
        });

        let snapshot = collect_vfx_evidence_snapshot(&mut world);

        assert_eq!(snapshot["reduced_effects"], true);
        assert_eq!(snapshot["primitive_world"], true);
        assert_eq!(snapshot["active"]["transient_roots"], 1);
        assert_eq!(snapshot["active"]["elemental_field_roots"], 1);
        assert_eq!(snapshot["active"]["animated_fire_areas"], 1);
        assert_eq!(snapshot["active"]["fire_area_underlays"], 1);
        assert_eq!(snapshot["transient_effects"][0]["recipe_id"], "impact");
        assert_eq!(snapshot["transient_effects"][0]["order"], 7);
        assert_eq!(
            snapshot["persistent_facts"]["elemental_fields"][0]["state"]["kind"],
            "Fire"
        );
        assert_ne!(snapshot["catalog_fingerprint"], 0);
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
                        recipe_id: "existing".to_string(),
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
            .init_resource::<Assets<super::super::elemental_style::FireAreaMaterial>>()
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
        let mut effects = world.query::<(&CombatEffect3d, &Name)>();
        let (effect, name) = effects.single(world).unwrap();
        assert_eq!(effect.recipe_id, "impact");
        assert_eq!(name.as_str(), "synthetic renderer extension");
        let mut layers = world.query::<(&CombatEffectLayer3d, &Mesh3d, &Transform)>();
        let (layer, mesh, transform) = layers.single(world).unwrap();
        assert_eq!(layer.layer_id, "impact");
        assert_eq!(mesh.0, sphere);
        assert_eq!(transform.translation, Vec3::new(10.0, 6.3, -20.0));
        assert_eq!(transform.scale, Vec3::splat(14.0));
    }

    #[test]
    fn synthetic_layered_recipe_materializes_in_authored_order_and_animates_scale() {
        let registry = registry();
        let mut first = recipe(&registry, COMBAT_IMPACT_VFX, false, None, None).layers[0].clone();
        first.id = "first".to_string();
        let mut second = first.clone();
        second.id = "second".to_string();
        second.timeline.delay_millis = 20;
        second.timeline.scale.start = 0.5;
        second.timeline.scale.peak = 1.5;
        second.timeline.scale.end = 1.0;

        let mut app = App::new();
        app.add_message::<PendingCombatEffect>().add_systems(
            Update,
            (materialize_combat_effects, animate_combat_effect_layers).chain(),
        );
        let mut descriptor = pending_effect("layered", Duration::from_millis(120));
        descriptor.layers = vec![pending_layer(&first), pending_layer(&second)];
        app.world_mut().write_message(descriptor);
        app.update();

        {
            let world = app.world_mut();
            let mut layers = world.query::<(&CombatEffectLayer3d, &Visibility)>();
            let mut ordered = layers
                .iter(world)
                .map(|(layer, visibility)| {
                    (layer.layer_index, layer.layer_id.as_str(), *visibility)
                })
                .collect::<Vec<_>>();
            ordered.sort_unstable_by_key(|(index, ..)| *index);
            assert_eq!(ordered[0].1, "first");
            assert_eq!(ordered[1].1, "second");
            assert_eq!(ordered[1].2, Visibility::Hidden);
        }

        {
            let world = app.world_mut();
            let mut effects = world.query::<&mut CombatEffect3d>();
            effects
                .single_mut(world)
                .unwrap()
                .timer
                .tick(Duration::from_millis(70));
        }
        app.update();

        let world = app.world_mut();
        let mut layers = world.query::<(&CombatEffectLayer3d, &Transform, &Visibility)>();
        let (_, transform, visibility) = layers
            .iter(world)
            .find(|(layer, ..)| layer.layer_id == "second")
            .unwrap();
        assert_eq!(*visibility, Visibility::Inherited);
        assert!(transform.scale.x > second.scale.resolve(None).unwrap());
    }

    #[test]
    fn effect_allocation_enforces_the_catalog_recipe_cap() {
        let mut app = effect_allocation_app();
        for label in ["first", "second", "third"] {
            let mut effect = pending_effect(label, Duration::from_millis(100));
            effect.recipe_id = "capped-family".to_string();
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
        let normal = recipe(&registry, COMBAT_RESET_VFX, false, None, None);
        let reduced = recipe(&registry, COMBAT_RESET_VFX, true, None, None);
        let VfxLifetime::Millis(normal) = normal.layers[0].timeline.duration else {
            panic!("reset VFX uses a fixed lifetime")
        };
        let VfxLifetime::Millis(reduced) = reduced.layers[0].timeline.duration else {
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
        let normal_recipe = recipe(&registry, COMBAT_MUZZLE_VFX, false, None, None);
        let reduced_recipe = recipe(&registry, COMBAT_MUZZLE_VFX, true, None, None);
        let normal = effect_layer_transform(&normal_recipe.layers[0], Vec2::new(10.0, 20.0), None);
        let reduced =
            effect_layer_transform(&reduced_recipe.layers[0], Vec2::new(10.0, 20.0), None);

        assert_eq!(reduced.translation, normal.translation);
        assert_eq!(normal.scale, Vec3::splat(8.0));
        assert_eq!(reduced.scale, Vec3::splat(5.2));
    }

    #[test]
    fn renderer_family_owns_ground_ring_orientation_and_placement() {
        let registry = registry();
        let recipe = recipe(
            &registry,
            REVEAL_SCAN_VFX,
            false,
            Some(5.2),
            Some(VfxDeadline::new(1, 2, Some(1))),
        );
        let ring = effect_layer_transform(&recipe.layers[0], Vec2::new(10.0, 20.0), Some(5.2));

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
        let fixed_recipe = recipe(&registry, WORLD_OBJECT_DAMAGED_VFX, true, None, None);
        let fixed = effect_layer_transform(&fixed_recipe.layers[0], Vec2::ZERO, None);
        assert!((fixed.translation.y - 8.0).abs() < f32::EPSILON);
        assert_eq!(fixed.scale, Vec3::splat(7.0));

        let radius = 20.0;
        let area_recipe = recipe(&registry, ELEMENTAL_FIELD_VFX, true, Some(radius), None);
        let area = effect_layer_transform(&area_recipe.layers[0], Vec2::ZERO, Some(radius));
        assert!((area.translation.y - radius * 0.45).abs() < f32::EPSILON);
        assert_eq!(area.scale, Vec3::splat(radius * 0.65));
    }

    #[test]
    fn authoritative_effect_expiry_cleans_up_materialized_effect() {
        let registry = registry();
        let impact = recipe(&registry, COMBAT_IMPACT_VFX, false, None, None);
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
        descriptor.layers = vec![pending_layer(&impact.layers[0])];
        app.world_mut().write_message(descriptor);

        app.update();
        let world = app.world_mut();
        let mut effects = world.query_filtered::<Entity, With<CombatEffect3d>>();
        assert_eq!(effects.iter(world).count(), 0);
        let mut layers = world.query_filtered::<Entity, With<CombatEffectLayer3d>>();
        assert_eq!(layers.iter(world).count(), 0);
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
