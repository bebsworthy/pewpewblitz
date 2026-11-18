//! BRL-fe7t review board for the affordance chosen by `BRAWLER_SURFACE_AFFORDANCE` (V cycles it).
//!
//! Two pages (B switches; `BRAWLER_UI_GALLERY_AFFORDANCE_BOARD=choices` opens the second):
//! action buttons, which only take focus, and the choice components that also have a current
//! option. Focus is where keyboard, gamepad or pointer navigation is; current is the option the
//! player has chosen. Choice cells mirror the role, style and content of the real screens' cards
//! (`flow/screens/{game_select,brawlers,equipment,map_maker}.rs`); those spawn helpers need flow
//! actions and profile data, so the board does not call them.

use super::super::{
    icons,
    surface::{ReducedMotionPreview, Surface, SurfaceAffordance, SurfaceState, SurfaceStyle},
    theme,
};
use bevy::prelude::*;

/// Action button columns: no current state.
const ACTION_STATES: [&str; 4] = ["REST", "FOCUS", "PRESSED", "DISABLED"];

/// Choice component columns.
const CHOICE_STATES: [&str; 5] = ["REST", "CURRENT", "FOCUS", "CURRENT + FOCUS", "DISABLED"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Page {
    Actions,
    Choices,
}

#[derive(Clone, Copy)]
enum Modality {
    /// Keyboard, gamepad or pointer: focus is visible.
    Navigation,
    /// The same states with reduced motion: no lift or pulse.
    Reduced,
    /// After a tap: focus is hidden; current and press stay.
    Touch,
}

#[derive(Clone, Copy)]
enum Action {
    Play,
    Practice,
    Fit,
}

/// One action row: label, button, height, input modality.
const ACTION_ROWS: [(&str, Action, f32, Modality); 5] = [
    (
        "PLAY BUTTON\nprimary",
        Action::Play,
        64.0,
        Modality::Navigation,
    ),
    (
        "SECONDARY\nDashboard",
        Action::Practice,
        56.0,
        Modality::Navigation,
    ),
    (
        "ICON BUTTON\nMap Maker",
        Action::Fit,
        50.0,
        Modality::Navigation,
    ),
    (
        "REDUCED MOTION\nsecondary",
        Action::Practice,
        56.0,
        Modality::Reduced,
    ),
    (
        "TOUCH · after tap\nfocus hidden",
        Action::Play,
        64.0,
        Modality::Touch,
    ),
];

#[derive(Clone, Copy)]
enum Choice {
    GameType,
    Brawler,
    Slot,
    Part,
    Tool,
}

impl Choice {
    fn style(self) -> Option<SurfaceStyle> {
        matches!(self, Self::Tool).then_some(SurfaceStyle::MAP_TOOL)
    }

    /// Whether the real screen ever disables this component (while a profile change is pending).
    fn disables(self) -> bool {
        matches!(self, Self::Brawler | Self::Slot | Self::Part)
    }
}

/// One choice row: label, component, height, input modality.
const CHOICE_ROWS: [(&str, Choice, f32, Modality); 6] = [
    (
        "GAME TYPE CARD\nGame Select",
        Choice::GameType,
        90.0,
        Modality::Navigation,
    ),
    (
        "BRAWLER CARD\nbrawler list",
        Choice::Brawler,
        78.0,
        Modality::Navigation,
    ),
    (
        "LOADOUT SLOT\nequip parts",
        Choice::Slot,
        88.0,
        Modality::Navigation,
    ),
    (
        "PART CARD\nequip parts",
        Choice::Part,
        88.0,
        Modality::Navigation,
    ),
    ("TOOL\nMap Maker", Choice::Tool, 80.0, Modality::Navigation),
    (
        "TOUCH · after tap\nfocus hidden",
        Choice::Brawler,
        78.0,
        Modality::Touch,
    ),
];

const LABEL_WIDTH: f32 = 170.0;

#[derive(Component)]
pub(super) struct BoardPage;

#[derive(Component)]
pub(super) struct BoardTitle(Page);

fn title(page: Page, affordance: SurfaceAffordance) -> String {
    let (focus, current) = match affordance {
        SurfaceAffordance::Current => ("cyan frame + pointer", "yellow rim, raised face"),
        SurfaceAffordance::Frame => ("white frame + glow + lift", "gold trim + check badge"),
        SurfaceAffordance::Reticle => ("white corner brackets", "gold bar + check badge"),
        SurfaceAffordance::Spotlight => ("lit face + white bevel", "gold outline + check badge"),
    };
    let name = affordance.name().to_uppercase();
    match page {
        Page::Actions => format!("ACTION BUTTONS · focus only · {name}: focus = {focus}"),
        Page::Choices => {
            format!("CHOICES · current vs focus · {name}: current = {current}; focus = {focus}")
        }
    }
}

fn initial_page() -> Page {
    if std::env::var("BRAWLER_UI_GALLERY_AFFORDANCE_BOARD").is_ok_and(|value| value == "choices") {
        Page::Choices
    } else {
        Page::Actions
    }
}

pub(super) fn spawn(root: &mut ChildSpawnerCommands, font: &TextFont, assets: &AssetServer) {
    let shown = initial_page();
    for page in [Page::Actions, Page::Choices] {
        root.spawn((
            BoardPage,
            Node {
                display: if page == shown {
                    Display::Flex
                } else {
                    Display::None
                },
                width: percent(100),
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(px(28), px(10)),
                row_gap: px(4),
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .with_children(|board| {
            board.spawn((
                BoardTitle(page),
                Text::new(title(page, SurfaceAffordance::Current)),
                sized(font, 24.0),
                TextColor(theme::TEXT),
            ));
            board.spawn((
                Text::new(
                    "FOCUS = where your keyboard, gamepad or mouse cursor is.   \
                     CURRENT = the option you have chosen.",
                ),
                sized(font, 16.0),
                TextColor(theme::PLAY_HIGHLIGHT),
            ));
            match page {
                Page::Actions => {
                    header(board, font, &ACTION_STATES);
                    for (label, action, height, modality) in ACTION_ROWS {
                        board.spawn(row_node()).with_children(|row| {
                            row_label(row, font, label);
                            for column in 0..ACTION_STATES.len() {
                                action_cell(row, font, assets, action, height, modality, column);
                            }
                        });
                    }
                }
                Page::Choices => {
                    header(board, font, &CHOICE_STATES);
                    for (label, choice, height, modality) in CHOICE_ROWS {
                        board.spawn(row_node()).with_children(|row| {
                            row_label(row, font, label);
                            for column in 0..CHOICE_STATES.len() {
                                choice_cell(row, font, assets, choice, height, modality, column);
                            }
                        });
                    }
                }
            }
        });
    }
}

fn sized(font: &TextFont, size: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Px(size),
        ..font.clone()
    }
}

fn header(board: &mut ChildSpawnerCommands, font: &TextFont, states: &[&str]) {
    board.spawn(row_node()).with_children(|header| {
        header.spawn(label_node());
        for state in states {
            header.spawn((
                Text::new(*state),
                sized(font, 18.0),
                TextColor(theme::ACCENT),
                TextLayout::justify(Justify::Center),
                cell_node(),
            ));
        }
    });
}

fn row_label(row: &mut ChildSpawnerCommands, font: &TextFont, label: &str) {
    row.spawn(label_node()).with_child((
        Text::new(label),
        sized(font, 17.0),
        TextColor(theme::TEXT),
    ));
}

fn row_node() -> Node {
    Node {
        width: percent(100),
        column_gap: px(18),
        align_items: AlignItems::Center,
        flex_shrink: 0.0,
        ..default()
    }
}

fn label_node() -> Node {
    Node {
        width: px(LABEL_WIDTH),
        flex_shrink: 0.0,
        ..default()
    }
}

/// An equal-width column that centers its sample.
fn cell_node() -> Node {
    Node {
        flex_grow: 1.0,
        flex_basis: px(0),
        min_width: px(0),
        justify_content: JustifyContent::Center,
        ..default()
    }
}

fn text(font: &TextFont, copy: &str, size: f32) -> impl Bundle {
    (
        Text::new(copy),
        sized(font, size),
        TextColor(theme::TEXT),
        TextLayout::justify(Justify::Center),
    )
}

fn image(assets: &AssetServer, name: &str, width: f32, height: f32) -> impl Bundle {
    (
        ImageNode::new(assets.load(icons::path(name))),
        Node {
            width: px(width),
            height: px(height),
            flex_shrink: 0.0,
            ..default()
        },
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "one board cell is fully described by its row and column"
)]
fn action_cell(
    row: &mut ChildSpawnerCommands,
    font: &TextFont,
    assets: &AssetServer,
    action: Action,
    height: f32,
    modality: Modality,
    column: usize,
) {
    let focus_visible = !matches!(modality, Modality::Touch);
    let state = SurfaceState {
        focused: focus_visible && matches!(column, 1 | 2),
        current: false,
        inactive: false,
        disabled: column == 3,
    };
    let role = match action {
        Action::Play => Surface::Primary,
        Action::Practice => Surface::Secondary,
        Action::Fit => Surface::Dark,
    };
    row.spawn(cell_node()).with_children(|cell| {
        let mut button = cell.spawn((
            role,
            state,
            Node {
                width: percent(100),
                height: px(height),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: px(7),
                ..default()
            },
        ));
        match action {
            Action::Play => {
                button.with_child(text(font, "PLAY", 24.0));
            }
            Action::Practice => {
                button.with_child(text(font, "PRACTICE", 22.0));
            }
            Action::Fit => {
                // Map Maker compact icon action: icon then label (`compact_icon_action_button`).
                button.insert(SurfaceStyle::MAP_ACTION).with_children(|b| {
                    b.spawn(image(assets, "fit", 23.0, 23.0));
                    b.spawn(text(font, "FIT", 15.0));
                });
            }
        }
        if column == 2 {
            button.insert((Interaction::Pressed, super::HeldPressDemo));
        }
        if matches!(modality, Modality::Reduced) {
            button.insert(ReducedMotionPreview);
        }
    });
}

#[allow(
    clippy::too_many_arguments,
    reason = "one board cell is fully described by its row and column"
)]
fn choice_cell(
    row: &mut ChildSpawnerCommands,
    font: &TextFont,
    assets: &AssetServer,
    choice: Choice,
    height: f32,
    modality: Modality,
    column: usize,
) {
    row.spawn(cell_node()).with_children(|cell| {
        if column == 4 && !choice.disables() {
            cell.spawn((
                Text::new("never disabled"),
                sized(font, 15.0),
                TextColor(theme::TEXT.with_alpha(0.55)),
            ));
            return;
        }
        let focus_visible = !matches!(modality, Modality::Touch);
        let state = SurfaceState {
            focused: focus_visible && matches!(column, 2 | 3),
            current: matches!(column, 1 | 3),
            inactive: false,
            disabled: column == 4,
        };
        let mut component = cell.spawn((
            // Every real choice component here is a `Surface::Card` (`surface_role`, `tool_button`).
            Surface::Card,
            state,
            Node {
                width: if matches!(choice, Choice::Tool) {
                    px(94)
                } else {
                    percent(100)
                },
                height: px(height),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                // Clear the ink border on top and the bottom lip.
                padding: UiRect {
                    left: px(10),
                    right: px(10),
                    top: px(8),
                    bottom: px(20),
                },
                row_gap: px(1),
                ..default()
            },
        ));
        if let Some(style) = choice.style() {
            component.insert(style);
        }
        component.with_children(|c| match choice {
            Choice::GameType => {
                c.spawn(image(assets, icons::MODE_PENNANTS[0], 40.0, 50.0));
                c.spawn(text(font, "WIPEOUT", 18.0));
            }
            Choice::Brawler => {
                c.spawn(text(font, "PULSE", 22.0));
                c.spawn(text(font, "Blaster", 15.0));
            }
            Choice::Slot => {
                c.spawn(text(font, "1", 14.0));
                c.spawn(image(assets, "part-barrel", 36.0, 30.0));
                c.spawn(text(font, "Long Barrel", 14.0));
            }
            Choice::Part => {
                c.spawn(image(assets, "part-magazine", 36.0, 30.0));
                c.spawn(text(font, "DRUM MAG", 15.0));
                c.spawn(text(font, "+2 ammo", 13.0));
            }
            Choice::Tool => {
                c.spawn(image(assets, "place", 32.0, 32.0));
                c.spawn(text(font, "PLACE", 15.0));
            }
        });
    });
}

/// V cycles the reviewed affordance in the running gallery.
#[allow(clippy::needless_pass_by_value, reason = "isolated gallery input")]
pub(super) fn cycle_affordance(
    keys: Res<ButtonInput<KeyCode>>,
    mut affordance: ResMut<SurfaceAffordance>,
    mut titles: Query<(&mut Text, &BoardTitle)>,
) {
    if keys.just_pressed(KeyCode::KeyV) {
        let all = SurfaceAffordance::ALL;
        let next = all
            .iter()
            .position(|variant| *variant == *affordance)
            .map_or(0, |index| (index + 1) % all.len());
        *affordance = all[next];
    }
    if affordance.is_changed() {
        for (mut text, title_of) in &mut titles {
            text.0 = title(title_of.0, *affordance);
        }
    }
}

/// B switches between the action and choice pages.
#[allow(clippy::needless_pass_by_value, reason = "isolated gallery input")]
pub(super) fn switch_page(
    keys: Res<ButtonInput<KeyCode>>,
    mut pages: Query<&mut Node, With<BoardPage>>,
) {
    if !keys.just_pressed(KeyCode::KeyB) {
        return;
    }
    for mut node in &mut pages {
        node.display = if node.display == Display::None {
            Display::Flex
        } else {
            Display::None
        };
    }
}
