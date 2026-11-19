//! Map Maker's flat application controls (BRL-2xpc, direction D "Compact tool UI").
//!
//! The shell palette and title font with plain controls: square icon buttons, segmented
//! controls, compact list rows and map tiles. They are local to Map Maker on purpose and are not
//! a shell-toolkit `Surface` variant. Every control is still a `FlowButton`, so it keeps release
//! activation and the touch-target floor; [`style_flat_controls`] owns its fill, selection,
//! hover/press and the flat focus ring.

use super::MapMakerRuntime;
use crate::{
    ClientAssetHandles,
    flow::{ClientFlow, actions::FlowUiAction, screens::shared::FlowButton},
    ui::{
        focus_visibility::FocusVisibility,
        icons,
        theme::{ACCENT, INK, PANEL, PANEL_RAISED, PLAY, PLAY_HIGHLIGHT, TEXT, font, type_stroke},
        touch_target::{MIN_TOUCH_TARGET_DP, TouchTargetPolicy},
        viewport::FIT_REFERENCE_HEIGHT,
    },
};
use bevy::{prelude::*, ui::InteractionDisabled};

/// Body text font: Fira Sans Bold, bundled so macOS and Android render the same glyphs (the
/// design board used the platform sans, which differs between the two).
pub(super) const BODY_FONT_PATH: &str = "brawler/ui/fonts/fira-sans-bold.ttf";

/// Corner radius of flat controls.
pub(super) const RADIUS: f32 = 6.0;
pub(super) const DANGER: Color = Color::srgb(0.78, 0.22, 0.20);

/// Flat bars and panels: `ink` lifted toward `panel`, opaque so the arena never shows through.
pub(super) fn bar_fill() -> Color {
    INK.mix(&PANEL, 0.35)
}

/// Desktop control height in UI pixels.
const DESKTOP_CONTROL: f32 = 40.0;
/// Compact (landscape phone) control height in UI pixels at the 411/640 viewport fit: 48 dp.
const COMPACT_CONTROL: f32 = 76.0;

/// Layout class and control size for the current window, UI scale and touch policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::flow) struct Metrics {
    /// A window shorter than the shell's 640 px reference: the landscape phone layout.
    pub(super) compact: bool,
    /// Standard control height (and square control width) in UI pixels.
    pub(super) ctl: f32,
}

impl Metrics {
    pub(super) fn new(window_height: Option<f32>, ui_scale: f32, touch: bool) -> Self {
        let compact = window_height.is_some_and(|height| height < FIT_REFERENCE_HEIGHT);
        let floor = MIN_TOUCH_TARGET_DP / ui_scale.max(0.01);
        let base = if compact {
            COMPACT_CONTROL
        } else {
            DESKTOP_CONTROL
        };
        // Compact controls are always 48 dp; on a touch platform every control is.
        let ctl = if compact || touch {
            base.max(floor.ceil())
        } else {
            base
        };
        Self { compact, ctl }
    }

    /// Desktop or compact value.
    pub(super) fn pick(self, desktop: f32, compact: f32) -> f32 {
        if self.compact { compact } else { desktop }
    }

    /// Height of the editor's top bar.
    pub(super) fn bar_height(self) -> f32 {
        self.ctl + 8.0
    }
}

/// Reads the layout metrics from the primary window, UI scale and touch policy.
#[derive(bevy::ecs::system::SystemParam)]
pub(in crate::flow) struct MetricsParam<'w, 's> {
    windows: Query<'w, 's, &'static Window, With<bevy::window::PrimaryWindow>>,
    scale: Option<Res<'w, UiScale>>,
    policy: Option<Res<'w, TouchTargetPolicy>>,
}

impl MetricsParam<'_, '_> {
    pub(super) fn get(&self) -> Metrics {
        Metrics::new(
            self.windows.iter().next().map(Window::height),
            self.scale.as_deref().map_or(1.0, |scale| scale.0),
            self.policy.as_deref().is_some_and(|policy| policy.enabled),
        )
    }
}

/// How a flat control is drawn. Selection ("current") and focus come from runtime state each
/// frame, so value-only changes never rebuild the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tone {
    /// A standalone button on a bar or panel.
    Plain,
    /// A segment or icon in a group: transparent until current.
    Quiet,
    /// The orange primary action.
    Primary,
    /// Test while the map check fails: muted, still pressable so it can point to Check.
    Blocked,
    Danger,
    /// A list row in a panel.
    Row,
    /// A map tile in the library.
    Tile,
}

/// A Map Maker flat control (not a shell `Surface`).
#[derive(Component, Clone, Copy, Debug)]
pub(in crate::flow) struct FlatControl {
    tone: Tone,
}

/// The shared helpers every Map Maker view builds with.
pub(super) struct Kit<'a> {
    pub(super) metrics: Metrics,
    pub(super) server: &'a AssetServer,
    pub(super) assets: Option<&'a ClientAssetHandles>,
}

/// One flat button's content and behaviour.
pub(super) struct ButtonSpec<'a> {
    pub(super) index: usize,
    pub(super) action: FlowUiAction,
    pub(super) icon: Option<&'a str>,
    pub(super) label: Option<&'a str>,
    /// Spoken name; defaults to the label or icon.
    pub(super) accessible: Option<&'a str>,
    pub(super) tone: Tone,
    pub(super) width: Val,
    pub(super) disabled: bool,
}

impl<'a> ButtonSpec<'a> {
    pub(super) fn new(index: usize, action: FlowUiAction, tone: Tone) -> Self {
        Self {
            index,
            action,
            icon: None,
            label: None,
            accessible: None,
            tone,
            width: Val::Auto,
            disabled: false,
        }
    }

    pub(super) fn icon(mut self, icon: &'a str) -> Self {
        self.icon = Some(icon);
        self
    }

    pub(super) fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    pub(super) fn named(mut self, accessible: &'a str) -> Self {
        self.accessible = Some(accessible);
        self
    }

    pub(super) fn width(mut self, width: Val) -> Self {
        self.width = width;
        self
    }

    pub(super) fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Kit<'_> {
    pub(super) fn ctl(&self) -> f32 {
        self.metrics.ctl
    }

    pub(super) fn pick(&self, desktop: f32, compact: f32) -> f32 {
        self.metrics.pick(desktop, compact)
    }

    /// Body text in the bundled sans.
    pub(super) fn sans(&self, size: f32) -> TextFont {
        TextFont {
            font: self.server.load(BODY_FONT_PATH).into(),
            font_size: FontSize::Px(size),
            ..default()
        }
    }

    pub(super) fn label(
        &self,
        parent: &mut ChildSpawnerCommands,
        copy: &str,
        size: f32,
        color: Color,
    ) -> Entity {
        parent
            .spawn((Text::new(copy), self.sans(size), TextColor(color)))
            .id()
    }

    /// A single-line label that never wraps (so it never inflates an auto-sized parent).
    pub(super) fn line(
        &self,
        parent: &mut ChildSpawnerCommands,
        copy: &str,
        size: f32,
        color: Color,
    ) -> Entity {
        parent
            .spawn((
                Text::new(copy),
                self.sans(size),
                TextColor(color),
                no_wrap(),
            ))
            .id()
    }

    /// Display type in the shell font with its stroke.
    pub(super) fn title(&self, parent: &mut ChildSpawnerCommands, copy: &str, size: f32) -> Entity {
        parent
            .spawn((
                Text::new(copy),
                font(self.assets, size),
                TextColor(TEXT),
                type_stroke(size * 0.6),
            ))
            .id()
    }

    pub(super) fn icon(
        &self,
        parent: &mut ChildSpawnerCommands,
        name: &str,
        size: f32,
        alpha: f32,
    ) {
        parent.spawn((
            ImageNode {
                color: Color::WHITE.with_alpha(alpha),
                ..ImageNode::new(self.server.load(icons::path(name)))
            },
            Node {
                width: px(size),
                height: px(size),
                flex_shrink: 0.0,
                ..default()
            },
        ));
    }

    /// Text size inside a control.
    pub(super) fn control_text(&self) -> f32 {
        self.pick(14.0, 20.0)
    }

    /// A flat button: an optional icon and short label; square when it has only an icon.
    pub(super) fn button(&self, parent: &mut ChildSpawnerCommands, spec: ButtonSpec) -> Entity {
        let height = self.ctl();
        let accessible = spec
            .accessible
            .or(spec.label)
            .or(spec.icon)
            .unwrap_or("button");
        let width = match (spec.width, spec.label) {
            (Val::Auto, None) => px(height),
            (width, _) => width,
        };
        let mut entity = parent.spawn((
            Button,
            AccessibleLabel::new(accessible),
            FlowButton {
                index: spec.index,
                action: spec.action,
                error_action: false,
            },
            FlatControl { tone: spec.tone },
            Node {
                width,
                min_width: px(height),
                min_height: px(height),
                flex_shrink: 0.0,
                padding: UiRect::axes(px(self.pick(10.0, 14.0)), px(0)),
                column_gap: px(self.pick(6.0, 8.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(RADIUS)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::NONE),
            Outline::new(px(0), px(2), Color::NONE),
        ));
        if spec.disabled {
            entity.insert(InteractionDisabled);
        }
        let alpha = if spec.disabled { 0.45 } else { 1.0 };
        let text_color = if spec.tone == Tone::Primary {
            INK
        } else {
            TEXT
        };
        let icon_size = self.pick(22.0, 36.0);
        let text_size = self.control_text();
        entity.with_children(|button| {
            if let Some(name) = spec.icon {
                self.icon(button, name, icon_size, alpha);
            }
            if let Some(copy) = spec.label {
                self.line(button, copy, text_size, text_color.with_alpha(alpha));
            }
        });
        entity.id()
    }

    /// Joined segmented control around `segments`; their selection follows runtime state.
    pub(super) fn segmented(
        &self,
        parent: &mut ChildSpawnerCommands,
        stretch: bool,
        segments: Vec<ButtonSpec>,
    ) -> Entity {
        parent
            .spawn((
                Node {
                    flex_grow: if stretch { 1.0 } else { 0.0 },
                    flex_shrink: 0.0,
                    padding: UiRect::all(px(2)),
                    column_gap: px(2),
                    border_radius: BorderRadius::all(px(RADIUS + 2.0)),
                    ..default()
                },
                BackgroundColor(INK.with_alpha(0.85)),
            ))
            .with_children(|group| {
                for segment in segments {
                    let id = self.button(group, segment);
                    if stretch {
                        group
                            .commands()
                            .entity(id)
                            .entry::<Node>()
                            .and_modify(|mut node| {
                                node.flex_grow = 1.0;
                                node.flex_basis = px(0);
                            });
                    }
                }
            })
            .id()
    }

    pub(super) fn divider(&self, parent: &mut ChildSpawnerCommands) {
        parent.spawn((
            Node {
                width: px(1),
                height: px(self.ctl() * 0.6),
                flex_shrink: 0.0,
                margin: UiRect::horizontal(px(self.pick(4.0, 6.0))),
                ..default()
            },
            BackgroundColor(TEXT.with_alpha(0.18)),
        ));
    }
}

/// Single-line text that clips instead of wrapping.
pub(super) fn no_wrap() -> TextLayout {
    TextLayout::new(Justify::Left, LineBreak::NoWrap)
}

pub(super) fn row(gap: f32) -> Node {
    Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: px(gap),
        ..default()
    }
}

pub(super) fn column(gap: f32) -> Node {
    Node {
        flex_direction: FlexDirection::Column,
        row_gap: px(gap),
        ..default()
    }
}

pub(super) fn grow() -> Node {
    Node {
        flex_grow: 1.0,
        ..default()
    }
}

/// Fill, border colour and border alpha for a tone in a given interaction state.
fn tone_colors(tone: Tone, current: bool, disabled: bool) -> (Color, Color) {
    if disabled {
        return (PANEL.with_alpha(0.45), PANEL_RAISED.with_alpha(0.3));
    }
    match (tone, current) {
        (Tone::Plain, false) => (PANEL, PANEL_RAISED.with_alpha(0.7)),
        (Tone::Quiet, false) => (Color::NONE, Color::NONE),
        (Tone::Plain | Tone::Quiet | Tone::Row, true) => (ACCENT.with_alpha(0.24), ACCENT),
        (Tone::Primary, _) => (PLAY, PLAY_HIGHLIGHT),
        (Tone::Blocked, _) => (PLAY.mix(&PANEL, 0.6), PLAY.with_alpha(0.5)),
        (Tone::Danger, _) => (DANGER, DANGER.lighter(0.1)),
        (Tone::Row, false) => (PANEL.with_alpha(0.6), Color::NONE),
        (Tone::Tile, false) => (PANEL.with_alpha(0.9), PANEL_RAISED.with_alpha(0.6)),
        (Tone::Tile, true) => (PANEL_RAISED.with_alpha(0.55), ACCENT),
    }
}

/// Draws every flat control from its tone and live state: current choice (runtime-owned),
/// hover/press, disabled, and the flat focus ring. The ring follows the shell's focus rule
/// (BRL-fe7t): an `accent` frame on the focused control, hidden after a touch press.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Bevy presentation system parameters"
)]
pub(in crate::flow) fn style_flat_controls(
    flow: Res<State<ClientFlow>>,
    runtime: Res<MapMakerRuntime>,
    navigation: Res<crate::flow::screens::shared::FlowNavigation>,
    focus: Option<Res<FocusVisibility>>,
    mut controls: Query<(
        &FlatControl,
        &FlowButton,
        &Interaction,
        Has<InteractionDisabled>,
        &mut BackgroundColor,
        &mut BorderColor,
        &mut Outline,
    )>,
) {
    let focus_visible =
        focus.as_deref().is_none_or(|focus| focus.visible) && *flow.get() == ClientFlow::MapLibrary;
    for (control, button, interaction, disabled, mut fill, mut border, mut outline) in &mut controls
    {
        let current = runtime.action_is_current(&button.action);
        let (mut base, edge) = tone_colors(control.tone, current, disabled);
        if !disabled {
            base = match interaction {
                Interaction::Pressed => {
                    if base == Color::NONE {
                        ACCENT.with_alpha(0.3)
                    } else {
                        base.darker(0.08)
                    }
                }
                Interaction::Hovered => {
                    if base == Color::NONE {
                        PANEL_RAISED.with_alpha(0.35)
                    } else {
                        base.lighter(0.06)
                    }
                }
                Interaction::None => base,
            };
        }
        fill.set_if_neq(BackgroundColor(base));
        border.set_if_neq(BorderColor::all(edge));
        let focused = focus_visible && !disabled && navigation.focused == button.index;
        let ring = if focused {
            Outline::new(px(3), px(2), ACCENT)
        } else {
            Outline::new(px(0), px(2), Color::NONE)
        };
        if *outline != ring {
            *outline = ring;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_controls_are_48_dp_at_every_ui_scale() {
        for scale in [411.0 / 640.0, 411.0 / 640.0 * 0.8] {
            let metrics = Metrics::new(Some(411.0), scale, false);
            assert!(metrics.compact);
            assert!(metrics.ctl * scale >= MIN_TOUCH_TARGET_DP);
        }
        let desktop = Metrics::new(Some(720.0), 1.0, false);
        assert!(!desktop.compact);
        assert!((desktop.ctl - DESKTOP_CONTROL).abs() < f32::EPSILON);
        assert!(Metrics::new(Some(720.0), 1.0, true).ctl >= MIN_TOUCH_TARGET_DP);
    }
}
