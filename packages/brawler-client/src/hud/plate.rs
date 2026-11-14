//! Top-center score and timer plate, plus the Zombie teammate strip under it.
//!
//! `hud.rs` derives [`PlateState`] from replicated match facts; this module turns a mode score view
//! into observer-relative chip content (own team left in ally blue, opponent right in enemy red)
//! and writes the retained nodes only when that state changes.

use super::{HudControlLayout, ModeScoreView, palette};
use crate::{touch::TouchControls, ui};
use bevy::prelude::*;
use bevy::ui::UiScale;
use bevy::window::PrimaryWindow;
use brawler::combat::TeamId;

/// Most teammates a Zombie roster can hold (three-fighter co-op).
pub(super) const TEAM_STRIP_ROWS: usize = 3;
const PLATE_TOP: f32 = 10.0;
const PLATE_PADDING: f32 = 6.0;
const PLATE_BORDER: f32 = 3.0;
const CHIP_WIDTH: f32 = 84.0;
const CHIP_HEIGHT: f32 = 54.0;
/// Bottom edge of the score plate in UI units (multiply by `UiScale` for logical pixels). The
/// plate's fixed-height chips set its height; the gameplay camera keeps the local fighter's
/// overhead below this band (BRL-fwtg).
pub(crate) const PLATE_BOTTOM: f32 = PLATE_TOP + 2.0 * (PLATE_BORDER + PLATE_PADDING) + CHIP_HEIGHT;
const STRIP_BAR_WIDTH: f32 = 96.0;
/// Team strip inset from the left edge, or from the touch menu button while it is shown.
const STRIP_LEFT: f32 = 16.0;
const STRIP_MENU_GAP: f32 = 10.0;

/// The color role of a plate element.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PlateTone {
    #[default]
    Neutral,
    Ally,
    Enemy,
    Warning,
}

impl PlateTone {
    fn color(self) -> Color {
        match self {
            Self::Neutral => ui::theme::PANEL_RAISED,
            Self::Ally => palette::ALLY,
            Self::Enemy => palette::ENEMY,
            Self::Warning => ui::theme::PLAY,
        }
    }
}

/// One team score chip: a large value and an optional caption.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PlateChip {
    pub(crate) value: String,
    pub(crate) caption: String,
    pub(crate) tone: PlateTone,
}

/// Observer-relative plate content for one mode score view.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PlateContent {
    pub(crate) left: PlateChip,
    pub(crate) right: PlateChip,
    pub(crate) subline: String,
    pub(crate) subline_tone: PlateTone,
}

/// One Zombie teammate: name, health share, and a short status.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TeamStripRow {
    pub(crate) name: String,
    pub(crate) health_fraction: Option<f32>,
    pub(crate) status: String,
    pub(crate) local: bool,
}

/// Everything the plate presents this frame; written with `set_if_neq` by the HUD fact system.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub(crate) struct PlateState {
    pub(crate) shown: bool,
    pub(crate) timer: String,
    pub(crate) content: PlateContent,
    pub(crate) team_strip: Vec<TeamStripRow>,
}

/// Maps a team-indexed score view onto the observer: own team left, opponent right.
pub(crate) fn plate_content(
    view: ModeScoreView,
    local_team: Option<TeamId>,
    zombie_overtime: bool,
) -> PlateContent {
    let ally = usize::from(local_team.map_or(0, |team| team.0.min(1)));
    let enemy = 1 - ally;
    let chip = |value: String, caption: &str, tone| PlateChip {
        value,
        caption: caption.to_string(),
        tone,
    };
    let sides = |values: [String; 2], captions: [&str; 2]| {
        let [ally_value, enemy_value] = [values[ally].clone(), values[enemy].clone()];
        (
            chip(ally_value, captions[0], PlateTone::Ally),
            chip(enemy_value, captions[1], PlateTone::Enemy),
        )
    };
    match view {
        ModeScoreView::Wipeout { scores, target } => {
            let (left, right) = sides(scores.map(|score| score.to_string()), ["", ""]);
            PlateContent {
                left,
                right,
                subline: format!("FIRST TO {target}"),
                subline_tone: PlateTone::Neutral,
            }
        }
        ModeScoreView::HotZone {
            progress_percent,
            status,
        } => {
            let (left, right) = sides(progress_percent.map(|value| format!("{value}%")), ["", ""]);
            let (subline, subline_tone) = match status {
                brawler::matchplay::HotZoneStatus::Empty => ("ZONE EMPTY", PlateTone::Neutral),
                brawler::matchplay::HotZoneStatus::Contested => ("CONTESTED", PlateTone::Warning),
                brawler::matchplay::HotZoneStatus::Controlled { team }
                    if usize::from(team.0) == ally =>
                {
                    ("ALLY CONTROL", PlateTone::Ally)
                }
                brawler::matchplay::HotZoneStatus::Controlled { .. } => {
                    ("ENEMY CONTROL", PlateTone::Enemy)
                }
            };
            PlateContent {
                left,
                right,
                subline: subline.to_string(),
                subline_tone,
            }
        }
        ModeScoreView::Heist {
            health, maximum, ..
        } => {
            let percent = |team: usize| {
                let value = u32::from(health[team]) * 100 / u32::from(maximum[team].max(1));
                format!("{value}%")
            };
            let (left, right) = sides([percent(0), percent(1)], ["DEFEND", "ATTACK"]);
            PlateContent {
                left,
                right,
                subline: "BREAK THEIR IDOL".to_string(),
                subline_tone: PlateTone::Neutral,
            }
        }
        ModeScoreView::Zombie {
            live_zombies,
            last_brawler_standing,
            local_boost_stacks,
        } => {
            let (subline, subline_tone) = if last_brawler_standing {
                ("LAST BRAWLER STANDING".to_string(), PlateTone::Warning)
            } else if zombie_overtime {
                ("CLEAR THE HORDE".to_string(), PlateTone::Warning)
            } else {
                (String::new(), PlateTone::Neutral)
            };
            PlateContent {
                left: chip(
                    local_boost_stacks.to_string(),
                    &format!("BOOST +{}%", u16::from(local_boost_stacks) * 5),
                    PlateTone::Ally,
                ),
                right: chip(live_zombies.to_string(), "ZOMBIES", PlateTone::Enemy),
                subline,
                subline_tone,
            }
        }
        ModeScoreView::Syncing => PlateContent {
            left: chip("-".to_string(), "", PlateTone::Ally),
            right: chip("-".to_string(), "", PlateTone::Enemy),
            subline: "SYNCING".to_string(),
            subline_tone: PlateTone::Neutral,
        },
    }
}

/// Layout-tested plate root (top center).
#[derive(Component)]
pub(super) struct PlateRoot;

/// Layout-tested Zombie teammate strip root (top left).
#[derive(Component)]
pub(super) struct TeamStripRoot;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) enum PlatePart {
    Chip(Side),
    Value(Side),
    Caption(Side),
    Timer,
    Subline,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Side {
    Left,
    Right,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) enum StripPart {
    Row(usize),
    Name(usize),
    Fill(usize),
    Status(usize),
}

fn text_bundle(font: &Handle<Font>, size: f32) -> impl Bundle {
    (
        Text::new(""),
        ui::theme::hud_text(font, size),
        TextColor(ui::theme::TEXT),
        TextLayout::new(Justify::Center, LineBreak::NoWrap),
    )
}

pub(super) fn spawn_plate(commands: &mut Commands, font: &Handle<Font>, parent: Entity) {
    let chip = |side: Side| {
        (
            PlatePart::Chip(side),
            Node {
                min_width: px(CHIP_WIDTH),
                padding: UiRect::horizontal(px(8.0)),
                height: px(CHIP_HEIGHT),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(3.0)),
                border_radius: BorderRadius::all(px(10.0)),
                ..default()
            },
            BackgroundColor(PlateTone::Neutral.color()),
            BorderColor::all(ui::theme::INK),
        )
    };
    let plate = commands
        .spawn((
            PlateRoot,
            Name::new("HUD score plate"),
            Node {
                position_type: PositionType::Absolute,
                top: px(PLATE_TOP),
                left: percent(50.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(10.0),
                padding: UiRect::all(px(PLATE_PADDING)),
                border: UiRect::all(px(PLATE_BORDER)),
                border_radius: BorderRadius::all(px(14.0)),
                ..default()
            },
            // Centered on its own width, independent of the content width.
            UiTransform::from_translation(Val2::percent(-50.0, 0.0)),
            BackgroundColor(palette::PLATE),
            BorderColor::all(ui::theme::INK),
            Visibility::Hidden,
        ))
        .id();
    commands.entity(parent).add_child(plate);
    for side in [Side::Left, Side::Right] {
        let chip = commands
            .spawn(chip(side))
            .with_children(|chip| {
                chip.spawn((PlatePart::Value(side), text_bundle(font, 30.0)));
                chip.spawn((PlatePart::Caption(side), text_bundle(font, 12.0)));
            })
            .id();
        let center = (side == Side::Left).then(|| {
            commands
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    min_width: px(120.0),
                    ..default()
                })
                .with_children(|center| {
                    center.spawn((PlatePart::Timer, text_bundle(font, 30.0)));
                    center.spawn((PlatePart::Subline, text_bundle(font, 14.0)));
                })
                .id()
        });
        commands.entity(plate).add_child(chip);
        if let Some(center) = center {
            commands.entity(plate).add_child(center);
        }
    }
    spawn_team_strip(commands, font, parent);
}

fn spawn_team_strip(commands: &mut Commands, font: &Handle<Font>, parent: Entity) {
    let strip = commands
        .spawn((
            TeamStripRoot,
            Name::new("HUD team strip"),
            Node {
                position_type: PositionType::Absolute,
                top: px(PLATE_TOP),
                left: px(STRIP_LEFT),
                flex_direction: FlexDirection::Column,
                row_gap: px(4.0),
                ..default()
            },
            Visibility::Hidden,
        ))
        .id();
    commands.entity(parent).add_child(strip);
    for index in 0..TEAM_STRIP_ROWS {
        let row = commands
            .spawn((
                StripPart::Row(index),
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(8.0),
                    padding: UiRect::axes(px(8.0), px(3.0)),
                    border_radius: BorderRadius::all(px(8.0)),
                    ..default()
                },
                BackgroundColor(palette::PLATE),
                Visibility::Hidden,
            ))
            .with_children(|row| {
                row.spawn((
                    StripPart::Name(index),
                    text_bundle(font, 14.0),
                    Node {
                        width: px(92.0),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                ));
                row.spawn((
                    Node {
                        width: px(STRIP_BAR_WIDTH),
                        height: px(9.0),
                        border_radius: BorderRadius::all(px(4.0)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(ui::theme::INK),
                ))
                .with_child((
                    StripPart::Fill(index),
                    Node {
                        width: percent(0.0),
                        height: percent(100.0),
                        ..default()
                    },
                    BackgroundColor(palette::ALLY),
                ));
                row.spawn((
                    StripPart::Status(index),
                    text_bundle(font, 13.0),
                    Node {
                        min_width: px(40.0),
                        ..default()
                    },
                ));
            })
            .id();
        commands.entity(strip).add_child(row);
    }
}

/// Keeps the teammate strip right of the touch menu button, which shares the top-left corner.
#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn place_team_strip(
    windows: Query<&Window, With<PrimaryWindow>>,
    scale: Option<Res<UiScale>>,
    controls: Res<TouchControls>,
    mut strips: Query<&mut Node, With<TeamStripRoot>>,
) {
    let Some(viewport) = windows
        .iter()
        .next()
        .map(|window| Vec2::new(window.width(), window.height()))
    else {
        return;
    };
    let left = if super::controls::touch_mode(&controls) {
        let scale = scale.map_or(1.0, |scale| scale.0).max(0.01);
        HudControlLayout::new(viewport).menu.max.x / scale + STRIP_MENU_GAP
    } else {
        STRIP_LEFT
    };
    for mut node in &mut strips {
        if node.left != px(left) {
            node.left = px(left);
        }
    }
}

type PlateRootQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Visibility,
    (With<PlateRoot>, Without<TeamStripRoot>, Without<StripPart>),
>;
type StripRootQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Visibility,
    (With<TeamStripRoot>, Without<PlateRoot>, Without<StripPart>),
>;

/// Writes plate nodes when [`PlateState`] changed.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn present_plate(
    state: Res<PlateState>,
    mut roots: PlateRootQuery,
    mut strips: StripRootQuery,
    mut parts: Query<
        (
            &PlatePart,
            Option<&mut Text>,
            Option<&mut TextColor>,
            Option<&mut BackgroundColor>,
        ),
        Without<StripPart>,
    >,
    mut strip_parts: Query<
        (
            &StripPart,
            Option<&mut Text>,
            Option<&mut TextColor>,
            Option<&mut Node>,
            Option<&mut Visibility>,
        ),
        (
            Without<TeamStripRoot>,
            Without<PlateRoot>,
            Without<PlatePart>,
        ),
    >,
) {
    if !state.is_changed() {
        return;
    }
    for mut visibility in &mut roots {
        visibility.set_if_neq(if state.shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    let content = &state.content;
    let chip = |side: Side| match side {
        Side::Left => &content.left,
        Side::Right => &content.right,
    };
    for (part, text, color, background) in &mut parts {
        let (label, tone) = match *part {
            PlatePart::Chip(side) => {
                if let Some(mut background) = background {
                    background.set_if_neq(BackgroundColor(chip(side).tone.color()));
                }
                continue;
            }
            PlatePart::Value(side) => (chip(side).value.as_str(), None),
            PlatePart::Caption(side) => (chip(side).caption.as_str(), None),
            PlatePart::Timer => (state.timer.as_str(), None),
            PlatePart::Subline => (content.subline.as_str(), Some(content.subline_tone)),
        };
        if let Some(mut text) = text
            && text.0 != label
        {
            label.clone_into(&mut text.0);
        }
        if let Some(mut color) = color {
            let next = match tone {
                Some(PlateTone::Neutral) | None => ui::theme::TEXT,
                Some(tone) => tone.color(),
            };
            color.set_if_neq(TextColor(next));
        }
    }
    for mut visibility in &mut strips {
        visibility.set_if_neq(if state.shown && !state.team_strip.is_empty() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    for (part, text, color, node, visibility) in &mut strip_parts {
        let (StripPart::Row(index)
        | StripPart::Name(index)
        | StripPart::Fill(index)
        | StripPart::Status(index)) = *part;
        let row = state.team_strip.get(index);
        match (*part, row) {
            (StripPart::Row(_), _) => {
                if let Some(mut visibility) = visibility {
                    visibility.set_if_neq(if row.is_some() {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    });
                }
            }
            (StripPart::Name(_), Some(row)) => {
                if let Some(mut text) = text {
                    text.set_if_neq(Text(row.name.clone()));
                }
                if let Some(mut color) = color {
                    color.set_if_neq(TextColor(if row.local {
                        palette::LOCAL
                    } else {
                        ui::theme::TEXT
                    }));
                }
            }
            (StripPart::Fill(_), Some(row)) => {
                if let Some(mut node) = node {
                    let width = percent(row.health_fraction.unwrap_or(0.0).clamp(0.0, 1.0) * 100.0);
                    if node.width != width {
                        node.width = width;
                    }
                }
            }
            (StripPart::Status(_), Some(row)) => {
                if let Some(mut text) = text {
                    text.set_if_neq(Text(row.status.clone()));
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brawler::matchplay::HotZoneStatus;

    #[test]
    fn own_team_is_always_on_the_left_in_ally_colors() {
        let view = ModeScoreView::Wipeout {
            scores: [3, 7],
            target: 10,
        };
        let team_one = plate_content(view, Some(TeamId(0)), false);
        assert_eq!(
            (team_one.left.value.as_str(), team_one.right.value.as_str()),
            ("3", "7")
        );
        assert_eq!(team_one.left.tone, PlateTone::Ally);
        assert_eq!(team_one.subline, "FIRST TO 10");
        let team_two = plate_content(view, Some(TeamId(1)), false);
        assert_eq!(
            (team_two.left.value.as_str(), team_two.right.value.as_str()),
            ("7", "3")
        );
        assert_eq!(team_two.right.tone, PlateTone::Enemy);
    }

    #[test]
    fn hot_zone_control_reads_relative_to_the_observer() {
        let view = |status| ModeScoreView::HotZone {
            progress_percent: [40, 15],
            status,
        };
        let controlled = view(HotZoneStatus::Controlled { team: TeamId(1) });
        let mine = plate_content(controlled, Some(TeamId(1)), false);
        assert_eq!(
            (mine.left.value.as_str(), mine.subline.as_str()),
            ("15%", "ALLY CONTROL")
        );
        assert_eq!(mine.subline_tone, PlateTone::Ally);
        let theirs = plate_content(controlled, Some(TeamId(0)), false);
        assert_eq!(
            (theirs.subline.as_str(), theirs.subline_tone),
            ("ENEMY CONTROL", PlateTone::Enemy)
        );
        let contested = plate_content(view(HotZoneStatus::Contested), None, false);
        assert_eq!(contested.subline_tone, PlateTone::Warning);
    }

    #[test]
    fn heist_and_zombie_show_their_objective_state() {
        let heist = plate_content(
            ModeScoreView::Heist {
                health: [500, 250],
                maximum: [1000, 1000],
                local_team: Some(TeamId(1)),
            },
            Some(TeamId(1)),
            false,
        );
        assert_eq!(
            (heist.left.value.as_str(), heist.left.caption.as_str()),
            ("25%", "DEFEND")
        );
        assert_eq!(
            (heist.right.value.as_str(), heist.right.caption.as_str()),
            ("50%", "ATTACK")
        );
        let zombie = |last, overtime| {
            plate_content(
                ModeScoreView::Zombie {
                    live_zombies: 12,
                    last_brawler_standing: last,
                    local_boost_stacks: 3,
                },
                Some(TeamId(0)),
                overtime,
            )
        };
        let calm = zombie(false, false);
        assert_eq!(
            (calm.left.value.as_str(), calm.left.caption.as_str()),
            ("3", "BOOST +15%")
        );
        assert_eq!(
            (calm.right.value.as_str(), calm.right.caption.as_str()),
            ("12", "ZOMBIES")
        );
        assert!(calm.subline.is_empty());
        assert_eq!(zombie(false, true).subline, "CLEAR THE HORDE");
        assert_eq!(zombie(true, true).subline, "LAST BRAWLER STANDING");
        assert_eq!(
            plate_content(ModeScoreView::Syncing, None, false).subline,
            "SYNCING"
        );
    }
}
