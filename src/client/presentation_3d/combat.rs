//! Complete 3D combat presentation over independent, client-only visual roots.

use super::*;
use crate::combat::client::{DeduplicatedCombatCue, MAX_PREVIEW_SEGMENTS, preview_segments};
use std::collections::{BTreeMap, HashMap, HashSet};

const PREVIEW_HEIGHT: f32 = 2.5;
const OVERHEAD_WORLD_HEIGHT: f32 = 86.0;
const OVERHEAD_WIDTH: f32 = 104.0;
const OVERHEAD_HEALTH_HEIGHT: f32 = 37.0;
const OVERHEAD_AMMO_HEIGHT: f32 = 50.0;
const HEALTH_BAR_WIDTH: f32 = 76.8;
const PLAYER_NAME_FONT_SIZE: f32 = 12.8;
const FIGHTER_BODY_WORLD_HEIGHT: f32 = 24.0;
const GROUND_MARKER_HEIGHT: f32 = 1.0;
const MAX_EFFECTS: usize = 96;

#[derive(Component)]
pub(super) struct SentryVisual3d;

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

#[derive(Component)]
pub(super) struct FighterOverheadUi {
    player_name: Entity,
    health_amount: Entity,
    fill: Entity,
    ammo_row: Entity,
    ammo_segments: Vec<Entity>,
}

#[derive(Component)]
pub(super) struct FighterHealthFillUi;

#[derive(Component)]
pub(super) struct FighterOverheadTextUi;

#[derive(Component)]
pub(super) struct FighterAmmoRowUi;

#[derive(Component)]
pub(super) struct FighterAmmoSegmentUi;

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
    fighters: FighterPresentationQuery,
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
    overhead_visuals: Query<(Entity, &CombatVisualOwner), With<FighterOverheadUi>>,
    trails: Query<(Entity, &CombatVisualOwner), With<DashTrailVisual3d>>,
    statuses: Query<(Entity, &CombatVisualOwner), With<StatusVisual3d>>,
    previews: Query<&WeaponPreviewVisual3d>,
    mut ground_markers: GroundMarkerQuery,
) {
    let fighter_roots = unique_roots(&mut commands, &fighter_visuals);
    let projectile_roots = unique_roots(&mut commands, &projectile_visuals);
    let sentry_roots = unique_roots(&mut commands, &sentry_visuals);
    let overhead_roots = unique_roots(&mut commands, &overhead_visuals);
    let controlled_team = fighters
        .iter()
        .find_map(|(_, _, team, controlled)| controlled.then_some(*team));

    update_ground_markers(&fighters, &mut ground_markers, controlled_team, &materials);

    for (owner, position, team, controlled) in &fighters {
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

    if previews.iter().count() == 0 {
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
    }
}

fn update_ground_markers(
    fighters: &FighterPresentationQuery,
    ground_markers: &mut GroundMarkerQuery,
    controlled_team: Option<crate::combat::TeamId>,
    materials: &Material3dAssets,
) {
    for (marker, mut material) in ground_markers {
        if let Ok((_, _, team, controlled)) = fighters.get(marker.owner) {
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
            Transform::from_xyz(0.0, 24.0, 0.0),
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
            NotShadowCaster,
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
            &crate::combat::TeamId,
            Option<&crate::matchplay::FighterDisplayName>,
            Option<&crate::combat::WeaponState>,
            Has<Controlled>,
        ),
        With<Fighter>,
    >,
    mut overhead_roots: Query<
        (&CombatVisualOwner, &mut FighterOverheadUi, &mut Visibility),
        Without<WeaponPreviewVisual3d>,
    >,
    mut fill_nodes: Query<&mut Node, With<FighterHealthFillUi>>,
    mut overhead_texts: Query<(&mut Text, &mut TextColor), With<FighterOverheadTextUi>>,
    mut overhead_colors: Query<
        &mut BackgroundColor,
        Or<(With<FighterHealthFillUi>, With<FighterAmmoSegmentUi>)>,
    >,
    mut ammo_rows: Query<&mut Visibility, (With<FighterAmmoRowUi>, Without<FighterOverheadUi>)>,
    mut statuses: Query<(Entity, &CombatVisualOwner, &StatusVisual3d)>,
    mut trails: Query<
        (
            Entity,
            &CombatVisualOwner,
            &mut DashTrailVisual3d,
            &mut Transform,
        ),
        (Without<FighterHealthFillUi>, Without<WeaponPreviewVisual3d>),
    >,
    mut previews: Query<
        (
            &WeaponPreviewVisual3d,
            &mut Transform,
            &mut Visibility,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        (
            Without<FighterHealthFillUi>,
            Without<DashTrailVisual3d>,
            Without<FighterOverheadUi>,
            Without<FighterAmmoRowUi>,
        ),
    >,
) {
    let mut desired_status = HashSet::new();
    let mut fighter_data = HashMap::new();
    let mut controlled = None;
    let controlled_team = fighters.iter().find_map(
        |(_, _, _, _, _, _, _, _, _, _, _, team, _, _, is_controlled)| {
            is_controlled.then_some(*team)
        },
    );
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
        fighter_data.insert(
            entity,
            (
                position.0,
                health.0,
                maximum,
                defeated.is_some(),
                ground_marker_relation(*team, is_controlled, controlled_team),
                display_name.map_or("Player", |name| name.0.as_str()),
                weapon.map_or(0, |state| state.ammo),
                loadout.map_or(0, |value| value.primary_weapon.recipe.economy.capacity()),
            ),
        );
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
                    NotShadowCaster,
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

    for (owner, mut overhead, mut visibility) in &mut overhead_roots {
        let Some((_, current, maximum, defeated, relation, name, ammo, capacity)) =
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
            if text.0 != *name {
                text.0 = (*name).to_string();
            }
            color.0 = overhead_name_color(*relation);
        }
        if let Ok((mut text, _)) = overhead_texts.get_mut(overhead.health_amount) {
            let amount = current.to_string();
            if text.0 != amount {
                text.0 = amount;
            }
        }

        let show_ammo = *relation == GroundMarkerRelation::Local && *capacity > 0;
        if let Ok(mut ammo_visibility) = ammo_rows.get_mut(overhead.ammo_row) {
            *ammo_visibility = if show_ammo {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
        let desired_segments = if show_ammo { usize::from(*capacity) } else { 0 };
        if overhead.ammo_segments.len() != desired_segments {
            for segment in overhead.ammo_segments.drain(..) {
                commands.entity(segment).despawn();
            }
            commands.entity(overhead.ammo_row).with_children(|parent| {
                for _ in 0..desired_segments {
                    let segment = parent
                        .spawn((
                            FighterAmmoSegmentUi,
                            Node {
                                flex_grow: 1.0,
                                height: percent(100.0),
                                border_radius: BorderRadius::all(px(2.5)),
                                ..default()
                            },
                            BackgroundColor(ammo_segment_color(false)),
                            Name::new("V3 fighter ammunition segment"),
                        ))
                        .id();
                    overhead.ammo_segments.push(segment);
                }
            });
        }
        for (index, segment) in overhead.ammo_segments.iter().enumerate() {
            if let Ok(mut color) = overhead_colors.get_mut(*segment) {
                color.0 = ammo_segment_color(index < usize::from(*ammo));
            }
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
            NotShadowCaster,
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
            NotShadowCaster,
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

    #[test]
    fn combat_visual_state_queries_are_runtime_disjoint() {
        let mut schedule = Schedule::default();
        schedule.add_systems(update_combat_visual_state);
        schedule.initialize(&mut World::new()).unwrap();
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
    fn compact_overhead_reserves_ammunition_height_only_for_the_local_player() {
        assert!((HEALTH_BAR_WIDTH - 76.8).abs() < f32::EPSILON);
        assert!((PLAYER_NAME_FONT_SIZE - 12.8).abs() < f32::EPSILON);
        assert!((overhead_height(false) - OVERHEAD_HEALTH_HEIGHT).abs() < f32::EPSILON);
        assert!(overhead_height(false) < overhead_height(true));
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
