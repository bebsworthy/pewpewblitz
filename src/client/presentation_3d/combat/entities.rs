use super::super::*;
use super::common::{
    CombatVfxContext, GROUND_EFFECT_HEIGHT, GroundMarkerRelation, ground_marker_material,
    ground_marker_relation, unique_roots,
};
use super::elemental_style::{spawn_elemental_area_style, spawn_elemental_projectile_style};
use super::{
    MAX_ACTIVE_PARTICLE_EMITTERS, VfxParticleAssets, VfxParticleDiagnostics, VfxParticleEmitter3d,
};
use std::collections::{BTreeMap, btree_map::Entry};

const GROUND_MARKER_HEIGHT: f32 = 1.0;
const STRAIGHT_PROJECTILE_VISUAL_THICKNESS: f32 = 6.0;
const LOBBED_PROJECTILE_SHADOW_RADIUS: f32 = 11.0;
const LOBBED_PROJECTILE_LANDING_MARKER_RADIUS: f32 = 15.0;

#[derive(Component)]
pub(in super::super) struct SentryVisual3d;

#[derive(Component)]
pub(in super::super) struct ConcealmentFieldVisual3d;

#[derive(Component)]
pub(in super::super) struct ElementalFieldVisual3d;

#[derive(Component)]
pub(in super::super) struct PersistentSplashVisual3d;

#[derive(Component)]
pub(in super::super) struct StickyBlobVisual3d;

#[derive(Component)]
pub(in super::super) struct StickyFuseProgressVisual3d {
    owner: Entity,
    explosion_radius: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LobbedProjectileAidKind {
    GroundShadow,
    LandingMarker,
}

#[derive(Component)]
pub(in super::super) struct LobbedProjectileAid3d {
    owner: Entity,
    kind: LobbedProjectileAidKind,
}

#[derive(Component)]
pub(in super::super) struct LobbedProjectileBody3d {
    owner: Entity,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ProjectileVisualReconcileAction {
    Keep,
    Spawn,
    Replace(Entity),
}

fn projectile_visual_reconcile_action(
    existing: Option<(Entity, vfx::CombatVisualSignature)>,
    desired: vfx::CombatVisualSignature,
) -> ProjectileVisualReconcileAction {
    match existing {
        None => ProjectileVisualReconcileAction::Spawn,
        Some((_, signature)) if signature == desired => ProjectileVisualReconcileAction::Keep,
        Some((root, _)) => ProjectileVisualReconcileAction::Replace(root),
    }
}

fn unique_projectile_visual_roots(
    commands: &mut Commands,
    visuals: &ProjectileVisualRootQuery,
) -> BTreeMap<Entity, (Entity, vfx::CombatVisualSignature)> {
    let mut roots = BTreeMap::new();
    let mut ordered_roots = visuals.iter().collect::<Vec<_>>();
    ordered_roots.sort_unstable_by_key(|(root, ..)| root.index());
    for (root, owner, signature) in ordered_roots {
        match roots.entry(owner.0) {
            Entry::Vacant(entry) => {
                entry.insert((root, *signature));
            }
            Entry::Occupied(_) => commands.entity(root).despawn(),
        }
    }
    roots
}

#[allow(
    clippy::cast_precision_loss,
    reason = "validated fuse deadlines are bounded far below f32 integer precision"
)]
fn sticky_fuse_progress(state: crate::combat::StickyBlobState, tick: u64) -> f32 {
    let duration = state
        .detonates_at_tick
        .saturating_sub(state.armed_at_tick)
        .max(1);
    let elapsed = tick.saturating_sub(state.armed_at_tick).min(duration);
    (elapsed as f32 / duration as f32).clamp(0.02, 1.0)
}

#[derive(Component)]
pub(in super::super) struct FighterGroundMarker3d {
    owner: Entity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FighterVisualIdentity {
    team: crate::combat::TeamId,
    marker_relation: GroundMarkerRelation,
}

type FighterPresentationQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        &'static crate::combat::TeamId,
        Has<Controlled>,
        Option<&'static crate::concealment::ConcealmentPresentationState>,
    ),
    With<Fighter>,
>;

type GroundMarkerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static FighterGroundMarker3d,
        &'static mut MeshMaterial3d<StandardMaterial>,
    ),
>;

type ProjectilePoseOwnerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static Rotation,
        Option<&'static crate::combat::StraightFlight>,
        Option<&'static crate::combat::LobbedFlight>,
    ),
    With<crate::combat::Projectile>,
>;

type ProjectileVisualPoseQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static CombatVisualOwner,
        &'static mut V3ProjectileVisual,
        &'static mut Transform,
    ),
    (Without<V3FighterVisual>, Without<SentryVisual3d>),
>;

type ProjectileVisualRootQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static CombatVisualOwner,
        &'static vfx::CombatVisualSignature,
    ),
    With<V3ProjectileVisual>,
>;

type SentryVisualPoseQuery<'w, 's> = Query<
    'w,
    's,
    (&'static CombatVisualOwner, &'static mut Transform),
    (
        With<SentryVisual3d>,
        Without<V3FighterVisual>,
        Without<V3ProjectileVisual>,
    ),
>;

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "fighter roots own fallback geometry and observer-relative ground markers"
)]
pub(in super::super) fn reconcile_fighter_visuals(
    mut commands: Commands,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    metrics: Res<FighterPresentationMetrics>,
    fighters: FighterPresentationQuery,
    visuals: Query<(Entity, &CombatVisualOwner), With<V3FighterVisual>>,
    mut ground_markers: GroundMarkerQuery,
) {
    let roots = unique_roots(&mut commands, &visuals);
    let controlled_team = fighters
        .iter()
        .find_map(|(_, _, team, controlled, _)| controlled.then_some(*team));
    update_ground_markers(&fighters, &mut ground_markers, controlled_team, &materials);

    for (owner, position, team, controlled, _) in &fighters {
        if !roots.contains(&owner) {
            spawn_fighter(
                &mut commands,
                &primitives,
                &materials,
                owner,
                position.0,
                FighterVisualIdentity {
                    team: *team,
                    marker_relation: ground_marker_relation(*team, controlled, controlled_team),
                },
                metrics.radius,
            );
        }
    }
    for (root, owner) in &visuals {
        if fighters.get(owner.0).is_err() {
            commands.entity(root).despawn();
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "projectile materialization consumes replicated delivery shape plus the bounded optional particle adapter"
)]
pub(in super::super) fn reconcile_projectile_visuals(
    mut commands: Commands,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    particle_assets: Option<Res<VfxParticleAssets>>,
    particle_emitters: Query<(Entity, &VfxParticleEmitter3d)>,
    mut particle_diagnostics: Option<ResMut<VfxParticleDiagnostics>>,
    vfx: CombatVfxContext,
    projectiles: Query<
        (
            Entity,
            &Position,
            &crate::combat::ProjectileSource,
            Option<&crate::combat::StraightFlight>,
            Option<&crate::combat::ProjectileBody>,
            Option<&crate::combat::LobbedFlight>,
            Option<&crate::combat::ProjectileDeadline>,
            Option<&crate::combat::ReplicatedAttackSource>,
            Option<&crate::combat::AttackDelivery>,
        ),
        With<crate::combat::Projectile>,
    >,
    visuals: ProjectileVisualRootQuery,
) {
    let roots = unique_projectile_visual_roots(&mut commands, &visuals);
    let mut particle_budget =
        MAX_ACTIVE_PARTICLE_EMITTERS.saturating_sub(particle_emitters.iter().count());
    for (owner, position, source, straight, body, lobbed, deadline, attack_source, delivery) in
        &projectiles
    {
        if straight.is_some() && body.is_none() {
            continue;
        }
        let carrier = if lobbed.is_some() {
            vfx::VfxCarrier::LobbedProjectile
        } else {
            vfx::VfxCarrier::StraightProjectile
        };
        let radius = body.map_or(1.0, |body| body.shape.bounding_radius());
        let timing = if let Some(lobbed) = lobbed {
            vfx::VfxAuthoritativeTiming::persistent(
                lobbed.launched_at_tick,
                None,
                lobbed.lands_at_tick,
            )
        } else {
            vfx::VfxAuthoritativeTiming {
                activated_at_tick: straight.map(|flight| flight.launched_at_tick),
                observed_at_tick: None,
                expires_at_tick: deadline.map(|deadline| deadline.expires_at_tick),
            }
        };
        let signature = attack_source.map_or_else(
            || vfx::CombatVisualSignature {
                carrier,
                phase: vfx::VfxPhase::Travel,
                accents: [Some(vfx::VfxAccent::Neutral), None],
                relationship: vfx
                    .cache()
                    .observer()
                    .relationship(source.owner_network_entity_id, source.team_id),
                context: None,
                geometry: vfx::VfxGeometry::Projectile { radius },
                identity: vfx::VfxPresentationIdentity::Neutral(source.shot_id.0),
                timing,
                variant: vfx.variant(),
            },
            |attack_source| {
                vfx.cache().signature_for_source(
                    attack_source.attack,
                    delivery.map_or(0, |delivery| delivery.delivery_index),
                    carrier,
                    vfx::VfxPhase::Travel,
                    vfx::VfxGeometry::Projectile { radius },
                    timing,
                    vfx.variant(),
                )
            },
        );
        match projectile_visual_reconcile_action(roots.get(&owner).copied(), signature) {
            ProjectileVisualReconcileAction::Keep => continue,
            ProjectileVisualReconcileAction::Spawn => {}
            ProjectileVisualReconcileAction::Replace(root) => {
                commands.entity(root).despawn();
                for (emitter_entity, emitter) in &particle_emitters {
                    if emitter.root == root {
                        commands.entity(emitter_entity).despawn();
                        particle_budget = particle_budget
                            .saturating_add(1)
                            .min(MAX_ACTIVE_PARTICLE_EMITTERS);
                    }
                }
            }
        }
        let wants_smoke = lobbed.is_some()
            && signature.accents.contains(&Some(vfx::VfxAccent::Fire))
            && signature.variant == vfx::VfxPresentationVariant::Normal;
        let spawned_smoke = spawn_projectile(
            &mut commands,
            &primitives,
            &materials,
            particle_assets.as_deref(),
            particle_budget > 0,
            owner,
            position.0,
            source.team_id,
            straight,
            body,
            lobbed,
            signature,
        );
        if spawned_smoke {
            particle_budget = particle_budget.saturating_sub(1);
        } else if wants_smoke && let Some(diagnostics) = particle_diagnostics.as_deref_mut() {
            diagnostics.suppressed = diagnostics.suppressed.saturating_add(1);
        }
    }
    for (root, owner, _) in &visuals {
        if projectiles.get(owner.0).is_err() {
            commands.entity(root).despawn();
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
pub(in super::super) fn reconcile_sentry_visuals(
    mut commands: Commands,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    vfx: CombatVfxContext,
    sentries: Query<
        (
            Entity,
            &Position,
            &crate::abilities::SentryIdentity,
            Option<&crate::abilities::SentryDeadline>,
        ),
        With<crate::abilities::Sentry>,
    >,
    visuals: Query<(Entity, &CombatVisualOwner), With<SentryVisual3d>>,
) {
    let roots = unique_roots(&mut commands, &visuals);
    for (owner, position, identity, deadline) in &sentries {
        if !roots.contains(&owner) {
            let signature = vfx::CombatVisualSignature {
                carrier: vfx::VfxCarrier::Deployable,
                phase: vfx::VfxPhase::Active,
                accents: [Some(vfx::VfxAccent::Neutral), None],
                relationship: vfx
                    .cache()
                    .observer()
                    .relationship(identity.owner_network_id, identity.team_id),
                context: Some(vfx::VfxContextOverride::Deployable {
                    ultimate_id: identity.ultimate_id,
                    deployable_id: identity.deployable_id,
                }),
                geometry: vfx::VfxGeometry::point(position.0),
                identity: vfx::VfxPresentationIdentity::Deployable(identity.deployable_id.0),
                timing: vfx::VfxAuthoritativeTiming {
                    activated_at_tick: None,
                    observed_at_tick: None,
                    expires_at_tick: deadline.map(|deadline| deadline.expires_at_tick),
                },
                variant: vfx.variant(),
            };
            spawn_sentry(
                &mut commands,
                &primitives,
                &materials,
                owner,
                position.0,
                identity.team_id,
                signature,
            );
        }
    }
    for (root, owner) in &visuals {
        if sentries.get(owner.0).is_err() {
            commands.entity(root).despawn();
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
pub(in super::super) fn reconcile_concealment_field_visuals(
    mut commands: Commands,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    vfx: CombatVfxContext,
    fields: Query<(Entity, &crate::concealment::ConcealmentFieldState)>,
    visuals: Query<(Entity, &CombatVisualOwner), With<ConcealmentFieldVisual3d>>,
) {
    let roots = unique_roots(&mut commands, &visuals);
    for (owner, state) in &fields {
        if !roots.contains(&owner)
            && let Some(radius) = state.radius()
        {
            let signature = vfx::CombatVisualSignature {
                carrier: vfx::VfxCarrier::PersistentArea,
                phase: vfx::VfxPhase::Active,
                accents: [Some(vfx::VfxAccent::Concealment), None],
                relationship: relationship_for_team(vfx.cache(), state.team),
                context: Some(vfx::VfxContextOverride::ConcealmentField),
                geometry: vfx::VfxGeometry::circle(state.center_vec2(), radius),
                identity: vfx::VfxPresentationIdentity::ReplicatedField(state.id.0),
                timing: vfx::VfxAuthoritativeTiming::persistent(
                    state.activated_at_tick,
                    None,
                    state.expires_at_tick,
                ),
                variant: vfx.variant(),
            };
            spawn_concealment_field(
                &mut commands,
                &primitives,
                &materials,
                owner,
                state.center_vec2(),
                radius,
                state.team,
                signature,
            );
        }
    }
    for (root, owner) in &visuals {
        if fields.get(owner.0).is_err() {
            commands.entity(root).despawn();
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
pub(in super::super) fn reconcile_elemental_field_visuals(
    mut commands: Commands,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    vfx: CombatVfxContext,
    fields: Query<(Entity, &crate::combat::ElementalFieldState)>,
    visuals: Query<(Entity, &CombatVisualOwner), With<ElementalFieldVisual3d>>,
) {
    let roots = unique_roots(&mut commands, &visuals);
    for (owner, state) in &fields {
        if !roots.contains(&owner)
            && let Some(radius) = state.radius()
        {
            let signature = vfx::CombatVisualSignature {
                carrier: vfx::VfxCarrier::PersistentArea,
                phase: vfx::VfxPhase::Active,
                accents: [Some(vfx::accent_for_elemental_field(state.kind)), None],
                relationship: vfx
                    .cache()
                    .observer()
                    .relationship(state.owner_network_entity_id, state.team_id),
                context: Some(vfx::VfxContextOverride::ElementalField(state.kind)),
                geometry: vfx::VfxGeometry::circle(state.center_vec2(), radius),
                identity: vfx::VfxPresentationIdentity::ReplicatedField(state.id.0),
                timing: vfx::VfxAuthoritativeTiming::persistent(
                    state.activated_at_tick,
                    None,
                    state.expires_at_tick,
                ),
                variant: vfx.variant(),
            };
            spawn_elemental_field(
                &mut commands,
                &primitives,
                &materials,
                owner,
                state.center_vec2(),
                radius,
                signature,
            );
        }
    }
    for (root, owner) in &visuals {
        if fields.get(owner.0).is_err() {
            commands.entity(root).despawn();
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
    reason = "persistent Splash reconciliation owns its bounded geometry, material, signature, and root lifecycle"
)]
pub(in super::super) fn reconcile_persistent_splash_visuals(
    mut commands: Commands,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    vfx: CombatVfxContext,
    splashes: Query<(
        Entity,
        &crate::combat::PersistentSplashState,
        Option<&crate::combat::ReplicatedAttackSource>,
        Option<&crate::combat::AttackDelivery>,
    )>,
    visuals: Query<(Entity, &CombatVisualOwner), With<PersistentSplashVisual3d>>,
) {
    let roots = unique_roots(&mut commands, &visuals);
    for (owner, state, source, delivery) in &splashes {
        if roots.contains(&owner) {
            continue;
        }
        let effect_material = |effect: crate::combat::PayloadEffectDefinition| match effect {
            crate::combat::PayloadEffectDefinition::Cold { .. }
            | crate::combat::PayloadEffectDefinition::Slow { .. } => {
                materials.elemental_cold.clone()
            }
            crate::combat::PayloadEffectDefinition::DamageOverTime {
                kind: crate::combat::DamageOverTimeKind::Poison,
                ..
            } => materials.elemental_poison.clone(),
            crate::combat::PayloadEffectDefinition::DamageOverTime {
                kind: crate::combat::DamageOverTimeKind::Fire,
                ..
            }
            | crate::combat::PayloadEffectDefinition::Damage { .. }
            | crate::combat::PayloadEffectDefinition::Knockback { .. } => {
                materials.elemental_fire.clone()
            }
            crate::combat::PayloadEffectDefinition::Heal { .. } => {
                materials.elemental_restoration.clone()
            }
        };
        let fill = state
            .effects
            .iter()
            .flatten()
            .copied()
            .find_map(|effect| {
                matches!(effect, crate::combat::PayloadEffectDefinition::Heal { .. })
                    .then(|| effect_material(effect))
            })
            .or_else(|| {
                state
                    .effects
                    .iter()
                    .flatten()
                    .next()
                    .copied()
                    .map(effect_material)
            })
            .unwrap_or_else(|| materials.neutral.clone());
        let boundary = state
            .effects
            .iter()
            .flatten()
            .copied()
            .find_map(|effect| {
                (!matches!(effect, crate::combat::PayloadEffectDefinition::Heal { .. })).then(
                    || match effect {
                        crate::combat::PayloadEffectDefinition::Cold { .. }
                        | crate::combat::PayloadEffectDefinition::Slow { .. } => {
                            materials.elemental_cold_bright.clone()
                        }
                        crate::combat::PayloadEffectDefinition::DamageOverTime {
                            kind: crate::combat::DamageOverTimeKind::Fire,
                            ..
                        } => materials.elemental_fire_bright.clone(),
                        _ => effect_material(effect),
                    },
                )
            })
            .unwrap_or_else(|| fill.clone());
        let geometry = match state.shape {
            crate::combat::PersistentAreaShape::Circle { radius } => {
                vfx::VfxGeometry::circle(state.center.as_vec2(), radius)
            }
            crate::combat::PersistentAreaShape::Rectangle { half_extents } => {
                vfx::VfxGeometry::Rectangle {
                    center: state.center.as_vec2().to_array(),
                    facing: state.facing,
                    half_extents,
                }
            }
        };
        let timing = vfx::VfxAuthoritativeTiming::persistent(
            state.activated_at_tick,
            None,
            state.expires_at_tick,
        );
        let mut signature = source.map_or_else(
            || vfx::CombatVisualSignature {
                carrier: vfx::VfxCarrier::PersistentArea,
                phase: vfx::VfxPhase::Active,
                accents: [Some(vfx::VfxAccent::Neutral), None],
                relationship: vfx::VfxRelationship::Neutral,
                context: None,
                geometry,
                identity: vfx::VfxPresentationIdentity::Neutral(state.activated_at_tick),
                timing,
                variant: vfx.variant(),
            },
            |source| {
                vfx.cache().signature_for_source(
                    source.attack,
                    delivery.map_or(0, |delivery| delivery.delivery_index),
                    vfx::VfxCarrier::PersistentArea,
                    vfx::VfxPhase::Active,
                    geometry,
                    timing,
                    vfx.variant(),
                )
            },
        );
        signature.accents = vfx::accents_for_payload_effects(state.effects.into_iter().flatten());
        spawn_persistent_splash(
            &mut commands,
            &primitives,
            &materials,
            owner,
            *state,
            fill,
            boundary,
            signature,
        );
    }
    for (root, owner) in &visuals {
        if splashes.get(owner.0).is_err() {
            commands.entity(root).despawn();
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the helper receives one complete immutable persistent-area presentation profile"
)]
fn spawn_persistent_splash(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    owner: Entity,
    state: crate::combat::PersistentSplashState,
    fill: Handle<StandardMaterial>,
    boundary: Handle<StandardMaterial>,
    signature: vfx::CombatVisualSignature,
) {
    let root = commands
        .spawn((
            CombatVisualOwner(owner),
            PersistentSplashVisual3d,
            signature,
            Transform::from_translation(ground_position(state.center.as_vec2()))
                .with_rotation(ground_rotation(Rotation::radians(state.facing))),
            Visibility::default(),
            Name::new("Persistent Splash visual root"),
        ))
        .id();
    commands
        .entity(root)
        .with_children(|parent| match state.shape {
            crate::combat::PersistentAreaShape::Circle { radius } => {
                if !spawn_elemental_area_style(parent, primitives, materials, signature, radius) {
                    parent.spawn((
                        Mesh3d(primitives.area_disc.clone()),
                        MeshMaterial3d(fill),
                        NotShadowCaster,
                        NotShadowReceiver,
                        Transform::from_xyz(0.0, GROUND_EFFECT_HEIGHT, 0.0)
                            .with_rotation(Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2))
                            .with_scale(Vec3::splat(radius)),
                        Name::new("Persistent Splash circle fill"),
                    ));
                    parent.spawn((
                        Mesh3d(primitives.area_ring.clone()),
                        MeshMaterial3d(boundary),
                        NotShadowCaster,
                        NotShadowReceiver,
                        Transform::from_xyz(0.0, GROUND_EFFECT_HEIGHT + 0.6, 0.0)
                            .with_rotation(Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2))
                            .with_scale(Vec3::splat(radius)),
                        Name::new("Persistent Splash circle boundary"),
                    ));
                }
            }
            crate::combat::PersistentAreaShape::Rectangle {
                half_extents: [x, y],
            } => {
                parent.spawn((
                    Mesh3d(primitives.unit_cuboid.clone()),
                    MeshMaterial3d(fill),
                    NotShadowCaster,
                    NotShadowReceiver,
                    Transform::from_xyz(0.0, GROUND_EFFECT_HEIGHT, 0.0).with_scale(Vec3::new(
                        x * 2.0,
                        0.25,
                        y * 2.0,
                    )),
                    Name::new("Persistent Splash rectangle fill"),
                ));
                for (position, scale) in [
                    (
                        Vec3::new(0.0, GROUND_EFFECT_HEIGHT + 0.6, -y),
                        Vec3::new(x * 2.0, 0.5, 2.0),
                    ),
                    (
                        Vec3::new(0.0, GROUND_EFFECT_HEIGHT + 0.6, y),
                        Vec3::new(x * 2.0, 0.5, 2.0),
                    ),
                    (
                        Vec3::new(-x, GROUND_EFFECT_HEIGHT + 0.6, 0.0),
                        Vec3::new(2.0, 0.5, y * 2.0),
                    ),
                    (
                        Vec3::new(x, GROUND_EFFECT_HEIGHT + 0.6, 0.0),
                        Vec3::new(2.0, 0.5, y * 2.0),
                    ),
                ] {
                    parent.spawn((
                        Mesh3d(primitives.unit_cuboid.clone()),
                        MeshMaterial3d(boundary.clone()),
                        NotShadowCaster,
                        NotShadowReceiver,
                        Transform::from_translation(position).with_scale(scale),
                        Name::new("Persistent Splash rectangle boundary"),
                    ));
                }
            }
        });
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "sticky reconciliation joins the complete replicated payload identity needed by one visual root"
)]
pub(in super::super) fn reconcile_sticky_blob_visuals(
    mut commands: Commands,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    vfx: CombatVfxContext,
    blobs: Query<(
        Entity,
        &Position,
        &crate::combat::StickyBlobState,
        Option<&crate::combat::ReplicatedAttackSource>,
        Option<&crate::combat::AttackDelivery>,
    )>,
    visuals: Query<(Entity, &CombatVisualOwner), With<StickyBlobVisual3d>>,
) {
    let roots = unique_roots(&mut commands, &visuals);
    for (owner, position, state, source, delivery) in &blobs {
        if roots.contains(&owner) {
            continue;
        }
        let blob_scale = match state.kind {
            crate::combat::StickyBlobKind::Primary => 9.0,
            crate::combat::StickyBlobKind::UltimateSecondary => 6.0,
        };
        let timing = vfx::VfxAuthoritativeTiming::persistent(
            state.armed_at_tick,
            None,
            state.detonates_at_tick,
        );
        let signature = source.map_or_else(
            || vfx::CombatVisualSignature {
                carrier: vfx::VfxCarrier::StickyPayload,
                phase: vfx::VfxPhase::Active,
                accents: [Some(vfx::VfxAccent::Neutral), None],
                relationship: vfx::VfxRelationship::Neutral,
                context: matches!(state.kind, crate::combat::StickyBlobKind::UltimateSecondary)
                    .then_some(vfx::VfxContextOverride::StickyUltimateSecondary),
                geometry: vfx::VfxGeometry::circle(position.0, state.explosion_radius),
                identity: vfx::VfxPresentationIdentity::Neutral(state.armed_at_tick),
                timing,
                variant: vfx.variant(),
            },
            |source| {
                vfx.cache().signature_for_source(
                    source.attack,
                    delivery.map_or(0, |delivery| delivery.delivery_index),
                    vfx::VfxCarrier::StickyPayload,
                    vfx::VfxPhase::Active,
                    vfx::VfxGeometry::circle(position.0, state.explosion_radius),
                    timing,
                    vfx.variant(),
                )
            },
        );
        let root = commands
            .spawn((
                CombatVisualOwner(owner),
                StickyBlobVisual3d,
                signature,
                Transform::from_translation(ground_position(position.0)),
                Visibility::default(),
                Name::new("Sticky Blomb armed visual root"),
            ))
            .id();
        commands.entity(root).with_children(|parent| {
            parent.spawn((
                Mesh3d(primitives.effect_sphere.clone()),
                MeshMaterial3d(materials.sticky_blob.clone()),
                NotShadowCaster,
                Transform::from_xyz(0.0, blob_scale, 0.0).with_scale(Vec3::splat(blob_scale)),
                Name::new("Sticky Blomb armed body"),
            ));
            parent.spawn((
                Mesh3d(primitives.area_ring.clone()),
                MeshMaterial3d(materials.sticky_warning.clone()),
                NotShadowCaster,
                NotShadowReceiver,
                Transform::from_xyz(0.0, GROUND_EFFECT_HEIGHT + 0.8, 0.0)
                    .with_rotation(Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::splat(state.explosion_radius)),
                Name::new("Sticky Blomb future explosion boundary"),
            ));
            parent.spawn((
                Mesh3d(primitives.area_disc.clone()),
                MeshMaterial3d(materials.sticky_warning.clone()),
                NotShadowCaster,
                NotShadowReceiver,
                StickyFuseProgressVisual3d {
                    owner,
                    explosion_radius: state.explosion_radius,
                },
                Transform::from_xyz(0.0, GROUND_EFFECT_HEIGHT + 0.6, 0.0)
                    .with_rotation(Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::splat(state.explosion_radius * 0.02)),
                Name::new("Sticky Blomb fuse progress fill"),
            ));
        });
    }
    for (root, owner) in &visuals {
        if blobs.get(owner.0).is_err() {
            commands.entity(root).despawn();
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
pub(in super::super) fn update_sticky_blob_fuse_progress(
    authoritative_ticks: Query<&AuthoritativeTick>,
    blobs: Query<&crate::combat::StickyBlobState>,
    mut fills: Query<(&StickyFuseProgressVisual3d, &mut Transform)>,
) {
    let tick = authoritative_ticks
        .iter()
        .map(|tick| tick.0)
        .max()
        .unwrap_or_default();
    for (fill, mut transform) in &mut fills {
        let Ok(state) = blobs.get(fill.owner) else {
            continue;
        };
        let progress = sticky_fuse_progress(*state, tick);
        transform.scale = Vec3::splat(fill.explosion_radius * progress);
    }
}

#[cfg(test)]
mod sticky_visual_tests {
    use super::*;

    #[test]
    fn fuse_fill_reaches_full_radius_at_the_authoritative_deadline() {
        let state = crate::combat::StickyBlobState {
            kind: crate::combat::StickyBlobKind::Primary,
            attached_to: None,
            armed_at_tick: 20,
            detonates_at_tick: 89,
            explosion_radius: 85.44,
        };
        assert!((sticky_fuse_progress(state, 20) - 0.02).abs() < f32::EPSILON);
        assert!((sticky_fuse_progress(state, 89) - 1.0).abs() < f32::EPSILON);
        assert!((sticky_fuse_progress(state, 200) - 1.0).abs() < f32::EPSILON);
    }
}

#[allow(clippy::needless_pass_by_value, clippy::type_complexity)]
pub(in super::super) fn write_sticky_blob_visual_poses(
    blobs: Query<&Position, With<crate::combat::StickyBlobState>>,
    mut visuals: Query<
        (&CombatVisualOwner, &mut Transform),
        (With<StickyBlobVisual3d>, Without<V3ProjectileVisual>),
    >,
) {
    for (owner, mut transform) in &mut visuals {
        if let Ok(position) = blobs.get(owner.0) {
            transform.translation = ground_position(position.0);
        }
    }
}

fn spawn_elemental_field(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    owner: Entity,
    center: Vec2,
    radius: f32,
    signature: vfx::CombatVisualSignature,
) {
    let root = commands
        .spawn((
            CombatVisualOwner(owner),
            ElementalFieldVisual3d,
            signature,
            Transform::from_translation(ground_position(center)),
            Visibility::default(),
            Name::new("Elemental Field visual root"),
        ))
        .id();
    commands.entity(root).with_children(|parent| {
        if !spawn_elemental_area_style(parent, primitives, materials, signature, radius) {
            let material = match signature.accents[0] {
                Some(vfx::VfxAccent::Poison) => materials.elemental_poison.clone(),
                Some(vfx::VfxAccent::Restoration) => materials.elemental_restoration.clone(),
                _ => materials.neutral.clone(),
            };
            parent.spawn((
                Mesh3d(primitives.area_disc.clone()),
                MeshMaterial3d(material.clone()),
                NotShadowCaster,
                NotShadowReceiver,
                Transform::from_xyz(0.0, GROUND_EFFECT_HEIGHT, 0.0)
                    .with_rotation(Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::splat(radius)),
                Name::new("Elemental Field fill"),
            ));
            parent.spawn((
                Mesh3d(primitives.area_ring.clone()),
                MeshMaterial3d(material),
                NotShadowCaster,
                NotShadowReceiver,
                Transform::from_xyz(0.0, GROUND_EFFECT_HEIGHT + 0.6, 0.0)
                    .with_rotation(Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::splat(radius)),
                Name::new("Elemental Field boundary"),
            ));
        }
    });
}

#[allow(
    clippy::too_many_arguments,
    reason = "the field helper receives one explicit visual recipe plus its semantic signature"
)]
fn spawn_concealment_field(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    owner: Entity,
    center: Vec2,
    radius: f32,
    team: crate::combat::TeamId,
    signature: vfx::CombatVisualSignature,
) {
    let (fill, boundary) = if team.0 == 1 {
        (
            materials.concealment_field_red_fill.clone(),
            materials.concealment_field_red_boundary.clone(),
        )
    } else {
        (
            materials.concealment_field_blue_fill.clone(),
            materials.concealment_field_blue_boundary.clone(),
        )
    };
    let root = commands
        .spawn((
            CombatVisualOwner(owner),
            ConcealmentFieldVisual3d,
            signature,
            Transform::from_translation(ground_position(center)),
            Visibility::default(),
            Name::new("V9 Concealment Field visual root"),
        ))
        .id();
    commands.entity(root).with_children(|parent| {
        parent.spawn((
            Mesh3d(primitives.area_disc.clone()),
            MeshMaterial3d(fill),
            NotShadowCaster,
            NotShadowReceiver,
            Transform::from_xyz(0.0, GROUND_EFFECT_HEIGHT, 0.0)
                .with_rotation(Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2))
                .with_scale(Vec3::splat(radius)),
            Name::new("V9 Concealment Field fill"),
        ));
        parent.spawn((
            Mesh3d(primitives.area_ring.clone()),
            MeshMaterial3d(boundary),
            NotShadowCaster,
            NotShadowReceiver,
            Transform::from_xyz(0.0, GROUND_EFFECT_HEIGHT + 0.5, 0.0)
                .with_rotation(Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2))
                .with_scale(Vec3::splat(radius)),
            Name::new("V9 Concealment Field boundary"),
        ));
    });
}

fn relationship_for_team(
    cache: &vfx::VfxSignatureCache,
    team: crate::combat::TeamId,
) -> vfx::VfxRelationship {
    let observer = cache.observer();
    if team.0 == 0 || observer.team_id.is_none() {
        vfx::VfxRelationship::Neutral
    } else if observer.team_id == Some(team) {
        vfx::VfxRelationship::Ally
    } else {
        vfx::VfxRelationship::Enemy
    }
}

fn update_ground_markers(
    fighters: &FighterPresentationQuery,
    ground_markers: &mut GroundMarkerQuery,
    controlled_team: Option<crate::combat::TeamId>,
    materials: &Material3dAssets,
) {
    for (marker, mut material) in ground_markers {
        if let Ok((_, _, team, controlled, _)) = fighters.get(marker.owner) {
            let desired = ground_marker_material(
                ground_marker_relation(*team, controlled, controlled_team),
                materials,
            );
            if material.0 != desired {
                material.0 = desired;
            }
        }
    }
}
fn spawn_fighter(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    owner: Entity,
    position: Vec2,
    identity: FighterVisualIdentity,
    fighter_radius: f32,
) {
    let root = commands
        .spawn((
            CombatVisualOwner(owner),
            V3FighterVisual {
                last_position: position,
                moving: false,
                shoot_seconds: 0.0,
            },
            Transform::from_translation(ground_position(position)),
            Visibility::default(),
            Name::new("V3 independent fighter visual root"),
        ))
        .id();
    commands.entity(root).with_children(|parent| {
        parent.spawn((
            V3FallbackVisual { owner },
            Mesh3d(primitives.fighter.clone()),
            MeshMaterial3d(team_material(identity.team, materials)),
            Transform::from_xyz(0.0, fighter_radius, 0.0),
            Name::new("V3 fighter fallback"),
        ));
        parent.spawn((
            FighterGroundMarker3d { owner },
            Mesh3d(primitives.ground_ring.clone()),
            MeshMaterial3d(ground_marker_material(identity.marker_relation, materials)),
            NotShadowCaster,
            NotShadowReceiver,
            Transform::from_xyz(0.0, GROUND_MARKER_HEIGHT, 0.0)
                .with_rotation(Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2)),
            Name::new("V3 fighter team ring"),
        ));
        parent.spawn((
            FighterGroundMarker3d { owner },
            Mesh3d(primitives.fighter_facing.clone()),
            MeshMaterial3d(ground_marker_material(identity.marker_relation, materials)),
            NotShadowCaster,
            NotShadowReceiver,
            Transform::from_xyz(0.0, GROUND_MARKER_HEIGHT, 0.0)
                .with_rotation(Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2)),
            Name::new("V3 fighter ring facing indicator"),
        ));
    });
}

#[allow(
    clippy::too_many_arguments,
    reason = "the projectile root needs the complete immutable spawn presentation profile"
)]
fn spawn_projectile(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    particle_assets: Option<&VfxParticleAssets>,
    allow_particle_smoke: bool,
    owner: Entity,
    position: Vec2,
    team: crate::combat::TeamId,
    straight: Option<&crate::combat::StraightFlight>,
    body: Option<&crate::combat::ProjectileBody>,
    lobbed: Option<&crate::combat::LobbedFlight>,
    signature: vfx::CombatVisualSignature,
) -> bool {
    let root = commands
        .spawn((
            CombatVisualOwner(owner),
            V3ProjectileVisual {
                planar_position: straight.map_or(position, |flight| flight.origin.as_vec2()),
            },
            signature,
            Transform::from_translation(ground_position(position)),
            Visibility::default(),
            Name::new("V3 independent projectile visual root"),
        ))
        .id();
    commands.entity(root).with_children(|parent| {
        let transform = if lobbed.is_some() {
            Transform::default()
        } else {
            Transform::from_scale(straight_projectile_visual_scale(
                *body.expect("straight projectile visual requires replicated body"),
            ))
        };
        let mesh = if lobbed.is_some() {
            primitives.lobbed_projectile.clone()
        } else {
            primitives.projectile.clone()
        };
        let relationship_material = team_material(team, materials);
        let visual_radius = body.map_or(9.0, |body| body.shape.bounding_radius());
        if lobbed.is_some() {
            parent
                .spawn((
                    LobbedProjectileBody3d { owner },
                    Transform::default(),
                    Visibility::default(),
                    Name::new("Lobbed projectile tangent-aligned body"),
                ))
                .with_children(|body_parent| {
                    spawn_projectile_geometry(
                        body_parent,
                        primitives,
                        materials,
                        signature,
                        mesh,
                        transform,
                        relationship_material,
                        team,
                        visual_radius,
                    );
                });
        } else {
            spawn_projectile_geometry(
                parent,
                primitives,
                materials,
                signature,
                mesh,
                transform,
                relationship_material,
                team,
                visual_radius,
            );
        }
        if lobbed.is_some() {
            spawn_lobbed_projectile_aids(parent, primitives, materials, owner);
        }
    });
    allow_particle_smoke
        && signature.accents.contains(&Some(vfx::VfxAccent::Fire))
        && signature.variant == vfx::VfxPresentationVariant::Normal
        && particle_assets.is_some_and(|assets| {
            assets.spawn_projectile_smoke(
                commands,
                primitives,
                owner,
                root,
                signature.stable_seed(),
            )
        })
}

fn spawn_lobbed_projectile_aids(
    parent: &mut ChildSpawnerCommands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    owner: Entity,
) {
    for (kind, mesh, material, name) in [
        (
            LobbedProjectileAidKind::GroundShadow,
            primitives.area_disc.clone(),
            materials.lobbed_shadow.clone(),
            "Lobbed projectile ground-projected shadow",
        ),
        (
            LobbedProjectileAidKind::LandingMarker,
            primitives.area_ring.clone(),
            materials.effect_impact.clone(),
            "Lobbed projectile landing-point marker",
        ),
    ] {
        parent.spawn((
            LobbedProjectileAid3d { owner, kind },
            Mesh3d(mesh),
            MeshMaterial3d(material),
            NotShadowCaster,
            NotShadowReceiver,
            Transform::default(),
            Visibility::Hidden,
            Name::new(name),
        ));
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the focused helper receives one complete retained projectile visual profile"
)]
fn spawn_projectile_geometry(
    parent: &mut ChildSpawnerCommands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    signature: vfx::CombatVisualSignature,
    mesh: Handle<Mesh>,
    transform: Transform,
    relationship_material: Handle<StandardMaterial>,
    team: crate::combat::TeamId,
    visual_radius: f32,
) {
    if !spawn_elemental_projectile_style(
        parent,
        primitives,
        materials,
        signature,
        mesh.clone(),
        transform,
        relationship_material.clone(),
        visual_radius,
    ) && !super::straight_projectile_style::spawn_neutral_straight_projectile_style(
        parent,
        primitives,
        materials,
        signature,
        transform,
        team,
        visual_radius,
    ) {
        parent.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(relationship_material),
            NotShadowCaster,
            transform,
            Name::new("V3 projectile geometry"),
        ));
    }
}

fn straight_projectile_visual_scale(body: crate::combat::ProjectileBody) -> Vec3 {
    match body.shape {
        crate::combat::ProjectileShape::Circle { radius } => {
            Vec3::new(radius, STRAIGHT_PROJECTILE_VISUAL_THICKNESS, radius)
        }
    }
}

fn spawn_sentry(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    owner: Entity,
    position: Vec2,
    team: crate::combat::TeamId,
    signature: vfx::CombatVisualSignature,
) {
    let root = commands
        .spawn((
            CombatVisualOwner(owner),
            SentryVisual3d,
            signature,
            Transform::from_translation(ground_position(position)),
            Visibility::default(),
            Name::new("V3 independent sentry visual root"),
        ))
        .id();
    commands.entity(root).with_children(|parent| {
        parent.spawn((
            Mesh3d(primitives.sentry_base.clone()),
            MeshMaterial3d(materials.neutral.clone()),
            Transform::from_xyz(0.0, 4.0, 0.0),
        ));
        parent.spawn((
            Mesh3d(primitives.sentry_body.clone()),
            MeshMaterial3d(team_material(team, materials)),
            Transform::from_xyz(0.0, 18.0, 0.0),
        ));
        parent.spawn((
            Mesh3d(primitives.sentry_direction.clone()),
            MeshMaterial3d(team_material(team, materials)),
            NotShadowCaster,
            Transform::from_xyz(25.0, 23.0, 0.0).with_scale(Vec3::new(0.8, 0.7, 0.7)),
        ));
    });
}
pub(in super::super) fn write_fighter_visual_poses(
    fighter_owners: Query<(&Position, &Rotation), With<Fighter>>,
    mut fighter_visuals: Query<(&CombatVisualOwner, &mut V3FighterVisual, &mut Transform)>,
) {
    for (owner, mut visual, mut transform) in &mut fighter_visuals {
        if let Ok((position, rotation)) = fighter_owners.get(owner.0) {
            visual.moving = visual.last_position.distance_squared(position.0) > 0.25;
            visual.last_position = position.0;
            transform.translation = ground_position(position.0);
            transform.rotation = ground_rotation(*rotation);
        }
    }
}

#[allow(clippy::cast_precision_loss, clippy::needless_pass_by_value)]
pub(in super::super) fn write_projectile_visual_poses(
    time: Res<Time>,
    ticks: Query<&AuthoritativeTick>,
    projectile_owners: ProjectilePoseOwnerQuery,
    mut projectile_visuals: ProjectileVisualPoseQuery,
) {
    let current_tick = ticks.iter().next().map_or(0, |tick| tick.0);
    for (owner, mut visual, mut transform) in &mut projectile_visuals {
        let Ok((position, rotation, straight, lobbed)) = projectile_owners.get(owner.0) else {
            continue;
        };
        let planar = if let Some(straight) = straight {
            visual.planar_position = catch_up_projectile_position(
                visual.planar_position,
                position.0,
                straight.speed,
                time.delta_secs(),
            );
            visual.planar_position
        } else {
            position.0
        };
        transform.translation = ground_position(planar);
        if let Some(lobbed) = lobbed {
            let duration = lobbed
                .lands_at_tick
                .saturating_sub(lobbed.launched_at_tick)
                .max(1);
            let progress =
                current_tick.saturating_sub(lobbed.launched_at_tick) as f32 / duration as f32;
            transform.translation.y = LOBBED_PROJECTILE_LAUNCH_HEIGHT
                + crate::combat::delivery::lob_height(progress, lobbed.visual_arc_height);
            transform.rotation = Quat::IDENTITY;
        } else {
            transform.translation.y = STRAIGHT_PROJECTILE_HEIGHT;
            transform.rotation = ground_rotation(*rotation);
        }
    }
}

#[allow(clippy::cast_precision_loss, clippy::needless_pass_by_value)]
pub(in super::super) fn write_lobbed_projectile_body_poses(
    ticks: Query<&AuthoritativeTick>,
    owners: Query<&crate::combat::LobbedFlight, With<crate::combat::Projectile>>,
    mut bodies: Query<(&LobbedProjectileBody3d, &mut Transform)>,
) {
    let current_tick = ticks.iter().map(|tick| tick.0).max().unwrap_or_default();
    for (body, mut transform) in &mut bodies {
        let Ok(flight) = owners.get(body.owner) else {
            continue;
        };
        let progress = lobbed_flight_progress(*flight, current_tick);
        transform.rotation = lobbed_arc_tangent_rotation(*flight, progress);
    }
}

#[allow(clippy::cast_precision_loss, clippy::needless_pass_by_value)]
pub(in super::super) fn write_lobbed_projectile_aid_poses(
    ticks: Query<&AuthoritativeTick>,
    owners: Query<(&Position, &crate::combat::LobbedFlight), With<crate::combat::Projectile>>,
    mut aids: Query<(&LobbedProjectileAid3d, &mut Transform, &mut Visibility)>,
) {
    let current_tick = ticks.iter().map(|tick| tick.0).max().unwrap_or_default();
    for (aid, mut transform, mut visibility) in &mut aids {
        let Ok((position, flight)) = owners.get(aid.owner) else {
            continue;
        };
        let progress = lobbed_flight_progress(*flight, current_tick);
        let height = lobbed_flight_height(*flight, progress);
        match aid.kind {
            LobbedProjectileAidKind::GroundShadow => {
                let maximum_height =
                    (LOBBED_PROJECTILE_LAUNCH_HEIGHT + flight.visual_arc_height).max(1.0);
                let height_ratio = (height / maximum_height).clamp(0.0, 1.0);
                let radius = LOBBED_PROJECTILE_SHADOW_RADIUS * (1.0 - height_ratio * 0.38);
                *transform = Transform::from_xyz(0.0, GROUND_EFFECT_HEIGHT - height, 0.0)
                    .with_rotation(Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::splat(radius));
                *visibility = Visibility::Inherited;
            }
            LobbedProjectileAidKind::LandingMarker => {
                let landing_offset = ground_position(flight.landing.as_vec2() - position.0)
                    + Vec3::Y * (GROUND_EFFECT_HEIGHT + 0.3 - height);
                *transform = Transform::from_translation(landing_offset)
                    .with_rotation(Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::splat(LOBBED_PROJECTILE_LANDING_MARKER_RADIUS));
                *visibility = Visibility::Inherited;
            }
        }
    }
}

#[allow(clippy::cast_precision_loss)]
fn lobbed_flight_progress(flight: crate::combat::LobbedFlight, current_tick: u64) -> f32 {
    let duration = flight
        .lands_at_tick
        .saturating_sub(flight.launched_at_tick)
        .max(1);
    let elapsed = current_tick
        .saturating_sub(flight.launched_at_tick)
        .min(duration);
    elapsed as f32 / duration as f32
}

fn lobbed_flight_height(flight: crate::combat::LobbedFlight, progress: f32) -> f32 {
    LOBBED_PROJECTILE_LAUNCH_HEIGHT
        + crate::combat::delivery::lob_height(progress, flight.visual_arc_height)
}

fn lobbed_arc_tangent(flight: crate::combat::LobbedFlight, progress: f32) -> Vec3 {
    let planar = ground_position(flight.landing.as_vec2() - flight.launch.as_vec2());
    let vertical = 4.0 * flight.visual_arc_height * (1.0 - 2.0 * progress.clamp(0.0, 1.0));
    (planar + Vec3::Y * vertical).normalize_or(Vec3::X)
}

fn lobbed_arc_tangent_rotation(flight: crate::combat::LobbedFlight, progress: f32) -> Quat {
    Quat::from_rotation_arc(Vec3::X, lobbed_arc_tangent(flight, progress))
}

pub(in super::super) fn write_sentry_visual_poses(
    sentry_owners: Query<(&Position, &Rotation), With<crate::abilities::Sentry>>,
    mut sentry_visuals: SentryVisualPoseQuery,
) {
    for (owner, mut transform) in &mut sentry_visuals {
        if let Ok((position, rotation)) = sentry_owners.get(owner.0) {
            transform.translation = ground_position(position.0);
            transform.rotation = ground_rotation(*rotation);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn straight_projectile_visual_footprint_matches_replicated_circle() {
        let pulse = straight_projectile_visual_scale(crate::combat::ProjectileBody::circle(6.0));
        let scatter = straight_projectile_visual_scale(crate::combat::ProjectileBody::circle(4.0));
        assert!((pulse.x - 6.0).abs() < f32::EPSILON);
        assert!((pulse.z - 6.0).abs() < f32::EPSILON);
        assert!((scatter.x - 4.0).abs() < f32::EPSILON);
        assert!((scatter.z - 4.0).abs() < f32::EPSILON);
        assert!((pulse.y - STRAIGHT_PROJECTILE_VISUAL_THICKNESS).abs() < f32::EPSILON);
    }

    #[test]
    fn lobbed_body_follows_bounded_authoritative_progress_and_arc_tangent() {
        let flight = crate::combat::LobbedFlight {
            launch: crate::combat::WorldPoint::from(Vec2::new(-100.0, 0.0)),
            landing: crate::combat::WorldPoint::from(Vec2::new(100.0, 0.0)),
            launched_at_tick: 20,
            lands_at_tick: 100,
            visual_arc_height: 120.0,
        };
        assert!(lobbed_flight_progress(flight, 0).abs() < f32::EPSILON);
        assert!((lobbed_flight_progress(flight, 60) - 0.5).abs() < f32::EPSILON);
        assert!((lobbed_flight_progress(flight, 200) - 1.0).abs() < f32::EPSILON);
        let ascending = lobbed_arc_tangent(flight, 0.25);
        let apex = lobbed_arc_tangent(flight, 0.5);
        let descending = lobbed_arc_tangent(flight, 0.75);
        assert!(ascending.y > 0.0);
        assert!(apex.y.abs() < f32::EPSILON);
        assert!(descending.y < 0.0);
        let body_forward = lobbed_arc_tangent_rotation(flight, 0.25) * Vec3::X;
        assert!(body_forward.dot(ascending) > 0.999);
    }

    #[test]
    fn late_replicated_projectile_style_replaces_the_neutral_visual() {
        let root = Entity::from_bits(42);
        let neutral = vfx::CombatVisualSignature::neutral_point(7, Vec2::ZERO);
        let mut fire = neutral;
        fire.carrier = vfx::VfxCarrier::LobbedProjectile;
        fire.phase = vfx::VfxPhase::Travel;
        fire.accents = [Some(vfx::VfxAccent::Fire), None];
        fire.geometry = vfx::VfxGeometry::Projectile { radius: 1.0 };

        assert_eq!(
            projectile_visual_reconcile_action(None, neutral),
            ProjectileVisualReconcileAction::Spawn
        );
        assert_eq!(
            projectile_visual_reconcile_action(Some((root, neutral)), neutral),
            ProjectileVisualReconcileAction::Keep
        );
        assert_eq!(
            projectile_visual_reconcile_action(Some((root, neutral)), fire),
            ProjectileVisualReconcileAction::Replace(root)
        );
    }

    fn fire_lobbed_test_loadout_and_attack() -> (
        crate::builds::ResolvedMatchLoadout,
        crate::combat::AttackSource,
    ) {
        let mut loadout = crate::builds::resolve_build_recipe(
            &crate::builds::BuildCatalog::embedded().unwrap(),
            &crate::combat::WeaponCatalog::embedded().unwrap(),
            crate::builds::BrawlerBuildRecipe {
                weapon: crate::builds::WeaponChoice::Preset(crate::combat::WeaponPresetId(3)),
                ultimate: crate::builds::UltimateDefinitionId(1),
                passives: [
                    crate::builds::PassiveDefinitionId(1),
                    crate::builds::PassiveDefinitionId(6),
                ],
            },
        )
        .unwrap();
        let fingerprint = crate::combat::WeaponRecipeFingerprint(0xfeed);
        loadout.primary_weapon.recipe_fingerprint = fingerprint;
        loadout.primary_weapon.recipe.payload_bundles[0]
            .effects
            .push(crate::combat::PayloadEffectDefinition::DamageOverTime {
                kind: crate::combat::DamageOverTimeKind::Fire,
                damage_per_tick: 2,
                tick_interval: 10,
                duration_ticks: 30,
                recipients: crate::combat::RecipientPolicy::Hostiles,
            });
        let attack = crate::combat::AttackSource {
            kind: crate::combat::CombatSourceKind::PrimaryWeapon,
            attack_id: crate::combat::AttackId(77),
            player_id: PlayerId(1),
            owner_network_entity_id: NetworkEntityId(9),
            team_id: crate::combat::TeamId(1),
            recipe_fingerprint: fingerprint,
            legacy_compatibility: false,
            source_preset_id: Some(crate::combat::WeaponPresetId(3)),
            origin: crate::combat::WorldPoint::from(Vec2::ZERO),
            facing: 0.0,
        };
        (loadout, attack)
    }

    #[test]
    fn production_reconcile_upgrades_a_late_replicated_lobbed_fire_style() {
        let (loadout, attack) = fire_lobbed_test_loadout_and_attack();

        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<super::super::elemental_style::FireAreaMaterial>>()
            .init_resource::<Assets<super::super::elemental_style::ElementalSupportAreaMaterial>>()
            .init_resource::<Assets<super::super::cone_spray::ConeSprayMaterial>>()
            .init_resource::<
                Assets<super::super::straight_projectile_style::StraightProjectileTrailMaterial>,
            >()
            .init_resource::<crate::builds::BuildCatalogResource>()
            .init_resource::<vfx::VfxSignatureCache>()
            .add_message::<crate::combat::client::DeduplicatedCombatCue>()
            .add_systems(Startup, setup_3d_foundation)
            .add_systems(
                Update,
                (
                    vfx::update_vfx_signature_cache,
                    reconcile_projectile_visuals,
                )
                    .chain(),
            );
        app.world_mut().spawn((
            Fighter,
            attack.owner_network_entity_id,
            attack.team_id,
            loadout,
        ));
        app.world_mut().spawn(AuthoritativeTick(20));
        let projectile = app
            .world_mut()
            .spawn((
                crate::combat::Projectile,
                crate::combat::ProjectileSource {
                    shot_id: crate::combat::ShotId(77),
                    player_id: attack.player_id,
                    owner_network_entity_id: attack.owner_network_entity_id,
                    team_id: attack.team_id,
                    weapon_definition_id: crate::combat::ARC_LAUNCHER_DEFINITION,
                },
                Position(Vec2::ZERO),
                crate::combat::LobbedFlight {
                    launch: Vec2::ZERO.into(),
                    landing: Vec2::new(100.0, 0.0).into(),
                    launched_at_tick: 20,
                    lands_at_tick: 65,
                    visual_arc_height: 140.0,
                },
            ))
            .id();

        app.update();
        {
            let world = app.world_mut();
            let mut visuals =
                world.query_filtered::<&vfx::CombatVisualSignature, With<V3ProjectileVisual>>();
            let signature = *visuals.single(world).unwrap();
            assert_eq!(signature.accents, [Some(vfx::VfxAccent::Neutral), None]);
            assert_eq!(
                world.query::<&LobbedProjectileAid3d>().iter(world).count(),
                2
            );
        }

        app.world_mut().entity_mut(projectile).insert((
            crate::combat::ReplicatedAttackSource { attack },
            crate::combat::AttackDelivery {
                attack_id: attack.attack_id,
                delivery_index: 0,
            },
        ));
        app.update();

        let world = app.world_mut();
        let mut visuals =
            world.query_filtered::<&vfx::CombatVisualSignature, With<V3ProjectileVisual>>();
        let signature = *visuals.single(world).unwrap();
        assert_eq!(signature.carrier, vfx::VfxCarrier::LobbedProjectile);
        assert!(signature.accents.contains(&Some(vfx::VfxAccent::Fire)));
        assert_eq!(
            world.query::<&LobbedProjectileAid3d>().iter(world).count(),
            2
        );
    }

    #[test]
    fn independent_root_maps_positive_and_negative_simulation_y_once() {
        let mut app = App::new();
        app.add_systems(PostUpdate, write_fighter_visual_poses);
        let owner = app
            .world_mut()
            .spawn((
                Fighter,
                Position(Vec2::new(25.0, -80.0)),
                Rotation::default(),
            ))
            .id();
        let root = app
            .world_mut()
            .spawn((
                CombatVisualOwner(owner),
                V3FighterVisual {
                    last_position: Vec2::ZERO,
                    moving: false,
                    shoot_seconds: 0.0,
                },
                Transform::default(),
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Transform>(root).unwrap().translation,
            Vec3::new(25.0, 0.0, 80.0)
        );
        assert!(app.world().get::<Transform>(owner).is_none());
    }

    #[test]
    #[allow(
        clippy::assertions_on_constants,
        reason = "the regression locks the marker's deliberate separation from the floor plane"
    )]
    fn ground_marker_is_lifted_above_the_floor_plane() {
        assert!(GROUND_MARKER_HEIGHT > 0.0);
    }
}
