//! The in-match scoreboard panel, held (Tab/Select) or latched from the match menu.
//!
//! `hud.rs` derives [`ScoreboardState`] from the public participant roster, so the panel shows
//! only observer-safe facts. This module owns the retained nodes: up to two observer-relative team
//! sections whose player rows grow to the largest roster seen and are rewritten in place. Panel
//! visibility, the latch, and the close button's action belong to `presentation`.

use super::{CachedRosterEntry, CachedRosterStatus, MatchRosterPresentation, palette};
use crate::ui::{
    self,
    surface::{Surface, SurfaceState},
};
use bevy::{ecs::relationship::RelationshipTarget, prelude::*};
use brawler::combat::TeamId;

/// Two-team modes show both sides; co-op modes fill only the first section.
const SECTIONS: usize = 2;
const PANEL_MAX_WIDTH: f32 = 1180.0;
const SECTION_MAX_WIDTH: f32 = 560.0;
const HEALTH_BAR_WIDTH: f32 = 150.0;
/// Touch-reachable close control (about 49 logical pixels tall on a 411-pixel-tall phone).
const CLOSE_HEIGHT: f32 = 76.0;
const CLOSE_MIN_WIDTH: f32 = 180.0;

/// The color role of a team section header.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SectionTone {
    /// Spectating observers see neutral team colors.
    #[default]
    Neutral,
    Ally,
    Enemy,
}

impl SectionTone {
    fn color(self) -> Color {
        match self {
            Self::Neutral => ui::theme::PANEL_RAISED,
            Self::Ally => palette::ALLY,
            Self::Enemy => palette::ENEMY,
        }
    }
}

/// The color role of a row's status label.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum StatusTone {
    #[default]
    Normal,
    Protected,
    Down,
    Offline,
}

impl StatusTone {
    fn color(self) -> Color {
        match self {
            Self::Normal => ui::theme::TEXT,
            Self::Protected => ui::theme::ACCENT,
            Self::Down => ui::theme::PLAY,
            Self::Offline => Color::srgb(0.62, 0.66, 0.78),
        }
    }
}

/// One player: public name, local marker, health share and value, and a readable status.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ScoreboardRow {
    pub(crate) name: String,
    pub(crate) local: bool,
    pub(crate) health_fraction: Option<f32>,
    pub(crate) health: String,
    pub(crate) status: String,
    pub(crate) status_tone: StatusTone,
}

/// One team: title, color, and its rows (local player first).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ScoreboardSection {
    pub(crate) title: String,
    pub(crate) tone: SectionTone,
    pub(crate) rows: Vec<ScoreboardRow>,
}

/// Everything the scoreboard presents; written with `set_if_neq` by the HUD fact system.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub(crate) struct ScoreboardState {
    pub(crate) sections: [ScoreboardSection; SECTIONS],
}

/// Builds observer-relative sections: the local team first as YOUR TEAM, then OPPONENTS; a
/// teamless observer sees TEAM 1 and TEAM 2 in neutral colors. `in_play` (countdown or active)
/// reads the lobby ready flags, which the public status keeps through the match, as ALIVE.
pub(super) fn scoreboard_state(
    roster: &MatchRosterPresentation,
    local_player: Option<u64>,
    local_team: Option<TeamId>,
    now: Option<u64>,
    in_play: bool,
) -> ScoreboardState {
    let mut teams: Vec<TeamId> = roster.entries.values().map(|entry| entry.team).collect();
    teams.sort_by_key(|team| (Some(*team) != local_team, team.0));
    teams.dedup();
    let mut state = ScoreboardState::default();
    for (section, team) in state.sections.iter_mut().zip(teams) {
        let (title, tone) = match local_team {
            Some(local) if local == team => ("YOUR TEAM".to_string(), SectionTone::Ally),
            Some(_) => ("OPPONENTS".to_string(), SectionTone::Enemy),
            None => (
                format!("TEAM {}", u16::from(team.0) + 1),
                SectionTone::Neutral,
            ),
        };
        let mut players: Vec<_> = roster
            .entries
            .iter()
            .filter(|(_, entry)| entry.team == team)
            .collect();
        players.sort_by_key(|(player, _)| (local_player != Some(**player), **player));
        *section = ScoreboardSection {
            title,
            tone,
            rows: players
                .into_iter()
                .map(|(player, entry)| {
                    scoreboard_row(entry, local_player == Some(*player), now, in_play)
                })
                .collect(),
        };
    }
    state
}

fn scoreboard_row(
    entry: &CachedRosterEntry,
    local: bool,
    now: Option<u64>,
    in_play: bool,
) -> ScoreboardRow {
    let seconds = |tick: u64| {
        tick.saturating_sub(now.unwrap_or(tick))
            .div_ceil(brawler::timing::SIMULATION_TICK_HZ)
    };
    let (status, status_tone) = if let (true, Some(until)) = (entry.connected, entry.held_until) {
        // A bot drives the disconnected player's held seat until it is reclaimed or expires.
        (format!("BOT {}s", seconds(until)), StatusTone::Offline)
    } else if entry.connected {
        match entry.status {
            CachedRosterStatus::Alive => ("ALIVE".to_string(), StatusTone::Normal),
            CachedRosterStatus::Ready | CachedRosterStatus::RestartReady if in_play => {
                ("ALIVE".to_string(), StatusTone::Normal)
            }
            CachedRosterStatus::Ready => ("READY".to_string(), StatusTone::Normal),
            CachedRosterStatus::RestartReady => ("RESTART READY".to_string(), StatusTone::Normal),
            CachedRosterStatus::Defeated => ("DEFEATED".to_string(), StatusTone::Down),
            CachedRosterStatus::Respawning(at) => {
                (format!("RESPAWN IN {}s", seconds(at)), StatusTone::Down)
            }
            CachedRosterStatus::Protected(until) => (
                format!("PROTECTED {}s", seconds(until)),
                StatusTone::Protected,
            ),
        }
    } else {
        ("DISCONNECTED".to_string(), StatusTone::Offline)
    };
    let status = if entry.boost_stacks > 0 && entry.connected {
        format!("{status}  BOOST +{}", entry.boost_stacks)
    } else {
        status
    };
    let defeated = matches!(entry.status, CachedRosterStatus::Defeated);
    ScoreboardRow {
        name: entry.display_name.clone(),
        local,
        health_fraction: if defeated {
            Some(0.0)
        } else {
            entry
                .health
                .map(|(current, maximum)| f32::from(current) / f32::from(maximum.max(1)))
        },
        health: entry.health.map_or_else(
            || "HP --".to_string(),
            |(current, maximum)| format!("HP {current}/{maximum}"),
        ),
        status,
        status_tone,
    }
}

/// The latched panel's visible close control; `presentation` shows it only while latched and
/// dismisses the scoreboard when it activates.
#[derive(Component)]
#[require(ui::press::ReleaseActivation, ui::touch_target::TouchTarget)]
pub(crate) struct ScoreboardCloseButton;

/// The rows container of one section; rows are spawned here on demand.
#[derive(Component)]
pub(super) struct ScoreboardRowList {
    section: usize,
    font: Handle<Font>,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ScoreboardPart {
    Section(usize),
    SectionHeader(usize),
    SectionTitle(usize),
    Row(usize, usize),
    Name(usize, usize),
    You(usize, usize),
    Fill(usize, usize),
    Health(usize, usize),
    Status(usize, usize),
}

fn text_bundle(font: &Handle<Font>, label: &str, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(label),
        ui::theme::hud_text(font, size),
        TextColor(color),
        TextLayout::new(Justify::Left, LineBreak::NoWrap),
    )
}

/// Spawns the hidden panel root (`ScoreboardOverlay`) with its header and empty team sections.
pub(super) fn spawn_scoreboard(commands: &mut Commands, font: &Handle<Font>) {
    commands
        .spawn((
            crate::ScoreboardOverlay,
            Name::new("Scoreboard"),
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                width: percent(100.0),
                height: percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::all(px(16.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.09, 0.5)),
            // Above the HUD (150) and phase panels (220-225), below the match menu (300).
            GlobalZIndex(250),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                Surface::Panel,
                SurfaceState::default(),
                Node {
                    width: percent(100.0),
                    max_width: px(PANEL_MAX_WIDTH),
                    flex_direction: FlexDirection::Column,
                    // Clear of the shell panel's slanted edges and lip.
                    padding: UiRect {
                        left: px(52.0),
                        right: px(52.0),
                        top: px(24.0),
                        bottom: px(36.0),
                    },
                    row_gap: px(14.0),
                    ..default()
                },
                BackgroundColor(ui::theme::PANEL),
            ))
            .with_children(|panel| {
                panel
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: px(16.0),
                        min_height: px(CLOSE_HEIGHT),
                        ..default()
                    })
                    .with_children(|header| {
                        header.spawn(text_bundle(font, "SCOREBOARD", 40.0, ui::theme::TEXT));
                        header
                            .spawn((
                                ScoreboardCloseButton,
                                Button,
                                Surface::Back,
                                SurfaceState::default(),
                                Node {
                                    min_width: px(CLOSE_MIN_WIDTH),
                                    height: px(CLOSE_HEIGHT),
                                    padding: UiRect::axes(px(24.0), px(8.0)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    ..default()
                                },
                                Visibility::Hidden,
                            ))
                            .with_child(text_bundle(font, "BACK", 22.0, ui::theme::TEXT));
                    });
                panel
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::FlexStart,
                        column_gap: px(24.0),
                        ..default()
                    })
                    .with_children(|sections| {
                        for section in 0..SECTIONS {
                            spawn_section(sections, font, section);
                        }
                    });
            });
        });
}

fn spawn_section(parent: &mut ChildSpawnerCommands, font: &Handle<Font>, section: usize) {
    parent
        .spawn((
            ScoreboardPart::Section(section),
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                flex_basis: px(0.0),
                min_width: px(0.0),
                max_width: px(SECTION_MAX_WIDTH),
                row_gap: px(8.0),
                ..default()
            },
            Visibility::Hidden,
        ))
        .with_children(|column| {
            column
                .spawn((
                    ScoreboardPart::SectionHeader(section),
                    Node {
                        padding: UiRect::axes(px(18.0), px(6.0)),
                        border: UiRect::all(px(3.0)),
                        border_radius: BorderRadius::all(px(10.0)),
                        ..default()
                    },
                    BackgroundColor(SectionTone::Neutral.color()),
                    BorderColor::all(ui::theme::INK),
                ))
                .with_child((
                    ScoreboardPart::SectionTitle(section),
                    text_bundle(font, "", 26.0, ui::theme::TEXT),
                ));
            column.spawn((
                ScoreboardRowList {
                    section,
                    font: font.clone(),
                },
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(8.0),
                    ..default()
                },
            ));
        });
}

/// Spawns one player row already showing `row`, so a newly seen player never shows a blank frame.
fn spawn_row(
    commands: &mut Commands,
    list: Entity,
    font: &Handle<Font>,
    (section, index): (usize, usize),
    row: &ScoreboardRow,
) {
    let fraction = row.health_fraction.unwrap_or(0.0).clamp(0.0, 1.0);
    let entity = commands
        .spawn((
            ScoreboardPart::Row(section, index),
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(px(16.0), px(8.0)),
                row_gap: px(6.0),
                border: UiRect::all(px(3.0)),
                border_radius: BorderRadius::all(px(10.0)),
                ..default()
            },
            BackgroundColor(palette::PLATE),
            BorderColor::all(row_border(row.local)),
        ))
        .with_children(|card| {
            card.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(10.0),
                ..default()
            })
            .with_children(|line| {
                line.spawn((
                    ScoreboardPart::Name(section, index),
                    text_bundle(font, &row.name, 28.0, ui::theme::TEXT),
                    Node {
                        flex_grow: 1.0,
                        flex_shrink: 1.0,
                        min_width: px(0.0),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                ));
                line.spawn((
                    ScoreboardPart::You(section, index),
                    Node {
                        flex_shrink: 0.0,
                        padding: UiRect::axes(px(10.0), px(2.0)),
                        border_radius: BorderRadius::all(px(6.0)),
                        ..default()
                    },
                    BackgroundColor(palette::LOCAL),
                    shown(row.local),
                ))
                .with_child(text_bundle(font, "YOU", 20.0, ui::theme::TEXT));
            });
            card.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(12.0),
                ..default()
            })
            .with_children(|line| {
                line.spawn((
                    Node {
                        // The bar gives way first, so the value and status never overlap.
                        flex_shrink: 1.0,
                        min_width: px(48.0),
                        width: px(HEALTH_BAR_WIDTH),
                        height: px(12.0),
                        border_radius: BorderRadius::all(px(5.0)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(ui::theme::INK),
                ))
                .with_child((
                    ScoreboardPart::Fill(section, index),
                    Node {
                        width: percent(fraction * 100.0),
                        height: percent(100.0),
                        ..default()
                    },
                    BackgroundColor(fill_color(fraction)),
                ));
                line.spawn((
                    ScoreboardPart::Health(section, index),
                    text_bundle(font, &row.health, 20.0, ui::theme::TEXT),
                    Node {
                        flex_shrink: 0.0,
                        ..default()
                    },
                ));
                line.spawn(Node {
                    flex_grow: 1.0,
                    ..default()
                });
                line.spawn((
                    ScoreboardPart::Status(section, index),
                    text_bundle(font, &row.status, 20.0, row.status_tone.color()),
                    Node {
                        flex_shrink: 0.0,
                        ..default()
                    },
                ));
            });
        })
        .id();
    commands.entity(list).add_child(entity);
}

fn shown(visible: bool) -> Visibility {
    if visible {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    }
}

fn row_border(local: bool) -> Color {
    if local {
        palette::LOCAL
    } else {
        ui::theme::INK
    }
}

/// Health fill: green while healthy, warning orange below a third.
fn fill_color(fraction: f32) -> Color {
    if fraction < 1.0 / 3.0 {
        ui::theme::PLAY
    } else {
        palette::LOCAL
    }
}

type PartQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ScoreboardPart,
        Option<&'static mut Text>,
        Option<&'static mut TextColor>,
        Option<&'static mut Node>,
        Option<&'static mut Visibility>,
        Option<&'static mut BackgroundColor>,
        Option<&'static mut BorderColor>,
    ),
>;

/// Grows each section's rows to its roster and rewrites the retained nodes when
/// [`ScoreboardState`] changed. Rows beyond the current roster stay spawned and hidden.
#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn present_scoreboard(
    mut commands: Commands,
    state: Res<ScoreboardState>,
    lists: Query<(Entity, &ScoreboardRowList, Option<&Children>)>,
    mut parts: PartQuery,
) {
    if !state.is_changed() {
        return;
    }
    for (list, rows, children) in &lists {
        let Some(section) = state.sections.get(rows.section) else {
            continue;
        };
        let spawned = children.map_or(0, RelationshipTarget::len);
        for (index, row) in section.rows.iter().enumerate().skip(spawned) {
            spawn_row(&mut commands, list, &rows.font, (rows.section, index), row);
        }
    }
    for (part, text, color, node, visibility, background, border) in &mut parts {
        match *part {
            ScoreboardPart::Section(section) => {
                if let Some(mut visibility) = visibility {
                    visibility.set_if_neq(shown(!state.sections[section].rows.is_empty()));
                }
            }
            ScoreboardPart::SectionHeader(section) => {
                if let Some(mut background) = background {
                    background.set_if_neq(BackgroundColor(state.sections[section].tone.color()));
                }
            }
            ScoreboardPart::SectionTitle(section) => {
                if let Some(mut text) = text {
                    set_text(&mut text, &state.sections[section].title);
                }
            }
            ScoreboardPart::Row(section, index) => {
                let row = state.sections[section].rows.get(index);
                if let Some(mut visibility) = visibility {
                    visibility.set_if_neq(shown(row.is_some()));
                }
                if let (Some(mut border), Some(row)) = (border, row) {
                    border.set_if_neq(BorderColor::all(row_border(row.local)));
                }
            }
            ScoreboardPart::Name(section, index)
            | ScoreboardPart::You(section, index)
            | ScoreboardPart::Fill(section, index)
            | ScoreboardPart::Health(section, index)
            | ScoreboardPart::Status(section, index) => {
                let Some(row) = state.sections[section].rows.get(index) else {
                    continue;
                };
                match *part {
                    ScoreboardPart::Name(..) => {
                        if let Some(mut text) = text {
                            set_text(&mut text, &row.name);
                        }
                    }
                    ScoreboardPart::You(..) => {
                        if let Some(mut visibility) = visibility {
                            visibility.set_if_neq(shown(row.local));
                        }
                    }
                    ScoreboardPart::Fill(..) => {
                        let fraction = row.health_fraction.unwrap_or(0.0).clamp(0.0, 1.0);
                        if let Some(mut node) = node {
                            let width = percent(fraction * 100.0);
                            if node.width != width {
                                node.width = width;
                            }
                        }
                        if let Some(mut background) = background {
                            background.set_if_neq(BackgroundColor(fill_color(fraction)));
                        }
                    }
                    ScoreboardPart::Health(..) => {
                        if let Some(mut text) = text {
                            set_text(&mut text, &row.health);
                        }
                    }
                    _ => {
                        if let Some(mut text) = text {
                            set_text(&mut text, &row.status);
                        }
                        if let Some(mut color) = color {
                            color.set_if_neq(TextColor(row.status_tone.color()));
                        }
                    }
                }
            }
        }
    }
}

fn set_text(text: &mut Mut<Text>, label: &str) {
    if text.0 != label {
        label.clone_into(&mut text.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(team: u8, name: &str, status: CachedRosterStatus) -> CachedRosterEntry {
        CachedRosterEntry {
            team: TeamId(team),
            display_name: name.to_string(),
            status,
            connected: true,
            held_until: None,
            health: Some((40, 100)),
            boost_stacks: 0,
        }
    }

    fn roster_of(
        entries: impl IntoIterator<Item = (u64, CachedRosterEntry)>,
    ) -> MatchRosterPresentation {
        MatchRosterPresentation {
            match_id: None,
            entries: entries.into_iter().collect(),
        }
    }

    #[test]
    fn sections_are_observer_relative_with_the_local_player_first() {
        let roster = roster_of([
            (1, entry(0, "Ann", CachedRosterStatus::Alive)),
            (2, entry(1, "Bo", CachedRosterStatus::Alive)),
            (3, entry(1, "Cy", CachedRosterStatus::Alive)),
            (4, entry(0, "Dee", CachedRosterStatus::Alive)),
        ]);
        let state = scoreboard_state(&roster, Some(3), Some(TeamId(1)), Some(0), true);
        let [own, other] = &state.sections;
        assert_eq!(
            (own.title.as_str(), own.tone),
            ("YOUR TEAM", SectionTone::Ally)
        );
        assert_eq!(
            own.rows
                .iter()
                .map(|row| (row.name.as_str(), row.local))
                .collect::<Vec<_>>(),
            [("Cy", true), ("Bo", false)]
        );
        assert_eq!(
            (other.title.as_str(), other.tone),
            ("OPPONENTS", SectionTone::Enemy)
        );
        assert_eq!(other.rows[0].name, "Ann");

        let spectator = scoreboard_state(&roster, None, None, Some(0), true);
        assert_eq!(spectator.sections[0].title, "TEAM 1");
        assert_eq!(spectator.sections[1].title, "TEAM 2");
        assert_eq!(spectator.sections[1].tone, SectionTone::Neutral);
    }

    #[test]
    fn rows_use_readable_health_and_status_labels() {
        let tick = brawler::timing::SIMULATION_TICK_HZ;
        let mut offline = entry(0, "Gone", CachedRosterStatus::Alive);
        offline.connected = false;
        let mut boosted = entry(0, "Buff", CachedRosterStatus::Alive);
        boosted.boost_stacks = 2;
        boosted.health = None;
        let mut held = entry(0, "Held", CachedRosterStatus::Respawning(100 + tick));
        held.held_until = Some(100 + 42 * tick);
        let roster = roster_of([
            (1, entry(0, "Down", CachedRosterStatus::Defeated)),
            (
                2,
                entry(0, "Back", CachedRosterStatus::Respawning(100 + 3 * tick)),
            ),
            (
                3,
                entry(0, "Safe", CachedRosterStatus::Protected(100 + tick)),
            ),
            (4, offline),
            (5, boosted),
            (6, held),
        ]);
        let state = scoreboard_state(&roster, None, Some(TeamId(0)), Some(100), true);
        let rows = &state.sections[0].rows;
        let labels: Vec<_> = rows
            .iter()
            .map(|row| (row.health.as_str(), row.status.as_str(), row.status_tone))
            .collect();
        assert_eq!(
            labels,
            [
                ("HP 40/100", "DEFEATED", StatusTone::Down),
                ("HP 40/100", "RESPAWN IN 3s", StatusTone::Down),
                ("HP 40/100", "PROTECTED 1s", StatusTone::Protected),
                ("HP 40/100", "DISCONNECTED", StatusTone::Offline),
                ("HP --", "ALIVE  BOOST +2", StatusTone::Normal),
                ("HP 40/100", "BOT 42s", StatusTone::Offline),
            ]
        );
        assert_eq!(
            rows[0].health_fraction,
            Some(0.0),
            "a defeated bar is empty"
        );
        assert_eq!(rows[1].health_fraction, Some(0.4));
        assert_eq!(rows[4].health_fraction, None);
        assert!(state.sections[1].rows.is_empty());

        // The lobby ready flag outlives the lobby; in play it reads as ALIVE.
        let ready = roster_of([(1, entry(0, "Rdy", CachedRosterStatus::Ready))]);
        let status = |in_play| {
            scoreboard_state(&ready, None, None, Some(0), in_play).sections[0].rows[0]
                .status
                .clone()
        };
        assert_eq!(
            (status(true).as_str(), status(false).as_str()),
            ("ALIVE", "READY")
        );
    }
}
