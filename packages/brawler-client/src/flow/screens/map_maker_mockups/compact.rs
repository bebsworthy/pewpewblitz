//! Direction D, "Compact tool UI" (owner request, 2026-11-19): the shell palette and title font
//! with flat application controls. Square icon buttons, segmented tabs, compact list rows, a thin
//! top bar and a collapsible side panel replace the slanted, bevelled shell surfaces in the
//! editor, so the arena keeps most of a landscape phone. Every phone control stays 48 dp
//! (76 UI px at 914x411). These widgets are local to the design board on purpose: they are not a
//! shell-toolkit variant.

use super::{
    ASSETS, CHECKS_PASSED, CanvasInsets, ChromeRegion, DOC_NAME, FlowButton, ISSUES, Kit, MAPS,
    SampleMap, Scene, Status, canvas, column, fill_node, grow, row,
};
use crate::{
    MapDraftId,
    flow::actions::FlowUiAction,
    ui::{
        icons,
        surface::Surface,
        theme::{ACCENT, INK, PANEL, PANEL_RAISED, PLAY, PLAY_HIGHLIGHT, TEXT, font, type_stroke},
    },
};
use bevy::{prelude::*, text::FontSource, ui::InteractionDisabled};

pub(super) fn spawn(root: &mut ChildSpawnerCommands, kit: &Kit, scene: Scene) {
    match scene {
        Scene::Library => light_library(root, kit, false),
        Scene::Empty => light_library(root, kit, true),
        Scene::Create => {
            light_library(root, kit, false);
            root.spawn((
                Node {
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..fill_node()
                },
                BackgroundColor(INK.with_alpha(0.72)),
            ))
            .with_children(|scrim| create_dialog(scrim, kit));
        }
        Scene::ToolLibrary => tool_library(root, kit),
        Scene::Editor => editor(root, kit, Panel::Collapsed),
        Scene::Assets => editor(root, kit, Panel::Assets),
        Scene::Check => editor(root, kit, Panel::Check),
    }
}

// ---------------------------------------------------------------------------------------------
// Flat widgets
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tone {
    Plain,
    Selected,
    Primary,
    Disabled,
    Quiet,
    Danger,
}

const RADIUS: f32 = 6.0;
const DANGER: Color = Color::srgb(0.78, 0.22, 0.20);
/// The flat bar and panel fill: `ink` lifted slightly toward `panel`.
fn bar_fill() -> Color {
    INK.mix(&PANEL, 0.35).with_alpha(0.97)
}

/// Standard control height: 48 dp on the phone, a desktop toolbar size otherwise.
fn ctl(kit: &Kit) -> f32 {
    kit.pick(40.0, 76.0)
}

/// Body text in the platform sans (Helvetica Neue on macOS, Roboto on Android); the shell font
/// stays for titles.
fn sans(size: f32) -> TextFont {
    TextFont {
        font: FontSource::SansSerif,
        font_size: FontSize::Px(size),
        weight: FontWeight(600),
        ..default()
    }
}

fn label(parent: &mut ChildSpawnerCommands, copy: &str, size: f32, color: Color) {
    parent.spawn((Text::new(copy), sans(size), TextColor(color)));
}

fn title(parent: &mut ChildSpawnerCommands, kit: &Kit, copy: &str, size: f32) {
    parent.spawn((
        Text::new(copy),
        font(kit.assets, size),
        TextColor(TEXT),
        type_stroke(size * 0.6),
    ));
}

fn icon(parent: &mut ChildSpawnerCommands, kit: &Kit, name: &str, size: f32, alpha: f32) {
    parent.spawn((
        ImageNode {
            color: Color::WHITE.with_alpha(alpha),
            ..ImageNode::new(kit.server.load(icons::path(name)))
        },
        Node {
            width: px(size),
            height: px(size),
            flex_shrink: 0.0,
            ..default()
        },
    ));
}

fn tone_colors(tone: Tone) -> (Color, Color, f32) {
    match tone {
        Tone::Plain => (PANEL, PANEL_RAISED.with_alpha(0.7), 1.0),
        Tone::Selected => (ACCENT.with_alpha(0.24), ACCENT, 2.0),
        Tone::Primary => (PLAY, PLAY_HIGHLIGHT, 1.0),
        Tone::Disabled => (PANEL.with_alpha(0.45), PANEL_RAISED.with_alpha(0.3), 1.0),
        Tone::Quiet => (Color::NONE, Color::NONE, 0.0),
        Tone::Danger => (DANGER, DANGER.lighter(0.1), 1.0),
    }
}

/// A flat button: optional icon and short label, square when it has no label.
fn button(
    parent: &mut ChildSpawnerCommands,
    kit: &Kit,
    icon_name: Option<&str>,
    copy: Option<&str>,
    tone: Tone,
    width: Val,
) -> Entity {
    let (fill, border, border_width) = tone_colors(tone);
    let height = ctl(kit);
    let mut entity = parent.spawn((
        Button,
        AccessibleLabel::new(copy.or(icon_name).unwrap_or("button")),
        FlowButton {
            index: 0,
            action: FlowUiAction::MapMakerFitView,
            error_action: false,
        },
        Node {
            width,
            min_width: px(height),
            min_height: px(height),
            flex_shrink: 0.0,
            padding: UiRect::axes(px(kit.pick(10.0, 14.0)), px(0)),
            column_gap: px(kit.pick(6.0, 8.0)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: UiRect::all(px(border_width)),
            border_radius: BorderRadius::all(px(RADIUS)),
            ..default()
        },
        BackgroundColor(fill),
        BorderColor::all(border),
    ));
    if tone == Tone::Disabled {
        entity.insert(InteractionDisabled);
    }
    let alpha = if tone == Tone::Disabled { 0.45 } else { 1.0 };
    entity.with_children(|button| {
        if let Some(name) = icon_name {
            icon(button, kit, name, kit.pick(22.0, 36.0), alpha);
        }
        if let Some(copy) = copy {
            let color = if tone == Tone::Primary { INK } else { TEXT };
            label(button, copy, kit.pick(14.0, 20.0), color.with_alpha(alpha));
        }
    });
    entity.id()
}

/// A square icon button: `ctl` × `ctl`, icon only.
fn square(parent: &mut ChildSpawnerCommands, kit: &Kit, icon_name: &str, tone: Tone) -> Entity {
    button(parent, kit, Some(icon_name), None, tone, px(ctl(kit)))
}

/// Joined segmented control; `selected` is the current segment.
fn segmented(
    parent: &mut ChildSpawnerCommands,
    kit: &Kit,
    segments: &[(Option<&str>, Option<&str>)],
    selected: usize,
    stretch: bool,
) {
    parent
        .spawn((
            Node {
                flex_grow: if stretch { 1.0 } else { 0.0 },
                padding: UiRect::all(px(2)),
                column_gap: px(2),
                border_radius: BorderRadius::all(px(RADIUS + 2.0)),
                ..default()
            },
            BackgroundColor(INK.with_alpha(0.85)),
        ))
        .with_children(|group| {
            for (index, (icon_name, copy)) in segments.iter().enumerate() {
                let tone = if index == selected {
                    Tone::Selected
                } else {
                    Tone::Quiet
                };
                let width = if copy.is_some() {
                    Val::Auto
                } else {
                    px(ctl(kit))
                };
                let id = button(group, kit, *icon_name, *copy, tone, width);
                if stretch {
                    group.commands().entity(id).insert(Node {
                        flex_grow: 1.0,
                        flex_basis: px(0),
                        min_height: px(ctl(kit)),
                        column_gap: px(6),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border: UiRect::all(px(if index == selected { 2.0 } else { 0.0 })),
                        border_radius: BorderRadius::all(px(RADIUS)),
                        ..default()
                    });
                }
            }
        });
}

fn divider(parent: &mut ChildSpawnerCommands, kit: &Kit) {
    parent.spawn((
        Node {
            width: px(1),
            height: px(ctl(kit) * 0.6),
            margin: UiRect::horizontal(px(kit.pick(4.0, 6.0))),
            ..default()
        },
        BackgroundColor(TEXT.with_alpha(0.18)),
    ));
}

fn status(parent: &mut ChildSpawnerCommands, kit: &Kit, status: Status, size: f32) {
    let (name, copy, color) = match status {
        Status::Ready => ("check", "Ready to test".to_string(), ACCENT),
        Status::Issues(1) => ("warning", "1 issue".to_string(), PLAY_HIGHLIGHT),
        Status::Issues(n) => ("warning", format!("{n} issues"), PLAY_HIGHLIGHT),
        Status::Unsaved => ("edit", "Unsaved changes".to_string(), PLAY_HIGHLIGHT),
    };
    parent.spawn(row(6.0)).with_children(|line| {
        icon(line, kit, name, size + 2.0, 1.0);
        label(line, &copy, size, color);
    });
}

fn swatch(parent: &mut ChildSpawnerCommands, color: Color, footprint: &str, cell: f32) {
    parent
        .spawn(Node {
            width: px(cell * 2.0 + 3.0),
            justify_content: JustifyContent::Center,
            ..row(3.0)
        })
        .with_children(|swatch| {
            for _ in 0..if footprint == "2×1" { 2 } else { 1 } {
                swatch.spawn((
                    Node {
                        width: px(cell),
                        height: px(cell),
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(px(3)),
                        ..default()
                    },
                    BackgroundColor(color),
                    BorderColor::all(INK),
                ));
            }
        });
}

fn overview(parent: &mut ChildSpawnerCommands, kit: &Kit, map: &SampleMap, width: f32) {
    let node = Node {
        width: px(width),
        height: px(width / 1.6),
        flex_shrink: 0.0,
        border_radius: BorderRadius::all(px(4)),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    };
    match map.overview {
        Some(key) => {
            parent.spawn((
                ImageNode::new(
                    kit.server
                        .load(format!("brawler/ui/shell/{key}-overview.png")),
                ),
                node,
            ));
        }
        None => {
            parent
                .spawn((node, BackgroundColor(super::BLANK_ARENA)))
                .with_children(|blank| label(blank, "Empty", width * 0.09, INK.with_alpha(0.5)));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Library: lighter shell look, and the same library in the flat tool style
// ---------------------------------------------------------------------------------------------

fn light_library(root: &mut ChildSpawnerCommands, kit: &Kit, empty: bool) {
    root.spawn(Node {
        width: percent(100),
        height: percent(100),
        padding: UiRect::all(px(kit.pick(22.0, 20.0))),
        ..column(kit.pick(14.0, 12.0))
    })
    .with_children(|screen| {
        screen
            .spawn(Node {
                width: percent(100),
                ..row(20.0)
            })
            .with_children(|header| {
                kit.action(
                    header,
                    FlowUiAction::CloseMapMaker,
                    "BACK",
                    Surface::Back,
                    Val::Auto,
                );
                header.spawn(column(0.0)).with_children(|copy| {
                    title(copy, kit, "MY MAPS", kit.pick(44.0, 46.0));
                    label(
                        copy,
                        if empty {
                            "Map Maker · saved on this device"
                        } else {
                            "Map Maker · 4 maps saved on this device"
                        },
                        kit.pick(15.0, 19.0),
                        ACCENT,
                    );
                });
                header.spawn(grow());
                if !empty {
                    kit.action(
                        header,
                        FlowUiAction::CreateMap,
                        "+ CREATE MAP",
                        Surface::Primary,
                        px(kit.pick(250.0, 290.0)),
                    );
                }
            });
        screen
            .spawn((
                Node {
                    width: percent(100),
                    flex_grow: 1.0,
                    min_height: px(0),
                    padding: UiRect::all(px(kit.pick(18.0, 16.0))),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(14)),
                    ..column(kit.pick(14.0, 12.0))
                },
                BackgroundColor(INK.with_alpha(0.55)),
                BorderColor::all(ACCENT.with_alpha(0.25)),
            ))
            .with_children(|body| {
                if empty {
                    light_empty(body, kit);
                } else {
                    light_tiles(body, kit);
                    light_actions(body, kit);
                }
            });
    });
}

fn light_tiles(body: &mut ChildSpawnerCommands, kit: &Kit) {
    let tile = kit.pick(274.0, 318.0);
    body.spawn(Node {
        width: percent(100),
        flex_grow: 1.0,
        min_height: px(0),
        align_items: AlignItems::Start,
        overflow: Overflow::clip(),
        ..row(kit.pick(14.0, 14.0))
    })
    .with_children(|tiles| {
        for (index, map) in MAPS.iter().enumerate() {
            let selected = index == 1;
            tiles
                .spawn((
                    Button,
                    FlowButton {
                        index,
                        action: FlowUiAction::OpenMapDraft(MapDraftId(index as u64 + 1)),
                        error_action: false,
                    },
                    Node {
                        width: px(tile),
                        flex_shrink: 0.0,
                        padding: UiRect::all(px(10)),
                        border: UiRect::all(px(if selected { 3.0 } else { 1.0 })),
                        border_radius: BorderRadius::all(px(10)),
                        ..column(6.0)
                    },
                    BackgroundColor(if selected {
                        PANEL_RAISED.with_alpha(0.55)
                    } else {
                        PANEL.with_alpha(0.9)
                    }),
                    BorderColor::all(if selected {
                        ACCENT
                    } else {
                        PANEL_RAISED.with_alpha(0.6)
                    }),
                ))
                .with_children(|card| {
                    overview(card, kit, map, tile - 26.0);
                    title(card, kit, map.name, kit.pick(22.0, 25.0));
                    card.spawn(row(6.0)).with_children(|line| {
                        icon(line, kit, map.pennant, kit.pick(20.0, 24.0), 1.0);
                        label(
                            line,
                            &format!("{}  ·  48 × 32", title_case(map.mode)),
                            kit.pick(14.0, 17.0),
                            TEXT.with_alpha(0.85),
                        );
                    });
                    status(card, kit, map.status, kit.pick(14.0, 17.0));
                });
        }
    });
}

fn title_case(mode: &str) -> String {
    match mode {
        "HOT ZONE" => "Hot Zone".into(),
        "HEIST" => "Heist".into(),
        _ => "Wipeout".into(),
    }
}

fn light_actions(body: &mut ChildSpawnerCommands, kit: &Kit) {
    body.spawn(Node {
        width: percent(100),
        ..row(kit.pick(10.0, 12.0))
    })
    .with_children(|bar| {
        bar.spawn(column(0.0)).with_children(|copy| {
            label(copy, "SELECTED", kit.pick(12.0, 15.0), ACCENT);
            title(copy, kit, MAPS[1].name, kit.pick(22.0, 24.0));
        });
        bar.spawn(grow());
        for (name, copy, tone) in [
            ("edit", "Rename", Tone::Plain),
            ("duplicate", "Duplicate", Tone::Plain),
            ("trash", "Delete", Tone::Danger),
        ] {
            button(bar, kit, Some(name), Some(copy), tone, Val::Auto);
        }
        kit.action(
            bar,
            FlowUiAction::OpenMapDraft(MapDraftId(2)),
            "EDIT MAP",
            Surface::Primary,
            px(kit.pick(220.0, 250.0)),
        );
    });
}

fn light_empty(body: &mut ChildSpawnerCommands, kit: &Kit) {
    body.spawn(Node {
        width: percent(100),
        flex_grow: 1.0,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..column(kit.pick(14.0, 12.0))
    })
    .with_children(|empty| {
        icon(empty, kit, "arena", kit.pick(80.0, 88.0), 1.0);
        title(empty, kit, "NO MAPS YET", kit.pick(38.0, 42.0));
        label(
            empty,
            "Build an arena from walls, cover and grass, then test it in Practice.",
            kit.pick(17.0, 21.0),
            TEXT,
        );
        empty.spawn(row(10.0)).with_children(|steps| {
            for (step, copy) in [
                ("1", "Create a map"),
                ("2", "Place spawns and cover"),
                ("3", "Test in Practice"),
            ] {
                steps
                    .spawn((
                        Node {
                            padding: UiRect::axes(px(14), px(8)),
                            border_radius: BorderRadius::all(px(RADIUS)),
                            ..row(8.0)
                        },
                        BackgroundColor(PANEL.with_alpha(0.9)),
                    ))
                    .with_children(|chip| {
                        title(chip, kit, step, kit.pick(22.0, 24.0));
                        label(chip, copy, kit.pick(15.0, 18.0), TEXT);
                    });
            }
        });
        kit.action(
            empty,
            FlowUiAction::CreateMap,
            "+ CREATE YOUR FIRST MAP",
            Surface::Primary,
            px(kit.pick(400.0, 460.0)),
        );
    });
}

fn create_dialog(scrim: &mut ChildSpawnerCommands, kit: &Kit) {
    scrim
        .spawn((
            Node {
                width: percent(if kit.phone { 58 } else { 46 }),
                padding: UiRect::all(px(kit.pick(24.0, 26.0))),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(12)),
                ..column(kit.pick(12.0, 12.0))
            },
            BackgroundColor(bar_fill()),
            BorderColor::all(ACCENT.with_alpha(0.4)),
        ))
        .with_children(|dialog| {
            title(dialog, kit, "NEW MAP", kit.pick(32.0, 36.0));
            label(dialog, "Name", kit.pick(14.0, 18.0), ACCENT);
            dialog
                .spawn((
                    Node {
                        width: percent(100),
                        min_height: px(ctl(kit)),
                        padding: UiRect::horizontal(px(14)),
                        align_items: AlignItems::Center,
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(px(RADIUS)),
                        ..default()
                    },
                    BackgroundColor(INK),
                    BorderColor::all(ACCENT),
                ))
                .with_children(|field| {
                    label(field, "Untitled Map 5", kit.pick(18.0, 23.0), TEXT);
                    field.spawn((
                        Node {
                            width: px(2),
                            height: px(kit.pick(22.0, 30.0)),
                            margin: UiRect::left(px(3)),
                            ..default()
                        },
                        BackgroundColor(ACCENT),
                    ));
                });
            dialog.spawn(row(24.0)).with_children(|facts| {
                for (key, value) in [
                    ("Mode", "Wipeout"),
                    ("Size", "48 × 32"),
                    ("Theme", "Feature Yard"),
                ] {
                    facts.spawn(row(8.0)).with_children(|fact| {
                        label(fact, key, kit.pick(14.0, 17.0), ACCENT);
                        label(fact, value, kit.pick(16.0, 20.0), TEXT);
                    });
                }
            });
            label(
                dialog,
                "Mode and size follow your selected game.",
                kit.pick(13.0, 16.0),
                TEXT.with_alpha(0.75),
            );
            dialog
                .spawn(Node {
                    justify_content: JustifyContent::FlexEnd,
                    ..row(10.0)
                })
                .with_children(|actions| {
                    button(
                        actions,
                        kit,
                        None,
                        Some("Cancel"),
                        Tone::Plain,
                        px(kit.pick(110.0, 150.0)),
                    );
                    button(
                        actions,
                        kit,
                        None,
                        Some("Create map"),
                        Tone::Primary,
                        px(kit.pick(150.0, 210.0)),
                    );
                });
        });
}

fn tool_library(root: &mut ChildSpawnerCommands, kit: &Kit) {
    root.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            ..fill_node()
        },
        BackgroundColor(INK.mix(&PANEL, 0.18)),
    ))
    .with_children(|screen| {
        screen
            .spawn((
                Node {
                    width: percent(100),
                    padding: UiRect::axes(px(kit.pick(10.0, 8.0)), px(4)),
                    border: UiRect::bottom(px(1)),
                    ..row(kit.pick(10.0, 12.0))
                },
                BackgroundColor(bar_fill()),
                BorderColor::all(ACCENT.with_alpha(0.25)),
            ))
            .with_children(|bar| {
                square(bar, kit, "chevron-left", Tone::Plain);
                title(bar, kit, "MY MAPS", kit.pick(26.0, 32.0));
                label(bar, "4 maps on this device", kit.pick(14.0, 18.0), ACCENT);
                bar.spawn(grow());
                button(bar, kit, Some("search"), None, Tone::Plain, px(ctl(kit)));
                button(bar, kit, None, Some("+ New map"), Tone::Primary, Val::Auto);
            });
        screen
            .spawn(Node {
                width: percent(100),
                flex_grow: 1.0,
                padding: UiRect::all(px(kit.pick(14.0, 12.0))),
                ..column(kit.pick(4.0, 6.0))
            })
            .with_children(|table| {
                table_header(table, kit);
                for (index, map) in MAPS.iter().enumerate() {
                    table_row(table, kit, map, index == 1);
                }
            });
    });
}

/// Column widths for the tool library table: thumbnail, name, mode, size, status, updated.
fn columns(kit: &Kit) -> [f32; 6] {
    if kit.phone {
        [110.0, 280.0, 170.0, 110.0, 210.0, 150.0]
    } else {
        [72.0, 250.0, 140.0, 90.0, 170.0, 150.0]
    }
}

fn table_header(table: &mut ChildSpawnerCommands, kit: &Kit) {
    table
        .spawn(Node {
            width: percent(100),
            padding: UiRect::horizontal(px(12)),
            ..row(kit.pick(12.0, 14.0))
        })
        .with_children(|header| {
            for (width, copy) in columns(kit)
                .into_iter()
                .zip(["", "MAP", "MODE", "SIZE", "STATUS", "SAVED"])
            {
                header
                    .spawn(Node {
                        width: px(width),
                        ..default()
                    })
                    .with_children(|cell| label(cell, copy, kit.pick(12.0, 15.0), ACCENT));
            }
        });
}

fn table_row(table: &mut ChildSpawnerCommands, kit: &Kit, map: &SampleMap, selected: bool) {
    let [thumb, name, mode, size, state, saved] = columns(kit);
    table
        .spawn((
            Button,
            FlowButton {
                index: 0,
                action: FlowUiAction::OpenMapDraft(MapDraftId(2)),
                error_action: false,
            },
            Node {
                width: percent(100),
                min_height: px(kit.pick(56.0, 90.0)),
                padding: UiRect::axes(px(12), px(4)),
                border: UiRect::all(px(if selected { 2.0 } else { 1.0 })),
                border_radius: BorderRadius::all(px(RADIUS)),
                ..row(kit.pick(12.0, 14.0))
            },
            BackgroundColor(if selected {
                ACCENT.with_alpha(0.18)
            } else {
                PANEL.with_alpha(0.75)
            }),
            BorderColor::all(if selected {
                ACCENT
            } else {
                PANEL_RAISED.with_alpha(0.4)
            }),
        ))
        .with_children(|line| {
            overview(line, kit, map, thumb);
            line.spawn(Node {
                width: px(name),
                ..default()
            })
            .with_children(|cell| title(cell, kit, map.name, kit.pick(18.0, 23.0)));
            line.spawn(Node {
                width: px(mode),
                ..row(6.0)
            })
            .with_children(|cell| {
                icon(cell, kit, map.pennant, kit.pick(20.0, 26.0), 1.0);
                label(cell, &title_case(map.mode), kit.pick(14.0, 18.0), TEXT);
            });
            line.spawn(Node {
                width: px(size),
                ..default()
            })
            .with_children(|cell| label(cell, "48 × 32", kit.pick(14.0, 18.0), TEXT));
            line.spawn(Node {
                width: px(state),
                ..default()
            })
            .with_children(|cell| status(cell, kit, map.status, kit.pick(13.0, 17.0)));
            line.spawn(Node {
                width: px(saved),
                ..default()
            })
            .with_children(|cell| {
                label(
                    cell,
                    "Today 14:02",
                    kit.pick(13.0, 17.0),
                    TEXT.with_alpha(0.7),
                );
            });
            line.spawn(grow());
            for (name, tone) in [
                ("edit", Tone::Quiet),
                ("duplicate", Tone::Quiet),
                ("trash", Tone::Quiet),
            ] {
                square(line, kit, name, tone);
            }
        });
}

// ---------------------------------------------------------------------------------------------
// Editor
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Panel {
    Collapsed,
    Assets,
    Check,
}

fn bar_height(kit: &Kit) -> f32 {
    ctl(kit) + 8.0
}

fn panel_width(kit: &Kit, panel: Panel) -> f32 {
    match panel {
        Panel::Collapsed => ctl(kit) + 8.0,
        Panel::Assets | Panel::Check => kit.pick(330.0, 400.0),
    }
}

fn editor(root: &mut ChildSpawnerCommands, kit: &Kit, panel: Panel) {
    let screen_width = kit.pick(1280.0, 1423.0);
    canvas(
        root,
        kit,
        CanvasInsets {
            right_percent: 100.0 * panel_width(kit, panel) / screen_width,
            top: bar_height(kit),
        },
    );
    top_bar(root, kit);
    side_panel(root, kit, panel);
    if panel == Panel::Collapsed {
        // What the next tap places, so the collapsed panel never hides the current asset.
        root.spawn(Node {
            position_type: PositionType::Absolute,
            left: px(8),
            bottom: px(8),
            ..default()
        })
        .insert(ChromeRegion)
        .with_children(|anchor| {
            anchor
                .spawn((
                    Button,
                    FlowButton {
                        index: 0,
                        action: FlowUiAction::MapMakerToggleAssets,
                        error_action: false,
                    },
                    Node {
                        min_height: px(ctl(kit)),
                        padding: UiRect::axes(px(12), px(4)),
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(RADIUS)),
                        ..row(10.0)
                    },
                    BackgroundColor(bar_fill()),
                    BorderColor::all(ACCENT.with_alpha(0.35)),
                ))
                .with_children(|chip| {
                    let (name, footprint, color) = ASSETS[0];
                    swatch(chip, color, footprint, kit.pick(18.0, 28.0));
                    chip.spawn(column(0.0)).with_children(|copy| {
                        label(copy, name, kit.pick(14.0, 19.0), TEXT);
                        label(copy, "Cover · R rotates", kit.pick(11.0, 14.0), ACCENT);
                    });
                });
        });
    }
}

fn top_bar(root: &mut ChildSpawnerCommands, kit: &Kit) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            height: px(bar_height(kit)),
            padding: UiRect::axes(px(kit.pick(8.0, 6.0)), px(4)),
            border: UiRect::bottom(px(1)),
            ..row(kit.pick(6.0, 6.0))
        },
        BackgroundColor(bar_fill()),
        BorderColor::all(ACCENT.with_alpha(0.25)),
        ChromeRegion,
    ))
    .with_children(|bar| {
        square(bar, kit, "chevron-left", Tone::Plain);
        bar.spawn(Node {
            width: px(kit.pick(210.0, 230.0)),
            ..column(0.0)
        })
        .with_children(|identity| {
            title(
                identity,
                kit,
                &DOC_NAME.to_uppercase(),
                kit.pick(17.0, 21.0),
            );
            identity.spawn(row(5.0)).with_children(|line| {
                icon(line, kit, "mode-hot-zone", kit.pick(14.0, 18.0), 1.0);
                label(
                    line,
                    "Hot Zone · 48 × 32 · unsaved",
                    kit.pick(11.0, 14.0),
                    PLAY_HIGHLIGHT,
                );
            });
        });
        divider(bar, kit);
        let tools: [(Option<&str>, Option<&str>); 3] = if kit.phone {
            [
                (Some("place"), None),
                (Some("select"), None),
                (Some("erase"), None),
            ]
        } else {
            [
                (Some("place"), Some("Place")),
                (Some("select"), Some("Select")),
                (Some("erase"), Some("Erase")),
            ]
        };
        segmented(bar, kit, &tools, 0, false);
        divider(bar, kit);
        button(bar, kit, None, Some("Undo"), Tone::Plain, Val::Auto);
        button(bar, kit, None, Some("Redo"), Tone::Disabled, Val::Auto);
        divider(bar, kit);
        square(bar, kit, "grid", Tone::Selected);
        square(bar, kit, "symmetry", Tone::Plain);
        square(bar, kit, "fit", Tone::Plain);
        bar.spawn(grow());
        button(bar, kit, None, Some("Save"), Tone::Plain, Val::Auto);
        button(
            bar,
            kit,
            Some("play"),
            Some("Test"),
            Tone::Disabled,
            Val::Auto,
        );
    });
}

fn side_panel(root: &mut ChildSpawnerCommands, kit: &Kit, panel: Panel) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: px(0),
            top: px(bar_height(kit)),
            bottom: px(0),
            width: px(panel_width(kit, panel)),
            padding: UiRect::all(px(4)),
            border: UiRect::left(px(1)),
            ..column(kit.pick(8.0, 6.0))
        },
        BackgroundColor(bar_fill()),
        BorderColor::all(ACCENT.with_alpha(0.25)),
        ChromeRegion,
    ))
    .with_children(|side| match panel {
        Panel::Collapsed => {
            rail_tab(side, kit, "menu", "Assets", None);
            rail_tab(side, kit, "warning", "Check", Some("2"));
        }
        Panel::Assets | Panel::Check => {
            side.spawn(Node {
                width: percent(100),
                ..row(6.0)
            })
            .with_children(|head| {
                segmented(
                    head,
                    kit,
                    &[(None, Some("Assets")), (Some("warning"), Some("Check 2"))],
                    usize::from(panel == Panel::Check),
                    true,
                );
                square(head, kit, "chevron-right", Tone::Plain);
            });
            if panel == Panel::Assets {
                assets_tab(side, kit);
            } else {
                check_tab(side, kit);
            }
        }
    });
}

/// A collapsed-panel tab: icon over a short label, with an optional count badge.
fn rail_tab(
    side: &mut ChildSpawnerCommands,
    kit: &Kit,
    name: &str,
    copy: &str,
    badge: Option<&str>,
) {
    side.spawn((
        Button,
        FlowButton {
            index: 0,
            action: FlowUiAction::MapMakerToggleAssets,
            error_action: false,
        },
        Node {
            width: px(ctl(kit)),
            min_height: px(ctl(kit)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(RADIUS)),
            ..column(2.0)
        },
        BackgroundColor(PANEL),
        BorderColor::all(PANEL_RAISED.with_alpha(0.7)),
    ))
    .with_children(|tab| {
        icon(tab, kit, name, kit.pick(20.0, 30.0), 1.0);
        label(tab, copy, kit.pick(10.0, 13.0), TEXT);
        if let Some(badge) = badge {
            tab.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: px(-4),
                    right: px(-4),
                    min_width: px(kit.pick(16.0, 24.0)),
                    height: px(kit.pick(16.0, 24.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(px(12)),
                    ..default()
                },
                BackgroundColor(PLAY),
            ))
            .with_children(|dot| label(dot, badge, kit.pick(10.0, 14.0), INK));
        }
    });
}

fn assets_tab(side: &mut ChildSpawnerCommands, kit: &Kit) {
    side.spawn(Node {
        width: percent(100),
        ..row(6.0)
    })
    .with_children(|filter| {
        // A category picker and search; the desktop also gets the category chips below.
        let picker = button(
            filter,
            kit,
            Some("chevron-down"),
            Some("Cover"),
            Tone::Plain,
            Val::Auto,
        );
        filter.commands().entity(picker).insert(Node {
            flex_grow: 1.0,
            min_height: px(ctl(kit)),
            padding: UiRect::horizontal(px(12)),
            column_gap: px(8),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(RADIUS)),
            flex_direction: FlexDirection::RowReverse,
            ..default()
        });
        square(filter, kit, "search", Tone::Plain);
    });
    side.spawn(Node {
        width: percent(100),
        flex_grow: 1.0,
        min_height: px(0),
        overflow: Overflow::scroll_y(),
        ..column(kit.pick(3.0, 4.0))
    })
    .with_children(|list| {
        let count = if kit.phone { 4 } else { 8 };
        for (index, (name, footprint, color)) in ASSETS.iter().take(count).enumerate() {
            let selected = index == 0;
            list.spawn((
                Button,
                FlowButton {
                    index,
                    action: FlowUiAction::MapMakerSelectAsset(index),
                    error_action: false,
                },
                Node {
                    width: percent(100),
                    min_height: px(ctl(kit)),
                    flex_shrink: 0.0,
                    padding: UiRect::horizontal(px(10)),
                    border: UiRect::all(px(if selected { 2.0 } else { 0.0 })),
                    border_radius: BorderRadius::all(px(RADIUS)),
                    ..row(10.0)
                },
                BackgroundColor(if selected {
                    ACCENT.with_alpha(0.22)
                } else {
                    PANEL.with_alpha(0.6)
                }),
                BorderColor::all(ACCENT),
            ))
            .with_children(|item| {
                swatch(item, *color, footprint, kit.pick(18.0, 30.0));
                label(item, name, kit.pick(14.0, 19.0), TEXT);
                item.spawn(grow());
                label(item, footprint, kit.pick(12.0, 16.0), ACCENT);
            });
        }
    });
    if kit.phone {
        return;
    }
    label(
        side,
        "Arena Wall blocks movement and shots",
        kit.pick(11.0, 14.0),
        TEXT.with_alpha(0.75),
    );
}

fn check_tab(side: &mut ChildSpawnerCommands, kit: &Kit) {
    label(
        side,
        "2 problems block Test",
        kit.pick(14.0, 18.0),
        PLAY_HIGHLIGHT,
    );
    for (headline, fix) in ISSUES {
        side.spawn((
            Node {
                width: percent(100),
                padding: UiRect::axes(px(8), px(4)),
                border_radius: BorderRadius::all(px(RADIUS)),
                ..row(8.0)
            },
            BackgroundColor(PANEL.with_alpha(0.7)),
        ))
        .with_children(|issue| {
            icon(issue, kit, "warning", kit.pick(18.0, 26.0), 1.0);
            issue
                .spawn(Node {
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    min_width: px(0),
                    ..column(0.0)
                })
                .with_children(|copy| {
                    label(copy, headline, kit.pick(13.0, 17.0), TEXT);
                    label(copy, fix, kit.pick(11.0, 14.0), ACCENT);
                });
            button(issue, kit, None, Some("Show"), Tone::Plain, Val::Auto);
        });
    }
    label(side, "Passed", kit.pick(12.0, 15.0), ACCENT);
    for passed in CHECKS_PASSED {
        side.spawn(Node {
            padding: UiRect::horizontal(px(8)),
            ..row(8.0)
        })
        .with_children(|line| {
            icon(line, kit, "check", kit.pick(16.0, 22.0), 1.0);
            label(line, passed, kit.pick(12.0, 16.0), TEXT.with_alpha(0.85));
        });
    }
}
