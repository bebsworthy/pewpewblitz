use super::super::*;
use super::aim_preview::WeaponPreviewVisual3d;
use super::common::{GroundMarkerRelation, ground_marker_relation, unique_roots};
use bevy::{
    asset::RenderAssetUsages,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use std::collections::HashMap;
use std::fmt::Write as _;

const FIGHTER_BODY_WORLD_HEIGHT: f32 = KENNEY_CHARACTER_WORLD_HEIGHT;
const OVERHEAD_WORLD_HEIGHT: f32 = FIGHTER_BODY_WORLD_HEIGHT + 12.0;
const OVERHEAD_WIDTH: f32 = 120.0;
const OVERHEAD_HEALTH_HEIGHT: f32 = 37.0;
const OVERHEAD_AMMO_HEIGHT: f32 = 50.0;
const HEALTH_BAR_WIDTH: f32 = 76.8;
const PLAYER_NAME_FONT_SIZE: f32 = 12.8;
const COLD_PIE_SIZE: f32 = 15.0;
const COLD_PIE_GAP: f32 = 3.0;
const COLD_PIE_TEXTURE_SIZE: u32 = 32;
const COLD_PIE_FRAME_COUNT: usize = 32;

#[derive(Resource)]
pub(in super::super) struct ColdPieAssets {
    frames: Vec<Handle<Image>>,
}

#[derive(Component)]
pub(in super::super) struct FighterOverheadUi {
    visual_root: Entity,
    player_name: Entity,
    health_amount: Entity,
    fill: Entity,
    cold_pie: Entity,
    ammo_row: Entity,
    ammo_segments: Vec<Entity>,
    ammo_fills: Vec<Entity>,
    last_health: Option<u16>,
}

#[derive(Component)]
pub(in super::super) struct FighterHealthFillUi;

#[derive(Component, Default)]
pub(in super::super) struct FighterColdPieUi {
    last_frame: Option<usize>,
}

#[derive(Component)]
pub(in super::super) struct FighterOverheadTextUi;

#[derive(Component)]
pub(in super::super) struct FighterAmmoRowUi;

#[derive(Component)]
pub(in super::super) struct FighterAmmoSegmentUi;

#[derive(Component)]
pub(in super::super) struct FighterAmmoSegmentFillUi;

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
        Option<&'static crate::combat::ActiveEffects>,
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

type AmmoRowQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Visibility,
    (
        With<FighterAmmoRowUi>,
        Without<FighterOverheadUi>,
        Without<FighterColdPieUi>,
    ),
>;

pub(in super::super) fn prepare_cold_pie_assets(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
) {
    let frames = (1..=COLD_PIE_FRAME_COUNT)
        .map(|step| images.add(cold_pie_image(step)))
        .collect();
    commands.insert_resource(ColdPieAssets { frames });
}

#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "the generated 32-pixel UI texture and 32 display steps are exactly bounded"
)]
fn cold_pie_image(step: usize) -> Image {
    let progress = step.min(COLD_PIE_FRAME_COUNT) as f32 / COLD_PIE_FRAME_COUNT as f32;
    let center = (COLD_PIE_TEXTURE_SIZE as f32 - 1.0) * 0.5;
    let outer_radius = center;
    let inner_radius = center - 2.5;
    let mut data = Vec::with_capacity((COLD_PIE_TEXTURE_SIZE * COLD_PIE_TEXTURE_SIZE * 4) as usize);
    for y in 0..COLD_PIE_TEXTURE_SIZE {
        for x in 0..COLD_PIE_TEXTURE_SIZE {
            let offset = Vec2::new(x as f32 - center, y as f32 - center);
            let radius = offset.length();
            let rgba = if radius > outer_radius {
                [0, 0, 0, 0]
            } else if radius > inner_radius {
                [98, 231, 246, 255]
            } else {
                let clockwise_from_top =
                    offset.x.atan2(-offset.y).rem_euclid(core::f32::consts::TAU)
                        / core::f32::consts::TAU;
                if clockwise_from_top <= progress {
                    [91, 225, 244, 255]
                } else {
                    [10, 31, 45, 230]
                }
            };
            data.extend_from_slice(&rgba);
        }
    }
    Image::new(
        Extent3d {
            width: COLD_PIE_TEXTURE_SIZE,
            height: COLD_PIE_TEXTURE_SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
}

fn cold_pie_frame(meter: u16, capacity: u16) -> Option<usize> {
    if meter == 0 || capacity == 0 {
        return None;
    }
    let scaled = usize::from(meter.min(capacity)) * COLD_PIE_FRAME_COUNT;
    Some(
        scaled
            .div_ceil(usize::from(capacity))
            .clamp(1, COLD_PIE_FRAME_COUNT)
            - 1,
    )
}

fn spawn_cold_pie(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            FighterColdPieUi::default(),
            ImageNode::default(),
            Node {
                position_type: PositionType::Absolute,
                left: px((OVERHEAD_WIDTH - HEALTH_BAR_WIDTH) * 0.5 - COLD_PIE_SIZE - COLD_PIE_GAP),
                top: px(22.0),
                width: px(COLD_PIE_SIZE),
                height: px(COLD_PIE_SIZE),
                ..default()
            },
            GlobalZIndex(123),
            Visibility::Hidden,
            Name::new("V3 fighter overhead Cold buildup pie"),
        ))
        .id()
}

#[derive(Clone, Copy)]
struct AmmoPresentation {
    visible: bool,
    capacity: u8,
    available: u8,
    recovery_progress: f32,
}

#[allow(clippy::needless_pass_by_value)]
pub(in super::super) fn reconcile_fighter_overheads(
    mut commands: Commands,
    fighters: Query<Entity, With<Fighter>>,
    fighter_visuals: Query<(Entity, &CombatVisualOwner), With<V3FighterVisual>>,
    overhead_roots: Query<(Entity, &CombatVisualOwner), With<FighterOverheadUi>>,
    overheads: Query<(Entity, &CombatVisualOwner, &FighterOverheadUi)>,
) {
    let roots = unique_roots(&mut commands, &overhead_roots);
    let visual_roots = fighter_visuals
        .iter()
        .map(|(root, owner)| (owner.0, root))
        .collect::<HashMap<_, _>>();

    for fighter in &fighters {
        if !roots.contains(&fighter)
            && let Some(&visual_root) = visual_roots.get(&fighter)
        {
            spawn_fighter_overhead(&mut commands, fighter, visual_root);
        }
    }
    for (root, owner, overhead) in &overheads {
        if fighters.get(owner.0).is_err() || !fighter_visuals.contains(overhead.visual_root) {
            commands.entity(root).despawn();
        }
    }
}

fn spawn_fighter_overhead(commands: &mut Commands, owner: Entity, visual_root: Entity) {
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
    let cold_pie = spawn_cold_pie(commands);
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
                visual_root,
                player_name,
                health_amount,
                fill,
                cold_pie,
                ammo_row,
                ammo_segments: Vec::new(),
                ammo_fills: Vec::new(),
                last_health: None,
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
            cold_pie,
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
pub(in super::super) fn project_fighter_overhead_ui(
    cameras: Query<(&Camera, &GlobalTransform), With<ArenaCamera>>,
    fighters: Query<
        (
            &crate::combat::CurrentHealth,
            Option<&crate::combat::Defeated>,
        ),
        With<Fighter>,
    >,
    fighter_visuals: Query<&GlobalTransform, With<V3FighterVisual>>,
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
        let Ok(visual_transform) = fighter_visuals.get(overhead.visual_root) else {
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
pub(in super::super) fn update_fighter_overhead_state(
    mut commands: Commands,
    definitions: Res<crate::combat::FighterDefinitions>,
    fighters: OverheadFighterQuery,
    mut overhead_roots: Query<
        (&CombatVisualOwner, &mut FighterOverheadUi, &mut Visibility),
        (Without<WeaponPreviewVisual3d>, Without<FighterColdPieUi>),
    >,
    mut fill_nodes: AmmoFillQuery,
    mut overhead_texts: Query<(&mut Text, &mut TextColor), With<FighterOverheadTextUi>>,
    mut overhead_colors: Query<
        &mut BackgroundColor,
        Or<(With<FighterHealthFillUi>, With<FighterAmmoSegmentUi>)>,
    >,
    mut ammo_rows: AmmoRowQuery,
    cold_pie_assets: Res<ColdPieAssets>,
    mut cold_pies: Query<
        (&mut FighterColdPieUi, &mut ImageNode, &mut Visibility),
        (
            With<FighterColdPieUi>,
            Without<FighterOverheadUi>,
            Without<FighterAmmoRowUi>,
        ),
    >,
) {
    let controlled_team =
        fighters
            .iter()
            .find_map(|(_, _, _, _, _, _, _, team, _, _, is_controlled)| {
                is_controlled.then_some(*team)
            });
    for (owner, mut overhead, mut visibility) in &mut overhead_roots {
        let Ok((
            _,
            health,
            definition,
            defeated,
            authoritative_tick,
            loadout,
            active_effects,
            team,
            display_name,
            weapon,
            is_controlled,
        )) = fighters.get(owner.0)
        else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let maximum = loadout.map_or_else(
            || {
                definitions
                    .get(*definition)
                    .map_or(1, |value| value.maximum_health)
            },
            |value| value.fighter_stats.maximum_health,
        );
        let observed_tick = authoritative_tick.map(|tick| tick.0);
        let current = health.0;
        let defeated = defeated.is_some();
        let relation = ground_marker_relation(*team, is_controlled, controlled_team);
        let name = display_name.map_or("Player", |name| name.0.as_str());
        let ammo = weapon.map_or(0, |state| state.ammo);
        let capacity = loadout.map_or(0, |value| value.primary_weapon.recipe.economy.capacity());
        let ammo_progress = ammo_recovery_progress(weapon, observed_tick);
        // State reconciliation may hide roots; projection alone reveals correctly positioned ones.
        if defeated {
            *visibility = Visibility::Hidden;
        }
        let ratio = (f32::from(current) / f32::from(maximum.max(1))).clamp(0.0, 1.0);
        if let Ok(mut fill) = fill_nodes.get_mut(overhead.fill) {
            fill.width = percent(ratio * 100.0);
        }
        if let Ok(mut color) = overhead_colors.get_mut(overhead.fill) {
            color.0 = overhead_health_color(relation);
        }
        if let Ok((mut text, mut color)) = overhead_texts.get_mut(overhead.player_name) {
            if text.0 != name {
                text.0.clear();
                text.0.push_str(name);
            }
            color.0 = overhead_name_color(relation);
        }
        if overhead.last_health != Some(current) {
            if let Ok((mut text, _)) = overhead_texts.get_mut(overhead.health_amount) {
                text.0.clear();
                write!(&mut text.0, "{current}").expect("writing health to String cannot fail");
            }
            overhead.last_health = Some(current);
        }

        let cold_frame = loadout.and_then(|loadout| {
            cold_pie_frame(
                active_effects.map_or(0, |effects| effects.cold.meter),
                loadout.fighter_stats.cold_capacity,
            )
        });
        if let Ok((mut pie, mut image, mut pie_visibility)) = cold_pies.get_mut(overhead.cold_pie) {
            *pie_visibility = if cold_frame.is_some() {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if pie.last_frame != cold_frame
                && let Some(frame) = cold_frame
            {
                image.image = cold_pie_assets.frames[frame].clone();
            }
            pie.last_frame = cold_frame;
        }

        reconcile_overhead_ammunition(
            &mut commands,
            &mut overhead,
            &mut ammo_rows,
            &mut fill_nodes,
            AmmoPresentation {
                visible: relation == GroundMarkerRelation::Local && capacity > 0,
                capacity,
                available: ammo,
                recovery_progress: ammo_progress,
            },
        );
    }
}

fn reconcile_overhead_ammunition(
    commands: &mut Commands,
    overhead: &mut FighterOverheadUi,
    ammo_rows: &mut AmmoRowQuery,
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

#[cfg(test)]
mod tests {
    use super::*;
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
        let health_left = (OVERHEAD_WIDTH - HEALTH_BAR_WIDTH) * 0.5;
        let pie_right = health_left - COLD_PIE_GAP;
        assert!(pie_right <= health_left);
        assert!(pie_right - COLD_PIE_SIZE >= 0.0);
    }

    #[test]
    fn cold_pie_is_hidden_without_buildup_and_quantizes_against_target_capacity() {
        assert_eq!(cold_pie_frame(0, 1_000), None);
        assert_eq!(cold_pie_frame(125, 0), None);
        assert_eq!(cold_pie_frame(1, 1_000), Some(0));
        assert_eq!(cold_pie_frame(500, 1_000), Some(15));
        assert_eq!(cold_pie_frame(1_000, 1_000), Some(31));
        assert_eq!(cold_pie_frame(2_000, 1_000), Some(31));
        assert_eq!(cold_pie_frame(375, 750), Some(15));
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
            Some(Vec2::new(260.0, 113.0))
        );
    }
}
