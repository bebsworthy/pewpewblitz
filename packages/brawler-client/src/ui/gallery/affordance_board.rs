//! BRL-fe7t review board: every focus/selection state on the main surface roles, plus reduced
//! motion and touch, for the affordance chosen by `BRAWLER_SURFACE_AFFORDANCE` (V cycles it).

use super::super::{
    surface::{ReducedMotionPreview, Surface, SurfaceAffordance, SurfaceState},
    theme,
};
use bevy::prelude::*;

/// Board columns, in order.
const STATES: [&str; 6] = [
    "REST",
    "FOCUS",
    "SELECTED",
    "FOCUS + SELECTED",
    "PRESSED",
    "DISABLED",
];

#[derive(Clone, Copy)]
enum Modality {
    /// Keyboard or gamepad: focus is visible.
    Navigation,
    /// The same states with reduced motion: no lift or pulse.
    Reduced,
    /// After touch input: focus is hidden, selection and press stay.
    Touch,
}

/// One board row: label, role, cell copy, height, and input modality.
const ROWS: [(&str, Surface, &str, f32, Modality); 6] = [
    (
        "PRIMARY",
        Surface::Primary,
        "PLAY",
        64.0,
        Modality::Navigation,
    ),
    (
        "SECONDARY",
        Surface::Secondary,
        "PRACTICE",
        56.0,
        Modality::Navigation,
    ),
    (
        "CARD",
        Surface::Card,
        "BRAWLER\nPulse",
        92.0,
        Modality::Navigation,
    ),
    ("ROW", Surface::Row, "Sound", 56.0, Modality::Navigation),
    (
        "REDUCED MOTION\nsecondary",
        Surface::Secondary,
        "PRACTICE",
        56.0,
        Modality::Reduced,
    ),
    (
        "TOUCH\nfocus hidden",
        Surface::Row,
        "Sound",
        56.0,
        Modality::Touch,
    ),
];

const LABEL_WIDTH: f32 = 170.0;

#[derive(Component)]
pub(super) struct BoardTitle;

fn title(affordance: SurfaceAffordance) -> String {
    let blurb = match affordance {
        SurfaceAffordance::Current => "baseline: cyan frame + pointer, yellow rim",
        SurfaceAffordance::Frame => "white frame + glow + lift, gold trim",
        SurfaceAffordance::Reticle => "white corner brackets, gold underline",
        SurfaceAffordance::Spotlight => "lit face + bevel, gold outline",
    };
    format!(
        "FOCUS & SELECTION · {} — {blurb}",
        affordance.name().to_uppercase()
    )
}

pub(super) fn spawn(root: &mut ChildSpawnerCommands, font: &TextFont) {
    let small = TextFont {
        font_size: FontSize::Px(18.0),
        ..font.clone()
    };
    root.spawn(Node {
        width: percent(100),
        flex_direction: FlexDirection::Column,
        padding: UiRect::axes(px(28), px(20)),
        row_gap: px(14),
        flex_shrink: 0.0,
        ..default()
    })
    .with_children(|board| {
        board.spawn((
            BoardTitle,
            Text::new(title(SurfaceAffordance::Current)),
            TextFont {
                font_size: FontSize::Px(28.0),
                ..font.clone()
            },
            TextColor(theme::TEXT),
        ));
        board.spawn(row_node()).with_children(|header| {
            header.spawn(label_node());
            for state in STATES {
                header.spawn((
                    Text::new(state),
                    small.clone(),
                    TextColor(theme::ACCENT),
                    TextLayout::justify(Justify::Center),
                    Node {
                        flex_grow: 1.0,
                        flex_basis: px(0),
                        ..default()
                    },
                ));
            }
        });
        for (label, role, copy, height, modality) in ROWS {
            board.spawn(row_node()).with_children(|row| {
                row.spawn(label_node()).with_child((
                    Text::new(label),
                    small.clone(),
                    TextColor(theme::TEXT),
                ));
                for column in 0..STATES.len() {
                    cell(row, font, role, copy, height, modality, column);
                }
            });
        }
    });
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

fn cell(
    row: &mut ChildSpawnerCommands,
    font: &TextFont,
    role: Surface,
    copy: &str,
    height: f32,
    modality: Modality,
    column: usize,
) {
    let focus_visible = !matches!(modality, Modality::Touch);
    let state = SurfaceState {
        focused: focus_visible && matches!(column, 1 | 3 | 4),
        selected: matches!(column, 2 | 3),
        inactive: false,
        disabled: column == 5,
    };
    let mut cell = row.spawn((
        role,
        state,
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            min_width: px(0),
            height: px(height),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
    ));
    cell.with_child((
        Text::new(copy),
        TextFont {
            font_size: FontSize::Px(22.0),
            ..font.clone()
        },
        TextColor(theme::TEXT),
        TextLayout::justify(Justify::Center),
    ));
    if column == 4 {
        cell.insert((Interaction::Pressed, super::HeldPressDemo));
    }
    if matches!(modality, Modality::Reduced) {
        cell.insert(ReducedMotionPreview);
    }
}

/// V cycles the reviewed affordance in the running gallery.
#[allow(clippy::needless_pass_by_value, reason = "isolated gallery input")]
pub(super) fn cycle_affordance(
    keys: Res<ButtonInput<KeyCode>>,
    mut affordance: ResMut<SurfaceAffordance>,
    mut titles: Query<&mut Text, With<BoardTitle>>,
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
        for mut text in &mut titles {
            text.0 = title(*affordance);
        }
    }
}
