use super::super::*;
use super::common::{
    GROUND_EFFECT_HEIGHT, GroundMarkerRelation, ground_marker_material, ground_marker_relation,
    unique_roots,
};

const GROUND_MARKER_HEIGHT: f32 = 1.0;
const STRAIGHT_PROJECTILE_VISUAL_THICKNESS: f32 = 6.0;

#[derive(Component)]
pub(in super::super) struct SentryVisual3d;

#[derive(Component)]
pub(in super::super) struct ConcealmentFieldVisual3d;

#[derive(Component)]
pub(in super::super) struct ElementalFieldVisual3d;

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
    clippy::type_complexity,
    reason = "projectile roots consume the complete replicated delivery shape at materialization"
)]
pub(in super::super) fn reconcile_projectile_visuals(
    mut commands: Commands,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    projectiles: Query<
        (
            Entity,
            &Position,
            &crate::combat::ProjectileSource,
            Option<&crate::combat::StraightFlight>,
            Option<&crate::combat::ProjectileBody>,
            Option<&crate::combat::LobbedFlight>,
        ),
        With<crate::combat::Projectile>,
    >,
    visuals: Query<(Entity, &CombatVisualOwner), With<V3ProjectileVisual>>,
) {
    let roots = unique_roots(&mut commands, &visuals);
    for (owner, position, source, straight, body, lobbed) in &projectiles {
        if straight.is_some() && body.is_none() {
            continue;
        }
        if !roots.contains(&owner) {
            spawn_projectile(
                &mut commands,
                &primitives,
                &materials,
                owner,
                position.0,
                source.team_id,
                straight,
                body,
                lobbed.is_some(),
            );
        }
    }
    for (root, owner) in &visuals {
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
    sentries: Query<
        (Entity, &Position, &crate::abilities::SentryIdentity),
        With<crate::abilities::Sentry>,
    >,
    visuals: Query<(Entity, &CombatVisualOwner), With<SentryVisual3d>>,
) {
    let roots = unique_roots(&mut commands, &visuals);
    for (owner, position, identity) in &sentries {
        if !roots.contains(&owner) {
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
    fields: Query<(Entity, &crate::concealment::ConcealmentFieldState)>,
    visuals: Query<(Entity, &CombatVisualOwner), With<ConcealmentFieldVisual3d>>,
) {
    let roots = unique_roots(&mut commands, &visuals);
    for (owner, state) in &fields {
        if !roots.contains(&owner)
            && let Some(radius) = state.radius()
        {
            spawn_concealment_field(
                &mut commands,
                &primitives,
                &materials,
                owner,
                state.center_vec2(),
                radius,
                state.team,
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
    fields: Query<(Entity, &crate::combat::ElementalFieldState)>,
    visuals: Query<(Entity, &CombatVisualOwner), With<ElementalFieldVisual3d>>,
) {
    let roots = unique_roots(&mut commands, &visuals);
    for (owner, state) in &fields {
        if !roots.contains(&owner)
            && let Some(radius) = state.radius()
        {
            let material = match state.kind {
                crate::combat::ElementalFieldKind::Cryogenic => materials.elemental_cold.clone(),
                crate::combat::ElementalFieldKind::Fire => materials.elemental_fire.clone(),
                crate::combat::ElementalFieldKind::Poison => materials.elemental_poison.clone(),
                crate::combat::ElementalFieldKind::Restoration => {
                    materials.elemental_restoration.clone()
                }
            };
            spawn_elemental_field(
                &mut commands,
                &primitives,
                owner,
                state.center_vec2(),
                radius,
                material,
            );
        }
    }
    for (root, owner) in &visuals {
        if fields.get(owner.0).is_err() {
            commands.entity(root).despawn();
        }
    }
}

fn spawn_elemental_field(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    owner: Entity,
    center: Vec2,
    radius: f32,
    material: Handle<StandardMaterial>,
) {
    let root = commands
        .spawn((
            CombatVisualOwner(owner),
            ElementalFieldVisual3d,
            Transform::from_translation(ground_position(center)),
            Visibility::default(),
            Name::new("Elemental Field visual root"),
        ))
        .id();
    commands.entity(root).with_children(|parent| {
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
    });
}

fn spawn_concealment_field(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    owner: Entity,
    center: Vec2,
    radius: f32,
    team: crate::combat::TeamId,
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
            Transform::from_xyz(0.0, FIGHTER_FALLBACK_RADIUS, 0.0),
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
    owner: Entity,
    position: Vec2,
    team: crate::combat::TeamId,
    straight: Option<&crate::combat::StraightFlight>,
    body: Option<&crate::combat::ProjectileBody>,
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
        let transform = if lobbed {
            Transform::default()
        } else {
            Transform::from_scale(straight_projectile_visual_scale(
                *body.expect("straight projectile visual requires replicated body"),
            ))
        };
        parent.spawn((
            Mesh3d(if lobbed {
                primitives.lobbed_projectile.clone()
            } else {
                primitives.projectile.clone()
            }),
            MeshMaterial3d(team_material(team, materials)),
            NotShadowCaster,
            transform,
            Name::new("V3 projectile geometry"),
        ));
    });
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
