//! Complete 3D combat presentation over independent, client-only visual roots.

use super::*;
use crate::combat::client::{
    AimTraceBlockerClass, AimTraceBlockerIndex, AimTraceDynamicBlocker, DeduplicatedCombatCue,
    MAX_PREVIEW_SEGMENTS, PreviewGeometry, PreviewPrimitive, preview_primitives,
};
use std::collections::{HashMap, HashSet, VecDeque};

const PREVIEW_HEIGHT: f32 = 2.5;
const FIGHTER_BODY_WORLD_HEIGHT: f32 = KENNEY_CHARACTER_WORLD_HEIGHT;
const OVERHEAD_WORLD_HEIGHT: f32 = FIGHTER_BODY_WORLD_HEIGHT + 12.0;
const OVERHEAD_WIDTH: f32 = 104.0;
const OVERHEAD_HEALTH_HEIGHT: f32 = 37.0;
const OVERHEAD_AMMO_HEIGHT: f32 = 50.0;
const HEALTH_BAR_WIDTH: f32 = 76.8;
const PLAYER_NAME_FONT_SIZE: f32 = 12.8;
const GROUND_MARKER_HEIGHT: f32 = 1.0;
const MAX_EFFECTS: usize = 96;
const CONCEALED_FIGHTER_ALPHA: f32 = 0.52;
const STRAIGHT_PROJECTILE_VISUAL_THICKNESS: f32 = 6.0;

#[derive(Component)]
pub(super) struct SentryVisual3d;

#[derive(Component)]
pub(super) struct ConcealmentFieldVisual3d;

#[derive(Component)]
pub(super) struct FighterGroundMarker3d {
    owner: Entity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GroundMarkerRelation {
    Local,
    Ally,
    Enemy,
}

#[derive(Clone, Copy)]
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

#[derive(Component)]
pub(super) struct FighterConcealmentMaterial {
    normal: Handle<StandardMaterial>,
    concealed: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
pub(super) struct ConcealedMaterialVariants {
    handles: HashMap<AssetId<StandardMaterial>, Handle<StandardMaterial>>,
}

type GroundMarkerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static FighterGroundMarker3d,
        &'static mut MeshMaterial3d<StandardMaterial>,
    ),
>;

#[derive(Component)]
pub(super) struct FighterOverheadUi {
    player_name: Entity,
    health_amount: Entity,
    fill: Entity,
    ammo_row: Entity,
    ammo_segments: Vec<Entity>,
    ammo_fills: Vec<Entity>,
}

#[derive(Component)]
pub(super) struct FighterHealthFillUi;

#[derive(Component)]
pub(super) struct FighterOverheadTextUi;

#[derive(Component)]
pub(super) struct FighterAmmoRowUi;

#[derive(Component)]
pub(super) struct FighterAmmoSegmentUi;

#[derive(Component)]
pub(super) struct FighterAmmoSegmentFillUi;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct StatusVisual3d(StatusKind);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum StatusKind {
    Slow,
    Knockback,
    Reveal,
}

#[derive(Component)]
pub(super) struct DashTrailVisual3d {
    last_position: Vec2,
}

#[derive(Component, Clone, Copy)]
pub(super) struct DashTrailLink(Entity);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DashTrailAction {
    Spawn,
    Update(Entity),
    Remove(Entity),
    ClearStaleLink,
    None,
}

#[derive(Component)]
pub(super) struct WeaponPreviewVisual3d {
    slot: u8,
}

type OverheadFighterQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static crate::combat::CurrentHealth,
        &'static crate::combat::FighterDefinitionId,
        Option<&'static crate::combat::Defeated>,
        Option<&'static AuthoritativeTick>,
        Option<&'static crate::builds::ResolvedMatchLoadout>,
        &'static crate::combat::TeamId,
        Option<&'static crate::matchplay::FighterDisplayName>,
        Option<&'static crate::combat::WeaponState>,
        Has<Controlled>,
    ),
    With<Fighter>,
>;

type AmmoFillQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Node,
    Or<(With<FighterHealthFillUi>, With<FighterAmmoSegmentFillUi>)>,
>;

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

type AimFighterQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static NetworkEntityId,
        &'static Position,
        &'static Rotation,
        Option<&'static crate::combat::Defeated>,
        Option<&'static crate::builds::AbilityState>,
        Option<&'static crate::builds::ResolvedMatchLoadout>,
        &'static crate::combat::TeamId,
        Has<Controlled>,
    ),
    With<Fighter>,
>;

type PreviewVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static WeaponPreviewVisual3d,
        &'static mut Transform,
        &'static mut Visibility,
        &'static mut Mesh3d,
        &'static mut MeshMaterial3d<StandardMaterial>,
    ),
    (
        Without<FighterHealthFillUi>,
        Without<DashTrailVisual3d>,
        Without<FighterOverheadUi>,
        Without<FighterAmmoRowUi>,
    ),
>;

type ControlledAim<'a> = (
    Vec2,
    f32,
    &'a crate::builds::ResolvedMatchLoadout,
    Option<&'a crate::builds::AbilityState>,
);
type ActivePreviewMap<'a> = (
    &'a crate::map::ResolvedMapSnapshot,
    &'a crate::map::MapDynamicState,
);

type SentryAimQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static crate::abilities::SentryIdentity,
        &'static crate::combat::CurrentHealth,
    ),
    With<crate::abilities::Sentry>,
>;

type SafeAimQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static Rotation,
        &'static crate::matchplay::HeistSafe,
        &'static crate::combat::CurrentHealth,
    ),
>;

#[derive(Clone, Copy)]
struct AmmoPresentation {
    visible: bool,
    capacity: u8,
    available: u8,
    recovery_progress: f32,
}

#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct AimPreviewInputs<'w, 's> {
    maps: Query<
        'w,
        's,
        (
            &'static crate::map::ResolvedMapSnapshot,
            &'static crate::map::MapDynamicState,
        ),
        With<crate::map::MapRoot>,
    >,
    catalog: Res<'w, crate::map::MapCatalogResource>,
    pending: Res<'w, PendingLocalActions>,
    index: ResMut<'w, AimTraceBlockerIndex>,
    sentries: SentryAimQuery<'w, 's>,
    safes: SafeAimQuery<'w, 's>,
}

#[derive(Component)]
pub(super) struct CombatEffect3d {
    timer: Timer,
    expires_at_tick: Option<u64>,
    order: u64,
}

#[derive(Default)]
pub(super) struct CombatEffectSequence(u64);

#[derive(Message)]
pub(super) struct PendingCombatEffect {
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
    clippy::cast_possible_truncation,
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity,
    reason = "this reconciliation phase owns the complete set of independent durable visual families"
)]
pub(super) fn reconcile_combat_visuals(
    mut commands: Commands,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    fighters: FighterPresentationQuery,
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
    sentries: Query<
        (Entity, &Position, &crate::abilities::SentryIdentity),
        With<crate::abilities::Sentry>,
    >,
    fields: Query<(Entity, &crate::concealment::ConcealmentFieldState)>,
    fighter_visuals: Query<(Entity, &CombatVisualOwner), With<V3FighterVisual>>,
    projectile_visuals: Query<(Entity, &CombatVisualOwner), With<V3ProjectileVisual>>,
    sentry_visuals: Query<(Entity, &CombatVisualOwner), With<SentryVisual3d>>,
    field_visuals: Query<(Entity, &CombatVisualOwner), With<ConcealmentFieldVisual3d>>,
    overhead_visuals: Query<(Entity, &CombatVisualOwner), With<FighterOverheadUi>>,
    trails: Query<(Entity, &CombatVisualOwner), With<DashTrailVisual3d>>,
    statuses: Query<(Entity, &CombatVisualOwner), With<StatusVisual3d>>,
    previews: Query<&WeaponPreviewVisual3d>,
    mut ground_markers: GroundMarkerQuery,
) {
    let fighter_roots = unique_roots(&mut commands, &fighter_visuals);
    let projectile_roots = unique_roots(&mut commands, &projectile_visuals);
    let sentry_roots = unique_roots(&mut commands, &sentry_visuals);
    let field_roots = unique_roots(&mut commands, &field_visuals);
    let overhead_roots = unique_roots(&mut commands, &overhead_visuals);
    let controlled_team = fighters
        .iter()
        .find_map(|(_, _, team, controlled, _)| controlled.then_some(*team));

    update_ground_markers(&fighters, &mut ground_markers, controlled_team, &materials);

    for (owner, position, team, controlled, _) in &fighters {
        if !fighter_roots.contains_key(&owner) {
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
        if !overhead_roots.contains_key(&owner) {
            spawn_fighter_overhead(&mut commands, owner);
        }
    }
    for (owner, position, source, straight, body, lobbed) in &projectiles {
        if straight.is_some() && body.is_none() {
            continue;
        }
        if !projectile_roots.contains_key(&owner) {
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
    for (owner, state) in &fields {
        if !field_roots.contains_key(&owner)
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
    for (root, owner) in &field_visuals {
        if fields.get(owner.0).is_err() {
            commands.entity(root).despawn();
        }
    }
    for (root, owner) in &overhead_visuals {
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

    ensure_targeting_previews(&mut commands, &primitives, &materials, &previews);
}

fn ensure_targeting_previews(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    previews: &Query<&WeaponPreviewVisual3d>,
) {
    if !previews.is_empty() {
        return;
    }
    for slot in 0..u8::try_from(MAX_PREVIEW_SEGMENTS).expect("preview slot bound fits u8") {
        commands.spawn((
            WeaponPreviewVisual3d { slot },
            Mesh3d(primitives.unit_cuboid.clone()),
            MeshMaterial3d(materials.preview.clone()),
            NotShadowCaster,
            Transform::default(),
            Visibility::Hidden,
            Name::new("V3 weapon preview slot"),
        ));
    }
    commands.spawn((
        WeaponPreviewVisual3d { slot: u8::MAX },
        Mesh3d(primitives.area_ring.clone()),
        MeshMaterial3d(materials.preview.clone()),
        NotShadowCaster,
        NotShadowReceiver,
        Transform::default(),
        Visibility::Hidden,
        Name::new("V9 targeted ultimate area ring"),
    ));
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
            Transform::from_xyz(0.0, PREVIEW_HEIGHT, 0.0)
                .with_rotation(Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2))
                .with_scale(Vec3::splat(radius)),
            Name::new("V9 Concealment Field fill"),
        ));
        parent.spawn((
            Mesh3d(primitives.area_ring.clone()),
            MeshMaterial3d(boundary),
            NotShadowCaster,
            NotShadowReceiver,
            Transform::from_xyz(0.0, PREVIEW_HEIGHT + 0.5, 0.0)
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

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "the presentation pass discovers material-bearing descendants across imported and fallback fighter hierarchies"
)]
pub(super) fn update_fighter_concealment_visuals(
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

fn ground_marker_relation(
    team: crate::combat::TeamId,
    controlled: bool,
    controlled_team: Option<crate::combat::TeamId>,
) -> GroundMarkerRelation {
    if controlled {
        GroundMarkerRelation::Local
    } else if controlled_team == Some(team) {
        GroundMarkerRelation::Ally
    } else {
        GroundMarkerRelation::Enemy
    }
}

fn ground_marker_material(
    relation: GroundMarkerRelation,
    materials: &Material3dAssets,
) -> Handle<StandardMaterial> {
    match relation {
        GroundMarkerRelation::Local => materials.marker_local.clone(),
        GroundMarkerRelation::Ally => materials.marker_ally.clone(),
        GroundMarkerRelation::Enemy => materials.marker_enemy.clone(),
    }
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

fn spawn_fighter_overhead(commands: &mut Commands, owner: Entity) {
    let (player_name_container, player_name) = spawn_overhead_text(
        commands,
        0.0,
        19.0,
        PLAYER_NAME_FONT_SIZE,
        "V3 fighter overhead player name",
    );
    let (health_amount_container, health_amount) = spawn_overhead_text(
        commands,
        14.0,
        18.0,
        15.0,
        "V3 fighter overhead health amount",
    );
    let fill = commands
        .spawn((
            FighterHealthFillUi,
            Node {
                width: percent(100.0),
                height: percent(100.0),
                border_radius: BorderRadius::all(px(5.0)),
                ..default()
            },
            BackgroundColor(Color::WHITE),
            Name::new("V3 fighter overhead health fill"),
        ))
        .id();
    let health_bar = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px((OVERHEAD_WIDTH - HEALTH_BAR_WIDTH) * 0.5),
                top: px(24.0),
                width: px(HEALTH_BAR_WIDTH),
                height: px(11.0),
                padding: UiRect::all(px(2.0)),
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.025, 0.03, 0.04)),
            Name::new("V3 fighter overhead rounded health bar"),
        ))
        .add_child(fill)
        .id();
    let ammo_row = commands
        .spawn((
            FighterAmmoRowUi,
            Node {
                position_type: PositionType::Absolute,
                left: px((OVERHEAD_WIDTH - HEALTH_BAR_WIDTH) * 0.5),
                top: px(39.0),
                width: px(HEALTH_BAR_WIDTH),
                height: px(7.0),
                column_gap: px(2.0),
                ..default()
            },
            Visibility::Hidden,
            Name::new("V3 fighter overhead ammunition row"),
        ))
        .id();
    commands
        .spawn((
            CombatVisualOwner(owner),
            FighterOverheadUi {
                player_name,
                health_amount,
                fill,
                ammo_row,
                ammo_segments: Vec::new(),
                ammo_fills: Vec::new(),
            },
            Node {
                position_type: PositionType::Absolute,
                width: px(OVERHEAD_WIDTH),
                height: px(OVERHEAD_HEALTH_HEIGHT),
                ..default()
            },
            GlobalZIndex(120),
            Visibility::Hidden,
            Name::new("V3 fighter projected overhead UI"),
        ))
        .add_children(&[
            player_name_container,
            health_amount_container,
            health_bar,
            ammo_row,
        ]);
}

fn spawn_overhead_text(
    commands: &mut Commands,
    top: f32,
    height: f32,
    font_size: f32,
    name: &'static str,
) -> (Entity, Entity) {
    let text = commands
        .spawn((
            FighterOverheadTextUi,
            Text::new(""),
            TextFont::from_font_size(font_size),
            TextColor(Color::WHITE),
            TextShadow {
                offset: Vec2::splat(1.5),
                color: Color::BLACK,
            },
            TextLayout::new(Justify::Center, LineBreak::NoWrap),
            Name::new(name),
        ))
        .id();
    let container = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(top),
                left: px(0.0),
                right: px(0.0),
                width: percent(100.0),
                height: px(height),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            GlobalZIndex(122),
            Name::new(format!("{name} centered container")),
        ))
        .add_child(text)
        .id();
    (container, text)
}

fn overhead_name_color(relation: GroundMarkerRelation) -> Color {
    match relation {
        GroundMarkerRelation::Local => Color::srgb(0.18, 0.95, 0.36),
        GroundMarkerRelation::Ally => Color::srgb(0.12, 0.72, 0.96),
        GroundMarkerRelation::Enemy => Color::srgb(1.0, 0.18, 0.14),
    }
}

fn overhead_health_color(relation: GroundMarkerRelation) -> Color {
    match relation {
        GroundMarkerRelation::Local | GroundMarkerRelation::Ally => Color::srgb(0.18, 0.92, 0.34),
        GroundMarkerRelation::Enemy => Color::srgb(0.95, 0.14, 0.12),
    }
}

fn ammo_segment_color(available: bool) -> Color {
    if available {
        Color::srgb(1.0, 0.55, 0.16)
    } else {
        Color::srgb(0.10, 0.14, 0.22)
    }
}

#[allow(clippy::cast_precision_loss)] // Tick precision beyond an on-screen percentage is irrelevant.
fn ammo_recovery_progress(
    state: Option<&crate::combat::WeaponState>,
    observed_tick: Option<u64>,
) -> f32 {
    let Some((recovery, tick)) = state
        .and_then(|state| state.ammo_recovery)
        .zip(observed_tick)
    else {
        return 0.0;
    };
    let duration = recovery
        .ready_at_tick
        .saturating_sub(recovery.started_at_tick)
        .max(1);
    let elapsed = tick.saturating_sub(recovery.started_at_tick).min(duration);
    elapsed as f32 / duration as f32
}

fn overhead_height(has_ammunition: bool) -> f32 {
    if has_ammunition {
        OVERHEAD_AMMO_HEIGHT
    } else {
        OVERHEAD_HEALTH_HEIGHT
    }
}

fn projected_overhead_top_left(
    viewport_size: Vec2,
    fighter_viewport: Vec2,
    overhead_viewport: Vec2,
    height: f32,
) -> Option<Vec2> {
    if !viewport_size.is_finite()
        || viewport_size.x <= 0.0
        || viewport_size.y <= 0.0
        || !fighter_viewport.is_finite()
        || fighter_viewport.x < 0.0
        || fighter_viewport.x > viewport_size.x
        || fighter_viewport.y < 0.0
        || fighter_viewport.y > viewport_size.y
        || !overhead_viewport.is_finite()
        || !height.is_finite()
        || height <= 0.0
    {
        return None;
    }
    let top_left = overhead_viewport - Vec2::new(OVERHEAD_WIDTH * 0.5, height);
    (top_left.x + OVERHEAD_WIDTH >= 0.0
        && top_left.x <= viewport_size.x
        && top_left.y + height >= 0.0
        && top_left.y <= viewport_size.y)
        .then_some(top_left)
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "the projection phase reads the propagated world camera and writes absolute UI nodes"
)]
pub(super) fn project_fighter_overhead_ui(
    cameras: Query<(&Camera, &GlobalTransform), With<ArenaCamera>>,
    fighters: Query<
        (
            &crate::combat::CurrentHealth,
            Option<&crate::combat::Defeated>,
        ),
        With<Fighter>,
    >,
    fighter_visuals: Query<(&CombatVisualOwner, &GlobalTransform), With<V3FighterVisual>>,
    mut overheads: Query<
        (
            &CombatVisualOwner,
            &FighterOverheadUi,
            &mut Node,
            &mut Visibility,
        ),
        With<FighterOverheadUi>,
    >,
) {
    let Ok((camera, camera_transform)) = cameras.single() else {
        for (_, _, _, mut visibility) in &mut overheads {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let Some(viewport_size) = camera.logical_viewport_size() else {
        for (_, _, _, mut visibility) in &mut overheads {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    for (owner, overhead, mut node, mut visibility) in &mut overheads {
        let Ok((health, defeated)) = fighters.get(owner.0) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        if character_is_visually_defeated(health.0, defeated.is_some()) {
            *visibility = Visibility::Hidden;
            continue;
        }
        let Some((_, visual_transform)) = fighter_visuals
            .iter()
            .find(|(visual_owner, _)| visual_owner.0 == owner.0)
        else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let ground_position = visual_transform.translation();
        let Ok(fighter_viewport) = camera.world_to_viewport(
            camera_transform,
            ground_position + Vec3::Y * FIGHTER_BODY_WORLD_HEIGHT,
        ) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Ok(overhead_viewport) = camera.world_to_viewport(
            camera_transform,
            ground_position + Vec3::Y * OVERHEAD_WORLD_HEIGHT,
        ) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let height = overhead_height(!overhead.ammo_segments.is_empty());
        let Some(top_left) =
            projected_overhead_top_left(viewport_size, fighter_viewport, overhead_viewport, height)
        else {
            *visibility = Visibility::Hidden;
            continue;
        };
        node.left = px(top_left.x);
        node.top = px(top_left.y);
        node.height = px(height);
        // Projection is the sole owner that reveals a root, after installing valid coordinates.
        *visibility = Visibility::Inherited;
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "the overhead lifecycle reads the complete replicated fighter label, health, and ammunition state"
)]
pub(super) fn update_fighter_overhead_state(
    mut commands: Commands,
    definitions: Res<crate::combat::FighterDefinitions>,
    fighters: OverheadFighterQuery,
    mut overhead_roots: Query<
        (&CombatVisualOwner, &mut FighterOverheadUi, &mut Visibility),
        Without<WeaponPreviewVisual3d>,
    >,
    mut fill_nodes: AmmoFillQuery,
    mut overhead_texts: Query<(&mut Text, &mut TextColor), With<FighterOverheadTextUi>>,
    mut overhead_colors: Query<
        &mut BackgroundColor,
        Or<(With<FighterHealthFillUi>, With<FighterAmmoSegmentUi>)>,
    >,
    mut ammo_rows: Query<&mut Visibility, (With<FighterAmmoRowUi>, Without<FighterOverheadUi>)>,
) {
    let mut fighter_data = HashMap::new();
    let controlled_team = fighters
        .iter()
        .find_map(|(_, _, _, _, _, _, team, _, _, is_controlled)| is_controlled.then_some(*team));
    for (
        entity,
        health,
        definition,
        defeated,
        authoritative_tick,
        loadout,
        team,
        display_name,
        weapon,
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
        let observed_tick = authoritative_tick.map(|tick| tick.0);
        fighter_data.insert(
            entity,
            (
                health.0,
                maximum,
                defeated.is_some(),
                ground_marker_relation(*team, is_controlled, controlled_team),
                display_name.map_or("Player", |name| name.0.as_str()),
                weapon.map_or(0, |state| state.ammo),
                loadout.map_or(0, |value| value.primary_weapon.recipe.economy.capacity()),
                ammo_recovery_progress(weapon, observed_tick),
            ),
        );
    }

    for (owner, mut overhead, mut visibility) in &mut overhead_roots {
        let Some((current, maximum, defeated, relation, name, ammo, capacity, ammo_progress)) =
            fighter_data.get(&owner.0)
        else {
            *visibility = Visibility::Hidden;
            continue;
        };
        // State reconciliation may hide roots; projection alone reveals correctly positioned ones.
        if *defeated {
            *visibility = Visibility::Hidden;
        }
        let ratio = (f32::from(*current) / f32::from((*maximum).max(1))).clamp(0.0, 1.0);
        if let Ok(mut fill) = fill_nodes.get_mut(overhead.fill) {
            fill.width = percent(ratio * 100.0);
        }
        if let Ok(mut color) = overhead_colors.get_mut(overhead.fill) {
            color.0 = overhead_health_color(*relation);
        }
        if let Ok((mut text, mut color)) = overhead_texts.get_mut(overhead.player_name) {
            let display = (*name).to_string();
            if text.0 != display {
                text.0 = display;
            }
            color.0 = overhead_name_color(*relation);
        }
        if let Ok((mut text, _)) = overhead_texts.get_mut(overhead.health_amount) {
            let amount = current.to_string();
            if text.0 != amount {
                text.0 = amount;
            }
        }

        reconcile_overhead_ammunition(
            &mut commands,
            &mut overhead,
            &mut ammo_rows,
            &mut fill_nodes,
            AmmoPresentation {
                visible: *relation == GroundMarkerRelation::Local && *capacity > 0,
                capacity: *capacity,
                available: *ammo,
                recovery_progress: *ammo_progress,
            },
        );
    }
}

fn reconcile_overhead_ammunition(
    commands: &mut Commands,
    overhead: &mut FighterOverheadUi,
    ammo_rows: &mut Query<&mut Visibility, (With<FighterAmmoRowUi>, Without<FighterOverheadUi>)>,
    fill_nodes: &mut AmmoFillQuery,
    presentation: AmmoPresentation,
) {
    if let Ok(mut visibility) = ammo_rows.get_mut(overhead.ammo_row) {
        *visibility = if presentation.visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    let desired_segments = if presentation.visible {
        usize::from(presentation.capacity)
    } else {
        0
    };
    if overhead.ammo_segments.len() != desired_segments {
        for segment in overhead.ammo_segments.drain(..) {
            commands.entity(segment).despawn();
        }
        overhead.ammo_fills.clear();
        commands.entity(overhead.ammo_row).with_children(|parent| {
            for _ in 0..desired_segments {
                let mut fill = None;
                let segment = parent
                    .spawn((
                        FighterAmmoSegmentUi,
                        Node {
                            flex_grow: 1.0,
                            height: percent(100.0),
                            overflow: Overflow::clip(),
                            border_radius: BorderRadius::all(px(2.5)),
                            ..default()
                        },
                        BackgroundColor(ammo_segment_color(false)),
                        Name::new("V3 fighter ammunition segment"),
                    ))
                    .with_children(|segment| {
                        fill = Some(
                            segment
                                .spawn((
                                    FighterAmmoSegmentFillUi,
                                    Node {
                                        width: percent(0.0),
                                        height: percent(100.0),
                                        ..default()
                                    },
                                    BackgroundColor(ammo_segment_color(true)),
                                    Name::new("V3 fighter ammunition segment fill"),
                                ))
                                .id(),
                        );
                    })
                    .id();
                overhead.ammo_segments.push(segment);
                overhead
                    .ammo_fills
                    .push(fill.expect("ammunition segment creates one fill"));
            }
        });
    }
    for (index, fill) in overhead.ammo_fills.iter().enumerate() {
        if let Ok(mut node) = fill_nodes.get_mut(*fill) {
            let ratio = match index.cmp(&usize::from(presentation.available)) {
                std::cmp::Ordering::Less => 1.0,
                std::cmp::Ordering::Equal => presentation.recovery_progress,
                std::cmp::Ordering::Greater => 0.0,
            };
            node.width = percent(ratio.clamp(0.0, 1.0) * 100.0);
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "durable status reconciliation reads the authoritative status sources and owns only status visuals"
)]
pub(super) fn reconcile_status_visuals(
    mut commands: Commands,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    fighters: StatusFighterQuery,
    mut statuses: Query<(Entity, &CombatVisualOwner, &StatusVisual3d)>,
) {
    let mut desired_status = HashMap::new();
    for (entity, position, defeated, authoritative_tick, effects, knockback, concealment) in
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
    }

    let existing_status: HashSet<_> = statuses
        .iter()
        .map(|(_, owner, kind)| (owner.0, kind.0))
        .collect();
    for (entity, owner, kind) in &mut statuses {
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
            MeshMaterial3d(match kind {
                StatusKind::Slow => materials.status_slow.clone(),
                StatusKind::Knockback => materials.status_knockback.clone(),
                StatusKind::Reveal => materials.status_reveal.clone(),
            }),
            NotShadowCaster,
            NotShadowReceiver,
            Transform {
                translation: ground_position(position)
                    + Vec3::Y * if kind == StatusKind::Slow { 2.0 } else { 3.0 },
                rotation: Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2),
                scale: Vec3::splat(match kind {
                    StatusKind::Slow => 1.15,
                    StatusKind::Knockback => 0.8,
                    StatusKind::Reveal => 1.8,
                }),
            },
            Name::new("V3 durable combat status"),
        ));
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "dash presentation owns the direct fighter-to-trail lifecycle"
)]
pub(super) fn reconcile_dash_trails(
    mut commands: Commands,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    fighters: DashFighterQuery,
    mut trails: Query<(&mut DashTrailVisual3d, &mut Transform)>,
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

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "aim preview owns controlled-loadout geometry and the complete bounded blocker set"
)]
pub(super) fn update_aim_preview(
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    mut aim: AimPreviewInputs,
    fighters: AimFighterQuery,
    mut previews: PreviewVisualQuery,
) {
    let mut controlled = None;
    let mut dynamic_blockers = Vec::new();
    let controlled_team = fighters
        .iter()
        .find_map(|(_, _, _, _, _, _, team, is_controlled)| is_controlled.then_some(*team));
    for (network_id, position, rotation, defeated, ability, loadout, team, is_controlled) in
        &fighters
    {
        if defeated.is_none()
            && controlled_team.is_some_and(|controlled_team| {
                crate::combat::teams_are_hostile(controlled_team, *team)
            })
        {
            dynamic_blockers.push(AimTraceDynamicBlocker {
                class: AimTraceBlockerClass::Fighter,
                stable_id: u128::from(network_id.0),
                position: position.0,
                rotation: 0.0,
                shape: crate::map::MapShape::Circle {
                    radius: crate::movement::STANDARD_FIGHTER_RADIUS,
                },
            });
        }
        if is_controlled {
            controlled =
                loadout.map(|loadout| (position.0, rotation.as_radians(), loadout, ability));
        }
    }

    append_nonfighter_aim_blockers(
        &aim.sentries,
        &aim.safes,
        controlled_team,
        &mut dynamic_blockers,
    );
    dynamic_blockers.sort_by_key(|blocker| (blocker.class, blocker.stable_id));

    let map = aim
        .maps
        .iter()
        .max_by_key(|(snapshot, _)| snapshot.identity.instance_id);
    if let Some((snapshot, state)) = map {
        aim.index.refresh(snapshot, state, &aim.catalog.0);
    }
    let scan_preview = targeted_ultimate_preview(map, controlled, &aim.pending);
    let segments = if let Some((origin, center, _)) = scan_preview {
        let delta = center - origin;
        vec![PreviewPrimitive {
            geometry: PreviewGeometry::Corridor {
                center: origin.midpoint(center),
                angle: delta.y.atan2(delta.x),
                length: delta.length().max(0.001),
                width: 3.0,
            },
            blocked: false,
        }]
    } else {
        match (map, controlled) {
            (Some((map, state)), Some((origin, facing, loadout, _))) => preview_primitives(
                origin,
                facing,
                aim.pending.aim_distance,
                &loadout.primary_weapon,
                map,
                state,
                &aim.catalog.0,
                &aim.index,
                &dynamic_blockers,
            ),
            _ => Vec::new(),
        }
    };
    apply_aim_preview_visuals(
        &mut previews,
        &primitives,
        &materials,
        scan_preview,
        &segments,
    );
}

fn append_nonfighter_aim_blockers(
    sentries: &SentryAimQuery,
    safes: &SafeAimQuery,
    controlled_team: Option<crate::combat::TeamId>,
    blockers: &mut Vec<AimTraceDynamicBlocker>,
) {
    for (position, identity, health) in sentries.iter() {
        if health.0 > 0
            && controlled_team.is_some_and(|controlled_team| {
                crate::combat::teams_are_hostile(controlled_team, identity.team_id)
            })
        {
            blockers.push(AimTraceDynamicBlocker {
                class: AimTraceBlockerClass::Sentry,
                stable_id: u128::from(identity.deployable_id.0),
                position: position.0,
                rotation: 0.0,
                shape: crate::map::MapShape::Circle {
                    radius: crate::abilities::SENTRY_RADIUS,
                },
            });
        }
    }
    for (position, rotation, safe, health) in safes.iter() {
        if health.0 > 0 {
            blockers.push(AimTraceDynamicBlocker {
                class: AimTraceBlockerClass::HeistSafe,
                stable_id: (safe.match_id.0 << 32) | u128::from(safe.anchor_id.0),
                position: position.0,
                rotation: rotation.as_radians(),
                shape: crate::map::MapShape::Rectangle {
                    half_extents: crate::matchplay::HEIST_SAFE_HALF_EXTENTS,
                },
            });
        }
    }
}

fn targeted_ultimate_preview(
    map: Option<ActivePreviewMap<'_>>,
    controlled: Option<ControlledAim<'_>>,
    pending: &PendingLocalActions,
) -> Option<(Vec2, Vec2, f32)> {
    match (map, controlled) {
        (Some((map, _)), Some((origin, facing, loadout, Some(ability))))
            if matches!(
                loadout.ultimate.kind,
                crate::builds::UltimateKind::RevealScan
                    | crate::builds::UltimateKind::ConcealmentField
            ) && pending.targeted_ultimate.is_targeting(loadout.ultimate.id)
                && matches!(ability.phase, crate::builds::AbilityPhase::Ready) =>
        {
            let (
                crate::builds::UltimateParameters::RevealScan {
                    maximum_range_milliunits,
                    radius_milliunits,
                    ..
                }
                | crate::builds::UltimateParameters::ConcealmentField {
                    maximum_range_milliunits,
                    radius_milliunits,
                    ..
                },
            ) = (loadout.ultimate.parameters,)
            else {
                unreachable!()
            };
            crate::builds::world_units_from_milliunits(maximum_range_milliunits)
                .and_then(|maximum_range| {
                    crate::abilities::reveal_scan_center(
                        origin,
                        Vec2::from_angle(facing),
                        pending.aim_axis,
                        pending.aim_distance,
                        maximum_range,
                        map.dimensions.bounds(),
                    )
                })
                .zip(crate::builds::world_units_from_milliunits(
                    radius_milliunits,
                ))
                .map(|(center, radius)| (origin, center, radius))
        }
        _ => None,
    }
}

fn apply_aim_preview_visuals(
    previews: &mut PreviewVisualQuery,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    scan_preview: Option<(Vec2, Vec2, f32)>,
    segments: &[PreviewPrimitive],
) {
    for (slot, mut transform, mut visibility, mut mesh, mut material) in previews.iter_mut() {
        if slot.slot == u8::MAX {
            let Some((_, center, radius)) = scan_preview else {
                *visibility = Visibility::Hidden;
                continue;
            };
            *visibility = Visibility::Inherited;
            transform.translation = ground_position(center) + Vec3::Y * PREVIEW_HEIGHT;
            transform.rotation = Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2);
            transform.scale = Vec3::splat(radius);
            material.0 = materials.preview.clone();
            continue;
        }
        let Some(primitive) = segments.get(usize::from(slot.slot)) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *visibility = Visibility::Inherited;
        match primitive.geometry {
            PreviewGeometry::Corridor {
                center,
                angle,
                length,
                width,
            } => {
                mesh.0 = primitives.unit_cuboid.clone();
                transform.translation = ground_position(center) + Vec3::Y * PREVIEW_HEIGHT;
                transform.rotation = ground_rotation(Rotation::radians(angle));
                transform.scale = Vec3::new(length, 1.2, width.max(0.001));
            }
            PreviewGeometry::Disc { center, radius } => {
                mesh.0 = primitives.area_disc.clone();
                transform.translation = ground_position(center) + Vec3::Y * PREVIEW_HEIGHT;
                transform.rotation = Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2);
                transform.scale = Vec3::splat(radius);
            }
        }
        material.0 = if primitive.blocked {
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
    mut cues: MessageReader<DeduplicatedCombatCue>,
    mut pending_effects: MessageWriter<PendingCombatEffect>,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    settings: Option<Res<ClientShellSettings>>,
    owners: Query<(Entity, &NetworkEntityId), With<Fighter>>,
    mut visuals: Query<(&CombatVisualOwner, &mut V3FighterVisual)>,
    authoritative_ticks: Query<&AuthoritativeTick>,
) {
    let reduced = settings.is_some_and(|value| value.reduced_combat_effects);
    let current_tick = authoritative_ticks.iter().map(|tick| tick.0).max();
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
        let scan_pulse = matches!(cue, crate::combat::CombatCue::RevealScanActivated { .. });
        let (lifetime, expires_at_tick) = combat_effect_lifetime(cue, current_tick, reduced);
        let transform = if scan_pulse {
            Transform {
                translation: ground_position(position) + Vec3::Y * PREVIEW_HEIGHT,
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
pub(super) fn consume_world_object_cues(
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
                    translation: ground_position(position) + Vec3::Y * PREVIEW_HEIGHT,
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
pub(super) fn consume_pickup_cues(
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
                    translation: ground_position(position) + Vec3::Y * PREVIEW_HEIGHT,
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
pub(super) fn consume_heist_objective_cues(
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
                translation: ground_position(cue.position.as_vec2()) + Vec3::Y * PREVIEW_HEIGHT,
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

pub(super) fn materialize_combat_effects(
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
        } => Some((
            center.as_vec2(),
            materials.scan_area.clone(),
            crate::builds::world_units_from_milliunits(*radius_milliunits).unwrap_or(0.0),
        )),
        C::Muzzle { .. }
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
pub(super) fn cleanup_combat_effects(
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
    mut status_visuals: Query<
        (&CombatVisualOwner, &mut Transform),
        (
            With<StatusVisual3d>,
            Without<V3FighterVisual>,
            Without<V3ProjectileVisual>,
            Without<SentryVisual3d>,
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
        assert!(!fighter_is_visually_concealed(
            Some(&crate::concealment::ConcealmentPresentationState {
                inside_concealing_terrain: false,
                revealed_until_tick: 0,
                ..default()
            }),
            12,
            Some(crate::combat::TeamId(1)),
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
        for (actual, expected) in [color.red, color.green, color.blue]
            .into_iter()
            .zip([0.2, 0.4, 0.8])
        {
            assert!((actual - expected).abs() < f32::EPSILON);
        }
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

    #[test]
    fn focused_combat_visual_state_queries_are_runtime_disjoint() {
        let mut schedule = Schedule::default();
        schedule.add_systems((
            update_fighter_overhead_state,
            reconcile_status_visuals,
            reconcile_dash_trails,
            update_aim_preview,
        ));
        schedule.initialize(&mut World::new()).unwrap();
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
    fn overhead_relation_colors_distinguish_names_and_health() {
        assert_ne!(
            overhead_name_color(GroundMarkerRelation::Local),
            overhead_name_color(GroundMarkerRelation::Ally)
        );
        assert_eq!(
            overhead_health_color(GroundMarkerRelation::Local),
            overhead_health_color(GroundMarkerRelation::Ally)
        );
        assert_ne!(
            overhead_health_color(GroundMarkerRelation::Ally),
            overhead_health_color(GroundMarkerRelation::Enemy)
        );
    }

    #[test]
    fn ammunition_segments_distinguish_available_shots() {
        assert_ne!(ammo_segment_color(true), ammo_segment_color(false));
    }

    #[test]
    fn ammunition_progress_uses_the_replicated_interval_and_clamps() {
        let state = crate::combat::WeaponState {
            ammo: 4,
            phase: crate::combat::WeaponPhase::Ready,
            ammo_recovery: Some(crate::combat::AmmoRecovery {
                started_at_tick: 100,
                ready_at_tick: 178,
            }),
        };
        assert!(ammo_recovery_progress(Some(&state), Some(100)).abs() < f32::EPSILON);
        assert!((ammo_recovery_progress(Some(&state), Some(139)) - 0.5).abs() < f32::EPSILON);
        assert!((ammo_recovery_progress(Some(&state), Some(200)) - 1.0).abs() < f32::EPSILON);
        assert!(ammo_recovery_progress(None, Some(139)).abs() < f32::EPSILON);
    }

    #[test]
    fn compact_overhead_reserves_ammunition_height_only_for_the_local_player() {
        assert!((HEALTH_BAR_WIDTH - 76.8).abs() < f32::EPSILON);
        assert!((PLAYER_NAME_FONT_SIZE - 12.8).abs() < f32::EPSILON);
        assert!((overhead_height(false) - OVERHEAD_HEALTH_HEIGHT).abs() < f32::EPSILON);
        assert!(overhead_height(false) < overhead_height(true));
        assert!((FIGHTER_BODY_WORLD_HEIGHT - KENNEY_CHARACTER_WORLD_HEIGHT).abs() < f32::EPSILON);
        assert!((OVERHEAD_WORLD_HEIGHT - FIGHTER_BODY_WORLD_HEIGHT - 12.0).abs() < f32::EPSILON);
    }

    #[test]
    fn overhead_is_hidden_when_only_its_elevated_anchor_intersects_the_viewport() {
        let viewport = Vec2::new(640.0, 360.0);
        assert_eq!(
            projected_overhead_top_left(
                viewport,
                Vec2::new(650.0, 180.0),
                Vec2::new(620.0, 150.0),
                OVERHEAD_HEALTH_HEIGHT,
            ),
            None
        );
    }

    #[test]
    fn overhead_uses_the_current_projected_anchor_when_the_fighter_is_visible() {
        assert_eq!(
            projected_overhead_top_left(
                Vec2::new(640.0, 360.0),
                Vec2::new(320.0, 180.0),
                Vec2::new(320.0, 150.0),
                OVERHEAD_HEALTH_HEIGHT,
            ),
            Some(Vec2::new(268.0, 113.0))
        );
    }

    #[test]
    fn ground_marker_colors_are_relative_to_the_controlled_fighter() {
        let local_team = Some(crate::combat::TeamId(1));

        assert_eq!(
            ground_marker_relation(crate::combat::TeamId(1), true, local_team),
            GroundMarkerRelation::Local
        );
        assert_eq!(
            ground_marker_relation(crate::combat::TeamId(1), false, local_team),
            GroundMarkerRelation::Ally
        );
        assert_eq!(
            ground_marker_relation(crate::combat::TeamId(0), false, local_team),
            GroundMarkerRelation::Enemy
        );
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
