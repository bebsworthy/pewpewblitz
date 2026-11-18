#[allow(
    clippy::wildcard_imports,
    reason = "the world-renderer submodule consumes the presentation_3d composition surface"
)]
use super::super::*;
use super::common::{GroundMarkerRelation, ground_marker_relation, unique_roots};
use bevy::{
    asset::RenderAssetUsages,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use std::collections::HashMap;
use std::fmt::Write as _;

const FIGHTER_BODY_WORLD_HEIGHT: f32 = FIGHTER_VISUAL_HEIGHT_WORLD;
/// World height of the overhead plate's bottom-center anchor above the fighter's feet.
pub(in super::super) const OVERHEAD_WORLD_HEIGHT: f32 = FIGHTER_BODY_WORLD_HEIGHT + 12.0;
// Logical-pixel plate sized for the BRL-0148 framing, where a fighter stands about 80 px tall at
// the 720 px reference height: a bold 22 px bar with the health number inside it.
const OVERHEAD_WIDTH: f32 = 140.0;
const PLAYER_NAME_FONT_SIZE: f32 = 15.0;
/// Clear space kept between the name and each plate edge, covering the drop-shadow offset and
/// sub-pixel differences between this measurement and the scaled render layout.
const PLAYER_NAME_SIDE_PADDING: f32 = 4.0;
/// Widest laid-out player name that stays inside its own fixed-width plate (BRL-9k7q).
const PLAYER_NAME_MAX_WIDTH: f32 = OVERHEAD_WIDTH - 2.0 * PLAYER_NAME_SIDE_PADDING;
/// Lilita One maps U+2026, so truncation uses the single-glyph ellipsis.
const PLAYER_NAME_ELLIPSIS: &str = "\u{2026}";
const PLAYER_NAME_HEIGHT: f32 = 19.0;
/// Fixed width of the BOT chip shown before a held seat's name (BRL-dwqx), so the name's fit
/// budget is known without measuring the chip.
const BOT_CHIP_WIDTH: f32 = 30.0;
const BOT_CHIP_HEIGHT: f32 = 15.0;
const BOT_CHIP_FONT_SIZE: f32 = 11.0;
/// Space between the BOT chip and the name.
const BOT_CHIP_GAP: f32 = 4.0;
/// Widest fitted name beside the BOT chip, so chip plus name stay inside the plate.
const HELD_PLAYER_NAME_MAX_WIDTH: f32 = PLAYER_NAME_MAX_WIDTH - BOT_CHIP_WIDTH - BOT_CHIP_GAP;
const HEALTH_BAR_WIDTH: f32 = 96.0;
const HEALTH_BAR_HEIGHT: f32 = 22.0;
const HEALTH_BAR_OUTLINE: f32 = 2.0;
const HEALTH_AMOUNT_FONT_SIZE: f32 = 18.0;
const AMMO_ROW_GAP: f32 = 3.0;
const AMMO_ROW_HEIGHT: f32 = 9.0;
const OVERHEAD_HEALTH_HEIGHT: f32 = PLAYER_NAME_HEIGHT + HEALTH_BAR_HEIGHT;
/// UI height of the local fighter's overhead (name, health bar, and ammunition row).
pub(in super::super) const OVERHEAD_AMMO_HEIGHT: f32 =
    OVERHEAD_HEALTH_HEIGHT + AMMO_ROW_GAP + AMMO_ROW_HEIGHT;
const OVERHEAD_STACK_GAP: f32 = 2.0;
const COLD_PIE_SIZE: f32 = 18.0;
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
    bot_chip: Entity,
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

/// The full replicated display name behind an overhead name label. The label shows it fitted to
/// the plate; fitting reruns only when the name changes (or until the display font is loaded).
#[derive(Component, Default)]
pub(in super::super) struct FighterOverheadNameUi {
    full: String,
    /// The BOT chip shares the name row, so a held seat's name fits a narrower budget.
    held: bool,
    fitted: bool,
}

/// The compact BOT chip before the name of a fighter whose held seat a server bot drives.
#[derive(Component)]
pub(in super::super) struct FighterBotChipUi;

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
        &'static brawler::combat::CurrentHealth,
        Option<&'static brawler::combat::Defeated>,
        Option<&'static AuthoritativeTick>,
        Option<&'static brawler::builds::ResolvedMatchLoadout>,
        Option<&'static brawler::combat::ActiveEffects>,
        &'static brawler::combat::TeamId,
        Option<&'static brawler::matchplay::FighterDisplayName>,
        Option<&'static brawler::combat::WeaponState>,
        Option<&'static brawler::matchplay::BoostStacks>,
        Has<Controlled>,
        Option<&'static NetworkEntityId>,
    ),
    With<Fighter>,
>;

type BotChipQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Node,
    (
        With<FighterBotChipUi>,
        Without<FighterHealthFillUi>,
        Without<FighterAmmoSegmentFillUi>,
    ),
>;

/// The overhead name labels and BOT chips, and the public roster that says which seats a server
/// bot holds (BRL-dwqx).
#[derive(bevy::ecs::system::SystemParam)]
pub(in super::super) struct OverheadNames<'w, 's> {
    labels: Query<'w, 's, &'static mut FighterOverheadNameUi>,
    participants: Query<'w, 's, &'static brawler::matchplay::PublicParticipantState>,
    chips: BotChipQuery<'w, 's>,
}

impl OverheadNames<'_, '_> {
    /// Fighter network IDs whose seats the server publishes as held by a bot.
    fn held_fighters(&self) -> Vec<u64> {
        self.participants
            .iter()
            .filter(|participant| {
                matches!(
                    participant.control,
                    brawler::matchplay::ParticipantControl::HeldByBot { .. }
                )
            })
            .map(|participant| participant.fighter_network_id.0)
            .collect()
    }

    /// Shows the BOT chip for a held seat and refits the name when it or the chip changed.
    fn present(&mut self, overhead: &FighterOverheadUi, name: &str, held: bool) {
        if let Ok(mut chip) = self.chips.get_mut(overhead.bot_chip) {
            let display = if held { Display::Flex } else { Display::None };
            if chip.display != display {
                chip.display = display;
            }
        }
        if let Ok(mut label) = self.labels.get_mut(overhead.player_name)
            && (label.full != name || label.held != held)
        {
            label.full.clear();
            label.full.push_str(name);
            label.held = held;
            label.fitted = false;
        }
    }
}

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
                top: px(PLAYER_NAME_HEIGHT + (HEALTH_BAR_HEIGHT - COLD_PIE_SIZE) * 0.5),
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
    asset_server: Option<Res<AssetServer>>,
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
            spawn_fighter_overhead(
                &mut commands,
                fighter,
                visual_root,
                &ui::theme::display_font_handle(asset_server.as_deref()),
            );
        }
    }
    for (root, owner, overhead) in &overheads {
        if fighters.get(owner.0).is_err() || !fighter_visuals.contains(overhead.visual_root) {
            commands.entity(root).despawn();
        }
    }
}

fn spawn_fighter_overhead(
    commands: &mut Commands,
    owner: Entity,
    visual_root: Entity,
    font: &Handle<Font>,
) {
    let (player_name_container, player_name, bot_chip) = spawn_overhead_name_row(commands, font);
    // The number sits centered inside the bar, above the fill.
    let (health_amount_container, health_amount) = spawn_overhead_text(
        commands,
        font,
        PLAYER_NAME_HEIGHT,
        HEALTH_BAR_HEIGHT,
        HEALTH_AMOUNT_FONT_SIZE,
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
                top: px(PLAYER_NAME_HEIGHT),
                width: px(HEALTH_BAR_WIDTH),
                height: px(HEALTH_BAR_HEIGHT),
                border: UiRect::all(px(HEALTH_BAR_OUTLINE)),
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(px(7.0)),
                ..default()
            },
            BorderColor::all(ui::theme::INK),
            BackgroundColor(Color::srgb(0.05, 0.06, 0.11)),
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
                top: px(OVERHEAD_HEALTH_HEIGHT + AMMO_ROW_GAP),
                width: px(HEALTH_BAR_WIDTH),
                height: px(AMMO_ROW_HEIGHT),
                column_gap: px(3.0),
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
                bot_chip,
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
    font: &Handle<Font>,
    top: f32,
    height: f32,
    font_size: f32,
    name: &'static str,
) -> (Entity, Entity) {
    let text = commands
        .spawn((
            FighterOverheadTextUi,
            Text::new(""),
            ui::theme::hud_text(font, font_size),
            TextColor(Color::WHITE),
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
                column_gap: px(BOT_CHIP_GAP),
                // Backstop for text the fitter has not measured yet: nothing paints past the
                // fixed plate width that overlap resolution assumes.
                overflow: Overflow::clip_x(),
                ..default()
            },
            GlobalZIndex(122),
            Name::new(format!("{name} centered container")),
        ))
        .add_child(text)
        .id();
    (container, text)
}

/// The centered name row: the fitted name label, preceded by the hidden BOT chip. Returns the row
/// container, the name label and the chip.
fn spawn_overhead_name_row(
    commands: &mut Commands,
    font: &Handle<Font>,
) -> (Entity, Entity, Entity) {
    let (container, player_name) = spawn_overhead_text(
        commands,
        font,
        0.0,
        PLAYER_NAME_HEIGHT,
        PLAYER_NAME_FONT_SIZE,
        "V3 fighter overhead player name",
    );
    commands
        .entity(player_name)
        .insert(FighterOverheadNameUi::default());
    let bot_chip = spawn_bot_chip(commands, font);
    commands.entity(container).insert_children(0, &[bot_chip]);
    (container, player_name, bot_chip)
}

/// The hidden BOT chip; it takes no row space until a held seat shows it.
fn spawn_bot_chip(commands: &mut Commands, font: &Handle<Font>) -> Entity {
    commands
        .spawn((
            FighterBotChipUi,
            Node {
                display: Display::None,
                width: px(BOT_CHIP_WIDTH),
                height: px(BOT_CHIP_HEIGHT),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.16, 0.18, 0.26, 0.92)),
            Name::new("V3 fighter overhead BOT chip"),
        ))
        .with_child((
            Text::new("BOT"),
            ui::theme::hud_text(font, BOT_CHIP_FONT_SIZE),
            TextColor(Color::srgb(0.84, 0.87, 0.94)),
            TextLayout::new(Justify::Center, LineBreak::NoWrap),
        ))
        .id()
}

/// A held seat's name is the relation color toward gray, so the BOT chip reads first.
fn held_overhead_name_color(relation: GroundMarkerRelation) -> Color {
    use bevy::color::Mix as _;
    overhead_name_color(relation).mix(&Color::srgb(0.62, 0.64, 0.7), 0.55)
}

fn overhead_name_color(relation: GroundMarkerRelation) -> Color {
    match relation {
        GroundMarkerRelation::Local => Color::srgb(0.18, 0.95, 0.36),
        GroundMarkerRelation::Ally => Color::srgb(0.12, 0.72, 0.96),
        GroundMarkerRelation::Enemy => Color::srgb(1.0, 0.18, 0.14),
    }
}

/// Relation-colored fill matching the ground rings: local green, ally blue, enemy red.
fn overhead_health_color(relation: GroundMarkerRelation) -> Color {
    match relation {
        GroundMarkerRelation::Local => Color::srgb(0.18, 0.92, 0.34),
        GroundMarkerRelation::Ally => Color::srgb(0.12, 0.66, 0.98),
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
    state: Option<&brawler::combat::WeaponState>,
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

/// One projected overhead plate awaiting collision resolution.
#[derive(Clone, Copy, Debug, PartialEq)]
struct OverheadPlacement {
    root: Entity,
    top_left: Vec2,
    height: f32,
    is_local: bool,
}

/// Stacks overlapping overhead plates upward so nearby fighters keep readable labels. The local
/// fighter's plate keeps its projected anchor; the others resolve in a stable order (nearer the
/// bottom of the screen first, then entity order) and each moves directly above any plate it would
/// overlap. Every move is strictly upward past one already-placed plate, so resolution is bounded.
fn resolve_overhead_overlaps(placements: &mut [OverheadPlacement]) {
    placements.sort_by(|a, b| {
        b.is_local
            .cmp(&a.is_local)
            .then_with(|| b.top_left.y.total_cmp(&a.top_left.y))
            .then_with(|| a.root.cmp(&b.root))
    });
    for index in 1..placements.len() {
        let (placed, rest) = placements.split_at_mut(index);
        let plate = &mut rest[0];
        for _ in 0..placed.len() {
            let Some(blocking) = placed
                .iter()
                .filter(|other| overhead_plates_overlap(plate, other))
                .map(|other| other.top_left.y)
                .reduce(f32::min)
            else {
                break;
            };
            plate.top_left.y = blocking - plate.height - OVERHEAD_STACK_GAP;
        }
    }
}

fn overhead_plates_overlap(a: &OverheadPlacement, b: &OverheadPlacement) -> bool {
    (a.top_left.x - b.top_left.x).abs() < OVERHEAD_WIDTH
        && a.top_left.y < b.top_left.y + b.height + OVERHEAD_STACK_GAP
        && b.top_left.y < a.top_left.y + a.height + OVERHEAD_STACK_GAP
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "the projection phase reads the propagated world camera and writes absolute UI nodes"
)]
pub(in super::super) fn project_fighter_overhead_ui(
    cameras: Query<(&Camera, &GlobalTransform), With<ArenaCamera>>,
    fighters: Query<
        (
            &brawler::combat::CurrentHealth,
            Option<&brawler::combat::Defeated>,
            Has<Controlled>,
        ),
        With<Fighter>,
    >,
    fighter_visuals: Query<&GlobalTransform, With<V3FighterVisual>>,
    mut overheads: Query<
        (
            Entity,
            &CombatVisualOwner,
            &FighterOverheadUi,
            &mut Node,
            &mut Visibility,
        ),
        With<FighterOverheadUi>,
    >,
    ui_scale: Option<Res<UiScale>>,
) {
    let to_ui = |point| ui::viewport::viewport_to_ui(point, ui_scale.as_deref());
    let Some((camera, camera_transform, viewport_size)) =
        cameras.single().ok().and_then(|(camera, transform)| {
            camera
                .logical_viewport_size()
                .map(|size| (camera, transform, to_ui(size)))
        })
    else {
        for (_, _, _, _, mut visibility) in &mut overheads {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let mut placements = Vec::new();
    for (root, owner, overhead, _, mut visibility) in &mut overheads {
        let placement = fighters
            .get(owner.0)
            .ok()
            .filter(|(health, defeated, _)| {
                !character_is_visually_defeated(health.0, defeated.is_some())
            })
            .and_then(|(_, _, is_local)| {
                let ground_position = fighter_visuals
                    .get(overhead.visual_root)
                    .ok()?
                    .translation();
                let fighter_viewport = to_ui(
                    camera
                        .world_to_viewport(
                            camera_transform,
                            ground_position + Vec3::Y * FIGHTER_BODY_WORLD_HEIGHT,
                        )
                        .ok()?,
                );
                let overhead_viewport = to_ui(
                    camera
                        .world_to_viewport(
                            camera_transform,
                            ground_position + Vec3::Y * OVERHEAD_WORLD_HEIGHT,
                        )
                        .ok()?,
                );
                let height = overhead_height(!overhead.ammo_segments.is_empty());
                projected_overhead_top_left(
                    viewport_size,
                    fighter_viewport,
                    overhead_viewport,
                    height,
                )
                .map(|top_left| OverheadPlacement {
                    root,
                    top_left,
                    height,
                    is_local,
                })
            });
        match placement {
            Some(placement) => placements.push(placement),
            None => *visibility = Visibility::Hidden,
        }
    }
    resolve_overhead_overlaps(&mut placements);
    for placement in placements {
        let Ok((_, _, _, mut node, mut visibility)) = overheads.get_mut(placement.root) else {
            continue;
        };
        node.left = px(placement.top_left.x);
        node.top = px(placement.top_left.y);
        node.height = px(placement.height);
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
    fighters: OverheadFighterQuery,
    mut overhead_roots: Query<
        (&CombatVisualOwner, &mut FighterOverheadUi, &mut Visibility),
        Without<FighterColdPieUi>,
    >,
    mut fill_nodes: AmmoFillQuery,
    mut overhead_texts: Query<(&mut Text, &mut TextColor), With<FighterOverheadTextUi>>,
    mut overhead_names: OverheadNames,
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
            .find_map(|(_, _, _, _, _, _, team, _, _, _, is_controlled, _)| {
                is_controlled.then_some(*team)
            });
    let held_fighters = overhead_names.held_fighters();
    for (owner, mut overhead, mut visibility) in &mut overhead_roots {
        let Ok((
            _,
            health,
            defeated,
            authoritative_tick,
            loadout,
            active_effects,
            team,
            display_name,
            weapon,
            boost_stacks,
            is_controlled,
            network_id,
        )) = fighters.get(owner.0)
        else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Some(loadout) = loadout else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let maximum = brawler::matchplay::effective_maximum_health(
            loadout.fighter_stats.maximum_health,
            boost_stacks.copied().unwrap_or_default(),
        );
        let observed_tick = authoritative_tick.map(|tick| tick.0);
        let current = health.0;
        let defeated = defeated.is_some();
        let relation = ground_marker_relation(*team, is_controlled, controlled_team);
        let name = display_name.map_or("Player", |name| name.0.as_str());
        let ammo = weapon.map_or(0, |state| state.ammo);
        let capacity = loadout.primary_weapon.recipe.economy.capacity();
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
        // Never on the local fighter: a rejoined player's own seat is Player by construction.
        let held =
            !is_controlled && network_id.is_some_and(|network| held_fighters.contains(&network.0));
        overhead_names.present(&overhead, name, held);
        if let Ok((_, mut color)) = overhead_texts.get_mut(overhead.player_name) {
            color.set_if_neq(TextColor(if held {
                held_overhead_name_color(relation)
            } else {
                overhead_name_color(relation)
            }));
        }
        if overhead.last_health != Some(current) {
            if let Ok((mut text, _)) = overhead_texts.get_mut(overhead.health_amount) {
                text.0.clear();
                write!(&mut text.0, "{current}").expect("writing health to String cannot fail");
            }
            overhead.last_health = Some(current);
        }

        let cold_frame = cold_pie_frame(
            active_effects.map_or(0, |effects| effects.cold.meter),
            loadout.fighter_stats.cold_capacity,
        );
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

/// Shows each changed overhead name fitted to its plate, measuring candidates with the same parley
/// font collection (display font plus system fallbacks) that lays the label out.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are passed by value"
)]
pub(in super::super) fn fit_overhead_player_names(
    fonts: Res<Assets<Font>>,
    mut font_cx: ResMut<bevy::text::FontCx>,
    mut layout_cx: ResMut<bevy::text::LayoutCx>,
    mut names: Query<(&mut FighterOverheadNameUi, &mut Text, &TextFont)>,
) {
    for (mut label, mut text, text_font) in &mut names {
        if label.fitted {
            continue;
        }
        // Until the display font is registered, parley would measure a fallback face instead.
        let Some(family) = registered_font_family(&fonts, &mut font_cx, text_font) else {
            continue;
        };
        let font_size = text_font.font_size.eval(Vec2::ZERO, PLAYER_NAME_FONT_SIZE);
        let budget = if label.held {
            HELD_PLAYER_NAME_MAX_WIDTH
        } else {
            PLAYER_NAME_MAX_WIDTH
        };
        let shown = fit_overhead_name(&label.full, budget, |candidate| {
            measure_text_width(&mut font_cx, &mut layout_cx, &family, font_size, candidate)
        });
        if text.0 != shown {
            text.0 = shown;
        }
        label.fitted = true;
    }
}

fn registered_font_family(
    fonts: &Assets<Font>,
    font_cx: &mut bevy::text::FontCx,
    text_font: &TextFont,
) -> Option<String> {
    let FontSource::Handle(handle) = &text_font.font else {
        return None;
    };
    let alias = &fonts.get(handle.id())?.alias;
    (!alias.is_empty() && font_cx.collection.family_id(alias).is_some()).then(|| alias.clone())
}

/// Logical-pixel advance width of `text` on one unwrapped line, as `bevy_text` would shape it.
fn measure_text_width(
    font_cx: &mut bevy::text::FontCx,
    layout_cx: &mut bevy::text::LayoutCx,
    family: &str,
    font_size: f32,
    text: &str,
) -> f32 {
    use parley::{FontFamily, StyleProperty, TextWrapMode};
    let mut builder = layout_cx
        .0
        .ranged_builder(&mut font_cx.context, text, 1.0, true);
    builder.push_default(StyleProperty::FontFamily(FontFamily::named(family)));
    builder.push_default(StyleProperty::FontSize(font_size));
    builder.push_default(StyleProperty::TextWrapMode(TextWrapMode::NoWrap));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    layout.full_width()
}

/// Returns `name` unchanged when `measure` says it fits in `max_width`; otherwise the longest
/// grapheme prefix that fits with a trailing ellipsis. `measure` returns a laid-out width, so
/// emoji and fallback-font glyphs count at their real size.
fn fit_overhead_name(name: &str, max_width: f32, mut measure: impl FnMut(&str) -> f32) -> String {
    use unicode_segmentation::UnicodeSegmentation as _;
    if measure(name) <= max_width {
        return name.to_owned();
    }
    let boundaries = name
        .grapheme_indices(true)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let candidate = |graphemes: usize| {
        let mut shown = name[..boundaries[graphemes]].trim_end().to_owned();
        shown.push_str(PLAYER_NAME_ELLIPSIS);
        shown
    };
    // Widths grow with the prefix, so binary-search the longest fitting grapheme count.
    let (mut fits, mut too_wide) = (0, boundaries.len());
    while too_wide - fits > 1 {
        let middle = usize::midpoint(fits, too_wide);
        if measure(&candidate(middle)) <= max_width {
            fits = middle;
        } else {
            too_wide = middle;
        }
    }
    candidate(fits)
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
                            border: UiRect::all(px(1.0)),
                            border_radius: BorderRadius::all(px(3.0)),
                            ..default()
                        },
                        BorderColor::all(ui::theme::INK),
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
        let relations = [
            GroundMarkerRelation::Local,
            GroundMarkerRelation::Ally,
            GroundMarkerRelation::Enemy,
        ];
        for (index, relation) in relations.iter().enumerate() {
            for other in &relations[index + 1..] {
                assert_ne!(
                    overhead_health_color(*relation),
                    overhead_health_color(*other)
                );
            }
        }
    }

    #[test]
    fn ammunition_segments_distinguish_available_shots() {
        assert_ne!(ammo_segment_color(true), ammo_segment_color(false));
    }

    #[test]
    fn ammunition_progress_uses_the_replicated_interval_and_clamps() {
        let state = brawler::combat::WeaponState {
            ammo: 4,
            phase: brawler::combat::WeaponPhase::Ready,
            ammo_recovery: Some(brawler::combat::AmmoRecovery {
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
    fn bold_overhead_reserves_ammunition_height_only_for_the_local_player() {
        // Twice the pre-BRL-0149 11 px bar, with the number inside it.
        const {
            assert!(HEALTH_BAR_HEIGHT >= 22.0);
            assert!(HEALTH_AMOUNT_FONT_SIZE + HEALTH_BAR_OUTLINE * 2.0 <= HEALTH_BAR_HEIGHT);
            assert!(HEALTH_BAR_WIDTH <= OVERHEAD_WIDTH);
        }
        assert!((overhead_height(false) - OVERHEAD_HEALTH_HEIGHT).abs() < f32::EPSILON);
        assert!(overhead_height(false) < overhead_height(true));
        assert!((FIGHTER_BODY_WORLD_HEIGHT - FIGHTER_VISUAL_HEIGHT_WORLD).abs() < f32::EPSILON);
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
    fn nearby_overheads_stack_without_overlap_and_keep_the_local_anchor() {
        let mut world = World::new();
        let [local, enemy, ally] = [(); 3].map(|()| world.spawn_empty().id());
        // Fighters touching at minimum separation project to nearly the same plate anchor.
        let anchor = Vec2::new(300.0, 200.0);
        let mut placements = vec![
            OverheadPlacement {
                root: enemy,
                top_left: anchor + Vec2::new(18.0, 4.0),
                height: OVERHEAD_HEALTH_HEIGHT,
                is_local: false,
            },
            OverheadPlacement {
                root: local,
                top_left: anchor,
                height: OVERHEAD_AMMO_HEIGHT,
                is_local: true,
            },
            OverheadPlacement {
                root: ally,
                top_left: anchor - Vec2::new(22.0, 0.0),
                height: OVERHEAD_HEALTH_HEIGHT,
                is_local: false,
            },
        ];
        resolve_overhead_overlaps(&mut placements);

        let local_plate = placements.iter().find(|plate| plate.root == local).unwrap();
        assert_eq!(local_plate.top_left, anchor);
        for (index, plate) in placements.iter().enumerate() {
            for other in &placements[index + 1..] {
                assert!(
                    !overhead_plates_overlap(plate, other),
                    "{plate:?} overlaps {other:?}"
                );
            }
        }
        // Distant plates keep their projected positions.
        let mut apart = [
            OverheadPlacement {
                root: local,
                top_left: anchor,
                height: OVERHEAD_HEALTH_HEIGHT,
                is_local: true,
            },
            OverheadPlacement {
                root: enemy,
                top_left: anchor + Vec2::new(OVERHEAD_WIDTH, 0.0),
                height: OVERHEAD_HEALTH_HEIGHT,
                is_local: false,
            },
        ];
        let before = apart;
        resolve_overhead_overlaps(&mut apart);
        assert_eq!(apart, before);
    }

    #[test]
    fn plates_exist_only_while_the_fighter_is_replicated() {
        let mut app = App::new();
        app.add_systems(Update, reconcile_fighter_overheads);
        let fighter = app.world_mut().spawn(Fighter).id();
        app.world_mut().spawn((
            V3FighterVisual {
                last_position: Vec2::ZERO,
                moving: false,
                shoot_seconds: 0.0,
                recoil_seconds: 0.0,
            },
            CombatVisualOwner(fighter),
        ));
        app.update();
        let plates = |app: &mut App| {
            app.world_mut()
                .query::<&FighterOverheadUi>()
                .iter(app.world())
                .count()
        };
        assert_eq!(plates(&mut app), 1);
        // Concealment withholds an enemy by removing its replicated entity; the plate goes with it.
        app.world_mut().despawn(fighter);
        app.update();
        assert_eq!(plates(&mut app), 0);
    }

    /// BRL-dwqx: the BOT chip shows only on a non-local fighter whose seat the server publishes
    /// as held, the name then fits the narrower budget beside it, and a reclaimed seat clears it.
    #[test]
    fn bot_chip_marks_only_a_held_non_local_fighter() {
        use brawler::matchplay::{
            MatchId, MatchParticipant, ParticipantControl, PublicParticipantState,
            PublicParticipantStatus,
        };
        let mut app = App::new();
        app.insert_resource(ColdPieAssets { frames: Vec::new() })
            .add_systems(
                Update,
                (reconcile_fighter_overheads, update_fighter_overhead_state).chain(),
            );
        let loadout = crate::tests::resolved_reveal_scan_loadout();
        let held = ParticipantControl::HeldByBot {
            expires_at_tick: 3_600,
        };
        let fighter = |app: &mut App, network: u64, controlled: bool| {
            let mut entity = app.world_mut().spawn((
                Fighter,
                brawler::combat::CurrentHealth(100),
                brawler::combat::TeamId(u8::from(!controlled)),
                loadout.clone(),
                NetworkEntityId(network),
            ));
            if controlled {
                entity.insert(Controlled);
            }
            let fighter = entity.id();
            app.world_mut().spawn((
                V3FighterVisual {
                    last_position: Vec2::ZERO,
                    moving: false,
                    shoot_seconds: 0.0,
                    recoil_seconds: 0.0,
                },
                CombatVisualOwner(fighter),
            ));
            app.world_mut()
                .spawn(PublicParticipantState {
                    player_id: PlayerId(network),
                    fighter_network_id: NetworkEntityId(network),
                    team_id: brawler::combat::TeamId(u8::from(!controlled)),
                    display_name: format!("P{network}"),
                    participant: MatchParticipant {
                        match_id: MatchId(1),
                        ready: true,
                        restart_ready: false,
                    },
                    selected: true,
                    weapon_preset_id: None,
                    status: PublicParticipantStatus::Alive,
                    // Even a (never published) held local seat must not show the chip.
                    control: held,
                })
                .id()
        };
        let local_projection = fighter(&mut app, 1, true);
        let remote_projection = fighter(&mut app, 2, false);
        // Spawn plates, then reconcile their state with the plate entities in place.
        app.update();
        app.update();
        let chips = |app: &mut App| {
            let world = app.world_mut();
            let mut overheads = world.query::<(&CombatVisualOwner, &FighterOverheadUi)>();
            let mut chip_display = Vec::new();
            for (owner, overhead) in overheads.iter(world) {
                let network = world.get::<NetworkEntityId>(owner.0).unwrap().0;
                let display = world.get::<Node>(overhead.bot_chip).unwrap().display;
                let label = world
                    .get::<FighterOverheadNameUi>(overhead.player_name)
                    .unwrap();
                chip_display.push((network, display == Display::Flex, label.held));
            }
            chip_display.sort_unstable();
            chip_display
        };
        assert_eq!(chips(&mut app), [(1, false, false), (2, true, true)]);

        for projection in [local_projection, remote_projection] {
            app.world_mut()
                .get_mut::<PublicParticipantState>(projection)
                .unwrap()
                .control = ParticipantControl::Player;
        }
        app.update();
        assert_eq!(chips(&mut app), [(1, false, false), (2, false, false)]);
    }

    /// Longest names the lobby accepts: 24 graphemes, or 21 three-byte CJK characters under the
    /// 64-byte cap, plus a mixed-script name that needs several fallback fonts.
    const MAX_LATIN_NAME: &str = "WWWWWWWWWWWWWWWWWWWWWWWW";
    // 李小雷さくら김민준李小雷さくら김민준李小雷
    const MAX_CJK_NAME: &str = "\u{674E}\u{5C0F}\u{96F7}\u{3055}\u{304F}\u{3089}\u{AE40}\u{BBFC}\u{C900}\u{674E}\u{5C0F}\u{96F7}\u{3055}\u{304F}\u{3089}\u{AE40}\u{BBFC}\u{C900}\u{674E}\u{5C0F}\u{96F7}";
    // Łukasz Жора Αλέξης 李
    const MIXED_SCRIPT_NAME: &str = "\u{141}ukasz \u{416}\u{43E}\u{440}\u{430} \u{391}\u{3BB}\u{3AD}\u{3BE}\u{3B7}\u{3C2} \u{674E}";

    /// Stand-in measure: wide Latin, wider CJK, everything else narrower, all per grapheme.
    fn fake_width(text: &str) -> f32 {
        text.chars()
            .map(|character| match character {
                'W' => 13.0,
                '\u{2E80}'.. => 15.0,
                _ => 8.0,
            })
            .sum()
    }

    #[test]
    fn maximum_names_are_valid_lobby_names() {
        for name in [MAX_LATIN_NAME, MAX_CJK_NAME, MIXED_SCRIPT_NAME] {
            assert!(
                brawler::lobby::normalize_proposed_display_name(name).is_ok(),
                "{name} is a valid display name"
            );
        }
        assert!(
            brawler::lobby::normalize_proposed_display_name(&format!("{MAX_CJK_NAME}\u{674E}"))
                .is_err(),
            "a 22nd CJK character exceeds the byte cap"
        );
    }

    #[test]
    fn display_font_draws_the_truncation_ellipsis() {
        assert!(
            PLAYER_NAME_ELLIPSIS
                .chars()
                .all(ui::theme::display_font_has_glyph)
        );
    }

    #[test]
    fn short_names_are_shown_unchanged() {
        for name in ["Ace", "Bot Kestrel", "\u{674E}\u{5C0F}\u{96F7}"] {
            assert_eq!(
                fit_overhead_name(name, PLAYER_NAME_MAX_WIDTH, fake_width),
                name
            );
        }
    }

    #[test]
    fn wide_names_truncate_by_grapheme_to_the_longest_fitting_prefix() {
        for name in [MAX_LATIN_NAME, MAX_CJK_NAME, MIXED_SCRIPT_NAME] {
            assert!(
                fake_width(name) > PLAYER_NAME_MAX_WIDTH,
                "{name} needs fitting"
            );
            let shown = fit_overhead_name(name, PLAYER_NAME_MAX_WIDTH, fake_width);
            assert!(fake_width(&shown) <= PLAYER_NAME_MAX_WIDTH, "{shown} fits");
            let prefix = shown
                .strip_suffix(PLAYER_NAME_ELLIPSIS)
                .expect("a truncated name ends with the ellipsis");
            assert!(name.starts_with(prefix) && !prefix.ends_with(' '));
            // One more grapheme would not fit.
            let next = name[prefix.len()..]
                .trim_start()
                .chars()
                .next()
                .expect("truncation dropped graphemes");
            assert!(
                fake_width(&format!("{prefix}{next}{PLAYER_NAME_ELLIPSIS}"))
                    > PLAYER_NAME_MAX_WIDTH
            );
        }
    }

    #[test]
    fn truncation_never_splits_a_grapheme_cluster() {
        use unicode_segmentation::UnicodeSegmentation as _;
        // One grapheme of seven code points (man, woman, girl, boy joined by ZWJ).
        const FAMILY: &str = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";
        // One grapheme of two regional indicators.
        const FLAG: &str = "\u{1F1EF}\u{1F1F5}";
        let name = format!("Pew{FAMILY}{FLAG}{FAMILY}{FLAG}{FAMILY}");
        let per_grapheme = |text: &str| text.graphemes(true).map(|_| 14.0).sum::<f32>();
        assert_eq!(fit_overhead_name(&name, 60.0, per_grapheme), "Pew\u{2026}");
        assert_eq!(
            fit_overhead_name(&name, 74.0, per_grapheme),
            format!("Pew{FAMILY}{PLAYER_NAME_ELLIPSIS}")
        );
    }

    /// Fits names through the real display font and system fallbacks, then checks that two
    /// plates exactly one plate width apart keep their names inside their own plate.
    #[test]
    fn adjacent_plates_with_maximum_names_do_not_overlap() {
        let mut world = World::new();
        let mut fonts = Assets::<Font>::default();
        let font = fonts.add(Font::from_bytes(
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/brawler/ui/fonts/lilita-one-regular.ttf"
            ))
            .to_vec(),
        ));
        world.insert_resource(fonts);
        world.init_resource::<bevy::text::FontCx>();
        world.init_resource::<bevy::text::LayoutCx>();
        world
            .run_system_cached(bevy::text::load_font_assets_into_font_collection)
            .unwrap();
        let label = |world: &mut World, name: &str| {
            world
                .spawn((
                    FighterOverheadNameUi {
                        full: name.to_owned(),
                        held: false,
                        fitted: false,
                    },
                    Text::new(""),
                    ui::theme::hud_text(&font, PLAYER_NAME_FONT_SIZE),
                ))
                .id()
        };
        let pairs = [
            (MAX_LATIN_NAME, MAX_CJK_NAME),
            (MAX_CJK_NAME, MAX_CJK_NAME),
            (MAX_LATIN_NAME, MIXED_SCRIPT_NAME),
        ];
        for (left_name, right_name) in pairs {
            let left = label(&mut world, left_name);
            let right = label(&mut world, right_name);
            world.run_system_cached(fit_overhead_player_names).unwrap();
            let width = |world: &mut World, entity: Entity, full: &str| {
                let shown = world.get::<Text>(entity).unwrap().0.clone();
                let family = world.resource_scope(|world, mut font_cx: Mut<bevy::text::FontCx>| {
                    registered_font_family(
                        world.resource::<Assets<Font>>(),
                        &mut font_cx,
                        world.get::<TextFont>(entity).unwrap(),
                    )
                    .expect("display font is registered")
                });
                world.resource_scope(|world, mut font_cx: Mut<bevy::text::FontCx>| {
                    let mut layout_cx = world.resource_mut::<bevy::text::LayoutCx>();
                    let full_width = measure_text_width(
                        &mut font_cx,
                        &mut layout_cx,
                        &family,
                        PLAYER_NAME_FONT_SIZE,
                        full,
                    );
                    assert!(full_width > OVERHEAD_WIDTH, "{full} is wider than a plate");
                    assert!(
                        shown.ends_with(PLAYER_NAME_ELLIPSIS),
                        "{shown} is truncated"
                    );
                    measure_text_width(
                        &mut font_cx,
                        &mut layout_cx,
                        &family,
                        PLAYER_NAME_FONT_SIZE,
                        &shown,
                    )
                })
            };
            let left_width = width(&mut world, left, left_name);
            let right_width = width(&mut world, right, right_name);
            assert!(left_width <= PLAYER_NAME_MAX_WIDTH && right_width <= PLAYER_NAME_MAX_WIDTH);
            // Centered names on plates whose centers are exactly OVERHEAD_WIDTH apart.
            let left_plate = OverheadPlacement {
                root: left,
                top_left: Vec2::ZERO,
                height: OVERHEAD_HEALTH_HEIGHT,
                is_local: false,
            };
            let right_plate = OverheadPlacement {
                root: right,
                top_left: Vec2::new(OVERHEAD_WIDTH, 0.0),
                height: OVERHEAD_HEALTH_HEIGHT,
                is_local: false,
            };
            assert!(!overhead_plates_overlap(&left_plate, &right_plate));
            let left_name_right_edge = OVERHEAD_WIDTH * 0.5 + left_width * 0.5;
            let right_name_left_edge = OVERHEAD_WIDTH * 1.5 - right_width * 0.5;
            assert!(left_name_right_edge <= right_name_left_edge);
        }

        // A fitted label is not re-measured until its name changes.
        let entity = label(&mut world, MAX_LATIN_NAME);
        world.run_system_cached(fit_overhead_player_names).unwrap();
        world.get_mut::<Text>(entity).unwrap().0 = "sentinel".to_owned();
        world.run_system_cached(fit_overhead_player_names).unwrap();
        assert_eq!(world.get::<Text>(entity).unwrap().0, "sentinel");
        let mut name = world.get_mut::<FighterOverheadNameUi>(entity).unwrap();
        name.full = "Ace".to_owned();
        name.fitted = false;
        world.run_system_cached(fit_overhead_player_names).unwrap();
        assert_eq!(world.get::<Text>(entity).unwrap().0, "Ace");
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
            Some(Vec2::new(250.0, 109.0))
        );
    }
}
