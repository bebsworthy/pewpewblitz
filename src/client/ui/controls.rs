//! Settings control presentation. Owners handle activation, adjustment and persistence.

use super::theme::{CREAM, INK, ORANGE, TEAL};
use bevy::{
    prelude::*,
    ui::{InteractionDisabled, RelativeCursorPosition, UiSystems},
};

pub(crate) struct UiControlsPlugin;

impl Plugin for UiControlsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (
                present_controls,
                present_switches,
                present_indicators,
                sync_surface_states,
            )
                .before(UiSystems::Layout),
        );
    }
}

#[derive(Component, Clone)]
pub(crate) enum ControlValue {
    Action,
    Tab,
    Toggle(bool),
    /// Normalized display position; the screen owns range, step and units.
    Slider {
        fraction: f32,
        label: String,
    },
    Binding(String),
}

#[derive(Component, Default)]
pub(crate) struct ControlState {
    pub(crate) focused: bool,
    pub(crate) selected: bool,
    pub(crate) inactive: bool,
}

#[derive(Component)]
struct ValueLabel(Entity);

#[derive(Component)]
pub(crate) struct SliderTrack(pub(crate) Entity);

/// Bevy reports the track center as zero, with edges at -0.5 and +0.5.
pub(crate) fn slider_fraction(cursor: &RelativeCursorPosition) -> Option<f32> {
    cursor
        .normalized
        .filter(|_| cursor.cursor_over())
        .map(|position| (position.x + 0.5).clamp(0.0, 1.0))
}

#[derive(Component)]
struct SliderFill(Entity);

#[derive(Component)]
struct SliderKnob(Entity);

#[derive(Component)]
pub(super) struct FocusMarker(Entity);

#[derive(Component)]
struct SwitchTrack(Entity);

/// Selected value and navigation focus are deliberately independent.
pub(crate) fn colors(
    state: &ControlState,
    interaction: Interaction,
    disabled: bool,
) -> (Color, Color) {
    if disabled {
        return (CREAM.mix(&INK, 0.12), INK.with_alpha(0.35));
    }
    let background = if state.selected { TEAL } else { CREAM };
    if state.inactive {
        return (background, INK);
    }
    if interaction == Interaction::Pressed {
        (background.mix(&INK, 0.12), ORANGE)
    } else if state.focused {
        (
            if state.selected {
                TEAL
            } else {
                CREAM.mix(&ORANGE, 0.22)
            },
            ORANGE,
        )
    } else if interaction == Interaction::Hovered {
        (background.mix(&TEAL, 0.16), TEAL)
    } else {
        (background, INK)
    }
}

pub(crate) fn spawn_control(
    parent: &mut ChildSpawnerCommands,
    font: &TextFont,
    label: &str,
    value: ControlValue,
) -> Entity {
    let slider = matches!(value, ControlValue::Slider { .. });
    let toggle = matches!(value, ControlValue::Toggle(_));
    let binding = matches!(value, ControlValue::Binding(_));
    let mut entity = parent.spawn((
        Button,
        ControlState::default(),
        super::surface::Surface::Row,
        super::surface::SurfaceState::default(),
        value,
        Node {
            width: percent(100),
            min_height: px(56),
            flex_shrink: 0.0,
            padding: UiRect::axes(px(16), px(10)),
            border: UiRect::all(px(2)),
            column_gap: px(16),
            border_radius: BorderRadius::all(px(6)),
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(CREAM),
        BorderColor::all(INK),
    ));
    let owner = entity.id();
    entity.with_children(|row| {
        row.spawn((
            Text::new(label),
            font.clone(),
            TextColor(INK),
            Node {
                flex_grow: 1.0,
                min_width: px(0),
                ..default()
            },
        ));
        row.spawn((
            FocusMarker(owner),
            Text::new("›"),
            font.clone(),
            TextColor(INK),
            Node {
                position_type: PositionType::Absolute,
                left: px(-18),
                top: percent(50),
                margin: UiRect::top(px(-16)),
                display: Display::None,
                ..default()
            },
        ));
        if slider {
            spawn_slider(row, owner);
        }
        if toggle {
            spawn_switch(row, owner);
        } else {
            let mut text = row.spawn((
                ValueLabel(owner),
                Text::new(""),
                font.clone(),
                TextColor(INK),
            ));
            if binding {
                text.insert((
                    Node {
                        min_width: px(64),
                        padding: UiRect::axes(px(14), px(4)),
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(px(6)),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    TextLayout::new(Justify::Center, LineBreak::NoWrap),
                    BackgroundColor(CREAM),
                    BorderColor::all(INK),
                ));
            }
        }
    });
    owner
}

fn spawn_slider(parent: &mut ChildSpawnerCommands, owner: Entity) {
    parent
        .spawn((
            SliderTrack(owner),
            RelativeCursorPosition::default(),
            Node {
                width: percent(28),
                min_width: px(100),
                height: px(32),
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .with_children(|track| {
            track.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: px(12),
                    width: percent(100),
                    height: px(8),
                    border_radius: BorderRadius::all(px(4)),
                    ..default()
                },
                BackgroundColor(TEAL.with_alpha(0.25)),
            ));
            track.spawn((
                SliderFill(owner),
                Node {
                    position_type: PositionType::Absolute,
                    top: px(12),
                    width: percent(0),
                    height: px(8),
                    border_radius: BorderRadius::all(px(4)),
                    ..default()
                },
                BackgroundColor(TEAL),
            ));
            track.spawn((
                SliderKnob(owner),
                Node {
                    position_type: PositionType::Absolute,
                    top: px(4),
                    left: percent(0),
                    margin: UiRect::left(px(-12)),
                    width: px(24),
                    height: px(24),
                    border: UiRect::all(px(3)),
                    border_radius: BorderRadius::all(px(12)),
                    ..default()
                },
                BackgroundColor(CREAM),
                BorderColor::all(INK),
            ));
        });
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Bevy control indicator presentation"
)]
fn present_indicators(
    controls: Query<(&ControlState, &ControlValue, Has<InteractionDisabled>)>,
    mut knobs: Query<(&SliderKnob, &mut Node, &mut BorderColor), Without<FocusMarker>>,
    mut markers: Query<(&FocusMarker, &mut Node), Without<SliderKnob>>,
) {
    for (owner, mut node, mut border) in &mut knobs {
        if let Ok((_, ControlValue::Slider { fraction, .. }, disabled)) = controls.get(owner.0) {
            node.left = percent(fraction.clamp(0.0, 1.0) * 100.0);
            border.set_all(if disabled { INK.with_alpha(0.35) } else { INK });
        }
    }
    for (owner, mut node) in &mut markers {
        if let Ok((state, _, disabled)) = controls.get(owner.0) {
            node.display = if state.focused && !state.inactive && !disabled {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
}

fn spawn_switch(parent: &mut ChildSpawnerCommands, owner: Entity) {
    parent
        .spawn((
            SwitchTrack(owner),
            Node {
                width: px(64),
                height: px(32),
                padding: UiRect::all(px(4)),
                border_radius: BorderRadius::all(px(16)),
                align_items: AlignItems::Center,
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(TEAL),
        ))
        .with_child((
            Node {
                width: px(24),
                height: px(24),
                border_radius: BorderRadius::all(px(12)),
                ..default()
            },
            BackgroundColor(CREAM),
        ));
}

#[allow(clippy::needless_pass_by_value, reason = "Bevy switch presentation")]
fn present_switches(
    values: Query<(&ControlValue, Has<InteractionDisabled>)>,
    mut switches: Query<(&SwitchTrack, &mut Node, &mut BackgroundColor)>,
) {
    for (owner, mut node, mut background) in &mut switches {
        if let Ok((ControlValue::Toggle(on), disabled)) = values.get(owner.0) {
            node.justify_content = if *on {
                JustifyContent::FlexEnd
            } else {
                JustifyContent::FlexStart
            };
            let fill = if *on { ORANGE } else { INK.mix(&CREAM, 0.55) };
            background.0 = if disabled { fill.with_alpha(0.4) } else { fill };
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Bevy control presentation queries"
)]
fn present_controls(
    mut controls: Query<
        (
            &ControlState,
            &Interaction,
            Has<InteractionDisabled>,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        Without<MaterialNode<super::surface::SurfaceMaterial>>,
    >,
    values: Query<&ControlValue>,
    mut labels: Query<(&ValueLabel, &mut Text)>,
    mut fills: Query<(&SliderFill, &mut Node)>,
) {
    for (state, interaction, disabled, mut background, mut border) in &mut controls {
        let (fill, outline) = colors(state, *interaction, disabled);
        background.0 = fill;
        border.set_all(outline);
    }
    for (owner, mut text) in &mut labels {
        let Ok(value) = values.get(owner.0) else {
            continue;
        };
        let label = match value {
            ControlValue::Action => "›",
            ControlValue::Tab | ControlValue::Toggle(_) => "",
            ControlValue::Slider { label, .. } | ControlValue::Binding(label) => label,
        };
        if text.0 != label {
            label.clone_into(&mut text.0);
        }
    }
    for (owner, mut node) in &mut fills {
        if let Ok(ControlValue::Slider { fraction, .. }) = values.get(owner.0) {
            node.width = percent(fraction.clamp(0.0, 1.0) * 100.0);
        }
    }
}

#[allow(clippy::type_complexity, reason = "presentation state bridge")]
fn sync_surface_states(
    mut controls: Query<(
        &ControlState,
        Has<InteractionDisabled>,
        &mut super::surface::SurfaceState,
    )>,
) {
    for (state, disabled, mut surface) in &mut controls {
        *surface = super::surface::SurfaceState {
            focused: state.focused,
            selected: state.selected,
            inactive: state.inactive,
            disabled,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_surface_does_not_receive_fallback_rectangle_colors() {
        let mut app = App::new();
        app.add_plugins(UiControlsPlugin);
        let entity = app
            .world_mut()
            .spawn((
                Button,
                ControlState {
                    focused: true,
                    selected: true,
                    inactive: false,
                },
                MaterialNode::<super::super::surface::SurfaceMaterial>(Handle::default()),
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<BackgroundColor>(entity).unwrap().0,
            Color::NONE
        );
    }

    #[test]
    fn slider_knob_tracks_endpoints_and_disabled_focus_is_hidden() {
        let mut app = App::new();
        app.add_plugins(UiControlsPlugin);
        let mut owner = Entity::PLACEHOLDER;
        app.world_mut()
            .commands()
            .spawn(Node::default())
            .with_children(|parent| {
                owner = spawn_control(
                    parent,
                    &TextFont::default(),
                    "Scale",
                    ControlValue::Slider {
                        fraction: -0.2,
                        label: "80%".into(),
                    },
                );
            });
        app.world_mut().flush();
        app.world_mut().entity_mut(owner).insert(ControlState {
            focused: true,
            ..default()
        });
        app.update();
        let mut knobs = app.world_mut().query::<(&SliderKnob, &Node)>();
        assert_eq!(knobs.single(app.world()).unwrap().1.left, percent(0));
        let mut markers = app.world_mut().query::<(&FocusMarker, &Node)>();
        assert_eq!(
            markers.single(app.world()).unwrap().1.display,
            Display::Flex
        );
        app.world_mut().entity_mut(owner).insert((
            InteractionDisabled,
            ControlValue::Slider {
                fraction: 1.2,
                label: "140%".into(),
            },
        ));
        app.update();
        assert_eq!(knobs.single(app.world()).unwrap().1.left, percent(100));
        assert_eq!(
            markers.single(app.world()).unwrap().1.display,
            Display::None
        );
    }

    #[test]
    fn focused_category_still_shows_its_selection() {
        let mut state = ControlState {
            focused: true,
            ..default()
        };
        let before = colors(&state, Interaction::None, false);
        state.selected = true;
        let after = colors(&state, Interaction::None, false);
        assert_ne!(before.0, after.0);
        assert_eq!(after, (TEAL, ORANGE));
    }

    #[test]
    fn toggling_moves_the_switch_knob_and_changes_its_fill() {
        let mut app = App::new();
        app.add_plugins(UiControlsPlugin);
        let mut owner = Entity::PLACEHOLDER;
        app.world_mut()
            .commands()
            .spawn(Node::default())
            .with_children(|parent| {
                owner = spawn_control(
                    parent,
                    &TextFont::default(),
                    "Reduced motion",
                    ControlValue::Toggle(false),
                );
            });
        app.world_mut().flush();
        app.update();
        let mut switches = app
            .world_mut()
            .query::<(&SwitchTrack, &Node, &BackgroundColor)>();
        let (_, node, color) = switches.single(app.world()).unwrap();
        assert_eq!(node.justify_content, JustifyContent::FlexStart);
        let off = color.0;
        app.world_mut()
            .entity_mut(owner)
            .insert(ControlValue::Toggle(true));
        app.update();
        let (_, node, color) = switches.single(app.world()).unwrap();
        assert_eq!(node.justify_content, JustifyContent::FlexEnd);
        assert_ne!(off, color.0);
    }

    #[test]
    fn selection_and_focus_have_distinct_treatments() {
        let selected = ControlState {
            selected: true,
            ..default()
        };
        let focused = ControlState {
            focused: true,
            ..default()
        };
        assert_ne!(
            colors(&selected, Interaction::None, false),
            colors(&focused, Interaction::None, false)
        );
        let inactive = ControlState {
            selected: true,
            focused: true,
            inactive: true,
        };
        assert_eq!(colors(&inactive, Interaction::Pressed, false), (TEAL, INK));
        assert_eq!(
            colors(&selected, Interaction::None, true),
            colors(&focused, Interaction::Pressed, true)
        );
    }
}
