//! The Map Maker editor in the compact tool style (BRL-2xpc direction D): a thin top bar, a
//! collapsible Assets/Check side panel, a current-asset chip and a status toast around the live
//! 3D arena. Structural edits rebuild this view; value-only edits update retained labels and the
//! flat controls' live state in place.

use super::{
    EditorTool, MapMakerRoot, MapMakerRuntime, MapPalettePreviewHost, PaletteCategory, PaletteItem,
    SidePanel,
    check::{MapCheck, Requirement, RuleIssue, map_check},
    library::{mode_identity, pennant_icon},
    palette, palette_category, palette_color, palette_label,
    scroll::{MapMakerScroll, MapMakerScrollMemory, ScrollArea},
    selected_palette_item, symmetry_label, visible_palette_entries,
    widgets::{ButtonSpec, Kit, MetricsParam, RADIUS, Tone, bar_fill, column, grow, row},
};
use crate::{
    ArenaCamera, MapEditorSession,
    flow::{ClientFlow, actions::FlowUiAction, screens::shared::FlowRoot},
    ui::theme::{ACCENT, INK, PANEL, PLAY, PLAY_HIGHLIGHT, TEXT},
};
use bevy::{prelude::*, ui::FocusPolicy};
use brawler::map::{MapAssetSlot, MapCatalogResource, MapCell, MapContentCatalog, MapSymmetry};
use std::collections::BTreeSet;

/// Editor control indices (kept clear of the library's focus order).
pub(super) mod index {
    pub(in super::super) const BACK: usize = 1000;
    pub(in super::super) const PLACE: usize = 1001;
    pub(in super::super) const SELECT: usize = 1002;
    pub(in super::super) const ERASE: usize = 1003;
    pub(in super::super) const UNDO: usize = 1004;
    pub(in super::super) const REDO: usize = 1005;
    pub(in super::super) const GRID: usize = 1006;
    pub(in super::super) const MIRROR: usize = 1007;
    pub(in super::super) const FIT: usize = 1008;
    pub(in super::super) const SAVE: usize = 1009;
    pub(in super::super) const TEST: usize = 1010;
    pub(in super::super) const CHIP: usize = 1011;
    pub(in super::super) const ASSETS_TAB: usize = 1020;
    pub(in super::super) const CHECK_TAB: usize = 1021;
    pub(in super::super) const COLLAPSE: usize = 1022;
    pub(in super::super) const CATEGORY: usize = 1023;
    pub(in super::super) const SEARCH: usize = 1024;
    pub(in super::super) const CLEAR_SEARCH: usize = 1025;
    pub(in super::super) const FIRST_CATEGORY: usize = 1030;
    pub(in super::super) const ROTATE: usize = 1040;
    pub(in super::super) const DUPLICATE: usize = 1041;
    pub(in super::super) const DELETE: usize = 1042;
    pub(in super::super) const DESELECT: usize = 1043;
    pub(in super::super) const SHOW_RULE: usize = 1045;
    pub(in super::super) const FOCUS_MODE: usize = 1046;
    pub(in super::super) const FIRST_FIX: usize = 1050;
    pub(in super::super) const FIRST_ASSET: usize = 1100;
}

/// The editor's top bar; its laid-out height is the arena's top inset.
#[derive(Component)]
pub(in crate::flow) struct EditorTopBar;

/// The Assets/Check side panel, open or collapsed; its width is the arena's right inset.
#[derive(Component)]
pub(in crate::flow) struct EditorSidePanel;

/// The corner chip naming the asset the next tap places.
#[derive(Component)]
pub(in crate::flow) struct CurrentAssetChip;

/// Editor chrome over the arena: a pointer over it never paints or selects on the map.
#[derive(Component)]
#[require(Interaction, FocusPolicy::Block)]
pub(in crate::flow) struct MapMakerChrome;

/// Retained editor labels whose text changes on value-only edits (BRL-0175).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::flow) enum MapMakerValueLabel {
    /// The mirror mode after the document facts, empty while mirroring is off.
    Symmetry,
    Search,
    Status,
    /// The current asset's name on the chip.
    Asset,
    /// The current asset's category and placement hint on the chip.
    AssetHint,
}

/// The always-spawned status toast; value-only edits show, hide, or retext it in place.
#[derive(Component)]
pub(in crate::flow) struct MapMakerStatusLine;

/// The selection toolbar follows the editor camera without rebuilding the editor UI.
#[derive(Component)]
pub(in crate::flow) struct MapMakerSelectionToolbar;

/// The chip's live asset preview frame; its preview host follows the palette selection.
#[derive(Component)]
pub(in crate::flow) struct ChipPreviewFrame;

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "the editor view consumes independent Bevy-owned resources and queries"
)]
pub(in crate::flow) fn present_map_editor(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    runtime: Res<MapMakerRuntime>,
    catalog: Option<Res<MapCatalogResource>>,
    roots: Query<(Entity, &MapMakerRoot)>,
    assets: Option<Res<crate::ClientAssetHandles>>,
    server: Res<AssetServer>,
    cameras: Query<(&Camera, &GlobalTransform), With<ArenaCamera>>,
    metrics: MetricsParam,
    scroll: Option<Res<MapMakerScrollMemory>>,
) {
    if *flow.get() != ClientFlow::MapEditor {
        return;
    }
    let metrics = metrics.get();
    let key = MapMakerRoot {
        revision: runtime.render_revision,
        metrics,
    };
    if roots.iter().any(|(_, root)| *root == key) {
        return;
    }
    for (entity, _) in &roots {
        commands.entity(entity).try_despawn();
    }
    let (Some(session), Some(catalog)) = (runtime.session.as_ref(), catalog.as_deref()) else {
        return;
    };
    let catalog = &catalog.0;
    let kit = Kit {
        metrics,
        server: &server,
        assets: assets.as_deref(),
    };
    let check = map_check(&session.document.recipe, catalog);
    let toolbar =
        selection_toolbar_position(session, catalog, &runtime.selection, cameras.iter().next());
    let default_scroll = MapMakerScrollMemory::default();
    let scroll = scroll.as_deref().unwrap_or(&default_scroll);
    commands
        .spawn((
            FlowRoot,
            key,
            DespawnOnExit(ClientFlow::MapEditor),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            GlobalZIndex(420),
        ))
        .with_children(|root| {
            top_bar(root, &kit, session, &check);
            side_panel(root, &kit, &runtime, session, catalog, &check, scroll);
            if runtime.panel != SidePanel::Assets {
                corner_chip(root, &kit, &runtime, session, catalog);
            }
            status_toast(root, &kit, &runtime);
            super::focus::focus_badge(root, &kit, &runtime, panel_width(&kit, runtime.panel));
            selection_toolbar(root, &kit, &runtime, toolbar);
        });
}

fn top_bar(
    root: &mut ChildSpawnerCommands,
    kit: &Kit,
    session: &MapEditorSession,
    check: &MapCheck,
) {
    root.spawn((
        EditorTopBar,
        MapMakerChrome,
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            min_height: px(kit.metrics.bar_height()),
            padding: UiRect::axes(px(kit.pick(8.0, 6.0)), px(4)),
            border: UiRect::bottom(px(1)),
            ..row(6.0)
        },
        BackgroundColor(bar_fill()),
        BorderColor::all(ACCENT.with_alpha(0.25)),
    ))
    .with_children(|bar| {
        kit.button(
            bar,
            ButtonSpec::new(index::BACK, FlowUiAction::BackToMapLibrary, Tone::Plain)
                .icon("chevron-left")
                .named("Back to My Maps"),
        );
        document_identity(bar, kit, session);
        kit.divider(bar);
        tool_segments(bar, kit);
        kit.divider(bar);
        history_and_view(bar, kit, session);
        bar.spawn(grow());
        kit.button(
            bar,
            ButtonSpec::new(index::SAVE, FlowUiAction::SaveMapDraft, Tone::Plain).label("Save"),
        );
        kit.button(
            bar,
            ButtonSpec::new(
                index::TEST,
                FlowUiAction::TestAuthoredMap,
                if check.ready {
                    Tone::Primary
                } else {
                    Tone::Blocked
                },
            )
            .icon("play")
            .label("Test")
            .named(if check.ready {
                "Test in Practice"
            } else {
                "Test in Practice (the map check has issues)"
            }),
        );
    });
}

/// The map's name over its mode, size, saved state and mirror mode.
fn document_identity(bar: &mut ChildSpawnerCommands, kit: &Kit, session: &MapEditorSession) {
    let recipe = &session.document.recipe;
    bar.spawn(Node {
        width: px(kit.pick(220.0, 240.0)),
        flex_shrink: 1.0,
        min_width: px(0),
        overflow: Overflow::clip_x(),
        ..column(0.0)
    })
    .with_children(|identity| {
        identity.spawn((
            Text::new(session.document.name.to_uppercase()),
            crate::ui::theme::font(kit.assets, kit.pick(17.0, 21.0)),
            TextColor(TEXT),
            crate::ui::theme::type_stroke(kit.pick(10.0, 12.0)),
            super::widgets::no_wrap(),
        ));
        identity.spawn(row(5.0)).with_children(|line| {
            let (mode, pennant) = mode_identity(recipe.mode_definition_id);
            pennant_icon(line, kit, pennant, kit.pick(16.0, 20.0));
            let saved = if session.dirty() { "unsaved" } else { "saved" };
            line.spawn((
                Text::new(format!(
                    "{mode} · {} × {} · {saved}",
                    recipe.dimensions.width, recipe.dimensions.height
                )),
                kit.sans(kit.pick(11.0, 14.0)),
                TextColor(if session.dirty() {
                    PLAY_HIGHLIGHT
                } else {
                    ACCENT
                }),
                super::widgets::no_wrap(),
            ))
            .with_child((
                TextSpan::new(symmetry_suffix(session.symmetry)),
                MapMakerValueLabel::Symmetry,
                kit.sans(kit.pick(11.0, 14.0)),
                TextColor(ACCENT),
            ));
        });
    });
}

/// Place, Select and Erase as one segmented control (icons only on the phone).
fn tool_segments(bar: &mut ChildSpawnerCommands, kit: &Kit) {
    let tools = [
        (
            index::PLACE,
            FlowUiAction::MapMakerUsePlaceTool,
            "place",
            "Place",
        ),
        (
            index::SELECT,
            FlowUiAction::MapMakerUseMoveTool,
            "select",
            "Select",
        ),
        (
            index::ERASE,
            FlowUiAction::MapMakerUseEraseTool,
            "erase",
            "Erase",
        ),
    ];
    let segments = tools
        .into_iter()
        .map(|(index, action, icon, copy)| {
            let spec = ButtonSpec::new(index, action, Tone::Quiet).icon(icon);
            if kit.metrics.compact {
                spec.named(copy)
            } else {
                spec.label(copy)
            }
        })
        .collect();
    kit.segmented(bar, false, segments);
}

/// Undo and Redo, then the Grid, Mirror and Fit view controls.
fn history_and_view(bar: &mut ChildSpawnerCommands, kit: &Kit, session: &MapEditorSession) {
    kit.button(
        bar,
        ButtonSpec::new(index::UNDO, FlowUiAction::MapMakerUndo, Tone::Plain)
            .label("Undo")
            .disabled(!session.can_undo()),
    );
    kit.button(
        bar,
        ButtonSpec::new(index::REDO, FlowUiAction::MapMakerRedo, Tone::Plain)
            .label("Redo")
            .disabled(!session.can_redo()),
    );
    kit.divider(bar);
    for (index, action, icon, name) in [
        (
            index::GRID,
            FlowUiAction::MapMakerToggleGrid,
            "grid",
            "Grid",
        ),
        (
            index::MIRROR,
            FlowUiAction::MapMakerCycleSymmetry,
            "symmetry",
            "Mirror",
        ),
        (index::FIT, FlowUiAction::MapMakerFitView, "fit", "Fit view"),
    ] {
        kit.button(
            bar,
            ButtonSpec::new(index, action, Tone::Plain)
                .icon(icon)
                .named(name),
        );
    }
}

fn symmetry_suffix(symmetry: MapSymmetry) -> String {
    if symmetry == MapSymmetry::None {
        String::new()
    } else {
        format!(" · mirror {}", symmetry_label(symmetry))
    }
}

/// Side panel width, open or collapsed to its two tabs.
fn panel_width(kit: &Kit, panel: SidePanel) -> f32 {
    match panel {
        SidePanel::Collapsed => kit.ctl() + 8.0,
        SidePanel::Assets | SidePanel::Check => kit.pick(330.0, 400.0),
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the panel shows the runtime's tab over the session, catalog and check"
)]
fn side_panel(
    root: &mut ChildSpawnerCommands,
    kit: &Kit,
    runtime: &MapMakerRuntime,
    session: &MapEditorSession,
    catalog: &MapContentCatalog,
    check: &MapCheck,
    scroll: &MapMakerScrollMemory,
) {
    root.spawn((
        EditorSidePanel,
        MapMakerChrome,
        Node {
            position_type: PositionType::Absolute,
            right: px(0),
            top: px(kit.metrics.bar_height()),
            bottom: px(0),
            width: px(panel_width(kit, runtime.panel)),
            padding: UiRect::all(px(4)),
            border: UiRect::left(px(1)),
            ..column(kit.pick(8.0, 6.0))
        },
        BackgroundColor(bar_fill()),
        BorderColor::all(ACCENT.with_alpha(0.25)),
    ))
    .with_children(|side| {
        let issues = check.issue_count();
        if runtime.panel == SidePanel::Collapsed {
            rail_tab(
                side,
                kit,
                index::ASSETS_TAB,
                FlowUiAction::MapMakerToggleAssets,
                "menu",
                "Assets",
                0,
            );
            rail_tab(
                side,
                kit,
                index::CHECK_TAB,
                FlowUiAction::MapMakerToggleValidation,
                if check.ready { "check" } else { "warning" },
                "Check",
                issues,
            );
            return;
        }
        let check_label = if issues == 0 {
            "Check".to_string()
        } else {
            format!("Check {issues}")
        };
        side.spawn(Node {
            width: percent(100),
            flex_shrink: 0.0,
            ..row(6.0)
        })
        .with_children(|head| {
            kit.segmented(
                head,
                true,
                vec![
                    ButtonSpec::new(
                        index::ASSETS_TAB,
                        FlowUiAction::MapMakerToggleAssets,
                        Tone::Quiet,
                    )
                    .label("Assets"),
                    ButtonSpec::new(
                        index::CHECK_TAB,
                        FlowUiAction::MapMakerToggleValidation,
                        Tone::Quiet,
                    )
                    .icon(if check.ready { "check" } else { "warning" })
                    .label(&check_label),
                ],
            );
            let collapse = match runtime.panel {
                SidePanel::Check => FlowUiAction::MapMakerToggleValidation,
                _ => FlowUiAction::MapMakerToggleAssets,
            };
            kit.button(
                head,
                ButtonSpec::new(index::COLLAPSE, collapse, Tone::Plain)
                    .icon("chevron-right")
                    .named("Collapse the panel"),
            );
        });
        if runtime.panel == SidePanel::Assets {
            assets_tab(side, kit, runtime, session, catalog, scroll);
        } else {
            check_tab(side, kit, session, catalog, check, scroll);
        }
    });
}

/// A collapsed-panel tab: icon over a short label, with an optional count badge.
fn rail_tab(
    side: &mut ChildSpawnerCommands,
    kit: &Kit,
    index: usize,
    action: FlowUiAction,
    icon: &str,
    copy: &str,
    badge: usize,
) {
    let id = kit.button(
        side,
        ButtonSpec::new(index, action, Tone::Plain)
            .named(&if badge == 0 {
                copy.to_string()
            } else {
                format!("{copy}: {badge} issues")
            })
            .width(px(kit.ctl())),
    );
    side.commands()
        .entity(id)
        .entry::<Node>()
        .and_modify(|mut node| {
            node.flex_direction = FlexDirection::Column;
            node.row_gap = px(2);
            node.padding = UiRect::all(px(2));
        });
    side.commands().entity(id).with_children(|tab| {
        kit.icon(tab, icon, kit.pick(20.0, 30.0), 1.0);
        kit.label(tab, copy, kit.pick(10.0, 13.0), TEXT);
        if badge > 0 {
            tab.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: px(-4),
                    right: px(-4),
                    min_width: px(kit.pick(16.0, 24.0)),
                    height: px(kit.pick(16.0, 24.0)),
                    padding: UiRect::horizontal(px(4)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(px(12)),
                    ..default()
                },
                BackgroundColor(PLAY),
            ))
            .with_children(|dot| {
                kit.label(dot, &badge.to_string(), kit.pick(10.0, 14.0), INK);
            });
        }
    });
}

#[allow(
    clippy::too_many_lines,
    reason = "the Assets tab declares its filter row, category menu, search field and list together"
)]
fn assets_tab(
    side: &mut ChildSpawnerCommands,
    kit: &Kit,
    runtime: &MapMakerRuntime,
    session: &MapEditorSession,
    catalog: &MapContentCatalog,
    scroll: &MapMakerScrollMemory,
) {
    side.spawn(Node {
        width: percent(100),
        flex_shrink: 0.0,
        ..row(6.0)
    })
    .with_children(|filter| {
        let picker = kit.button(
            filter,
            ButtonSpec::new(
                index::CATEGORY,
                FlowUiAction::MapMakerToggleCategoryMenu,
                Tone::Plain,
            )
            .icon(if runtime.category_menu_open {
                "chevron-up"
            } else {
                "chevron-down"
            })
            .label(runtime.palette_category.title())
            .named("Asset category"),
        );
        filter
            .commands()
            .entity(picker)
            .entry::<Node>()
            .and_modify(|mut node| {
                node.flex_grow = 1.0;
                node.flex_shrink = 1.0;
                node.justify_content = JustifyContent::SpaceBetween;
                node.flex_direction = FlexDirection::RowReverse;
            });
        kit.button(
            filter,
            ButtonSpec::new(
                index::SEARCH,
                FlowUiAction::MapMakerToggleSearch,
                Tone::Plain,
            )
            .icon("search")
            .named("Search assets"),
        );
    });
    if runtime.palette_search_active || !runtime.palette_query.is_empty() {
        side.spawn(Node {
            width: percent(100),
            flex_shrink: 0.0,
            ..row(6.0)
        })
        .with_children(|search| {
            search
                .spawn((
                    Node {
                        flex_grow: 1.0,
                        min_width: px(0),
                        min_height: px(kit.ctl()),
                        padding: UiRect::horizontal(px(12)),
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(px(RADIUS)),
                        overflow: Overflow::clip_x(),
                        ..row(8.0)
                    },
                    BackgroundColor(INK),
                    BorderColor::all(if runtime.palette_search_active {
                        ACCENT
                    } else {
                        PANEL
                    }),
                ))
                .with_children(|field| {
                    kit.icon(field, "search", kit.pick(18.0, 26.0), 0.8);
                    field.spawn((
                        MapMakerValueLabel::Search,
                        Text::new(super::search_label(runtime)),
                        kit.sans(kit.control_text()),
                        TextColor(TEXT),
                        super::widgets::no_wrap(),
                    ));
                });
            kit.button(
                search,
                ButtonSpec::new(
                    index::CLEAR_SEARCH,
                    FlowUiAction::MapMakerClearSearch,
                    Tone::Plain,
                )
                .icon("close")
                .named("Clear asset search"),
            );
        });
    }
    if runtime.category_menu_open {
        side.spawn((
            MapMakerScroll(ScrollArea::Assets),
            Node {
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(0),
                overflow: Overflow::scroll_y(),
                ..column(kit.pick(3.0, 4.0))
            },
        ))
        .with_children(|menu| {
            for (position, category) in PaletteCategory::ALL.into_iter().enumerate() {
                let id = kit.button(
                    menu,
                    ButtonSpec::new(
                        index::FIRST_CATEGORY + position,
                        FlowUiAction::MapMakerSelectCategory(category as u8),
                        Tone::Row,
                    )
                    .label(category.title())
                    .width(percent(100)),
                );
                menu.commands()
                    .entity(id)
                    .entry::<Node>()
                    .and_modify(|mut node| {
                        node.justify_content = JustifyContent::FlexStart;
                    });
            }
        });
        return;
    }
    let mode = session.document.recipe.mode_definition_id;
    let entries = visible_palette_entries(catalog, runtime, mode);
    side.spawn((
        MapMakerScroll(ScrollArea::Assets),
        scroll.position(ScrollArea::Assets),
        Node {
            width: percent(100),
            flex_grow: 1.0,
            min_height: px(0),
            overflow: Overflow::scroll_y(),
            ..column(kit.pick(3.0, 4.0))
        },
    ))
    .with_children(|list| {
        if entries.is_empty() {
            kit.label(
                list,
                "No assets match this category and search.",
                kit.pick(13.0, 17.0),
                TEXT.with_alpha(0.8),
            );
        }
        for (palette_index, item) in entries {
            asset_row(list, kit, catalog, palette_index, item);
        }
    });
    if !kit.metrics.compact {
        kit.label(
            side,
            "Click or drag to place · R rotates · Space-drag pans",
            11.0,
            TEXT.with_alpha(0.75),
        );
    }
}

fn asset_row(
    list: &mut ChildSpawnerCommands,
    kit: &Kit,
    catalog: &MapContentCatalog,
    palette_index: usize,
    item: PaletteItem,
) {
    let name = palette_label(Some(item), catalog);
    let id = kit.button(
        list,
        ButtonSpec::new(
            index::FIRST_ASSET + palette_index,
            FlowUiAction::MapMakerSelectAsset(palette_index),
            Tone::Row,
        )
        .named(&name)
        .width(percent(100)),
    );
    list.commands()
        .entity(id)
        .entry::<Node>()
        .and_modify(|mut node| {
            node.justify_content = JustifyContent::FlexStart;
            node.column_gap = px(10);
        });
    let (width, height) = footprint(item, catalog);
    list.commands().entity(id).with_children(|row_node| {
        swatch(
            row_node,
            palette_color(item, catalog),
            width,
            height,
            kit.pick(16.0, 26.0),
        );
        row_node.spawn((
            Text::new(name),
            kit.sans(kit.pick(14.0, 19.0)),
            TextColor(TEXT),
            Node {
                flex_grow: 1.0,
                flex_shrink: 1.0,
                min_width: px(0),
                ..default()
            },
            super::widgets::no_wrap(),
        ));
        kit.label(
            row_node,
            &format!("{width}×{height}"),
            kit.pick(12.0, 16.0),
            ACCENT,
        );
    });
}

/// Footprint in cells of a palette entry.
fn footprint(item: PaletteItem, catalog: &MapContentCatalog) -> (u8, u8) {
    match item {
        PaletteItem::Asset(id) => catalog.asset(id).map_or((1, 1), |asset| {
            (asset.footprint_cells.width, asset.footprint_cells.height)
        }),
        PaletteItem::Spawn(_) => (1, 1),
        PaletteItem::HotZone => (3, 3),
        PaletteItem::HeistSafe(_) => (3, 2),
    }
}

/// A flat footprint swatch: up to 3×2 cells in the entry's palette colour.
fn swatch(parent: &mut ChildSpawnerCommands, color: Color, width: u8, height: u8, cell: f32) {
    let columns = width.clamp(1, 3);
    let rows = height.clamp(1, 2);
    parent
        .spawn(Node {
            width: px(cell * 3.0 + 4.0),
            flex_shrink: 0.0,
            justify_content: JustifyContent::Center,
            ..column(2.0)
        })
        .with_children(|swatch| {
            for _ in 0..rows {
                swatch
                    .spawn(Node {
                        justify_content: JustifyContent::Center,
                        ..row(2.0)
                    })
                    .with_children(|line| {
                        for _ in 0..columns {
                            line.spawn((
                                Node {
                                    width: px(cell),
                                    height: px(cell / f32::from(rows).max(1.0) * 1.0),
                                    border: UiRect::all(px(1)),
                                    border_radius: BorderRadius::all(px(3)),
                                    ..default()
                                },
                                BackgroundColor(color),
                                BorderColor::all(INK),
                            ));
                        }
                    });
            }
        });
}

fn check_tab(
    side: &mut ChildSpawnerCommands,
    kit: &Kit,
    session: &MapEditorSession,
    catalog: &MapContentCatalog,
    check: &MapCheck,
    scroll: &MapMakerScrollMemory,
) {
    let issues = check.issue_count();
    let summary = match (check.ready, issues) {
        (true, _) => "Ready to test".to_string(),
        (false, 0 | 1) => "1 problem blocks Test".to_string(),
        (false, count) => format!("{count} problems block Test"),
    };
    side.spawn(Node {
        padding: UiRect::axes(px(6), px(2)),
        flex_shrink: 0.0,
        ..row(6.0)
    })
    .with_children(|line| {
        kit.icon(
            line,
            if check.ready { "check" } else { "warning" },
            kit.pick(18.0, 26.0),
            1.0,
        );
        kit.label(
            line,
            &summary,
            kit.pick(14.0, 18.0),
            if check.ready { ACCENT } else { PLAY_HIGHLIGHT },
        );
    });
    let palette = palette(catalog, session.document.recipe.mode_definition_id);
    side.spawn((
        MapMakerScroll(ScrollArea::Check),
        scroll.position(ScrollArea::Check),
        Node {
            width: percent(100),
            flex_grow: 1.0,
            min_height: px(0),
            overflow: Overflow::scroll_y(),
            ..column(kit.pick(4.0, 6.0))
        },
    ))
    .with_children(|list| {
        let mut fix_slot = 0;
        for requirement in check.requirements.iter().filter(|r| !r.met()) {
            let fix = palette
                .iter()
                .position(|item| *item == requirement.fix)
                .map(|position| {
                    fix_slot += 1;
                    (index::FIRST_FIX + fix_slot, position)
                });
            let headline = requirement.headline();
            let fix = fix.map(|(index, palette_index)| IssueAction {
                index,
                action: FlowUiAction::MapMakerShowPaletteItem(palette_index),
                label: "Add",
                spoken: format!("Show the asset that fixes: {headline}"),
            });
            issue_row(list, kit, &headline, &requirement.advice(), fix);
        }
        if let Some(rule) = check.rule_error.as_ref() {
            rule_issue_row(list, kit, rule);
        }
        kit.label(list, "Passed", kit.pick(12.0, 15.0), ACCENT);
        let passed: Vec<String> = check
            .requirements
            .iter()
            .filter(|r| r.met())
            .map(Requirement::headline)
            .chain(check.ready.then(|| "Map rules pass".to_string()))
            .collect();
        if passed.is_empty() {
            kit.label(
                list,
                "Nothing yet.",
                kit.pick(12.0, 16.0),
                TEXT.with_alpha(0.7),
            );
        }
        for line_copy in passed {
            list.spawn(Node {
                padding: UiRect::horizontal(px(8)),
                ..row(8.0)
            })
            .with_children(|line| {
                kit.icon(line, "check", kit.pick(16.0, 22.0), 1.0);
                kit.label(
                    line,
                    &line_copy,
                    kit.pick(12.0, 16.0),
                    TEXT.with_alpha(0.85),
                );
            });
        }
    });
}

/// The validator refusal no count explains, with Show when it names placements or cells.
fn rule_issue_row(list: &mut ChildSpawnerCommands, kit: &Kit, rule: &RuleIssue) {
    let show = rule.located().then(|| IssueAction {
        index: index::SHOW_RULE,
        action: FlowUiAction::MapMakerShowRuleIssue,
        label: "Show",
        spoken: format!("Show on the map: {}", rule.text),
    });
    let advice = if rule.located() {
        "Show selects it on the map. Fix it, then check again."
    } else {
        "Fix this, then check again."
    };
    issue_row(list, kit, &rule.text, advice, show);
}

/// A Check issue's action: Add (the palette entry that fixes it) or Show (where it is).
struct IssueAction {
    index: usize,
    action: FlowUiAction,
    label: &'static str,
    spoken: String,
}

/// One Check issue: what is wrong, how to fix it, and its Add or Show action.
fn issue_row(
    list: &mut ChildSpawnerCommands,
    kit: &Kit,
    headline: &str,
    advice: &str,
    action: Option<IssueAction>,
) {
    list.spawn((
        Node {
            width: percent(100),
            flex_shrink: 0.0,
            padding: UiRect::axes(px(8), px(4)),
            border_radius: BorderRadius::all(px(RADIUS)),
            ..row(8.0)
        },
        BackgroundColor(PANEL.with_alpha(0.7)),
    ))
    .with_children(|issue| {
        kit.icon(issue, "warning", kit.pick(18.0, 26.0), 1.0);
        issue
            .spawn(Node {
                flex_grow: 1.0,
                flex_shrink: 1.0,
                min_width: px(0),
                ..column(0.0)
            })
            .with_children(|copy| {
                kit.label(copy, headline, kit.pick(13.0, 17.0), TEXT);
                kit.label(copy, advice, kit.pick(11.0, 14.0), ACCENT);
            });
        if let Some(action) = action {
            kit.button(
                issue,
                ButtonSpec::new(action.index, action.action, Tone::Plain)
                    .label(action.label)
                    .named(&action.spoken),
            );
        }
    });
}

/// The bottom-left chip naming what the next tap places, or the active tool's gesture.
fn corner_chip(
    root: &mut ChildSpawnerCommands,
    kit: &Kit,
    runtime: &MapMakerRuntime,
    session: &MapEditorSession,
    catalog: &MapContentCatalog,
) {
    let anchor = Node {
        position_type: PositionType::Absolute,
        left: px(8),
        bottom: px(8),
        ..default()
    };
    if runtime.tool != EditorTool::Place {
        let hint = match (runtime.tool, kit.metrics.compact) {
            (EditorTool::Erase, true) => "Erase · tap or drag",
            (EditorTool::Erase, false) => "Erase · click or drag · Esc cancels",
            (_, true) => "Select · drag a box · drag a selection to move",
            (_, false) => "Select · drag a box · Shift adds · Delete removes",
        };
        root.spawn((
            anchor,
            MapMakerChrome,
            CurrentAssetChip,
            children![(
                Node {
                    min_height: px(kit.ctl()),
                    padding: UiRect::axes(px(12), px(4)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(RADIUS)),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(bar_fill()),
                BorderColor::all(ACCENT.with_alpha(0.35)),
                children![(
                    Text::new(hint),
                    kit.sans(kit.pick(13.0, 18.0)),
                    TextColor(TEXT),
                )],
            )],
        ));
        return;
    }
    let item = selected_palette_item(catalog, runtime, session.document.recipe.mode_definition_id);
    let theme = session.document.recipe.presentation_theme_id;
    root.spawn((anchor, MapMakerChrome, CurrentAssetChip))
        .with_children(|anchor| {
            let id = kit.button(
                anchor,
                ButtonSpec::new(index::CHIP, FlowUiAction::MapMakerToggleAssets, Tone::Plain)
                    .named("Current asset: open Assets"),
            );
            anchor
                .commands()
                .entity(id)
                .entry::<Node>()
                .and_modify(|mut node| {
                    node.justify_content = JustifyContent::FlexStart;
                    node.width = Val::Auto;
                    node.padding = UiRect::axes(px(8), px(4));
                    node.column_gap = px(10);
                });
            anchor.commands().entity(id).with_children(|chip| {
                let preview = kit.ctl() - 12.0;
                chip.spawn((
                    ChipPreviewFrame,
                    Node {
                        width: px(preview * 168.0 / 124.0),
                        height: px(preview),
                        flex_shrink: 0.0,
                        ..default()
                    },
                ))
                .with_children(|frame| {
                    if let Some(item) = item {
                        frame.spawn(chip_preview_host(item, theme));
                    }
                });
                chip.spawn(column(0.0)).with_children(|copy| {
                    copy.spawn((
                        MapMakerValueLabel::Asset,
                        Text::new(chip_name(runtime, catalog, item)),
                        kit.sans(kit.pick(14.0, 19.0)),
                        TextColor(TEXT),
                        super::widgets::no_wrap(),
                    ));
                    copy.spawn((
                        MapMakerValueLabel::AssetHint,
                        Text::new(chip_hint(kit.metrics.compact, item, catalog)),
                        kit.sans(kit.pick(11.0, 14.0)),
                        TextColor(ACCENT),
                        super::widgets::no_wrap(),
                    ));
                });
            });
        });
}

pub(super) fn chip_preview_host(
    item: PaletteItem,
    theme: brawler::map::MapPresentationThemeId,
) -> impl Bundle {
    (
        MapPalettePreviewHost {
            slot: 0,
            visual: item.into(),
            theme,
        },
        Node {
            width: percent(100),
            height: percent(100),
            ..default()
        },
        BackgroundColor(PANEL),
    )
}

pub(super) fn chip_name(
    _runtime: &MapMakerRuntime,
    catalog: &MapContentCatalog,
    item: Option<PaletteItem>,
) -> String {
    palette_label(item, catalog)
}

pub(super) fn chip_hint(
    compact: bool,
    item: Option<PaletteItem>,
    catalog: &MapContentCatalog,
) -> String {
    let category = item.map_or("Assets", |item| palette_category(item, catalog).title());
    let rotates = item.is_some_and(|item| match item {
        PaletteItem::Asset(id) => catalog
            .asset(id)
            .is_some_and(|asset| asset.slot != MapAssetSlot::Surface),
        PaletteItem::Spawn(_) => true,
        PaletteItem::HotZone | PaletteItem::HeistSafe(_) => false,
    });
    match (compact, rotates) {
        (true, _) => format!("{category} · tap to change"),
        (false, true) => format!("{category} · R rotates"),
        (false, false) => category.to_string(),
    }
}

fn status_toast(root: &mut ChildSpawnerCommands, kit: &Kit, runtime: &MapMakerRuntime) {
    root.spawn((
        MapMakerStatusLine,
        MapMakerChrome,
        Node {
            display: super::status_display(runtime),
            position_type: PositionType::Absolute,
            left: px(8),
            top: px(kit.metrics.bar_height() + 8.0),
            max_width: percent(55),
            padding: UiRect::axes(px(12), px(7)),
            border: UiRect::left(px(4)),
            border_radius: BorderRadius::all(px(RADIUS)),
            ..default()
        },
        BackgroundColor(bar_fill()),
        BorderColor::all(PLAY_HIGHLIGHT),
    ))
    .with_child((
        MapMakerValueLabel::Status,
        Text::new(runtime.status.clone().unwrap_or_default()),
        kit.sans(kit.pick(14.0, 18.0)),
        TextColor(TEXT),
    ));
}

fn selection_toolbar(
    root: &mut ChildSpawnerCommands,
    kit: &Kit,
    runtime: &MapMakerRuntime,
    position: Option<Vec2>,
) {
    if runtime.tool != EditorTool::Select || runtime.selection.is_empty() {
        return;
    }
    let (left, top) = selection_toolbar_offset(position, kit.metrics.ctl, kit.metrics.bar_height());
    root.spawn((
        MapMakerSelectionToolbar,
        MapMakerChrome,
        Node {
            position_type: PositionType::Absolute,
            left,
            top,
            padding: UiRect::all(px(4)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(RADIUS + 2.0)),
            ..row(4.0)
        },
        BackgroundColor(bar_fill()),
        BorderColor::all(ACCENT.with_alpha(0.35)),
    ))
    .with_children(|bar| {
        for (index, action, icon, name, tone) in [
            (
                index::ROTATE,
                FlowUiAction::MapMakerRotate,
                "rotate",
                "Rotate selection 90 degrees",
                Tone::Plain,
            ),
            (
                index::DUPLICATE,
                FlowUiAction::MapMakerDuplicateSelection,
                "duplicate",
                "Duplicate selection",
                Tone::Plain,
            ),
            (
                index::DELETE,
                FlowUiAction::MapMakerDeleteSelection,
                "trash",
                "Delete selection",
                Tone::Danger,
            ),
            (
                index::DESELECT,
                FlowUiAction::MapMakerClearSelection,
                "close",
                "Clear selection",
                Tone::Plain,
            ),
        ] {
            kit.button(
                bar,
                ButtonSpec::new(index, action, tone).icon(icon).named(name),
            );
        }
    });
}

/// Toolbar origin above the selection, kept below the top bar and away from the left edge.
fn selection_toolbar_offset(position: Option<Vec2>, ctl: f32, bar_height: f32) -> (Val, Val) {
    let position = position.unwrap_or(Vec2::new(640.0, 260.0));
    let width = ctl * 4.0 + 20.0;
    (
        px((position.x - width * 0.5).max(8.0)),
        px((position.y - ctl - 28.0).max(bar_height + 8.0)),
    )
}

pub(super) fn selection_toolbar_position(
    session: &MapEditorSession,
    catalog: &MapContentCatalog,
    selection: &BTreeSet<brawler::map::MapPlacementId>,
    camera: Option<(&Camera, &GlobalTransform)>,
) -> Option<Vec2> {
    let (camera, camera_transform) = camera?;
    let recipe = &session.document.recipe;
    let mut cells = recipe
        .placements
        .iter()
        .filter(|placement| selection.contains(&placement.placement_id))
        .flat_map(|placement| {
            catalog
                .asset(placement.asset_id)
                .and_then(|asset| {
                    brawler::map::placement_cells(recipe.dimensions, asset, placement)
                })
                .unwrap_or_else(|| vec![placement.cell])
        })
        .collect::<Vec<_>>();
    cells.extend(
        recipe
            .mode_anchors
            .iter()
            .filter(|anchor| selection.contains(&anchor.placement_id))
            .map(|anchor| match anchor.kind {
                brawler::map::MapModeAnchorKind::HotZoneCircle {
                    center_half_cell, ..
                }
                | brawler::map::MapModeAnchorKind::ZombieIngress { center_half_cell } => {
                    MapCell::new(center_half_cell.x / 2, center_half_cell.y / 2)
                }
                brawler::map::MapModeAnchorKind::HeistSafe { origin_cell, .. } => origin_cell,
            }),
    );
    let min_x = cells.iter().map(|cell| cell.x).min()?;
    let max_x = cells.iter().map(|cell| cell.x).max()?;
    let min_y = cells.iter().map(|cell| cell.y).min()?;
    let max_y = cells.iter().map(|cell| cell.y).max()?;
    let a = recipe.dimensions.cell_center(MapCell::new(min_x, min_y));
    let b = recipe.dimensions.cell_center(MapCell::new(max_x, max_y));
    let center = (a + b) * 0.5;
    camera
        .world_to_viewport(camera_transform, Vec3::new(center.x, 7.0, -center.y))
        .ok()
}

/// Keeps the retained selection toolbar anchored to the selection while the camera pans or zooms.
#[allow(clippy::needless_pass_by_value, reason = "Bevy system parameters")]
pub(in crate::flow) fn position_map_editor_selection_toolbar(
    runtime: Res<MapMakerRuntime>,
    catalog: Option<Res<MapCatalogResource>>,
    cameras: Query<(&Camera, Ref<GlobalTransform>), With<ArenaCamera>>,
    mut toolbars: Query<&mut Node, With<MapMakerSelectionToolbar>>,
    ui_scale: Option<Res<UiScale>>,
    metrics: MetricsParam,
) {
    let camera = cameras.iter().next();
    let camera_moved = camera
        .as_ref()
        .is_some_and(|(_, transform)| transform.is_changed());
    if toolbars.is_empty() || !(runtime.is_changed() || camera_moved) {
        return;
    }
    let (Some(session), Some(catalog)) = (runtime.session.as_ref(), catalog.as_deref()) else {
        return;
    };
    let metrics = metrics.get();
    let position = selection_toolbar_position(
        session,
        &catalog.0,
        &runtime.selection,
        camera
            .as_ref()
            .map(|(camera, transform)| (*camera, &**transform)),
    )
    .map(|point| crate::ui::viewport::viewport_to_ui(point, ui_scale.as_deref()));
    let (left, top) = selection_toolbar_offset(position, metrics.ctl, metrics.bar_height());
    for mut node in &mut toolbars {
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
    }
}

fn value_label_text(
    label: MapMakerValueLabel,
    runtime: &MapMakerRuntime,
    catalog: Option<&MapContentCatalog>,
    compact: bool,
) -> Option<String> {
    let item = || {
        let session = runtime.session.as_ref()?;
        let catalog = catalog?;
        selected_palette_item(catalog, runtime, session.document.recipe.mode_definition_id)
    };
    Some(match label {
        MapMakerValueLabel::Symmetry => symmetry_suffix(
            runtime
                .session
                .as_ref()
                .map_or(MapSymmetry::None, |session| session.symmetry),
        ),
        MapMakerValueLabel::Search => super::search_label(runtime),
        MapMakerValueLabel::Status => runtime.status.clone().unwrap_or_default(),
        MapMakerValueLabel::Asset => chip_name(runtime, catalog?, item()),
        MapMakerValueLabel::AssetHint => chip_hint(compact, item(), catalog?),
    })
}

/// Applies value-only edits to retained editor labels, the status toast and the chip's asset
/// preview; structural edits still bump the render revision and rebuild through
/// `present_map_editor`.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Bevy system parameters"
)]
pub(in crate::flow) fn update_map_editor_values(
    runtime: Res<MapMakerRuntime>,
    catalog: Option<Res<MapCatalogResource>>,
    metrics: MetricsParam,
    mut labels: Query<(
        &MapMakerValueLabel,
        Option<&mut Text>,
        Option<&mut TextSpan>,
    )>,
    mut status_lines: Query<&mut Node, With<MapMakerStatusLine>>,
) {
    if !runtime.is_changed() {
        return;
    }
    let catalog = catalog.as_deref().map(|catalog| &catalog.0);
    let compact = metrics.get().compact;
    for (label, text, span) in &mut labels {
        let Some(next) = value_label_text(*label, &runtime, catalog, compact) else {
            continue;
        };
        if let Some(mut text) = text
            && text.0 != next
        {
            text.0 = next;
        } else if let Some(mut span) = span
            && span.0 != next
        {
            span.0 = next;
        }
    }
    let display = super::status_display(&runtime);
    for mut node in &mut status_lines {
        if node.display != display {
            node.display = display;
        }
    }
}

/// Swaps the chip's live asset preview when the selected asset changes, without a rebuild.
#[allow(clippy::needless_pass_by_value, reason = "Bevy system parameters")]
pub(in crate::flow) fn follow_chip_preview(
    mut commands: Commands,
    runtime: Res<MapMakerRuntime>,
    catalog: Option<Res<MapCatalogResource>>,
    frames: Query<(Entity, Option<&Children>), With<ChipPreviewFrame>>,
    hosts: Query<&MapPalettePreviewHost>,
) {
    if !runtime.is_changed() {
        return;
    }
    let catalog = catalog.as_deref().map(|catalog| &catalog.0);
    let (Some(session), Some(catalog)) = (runtime.session.as_ref(), catalog) else {
        return;
    };
    let item = selected_palette_item(
        catalog,
        &runtime,
        session.document.recipe.mode_definition_id,
    );
    let theme = session.document.recipe.presentation_theme_id;
    for (frame, children) in &frames {
        let current = children
            .into_iter()
            .flatten()
            .find_map(|child| hosts.get(*child).ok().map(|host| (*child, host.visual)));
        let wanted = item.map(super::MapPaletteVisual::from);
        if current.map(|(_, visual)| visual) == wanted {
            continue;
        }
        if let Some((child, _)) = current {
            commands.entity(child).despawn();
        }
        if let Some(item) = item {
            commands
                .entity(frame)
                .with_child(chip_preview_host(item, theme));
        }
    }
}

/// The arena's top and right insets in physical pixels: the area the editor camera draws into.
#[allow(clippy::needless_pass_by_value, reason = "Bevy system parameters")]
pub(in crate::flow) fn publish_map_editor_arena_insets(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    bars: Query<&ComputedNode, With<EditorTopBar>>,
    panels: Query<&ComputedNode, With<EditorSidePanel>>,
    current: Option<Res<crate::presentation_3d::MapEditorArenaInsets>>,
) {
    if *flow.get() != ClientFlow::MapEditor {
        if current.is_some() {
            commands.remove_resource::<crate::presentation_3d::MapEditorArenaInsets>();
        }
        return;
    }
    let (Some(bar), Some(panel)) = (bars.iter().next(), panels.iter().next()) else {
        return;
    };
    let insets = crate::presentation_3d::MapEditorArenaInsets {
        top: bar.size().y,
        right: panel.size().x,
    };
    if current.as_deref() != Some(&insets) {
        commands.insert_resource(insets);
    }
}
