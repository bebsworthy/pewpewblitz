//! Complete 3D combat presentation over independent, client-only visual roots.

use super::*;
use crate::combat::client::{DeduplicatedCombatCue, MAX_PREVIEW_SEGMENTS, preview_segments};
use std::collections::{BTreeMap, HashMap, HashSet};

const PREVIEW_HEIGHT: f32 = 2.5;
const HEALTH_HEIGHT: f32 = 72.0;
const MAX_EFFECTS: usize = 96;

#[derive(Component)]
pub(super) struct SentryVisual3d;

#[derive(Component)]
pub(super) struct WorldHealthVisual3d {
    fill: Entity,
}

#[derive(Component)]
pub(super) struct HealthFill3d;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct StatusVisual3d(StatusKind);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum StatusKind {
    Slow,
    Knockback,
}

#[derive(Component)]
pub(super) struct DashTrailVisual3d {
    last_position: Vec2,
}

#[derive(Component)]
pub(super) struct WeaponPreviewVisual3d {
    slot: u8,
}

#[derive(Component)]
pub(super) struct CombatEffect3d {
    timer: Timer,
    order: u64,
}

#[derive(Resource, Default)]
pub(super) struct CombatEffectSequence(u64);

#[allow(
    clippy::cast_possible_truncation,
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "this reconciliation phase owns the complete set of independent durable visual families"
)]
pub(super) fn reconcile_combat_visuals(
    mut commands: Commands,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    fighters: Query<(Entity, &Position, &crate::combat::TeamId, Has<Controlled>), With<Fighter>>,
    projectiles: Query<
        (
            Entity,
            &Position,
            &crate::combat::ProjectileSource,
            Option<&crate::combat::StraightFlight>,
            Option<&crate::combat::LobbedFlight>,
        ),
        With<crate::combat::Projectile>,
    >,
    sentries: Query<
        (Entity, &Position, &crate::abilities::SentryIdentity),
        With<crate::abilities::Sentry>,
    >,
    fighter_visuals: Query<(Entity, &CombatVisualOwner), With<V3FighterVisual>>,
    projectile_visuals: Query<(Entity, &CombatVisualOwner), With<V3ProjectileVisual>>,
    sentry_visuals: Query<(Entity, &CombatVisualOwner), With<SentryVisual3d>>,
    health_visuals: Query<(Entity, &CombatVisualOwner), With<WorldHealthVisual3d>>,
    trails: Query<(Entity, &CombatVisualOwner), With<DashTrailVisual3d>>,
    statuses: Query<(Entity, &CombatVisualOwner), With<StatusVisual3d>>,
    previews: Query<&WeaponPreviewVisual3d>,
) {
    let fighter_roots = unique_roots(&mut commands, &fighter_visuals);
    let projectile_roots = unique_roots(&mut commands, &projectile_visuals);
    let sentry_roots = unique_roots(&mut commands, &sentry_visuals);
    let health_roots = unique_roots(&mut commands, &health_visuals);

    for (owner, position, team, controlled) in &fighters {
        if !fighter_roots.contains_key(&owner) {
            spawn_fighter(
                &mut commands,
                &primitives,
                &materials,
                owner,
                position.0,
                *team,
                controlled,
            );
        }
        if !health_roots.contains_key(&owner) {
            spawn_health(&mut commands, &primitives, &materials, owner, position.0);
        }
    }
    for (owner, position, source, straight, lobbed) in &projectiles {
        if !projectile_roots.contains_key(&owner) {
            spawn_projectile(
                &mut commands,
                &primitives,
                &materials,
                owner,
                position.0,
                source.team_id,
                straight,
                lobbed.is_some(),
            );
        }
    }
    for (owner, position, identity) in &sentries {
        if !sentry_roots.contains_key(&owner) {
            spawn_sentry(
                &mut commands,
                &primitives,
                &materials,
                owner,
                position.0,
                identity.team_id,
            );
        }
    }

    for (root, owner) in &fighter_visuals {
        if fighters.get(owner.0).is_err() {
            commands.entity(root).despawn();
        }
    }
    for (root, owner) in &projectile_visuals {
        if projectiles.get(owner.0).is_err() {
            commands.entity(root).despawn();
        }
    }
    for (root, owner) in &sentry_visuals {
        if sentries.get(owner.0).is_err() {
            commands.entity(root).despawn();
        }
    }
    for (root, owner) in &health_visuals {
        if fighters.get(owner.0).is_err() {
            commands.entity(root).despawn();
        }
    }
    for (root, owner) in &trails {
        if fighters.get(owner.0).is_err() {
            commands.entity(root).despawn();
        }
    }
    for (root, owner) in &statuses {
        if fighters.get(owner.0).is_err() {
            commands.entity(root).despawn();
        }
    }

    if previews.iter().count() == 0 {
        for slot in 0..u8::try_from(MAX_PREVIEW_SEGMENTS).expect("preview slot bound fits u8") {
            commands.spawn((
                WeaponPreviewVisual3d { slot },
                Mesh3d(primitives.unit_cuboid.clone()),
                MeshMaterial3d(materials.preview.clone()),
                Transform::default(),
                Visibility::Hidden,
                Name::new("V3 weapon preview slot"),
            ));
        }
    }
}

fn unique_roots<T: Component>(
    commands: &mut Commands,
    roots: &Query<(Entity, &CombatVisualOwner), With<T>>,
) -> HashMap<Entity, Entity> {
    let mut result = HashMap::new();
    let mut ordered = roots.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|(entity, _)| entity.index());
    for (root, owner) in ordered {
        if result.insert(owner.0, root).is_some() {
            commands.entity(root).despawn();
        }
    }
    result
}

fn spawn_fighter(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    owner: Entity,
    position: Vec2,
    team: crate::combat::TeamId,
    controlled: bool,
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
            MeshMaterial3d(team_material(team, materials)),
            Transform::from_xyz(0.0, 24.0, 0.0),
            Name::new("V3 fighter fallback"),
        ));
        parent.spawn((
            Mesh3d(primitives.ground_ring.clone()),
            MeshMaterial3d(team_material(team, materials)),
            Transform::from_rotation(Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2)),
            Name::new("V3 fighter team ring"),
        ));
        parent.spawn((
            Mesh3d(primitives.direction.clone()),
            MeshMaterial3d(if controlled {
                materials.neutral.clone()
            } else {
                team_material(team, materials)
            }),
            Transform::from_xyz(27.0, 6.0, 0.0),
            Name::new("V3 fighter facing"),
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
    owner: Entity,
    position: Vec2,
    team: crate::combat::TeamId,
    straight: Option<&crate::combat::StraightFlight>,
    lobbed: bool,
) {
    let root = commands
        .spawn((
            CombatVisualOwner(owner),
            V3ProjectileVisual {
                planar_position: straight.map_or(position, |flight| flight.origin.as_vec2()),
            },
            Transform::from_translation(ground_position(position)),
            Visibility::default(),
            Name::new("V3 independent projectile visual root"),
        ))
        .id();
    commands.entity(root).with_children(|parent| {
        parent.spawn((
            Mesh3d(if lobbed {
                primitives.lobbed_projectile.clone()
            } else {
                primitives.projectile.clone()
            }),
            MeshMaterial3d(team_material(team, materials)),
            Transform::from_rotation(if lobbed {
                Quat::IDENTITY
            } else {
                Quat::from_rotation_z(core::f32::consts::FRAC_PI_2)
            }),
            Name::new("V3 projectile geometry"),
        ));
    });
}

fn spawn_sentry(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    owner: Entity,
    position: Vec2,
    team: crate::combat::TeamId,
) {
    let root = commands
        .spawn((
            CombatVisualOwner(owner),
            SentryVisual3d,
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
            Mesh3d(primitives.direction.clone()),
            MeshMaterial3d(team_material(team, materials)),
            Transform::from_xyz(25.0, 23.0, 0.0).with_scale(Vec3::new(0.8, 0.7, 0.7)),
        ));
    });
}

fn spawn_health(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    owner: Entity,
    position: Vec2,
) {
    let fill = commands
        .spawn((
            HealthFill3d,
            Mesh3d(primitives.unit_cuboid.clone()),
            MeshMaterial3d(materials.health_fill.clone()),
            Transform::from_xyz(0.0, 0.8, 0.0).with_scale(Vec3::new(52.0, 4.0, 3.0)),
            Name::new("V3 health fill"),
        ))
        .id();
    let root = commands
        .spawn((
            CombatVisualOwner(owner),
            WorldHealthVisual3d { fill },
            Transform::from_translation(ground_position(position) + Vec3::Y * HEALTH_HEIGHT),
            Visibility::default(),
            Name::new("V3 world health visual root"),
        ))
        .id();
    commands
        .entity(root)
        .add_child(fill)
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(primitives.unit_cuboid.clone()),
                MeshMaterial3d(materials.health_back.clone()),
                Transform::default().with_scale(Vec3::new(56.0, 6.0, 4.0)),
                Name::new("V3 health background"),
            ));
        });
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity,
    reason = "this state phase coordinates health, status, trail, and preview visual owners"
)]
pub(super) fn update_combat_visual_state(
    mut commands: Commands,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    definitions: Res<crate::combat::FighterDefinitions>,
    maps: Query<&crate::map::ResolvedMapSnapshot, With<crate::map::MapRoot>>,
    pending: Res<PendingLocalActions>,
    convergence: Option<Res<crate::terrain::ClientTerrainConvergence>>,
    fighters: Query<
        (
            Entity,
            &Position,
            &Rotation,
            &crate::combat::CurrentHealth,
            &crate::combat::FighterDefinitionId,
            Option<&crate::combat::Defeated>,
            Option<&AuthoritativeTick>,
            Option<&crate::combat::ActiveEffects>,
            Option<&crate::combat::KnockbackFeedback>,
            Option<&crate::builds::AbilityState>,
            Option<&crate::builds::ResolvedMatchLoadout>,
            Has<Controlled>,
        ),
        With<Fighter>,
    >,
    mut health_roots: Query<
        (&CombatVisualOwner, &WorldHealthVisual3d, &mut Visibility),
        Without<WeaponPreviewVisual3d>,
    >,
    mut fill_transforms: Query<
        &mut Transform,
        (
            With<HealthFill3d>,
            Without<DashTrailVisual3d>,
            Without<WeaponPreviewVisual3d>,
        ),
    >,
    mut statuses: Query<(Entity, &CombatVisualOwner, &StatusVisual3d)>,
    mut trails: Query<
        (
            Entity,
            &CombatVisualOwner,
            &mut DashTrailVisual3d,
            &mut Transform,
        ),
        (Without<HealthFill3d>, Without<WeaponPreviewVisual3d>),
    >,
    mut previews: Query<
        (
            &WeaponPreviewVisual3d,
            &mut Transform,
            &mut Visibility,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        (
            Without<HealthFill3d>,
            Without<DashTrailVisual3d>,
            Without<WorldHealthVisual3d>,
        ),
    >,
) {
    let mut desired_status = HashSet::new();
    let mut fighter_data = HashMap::new();
    let mut controlled = None;
    for (
        entity,
        position,
        rotation,
        health,
        definition,
        defeated,
        authoritative_tick,
        effects,
        knockback,
        ability,
        loadout,
        is_controlled,
    ) in &fighters
    {
        let maximum = loadout.map_or_else(
            || {
                definitions
                    .get(*definition)
                    .map_or(1, |value| value.maximum_health)
            },
            |value| value.fighter_stats.maximum_health,
        );
        fighter_data.insert(entity, (position.0, health.0, maximum, defeated.is_some()));
        if defeated.is_none() {
            if effects.is_some_and(|value| {
                value.slow.is_some_and(|slow| {
                    authoritative_tick.is_none_or(|now| now.0 < slow.expires_at_tick)
                })
            }) {
                desired_status.insert((entity, StatusKind::Slow));
            }
            if knockback.is_some() {
                desired_status.insert((entity, StatusKind::Knockback));
            }
        }
        let dashing = ability.is_some_and(|value| {
            matches!(value.phase, crate::builds::AbilityPhase::Dashing { .. })
        });
        let trail = trails.iter_mut().find(|(_, owner, _, _)| owner.0 == entity);
        match (dashing, trail) {
            (true, None) => {
                commands.spawn((
                    CombatVisualOwner(entity),
                    DashTrailVisual3d {
                        last_position: position.0,
                    },
                    Mesh3d(primitives.unit_cuboid.clone()),
                    MeshMaterial3d(materials.dash.clone()),
                    Transform::from_translation(ground_position(position.0) + Vec3::Y * 3.0),
                    Name::new("V3 dash trail"),
                ));
            }
            (true, Some((_, _, mut trail, mut transform))) => {
                let delta = position.0 - trail.last_position;
                if delta.length_squared() > f32::EPSILON {
                    transform.translation =
                        ground_position(trail.last_position.midpoint(position.0)) + Vec3::Y * 3.0;
                    transform.rotation = ground_rotation(Rotation::radians(delta.y.atan2(delta.x)));
                    transform.scale = Vec3::new(delta.length().max(2.0), 3.0, 12.0);
                    trail.last_position = position.0;
                }
            }
            (false, Some((trail_entity, _, _, _))) => commands.entity(trail_entity).despawn(),
            (false, None) => {}
        }
        if is_controlled {
            controlled = loadout.map(|loadout| (position.0, rotation.as_radians(), loadout));
        }
    }

    for (owner, health, mut visibility) in &mut health_roots {
        let Some((_, current, maximum, defeated)) = fighter_data.get(&owner.0) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *visibility = if *defeated {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        let ratio = (f32::from(*current) / f32::from((*maximum).max(1))).clamp(0.0, 1.0);
        if let Ok(mut fill) = fill_transforms.get_mut(health.fill) {
            fill.scale.x = 52.0 * ratio;
            fill.translation.x = -26.0 * (1.0 - ratio);
        }
    }

    let existing_status: HashSet<_> = statuses
        .iter()
        .map(|(_, owner, kind)| (owner.0, kind.0))
        .collect();
    for (entity, owner, kind) in &mut statuses {
        if !desired_status.contains(&(owner.0, kind.0)) {
            commands.entity(entity).despawn();
        }
    }
    for (owner, kind) in desired_status.difference(&existing_status).copied() {
        let Some((position, ..)) = fighter_data.get(&owner) else {
            continue;
        };
        commands.spawn((
            CombatVisualOwner(owner),
            StatusVisual3d(kind),
            Mesh3d(primitives.ground_ring.clone()),
            MeshMaterial3d(match kind {
                StatusKind::Slow => materials.status_slow.clone(),
                StatusKind::Knockback => materials.status_knockback.clone(),
            }),
            Transform {
                translation: ground_position(*position)
                    + Vec3::Y * if kind == StatusKind::Slow { 2.0 } else { 3.0 },
                rotation: Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2),
                scale: Vec3::splat(if kind == StatusKind::Slow { 1.15 } else { 0.8 }),
            },
            Name::new("V3 durable combat status"),
        ));
    }

    let no_terrain = BTreeMap::new();
    let map = maps.iter().max_by_key(|map| map.identity.instance_id);
    let segments = match (map, controlled) {
        (Some(map), Some((origin, facing, loadout))) => preview_segments(
            origin,
            facing,
            pending.aim_distance,
            &loadout.primary_weapon,
            map,
            convergence
                .as_deref()
                .map_or(&no_terrain, |value| value.chunks()),
        ),
        _ => Vec::new(),
    };
    for (slot, mut transform, mut visibility, mut material) in &mut previews {
        let Some((center, angle, size, color)) = segments.get(usize::from(slot.slot)) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *visibility = Visibility::Inherited;
        transform.translation = ground_position(*center) + Vec3::Y * PREVIEW_HEIGHT;
        transform.rotation = ground_rotation(Rotation::radians(*angle));
        transform.scale = Vec3::new(size.x, 1.2, size.y.max(2.0));
        let rgba = color.to_srgba();
        material.0 = if rgba.red > 0.9 && rgba.green < 0.4 {
            materials.preview_blocked.clone()
        } else {
            materials.preview.clone()
        };
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "cue consumption resolves actor intents and one bounded effect transaction"
)]
pub(super) fn consume_combat_cues(
    mut commands: Commands,
    mut cues: MessageReader<DeduplicatedCombatCue>,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    settings: Option<Res<ClientShellSettings>>,
    owners: Query<(Entity, &NetworkEntityId), With<Fighter>>,
    mut visuals: Query<(&CombatVisualOwner, &mut V3FighterVisual)>,
    effects: Query<(Entity, &CombatEffect3d)>,
    mut sequence: Local<CombatEffectSequence>,
) {
    let reduced = settings.is_some_and(|value| value.reduced_combat_effects);
    for DeduplicatedCombatCue(cue) in cues.read() {
        if let crate::combat::CombatCue::AttackAccepted { source, .. } = cue
            && let Some((owner, _)) = owners.iter().find(|(_, id)| **id == *source)
        {
            for (link, mut visual) in &mut visuals {
                if link.0 == owner {
                    visual.shoot_seconds = 0.18;
                }
            }
        }
        let Some((position, material, scale)) = cue_effect(cue, &materials) else {
            continue;
        };
        if effects.iter().count() >= MAX_EFFECTS
            && let Some((oldest, _)) = effects.iter().min_by_key(|(_, effect)| effect.order)
        {
            commands.entity(oldest).despawn();
        }
        sequence.0 = sequence.0.saturating_add(1);
        commands.spawn((
            CombatEffect3d {
                timer: Timer::from_seconds(if reduced { 0.10 } else { 0.18 }, TimerMode::Once),
                order: sequence.0,
            },
            Mesh3d(primitives.effect_sphere.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(ground_position(position) + Vec3::Y * (scale * 0.45))
                .with_scale(Vec3::splat(scale * if reduced { 0.65 } else { 1.0 })),
            Name::new("V3 bounded combat cue effect"),
        ));
    }
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
        C::Muzzle { .. }
        | C::Impact { .. }
        | C::Damage { .. }
        | C::Defeat { .. }
        | C::Reset { .. } => None,
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Time is a Bevy system resource parameter"
)]
pub(super) fn cleanup_combat_effects(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut effects: Query<(Entity, &mut CombatEffect3d)>,
) {
    for (entity, mut effect) in &mut effects {
        effect.timer.tick(time.delta());
        if effect.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

#[allow(
    clippy::cast_precision_loss,
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "the single final pose phase owns every dynamic visual-root coordinate conversion"
)]
pub(super) fn write_combat_visual_poses(
    time: Res<Time>,
    ticks: Query<&AuthoritativeTick>,
    fighter_owners: Query<(&Position, &Rotation), With<Fighter>>,
    projectile_owners: Query<
        (
            &Position,
            &Rotation,
            Option<&crate::combat::StraightFlight>,
            Option<&crate::combat::LobbedFlight>,
        ),
        With<crate::combat::Projectile>,
    >,
    sentry_owners: Query<(&Position, &Rotation), With<crate::abilities::Sentry>>,
    mut fighter_visuals: Query<(&CombatVisualOwner, &mut V3FighterVisual, &mut Transform)>,
    mut projectile_visuals: Query<
        (&CombatVisualOwner, &mut V3ProjectileVisual, &mut Transform),
        (Without<V3FighterVisual>, Without<SentryVisual3d>),
    >,
    mut sentry_visuals: Query<
        (&CombatVisualOwner, &mut Transform),
        (
            With<SentryVisual3d>,
            Without<V3FighterVisual>,
            Without<V3ProjectileVisual>,
        ),
    >,
    mut health_visuals: Query<
        (&CombatVisualOwner, &mut Transform),
        (
            With<WorldHealthVisual3d>,
            Without<V3FighterVisual>,
            Without<V3ProjectileVisual>,
            Without<SentryVisual3d>,
        ),
    >,
    mut status_visuals: Query<
        (&CombatVisualOwner, &mut Transform),
        (
            With<StatusVisual3d>,
            Without<V3FighterVisual>,
            Without<V3ProjectileVisual>,
            Without<SentryVisual3d>,
            Without<WorldHealthVisual3d>,
        ),
    >,
) {
    for (owner, mut visual, mut transform) in &mut fighter_visuals {
        if let Ok((position, rotation)) = fighter_owners.get(owner.0) {
            visual.moving = visual.last_position.distance_squared(position.0) > 0.25;
            visual.last_position = position.0;
            transform.translation = ground_position(position.0);
            transform.rotation = ground_rotation(*rotation);
        }
    }
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
    for (owner, mut transform) in &mut sentry_visuals {
        if let Ok((position, rotation)) = sentry_owners.get(owner.0) {
            transform.translation = ground_position(position.0);
            transform.rotation = ground_rotation(*rotation);
        }
    }
    for (owner, mut transform) in &mut health_visuals {
        if let Ok((position, _)) = fighter_owners.get(owner.0) {
            transform.translation = ground_position(position.0) + Vec3::Y * HEALTH_HEIGHT;
            transform.rotation = Quat::IDENTITY;
        }
    }
    for (owner, mut transform) in &mut status_visuals {
        if let Ok((position, _)) = fighter_owners.get(owner.0) {
            let height = transform.translation.y;
            transform.translation = ground_position(position.0) + Vec3::Y * height;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_root_maps_positive_and_negative_simulation_y_once() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .add_systems(PostUpdate, write_combat_visual_poses);
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
    fn health_ratio_is_clamped_and_offsets_fill_from_the_left_edge() {
        let mut transform = Transform::default().with_scale(Vec3::new(52.0, 4.0, 3.0));
        let ratio = (25.0_f32 / 100.0).clamp(0.0, 1.0);
        transform.scale.x = 52.0 * ratio;
        transform.translation.x = -26.0 * (1.0 - ratio);
        assert!((transform.scale.x - 13.0).abs() < f32::EPSILON);
        assert!((transform.translation.x + 19.5).abs() < f32::EPSILON);
    }
}
