//! Fullscreen Practice map browsing; focus is independent from the requested map.
use super::dashboard::{CREAM, INK, dashboard_font};
use super::shared::FlowRoot;
use crate::client::{
    ClientLobbyMembership, ClientPracticeModel,
    flow::{
        ClientFlow, ClientOverlay, SelectedGameType,
        actions::{FlowUiAction, PendingFlowActions},
        input::queue_ui_action,
    },
};
use crate::map::MapPresetId;
use bevy::{
    input::mouse::{MouseScrollUnit, MouseWheel},
    prelude::*,
};
use lightyear::prelude::client::Client;

#[derive(Resource, Default)]
pub(in crate::client::flow) struct PracticeCarousel {
    maps: Vec<MapPresetId>,
    slot: usize,
    start: bool,
    stride: f32,
    offset: f32,
    wheel: f32,
}
impl PracticeCarousel {
    fn step(&mut self, right: bool) {
        let n = self.maps.len();
        if n < 2 {
            return;
        }
        self.start = false;
        if n <= 2 {
            self.slot = (self.slot + if right { 1 } else { n - 1 }) % n;
        } else {
            // Rebase before another step at the outer copy, preserving visible geometry.
            if self.slot == 0 {
                self.slot += n;
                self.offset +=
                    f32::from(u16::try_from(n).expect("bounded map count")) * self.stride;
            }
            if self.slot == 3 * n - 1 {
                self.slot -= n;
                self.offset -=
                    f32::from(u16::try_from(n).expect("bounded map count")) * self.stride;
            }
            self.slot = if right { self.slot + 1 } else { self.slot - 1 };
        }
    }
    fn target(&self) -> f32 {
        if self.maps.len() <= 2 {
            0.0
        } else {
            f32::from(u16::try_from(self.slot).expect("bounded carousel slot")) * self.stride
        }
    }
}
#[derive(Component)]
pub(in crate::client::flow) struct PracticeMapRoot {
    width: f32,
    height: f32,
    maps: Vec<MapPresetId>,
}
#[derive(Component)]
pub(in crate::client::flow) struct PracticeMapRow;
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(in crate::client::flow) enum PracticeControl {
    Map(usize),
    Previous,
    Next,
    Start,
}
#[derive(Component)]
pub(in crate::client::flow) struct PracticeChrome {
    normal: Handle<Image>,
    selected: Handle<Image>,
    focus: Handle<Image>,
    selected_focus: Handle<Image>,
}

#[allow(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    reason = "screen snapshot reads authenticated catalog, viewport and presentation assets"
)]
pub(in crate::client::flow) fn present_practice_maps(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    selection: Res<SelectedGameType>,
    modes: Res<crate::modes::ModeRegistry>,
    practice: Res<ClientPracticeModel>,
    mut carousel: ResMut<PracticeCarousel>,
    assets: Option<Res<crate::client::ClientAssetHandles>>,
    server: Option<Res<AssetServer>>,
    windows: Query<&Window>,
    scale: Option<Res<UiScale>>,
    roots: Query<(Entity, &PracticeMapRoot)>,
) {
    if *flow.get() != ClientFlow::PracticeMapSelect {
        return;
    }
    let Some(game) = memberships.iter().next().and_then(|m| {
        m.game_types
            .iter()
            .find(|g| Some(&g.id) == selection.game_type_id.as_ref())
    }) else {
        return;
    };
    let scale = scale.map_or(1.0, |s| s.0);
    let width = windows.iter().next().map_or(1600.0, Window::width) / scale;
    let height = windows.iter().next().map_or(900.0, Window::height) / scale;
    if roots.iter().any(|(_, r)| {
        r.maps == game.practice_map_preset_ids
            && (r.width - width).abs() < 0.5
            && (r.height - height).abs() < 0.5
    }) {
        return;
    }
    let entering = roots.is_empty();
    for (e, _) in &roots {
        commands.entity(e).despawn();
    }
    let factor = (width / 1600.0).min(height / 900.0).clamp(0.5, 1.25);
    let maps = game.practice_map_preset_ids.clone();
    let n = maps.len();
    let card_width = if n == 1 {
        (width - 100.0 * factor).min(900.0 * factor)
    } else if n == 2 {
        (width - 100.0 * factor) / 2.0
    } else {
        (width - 100.0 * factor) * 0.58
    };
    carousel.stride = card_width + 24.0 * factor;
    if entering || carousel.maps != maps {
        carousel.slot = maps
            .iter()
            .position(|m| {
                practice
                    .selected_map
                    .as_ref()
                    .is_some_and(|(_, selected)| selected == m)
            })
            .unwrap_or(0)
            + if n > 2 { n } else { 0 };
        carousel.start = false;
    }
    carousel.maps.clone_from(&maps);
    carousel.offset = carousel.target();
    let mode = modes
        .descriptor_for_definition(game.mode_definition_id)
        .and_then(|m| m.presentation)
        .map_or("PRACTICE", |p| p.selection_label)
        .split(" ·")
        .next()
        .unwrap_or("PRACTICE")
        .to_uppercase();
    let subtitle = if game.team_count == 1 {
        format!("{mode} · {} PLAYERS", game.players_per_team)
    } else {
        format!(
            "{mode} · {}v{}",
            game.players_per_team, game.players_per_team
        )
    };
    let view = MapPickerView {
        assets: assets.as_deref(),
        server: server.as_deref(),
        factor,
        card_width,
        image_height: (height - 478.0 * factor).max(100.0),
        catalog: crate::map::MapContentCatalog::embedded().ok(),
    };
    commands
        .spawn((
            FlowRoot,
            PracticeMapRoot {
                width,
                height,
                maps: maps.clone(),
            },
            DespawnOnExit(ClientFlow::PracticeMapSelect),
            Node {
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(28.0 * factor)),
                flex_direction: FlexDirection::Column,
                row_gap: px(20.0 * factor),
                ..default()
            },
            BackgroundColor(CREAM),
            GlobalZIndex(410),
        ))
        .with_children(|root| {
            view.header(root, &subtitle);
            view.cards(root, &maps, carousel.offset);
            view.footer(root, n);
        });
}

struct MapPickerView<'a> {
    assets: Option<&'a crate::client::ClientAssetHandles>,
    server: Option<&'a AssetServer>,
    factor: f32,
    card_width: f32,
    image_height: f32,
    catalog: Option<crate::map::MapContentCatalog>,
}
impl MapPickerView<'_> {
    fn header(&self, root: &mut ChildSpawnerCommands, game: &str) {
        if let Some(server) = self.server {
            root.spawn((
                ImageNode {
                    image_mode: NodeImageMode::Stretch,
                    ..ImageNode::new(server.load("brawler/ui/shell/game-selection-background.png"))
                },
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    left: px(0),
                    top: px(0),
                    ..default()
                },
                ZIndex(-1),
            ));
        }
        root.spawn(Node {
            width: percent(100),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            flex_shrink: 0.0,
            height: px(110.0 * self.factor),
            ..default()
        })
        .with_children(|header| {
            header
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    ..default()
                })
                .with_children(|copy| {
                    label(copy, self.assets, "CHOOSE PRACTICE MAP", 54.0 * self.factor);
                    label(copy, self.assets, &game.to_uppercase(), 28.0 * self.factor);
                });
            if let Some(a) = self.assets {
                header.spawn((
                    ImageNode::new(a.dashboard_logo.clone()),
                    Node {
                        width: px(170.0 * self.factor),
                        ..default()
                    },
                ));
            }
        });
    }
    fn cards(&self, root: &mut ChildSpawnerCommands, maps: &[MapPresetId], offset: f32) {
        let n = maps.len();
        root.spawn(Node {
            width: percent(100),
            flex_grow: 1.0,
            min_height: px(0),
            overflow: Overflow::clip(),
            ..default()
        })
        .with_children(|viewport| {
            viewport
                .spawn((
                    PracticeMapRow,
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(-offset),
                        width: if n <= 2 { percent(100) } else { Val::Auto },
                        justify_content: if n <= 2 {
                            JustifyContent::Center
                        } else {
                            JustifyContent::FlexStart
                        },
                        top: px(0),
                        height: percent(100),
                        column_gap: px(24.0 * self.factor),
                        padding: UiRect::all(px(8.0 * self.factor)),
                        ..default()
                    },
                ))
                .with_children(|row| {
                    for slot in 0..if n > 2 { n * 3 } else { n } {
                        self.card(row, slot, maps[slot % n]);
                    }
                });
        });
    }
    fn card(&self, row: &mut ChildSpawnerCommands, slot: usize, map: MapPresetId) {
        let preset = self
            .catalog
            .as_ref()
            .and_then(|c| c.presets.iter().find(|p| p.id == map));
        let name = preset.map_or_else(
            || format!("MAP {}", map.0),
            |p| p.display_name.to_uppercase(),
        );
        let id = control(
            row,
            self.server,
            PracticeControl::Map(slot),
            Node {
                width: px(self.card_width),
                height: percent(100),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                padding: UiRect {
                    left: px(55.0 * self.factor),
                    right: px(55.0 * self.factor),
                    top: px(35.0 * self.factor),
                    bottom: px(55.0 * self.factor),
                },
                row_gap: px(16.0 * self.factor),
                align_items: AlignItems::Center,
                ..default()
            },
            &name,
        );
        row.commands().entity(id).with_children(|card| {
            card.spawn(Node {
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|view| {
                if let (Some(s), Some(p)) = (self.server, preset) {
                    let ratio = f32::from(p.recipe.dimensions.width)
                        / f32::from(p.recipe.dimensions.height);
                    let width =
                        (self.card_width - 110.0 * self.factor).min(self.image_height * ratio);
                    view.spawn((
                        ImageNode::new(s.load(format!("brawler/ui/shell/{}-overview.png", p.key))),
                        Node {
                            width: px(width),
                            height: px(width / ratio),
                            flex_shrink: 0.0,
                            ..default()
                        },
                    ));
                }
            });
            label(card, self.assets, &name, 32.0 * self.factor);
        });
    }
    fn footer(&self, root: &mut ChildSpawnerCommands, n: usize) {
        root.spawn(Node {
            width: percent(100),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|footer| {
            footer
                .spawn(Node {
                    column_gap: px(16),
                    ..default()
                })
                .with_children(|arrows| {
                    if n > 1 {
                        for (kind, text) in [
                            (PracticeControl::Previous, "‹"),
                            (PracticeControl::Next, "›"),
                        ] {
                            let id = control(
                                arrows,
                                self.server,
                                kind,
                                Node {
                                    width: px(70.0 * self.factor),
                                    height: px(70.0 * self.factor),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                text,
                            );
                            arrows.commands().entity(id).with_children(|c| {
                                label(c, self.assets, text, 48.0 * self.factor);
                            });
                        }
                    }
                });
            if n > 0 {
                let id = control(
                    footer,
                    self.server,
                    PracticeControl::Start,
                    Node {
                        width: px(360.0 * self.factor),
                        height: px(80.0 * self.factor),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    "Start Practice",
                );
                footer.commands().entity(id).with_children(|c| {
                    label(c, self.assets, "START PRACTICE", 32.0 * self.factor);
                });
            } else {
                label(
                    footer,
                    self.assets,
                    "NO COMPATIBLE MAPS",
                    28.0 * self.factor,
                );
            }
        });
    }
}

fn label(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    text: &str,
    size: f32,
) {
    parent.spawn((
        Text::new(text),
        dashboard_font(assets, size),
        TextColor(INK),
    ));
}
fn control(
    parent: &mut ChildSpawnerCommands,
    server: Option<&AssetServer>,
    control: PracticeControl,
    node: Node,
    name: &str,
) -> Entity {
    let load = |key: &str| {
        server.map_or_else(Handle::default, |s| {
            s.load(format!("brawler/ui/shell/{key}.png"))
        })
    };
    let base = if matches!(control, PracticeControl::Map(_)) {
        "practice-card"
    } else {
        "panel-mode"
    };
    let selected = if matches!(control, PracticeControl::Map(_)) {
        "practice-card-selected"
    } else {
        "panel-play"
    };
    let chrome = PracticeChrome {
        normal: load(base),
        selected: load(selected),
        focus: load(&format!("{base}-focus")),
        selected_focus: load(&format!("{selected}-focus")),
    };
    parent
        .spawn((
            Button,
            AccessibleLabel::new(name),
            control,
            ImageNode {
                image_mode: NodeImageMode::Stretch,
                visual_box: VisualBox::BorderBox,
                ..ImageNode::new(chrome.normal.clone())
            },
            chrome,
            node,
        ))
        .id()
}

#[allow(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    reason = "one screen owns all input devices and suppresses them under overlays"
)]
pub(in crate::client::flow) fn practice_map_input(
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    practice: Res<ClientPracticeModel>,
    keyboard: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    mut wheel: MessageReader<MouseWheel>,
    buttons: Query<(&PracticeControl, &Interaction), Changed<Interaction>>,
    mut carousel: ResMut<PracticeCarousel>,
    mut actions: ResMut<PendingFlowActions>,
) {
    let delta: f32 = wheel
        .read()
        .map(|e| {
            let value = if e.x.abs() > e.y.abs() { e.x } else { e.y };
            value
                * if e.unit == MouseScrollUnit::Line {
                    40.0
                } else {
                    1.0
                }
        })
        .sum();
    if *flow.get() != ClientFlow::PracticeMapSelect
        || *overlay != ClientOverlay::None
        || practice.pending()
    {
        return;
    }
    let pad = |b| pads.iter().any(|p| p.just_pressed(b));
    if keyboard.just_pressed(KeyCode::Escape) || pad(GamepadButton::East) {
        queue_ui_action(&mut actions, FlowUiAction::CancelPracticeMaps);
        return;
    }
    let mut activation = None;
    for (control, interaction) in &buttons {
        if *interaction == Interaction::Pressed {
            activation = Some(*control);
        }
    }
    carousel.wheel += delta;
    let left = keyboard.just_pressed(KeyCode::ArrowLeft)
        || keyboard.just_pressed(KeyCode::KeyA)
        || pad(GamepadButton::DPadLeft)
        || carousel.wheel >= 40.0;
    let right = keyboard.just_pressed(KeyCode::ArrowRight)
        || keyboard.just_pressed(KeyCode::KeyD)
        || pad(GamepadButton::DPadRight)
        || carousel.wheel <= -40.0;
    if left || right {
        carousel.step(right);
        carousel.wheel = 0.0;
    }
    if keyboard.just_pressed(KeyCode::ArrowDown) || pad(GamepadButton::DPadDown) {
        carousel.start = true;
    }
    if keyboard.just_pressed(KeyCode::ArrowUp) || pad(GamepadButton::DPadUp) {
        carousel.start = false;
    }
    if keyboard.just_pressed(KeyCode::Enter)
        || keyboard.just_pressed(KeyCode::Space)
        || pad(GamepadButton::South)
    {
        activation = Some(if carousel.start {
            PracticeControl::Start
        } else {
            PracticeControl::Map(carousel.slot)
        });
    }
    match activation {
        Some(PracticeControl::Previous) => carousel.step(false),
        Some(PracticeControl::Next) => carousel.step(true),
        Some(PracticeControl::Start) => {
            carousel.start = true;
            queue_ui_action(&mut actions, FlowUiAction::StartPractice);
        }
        Some(PracticeControl::Map(slot)) if !carousel.maps.is_empty() => {
            carousel.slot = slot;
            carousel.start = false;
            queue_ui_action(
                &mut actions,
                FlowUiAction::ChoosePracticeMap(carousel.maps[slot % carousel.maps.len()]),
            );
        }
        _ => {}
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy screen presentation resources"
)]
pub(in crate::client::flow) fn animate_practice_maps(
    time: Res<Time>,
    settings: Option<Res<crate::client::ClientShellSettings>>,
    practice: Res<ClientPracticeModel>,
    overlay: Res<ClientOverlay>,
    mut carousel: ResMut<PracticeCarousel>,
    mut rows: Query<&mut Node, With<PracticeMapRow>>,
    mut buttons: Query<(&PracticeControl, &PracticeChrome, &mut ImageNode)>,
) {
    let target = carousel.target();
    if settings.is_some_and(|s| s.reduced_motion) {
        carousel.offset = target;
    }
    carousel.offset += (target - carousel.offset) * (time.delta_secs() * 16.0).min(1.0);
    if (carousel.offset - target).abs() < 0.5 {
        carousel.offset = target;
        let n = carousel.maps.len();
        if n > 2 && (carousel.slot < n || carousel.slot >= n * 2) {
            carousel.slot = n + carousel.slot % n;
            carousel.offset = carousel.target();
        }
    }
    for mut node in &mut rows {
        node.left = px(-carousel.offset);
    }
    for (control, chrome, mut image) in &mut buttons {
        let selected = match *control {
            PracticeControl::Map(slot) => carousel
                .maps
                .get(slot % carousel.maps.len().max(1))
                .is_some_and(|m| {
                    practice
                        .selected_map
                        .as_ref()
                        .is_some_and(|(_, chosen)| chosen == m)
                }),
            PracticeControl::Start => true,
            _ => false,
        };
        let focused = *overlay == ClientOverlay::None
            && !practice.pending()
            && match *control {
                PracticeControl::Map(slot) => !carousel.start && slot == carousel.slot,
                PracticeControl::Start => carousel.start,
                _ => false,
            };
        image.image = match (selected, focused) {
            (true, true) => &chrome.selected_focus,
            (true, false) => &chrome.selected,
            (false, true) => &chrome.focus,
            (false, false) => &chrome.normal,
        }
        .clone();
        image.color = if practice.pending() {
            Color::srgba(1.0, 1.0, 1.0, 0.5)
        } else {
            Color::WHITE
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn carousel_wraps_both_directions_without_changing_map_order() {
        for n in [0, 1, 2, 5] {
            let maps = (0..n)
                .map(|i| MapPresetId(u16::try_from(i).unwrap() + 1))
                .collect::<Vec<_>>();
            let mut carousel = PracticeCarousel {
                maps: maps.clone(),
                slot: if n > 2 { n } else { 0 },
                stride: 100.0,
                ..default()
            };
            carousel.step(false);
            assert_eq!(carousel.slot % n.max(1), n.saturating_sub(1));
            carousel.step(true);
            assert_eq!(carousel.slot % n.max(1), 0);
            for _ in 0..30 {
                carousel.step(true);
            }
            for _ in 0..30 {
                carousel.step(false);
            }
            assert_eq!(carousel.slot % n.max(1), 0);
            assert_eq!(carousel.maps, maps);
        }
    }
}
