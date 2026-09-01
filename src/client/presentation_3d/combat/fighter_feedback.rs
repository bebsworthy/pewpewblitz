use super::super::*;
use super::{
    common::CombatVfxContext,
    dash_style::{DashTrailMaterial, DashTrailQuality, quality_for_variant},
    entities::FighterGroundMarker3d,
};
use std::collections::{HashMap, HashSet};

const CONCEALED_FIGHTER_ALPHA: f32 = 0.52;
const DASH_DISSIPATION_TICKS: u64 = 15;
const DASH_WAKE_WIDTH: f32 = 38.0;
const DASH_WAKE_REDUCED_WIDTH: f32 = 30.0;

#[derive(Component)]
pub(in super::super) struct FighterConcealmentMaterial {
    normal: Handle<StandardMaterial>,
    concealed: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
pub(in super::super) struct ConcealedMaterialVariants {
    handles: HashMap<AssetId<StandardMaterial>, Handle<StandardMaterial>>,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in super::super) struct StatusVisual3d(StatusKind);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum StatusKind {
    Slow,
    Knockback,
    Reveal,
    Cold,
    Frozen,
    Poison,
    Fire,
    TileSpeed,
    TileSlow,
    TileDamage,
}

fn effect_tile_status(tile: &crate::map::EffectTileOccupancy) -> Option<StatusKind> {
    tile.capabilities()
        .presentation
        .map(|presentation| match presentation {
            crate::map::EffectTilePresentation::Speed => StatusKind::TileSpeed,
            crate::map::EffectTilePresentation::Slow => StatusKind::TileSlow,
            crate::map::EffectTilePresentation::Damage => StatusKind::TileDamage,
        })
}

#[derive(Component)]
pub(in super::super) struct DashTrailVisual3d {
    origin_position: Vec2,
    last_position: Vec2,
    ends_at_tick: u64,
    dissipates_at_tick: u64,
    procedural: bool,
    reduced: bool,
}

#[derive(Component)]
pub(in super::super) struct DashTrailMaterialAsset(Handle<DashTrailMaterial>);

#[derive(Component, Clone, Copy)]
pub(in super::super) struct DashTrailLink(Entity);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DashTrailAction {
    Spawn,
    Update(Entity),
    Dissipate(Entity),
    Remove(Entity),
    ClearStaleLink,
    None,
}

type StatusFighterQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        &'static NetworkEntityId,
        &'static crate::combat::TeamId,
        Option<&'static crate::combat::Defeated>,
        Option<&'static AuthoritativeTick>,
        Option<&'static crate::combat::ActiveEffects>,
        Option<&'static crate::combat::KnockbackFeedback>,
        Option<&'static crate::concealment::ConcealmentPresentationState>,
        Option<&'static crate::map::EffectTileOccupancy>,
    ),
    With<Fighter>,
>;

type DashFighterQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        &'static NetworkEntityId,
        &'static crate::combat::TeamId,
        &'static crate::builds::ResolvedMatchLoadout,
        Option<&'static AuthoritativeTick>,
        Option<&'static crate::builds::AbilityState>,
        Option<&'static DashTrailLink>,
    ),
    (
        With<Fighter>,
        Or<(
            Changed<Position>,
            Changed<crate::builds::AbilityState>,
            Changed<AuthoritativeTick>,
            Added<DashTrailLink>,
        )>,
    ),
>;

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "the presentation pass discovers material-bearing descendants across imported and fallback fighter hierarchies"
)]
pub(in super::super) fn update_fighter_concealment_visuals(
    mut commands: Commands,
    roots: Query<(Entity, &CombatVisualOwner), With<V3FighterVisual>>,
    children: Query<&Children>,
    fighters: Query<
        (
            Option<&crate::concealment::ConcealmentPresentationState>,
            Option<&AuthoritativeTick>,
            &crate::combat::TeamId,
            Has<Controlled>,
        ),
        With<Fighter>,
    >,
    mut body_materials: Query<
        (
            Entity,
            &mut MeshMaterial3d<StandardMaterial>,
            Option<&FighterConcealmentMaterial>,
        ),
        Without<FighterGroundMarker3d>,
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut variants: ResMut<ConcealedMaterialVariants>,
) {
    let controlled_team = fighters
        .iter()
        .find_map(|(_, _, team, controlled)| controlled.then_some(*team));
    for (root, owner) in &roots {
        let Ok((concealment, tick, team, _)) = fighters.get(owner.0) else {
            continue;
        };
        let concealed = controlled_team == Some(*team)
            && fighter_is_visually_concealed(
                concealment,
                tick.map_or(0, |authoritative_tick| authoritative_tick.0),
                controlled_team,
            );
        for descendant in children.iter_descendants(root) {
            let Ok((entity, mut material, binding)) = body_materials.get_mut(descendant) else {
                continue;
            };
            if let Some(binding) = binding {
                let desired = if concealed {
                    &binding.concealed
                } else {
                    &binding.normal
                };
                if material.0 != *desired {
                    material.0 = desired.clone();
                }
                continue;
            }

            let normal = material.0.clone();
            let Some(concealed_material) =
                concealed_material_variant(&normal, &mut materials, &mut variants)
            else {
                continue;
            };
            if concealed {
                material.0 = concealed_material.clone();
            }
            commands.entity(entity).insert(FighterConcealmentMaterial {
                normal,
                concealed: concealed_material,
            });
        }
    }
}

fn fighter_is_visually_concealed(
    state: Option<&crate::concealment::ConcealmentPresentationState>,
    current_tick: u64,
    observer_team: Option<crate::combat::TeamId>,
) -> bool {
    state.is_some_and(|state| {
        (state.inside_concealing_terrain
            || state.inside_allied_concealment_field
            || current_tick < state.self_cloaked_until_tick)
            && current_tick >= state.revealed_until_tick
            && !state.forced_reveals.iter().any(|reveal| {
                Some(reveal.team) == observer_team && current_tick < reveal.expires_at_tick
            })
    })
}

fn concealed_material_variant(
    source: &Handle<StandardMaterial>,
    materials: &mut Assets<StandardMaterial>,
    variants: &mut ConcealedMaterialVariants,
) -> Option<Handle<StandardMaterial>> {
    if let Some(handle) = variants.handles.get(&source.id()) {
        return Some(handle.clone());
    }
    let mut material = materials.get(source)?.clone();
    let color = material.base_color.to_srgba();
    material.base_color = Color::srgba(
        color.red,
        color.green,
        color.blue,
        color.alpha * CONCEALED_FIGHTER_ALPHA,
    );
    material.alpha_mode = AlphaMode::Blend;
    let handle = materials.add(material);
    variants.handles.insert(source.id(), handle.clone());
    Some(handle)
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "durable status reconciliation reads the authoritative status sources and owns only status visuals"
)]
pub(in super::super) fn reconcile_status_visuals(
    mut commands: Commands,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    vfx: CombatVfxContext,
    fighters: StatusFighterQuery,
    statuses: Query<(Entity, &CombatVisualOwner, &StatusVisual3d)>,
) {
    let mut desired_status = HashMap::new();
    for (
        entity,
        position,
        network_id,
        team,
        defeated,
        authoritative_tick,
        effects,
        knockback,
        concealment,
        tile,
    ) in &fighters
    {
        if defeated.is_some() {
            continue;
        }
        if effects.is_some_and(|value| {
            value.slow.is_some_and(|slow| {
                authoritative_tick.is_none_or(|now| now.0 < slow.expires_at_tick)
            })
        }) {
            desired_status.insert(
                (entity, StatusKind::Slow),
                (
                    position.0,
                    status_signature(
                        StatusKind::Slow,
                        position.0,
                        *network_id,
                        *team,
                        authoritative_tick,
                        effects,
                        knockback,
                        concealment,
                        tile,
                        &vfx,
                    ),
                ),
            );
        }
        if knockback.is_some() {
            desired_status.insert(
                (entity, StatusKind::Knockback),
                (
                    position.0,
                    status_signature(
                        StatusKind::Knockback,
                        position.0,
                        *network_id,
                        *team,
                        authoritative_tick,
                        effects,
                        knockback,
                        concealment,
                        tile,
                        &vfx,
                    ),
                ),
            );
        }
        if effects
            .is_some_and(|value| authoritative_tick.is_some_and(|now| value.is_poisoned(now.0)))
        {
            desired_status.insert(
                (entity, StatusKind::Poison),
                (
                    position.0,
                    status_signature(
                        StatusKind::Poison,
                        position.0,
                        *network_id,
                        *team,
                        authoritative_tick,
                        effects,
                        knockback,
                        concealment,
                        tile,
                        &vfx,
                    ),
                ),
            );
        }
        if effects.is_some_and(|value| value.cold.meter > 0) {
            desired_status.insert(
                (entity, StatusKind::Cold),
                (
                    position.0,
                    status_signature(
                        StatusKind::Cold,
                        position.0,
                        *network_id,
                        *team,
                        authoritative_tick,
                        effects,
                        knockback,
                        concealment,
                        tile,
                        &vfx,
                    ),
                ),
            );
        }
        if effects.is_some_and(|value| authoritative_tick.is_some_and(|now| value.is_frozen(now.0)))
        {
            desired_status.remove(&(entity, StatusKind::Cold));
            desired_status.insert(
                (entity, StatusKind::Frozen),
                (
                    position.0,
                    status_signature(
                        StatusKind::Frozen,
                        position.0,
                        *network_id,
                        *team,
                        authoritative_tick,
                        effects,
                        knockback,
                        concealment,
                        tile,
                        &vfx,
                    ),
                ),
            );
        }
        if effects.is_some_and(|value| {
            value.fire.is_some_and(|fire| {
                authoritative_tick.is_some_and(|now| now.0 <= fire.expires_at_tick)
            })
        }) {
            desired_status.insert(
                (entity, StatusKind::Fire),
                (
                    position.0,
                    status_signature(
                        StatusKind::Fire,
                        position.0,
                        *network_id,
                        *team,
                        authoritative_tick,
                        effects,
                        knockback,
                        concealment,
                        tile,
                        &vfx,
                    ),
                ),
            );
        }
        // Forced reveal is a public status wherever the subject is otherwise legally present.
        // Observer-specific entity visibility still prevents this marker leaking a hidden target.
        if concealment
            .zip(authoritative_tick)
            .is_some_and(|(state, now)| {
                state
                    .forced_reveals
                    .iter()
                    .any(|reveal| now.0 < reveal.expires_at_tick)
            })
        {
            desired_status.insert(
                (entity, StatusKind::Reveal),
                (
                    position.0,
                    status_signature(
                        StatusKind::Reveal,
                        position.0,
                        *network_id,
                        *team,
                        authoritative_tick,
                        effects,
                        knockback,
                        concealment,
                        tile,
                        &vfx,
                    ),
                ),
            );
        }
        if let Some(tile) = tile
            && let Some(kind) = effect_tile_status(tile)
        {
            desired_status.insert(
                (entity, kind),
                (
                    position.0,
                    status_signature(
                        kind,
                        position.0,
                        *network_id,
                        *team,
                        authoritative_tick,
                        effects,
                        knockback,
                        concealment,
                        Some(tile),
                        &vfx,
                    ),
                ),
            );
        }
    }

    let existing_status: HashSet<_> = statuses
        .iter()
        .map(|(_, owner, kind)| (owner.0, kind.0))
        .collect();
    for (entity, owner, kind) in &statuses {
        if !desired_status.contains_key(&(owner.0, kind.0)) {
            commands.entity(entity).despawn();
        }
    }
    for (&(owner, kind), &(position, signature)) in &desired_status {
        if existing_status.contains(&(owner, kind)) {
            continue;
        }
        if kind.is_effect_tile() {
            commands.spawn((
                CombatVisualOwner(owner),
                StatusVisual3d(kind),
                signature,
                Mesh3d(primitives.ground_ring.clone()),
                MeshMaterial3d(status_material(kind, &materials)),
                NotShadowCaster,
                NotShadowReceiver,
                Transform {
                    translation: ground_position(position) + Vec3::Y * 3.0,
                    rotation: Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2),
                    scale: Vec3::splat(status_scale(kind)),
                },
                Name::new("V3 durable effect-tile status"),
            ));
            continue;
        }

        let root = commands
            .spawn((
                CombatVisualOwner(owner),
                StatusVisual3d(kind),
                signature,
                Transform::from_translation(
                    ground_position(position)
                        + Vec3::Y * if kind == StatusKind::Slow { 2.0 } else { 3.0 },
                ),
                Visibility::default(),
                Name::new("V3 layered attached combat status"),
            ))
            .id();
        commands.entity(root).with_children(|parent| {
            super::status_style::spawn_status_style(
                parent,
                &primitives,
                &materials,
                owner,
                kind,
                signature,
            );
        });
    }
}

pub(super) fn status_material(
    kind: StatusKind,
    materials: &Material3dAssets,
) -> Handle<StandardMaterial> {
    match kind {
        StatusKind::Slow | StatusKind::TileSlow => materials.status_slow.clone(),
        StatusKind::Knockback => materials.status_knockback.clone(),
        StatusKind::Reveal => materials.status_reveal.clone(),
        StatusKind::Cold | StatusKind::Frozen => materials.elemental_cold.clone(),
        StatusKind::Poison => materials.status_poison.clone(),
        StatusKind::Fire => materials.status_fire.clone(),
        StatusKind::TileSpeed => materials.dash.clone(),
        StatusKind::TileDamage => materials.effect_damage.clone(),
    }
}

pub(super) const fn status_scale(kind: StatusKind) -> f32 {
    match kind {
        StatusKind::Slow => 1.15,
        StatusKind::Knockback => 0.8,
        StatusKind::Reveal => 1.8,
        StatusKind::Cold => 1.3,
        StatusKind::Frozen => 1.9,
        StatusKind::Poison => 1.35,
        StatusKind::Fire => 1.55,
        StatusKind::TileSpeed | StatusKind::TileSlow | StatusKind::TileDamage => 1.45,
    }
}

impl StatusKind {
    const fn is_effect_tile(self) -> bool {
        matches!(self, Self::TileSpeed | Self::TileSlow | Self::TileDamage)
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "status signatures snapshot the same authoritative facts used by status reconciliation"
)]
fn status_signature(
    kind: StatusKind,
    position: Vec2,
    network_id: NetworkEntityId,
    team: crate::combat::TeamId,
    authoritative_tick: Option<&AuthoritativeTick>,
    effects: Option<&crate::combat::ActiveEffects>,
    knockback: Option<&crate::combat::KnockbackFeedback>,
    concealment: Option<&crate::concealment::ConcealmentPresentationState>,
    tile: Option<&crate::map::EffectTileOccupancy>,
    vfx: &CombatVfxContext,
) -> vfx::CombatVisualSignature {
    let observed_at_tick = authoritative_tick.map(|tick| tick.0);
    let (attack_id, expires_at_tick) = match kind {
        StatusKind::Slow => effects
            .and_then(|effects| effects.slow)
            .map_or((None, None), |slow| {
                (Some(slow.source_attack_id), Some(slow.expires_at_tick))
            }),
        StatusKind::Cold | StatusKind::Frozen => effects.map_or((None, None), |effects| {
            (
                effects.cold.source.map(|source| source.action_id),
                effects.cold.frozen_until_tick,
            )
        }),
        StatusKind::Poison => {
            effects
                .and_then(|effects| effects.poison)
                .map_or((None, None), |condition| {
                    (
                        Some(condition.source.action_id),
                        Some(condition.expires_at_tick),
                    )
                })
        }
        StatusKind::Fire => {
            effects
                .and_then(|effects| effects.fire)
                .map_or((None, None), |condition| {
                    (
                        Some(condition.source.action_id),
                        Some(condition.expires_at_tick),
                    )
                })
        }
        StatusKind::Knockback => (None, knockback.map(|feedback| feedback.expires_at_tick)),
        StatusKind::Reveal => (
            None,
            concealment.and_then(|state| {
                state
                    .forced_reveals
                    .iter()
                    .map(|reveal| reveal.expires_at_tick)
                    .max()
            }),
        ),
        StatusKind::TileSpeed | StatusKind::TileSlow | StatusKind::TileDamage => (None, None),
    };
    let timing = vfx::VfxAuthoritativeTiming {
        activated_at_tick: tile.map(|tile| tile.entered_at_tick),
        observed_at_tick,
        expires_at_tick,
    };
    let geometry = vfx::VfxGeometry::point(position);
    let mut signature = attack_id.map_or_else(
        || vfx::CombatVisualSignature {
            carrier: vfx::VfxCarrier::AttachedStatus,
            phase: vfx::VfxPhase::Active,
            accents: [Some(status_accent(kind)), None],
            relationship: vfx.cache().observer().relationship(network_id, team),
            context: (kind == StatusKind::Reveal).then_some(vfx::VfxContextOverride::RevealScan),
            geometry,
            identity: vfx::VfxPresentationIdentity::NetworkEntity {
                network_id: network_id.0,
                generation: expires_at_tick
                    .or_else(|| tile.map(|tile| tile.entered_at_tick))
                    .unwrap_or_default(),
            },
            timing,
            variant: vfx.variant(),
        },
        |attack_id| {
            vfx.cache().signature_for_attack(
                attack_id,
                0,
                vfx::VfxCarrier::AttachedStatus,
                vfx::VfxPhase::Active,
                geometry,
                timing,
                vfx.variant(),
            )
        },
    );
    signature.carrier = vfx::VfxCarrier::AttachedStatus;
    signature.accents = [Some(status_accent(kind)), None];
    signature.relationship = vfx.cache().observer().relationship(network_id, team);
    signature
}

const fn status_accent(kind: StatusKind) -> vfx::VfxAccent {
    match kind {
        StatusKind::Slow | StatusKind::Cold | StatusKind::Frozen | StatusKind::TileSlow => {
            vfx::VfxAccent::Cold
        }
        StatusKind::Poison => vfx::VfxAccent::Poison,
        StatusKind::Fire | StatusKind::TileDamage => vfx::VfxAccent::Fire,
        StatusKind::Reveal => vfx::VfxAccent::Reveal,
        StatusKind::Knockback | StatusKind::TileSpeed => vfx::VfxAccent::Neutral,
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "dash presentation owns the direct fighter-to-trail lifecycle"
)]
pub(in super::super) fn reconcile_dash_trails(
    mut commands: Commands,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    mut dash_materials: ResMut<Assets<DashTrailMaterial>>,
    vfx: CombatVfxContext,
    fighters: DashFighterQuery,
    mut trails: Query<(&mut DashTrailVisual3d, &mut Transform)>,
    orphan_trails: Query<(Entity, &CombatVisualOwner), With<DashTrailVisual3d>>,
    material_handles: Query<&DashTrailMaterialAsset>,
    fighter_owners: Query<(), With<Fighter>>,
) {
    for (fighter, position, network_id, team, loadout, authoritative_tick, ability, link) in
        &fighters
    {
        let dashing = ability.is_some_and(|value| {
            matches!(value.phase, crate::builds::AbilityPhase::Dashing { .. })
        });
        let current_tick = authoritative_tick.map_or(0, |tick| tick.0);
        let linked_trail = link.map(|link| link.0);
        let linked_trail_expired = linked_trail
            .and_then(|trail| trails.get(trail).ok())
            .is_some_and(|(trail, _)| current_tick >= trail.dissipates_at_tick);
        match dash_trail_action(
            dashing,
            linked_trail,
            linked_trail.is_some_and(|trail| trails.contains(trail)),
            linked_trail_expired,
        ) {
            DashTrailAction::Update(trail_entity) => {
                let Ok((mut trail, mut transform)) = trails.get_mut(trail_entity) else {
                    continue;
                };
                let reduced = trail.reduced;
                update_dash_trail_geometry(&mut trail, &mut transform, position.0, reduced);
                update_dash_trail_material(
                    material_handles.get(trail_entity).ok(),
                    &mut dash_materials,
                    current_tick,
                    1.0,
                );
            }
            DashTrailAction::Spawn => {
                let ends_at_tick = ability.and_then(|ability| match ability.phase {
                    crate::builds::AbilityPhase::Dashing { ends_at_tick } => Some(ends_at_tick),
                    _ => None,
                });
                let signature = vfx::CombatVisualSignature {
                    carrier: vfx::VfxCarrier::AttachedStatus,
                    phase: vfx::VfxPhase::Travel,
                    accents: [Some(vfx::VfxAccent::Neutral), None],
                    relationship: vfx.cache().observer().relationship(*network_id, *team),
                    context: Some(vfx::VfxContextOverride::Ultimate(loadout.ultimate.id)),
                    geometry: vfx::VfxGeometry::Point {
                        position: position.0.to_array(),
                        normal: None,
                        direction: None,
                    },
                    identity: vfx::VfxPresentationIdentity::NetworkEntity {
                        network_id: network_id.0,
                        generation: ends_at_tick.unwrap_or_default(),
                    },
                    timing: vfx::VfxAuthoritativeTiming {
                        activated_at_tick: None,
                        observed_at_tick: authoritative_tick.map(|tick| tick.0),
                        expires_at_tick: ends_at_tick,
                    },
                    variant: vfx.variant(),
                };
                let trail = spawn_dash_trail(
                    &mut commands,
                    &primitives,
                    &materials,
                    fighter,
                    position.0,
                    ends_at_tick.unwrap_or(current_tick),
                    signature,
                    &mut dash_materials,
                );
                commands.entity(fighter).insert(DashTrailLink(trail));
            }
            DashTrailAction::Dissipate(trail_entity) => {
                let Ok((mut trail, mut transform)) = trails.get_mut(trail_entity) else {
                    continue;
                };
                let Some(material) = material_handles.get(trail_entity).ok() else {
                    commands.entity(trail_entity).despawn();
                    commands.entity(fighter).remove::<DashTrailLink>();
                    continue;
                };
                let reduced = trail.reduced;
                update_dash_trail_geometry(&mut trail, &mut transform, position.0, reduced);
                update_dash_trail_material(
                    Some(material),
                    &mut dash_materials,
                    current_tick,
                    dash_trail_opacity(current_tick, trail.ends_at_tick, trail.dissipates_at_tick),
                );
            }
            DashTrailAction::Remove(trail) => {
                remove_dash_material(trail, &material_handles, &mut dash_materials);
                commands.entity(trail).despawn();
                commands.entity(fighter).remove::<DashTrailLink>();
            }
            DashTrailAction::ClearStaleLink => {
                commands.entity(fighter).remove::<DashTrailLink>();
            }
            DashTrailAction::None => {}
        }
    }
    for (trail, owner) in &orphan_trails {
        if fighter_owners.get(owner.0).is_err() {
            remove_dash_material(trail, &material_handles, &mut dash_materials);
            commands.entity(trail).despawn();
        }
    }
}

fn dash_trail_action(
    dashing: bool,
    linked_trail: Option<Entity>,
    linked_trail_exists: bool,
    linked_trail_expired: bool,
) -> DashTrailAction {
    match (
        dashing,
        linked_trail,
        linked_trail_exists,
        linked_trail_expired,
    ) {
        (true, Some(trail), true, _) => DashTrailAction::Update(trail),
        (true, _, _, _) => DashTrailAction::Spawn,
        (false, Some(trail), true, false) => DashTrailAction::Dissipate(trail),
        (false, Some(trail), true, true) => DashTrailAction::Remove(trail),
        (false, Some(_), false, _) => DashTrailAction::ClearStaleLink,
        (false, None, _, _) => DashTrailAction::None,
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the helper receives the complete immutable Dash presentation profile and asset stores"
)]
fn spawn_dash_trail(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    fighter: Entity,
    position: Vec2,
    ends_at_tick: u64,
    signature: vfx::CombatVisualSignature,
    dash_materials: &mut Assets<DashTrailMaterial>,
) -> Entity {
    let quality = quality_for_variant(signature.variant);
    let material = quality.map(|quality| dash_materials.add(DashTrailMaterial::new(quality)));
    let mut entity = commands.spawn((
        CombatVisualOwner(fighter),
        DashTrailVisual3d {
            origin_position: position,
            last_position: position,
            ends_at_tick,
            dissipates_at_tick: ends_at_tick.saturating_add(DASH_DISSIPATION_TICKS),
            procedural: quality.is_some(),
            reduced: quality == Some(DashTrailQuality::Reduced),
        },
        signature,
        NotShadowCaster,
        NotShadowReceiver,
        Transform::from_translation(ground_position(position) + Vec3::Y * 3.0),
        Name::new("V3 dash trail"),
    ));
    if let Some(material) = material {
        entity.insert((
            DashTrailMaterialAsset(material.clone()),
            Mesh3d(primitives.projectile_trail.clone()),
            MeshMaterial3d(material),
        ));
    } else {
        entity.insert((
            Mesh3d(primitives.unit_cuboid.clone()),
            MeshMaterial3d(materials.dash.clone()),
        ));
    }
    entity.id()
}

fn update_dash_trail_geometry(
    trail: &mut DashTrailVisual3d,
    transform: &mut Transform,
    position: Vec2,
    reduced: bool,
) {
    let delta = position
        - if trail.procedural {
            trail.origin_position
        } else {
            trail.last_position
        };
    if delta.length_squared() <= f32::EPSILON {
        return;
    }
    let direction = delta.normalize();
    let rotation = Rotation::radians(delta.y.atan2(delta.x));
    if trail.procedural {
        let length_scale = delta.length() / 1.22;
        let head_offset = direction * (0.22 * length_scale);
        transform.translation = ground_position(position - head_offset) + Vec3::Y * 2.4;
        transform.rotation = ground_rotation(rotation);
        transform.scale = Vec3::new(
            length_scale.max(1.0),
            1.0,
            if reduced {
                DASH_WAKE_REDUCED_WIDTH
            } else {
                DASH_WAKE_WIDTH
            },
        );
    } else {
        transform.translation =
            ground_position(trail.last_position.midpoint(position)) + Vec3::Y * 3.0;
        transform.rotation = ground_rotation(rotation);
        transform.scale = Vec3::new(delta.length().max(2.0), 3.0, 12.0);
    }
    trail.last_position = position;
}

fn update_dash_trail_material(
    handle: Option<&DashTrailMaterialAsset>,
    materials: &mut Assets<DashTrailMaterial>,
    current_tick: u64,
    opacity: f32,
) {
    let Some(handle) = handle else {
        return;
    };
    if let Some(mut material) = materials.get_mut(&handle.0) {
        let cycle_ticks = crate::timing::SIMULATION_TICK_HZ * 900;
        let visual_tick = u16::try_from(current_tick % cycle_ticks)
            .expect("the fifteen-minute dash visual cycle fits u16");
        let tick_hz = u16::try_from(crate::timing::SIMULATION_TICK_HZ)
            .expect("the simulation tick rate fits u16");
        material.set_time_and_opacity(f32::from(visual_tick) / f32::from(tick_hz), opacity);
    }
}

fn dash_trail_opacity(current_tick: u64, ends_at_tick: u64, dissipates_at_tick: u64) -> f32 {
    if current_tick <= ends_at_tick {
        return 1.0;
    }
    let duration = dissipates_at_tick.saturating_sub(ends_at_tick).max(1);
    let elapsed = current_tick.saturating_sub(ends_at_tick).min(duration);
    let duration = u16::try_from(duration).expect("bounded dash dissipation fits u16");
    let elapsed = u16::try_from(elapsed).expect("bounded dash dissipation fits u16");
    1.0 - f32::from(elapsed) / f32::from(duration)
}

fn remove_dash_material(
    entity: Entity,
    handles: &Query<&DashTrailMaterialAsset>,
    materials: &mut Assets<DashTrailMaterial>,
) {
    if let Ok(handle) = handles.get(entity) {
        materials.remove(handle.0.id());
    }
}

pub(in super::super) fn write_status_visual_poses(
    fighter_owners: Query<&Position, With<Fighter>>,
    mut status_visuals: Query<(&CombatVisualOwner, &mut Transform), With<StatusVisual3d>>,
) {
    for (owner, mut transform) in &mut status_visuals {
        if let Ok(position) = fighter_owners.get(owner.0) {
            let height = transform.translation.y;
            transform.translation = ground_position(position.0) + Vec3::Y * height;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terrain_concealed_state(
        revealed_until_tick: u64,
    ) -> crate::concealment::ConcealmentPresentationState {
        crate::concealment::ConcealmentPresentationState {
            inside_concealing_terrain: true,
            revealed_until_tick,
            ..default()
        }
    }

    fn tile_occupancy(
        behavior: crate::map::MapEffectTileBehavior,
    ) -> crate::map::EffectTileOccupancy {
        crate::map::EffectTileOccupancy {
            generation: crate::map::MapDynamicGeneration {
                map_instance_id: crate::map::MapInstanceId(1),
                generation: 1,
            },
            placement_id: crate::map::MapPlacementId(1),
            behavior,
            entered_at_tick: 0,
            next_pulse_at_tick: None,
        }
    }

    #[test]
    fn effect_tile_feedback_uses_the_focused_presentation_capability() {
        assert_eq!(
            effect_tile_status(&tile_occupancy(crate::map::MapEffectTileBehavior::Speed {
                movement_multiplier_milli: 1_250,
            })),
            Some(StatusKind::TileSpeed)
        );
        assert_eq!(
            effect_tile_status(&tile_occupancy(crate::map::MapEffectTileBehavior::Slow {
                movement_multiplier_milli: 700,
            })),
            Some(StatusKind::TileSlow)
        );
        assert_eq!(
            effect_tile_status(&tile_occupancy(crate::map::MapEffectTileBehavior::Damage {
                damage: 10,
                interval_ticks: 30,
            })),
            Some(StatusKind::TileDamage)
        );
        assert_eq!(
            effect_tile_status(&tile_occupancy(crate::map::MapEffectTileBehavior::None)),
            None
        );
    }

    #[test]
    fn fighter_alpha_signifier_tracks_concealment_and_exclusive_reveal_deadline() {
        let concealed = terrain_concealed_state(12);
        assert!(!fighter_is_visually_concealed(
            Some(&concealed),
            11,
            Some(crate::combat::TeamId(1))
        ));
        assert!(fighter_is_visually_concealed(
            Some(&concealed),
            12,
            Some(crate::combat::TeamId(1))
        ));
        assert!(!fighter_is_visually_concealed(
            None,
            12,
            Some(crate::combat::TeamId(1))
        ));
    }

    #[test]
    fn concealed_material_preserves_color_and_blends_at_bounded_alpha() {
        let mut materials = Assets::<StandardMaterial>::default();
        let source = materials.add(StandardMaterial {
            base_color: Color::srgba(0.2, 0.4, 0.8, 0.9),
            ..default()
        });
        let mut variants = ConcealedMaterialVariants::default();
        let concealed = concealed_material_variant(&source, &mut materials, &mut variants).unwrap();
        let color = materials.get(&concealed).unwrap().base_color.to_srgba();
        assert!((color.red - 0.2).abs() < f32::EPSILON);
        assert!((color.green - 0.4).abs() < f32::EPSILON);
        assert!((color.blue - 0.8).abs() < f32::EPSILON);
        assert!((color.alpha - 0.9 * CONCEALED_FIGHTER_ALPHA).abs() < f32::EPSILON);
        assert_eq!(
            materials.get(&concealed).unwrap().alpha_mode,
            AlphaMode::Blend
        );
        assert_eq!(
            concealed_material_variant(&source, &mut materials, &mut variants).unwrap(),
            concealed
        );
    }

    #[test]
    fn dash_trail_lifecycle_uses_the_direct_link_and_repairs_stale_links() {
        let trail = Entity::from_raw_u32(7).expect("valid trail entity");
        assert_eq!(
            dash_trail_action(true, Some(trail), true, false),
            DashTrailAction::Update(trail)
        );
        assert_eq!(
            dash_trail_action(false, Some(trail), true, false),
            DashTrailAction::Dissipate(trail)
        );
        assert_eq!(
            dash_trail_action(false, Some(trail), true, true),
            DashTrailAction::Remove(trail)
        );
        assert_eq!(
            dash_trail_action(true, Some(trail), false, false),
            DashTrailAction::Spawn
        );
        assert_eq!(
            dash_trail_action(false, Some(trail), false, false),
            DashTrailAction::ClearStaleLink
        );
        assert_eq!(
            dash_trail_action(false, None, false, false),
            DashTrailAction::None
        );
    }

    #[test]
    fn dash_dissipation_is_authoritative_tick_driven_and_bounded() {
        assert!((dash_trail_opacity(100, 100, 115) - 1.0).abs() < f32::EPSILON);
        assert!((dash_trail_opacity(107, 100, 115) - 8.0 / 15.0).abs() < f32::EPSILON);
        assert!(dash_trail_opacity(115, 100, 115).abs() < f32::EPSILON);
        assert!(dash_trail_opacity(200, 100, 115).abs() < f32::EPSILON);
    }

    #[test]
    fn dash_material_cleanup_releases_the_per_instance_asset() {
        let mut world = World::new();
        world.init_resource::<Assets<DashTrailMaterial>>();
        let handle = world
            .resource_mut::<Assets<DashTrailMaterial>>()
            .add(DashTrailMaterial::new(DashTrailQuality::Normal));
        let entity = world.spawn(DashTrailMaterialAsset(handle)).id();
        let mut state = bevy::ecs::system::SystemState::<(
            Query<&DashTrailMaterialAsset>,
            ResMut<Assets<DashTrailMaterial>>,
        )>::new(&mut world);
        let (handles, mut materials) = state
            .get_mut(&mut world)
            .expect("Dash cleanup test system state is valid");
        assert_eq!(materials.len(), 1);
        remove_dash_material(entity, &handles, &mut materials);
        assert!(materials.is_empty());
    }

    #[test]
    fn fighter_body_restores_its_exact_material_during_reveal() {
        let mut app = App::new();
        app.init_resource::<Assets<StandardMaterial>>()
            .init_resource::<ConcealedMaterialVariants>()
            .add_systems(Update, update_fighter_concealment_visuals);
        let normal = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let owner = app
            .world_mut()
            .spawn((
                Fighter,
                terrain_concealed_state(0),
                AuthoritativeTick(8),
                crate::combat::TeamId(1),
                Controlled,
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
            ))
            .id();
        let body = app.world_mut().spawn(MeshMaterial3d(normal.clone())).id();
        app.world_mut().entity_mut(root).add_child(body);
        let enemy = app
            .world_mut()
            .spawn((
                Fighter,
                terrain_concealed_state(0),
                AuthoritativeTick(8),
                crate::combat::TeamId(2),
            ))
            .id();
        let enemy_root = app
            .world_mut()
            .spawn((
                CombatVisualOwner(enemy),
                V3FighterVisual {
                    last_position: Vec2::ZERO,
                    moving: false,
                    shoot_seconds: 0.0,
                },
            ))
            .id();
        let enemy_body = app.world_mut().spawn(MeshMaterial3d(normal.clone())).id();
        app.world_mut().entity_mut(enemy_root).add_child(enemy_body);

        app.update();
        let concealed = app
            .world()
            .get::<MeshMaterial3d<StandardMaterial>>(body)
            .unwrap()
            .0
            .clone();
        assert_ne!(concealed, normal);
        assert_eq!(
            app.world()
                .get::<MeshMaterial3d<StandardMaterial>>(enemy_body)
                .unwrap()
                .0,
            normal,
            "a proximity-revealed enemy must not look locally concealed"
        );

        app.world_mut()
            .get_mut::<crate::concealment::ConcealmentPresentationState>(owner)
            .unwrap()
            .revealed_until_tick = 10;
        app.update();
        assert_eq!(
            app.world()
                .get::<MeshMaterial3d<StandardMaterial>>(body)
                .unwrap()
                .0,
            normal
        );

        app.world_mut()
            .get_mut::<AuthoritativeTick>(owner)
            .unwrap()
            .0 = 10;
        app.update();
        assert_eq!(
            app.world()
                .get::<MeshMaterial3d<StandardMaterial>>(body)
                .unwrap()
                .0,
            concealed
        );
    }
}
