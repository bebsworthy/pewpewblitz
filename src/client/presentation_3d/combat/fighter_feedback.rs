use super::super::*;
use super::entities::FighterGroundMarker3d;
use std::collections::{HashMap, HashSet};

const CONCEALED_FIGHTER_ALPHA: f32 = 0.52;

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
enum StatusKind {
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

#[derive(Component)]
pub(in super::super) struct DashTrailVisual3d {
    last_position: Vec2,
}

#[derive(Component, Clone, Copy)]
pub(in super::super) struct DashTrailLink(Entity);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DashTrailAction {
    Spawn,
    Update(Entity),
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
        Option<&'static crate::builds::AbilityState>,
        Option<&'static DashTrailLink>,
    ),
    (
        With<Fighter>,
        Or<(
            Changed<Position>,
            Changed<crate::builds::AbilityState>,
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
    reason = "durable status reconciliation reads the authoritative status sources and owns only status visuals"
)]
pub(in super::super) fn reconcile_status_visuals(
    mut commands: Commands,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    fighters: StatusFighterQuery,
    statuses: Query<(Entity, &CombatVisualOwner, &StatusVisual3d)>,
) {
    let mut desired_status = HashMap::new();
    for (entity, position, defeated, authoritative_tick, effects, knockback, concealment, tile) in
        &fighters
    {
        if defeated.is_some() {
            continue;
        }
        if effects.is_some_and(|value| {
            value.slow.is_some_and(|slow| {
                authoritative_tick.is_none_or(|now| now.0 < slow.expires_at_tick)
            })
        }) {
            desired_status.insert((entity, StatusKind::Slow), position.0);
        }
        if knockback.is_some() {
            desired_status.insert((entity, StatusKind::Knockback), position.0);
        }
        if effects
            .is_some_and(|value| authoritative_tick.is_some_and(|now| value.is_poisoned(now.0)))
        {
            desired_status.insert((entity, StatusKind::Poison), position.0);
        }
        if effects.is_some_and(|value| value.cold.meter > 0) {
            desired_status.insert((entity, StatusKind::Cold), position.0);
        }
        if effects.is_some_and(|value| authoritative_tick.is_some_and(|now| value.is_frozen(now.0)))
        {
            desired_status.remove(&(entity, StatusKind::Cold));
            desired_status.insert((entity, StatusKind::Frozen), position.0);
        }
        if effects.is_some_and(|value| {
            value.fire.is_some_and(|fire| {
                authoritative_tick.is_some_and(|now| now.0 <= fire.expires_at_tick)
            })
        }) {
            desired_status.insert((entity, StatusKind::Fire), position.0);
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
            desired_status.insert((entity, StatusKind::Reveal), position.0);
        }
        if let Some(tile) = tile {
            let kind = match tile.kind {
                crate::map::EffectTileKind::Speed => StatusKind::TileSpeed,
                crate::map::EffectTileKind::Slow => StatusKind::TileSlow,
                crate::map::EffectTileKind::Damage => StatusKind::TileDamage,
            };
            desired_status.insert((entity, kind), position.0);
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
    for (&(owner, kind), &position) in &desired_status {
        if existing_status.contains(&(owner, kind)) {
            continue;
        }
        commands.spawn((
            CombatVisualOwner(owner),
            StatusVisual3d(kind),
            Mesh3d(primitives.ground_ring.clone()),
            MeshMaterial3d(status_material(kind, &materials)),
            NotShadowCaster,
            NotShadowReceiver,
            Transform {
                translation: ground_position(position)
                    + Vec3::Y * if kind == StatusKind::Slow { 2.0 } else { 3.0 },
                rotation: Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2),
                scale: Vec3::splat(status_scale(kind)),
            },
            Name::new("V3 durable combat status"),
        ));
    }
}

fn status_material(kind: StatusKind, materials: &Material3dAssets) -> Handle<StandardMaterial> {
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

const fn status_scale(kind: StatusKind) -> f32 {
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

#[allow(
    clippy::needless_pass_by_value,
    reason = "dash presentation owns the direct fighter-to-trail lifecycle"
)]
pub(in super::super) fn reconcile_dash_trails(
    mut commands: Commands,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    fighters: DashFighterQuery,
    mut trails: Query<(&mut DashTrailVisual3d, &mut Transform)>,
    orphan_trails: Query<(Entity, &CombatVisualOwner), With<DashTrailVisual3d>>,
    fighter_owners: Query<(), With<Fighter>>,
) {
    for (fighter, position, ability, link) in &fighters {
        let dashing = ability.is_some_and(|value| {
            matches!(value.phase, crate::builds::AbilityPhase::Dashing { .. })
        });
        let linked_trail = link.map(|link| link.0);
        match dash_trail_action(
            dashing,
            linked_trail,
            linked_trail.is_some_and(|trail| trails.contains(trail)),
        ) {
            DashTrailAction::Update(trail_entity) => {
                let Ok((mut trail, mut transform)) = trails.get_mut(trail_entity) else {
                    continue;
                };
                let delta = position.0 - trail.last_position;
                if delta.length_squared() > f32::EPSILON {
                    transform.translation =
                        ground_position(trail.last_position.midpoint(position.0)) + Vec3::Y * 3.0;
                    transform.rotation = ground_rotation(Rotation::radians(delta.y.atan2(delta.x)));
                    transform.scale = Vec3::new(delta.length().max(2.0), 3.0, 12.0);
                    trail.last_position = position.0;
                }
            }
            DashTrailAction::Spawn => {
                let trail =
                    spawn_dash_trail(&mut commands, &primitives, &materials, fighter, position.0);
                commands.entity(fighter).insert(DashTrailLink(trail));
            }
            DashTrailAction::Remove(trail) => {
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
            commands.entity(trail).despawn();
        }
    }
}

fn dash_trail_action(
    dashing: bool,
    linked_trail: Option<Entity>,
    linked_trail_exists: bool,
) -> DashTrailAction {
    match (dashing, linked_trail, linked_trail_exists) {
        (true, Some(trail), true) => DashTrailAction::Update(trail),
        (true, _, _) => DashTrailAction::Spawn,
        (false, Some(trail), true) => DashTrailAction::Remove(trail),
        (false, Some(_), false) => DashTrailAction::ClearStaleLink,
        (false, None, _) => DashTrailAction::None,
    }
}

fn spawn_dash_trail(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    fighter: Entity,
    position: Vec2,
) -> Entity {
    commands
        .spawn((
            CombatVisualOwner(fighter),
            DashTrailVisual3d {
                last_position: position,
            },
            Mesh3d(primitives.unit_cuboid.clone()),
            MeshMaterial3d(materials.dash.clone()),
            NotShadowCaster,
            Transform::from_translation(ground_position(position) + Vec3::Y * 3.0),
            Name::new("V3 dash trail"),
        ))
        .id()
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
            dash_trail_action(true, Some(trail), true),
            DashTrailAction::Update(trail)
        );
        assert_eq!(
            dash_trail_action(false, Some(trail), true),
            DashTrailAction::Remove(trail)
        );
        assert_eq!(
            dash_trail_action(true, Some(trail), false),
            DashTrailAction::Spawn
        );
        assert_eq!(
            dash_trail_action(false, Some(trail), false),
            DashTrailAction::ClearStaleLink
        );
        assert_eq!(dash_trail_action(false, None, false), DashTrailAction::None);
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
