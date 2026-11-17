use super::super::render_quality::{ActiveRenderQuality, ParticleBackend};
#[allow(
    clippy::wildcard_imports,
    reason = "the world-renderer submodule consumes the presentation_3d composition surface"
)]
use super::super::*;
use super::{
    MAX_ACTIVE_PARTICLE_EMITTERS, PendingVfxParticleLayer, RetainedProjectileSmoke3d,
    VfxParticleAssets, VfxParticleDiagnostics, VfxParticleEmitter3d,
};
use crate::vfx::{
    CombatVisualSignature, ResolvedVfxRecipe, VfxDeadline, VfxGeometry, VfxLifetime,
    VfxMaterialKey, VfxMeshLayer, VfxPresentationVariant, VfxReadabilityClass, VfxRegistry,
    VfxRendererFamily, VfxRequest, VfxTimeline, vfx_counter_sample, vfx_presentation_variant,
};
use brawler::combat::client::DeduplicatedCombatCue;
use std::collections::{HashMap, VecDeque};

mod evidence;
pub(crate) use evidence::collect_vfx_evidence_snapshot;

const MAX_EFFECTS: usize = 96;

#[derive(Component)]
pub(crate) struct CombatEffect3d {
    timer: Timer,
    expires_at_tick: Option<u64>,
    order: u64,
    recipe_id: String,
    mesh_layers: u8,
    particle_emitters: u8,
}

#[derive(Resource, Default, Clone, Copy)]
pub(crate) struct VfxTransientDiagnostics {
    root_high_water: usize,
    mesh_layer_high_water: usize,
    evicted_roots: u64,
}

impl CombatEffect3d {
    pub(crate) fn advance_and_is_finished(
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
pub(crate) struct CombatEffectSequence(u64);

#[derive(Message)]
pub(crate) struct PendingCombatEffect {
    lifetime: Duration,
    expires_at_tick: Option<u64>,
    layers: Vec<PendingCombatEffectLayer>,
    particle_layers: Vec<PendingVfxParticleLayer>,
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
pub(crate) struct CombatEffectLayer3d {
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
    fn spawn(
        &self,
        commands: &mut Commands,
        order: u64,
        particle_budget: usize,
    ) -> (Entity, usize) {
        let particle_emitters = self.particle_layers.len().min(particle_budget);
        let root = commands
            .spawn((
                CombatEffect3d {
                    timer: Timer::new(self.lifetime, TimerMode::Once),
                    expires_at_tick: self.expires_at_tick,
                    order,
                    recipe_id: self.recipe_id.clone(),
                    mesh_layers: u8::try_from(self.layers.len())
                        .expect("validated recipe mesh-layer count fits u8"),
                    particle_emitters: u8::try_from(particle_emitters)
                        .expect("bounded particle emitter count fits u8"),
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
            for layer in self.particle_layers.iter().take(particle_emitters) {
                layer.spawn(parent, root);
            }
        });
        (root, particle_emitters)
    }
}

fn resolved_effect(
    request: &VfxRequest,
    recipe: &ResolvedVfxRecipe,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    particles: Option<(&VfxParticleAssets, ParticleBackend)>,
    mut meshes: Option<&mut Assets<Mesh>>,
    melee_meshes: &mut super::melee_style::MeleeArcMeshCache,
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
                mesh: effect_layer_mesh(
                    layer.renderer,
                    request.signature,
                    primitives,
                    meshes.as_deref_mut(),
                    melee_meshes,
                ),
                material: vfx_material(layer.material, materials),
                transform: effect_layer_transform(
                    layer,
                    request.position,
                    request.authoritative_radius,
                    request.signature.geometry,
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
    let mut particle_layers = Vec::new();
    if particle_layers_enabled(request.signature.variant)
        && let Some((assets, particle_backend)) = particles
    {
        for (occurrence, layer) in recipe.particle_layers.iter().enumerate() {
            if let Some(pending) = assets.pending(
                layer,
                primitives,
                request.position,
                &recipe.id,
                request.order.event_id,
                u8::try_from(occurrence).expect("validated particle layer count fits u8"),
                particle_backend,
            ) {
                lifetime = lifetime.max(layer.lifetime());
                particle_layers.push(pending);
            }
        }
    }
    PendingCombatEffect {
        lifetime,
        expires_at_tick,
        layers,
        particle_layers,
        label: request.label,
        recipe_id: recipe.id.clone(),
        concurrency_cap: recipe.concurrency_cap,
        signature: request.signature,
    }
}

fn effect_layer_mesh(
    renderer: VfxRendererFamily,
    signature: CombatVisualSignature,
    primitives: &Primitive3dAssets,
    meshes: Option<&mut Assets<Mesh>>,
    melee_meshes: &mut super::melee_style::MeleeArcMeshCache,
) -> Handle<Mesh> {
    match renderer {
        VfxRendererFamily::Sphere => primitives.effect_sphere.clone(),
        VfxRendererFamily::GroundRing => primitives.area_ring.clone(),
        VfxRendererFamily::GroundThinRing => primitives.area_thin_ring.clone(),
        VfxRendererFamily::GroundDisc => primitives.area_disc.clone(),
        VfxRendererFamily::GroundSpark => primitives.vitality_spark.clone(),
        VfxRendererFamily::MeleeArc => {
            let VfxGeometry::Cone { angle_degrees, .. } = signature.geometry else {
                return primitives.area_ring.clone();
            };
            meshes.map_or_else(
                || primitives.area_ring.clone(),
                |meshes| melee_meshes.resolve(angle_degrees, meshes),
            )
        }
    }
}

const fn particle_layers_enabled(variant: VfxPresentationVariant) -> bool {
    matches!(variant, VfxPresentationVariant::Normal)
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
    geometry: VfxGeometry,
) -> Transform {
    let geometry_reach = match geometry {
        VfxGeometry::Cone { reach, .. } => Some(reach),
        _ => None,
    };
    let scale = layer
        .scale
        .resolve(authoritative_radius, geometry_reach)
        .expect("validated VFX layer supplies required authoritative radius");
    let height = layer
        .anchor
        .resolve_height(authoritative_radius)
        .expect("validated VFX layer supplies required authoritative radius");
    match layer.renderer {
        VfxRendererFamily::GroundRing
        | VfxRendererFamily::GroundThinRing
        | VfxRendererFamily::GroundDisc
        | VfxRendererFamily::GroundSpark => Transform {
            translation: ground_position(position) + Vec3::Y * height,
            rotation: Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2),
            scale: Vec3::splat(scale),
        },
        VfxRendererFamily::Sphere => {
            Transform::from_translation(ground_position(position) + Vec3::Y * height)
                .with_scale(Vec3::splat(scale))
        }
        VfxRendererFamily::MeleeArc => {
            let VfxGeometry::Cone { origin, facing, .. } = geometry else {
                unreachable!("validated melee arc layers require cone geometry")
            };
            Transform {
                translation: ground_position(Vec2::from_array(origin)) + Vec3::Y * height,
                rotation: ground_rotation(Rotation::radians(facing))
                    * Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2),
                scale: Vec3::splat(scale),
            }
        }
    }
}

pub(crate) fn animate_combat_effect_layers(
    effects: Query<&CombatEffect3d>,
    mut layers: Query<(&CombatEffectLayer3d, &mut Transform, &mut Visibility)>,
) {
    for (layer, mut transform, mut visibility) in &mut layers {
        let Ok(effect) = effects.get(layer.root) else {
            continue;
        };
        let layer_end = Duration::from_millis(u64::from(layer.timeline.delay_millis))
            .saturating_add(layer.duration);
        if effect.timer.elapsed() >= layer_end {
            *visibility = Visibility::Hidden;
            continue;
        }
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
        VfxMaterialKey::NeutralBright => materials.neutral_energy_bright.clone(),
        VfxMaterialKey::NeutralSoft => materials.neutral_energy_soft.clone(),
        VfxMaterialKey::ScanArea => materials.scan_area.clone(),
        VfxMaterialKey::ConcealmentBright => materials.concealment_field_blue_boundary.clone(),
        VfxMaterialKey::ConcealmentSoft => materials.concealment_field_blue_fill.clone(),
        VfxMaterialKey::DemolitionArea => materials.demolition_area.clone(),
        VfxMaterialKey::PickupGlow => materials.pickup_glow.clone(),
        VfxMaterialKey::ColdBright => materials.elemental_cold_bright.clone(),
        VfxMaterialKey::ColdSoft => materials.elemental_cold.clone(),
        VfxMaterialKey::FireBright => materials.elemental_fire_bright.clone(),
        VfxMaterialKey::FireSoft => materials.elemental_fire.clone(),
        VfxMaterialKey::PoisonBright => materials.elemental_poison_bright.clone(),
        VfxMaterialKey::PoisonSoft => materials.elemental_poison.clone(),
        VfxMaterialKey::RestorationBright => materials.elemental_restoration_bright.clone(),
        VfxMaterialKey::RestorationSoft => materials.elemental_restoration.clone(),
    }
}
/// Seconds the shoot motion plays after an attack starts.
const SHOOT_ANIMATION_SECONDS: f32 = 0.18;

/// Keeps fighter animation feedback separate from semantic VFX production.
///
/// The controlled fighter's attacks start from the local input-frame presentation; the later
/// authoritative cue for the same attack is an echo and does not restart the motion. Every other
/// accepted attack starts from its cue as before.
#[allow(
    clippy::needless_pass_by_value,
    reason = "the ledger is a read-only Bevy system resource"
)]
pub(in super::super) fn animate_attack_acceptance(
    mut cues: MessageReader<DeduplicatedCombatCue>,
    mut local_attacks: MessageReader<local_attack::LocalAttackPresented>,
    ledger: Option<Res<local_attack::LocalAttackLedger>>,
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
    let mut start = |visual_entity: Entity| {
        if let Ok((_, _, mut visual)) = visuals.get_mut(visual_entity) {
            visual.shoot_seconds = SHOOT_ANIMATION_SECONDS;
            visual.recoil_seconds = FIGHTER_RECOIL_SECONDS;
        }
    };
    for local in local_attacks.read() {
        if let local_attack::LocalAttackKind::Ultimate { kind, .. } = local.kind
            && !local_attack::presents_cast_motion_locally(kind)
        {
            continue;
        }
        if let Some(&visual_entity) = visuals_by_owner.get(&local.fighter) {
            start(visual_entity);
        }
    }
    for DeduplicatedCombatCue(cue) in cues.read() {
        if let brawler::combat::CombatCue::AttackAccepted {
            source, attack_id, ..
        } = cue
            && !ledger
                .as_deref()
                .is_some_and(|ledger| ledger.is_echo(*attack_id))
            && let Some(&visual_entity) = visuals_by_network_id.get(&source.0)
        {
            start(visual_entity);
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    reason = "the renderer adapter resolves one bounded request batch against retained render assets"
)]
pub(crate) fn resolve_vfx_requests(
    mut requests: MessageReader<VfxRequest>,
    mut pending_effects: MessageWriter<PendingCombatEffect>,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    registry: Res<VfxRegistry>,
    settings: Option<Res<ClientShellSettings>>,
    fallback: Option<Res<ImportedWorldFallbackPolicy>>,
    particle_assets: Option<Res<VfxParticleAssets>>,
    quality: Option<Res<ActiveRenderQuality>>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut melee_meshes: Option<ResMut<super::melee_style::MeleeArcMeshCache>>,
) {
    let particle_backend = quality
        .as_deref()
        .map_or(ParticleBackend::Gpu, ActiveRenderQuality::particle_backend);
    let reduced = settings.is_some_and(|value| value.reduced_combat_effects);
    let primitive = fallback
        .as_deref()
        .is_some_and(|policy| matches!(policy, ImportedWorldFallbackPolicy::ForcePrimitive));
    let variant = vfx_presentation_variant(reduced, primitive);
    let mut fallback_melee_meshes = super::melee_style::MeleeArcMeshCache::default();
    let melee_meshes = melee_meshes
        .as_deref_mut()
        .unwrap_or(&mut fallback_melee_meshes);
    let ordered = ordered_vfx_requests(requests.read().copied());
    for request in &ordered {
        let mut request = *request;
        request.signature = request.signature.with_variant(variant);
        let Some(recipe) = registry.resolve(&request, reduced) else {
            continue;
        };
        pending_effects.write(resolved_effect(
            &request,
            &recipe,
            &primitives,
            &materials,
            particle_assets
                .as_deref()
                .map(|assets| (assets, particle_backend)),
            meshes.as_deref_mut(),
            melee_meshes,
        ));
    }
}

fn ordered_vfx_requests(requests: impl IntoIterator<Item = VfxRequest>) -> Vec<VfxRequest> {
    let mut ordered = requests.into_iter().collect::<Vec<_>>();
    ordered.sort_by_key(|request| (request.order, request.key));
    ordered
}

#[allow(
    clippy::too_many_lines,
    reason = "the allocation system keeps global/per-recipe eviction, spawn ordering, and high-water accounting auditable in one schedule phase"
)]
pub(crate) fn materialize_combat_effects(
    mut commands: Commands,
    mut pending_effects: MessageReader<PendingCombatEffect>,
    effects: Query<(Entity, &CombatEffect3d)>,
    retained_particle_emitters: Query<(), With<RetainedProjectileSmoke3d>>,
    mut sequence: Local<CombatEffectSequence>,
    mut transient_diagnostics: Option<ResMut<VfxTransientDiagnostics>>,
    mut particle_diagnostics: Option<ResMut<VfxParticleDiagnostics>>,
) {
    let retained_particle_emitters = retained_particle_emitters.count();
    let mut evicted_roots = 0_u64;
    let mut active = effects
        .iter()
        .map(|(entity, effect)| {
            (
                effect.order,
                entity,
                effect.recipe_id.clone(),
                usize::from(effect.mesh_layers),
                usize::from(effect.particle_emitters),
            )
        })
        .collect::<VecDeque<_>>();
    active
        .make_contiguous()
        .sort_unstable_by_key(|(order, entity, ..)| (*order, entity.to_bits()));
    if let Some((maximum_order, ..)) = active.back() {
        sequence.0 = sequence.0.max(*maximum_order);
    }
    let mut active_mesh_layers = active
        .iter()
        .map(|(_, _, _, mesh_layers, _)| *mesh_layers)
        .sum::<usize>();
    let mut active_particle_emitters = active
        .iter()
        .map(|(_, _, _, _, emitters)| *emitters)
        .sum::<usize>();
    while active.len() > MAX_EFFECTS {
        if let Some((_, oldest, _, mesh_layers, emitters)) = active.pop_front() {
            active_mesh_layers = active_mesh_layers.saturating_sub(mesh_layers);
            active_particle_emitters = active_particle_emitters.saturating_sub(emitters);
            evicted_roots = evicted_roots.saturating_add(1);
            commands.entity(oldest).try_despawn();
        }
    }

    for descriptor in pending_effects.read() {
        while active
            .iter()
            .filter(|(_, _, recipe, _, _)| recipe == &descriptor.recipe_id)
            .count()
            >= descriptor.concurrency_cap
        {
            let Some(index) = active
                .iter()
                .position(|(_, _, recipe, _, _)| recipe == &descriptor.recipe_id)
            else {
                break;
            };
            if let Some((_, oldest, _, mesh_layers, emitters)) = active.remove(index) {
                active_mesh_layers = active_mesh_layers.saturating_sub(mesh_layers);
                active_particle_emitters = active_particle_emitters.saturating_sub(emitters);
                evicted_roots = evicted_roots.saturating_add(1);
                commands.entity(oldest).try_despawn();
            }
        }
        while active.len() >= MAX_EFFECTS {
            if let Some((_, oldest, _, mesh_layers, emitters)) = active.pop_front() {
                active_mesh_layers = active_mesh_layers.saturating_sub(mesh_layers);
                active_particle_emitters = active_particle_emitters.saturating_sub(emitters);
                evicted_roots = evicted_roots.saturating_add(1);
                commands.entity(oldest).try_despawn();
            }
        }
        sequence.0 = sequence.0.saturating_add(1);
        let particle_budget = MAX_ACTIVE_PARTICLE_EMITTERS
            .saturating_sub(active_particle_emitters.saturating_add(retained_particle_emitters));
        let (entity, particle_emitters) =
            descriptor.spawn(&mut commands, sequence.0, particle_budget);
        if let Some(diagnostics) = particle_diagnostics.as_deref_mut() {
            diagnostics.suppressed = diagnostics.suppressed.saturating_add(
                u64::try_from(
                    descriptor
                        .particle_layers
                        .len()
                        .saturating_sub(particle_emitters),
                )
                .expect("bounded particle emitter count fits u64"),
            );
        }
        let mesh_layers = descriptor.layers.len();
        active_mesh_layers = active_mesh_layers.saturating_add(mesh_layers);
        active_particle_emitters = active_particle_emitters.saturating_add(particle_emitters);
        active.push_back((
            sequence.0,
            entity,
            descriptor.recipe_id.clone(),
            mesh_layers,
            particle_emitters,
        ));
    }
    record_transient_diagnostics(
        transient_diagnostics.as_deref_mut(),
        active.len(),
        active_mesh_layers,
        evicted_roots,
    );
    record_particle_diagnostics(
        particle_diagnostics.as_deref_mut(),
        active_particle_emitters.saturating_add(retained_particle_emitters),
    );
}

fn record_transient_diagnostics(
    diagnostics: Option<&mut VfxTransientDiagnostics>,
    active_roots: usize,
    active_mesh_layers: usize,
    evicted_roots: u64,
) {
    let Some(diagnostics) = diagnostics else {
        return;
    };
    diagnostics.root_high_water = diagnostics.root_high_water.max(active_roots);
    diagnostics.mesh_layer_high_water = diagnostics.mesh_layer_high_water.max(active_mesh_layers);
    diagnostics.evicted_roots = diagnostics.evicted_roots.saturating_add(evicted_roots);
}

fn record_particle_diagnostics(
    diagnostics: Option<&mut VfxParticleDiagnostics>,
    active_emitters: usize,
) {
    let Some(diagnostics) = diagnostics else {
        return;
    };
    diagnostics.active = active_emitters;
    diagnostics.high_water = diagnostics.high_water.max(active_emitters);
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
    let whole_seconds = remaining_ticks / brawler::timing::SIMULATION_TICK_HZ;
    let subsecond_ticks = u32::try_from(remaining_ticks % brawler::timing::SIMULATION_TICK_HZ)
        .expect("subsecond tick remainder fits u32");
    Duration::from_secs(whole_seconds)
        .saturating_add(brawler::timing::SIMULATION_TICK.saturating_mul(subsecond_ticks))
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Time is a Bevy system resource parameter"
)]
pub(crate) fn cleanup_combat_effects(
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

#[cfg(test)]
mod tests {
    use super::evidence::component_count;
    use super::*;
    use crate::vfx::{
        COMBAT_IMPACT_VFX, COMBAT_MUZZLE_VFX, COMBAT_RESET_VFX, COMBAT_VFX_PRODUCER_RANK,
        CombatVfxProducerPlugin, ELEMENTAL_FIELD_VFX, HEIST_DAMAGED_VFX, HEIST_VFX_PRODUCER_RANK,
        HeistVfxProducerPlugin, PICKUP_COLLECTED_VFX, PICKUP_VFX_PRODUCER_RANK,
        PickupVfxProducerPlugin, REVEAL_SCAN_VFX, VfxDeadline, VfxRegistryPlugin,
        VfxRequestCapabilities, VfxRequestKey, VfxRequestOrder, VfxRequestRegistration,
        WORLD_OBJECT_DAMAGED_VFX, WORLD_OBJECT_VFX_PRODUCER_RANK, WorldObjectVfxProducerPlugin,
    };

    pub(super) fn registry() -> VfxRegistry {
        let mut app = App::new();
        app.add_plugins(VfxRegistryPlugin).add_plugins((
            CombatVfxProducerPlugin,
            WorldObjectVfxProducerPlugin,
            PickupVfxProducerPlugin,
            HeistVfxProducerPlugin,
        ));
        brawler::test_app::finalize(&mut app);
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
            particle_layers: Vec::new(),
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
            transform: effect_layer_transform(
                layer,
                Vec2::ZERO,
                None,
                VfxGeometry::point(Vec2::ZERO),
            ),
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
    fn particles_are_normal_only_and_never_define_degraded_variants() {
        assert!(particle_layers_enabled(VfxPresentationVariant::Normal));
        assert!(!particle_layers_enabled(VfxPresentationVariant::Reduced));
        assert!(!particle_layers_enabled(VfxPresentationVariant::Primitive));
        assert!(!particle_layers_enabled(
            VfxPresentationVariant::ReducedPrimitive
        ));
    }

    fn effect_allocation_app() -> App {
        let mut app = App::new();
        app.add_message::<PendingCombatEffect>()
            .init_resource::<VfxTransientDiagnostics>()
            .init_resource::<VfxParticleDiagnostics>()
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
                        mesh_layers: 0,
                        particle_emitters: 0,
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
        let diagnostics = world.resource::<VfxTransientDiagnostics>();
        assert_eq!(diagnostics.root_high_water, MAX_EFFECTS);
        assert_eq!(diagnostics.evicted_roots, 4);
    }

    #[test]
    fn fallback_particles_spawn_a_bounded_burst_under_a_budgeted_emitter() {
        let mut app = effect_allocation_app();
        let mut pending = pending_effect("fallback family", Duration::from_secs(1));
        pending
            .particle_layers
            .push(PendingVfxParticleLayer::test_cpu_layer(
                "fallback_sparks",
                40,
            ));
        app.world_mut().write_message(pending);

        app.update();

        let world = app.world_mut();
        assert_eq!(component_count::<VfxParticleEmitter3d>(world), 1);
        assert_eq!(
            component_count::<super::super::cpu_particles::CpuParticle3d>(world),
            usize::from(super::super::cpu_particles::MAX_CPU_PARTICLES_PER_BURST)
        );
        assert_eq!(world.resource::<VfxParticleDiagnostics>().active, 1);
    }

    #[test]
    fn particle_emitters_settle_at_the_global_capacity() {
        let mut app = effect_allocation_app();
        for index in 0..(MAX_ACTIVE_PARTICLE_EMITTERS + 12) {
            let mut pending = pending_effect("particle family", Duration::from_secs(1));
            pending
                .particle_layers
                .push(PendingVfxParticleLayer::test_layer(&format!(
                    "particle_{index}"
                )));
            app.world_mut().write_message(pending);
        }

        app.update();

        let world = app.world_mut();
        assert_eq!(
            component_count::<VfxParticleEmitter3d>(world),
            MAX_ACTIVE_PARTICLE_EMITTERS
        );
        let diagnostics = world.resource::<VfxParticleDiagnostics>();
        assert_eq!(diagnostics.active, MAX_ACTIVE_PARTICLE_EMITTERS);
        assert_eq!(diagnostics.high_water, MAX_ACTIVE_PARTICLE_EMITTERS);
        assert_eq!(diagnostics.suppressed, 0);
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
            .init_resource::<Assets<super::super::elemental_style::ElementalSupportAreaMaterial>>()
            .init_resource::<Assets<super::super::cone_spray::ConeSprayMaterial>>()
            .init_resource::<
                Assets<super::super::straight_projectile_style::StraightProjectileTrailMaterial>,
            >()
            .init_resource::<brawler::builds::BuildCatalogResource>()
            .init_resource::<Assets<ToonMaterial>>()
            .insert_resource(ToonShading::embedded().unwrap())
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
        assert!(transform.scale.x > second.scale.resolve(None, None).unwrap());
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
        let normal = effect_layer_transform(
            &normal_recipe.layers[0],
            Vec2::new(10.0, 20.0),
            None,
            VfxGeometry::point(Vec2::new(10.0, 20.0)),
        );
        let reduced = effect_layer_transform(
            &reduced_recipe.layers[0],
            Vec2::new(10.0, 20.0),
            None,
            VfxGeometry::point(Vec2::new(10.0, 20.0)),
        );

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
        let ring = effect_layer_transform(
            &recipe.layers[0],
            Vec2::new(10.0, 20.0),
            Some(5.2),
            VfxGeometry::point(Vec2::new(10.0, 20.0)),
        );

        assert_eq!(
            ring.translation,
            ground_position(Vec2::new(10.0, 20.0)) + Vec3::Y * 2.7
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
        let fixed = effect_layer_transform(
            &fixed_recipe.layers[0],
            Vec2::ZERO,
            None,
            VfxGeometry::point(Vec2::ZERO),
        );
        assert!((fixed.translation.y - 8.0).abs() < f32::EPSILON);
        assert_eq!(fixed.scale, Vec3::splat(7.0));

        let radius = 20.0;
        let area_recipe = recipe(&registry, ELEMENTAL_FIELD_VFX, true, Some(radius), None);
        let area = effect_layer_transform(
            &area_recipe.layers[0],
            Vec2::ZERO,
            Some(radius),
            VfxGeometry::point(Vec2::ZERO),
        );
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
            .init_resource::<VfxTransientDiagnostics>()
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
        let diagnostics = world.resource::<VfxTransientDiagnostics>();
        assert_eq!(diagnostics.root_high_water, 1);
        assert_eq!(diagnostics.mesh_layer_high_water, 1);
        assert_eq!(diagnostics.evicted_roots, 0);
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

    #[test]
    fn local_attack_starts_the_shoot_motion_once_and_its_echo_does_not_restart_it() {
        use crate::local_attack::{LocalAttackKind, LocalAttackLedger, LocalAttackPresented};
        use brawler::combat::{AttackId, CombatCue, CombatEventId, WeaponDefinitionId, WorldPoint};
        let mut app = App::new();
        app.add_message::<DeduplicatedCombatCue>()
            .add_message::<LocalAttackPresented>()
            .add_systems(Update, animate_attack_acceptance);
        let mut ledger = LocalAttackLedger::default();
        let shot = ledger.record(lightyear::prelude::Tick(1), 100);
        ledger.observe_own_attack(AttackId(11), 100);
        app.insert_resource(ledger);
        let fighter = app.world_mut().spawn((Fighter, NetworkEntityId(12))).id();
        let visual = app
            .world_mut()
            .spawn((
                CombatVisualOwner(fighter),
                V3FighterVisual {
                    last_position: Vec2::ZERO,
                    moving: false,
                    shoot_seconds: 0.0,
                    recoil_seconds: 0.0,
                },
            ))
            .id();
        let accepted = |attack_id| {
            DeduplicatedCombatCue(CombatCue::AttackAccepted {
                event_id: CombatEventId(attack_id),
                tick: 100,
                attack_id: AttackId(attack_id),
                source: NetworkEntityId(12),
                position: WorldPoint { x: 0.0, y: 0.0 },
                weapon_definition_id: WeaponDefinitionId(1),
            })
        };
        let started = |app: &App| {
            let visual = app.world().get::<V3FighterVisual>(visual).expect("visual");
            (visual.shoot_seconds > 0.0, visual.recoil_seconds > 0.0)
        };
        let clear = |app: &mut App| {
            let mut visual = app
                .world_mut()
                .get_mut::<V3FighterVisual>(visual)
                .expect("visual");
            visual.shoot_seconds = 0.0;
            visual.recoil_seconds = 0.0;
        };

        app.world_mut().write_message(LocalAttackPresented {
            fighter,
            owner: NetworkEntityId(12),
            team: brawler::combat::TeamId(1),
            kind: LocalAttackKind::PrimaryFire(shot),
            origin: Vec2::ZERO,
            facing: 0.0,
            simulation_tick: 100,
        });
        app.update();
        assert_eq!(
            started(&app),
            (true, true),
            "shoot motion and recoil on the input frame"
        );

        clear(&mut app);
        app.world_mut().write_message(accepted(11));
        app.update();
        assert_eq!(
            started(&app),
            (false, false),
            "the echo never replays the motion"
        );

        app.world_mut().write_message(accepted(21));
        app.update();
        assert_eq!(
            started(&app),
            (true, true),
            "an unpaired accepted attack still animates"
        );

        // The kick is immediate and eases back to rest within its authored duration.
        assert!(
            (fighter_recoil_offset(FIGHTER_RECOIL_SECONDS) - FIGHTER_RECOIL_DISTANCE).abs() < 1e-6
        );
        assert!(fighter_recoil_offset(FIGHTER_RECOIL_SECONDS * 0.5) < FIGHTER_RECOIL_DISTANCE);
        assert!(fighter_recoil_offset(0.0).abs() < 1e-6);
    }
}
