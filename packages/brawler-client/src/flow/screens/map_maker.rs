//! Native Map Maker: library and editor state, action resolution and input ownership. The
//! views live in `library` (My Maps) and `editor`, built from the flat controls in `widgets`
//! (BRL-2xpc direction D).

mod check;
mod editor;
mod focus;
mod library;
mod scroll;
mod widgets;

#[cfg(test)]
pub(in crate::flow) use editor::{CurrentAssetChip, EditorSidePanel, EditorTopBar};
pub(in crate::flow) use editor::{
    follow_chip_preview, position_map_editor_selection_toolbar, present_map_editor,
    publish_map_editor_arena_insets, update_map_editor_values,
};
pub(in crate::flow) use focus::{navigate_map_editor, show_map_editor_focus_badge};
pub(in crate::flow) use library::{
    keep_map_selection_valid, navigate_map_library, present_map_library, update_map_library_values,
};
pub(in crate::flow) use scroll::{MapMakerScrollMemory, reveal_map_maker_focus, scroll_map_maker};
pub(in crate::flow) use widgets::{FlatControl, style_flat_controls};

use crate::text_entry::TextEdit;
use crate::{
    ArenaCamera, ClientLobbyMembership, ClientPracticeModel, MapDraftDocument, MapDraftId,
    MapDraftLibrary, MapEditorSession,
    flow::{
        ClientFlow, SelectedGameType, SessionPurpose,
        actions::{FlowCommit, FlowUiAction, OverlayCommit, PendingFlowActions},
        input::queue_ui_action,
        screens::shared::FlowButton,
    },
    map_maker::{MapDraftStorage, delete_map_draft, load_map_drafts, save_map_draft},
    ui::theme::{PLAY_HIGHLIGHT, TEXT},
};
use bevy::{
    input::mouse::{AccumulatedMouseMotion, MouseWheel},
    prelude::*,
    window::PrimaryWindow,
};
use brawler::map::{
    AuthoredMapCandidate, MapAssetId, MapAssetSlot, MapCatalogResource, MapCell, MapSymmetry,
    authorable_assets,
};
use lightyear::prelude::client::Client;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum EditorTool {
    #[default]
    Place,
    Erase,
    Select,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum EditorInteraction {
    #[default]
    Idle,
    Paint {
        cells: Vec<MapCell>,
        asset: Option<MapAssetId>,
    },
    Erase {
        cells: Vec<MapCell>,
    },
    MarqueeSelect {
        anchor: MapCell,
        additive: bool,
    },
    MoveSelection {
        anchor: MapCell,
    },
}

/// The editor's side panel: open on Assets or Check, or collapsed to its two tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SidePanel {
    Collapsed,
    #[default]
    Assets,
    Check,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
enum PaletteCategory {
    #[default]
    Ground,
    CoverObjects,
    Vegetation,
    EffectTiles,
    Decorations,
    SpawnsObjectives,
}

impl PaletteCategory {
    const ALL: [Self; 6] = [
        Self::Ground,
        Self::CoverObjects,
        Self::Vegetation,
        Self::EffectTiles,
        Self::Decorations,
        Self::SpawnsObjectives,
    ];

    const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Ground),
            1 => Some(Self::CoverObjects),
            2 => Some(Self::Vegetation),
            3 => Some(Self::EffectTiles),
            4 => Some(Self::Decorations),
            5 => Some(Self::SpawnsObjectives),
            _ => None,
        }
    }
}

#[allow(
    clippy::struct_excessive_bools,
    reason = "these flags represent independent editor presentation and confirmation concerns"
)]
#[derive(Resource)]
pub(in crate::flow) struct MapMakerRuntime {
    pub session: Option<MapEditorSession>,
    cursor: MapCell,
    palette_index: usize,
    quarter_turns: u8,
    status: Option<String>,
    pending_delete: Option<MapDraftId>,
    renaming: Option<(MapDraftId, String)>,
    creating: Option<String>,
    /// The library tile the detail actions act on.
    selected_draft: Option<MapDraftId>,
    /// My Maps shows the maps published to this server instead of local drafts (BRL-s3dh).
    published_view: bool,
    selected_published: Option<brawler::lobby::PublishedMapId>,
    pending_withdraw: Option<brawler::lobby::PublishedMapId>,
    /// Re-list the published maps as soon as no request is in flight.
    refresh_published: bool,
    clear_armed: bool,
    grid_visible: bool,
    tool: EditorTool,
    interaction: EditorInteraction,
    selection: BTreeSet<brawler::map::MapPlacementId>,
    palette_category: PaletteCategory,
    palette_query: String,
    palette_search_active: bool,
    category_menu_open: bool,
    panel: SidePanel,
    render_revision: u64,
    return_to_editor: bool,
    camera_center: Vec2,
    camera_zoom: f32,
    /// Keyboard and controller input drive the editor chrome, not the map cursor (BRL-q5sm).
    chrome_focus: bool,
}

impl MapMakerRuntime {
    /// The new or renamed map name being typed in the library, if any.
    pub(in crate::flow) fn editing_map_name(&self) -> Option<&str> {
        self.creating
            .as_deref()
            .or(self.renaming.as_ref().map(|(_, name)| name.as_str()))
    }

    /// Chrome-focus mode, the selected palette index and the map cursor, for flow tests.
    #[cfg(test)]
    pub(in crate::flow) fn focus_test_state(&self) -> (bool, usize, MapCell) {
        (self.chrome_focus, self.palette_index, self.cursor)
    }

    /// The asset search being typed in the editor, if any.
    pub(in crate::flow) fn editing_asset_search(&self) -> Option<&str> {
        self.palette_search_active
            .then_some(self.palette_query.as_str())
    }
}

impl Default for MapMakerRuntime {
    fn default() -> Self {
        Self {
            session: None,
            cursor: MapCell::new(0, 0),
            palette_index: 0,
            quarter_turns: 0,
            status: None,
            pending_delete: None,
            renaming: None,
            creating: None,
            selected_draft: None,
            published_view: false,
            selected_published: None,
            pending_withdraw: None,
            refresh_published: false,
            clear_armed: false,
            grid_visible: true,
            tool: EditorTool::Place,
            interaction: EditorInteraction::Idle,
            selection: BTreeSet::new(),
            palette_category: PaletteCategory::CoverObjects,
            palette_query: String::new(),
            palette_search_active: false,
            category_menu_open: false,
            panel: SidePanel::Assets,
            render_revision: 0,
            return_to_editor: false,
            camera_center: Vec2::ZERO,
            camera_zoom: 1.0,
            chrome_focus: false,
        }
    }
}

/// A Map Maker view root: rebuilt when the render revision or the layout metrics change.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(in crate::flow) struct MapMakerRoot {
    revision: u64,
    metrics: widgets::Metrics,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum PaletteItem {
    Asset(MapAssetId),
    Spawn(u8),
    HotZone,
    HeistSafe(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MapPaletteVisual {
    Asset(MapAssetId),
    Spawn(u8),
    HotZone,
    HeistSafe(u8),
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MapPalettePreviewHost {
    pub(crate) slot: usize,
    pub(crate) visual: MapPaletteVisual,
    pub(crate) theme: brawler::map::MapPresentationThemeId,
}

impl From<PaletteItem> for MapPaletteVisual {
    fn from(value: PaletteItem) -> Self {
        match value {
            PaletteItem::Asset(id) => Self::Asset(id),
            PaletteItem::Spawn(team) => Self::Spawn(team),
            PaletteItem::HotZone => Self::HotZone,
            PaletteItem::HeistSafe(team) => Self::HeistSafe(team),
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
pub(in crate::flow) fn load_map_library(mut commands: Commands, storage: Res<MapDraftStorage>) {
    commands.insert_resource(load_map_drafts(&storage));
}

#[allow(clippy::needless_pass_by_value)]
pub(in crate::flow) fn sync_map_editor_preview(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    runtime: Res<MapMakerRuntime>,
    catalog: Option<Res<MapCatalogResource>>,
    current: Option<Res<crate::presentation_3d::MapEditorPreview>>,
) {
    if *flow.get() != ClientFlow::MapEditor {
        if current.is_some() {
            commands.remove_resource::<crate::presentation_3d::MapEditorPreview>();
        }
        return;
    }
    let (Some(session), Some(catalog)) = (runtime.session.as_ref(), catalog.as_deref()) else {
        return;
    };
    let mut preview_session = session.clone();
    let mut valid = true;
    if runtime.tool == EditorTool::Place {
        let item = selected_palette_item(
            &catalog.0,
            &runtime,
            session.document.recipe.mode_definition_id,
        );
        let result = match &runtime.interaction {
            EditorInteraction::Paint {
                cells,
                asset: Some(asset_id),
            } => preview_session.paint_stroke(
                &catalog.0,
                *asset_id,
                cells.iter().copied(),
                runtime.quarter_turns,
            ),
            _ => item.map_or(Ok(()), |item| {
                preview_place_item(
                    &mut preview_session,
                    &catalog.0,
                    item,
                    runtime.cursor,
                    runtime.quarter_turns,
                )
            }),
        };
        valid = result.is_ok();
    } else if runtime.tool == EditorTool::Erase {
        if let EditorInteraction::Erase { cells } = &runtime.interaction {
            valid = preview_session
                .erase_cells(&catalog.0, cells.iter().copied())
                .is_ok();
        }
    } else if runtime.tool == EditorTool::Select
        && let EditorInteraction::MoveSelection { anchor } = &runtime.interaction
    {
        valid = preview_session
            .move_placements_by(
                &catalog.0,
                &runtime.selection,
                *anchor,
                i32::from(runtime.cursor.x) - i32::from(anchor.x),
                i32::from(runtime.cursor.y) - i32::from(anchor.y),
            )
            .is_ok();
    }
    let hovered = (runtime.tool == EditorTool::Select)
        .then(|| {
            session
                .placements_in_rect(&catalog.0, runtime.cursor, runtime.cursor)
                .into_iter()
                .find(|id| !runtime.selection.contains(id))
        })
        .flatten();
    let preview = crate::presentation_3d::MapEditorPreview::from_recipe(
        session.document.id,
        &preview_session.document.recipe,
        &catalog.0,
        crate::presentation_3d::MapEditorPreviewOptions {
            cursor: runtime.cursor,
            grid_visible: runtime.grid_visible,
            camera_center: runtime.camera_center,
            camera_zoom: runtime.camera_zoom,
            symmetry: session.symmetry,
            pointer_mode: match runtime.tool {
                EditorTool::Place => crate::presentation_3d::MapEditorPointerMode::Place,
                EditorTool::Erase => crate::presentation_3d::MapEditorPointerMode::Erase,
                EditorTool::Select => crate::presentation_3d::MapEditorPointerMode::Move,
            },
            pointer_valid: valid,
            selected: runtime.selection.iter().copied().collect(),
            hovered,
            selection_rect: match runtime.interaction {
                EditorInteraction::MarqueeSelect { anchor, .. } => Some((anchor, runtime.cursor)),
                _ => None,
            },
        },
    );
    if current.as_deref() != Some(&preview) {
        commands.insert_resource(preview);
    }
}

fn preview_place_item(
    session: &mut MapEditorSession,
    catalog: &brawler::map::MapContentCatalog,
    item: PaletteItem,
    cell: MapCell,
    quarter_turns: u8,
) -> Result<(), String> {
    match item {
        PaletteItem::Asset(id) => session.place(catalog, id, cell, quarter_turns),
        PaletteItem::Spawn(team) => session.place_spawn(catalog, cell, team, quarter_turns),
        PaletteItem::HotZone => session.place_hot_zone(cell),
        PaletteItem::HeistSafe(team) => session.place_heist_safe(cell, team),
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]
pub(in crate::flow) fn resolve_map_maker_action(
    mut actions: ResMut<PendingFlowActions>,
    flow: Res<State<ClientFlow>>,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    catalog: Option<Res<MapCatalogResource>>,
    storage: Res<MapDraftStorage>,
    mut library: ResMut<MapDraftLibrary>,
    mut runtime: ResMut<MapMakerRuntime>,
    mut selection: ResMut<SelectedGameType>,
    mut practice: ResMut<ClientPracticeModel>,
    mut publish: ResMut<crate::ClientMapPublishModel>,
    mut published: ResMut<crate::ClientPublishedMapsModel>,
    mut purpose: ResMut<SessionPurpose>,
    mut commit: ResMut<FlowCommit>,
    metrics: widgets::MetricsParam,
) {
    let Some(action) = actions.ordinary.as_ref() else {
        return;
    };
    if !is_map_maker_action(action) {
        return;
    }
    let Some(catalog) = catalog else {
        return;
    };
    let Some(action) = actions.ordinary.take() else {
        return;
    };
    match action {
        FlowUiAction::OpenMapMaker if *flow.get() == ClientFlow::Dashboard => {
            // Published states change on the server between visits (BRL-s3dh).
            runtime.refresh_published = true;
            commit.overlay = Some(OverlayCommit::Clear);
            commit.next_flow = Some(ClientFlow::MapLibrary);
        }
        FlowUiAction::CloseMapMaker if *flow.get() == ClientFlow::MapLibrary => {
            commit.next_flow = Some(ClientFlow::Dashboard);
        }
        FlowUiAction::CreateMap if *flow.get() == ClientFlow::MapLibrary => {
            runtime.creating = Some(format!("Untitled Map {}", library.next_id().0));
            runtime.renaming = None;
            runtime.pending_delete = None;
            runtime.status = None;
            runtime.bump_render();
        }
        FlowUiAction::ConfirmCreateMap if *flow.get() == ClientFlow::MapLibrary => {
            let name = runtime
                .creating
                .clone()
                .unwrap_or_else(|| "Untitled Map".to_string());
            create_default_document(
                &memberships,
                &selection,
                &catalog.0,
                &mut library,
                &mut runtime,
                &mut commit,
                &name,
            );
            if commit.next_flow == Some(ClientFlow::MapEditor) {
                runtime.panel = SidePanel::opening(metrics.get().compact);
            }
        }
        FlowUiAction::CancelCreateMap if *flow.get() == ClientFlow::MapLibrary => {
            runtime.creating = None;
            runtime.status = None;
            runtime.bump_render();
        }
        FlowUiAction::OpenMapDraft(id) if *flow.get() == ClientFlow::MapLibrary => {
            open_map_draft(
                &library,
                &catalog.0,
                &mut runtime,
                &mut commit,
                metrics.get().compact,
                id,
            );
        }
        FlowUiAction::SelectMapDraft(id) if *flow.get() == ClientFlow::MapLibrary => {
            if runtime.selected_draft == Some(id) && runtime.renaming.is_none() {
                // Choosing the selected tile again opens it.
                open_map_draft(
                    &library,
                    &catalog.0,
                    &mut runtime,
                    &mut commit,
                    metrics.get().compact,
                    id,
                );
            } else if library.drafts.contains_key(&id) {
                runtime.selected_draft = Some(id);
                if runtime
                    .renaming
                    .as_ref()
                    .is_some_and(|(renamed, _)| *renamed != id)
                {
                    runtime.renaming = None;
                }
                runtime.status = None;
                runtime.bump_render();
            }
        }
        FlowUiAction::BeginRenameMapDraft(id) if *flow.get() == ClientFlow::MapLibrary => {
            if let Some(document) = library.drafts.get(&id) {
                runtime.renaming = Some((id, document.name.clone()));
                runtime.selected_draft = Some(id);
                runtime.pending_delete = None;
                runtime.status = None;
                runtime.bump_render();
            }
        }
        FlowUiAction::ConfirmRenameMapDraft if *flow.get() == ClientFlow::MapLibrary => {
            if let Some((id, name)) = runtime.renaming.clone() {
                let result = library
                    .drafts
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| "map draft disappeared".to_string())
                    .and_then(|document| document.renamed(&name))
                    .and_then(|renamed| {
                        save_map_draft(&storage, &renamed)?;
                        library.insert(renamed)
                    });
                match result {
                    Ok(()) => {
                        runtime.renaming = None;
                        runtime.status = Some("Map renamed.".to_string());
                    }
                    Err(error) => runtime.status = Some(error),
                }
                runtime.bump_render();
            }
        }
        FlowUiAction::CancelRenameMapDraft if *flow.get() == ClientFlow::MapLibrary => {
            runtime.renaming = None;
            runtime.status = None;
            runtime.bump_render();
        }
        FlowUiAction::DuplicateMapDraft(id) if *flow.get() == ClientFlow::MapLibrary => {
            let result = library
                .drafts
                .get(&id)
                .cloned()
                .ok_or_else(|| "map draft disappeared".to_string())
                .and_then(|document| document.duplicate(library.next_id()))
                .and_then(|duplicate| {
                    save_map_draft(&storage, &duplicate)?;
                    let id = duplicate.id;
                    library.insert(duplicate).map(|()| id)
                });
            match result {
                Ok(id) => {
                    runtime.selected_draft = Some(id);
                    runtime.status = Some("Map duplicated.".to_string());
                }
                Err(error) => runtime.status = Some(error),
            }
            runtime.pending_delete = None;
            runtime.bump_render();
        }
        FlowUiAction::DeleteMapDraft(id) if *flow.get() == ClientFlow::MapLibrary => {
            if runtime.pending_delete == Some(id) {
                match delete_map_draft(&storage, id) {
                    Ok(()) => {
                        library.drafts.remove(&id);
                        runtime.status = Some("Map deleted.".to_string());
                    }
                    Err(error) => runtime.status = Some(error),
                }
                runtime.pending_delete = None;
            } else {
                // Opens the delete confirmation dialog.
                runtime.pending_delete = Some(id);
                runtime.renaming = None;
                runtime.status = None;
            }
            runtime.bump_render();
        }
        FlowUiAction::CancelDeleteMapDraft if *flow.get() == ClientFlow::MapLibrary => {
            runtime.pending_delete = None;
            runtime.bump_render();
        }
        FlowUiAction::PublishMapDraft(id) if *flow.get() == ClientFlow::MapLibrary => {
            runtime.pending_delete = None;
            runtime.status = Some(publish_draft(
                &memberships,
                &catalog.0,
                &library,
                id,
                &mut publish,
            ));
            runtime.bump_render();
        }
        action @ (FlowUiAction::ShowPublishedMaps(_)
        | FlowUiAction::SelectPublishedMap(_)
        | FlowUiAction::WithdrawPublishedMap(_)
        | FlowUiAction::CancelWithdrawPublishedMap)
            if *flow.get() == ClientFlow::MapLibrary =>
        {
            library::resolve_published_map_action(&action, &mut runtime, &mut published);
        }
        FlowUiAction::BackToMapLibrary if *flow.get() == ClientFlow::MapEditor => {
            if runtime
                .session
                .as_ref()
                .is_some_and(MapEditorSession::dirty)
            {
                // Value-only: the retained status line shows the warning.
                runtime.status = Some("Unsaved changes: Save before leaving, or use Clear/Undo to restore the saved revision.".to_string());
            } else {
                runtime.session = None;
                // Editor messages do not carry over into My Maps.
                runtime.status = None;
                commit.next_flow = Some(ClientFlow::MapLibrary);
            }
        }
        FlowUiAction::SaveMapDraft => save_open_document(&storage, &mut library, &mut runtime),
        FlowUiAction::TestAuthoredMap => test_open_document(
            &memberships,
            &catalog.0,
            &storage,
            &mut library,
            &mut runtime,
            &mut selection,
            &mut practice,
            &mut purpose,
        ),
        FlowUiAction::MapMakerUndo => {
            if let Some(session) = runtime.session.as_mut() {
                session.undo();
            }
            runtime.bump_render();
        }
        FlowUiAction::MapMakerRedo => {
            if let Some(session) = runtime.session.as_mut() {
                session.redo();
            }
            runtime.bump_render();
        }
        FlowUiAction::MapMakerClear => {
            if runtime.clear_armed {
                if let Some(session) = runtime.session.as_mut() {
                    let _ = session.clear();
                }
                runtime.clear_armed = false;
                runtime.status = Some("Map cleared. Undo is available.".to_string());
                runtime.bump_render();
            } else {
                // Value-only: arming changes the status line and button chrome.
                runtime.clear_armed = true;
                runtime.status =
                    Some("Press CLEAR again to remove all placements and anchors.".to_string());
            }
        }
        FlowUiAction::MapMakerCycleAsset(delta) => {
            cycle_palette(&catalog.0, &mut runtime, delta);
        }
        FlowUiAction::MapMakerSelectAsset(index) => {
            let count = runtime.session.as_ref().map_or(0, |session| {
                palette(&catalog.0, session.document.recipe.mode_definition_id).len()
            });
            if index < count {
                let before = palette_layout(&catalog.0, &runtime);
                runtime.palette_index = index;
                // Same-page selection is value-only: the retained selection marks move.
                if palette_layout(&catalog.0, &runtime) != before {
                    runtime.bump_render();
                }
            }
        }
        FlowUiAction::MapMakerCycleSymmetry => {
            if let Some(session) = runtime.session.as_mut() {
                session.symmetry = match session.symmetry {
                    MapSymmetry::None => MapSymmetry::LeftRight,
                    MapSymmetry::LeftRight => MapSymmetry::TopBottom,
                    MapSymmetry::TopBottom => MapSymmetry::Rotate180,
                    MapSymmetry::Rotate180 => MapSymmetry::FourWay,
                    MapSymmetry::FourWay => MapSymmetry::None,
                };
                let symmetry = session.symmetry;
                runtime.status = Some(format!("Mirror: {}.", symmetry_label(symmetry)));
            }
            // Value-only: the retained mirror label and status toast update in place.
        }
        FlowUiAction::MapMakerRotate => {
            if runtime.tool == EditorTool::Select && !runtime.selection.is_empty() {
                let selected = runtime.selection.clone();
                if let Some(session) = runtime.session.as_mut() {
                    runtime.status = session.rotate_placements(&catalog.0, &selected).err();
                }
                runtime.bump_render();
            } else {
                // Value-only: placement rotation shows in the 3D pointer preview, not the UI.
                runtime.quarter_turns = (runtime.quarter_turns + 1) % 4;
            }
        }
        FlowUiAction::MapMakerPlace => {
            place_current(&catalog.0, &mut runtime);
        }
        FlowUiAction::MapMakerPaintStroke(cells) => {
            paint_current(&catalog.0, &mut runtime, cells);
        }
        FlowUiAction::MapMakerErase => {
            let cursor = runtime.cursor;
            if let Some(session) = runtime.session.as_mut() {
                runtime.status = session.erase_at(&catalog.0, cursor).err();
            }
            runtime.bump_render();
        }
        FlowUiAction::MapMakerEraseStroke(cells) => {
            if let Some(session) = runtime.session.as_mut() {
                runtime.status = session.erase_cells(&catalog.0, cells).err();
            }
            runtime.bump_render();
        }
        FlowUiAction::MapMakerUsePlaceTool => {
            activate_editor_tool(&mut runtime, EditorTool::Place);
            runtime.bump_render();
        }
        FlowUiAction::MapMakerUseEraseTool => {
            activate_editor_tool(&mut runtime, EditorTool::Erase);
            runtime.bump_render();
        }
        FlowUiAction::MapMakerUseMoveTool => {
            activate_editor_tool(&mut runtime, EditorTool::Select);
            runtime.bump_render();
        }
        FlowUiAction::MapMakerSelectRect(first, second, additive) => {
            if let Some(session) = runtime.session.as_ref() {
                let selected = session.placements_in_rect(&catalog.0, first, second);
                let selected = session.symmetry_selection(&catalog.0, &selected);
                if additive {
                    runtime.selection.extend(selected);
                } else {
                    runtime.selection = selected;
                }
                runtime.status = None;
            }
            runtime.bump_render();
        }
        FlowUiAction::MapMakerMoveSelection(anchor, destination) => {
            let selected = runtime.selection.clone();
            if let Some(session) = runtime.session.as_mut() {
                runtime.status = session
                    .move_placements_by(
                        &catalog.0,
                        &selected,
                        anchor,
                        i32::from(destination.x) - i32::from(anchor.x),
                        i32::from(destination.y) - i32::from(anchor.y),
                    )
                    .err();
            }
            runtime.bump_render();
        }
        FlowUiAction::MapMakerDuplicateSelection => {
            let selected = runtime.selection.clone();
            if let Some(session) = runtime.session.as_mut() {
                match session.duplicate_placements(&catalog.0, &selected) {
                    Ok(duplicated) => {
                        runtime.selection = duplicated;
                        runtime.status = None;
                    }
                    Err(error) => runtime.status = Some(error),
                }
            }
            runtime.bump_render();
        }
        FlowUiAction::MapMakerDeleteSelection => {
            let selected = std::mem::take(&mut runtime.selection);
            if let Some(session) = runtime.session.as_mut() {
                runtime.status = session.delete_placements(&selected).err();
            }
            runtime.bump_render();
        }
        FlowUiAction::MapMakerClearSelection => {
            runtime.interaction = EditorInteraction::Idle;
            runtime.selection.clear();
            runtime.status = None;
            runtime.bump_render();
        }
        FlowUiAction::MapMakerSelectCategory(category) => {
            if let Some(category) = PaletteCategory::from_u8(category) {
                if runtime.tool != EditorTool::Place {
                    activate_editor_tool(&mut runtime, EditorTool::Place);
                }
                runtime.palette_category = category;
                runtime.category_menu_open = false;
                runtime.panel = SidePanel::Assets;
                select_first_visible_palette_item(&catalog.0, &mut runtime);
                runtime.bump_render();
            }
        }
        FlowUiAction::MapMakerShowPaletteItem(index) => {
            show_palette_item(&catalog.0, &mut runtime, index);
        }
        FlowUiAction::MapMakerToggleCategoryMenu => {
            runtime.category_menu_open = !runtime.category_menu_open;
            runtime.bump_render();
        }
        FlowUiAction::MapMakerToggleSearch => {
            let field_before = search_field_shown(&runtime);
            let panel_before = runtime.panel;
            runtime.palette_search_active = !runtime.palette_search_active;
            if runtime.palette_search_active {
                runtime.panel = SidePanel::Assets;
            }
            // Only a search field that appears or goes rebuilds; otherwise it retexts in place.
            if search_field_shown(&runtime) != field_before
                || runtime.panel != panel_before
                || runtime.category_menu_open
            {
                runtime.category_menu_open = false;
                runtime.bump_render();
            }
        }
        FlowUiAction::MapMakerClearSearch => {
            runtime.palette_query.clear();
            runtime.palette_search_active = false;
            select_first_visible_palette_item(&catalog.0, &mut runtime);
            runtime.bump_render();
        }
        FlowUiAction::MapMakerToggleAssets => {
            toggle_assets_drawer(&mut runtime);
            runtime.bump_render();
        }
        FlowUiAction::MapMakerToggleValidation => {
            runtime.panel = if runtime.panel == SidePanel::Check {
                SidePanel::Collapsed
            } else {
                SidePanel::Check
            };
            runtime.palette_search_active = false;
            runtime.bump_render();
        }
        FlowUiAction::MapMakerToggleGrid => {
            // Value-only: the retained grid label and button chrome update in place.
            runtime.grid_visible = !runtime.grid_visible;
        }
        FlowUiAction::MapMakerFitView => {
            runtime.camera_center = Vec2::ZERO;
            runtime.camera_zoom = 1.0;
        }
        FlowUiAction::MapMakerShowRuleIssue => show_rule_issue(&catalog.0, &mut runtime),
        FlowUiAction::MapMakerToggleChromeFocus if *flow.get() == ClientFlow::MapEditor => {
            runtime.chrome_focus = !runtime.chrome_focus;
            runtime.interaction = EditorInteraction::Idle;
            runtime.bump_render();
        }
        _ => {}
    }
}

fn is_map_maker_action(action: &FlowUiAction) -> bool {
    matches!(
        action,
        FlowUiAction::OpenMapMaker
            | FlowUiAction::CreateMap
            | FlowUiAction::ConfirmCreateMap
            | FlowUiAction::CancelCreateMap
            | FlowUiAction::OpenMapDraft(_)
            | FlowUiAction::SelectMapDraft(_)
            | FlowUiAction::CancelDeleteMapDraft
            | FlowUiAction::MapMakerShowPaletteItem(_)
            | FlowUiAction::MapMakerToggleCategoryMenu
            | FlowUiAction::BeginRenameMapDraft(_)
            | FlowUiAction::ConfirmRenameMapDraft
            | FlowUiAction::CancelRenameMapDraft
            | FlowUiAction::DuplicateMapDraft(_)
            | FlowUiAction::DeleteMapDraft(_)
            | FlowUiAction::PublishMapDraft(_)
            | FlowUiAction::ShowPublishedMaps(_)
            | FlowUiAction::SelectPublishedMap(_)
            | FlowUiAction::WithdrawPublishedMap(_)
            | FlowUiAction::CancelWithdrawPublishedMap
            | FlowUiAction::CloseMapMaker
            | FlowUiAction::BackToMapLibrary
            | FlowUiAction::SaveMapDraft
            | FlowUiAction::TestAuthoredMap
            | FlowUiAction::MapMakerUndo
            | FlowUiAction::MapMakerRedo
            | FlowUiAction::MapMakerClear
            | FlowUiAction::MapMakerCycleAsset(_)
            | FlowUiAction::MapMakerSelectAsset(_)
            | FlowUiAction::MapMakerCycleSymmetry
            | FlowUiAction::MapMakerRotate
            | FlowUiAction::MapMakerPlace
            | FlowUiAction::MapMakerPaintStroke(_)
            | FlowUiAction::MapMakerErase
            | FlowUiAction::MapMakerEraseStroke(_)
            | FlowUiAction::MapMakerUsePlaceTool
            | FlowUiAction::MapMakerUseEraseTool
            | FlowUiAction::MapMakerUseMoveTool
            | FlowUiAction::MapMakerSelectRect(_, _, _)
            | FlowUiAction::MapMakerMoveSelection(_, _)
            | FlowUiAction::MapMakerDuplicateSelection
            | FlowUiAction::MapMakerDeleteSelection
            | FlowUiAction::MapMakerClearSelection
            | FlowUiAction::MapMakerSelectCategory(_)
            | FlowUiAction::MapMakerToggleSearch
            | FlowUiAction::MapMakerClearSearch
            | FlowUiAction::MapMakerToggleAssets
            | FlowUiAction::MapMakerToggleValidation
            | FlowUiAction::MapMakerToggleGrid
            | FlowUiAction::MapMakerFitView
            | FlowUiAction::MapMakerShowRuleIssue
            | FlowUiAction::MapMakerToggleChromeFocus
    )
}

fn open_map_draft(
    library: &MapDraftLibrary,
    catalog: &brawler::map::MapContentCatalog,
    runtime: &mut MapMakerRuntime,
    commit: &mut FlowCommit,
    compact: bool,
    id: MapDraftId,
) {
    if let Some(document) = library.drafts.get(&id).cloned() {
        runtime.session = Some(MapEditorSession::open(document));
        runtime.selected_draft = Some(id);
        runtime.panel = SidePanel::opening(compact);
        runtime.category_menu_open = false;
        select_first_visible_palette_item(catalog, runtime);
        runtime.status = None;
        runtime.bump_render();
        commit.next_flow = Some(ClientFlow::MapEditor);
    }
}

impl SidePanel {
    /// The panel a freshly opened draft shows: Assets on desktop; collapsed on a phone, where
    /// the arena needs the room and the corner chip names the current asset.
    const fn opening(compact: bool) -> Self {
        if compact {
            Self::Collapsed
        } else {
            Self::Assets
        }
    }
}

/// What a new map gets from the connected server: the selected game's mode, and the size, theme
/// and ground of that game's first Practice preset. The create dialog shows exactly these.
pub(in crate::flow) struct CreateDefaults {
    game_type_id: brawler::lobby::GameTypeId,
    game: String,
    mode: brawler::map::ModeDefinitionId,
    dimensions: brawler::map::MapDimensions,
    theme_id: brawler::map::MapPresentationThemeId,
    theme: String,
    surface: MapAssetId,
}

fn create_defaults(
    membership: &ClientLobbyMembership,
    selection: &SelectedGameType,
    catalog: &brawler::map::MapContentCatalog,
) -> Result<CreateDefaults, String> {
    let game = membership
        .game_types
        .iter()
        .find(|game| {
            Some(&game.id) == selection.game_type_id.as_ref()
                && supported_mode(game.mode_definition_id)
        })
        .or_else(|| {
            membership
                .game_types
                .iter()
                .find(|game| supported_mode(game.mode_definition_id))
        })
        .ok_or_else(|| "This server does not advertise a Map Maker mode.".to_string())?;
    let preset = game
        .practice_map_preset_ids
        .first()
        .and_then(|id| catalog.preset(*id))
        .ok_or_else(|| "No compatible map defaults are available.".to_string())?;
    Ok(CreateDefaults {
        game_type_id: game.id.clone(),
        game: game.display_name.clone(),
        mode: game.mode_definition_id,
        dimensions: preset.recipe.dimensions,
        theme_id: preset.recipe.presentation_theme_id,
        theme: library::theme_name(catalog, preset.recipe.presentation_theme_id),
        surface: preset.recipe.default_surface_asset_id,
    })
}

fn create_default_document(
    memberships: &Query<&ClientLobbyMembership, With<Client>>,
    selection: &SelectedGameType,
    catalog: &brawler::map::MapContentCatalog,
    library: &mut MapDraftLibrary,
    runtime: &mut MapMakerRuntime,
    commit: &mut FlowCommit,
    name: &str,
) {
    let Some(membership) = memberships.iter().next() else {
        return;
    };
    let defaults = match create_defaults(membership, selection, catalog) {
        Ok(defaults) => defaults,
        Err(error) => {
            runtime.status = Some(error);
            return;
        }
    };
    let id = library.next_id();
    match MapDraftDocument::blank(
        id,
        name,
        defaults.mode,
        defaults.dimensions,
        defaults.theme_id,
        defaults.surface,
    ) {
        Ok(mut document) => {
            document.intended_game_type_id = Some(defaults.game_type_id);
            if let Err(error) = library.insert(document.clone()) {
                runtime.status = Some(error);
                return;
            }
            runtime.session = Some(MapEditorSession::open(document));
            runtime.creating = None;
            runtime.selected_draft = Some(id);
            runtime.cursor = MapCell::new(
                defaults.dimensions.width / 2,
                defaults.dimensions.height / 2,
            );
            select_first_visible_palette_item(catalog, runtime);
            runtime.status =
                Some("Draft created. Add spawns and the mode objective, then Test.".to_string());
            runtime.bump_render();
            commit.next_flow = Some(ClientFlow::MapEditor);
        }
        Err(error) => runtime.status = Some(error),
    }
}

/// Submit one saved draft to the connected server's community rotation (BRL-w9z1). The target is
/// the draft's intended game type when it takes community maps, otherwise the first advertised
/// game type of the draft's mode that does. Returns the status line to show.
fn publish_draft(
    memberships: &Query<&ClientLobbyMembership, With<Client>>,
    catalog: &brawler::map::MapContentCatalog,
    library: &MapDraftLibrary,
    id: MapDraftId,
    publish: &mut crate::ClientMapPublishModel,
) -> String {
    let Some(document) = library.drafts.get(&id) else {
        return "map draft disappeared".to_string();
    };
    let Some(membership) = memberships.iter().next() else {
        return "Connect to a server to publish.".to_string();
    };
    let mode = document.recipe.mode_definition_id;
    let game = membership
        .game_types
        .iter()
        .filter(|game| game.community_maps && game.mode_definition_id == mode)
        .find(|game| Some(&game.id) == document.intended_game_type_id.as_ref())
        .or_else(|| {
            membership
                .game_types
                .iter()
                .find(|game| game.community_maps && game.mode_definition_id == mode)
        });
    let Some(game) = game else {
        return "This server does not take community maps for this mode.".to_string();
    };
    if let Err(error) = brawler::map::validate_authored_recipe(
        &document.recipe,
        mode,
        catalog,
        brawler::map::MapInstanceId(1),
    ) {
        return error.to_string();
    }
    if publish.start(
        membership.catalog_revision,
        game,
        document.name.clone(),
        AuthoredMapCandidate {
            draft_id: document.id.0,
            recipe: document.recipe.clone(),
        },
    ) {
        format!("Publishing to {}…", game.display_name)
    } else {
        "A publish request is already in progress.".to_string()
    }
}

/// Show the server's publish decision on the library status line, and keep the author's
/// published-map listing current while My Maps is open (BRL-s3dh).
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(in crate::flow) fn observe_map_publish_outcome(
    flow: Res<State<ClientFlow>>,
    mut publish: ResMut<crate::ClientMapPublishModel>,
    mut published: ResMut<crate::ClientPublishedMapsModel>,
    mut runtime: ResMut<MapMakerRuntime>,
) {
    if let Some(decision) = publish.take_outcome() {
        if matches!(
            decision,
            brawler::lobby::MapPublishDecision::Accepted { .. }
        ) {
            // The new submission belongs in the Published list; ask once the reply is in.
            runtime.refresh_published = true;
        }
        runtime.status = Some(crate::map_publish_decision_text(decision));
        runtime.bump_render();
    }
    library::observe_published_maps(*flow.get(), &mut published, &mut runtime);
}

fn supported_mode(mode: brawler::map::ModeDefinitionId) -> bool {
    matches!(
        mode,
        brawler::map::WIPEOUT_MODE_DEFINITION
            | brawler::map::HOT_ZONE_MODE_DEFINITION
            | brawler::map::HEIST_MODE_DEFINITION
    )
}

fn save_open_document(
    storage: &MapDraftStorage,
    library: &mut MapDraftLibrary,
    runtime: &mut MapMakerRuntime,
) {
    let Some(session) = runtime.session.as_mut() else {
        return;
    };
    match save_map_draft(storage, &session.document)
        .and_then(|()| library.insert(session.document.clone()))
    {
        Ok(()) => {
            session.mark_saved();
            runtime.status = Some("Saved locally. Validation is revision-specific.".to_string());
        }
        Err(error) => runtime.status = Some(error),
    }
    runtime.bump_render();
}

#[allow(clippy::too_many_arguments)]
fn test_open_document(
    memberships: &Query<&ClientLobbyMembership, With<Client>>,
    catalog: &brawler::map::MapContentCatalog,
    storage: &MapDraftStorage,
    library: &mut MapDraftLibrary,
    runtime: &mut MapMakerRuntime,
    selection: &mut SelectedGameType,
    practice: &mut ClientPracticeModel,
    purpose: &mut SessionPurpose,
) {
    let Some(session) = runtime.session.as_mut() else {
        return;
    };
    let Some(membership) = memberships.iter().next() else {
        return;
    };
    let game = membership
        .game_types
        .iter()
        .find(|game| {
            game.mode_definition_id == session.document.recipe.mode_definition_id
                && Some(&game.id) == session.document.intended_game_type_id.as_ref()
        })
        .or_else(|| {
            membership
                .game_types
                .iter()
                .find(|game| game.mode_definition_id == session.document.recipe.mode_definition_id)
        });
    let Some(game) = game else {
        runtime.status = Some("The connected server no longer offers this mode.".to_string());
        return;
    };
    if brawler::map::validate_authored_recipe(
        &session.document.recipe,
        game.mode_definition_id,
        catalog,
        brawler::map::MapInstanceId(1),
    )
    .is_err()
    {
        // A refused Test points to Check, which lists what to fix.
        let issues = check::map_check(&session.document.recipe, catalog).issue_count();
        runtime.status = Some(match issues {
            0 | 1 => "Test needs one fix first. See Check.".to_string(),
            count => format!("Test needs {count} fixes first. See Check."),
        });
        runtime.panel = SidePanel::Check;
        runtime.palette_search_active = false;
        runtime.bump_render();
        return;
    }
    if let Err(error) = save_map_draft(storage, &session.document)
        .and_then(|()| library.insert(session.document.clone()))
    {
        runtime.status = Some(error);
        return;
    }
    session.mark_saved();
    selection.catalog_revision = Some(membership.catalog_revision);
    selection.game_type_id = Some(game.id.clone());
    selection.configuration_revision = Some(game.configuration_revision);
    practice.select_authored_map(
        game.id.clone(),
        AuthoredMapCandidate {
            draft_id: session.document.id.0,
            recipe: session.document.recipe.clone(),
        },
    );
    *purpose = SessionPurpose::Practice;
    if practice.start(selection, membership.profile.revision) {
        runtime.return_to_editor = true;
        runtime.status = Some("Submitting this saved revision to the server…".to_string());
    }
    runtime.bump_render();
}

impl MapMakerRuntime {
    fn bump_render(&mut self) {
        self.render_revision = self.render_revision.wrapping_add(1).max(1);
    }

    pub(in crate::flow) fn action_is_current(&self, action: &FlowUiAction) -> bool {
        match action {
            FlowUiAction::MapMakerSelectAsset(index) => self.palette_index == *index,
            FlowUiAction::MapMakerUsePlaceTool => self.tool == EditorTool::Place,
            FlowUiAction::MapMakerUseEraseTool => self.tool == EditorTool::Erase,
            FlowUiAction::MapMakerUseMoveTool => self.tool == EditorTool::Select,
            FlowUiAction::MapMakerSelectCategory(category) => {
                PaletteCategory::from_u8(*category) == Some(self.palette_category)
            }
            FlowUiAction::MapMakerToggleSearch => self.palette_search_active,
            FlowUiAction::MapMakerToggleAssets => self.panel == SidePanel::Assets,
            FlowUiAction::MapMakerToggleValidation => self.panel == SidePanel::Check,
            FlowUiAction::MapMakerToggleCategoryMenu => self.category_menu_open,
            FlowUiAction::SelectMapDraft(id) => self.selected_draft == Some(*id),
            FlowUiAction::SelectPublishedMap(id) => self.selected_published == Some(*id),
            FlowUiAction::ShowPublishedMaps(published) => self.published_view == *published,
            FlowUiAction::MapMakerCycleSymmetry => self
                .session
                .as_ref()
                .is_some_and(|session| session.symmetry != MapSymmetry::None),
            FlowUiAction::MapMakerToggleGrid => self.grid_visible,
            FlowUiAction::MapMakerClear => self.clear_armed,
            FlowUiAction::MapMakerToggleChromeFocus => self.chrome_focus,
            _ => false,
        }
    }
}

/// Switches tools. Select and Erase have no assets to choose, so an open Assets panel collapses
/// for them; the Check panel stays as it was.
fn activate_editor_tool(runtime: &mut MapMakerRuntime, tool: EditorTool) {
    runtime.tool = tool;
    runtime.interaction = EditorInteraction::Idle;
    if tool != EditorTool::Place {
        runtime.palette_search_active = false;
        runtime.category_menu_open = false;
        if runtime.panel == SidePanel::Assets {
            runtime.panel = SidePanel::Collapsed;
        }
    }
    if tool != EditorTool::Select {
        runtime.selection.clear();
    }
}

/// Check's Show (BRL-855z): the Select tool with the placements a located rule refusal names,
/// and the camera framed on its cells. Placements the editor cannot select by ID (cells of a
/// filled rectangle) are selected by their cells instead.
fn show_rule_issue(catalog: &brawler::map::MapContentCatalog, runtime: &mut MapMakerRuntime) {
    let Some(session) = runtime.session.as_ref() else {
        return;
    };
    let recipe = &session.document.recipe;
    let Some(issue) = check::map_check(recipe, catalog)
        .rule_error
        .filter(check::RuleIssue::located)
    else {
        return;
    };
    let selectable: BTreeSet<_> = recipe
        .placements
        .iter()
        .map(|placement| placement.placement_id)
        .chain(recipe.mode_anchors.iter().map(|anchor| anchor.placement_id))
        .collect();
    let mut selected: BTreeSet<_> = issue
        .placements
        .iter()
        .copied()
        .filter(|id| selectable.contains(id))
        .collect();
    if selected.len() < issue.placements.len() || selected.is_empty() {
        for cell in &issue.cells {
            selected.extend(session.placements_in_rect(catalog, *cell, *cell));
        }
    }
    let dimensions = recipe.dimensions;
    let mut cells = issue.cells.clone();
    cells.extend(recipe.placements.iter().filter_map(|placement| {
        selected
            .contains(&placement.placement_id)
            .then_some(placement.cell)
    }));
    activate_editor_tool(runtime, EditorTool::Select);
    runtime.selection = selected;
    if let Some((center, zoom)) = framing(dimensions, &cells) {
        runtime.camera_center = center;
        runtime.camera_zoom = zoom;
    }
    runtime.status = Some(issue.text);
    runtime.bump_render();
}

/// The camera centre and zoom that frame `cells` with a margin; zoom 1 fits the whole map.
fn framing(dimensions: brawler::map::MapDimensions, cells: &[MapCell]) -> Option<(Vec2, f32)> {
    let mut points = cells.iter().map(|cell| dimensions.cell_center(*cell));
    let first = points.next()?;
    let (min, max) = points.fold((first, first), |(min, max), point| {
        (min.min(point), max.max(point))
    });
    let margin = Vec2::splat(brawler::map::MAP_CELL_SIZE_WORLD * 6.0);
    let extent = max - min + margin;
    let world = dimensions.world_size();
    let zoom = (extent.x / world.x).max(extent.y / world.y).clamp(0.5, 1.0);
    Some(((min + max) * 0.5, zoom))
}

/// The Assets tab: opens it (with the Place tool) or collapses it.
fn toggle_assets_drawer(runtime: &mut MapMakerRuntime) {
    if runtime.tool == EditorTool::Place && runtime.panel == SidePanel::Assets {
        runtime.panel = SidePanel::Collapsed;
        runtime.palette_search_active = false;
        runtime.category_menu_open = false;
    } else {
        if runtime.tool != EditorTool::Place {
            activate_editor_tool(runtime, EditorTool::Place);
        }
        runtime.panel = SidePanel::Assets;
    }
}

/// Check's Add: the Assets panel on the palette entry that fixes an issue.
fn show_palette_item(
    catalog: &brawler::map::MapContentCatalog,
    runtime: &mut MapMakerRuntime,
    index: usize,
) {
    let Some(item) = runtime.session.as_ref().and_then(|session| {
        palette(catalog, session.document.recipe.mode_definition_id)
            .get(index)
            .copied()
    }) else {
        return;
    };
    if runtime.tool != EditorTool::Place {
        activate_editor_tool(runtime, EditorTool::Place);
    }
    runtime.palette_category = palette_category(item, catalog);
    runtime.palette_query.clear();
    runtime.palette_search_active = false;
    runtime.category_menu_open = false;
    runtime.palette_index = index;
    runtime.panel = SidePanel::Assets;
    runtime.status = Some(format!(
        "{} selected. Tap the map to place it.",
        palette_label(Some(item), catalog)
    ));
    runtime.bump_render();
}

fn palette(
    catalog: &brawler::map::MapContentCatalog,
    mode: brawler::map::ModeDefinitionId,
) -> Vec<PaletteItem> {
    let mut items = authorable_assets(catalog)
        .into_iter()
        .map(|asset| PaletteItem::Asset(asset.id))
        .collect::<Vec<_>>();
    items.extend([PaletteItem::Spawn(0), PaletteItem::Spawn(1)]);
    if mode == brawler::map::HOT_ZONE_MODE_DEFINITION {
        items.push(PaletteItem::HotZone);
    }
    if mode == brawler::map::HEIST_MODE_DEFINITION {
        items.extend([PaletteItem::HeistSafe(0), PaletteItem::HeistSafe(1)]);
    }
    items
}

fn cycle_palette(
    catalog: &brawler::map::MapContentCatalog,
    runtime: &mut MapMakerRuntime,
    delta: i8,
) {
    let Some(session) = runtime.session.as_ref() else {
        return;
    };
    let visible =
        visible_palette_entries(catalog, runtime, session.document.recipe.mode_definition_id);
    let before = palette_layout(catalog, runtime);
    if !visible.is_empty() {
        let current = visible
            .iter()
            .position(|(index, _)| *index == runtime.palette_index)
            .unwrap_or(0);
        let next = cycled_palette_index(current, visible.len(), delta);
        runtime.palette_index = visible[next].0;
    }
    // The list is unchanged, so the selection moves in place without a rebuild.
    if palette_layout(catalog, runtime) != before {
        runtime.bump_render();
    }
}

fn select_first_visible_palette_item(
    catalog: &brawler::map::MapContentCatalog,
    runtime: &mut MapMakerRuntime,
) {
    let Some(session) = runtime.session.as_ref() else {
        return;
    };
    if let Some((index, _)) =
        visible_palette_entries(catalog, runtime, session.document.recipe.mode_definition_id)
            .first()
    {
        runtime.palette_index = *index;
    }
}

fn visible_palette_entries(
    catalog: &brawler::map::MapContentCatalog,
    runtime: &MapMakerRuntime,
    mode: brawler::map::ModeDefinitionId,
) -> Vec<(usize, PaletteItem)> {
    let query = runtime.palette_query.trim().to_lowercase();
    let mut entries = palette(catalog, mode)
        .into_iter()
        .enumerate()
        .filter(|(_, item)| palette_category(*item, catalog) == runtime.palette_category)
        .filter(|(_, item)| {
            query.is_empty()
                || palette_label(Some(*item), catalog)
                    .to_lowercase()
                    .contains(&query)
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|(_, item)| palette_label(Some(*item), catalog).to_lowercase());
    entries
}

/// What the asset list's spawned structure depends on besides tool and panel state: the visible
/// rows and whether the search field exists. The selected row is live control state, not
/// structure.
#[derive(PartialEq, Eq)]
struct PaletteLayout {
    visible: Vec<usize>,
    query_empty: bool,
}

fn palette_layout(
    catalog: &brawler::map::MapContentCatalog,
    runtime: &MapMakerRuntime,
) -> Option<PaletteLayout> {
    let session = runtime.session.as_ref()?;
    let visible =
        visible_palette_entries(catalog, runtime, session.document.recipe.mode_definition_id)
            .into_iter()
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
    Some(PaletteLayout {
        visible,
        query_empty: runtime.palette_query.is_empty(),
    })
}

fn selected_palette_item(
    catalog: &brawler::map::MapContentCatalog,
    runtime: &MapMakerRuntime,
    mode: brawler::map::ModeDefinitionId,
) -> Option<PaletteItem> {
    visible_palette_entries(catalog, runtime, mode)
        .into_iter()
        .find_map(|(index, item)| (index == runtime.palette_index).then_some(item))
}

fn palette_category(
    item: PaletteItem,
    catalog: &brawler::map::MapContentCatalog,
) -> PaletteCategory {
    let PaletteItem::Asset(id) = item else {
        return PaletteCategory::SpawnsObjectives;
    };
    let Some(asset) = catalog.asset(id) else {
        return PaletteCategory::CoverObjects;
    };
    match asset.slot {
        MapAssetSlot::Surface => PaletteCategory::Ground,
        MapAssetSlot::Decoration | MapAssetSlot::Marker => PaletteCategory::Decorations,
        MapAssetSlot::Feature => {
            let Some(profile) = catalog.profile(asset.gameplay_profile_id) else {
                return PaletteCategory::CoverObjects;
            };
            if profile.effect_tile_definition_id.is_some() {
                PaletteCategory::EffectTiles
            } else if profile.concealment == brawler::map::MapConcealmentBehavior::HideOccupants {
                PaletteCategory::Vegetation
            } else {
                PaletteCategory::CoverObjects
            }
        }
    }
}

fn cycled_palette_index(current: usize, count: usize, delta: i8) -> usize {
    if count == 0 {
        return 0;
    }
    let amount = usize::from(delta.unsigned_abs()) % count;
    if delta.is_negative() {
        (current + count - amount) % count
    } else {
        (current + amount) % count
    }
}

fn place_current(catalog: &brawler::map::MapContentCatalog, runtime: &mut MapMakerRuntime) {
    let cursor = runtime.cursor;
    let rotation = runtime.quarter_turns;
    let Some(mode) = runtime
        .session
        .as_ref()
        .map(|session| session.document.recipe.mode_definition_id)
    else {
        return;
    };
    let Some(item) = selected_palette_item(catalog, runtime, mode) else {
        runtime.status = Some("Choose an asset from the visible palette first.".to_string());
        return;
    };
    let Some(session) = runtime.session.as_mut() else {
        return;
    };
    let result = match item {
        PaletteItem::Asset(id) => session.place(catalog, id, cursor, rotation),
        PaletteItem::Spawn(team) => session.place_spawn(catalog, cursor, team, rotation),
        PaletteItem::HotZone => session.place_hot_zone(cursor),
        PaletteItem::HeistSafe(team) => session.place_heist_safe(cursor, team),
    };
    runtime.status = result.err();
    runtime.clear_armed = false;
    runtime.bump_render();
}

fn paint_current(
    catalog: &brawler::map::MapContentCatalog,
    runtime: &mut MapMakerRuntime,
    cells: Vec<MapCell>,
) {
    let rotation = runtime.quarter_turns;
    let Some(mode) = runtime
        .session
        .as_ref()
        .map(|session| session.document.recipe.mode_definition_id)
    else {
        return;
    };
    let Some(PaletteItem::Asset(asset_id)) = selected_palette_item(catalog, runtime, mode) else {
        runtime.status =
            Some("Drag painting is available for visible map assets only.".to_string());
        return;
    };
    let Some(session) = runtime.session.as_mut() else {
        return;
    };
    runtime.status = session
        .paint_stroke(catalog, asset_id, cells, rotation)
        .err();
    runtime.clear_armed = false;
    runtime.bump_render();
}

/// Which Map Maker name field a keyed Enter confirmed this frame (BRL-yrys).
#[derive(Clone, Copy, Debug)]
pub(in crate::flow) enum KeyedMapNameCommit {
    Create,
    Rename,
}

/// Applies typed edits to a new or renamed map's name: Backspace removes the last character and
/// inserted characters stop at the authored name bound.
fn edit_map_name(name: &mut String, edits: &[TextEdit]) {
    for edit in edits {
        match edit {
            TextEdit::Backspace => {
                name.pop();
            }
            TextEdit::Insert(text) => {
                for character in text.chars().filter(|character| !character.is_control()) {
                    if name.len() + character.len_utf8()
                        <= brawler::map::authoring::MAX_AUTHORED_MAP_NAME_BYTES
                    {
                        name.push(character);
                    }
                }
            }
            TextEdit::Delete => {}
        }
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity
)]
pub(in crate::flow) fn map_maker_input(
    flow: Res<State<ClientFlow>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    back: Res<crate::BackInput>,
    mut typed: crate::text_entry::TypedText,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    mouse_motion: Option<Res<AccumulatedMouseMotion>>,
    mut mouse_wheel: MessageReader<MouseWheel>,
    gamepads: Query<&Gamepad>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<ArenaCamera>>,
    catalog: Option<Res<MapCatalogResource>>,
    // Paired to stay within Bevy's system parameter limit: Map Maker controls and the chrome
    // under the pointer.
    (buttons, chrome): (
        Query<(
            &FlowButton,
            &Interaction,
            &crate::ui::press::ReleaseActivation,
        )>,
        Query<&Interaction, With<editor::MapMakerChrome>>,
    ),
    mut runtime: ResMut<MapMakerRuntime>,
    mut actions: ResMut<PendingFlowActions>,
    mut feedback: MessageWriter<crate::ui::feedback::UiFeedback>,
    mut name_commit: Local<Option<KeyedMapNameCommit>>,
) {
    use crate::ui::{feedback::UiFeedback, surface::Surface};
    // A keyed create/rename confirm resolves after this system; it sounds on the next frame only
    // when the name field actually closed, so a failed commit plays nothing (BRL-yrys).
    if let Some(commit) = name_commit.take() {
        let committed = match commit {
            KeyedMapNameCommit::Create => runtime.creating.is_none(),
            KeyedMapNameCommit::Rename => runtime.renaming.is_none(),
        };
        if committed {
            feedback.write(UiFeedback::Activated(Some(Surface::Primary)));
        }
    }
    if !matches!(*flow.get(), ClientFlow::MapLibrary | ClientFlow::MapEditor) {
        return;
    }
    // The platform Back request cancels exactly where Escape does.
    let escape = keyboard.just_pressed(KeyCode::Escape) || back.just_pressed();
    let mut ui_activated = false;
    // Release activation also keeps a button rebuilt under a held pointer from firing: it never
    // observed that press.
    for (button, _, release) in &buttons {
        if is_map_maker_action(&button.action) && release.activated() {
            queue_ui_action(&mut actions, button.action.clone());
            ui_activated = true;
        }
    }
    if *flow.get() == ClientFlow::MapLibrary {
        if runtime.pending_delete.is_some() {
            if escape
                || gamepads
                    .iter()
                    .any(|pad| pad.just_pressed(GamepadButton::East))
            {
                queue_ui_action(&mut actions, FlowUiAction::CancelDeleteMapDraft);
                feedback.write(UiFeedback::Back);
            }
            return;
        }
        if let Some(name) = runtime.creating.as_mut() {
            edit_map_name(name, &typed.edits());
            if keyboard.just_pressed(KeyCode::Enter) {
                queue_ui_action(&mut actions, FlowUiAction::ConfirmCreateMap);
                *name_commit = Some(KeyedMapNameCommit::Create);
            } else if escape {
                queue_ui_action(&mut actions, FlowUiAction::CancelCreateMap);
                feedback.write(UiFeedback::Back);
            }
            return;
        }
        if let Some((_, name)) = runtime.renaming.as_mut() {
            edit_map_name(name, &typed.edits());
            if keyboard.just_pressed(KeyCode::Enter) {
                queue_ui_action(&mut actions, FlowUiAction::ConfirmRenameMapDraft);
                *name_commit = Some(KeyedMapNameCommit::Rename);
            } else if escape {
                queue_ui_action(&mut actions, FlowUiAction::CancelRenameMapDraft);
                feedback.write(UiFeedback::Back);
            }
            return;
        }
        if escape
            || gamepads
                .iter()
                .any(|pad| pad.just_pressed(GamepadButton::East))
        {
            queue_ui_action(&mut actions, FlowUiAction::CloseMapMaker);
            feedback.write(UiFeedback::Back);
        }
        return;
    }
    if runtime.palette_search_active {
        let palette_before = catalog
            .as_deref()
            .map(|catalog| palette_layout(&catalog.0, &runtime));
        let mut changed = false;
        for edit in typed.edits() {
            match edit {
                // The Slash press that opened search is read on the next frame.
                TextEdit::Insert(text) if text == "/" && runtime.palette_query.is_empty() => {}
                TextEdit::Backspace => changed |= runtime.palette_query.pop().is_some(),
                TextEdit::Insert(text) => {
                    for character in text.chars().filter(|character| !character.is_control()) {
                        if runtime.palette_query.len() + character.len_utf8() <= 48 {
                            runtime.palette_query.push(character);
                            changed = true;
                        }
                    }
                }
                TextEdit::Delete => {}
            }
        }
        if keyboard.just_pressed(KeyCode::Enter) {
            runtime.palette_search_active = false;
            changed = true;
            feedback.write(UiFeedback::Activated(Some(Surface::Primary)));
        } else if escape {
            runtime.palette_search_active = false;
            changed = true;
            feedback.write(UiFeedback::Back);
        }
        if changed {
            let palette_after = catalog.as_deref().map(|catalog| {
                select_first_visible_palette_item(&catalog.0, &mut runtime);
                palette_layout(&catalog.0, &runtime)
            });
            // Only a changed card set or selection rebuilds; otherwise the retained search
            // label updates in place.
            if palette_after != palette_before {
                runtime.bump_render();
            }
        }
        return;
    }
    let pad = |button| gamepads.iter().any(|gamepad| gamepad.just_pressed(button));
    if keyboard.just_pressed(focus::CHROME_FOCUS_KEY) || pad(focus::CHROME_FOCUS_BUTTON) {
        queue_ui_action(&mut actions, FlowUiAction::MapMakerToggleChromeFocus);
        feedback.write(UiFeedback::Activated(None));
        return;
    }
    // Chrome focus owns the arrows, D-pad, Enter, Space and South (`navigate_map_editor`); Esc
    // and East close the category menu, then return to the map cursor.
    let chrome_mode = runtime.chrome_focus;
    if chrome_mode && (escape || pad(GamepadButton::East)) {
        if runtime.category_menu_open {
            runtime.category_menu_open = false;
            runtime.bump_render();
        } else {
            queue_ui_action(&mut actions, FlowUiAction::MapMakerToggleChromeFocus);
        }
        feedback.write(UiFeedback::Back);
        return;
    }
    if keyboard.just_pressed(KeyCode::Slash) {
        activate_editor_tool(&mut runtime, EditorTool::Place);
        runtime.palette_search_active = true;
        runtime.bump_render();
        feedback.write(UiFeedback::Activated(None));
        return;
    }
    let dimensions = runtime
        .session
        .as_ref()
        .map(|session| session.document.recipe.dimensions);
    let world_cursor = windows
        .iter()
        .next()
        .and_then(Window::cursor_position)
        .and_then(|cursor| {
            cameras.iter().next().and_then(|(camera, transform)| {
                crate::presentation_3d::cursor_ground_point(camera, transform, cursor)
            })
        });
    if let (Some(dimensions), Some(world)) = (dimensions, world_cursor)
        && let Some(cell) = dimensions.world_cell(world)
    {
        runtime.cursor = cell;
    }
    // Keyed cursor moves tick only when the cursor really moved: a grid edge is silent.
    let cursor_before = runtime.cursor;
    if let Some(dimensions) = dimensions.filter(|_| !chrome_mode) {
        if keyboard.just_pressed(KeyCode::ArrowLeft) || pad(GamepadButton::DPadLeft) {
            runtime.cursor.x = runtime.cursor.x.saturating_sub(1);
        }
        if keyboard.just_pressed(KeyCode::ArrowRight) || pad(GamepadButton::DPadRight) {
            runtime.cursor.x = runtime.cursor.x.saturating_add(1).min(dimensions.width - 1);
        }
        if keyboard.just_pressed(KeyCode::ArrowUp) || pad(GamepadButton::DPadUp) {
            runtime.cursor.y = runtime.cursor.y.saturating_sub(1);
        }
        if keyboard.just_pressed(KeyCode::ArrowDown) || pad(GamepadButton::DPadDown) {
            runtime.cursor.y = runtime
                .cursor
                .y
                .saturating_add(1)
                .min(dimensions.height - 1);
        }
    }
    if runtime.cursor != cursor_before {
        feedback.write(UiFeedback::Moved);
    }
    // Keyboard/gamepad shortcuts click like the toolbar control they stand for; pointer strokes
    // and wheel cycling stay silent, as before.
    let mut keyed = false;
    if keyboard.just_pressed(KeyCode::Tab) || pad(GamepadButton::RightTrigger2) {
        queue_ui_action(&mut actions, FlowUiAction::MapMakerCycleAsset(1));
        keyed = true;
    }
    if pad(GamepadButton::LeftTrigger2) {
        queue_ui_action(&mut actions, FlowUiAction::MapMakerCycleAsset(-1));
        keyed = true;
    }
    if keyboard.just_pressed(KeyCode::KeyR) || pad(GamepadButton::West) {
        queue_ui_action(&mut actions, FlowUiAction::MapMakerRotate);
        keyed = true;
    }
    if keyboard.just_pressed(KeyCode::KeyM) || pad(GamepadButton::North) {
        queue_ui_action(&mut actions, FlowUiAction::MapMakerCycleSymmetry);
        keyed = true;
    }
    if keyboard.just_pressed(KeyCode::KeyP) {
        queue_ui_action(&mut actions, FlowUiAction::MapMakerUsePlaceTool);
        keyed = true;
    }
    if keyboard.just_pressed(KeyCode::KeyE) {
        queue_ui_action(&mut actions, FlowUiAction::MapMakerUseEraseTool);
        keyed = true;
    }
    if keyboard.just_pressed(KeyCode::KeyV) {
        queue_ui_action(&mut actions, FlowUiAction::MapMakerUseMoveTool);
        keyed = true;
    }
    let ui_owns_pointer = ui_activated
        || buttons
            .iter()
            .map(|(_, interaction, _)| interaction)
            .chain(&chrome)
            .any(|interaction| matches!(*interaction, Interaction::Hovered | Interaction::Pressed));
    let pointer_over_world = world_cursor
        .zip(dimensions)
        .is_some_and(|(world, dimensions)| dimensions.world_cell(world).is_some());
    let mouse_just_pressed = mouse
        .as_deref()
        .is_some_and(|mouse| mouse.just_pressed(MouseButton::Left));
    let mouse_pressed = mouse
        .as_deref()
        .is_some_and(|mouse| mouse.pressed(MouseButton::Left));
    let mouse_just_released = mouse
        .as_deref()
        .is_some_and(|mouse| mouse.just_released(MouseButton::Left));
    let panning = mouse.as_deref().is_some_and(|mouse| {
        mouse.pressed(MouseButton::Middle)
            || (keyboard.pressed(KeyCode::Space) && mouse.pressed(MouseButton::Left))
    });
    if panning {
        if let Some(motion) = mouse_motion.as_deref() {
            let pan_scale = runtime.camera_zoom * 1.5;
            runtime.camera_center += Vec2::new(-motion.delta.x, -motion.delta.y) * pan_scale;
            if let Some(dimensions) = dimensions {
                let half = dimensions.world_size() * 0.5;
                runtime.camera_center = runtime.camera_center.clamp(-half, half);
            }
        }
        runtime.interaction = EditorInteraction::Idle;
    }
    // Over the chrome the wheel scrolls its lists (`scroll_map_maker`); over the arena it zooms.
    for event in mouse_wheel.read() {
        if !ui_owns_pointer {
            runtime.camera_zoom = (runtime.camera_zoom * (-event.y * 0.10).exp()).clamp(0.5, 3.0);
        }
    }
    if mouse_just_pressed && pointer_over_world && !ui_owns_pointer && !panning {
        let cursor = runtime.cursor;
        match runtime.tool {
            EditorTool::Place => {
                let asset = runtime
                    .session
                    .as_ref()
                    .zip(catalog.as_deref())
                    .and_then(|(session, catalog)| {
                        selected_palette_item(
                            &catalog.0,
                            &runtime,
                            session.document.recipe.mode_definition_id,
                        )
                    })
                    .and_then(|item| match item {
                        PaletteItem::Asset(id) => Some(id),
                        _ => None,
                    });
                runtime.interaction = EditorInteraction::Paint {
                    cells: vec![cursor],
                    asset,
                };
            }
            EditorTool::Erase => {
                runtime.interaction = EditorInteraction::Erase {
                    cells: vec![cursor],
                };
            }
            EditorTool::Select => {
                let clicked_selected = runtime
                    .session
                    .as_ref()
                    .zip(catalog.as_deref())
                    .is_some_and(|(session, catalog)| {
                        session
                            .placements_in_rect(&catalog.0, cursor, cursor)
                            .iter()
                            .any(|id| runtime.selection.contains(id))
                    });
                if clicked_selected {
                    runtime.interaction = EditorInteraction::MoveSelection { anchor: cursor };
                } else {
                    runtime.interaction = EditorInteraction::MarqueeSelect {
                        anchor: cursor,
                        additive: keyboard.pressed(KeyCode::ShiftLeft)
                            || keyboard.pressed(KeyCode::ShiftRight),
                    };
                }
            }
        }
    } else if mouse_pressed && pointer_over_world && !ui_owns_pointer && !panning {
        let cursor = runtime.cursor;
        match &mut runtime.interaction {
            EditorInteraction::Paint {
                cells,
                asset: Some(_),
            }
            | EditorInteraction::Erase { cells } => append_interpolated_cells(cells, cursor),
            _ => {}
        }
    }
    if mouse_just_released && !panning {
        match std::mem::take(&mut runtime.interaction) {
            EditorInteraction::Paint { cells, asset } if !cells.is_empty() => {
                if asset.is_some() {
                    let stroke = cells;
                    queue_ui_action(&mut actions, FlowUiAction::MapMakerPaintStroke(stroke));
                } else {
                    queue_ui_action(&mut actions, FlowUiAction::MapMakerPlace);
                }
            }
            EditorInteraction::Erase { cells } if !cells.is_empty() => {
                queue_ui_action(&mut actions, FlowUiAction::MapMakerEraseStroke(cells));
            }
            EditorInteraction::MoveSelection { anchor } => queue_ui_action(
                &mut actions,
                FlowUiAction::MapMakerMoveSelection(anchor, runtime.cursor),
            ),
            EditorInteraction::MarqueeSelect { anchor, additive } => {
                queue_ui_action(
                    &mut actions,
                    FlowUiAction::MapMakerSelectRect(anchor, runtime.cursor, additive),
                );
            }
            _ => {}
        }
    }
    if !chrome_mode
        && (keyboard.just_pressed(KeyCode::Enter)
            || (keyboard.just_pressed(KeyCode::Space) && !mouse_pressed)
            || pad(GamepadButton::South))
    {
        let action = match runtime.tool {
            EditorTool::Place => Some(FlowUiAction::MapMakerPlace),
            EditorTool::Erase => Some(FlowUiAction::MapMakerErase),
            EditorTool::Select => None,
        };
        if let Some(action) = action {
            queue_ui_action(&mut actions, action);
            keyed = true;
        }
    }
    if keyboard.just_pressed(KeyCode::Delete)
        || keyboard.just_pressed(KeyCode::Backspace)
        || pad(GamepadButton::RightTrigger)
    {
        if runtime.tool == EditorTool::Select && !runtime.selection.is_empty() {
            queue_ui_action(&mut actions, FlowUiAction::MapMakerDeleteSelection);
        } else {
            queue_ui_action(&mut actions, FlowUiAction::MapMakerErase);
        }
        keyed = true;
    }
    for (key, action) in [
        (KeyCode::KeyZ, FlowUiAction::MapMakerUndo),
        (KeyCode::KeyY, FlowUiAction::MapMakerRedo),
        (KeyCode::KeyS, FlowUiAction::SaveMapDraft),
        (KeyCode::KeyT, FlowUiAction::TestAuthoredMap),
        (KeyCode::KeyG, FlowUiAction::MapMakerToggleGrid),
        (KeyCode::KeyC, FlowUiAction::MapMakerClear),
        (KeyCode::KeyF, FlowUiAction::MapMakerFitView),
    ] {
        if keyboard.just_pressed(key) {
            queue_ui_action(&mut actions, action);
            keyed = true;
        }
    }
    // One click per frame however many shortcuts landed together.
    if keyed {
        feedback.write(UiFeedback::Activated(None));
    }
    if escape || pad(GamepadButton::East) {
        if runtime.category_menu_open || runtime.panel == SidePanel::Check {
            runtime.category_menu_open = false;
            if runtime.panel == SidePanel::Check {
                runtime.panel = SidePanel::Collapsed;
            }
            runtime.bump_render();
            feedback.write(UiFeedback::Back);
        } else if runtime.interaction != EditorInteraction::Idle || !runtime.selection.is_empty() {
            runtime.interaction = EditorInteraction::Idle;
            runtime.selection.clear();
            runtime.status = None;
            runtime.bump_render();
            feedback.write(UiFeedback::Back);
        } else {
            queue_ui_action(&mut actions, FlowUiAction::BackToMapLibrary);
            feedback.write(UiFeedback::Back);
        }
    }
}

fn append_interpolated_cells(cells: &mut Vec<MapCell>, destination: MapCell) {
    let Some(start) = cells.last().copied() else {
        cells.push(destination);
        return;
    };
    if start == destination {
        return;
    }
    let mut x = i32::from(start.x);
    let mut y = i32::from(start.y);
    let target_x = i32::from(destination.x);
    let target_y = i32::from(destination.y);
    let delta_x = (target_x - x).abs();
    let step_x = (target_x - x).signum();
    let delta_y = -(target_y - y).abs();
    let step_y = (target_y - y).signum();
    let mut error = delta_x + delta_y;
    while x != target_x || y != target_y {
        let doubled = error * 2;
        if doubled >= delta_y {
            error += delta_y;
            x += step_x;
        }
        if doubled <= delta_x {
            error += delta_x;
            y += step_y;
        }
        let Ok(cell_x) = u16::try_from(x) else {
            return;
        };
        let Ok(cell_y) = u16::try_from(y) else {
            return;
        };
        let cell = MapCell::new(cell_x, cell_y);
        if cells.last() != Some(&cell) {
            cells.push(cell);
        }
    }
}

/// The Assets tab shows its search field while a search is typed or applied.
fn search_field_shown(runtime: &MapMakerRuntime) -> bool {
    runtime.palette_search_active || !runtime.palette_query.is_empty()
}

fn status_display(runtime: &MapMakerRuntime) -> Display {
    if runtime.status.is_some() {
        Display::Flex
    } else {
        Display::None
    }
}

/// The asset search field's text: the query, with a caret while it is being typed.
fn search_label(runtime: &MapMakerRuntime) -> String {
    if runtime.palette_query.is_empty() && !runtime.palette_search_active {
        "Search assets".to_string()
    } else if runtime.palette_search_active {
        format!("{}|", runtime.palette_query)
    } else {
        runtime.palette_query.clone()
    }
}

const fn symmetry_label(symmetry: MapSymmetry) -> &'static str {
    match symmetry {
        MapSymmetry::None => "off",
        MapSymmetry::LeftRight => "left / right",
        MapSymmetry::TopBottom => "top / bottom",
        MapSymmetry::Rotate180 => "rotate 180°",
        MapSymmetry::FourWay => "four-way",
    }
}

fn palette_label(item: Option<PaletteItem>, catalog: &brawler::map::MapContentCatalog) -> String {
    match item {
        Some(PaletteItem::Asset(id)) => catalog
            .asset(id)
            .map_or("Unavailable asset".to_string(), |asset| {
                asset.display_name.clone()
            }),
        Some(PaletteItem::Spawn(0)) => "Team A Spawn".to_string(),
        Some(PaletteItem::Spawn(_)) => "Team B Spawn".to_string(),
        Some(PaletteItem::HotZone) => "Capture Zone".to_string(),
        Some(PaletteItem::HeistSafe(0)) => "Team A Safe".to_string(),
        Some(PaletteItem::HeistSafe(_)) => "Team B Safe".to_string(),
        None => "No asset selected".to_string(),
    }
}

/// Team colours for spawns and safes in swatches and overviews.
const TEAM_A: Color = Color::srgb(0.25, 0.60, 1.0);
const TEAM_B: Color = Color::srgb(1.0, 0.36, 0.36);

/// A palette entry's flat colour in asset swatches and library overviews, by category.
fn palette_color(item: PaletteItem, catalog: &brawler::map::MapContentCatalog) -> Color {
    match item {
        PaletteItem::Spawn(0) => TEAM_A,
        PaletteItem::Spawn(_) => TEAM_B,
        PaletteItem::HeistSafe(0) => TEAM_A.darker(0.15),
        PaletteItem::HeistSafe(_) => TEAM_B.darker(0.15),
        PaletteItem::HotZone => PLAY_HIGHLIGHT,
        PaletteItem::Asset(_) => match palette_category(item, catalog) {
            PaletteCategory::Ground => Color::srgb(0.62, 0.56, 0.42),
            PaletteCategory::CoverObjects => Color::srgb(0.42, 0.48, 0.58),
            PaletteCategory::Vegetation => Color::srgb(0.24, 0.55, 0.30),
            PaletteCategory::EffectTiles => Color::srgb(0.36, 0.78, 0.86),
            PaletteCategory::Decorations => Color::srgb(0.72, 0.58, 0.42),
            PaletteCategory::SpawnsObjectives => TEXT,
        },
    }
}

pub(in crate::flow) fn return_to_editor_after_practice(
    mut runtime: ResMut<MapMakerRuntime>,
    mut next: ResMut<NextState<ClientFlow>>,
) {
    if runtime.return_to_editor && runtime.session.is_some() {
        runtime.return_to_editor = false;
        runtime.status = Some(
            "Returned from Practice. Edit this draft to require a fresh validation.".to_string(),
        );
        runtime.bump_render();
        next.set(ClientFlow::MapEditor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::{ButtonState, keyboard::KeyboardInput};

    #[test]
    fn editor_control_selection_is_owned_by_runtime_state() {
        let runtime = MapMakerRuntime::default();
        assert!(runtime.action_is_current(&FlowUiAction::MapMakerUsePlaceTool));
        assert!(runtime.action_is_current(&FlowUiAction::MapMakerSelectAsset(0)));
        assert!(runtime.action_is_current(&FlowUiAction::MapMakerToggleGrid));
        assert!(!runtime.action_is_current(&FlowUiAction::MapMakerUseEraseTool));
        assert!(!runtime.action_is_current(&FlowUiAction::MapMakerCycleSymmetry));
        assert!(runtime.action_is_current(&FlowUiAction::MapMakerToggleAssets));
        assert!(!runtime.action_is_current(&FlowUiAction::MapMakerToggleValidation));
    }

    #[test]
    fn asset_cycling_wraps_around_the_visible_list() {
        assert_eq!(cycled_palette_index(0, 34, -1), 33);
        assert_eq!(cycled_palette_index(33, 34, 1), 0);
        assert_eq!(cycled_palette_index(5, 34, 1), 6);
        assert_eq!(cycled_palette_index(0, 0, 1), 0);
    }

    #[test]
    fn tools_and_tabs_drive_the_side_panel() {
        let mut runtime = MapMakerRuntime::default();
        assert_eq!(runtime.panel, SidePanel::Assets);

        activate_editor_tool(&mut runtime, EditorTool::Select);
        assert_eq!(runtime.panel, SidePanel::Collapsed, "Select has no assets");
        assert_eq!(runtime.tool, EditorTool::Select);

        toggle_assets_drawer(&mut runtime);
        assert_eq!(runtime.panel, SidePanel::Assets);
        assert_eq!(runtime.tool, EditorTool::Place);

        toggle_assets_drawer(&mut runtime);
        assert_eq!(runtime.panel, SidePanel::Collapsed);

        // Check stays open across tool switches.
        runtime.panel = SidePanel::Check;
        activate_editor_tool(&mut runtime, EditorTool::Erase);
        assert_eq!(runtime.panel, SidePanel::Check);
        assert_eq!(SidePanel::opening(true), SidePanel::Collapsed);
        assert_eq!(SidePanel::opening(false), SidePanel::Assets);
    }

    #[test]
    fn check_add_opens_assets_on_the_fixing_entry() {
        let catalog = brawler::map::MapContentCatalog::embedded().unwrap();
        let mut runtime = MapMakerRuntime {
            session: Some(MapEditorSession::open(blank_feedback_document())),
            panel: SidePanel::Check,
            tool: EditorTool::Erase,
            palette_query: "wall".to_string(),
            ..MapMakerRuntime::default()
        };
        let mode = brawler::map::WIPEOUT_MODE_DEFINITION;
        let team_b = palette(&catalog, mode)
            .iter()
            .position(|item| *item == PaletteItem::Spawn(1))
            .unwrap();
        show_palette_item(&catalog, &mut runtime, team_b);
        assert_eq!(runtime.panel, SidePanel::Assets);
        assert_eq!(runtime.tool, EditorTool::Place);
        assert_eq!(runtime.palette_category, PaletteCategory::SpawnsObjectives);
        assert!(runtime.palette_query.is_empty());
        assert_eq!(
            selected_palette_item(&catalog, &runtime, mode),
            Some(PaletteItem::Spawn(1))
        );
    }

    /// A Wipeout draft with three spawns per team, so only the rule under test refuses it.
    fn spawned_session(catalog: &brawler::map::MapContentCatalog) -> MapEditorSession {
        let mut session = MapEditorSession::open(blank_feedback_document());
        for (team, x) in [(0, 4), (1, 26)] {
            for y in [5, 11, 17] {
                session
                    .place_spawn(catalog, MapCell::new(x, y), team, 0)
                    .unwrap();
            }
        }
        session
    }

    fn placement_at(
        session: &MapEditorSession,
        asset: MapAssetId,
        cell: MapCell,
    ) -> brawler::map::MapPlacementId {
        session
            .document
            .recipe
            .placements
            .iter()
            .find(|placement| placement.asset_id == asset && placement.cell == cell)
            .unwrap()
            .placement_id
    }

    /// BRL-855z: Show selects exactly what the rule refused and frames the camera on it.
    #[test]
    fn check_show_selects_and_frames_a_spawn_clearance_refusal() {
        let catalog = brawler::map::MapContentCatalog::embedded().unwrap();
        let mut session = spawned_session(&catalog);
        let tile_cell = MapCell::new(5, 11);
        session
            .place(&catalog, brawler::map::DAMAGE_TILE_ASSET, tile_cell, 0)
            .unwrap();
        let tile = placement_at(&session, brawler::map::DAMAGE_TILE_ASSET, tile_cell);
        let spawn = session
            .document
            .recipe
            .placements
            .iter()
            .find(|placement| placement.cell == MapCell::new(4, 11))
            .unwrap()
            .placement_id;
        let issue = check::map_check(&session.document.recipe, &catalog)
            .rule_error
            .unwrap();
        assert!(issue.located(), "{issue:?}");
        assert_eq!(issue.text, "Effect tile violates spawn safety clearance.");
        let mut runtime = MapMakerRuntime {
            session: Some(session),
            panel: SidePanel::Check,
            ..MapMakerRuntime::default()
        };
        show_rule_issue(&catalog, &mut runtime);
        assert_eq!(runtime.tool, EditorTool::Select);
        assert_eq!(runtime.selection, BTreeSet::from([tile, spawn]));
        assert_eq!(runtime.panel, SidePanel::Check, "Check stays open");
        let dimensions = runtime.session.as_ref().unwrap().document.recipe.dimensions;
        let expected =
            (dimensions.cell_center(tile_cell) + dimensions.cell_center(MapCell::new(4, 11))) * 0.5;
        assert!(runtime.camera_center.distance(expected) < 1.0);
        assert!(runtime.camera_zoom < 1.0, "zoomed in on the problem");
        assert_eq!(runtime.status.as_deref(), Some(issue.text.as_str()));
    }

    #[test]
    fn check_show_selects_a_placement_on_an_incompatible_surface() {
        let catalog = brawler::map::MapContentCatalog::embedded().unwrap();
        let mut session = spawned_session(&catalog);
        let wall_cell = MapCell::new(15, 11);
        let wall_asset = brawler::map::WALL_ARENA_ASSET;
        session.place(&catalog, wall_asset, wall_cell, 0).unwrap();
        session
            .place(&catalog, brawler::map::WATER_ASSET, wall_cell, 0)
            .unwrap();
        let wall = placement_at(&session, wall_asset, wall_cell);
        let check = check::map_check(&session.document.recipe, &catalog);
        let issue = check.rule_error.unwrap();
        assert_eq!(
            issue.text,
            "Placement is incompatible with the effective surface."
        );
        assert_eq!(issue.placements, [wall]);
        let mut runtime = MapMakerRuntime {
            session: Some(session),
            ..MapMakerRuntime::default()
        };
        show_rule_issue(&catalog, &mut runtime);
        assert_eq!(runtime.selection, BTreeSet::from([wall]));
        assert_eq!(runtime.tool, EditorTool::Select);
    }

    #[test]
    fn a_located_refusal_shows_even_while_counts_are_unmet() {
        let catalog = brawler::map::MapContentCatalog::embedded().unwrap();
        let mut session = MapEditorSession::open(blank_feedback_document());
        let cell = MapCell::new(15, 11);
        session
            .place(&catalog, brawler::map::WALL_ARENA_ASSET, cell, 0)
            .unwrap();
        let check = check::map_check(&session.document.recipe, &catalog);
        assert_eq!(check.rule_error, None, "the counts explain a plain draft");
        session
            .place(&catalog, brawler::map::WATER_ASSET, cell, 0)
            .unwrap();
        let check = check::map_check(&session.document.recipe, &catalog);
        assert!(
            check
                .rule_error
                .as_ref()
                .is_some_and(check::RuleIssue::located)
        );
        assert_eq!(check.issue_count(), 3, "two spawn counts and the surface");
    }

    #[test]
    fn fast_pointer_motion_interpolates_every_crossed_cell() {
        let mut cells = vec![MapCell::new(2, 3)];
        append_interpolated_cells(&mut cells, MapCell::new(6, 3));
        assert_eq!(
            cells,
            vec![
                MapCell::new(2, 3),
                MapCell::new(3, 3),
                MapCell::new(4, 3),
                MapCell::new(5, 3),
                MapCell::new(6, 3),
            ]
        );
    }

    #[test]
    fn palette_categories_are_derived_from_catalog_facts() {
        let catalog = brawler::map::MapContentCatalog::embedded().unwrap();
        assert_eq!(
            palette_category(PaletteItem::Asset(brawler::map::GROUND_ASSET), &catalog),
            PaletteCategory::Ground
        );
        assert_eq!(
            palette_category(PaletteItem::Asset(brawler::map::TALL_GRASS_ASSET), &catalog),
            PaletteCategory::Vegetation
        );
        assert_eq!(
            palette_category(PaletteItem::Asset(brawler::map::SPEED_TILE_ASSET), &catalog),
            PaletteCategory::EffectTiles
        );
        assert_eq!(
            palette_category(PaletteItem::Spawn(0), &catalog),
            PaletteCategory::SpawnsObjectives
        );
    }

    /// An editor runtime with one selected placement, so the camera-anchored selection toolbar
    /// is present too.
    fn camera_test_runtime(catalog: &brawler::map::MapContentCatalog) -> MapMakerRuntime {
        let document = MapDraftDocument::blank(
            MapDraftId(1),
            "Camera",
            brawler::map::WIPEOUT_MODE_DEFINITION,
            brawler::map::MapDimensions {
                width: 32,
                height: 24,
            },
            brawler::map::MapPresentationThemeId(1),
            brawler::map::GROUND_ASSET,
        )
        .unwrap();
        let mut session = MapEditorSession::open(document);
        session
            .place(
                catalog,
                brawler::map::TALL_GRASS_ASSET,
                MapCell::new(10, 10),
                0,
            )
            .unwrap();
        let selected = session.document.recipe.placements[0].placement_id;
        let mut runtime = MapMakerRuntime {
            session: Some(session),
            ..MapMakerRuntime::default()
        };
        activate_editor_tool(&mut runtime, EditorTool::Select);
        runtime.selection.insert(selected);
        runtime.bump_render();
        runtime
    }

    #[test]
    fn five_seconds_of_pan_and_zoom_never_respawn_the_editor_ui() {
        let catalog = brawler::map::MapContentCatalog::embedded().unwrap();
        let runtime = camera_test_runtime(&catalog);

        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::state::app::StatesPlugin,
            AssetPlugin::default(),
        ))
        .init_asset::<Image>()
        .init_asset::<Font>()
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            brawler::timing::SIMULATION_TICK,
        ))
        .add_message::<KeyboardInput>()
        .add_message::<Ime>()
        .add_message::<MouseWheel>()
        .insert_resource(ButtonInput::<KeyCode>::default())
        .insert_resource(ButtonInput::<MouseButton>::default())
        .insert_resource(AccumulatedMouseMotion::default())
        .insert_resource(PendingFlowActions::default())
        .init_resource::<crate::BackInput>()
        .add_message::<crate::ui::feedback::UiFeedback>()
        .insert_resource(MapCatalogResource(catalog))
        .insert_resource(runtime)
        .insert_state(ClientFlow::MapEditor)
        .add_systems(
            Update,
            (
                map_maker_input,
                present_map_editor,
                position_map_editor_selection_toolbar,
            )
                .chain(),
        );
        app.update();
        let root_and_toolbar = |app: &mut App| {
            let world = app.world_mut();
            let roots = world
                .query_filtered::<Entity, With<MapMakerRoot>>()
                .iter(world)
                .collect::<Vec<_>>();
            let toolbars = world
                .query_filtered::<Entity, With<editor::MapMakerSelectionToolbar>>()
                .iter(world)
                .collect::<Vec<_>>();
            (roots, toolbars)
        };
        let initial = root_and_toolbar(&mut app);
        assert_eq!(initial.0.len(), 1);
        assert_eq!(initial.1.len(), 1);

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Middle);
        let frames = brawler::timing::simulation_ticks_from_seconds(5).unwrap();
        for frame in 0..frames {
            app.world_mut()
                .resource_mut::<AccumulatedMouseMotion>()
                .delta = Vec2::new(0.4, -0.3);
            app.world_mut().write_message(MouseWheel {
                unit: bevy::input::mouse::MouseScrollUnit::Line,
                x: 0.0,
                y: if frame % 2 == 0 { 1.0 } else { -0.5 },
                window: Entity::PLACEHOLDER,
                phase: bevy::input::touch::TouchPhase::Moved,
            });
            app.update();
            assert_eq!(
                root_and_toolbar(&mut app),
                initial,
                "frame {frame} respawned editor UI"
            );
        }

        let runtime = app.world().resource::<MapMakerRuntime>();
        assert_ne!(runtime.camera_center, Vec2::ZERO);
        assert!((runtime.camera_zoom - 1.0).abs() > f32::EPSILON);
    }

    fn editor_ui_nodes(app: &mut App) -> BTreeSet<Entity> {
        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<Node>>()
            .iter(world)
            .collect()
    }

    fn editor_texts(app: &mut App) -> Vec<String> {
        let world = app.world_mut();
        world
            .query::<&Text>()
            .iter(world)
            .map(|text| text.0.clone())
            .collect()
    }

    #[test]
    fn ime_commits_type_a_new_map_name_within_its_bound() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .add_message::<KeyboardInput>()
            .add_message::<Ime>()
            .add_message::<MouseWheel>()
            .add_message::<crate::ui::feedback::UiFeedback>()
            .insert_resource(ButtonInput::<KeyCode>::default())
            .insert_resource(PendingFlowActions::default())
            .init_resource::<crate::BackInput>()
            .insert_resource(MapMakerRuntime {
                creating: Some("Arena".to_string()),
                ..MapMakerRuntime::default()
            })
            .insert_state(ClientFlow::MapLibrary)
            .add_systems(Update, map_maker_input);
        let commit = |app: &mut App, value: &str| {
            app.world_mut().write_message(Ime::Commit {
                window: Entity::PLACEHOLDER,
                value: value.to_string(),
            });
            app.update();
        };
        let name = |app: &App| {
            app.world()
                .resource::<MapMakerRuntime>()
                .editing_map_name()
                .map(str::to_string)
        };

        commit(&mut app, " Two");
        assert_eq!(name(&app).as_deref(), Some("Arena Two"));
        // An on-screen keyboard's deletion arrives as Backspace before its commit.
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::Backspace,
            logical_key: bevy::input::keyboard::Key::Backspace,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        commit(&mut app, "o\u{7}");
        assert_eq!(name(&app).as_deref(), Some("Arena Two"));
        commit(&mut app, &"x".repeat(200));
        assert_eq!(
            name(&app).map(|name| name.len()),
            Some(brawler::map::authoring::MAX_AUTHORED_MAP_NAME_BYTES)
        );
        assert!(
            app.world()
                .resource::<PendingFlowActions>()
                .ordinary
                .is_none()
        );
    }

    fn press_editor_key(app: &mut App, key: KeyCode, text: Option<&str>) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.world_mut().write_message(KeyboardInput {
            key_code: key,
            logical_key: bevy::input::keyboard::Key::Unidentified(
                bevy::input::keyboard::NativeKey::Unidentified,
            ),
            state: ButtonState::Pressed,
            text: text.map(Into::into),
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
        let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keyboard.release(key);
        keyboard.clear();
    }

    /// Editor input, action resolution, rebuild, and retained-value systems in their flow order.
    fn value_edit_test_app(
        catalog: brawler::map::MapContentCatalog,
        runtime: MapMakerRuntime,
    ) -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::state::app::StatesPlugin,
            AssetPlugin::default(),
        ))
        .init_asset::<Image>()
        .init_asset::<Font>()
        .add_message::<KeyboardInput>()
        .add_message::<Ime>()
        .add_message::<MouseWheel>()
        .insert_resource(ButtonInput::<KeyCode>::default())
        .insert_resource(ButtonInput::<MouseButton>::default())
        .insert_resource(AccumulatedMouseMotion::default())
        .insert_resource(PendingFlowActions::default())
        .init_resource::<crate::BackInput>()
        .add_message::<crate::ui::feedback::UiFeedback>()
        // No exercised action saves, renames, or deletes, so this path is never written.
        .insert_resource(MapDraftStorage(
            std::env::temp_dir().join("brl0175-value-edit-test-unused"),
        ))
        .init_resource::<MapDraftLibrary>()
        .init_resource::<SelectedGameType>()
        .init_resource::<ClientPracticeModel>()
        .init_resource::<crate::ClientMapPublishModel>()
        .init_resource::<crate::ClientPublishedMapsModel>()
        .init_resource::<SessionPurpose>()
        .init_resource::<FlowCommit>()
        .insert_resource(MapCatalogResource(catalog))
        .insert_resource(runtime)
        .insert_state(ClientFlow::MapEditor)
        .add_systems(
            Update,
            (
                map_maker_input,
                resolve_map_maker_action,
                present_map_editor,
                update_map_editor_values,
                follow_chip_preview,
            )
                .chain(),
        );
        app
    }

    #[test]
    fn value_only_edits_keep_the_same_editor_ui_nodes() {
        let catalog = brawler::map::MapContentCatalog::embedded().unwrap();
        // A non-empty query keeps the search field present, so a trailing space or its removal
        // changes only the field's text: the trimmed filter and visible rows stay put.
        let mut runtime = MapMakerRuntime {
            session: Some(MapEditorSession::open(blank_feedback_document())),
            palette_query: "e".to_string(),
            ..MapMakerRuntime::default()
        };
        select_first_visible_palette_item(&catalog, &mut runtime);
        runtime.bump_render();

        let mut app = value_edit_test_app(catalog, runtime);
        app.update();
        let initial = editor_ui_nodes(&mut app);
        let rows = {
            let world = app.world_mut();
            world
                .query::<&FlowButton>()
                .iter(world)
                .filter(|button| matches!(button.action, FlowUiAction::MapMakerSelectAsset(_)))
                .count()
        };
        assert!(rows > 0, "the Assets panel lists asset rows");
        let has_span = |app: &mut App, expected: &str| {
            let world = app.world_mut();
            world
                .query::<&TextSpan>()
                .iter(world)
                .any(|span| span.0 == expected)
        };
        let has_text =
            |app: &mut App, expected: &str| editor_texts(app).iter().any(|text| text == expected);
        let status_shown = |app: &mut App| {
            let world = app.world_mut();
            world
                .query_filtered::<&Node, With<editor::MapMakerStatusLine>>()
                .iter(world)
                .any(|node| node.display != Display::None)
        };
        assert!(has_text(&mut app, "e"));
        assert!(!status_shown(&mut app));

        app.world_mut()
            .resource_mut::<PendingFlowActions>()
            .ordinary = Some(FlowUiAction::MapMakerToggleSearch);
        app.update();
        assert_eq!(
            editor_ui_nodes(&mut app),
            initial,
            "search toggle respawned"
        );
        assert!(has_text(&mut app, "e|"));

        press_editor_key(&mut app, KeyCode::Space, Some(" "));
        assert_eq!(
            editor_ui_nodes(&mut app),
            initial,
            "search typing respawned"
        );
        assert!(has_text(&mut app, "e |"));

        press_editor_key(&mut app, KeyCode::Backspace, None);
        assert_eq!(
            editor_ui_nodes(&mut app),
            initial,
            "search backspace respawned"
        );
        assert!(has_text(&mut app, "e|"));

        press_editor_key(&mut app, KeyCode::Enter, None);
        assert_eq!(editor_ui_nodes(&mut app), initial, "search close respawned");
        assert!(has_text(&mut app, "e"));

        press_editor_key(&mut app, KeyCode::KeyG, None);
        assert_eq!(editor_ui_nodes(&mut app), initial, "grid toggle respawned");
        assert!(!app.world().resource::<MapMakerRuntime>().grid_visible);

        press_editor_key(&mut app, KeyCode::KeyM, None);
        assert_eq!(
            editor_ui_nodes(&mut app),
            initial,
            "symmetry cycle respawned"
        );
        assert!(has_span(&mut app, " · mirror left / right"));
        assert!(has_text(&mut app, "Mirror: left / right."));

        press_editor_key(&mut app, KeyCode::KeyR, None);
        assert_eq!(editor_ui_nodes(&mut app), initial, "rotation respawned");
        assert_eq!(app.world().resource::<MapMakerRuntime>().quarter_turns, 1);

        press_editor_key(&mut app, KeyCode::KeyC, None);
        assert_eq!(
            editor_ui_nodes(&mut app),
            initial,
            "status message respawned"
        );
        assert!(has_text(
            &mut app,
            "Press CLEAR again to remove all placements and anchors."
        ));
        assert!(status_shown(&mut app));

        // A structural edit (tool switch) still takes the full rebuild path.
        press_editor_key(&mut app, KeyCode::KeyV, None);
        assert!(
            editor_ui_nodes(&mut app).is_disjoint(&initial),
            "tool switch rebuilds the editor UI"
        );
    }

    #[test]
    fn asset_selection_and_cycling_keep_the_editor_ui_nodes() {
        let catalog = brawler::map::MapContentCatalog::embedded().unwrap();
        let mut runtime = MapMakerRuntime {
            session: Some(MapEditorSession::open(blank_feedback_document())),
            ..MapMakerRuntime::default()
        };
        let visible: Vec<usize> =
            visible_palette_entries(&catalog, &runtime, brawler::map::WIPEOUT_MODE_DEFINITION)
                .into_iter()
                .map(|(index, _)| index)
                .collect();
        assert!(
            visible.len() > 3,
            "the default category lists several assets"
        );
        select_first_visible_palette_item(&catalog, &mut runtime);
        runtime.bump_render();

        let mut app = value_edit_test_app(catalog, runtime);
        app.update();
        let initial = editor_ui_nodes(&mut app);
        let revision = app.world().resource::<MapMakerRuntime>().render_revision;
        let current = |app: &App, index: usize| {
            app.world()
                .resource::<MapMakerRuntime>()
                .action_is_current(&FlowUiAction::MapMakerSelectAsset(index))
        };

        app.world_mut()
            .resource_mut::<PendingFlowActions>()
            .ordinary = Some(FlowUiAction::MapMakerSelectAsset(visible[2]));
        app.update();
        assert_eq!(editor_ui_nodes(&mut app), initial, "row click respawned");
        assert!(current(&app, visible[2]));

        app.world_mut()
            .resource_mut::<PendingFlowActions>()
            .ordinary = Some(FlowUiAction::MapMakerCycleAsset(-1));
        app.update();
        assert_eq!(editor_ui_nodes(&mut app), initial, "asset cycle respawned");
        assert!(current(&app, visible[1]));

        // Past the end of the list the selection wraps, still without a rebuild.
        app.world_mut()
            .resource_mut::<PendingFlowActions>()
            .ordinary = Some(FlowUiAction::MapMakerSelectAsset(
            visible[visible.len() - 1],
        ));
        app.update();
        app.world_mut()
            .resource_mut::<PendingFlowActions>()
            .ordinary = Some(FlowUiAction::MapMakerCycleAsset(1));
        app.update();
        assert_eq!(editor_ui_nodes(&mut app), initial, "wrapping respawned");
        assert!(current(&app, visible[0]));
        assert_eq!(
            app.world().resource::<MapMakerRuntime>().render_revision,
            revision
        );
    }

    #[test]
    fn the_collapsed_chip_follows_the_selected_asset_without_a_rebuild() {
        let catalog = brawler::map::MapContentCatalog::embedded().unwrap();
        let mut runtime = MapMakerRuntime {
            session: Some(MapEditorSession::open(blank_feedback_document())),
            panel: SidePanel::Collapsed,
            ..MapMakerRuntime::default()
        };
        select_first_visible_palette_item(&catalog, &mut runtime);
        runtime.bump_render();
        let mut app = value_edit_test_app(catalog, runtime);
        app.update();
        let initial = editor_ui_nodes(&mut app);
        let chip_texts = |app: &mut App| {
            let world = app.world_mut();
            world
                .query::<(&editor::MapMakerValueLabel, &Text)>()
                .iter(world)
                .filter(|(label, _)| **label == editor::MapMakerValueLabel::Asset)
                .map(|(_, text)| text.0.clone())
                .collect::<Vec<_>>()
        };
        let before = chip_texts(&mut app);
        assert_eq!(before.len(), 1, "the collapsed panel shows the asset chip");
        let previews = |app: &mut App| {
            let world = app.world_mut();
            world
                .query::<&MapPalettePreviewHost>()
                .iter(world)
                .map(|host| host.visual)
                .collect::<Vec<_>>()
        };
        let preview_before = previews(&mut app);
        assert_eq!(preview_before.len(), 1);

        press_editor_key(&mut app, KeyCode::Tab, None);
        app.update();
        assert_ne!(chip_texts(&mut app), before, "the chip names the new asset");
        let preview_after = previews(&mut app);
        assert_eq!(preview_after.len(), 1);
        assert_ne!(preview_after, preview_before, "the chip preview follows");
        let kept = editor_ui_nodes(&mut app);
        assert!(
            initial.iter().filter(|node| !kept.contains(node)).count() <= 1,
            "only the chip's preview host was replaced"
        );
    }

    #[test]
    fn hidden_palette_selection_cannot_be_placed() {
        let catalog = brawler::map::MapContentCatalog::embedded().unwrap();
        let runtime = MapMakerRuntime {
            palette_query: "no asset has this name".to_string(),
            ..MapMakerRuntime::default()
        };

        assert!(
            selected_palette_item(&catalog, &runtime, brawler::map::WIPEOUT_MODE_DEFINITION,)
                .is_none()
        );
    }

    #[derive(Resource, Default)]
    struct RecordedUiFeedback(Vec<crate::ui::feedback::UiFeedback>);

    fn record_ui_feedback(
        mut feedback: MessageReader<crate::ui::feedback::UiFeedback>,
        mut recorded: ResMut<RecordedUiFeedback>,
    ) {
        recorded.0.extend(feedback.read().copied());
    }

    fn take_ui_feedback(app: &mut App) -> Vec<crate::ui::feedback::UiFeedback> {
        std::mem::take(&mut app.world_mut().resource_mut::<RecordedUiFeedback>().0)
    }

    fn blank_feedback_document() -> MapDraftDocument {
        MapDraftDocument::blank(
            MapDraftId(1),
            "Sounds",
            brawler::map::WIPEOUT_MODE_DEFINITION,
            brawler::map::MapDimensions {
                width: 32,
                height: 24,
            },
            brawler::map::MapPresentationThemeId(1),
            brawler::map::GROUND_ASSET,
        )
        .unwrap()
    }

    /// The editor at the grid's top-left corner with the Place tool and an asset selected.
    fn feedback_editor_app() -> App {
        let catalog = brawler::map::MapContentCatalog::embedded().unwrap();
        let mut runtime = MapMakerRuntime {
            session: Some(MapEditorSession::open(blank_feedback_document())),
            ..MapMakerRuntime::default()
        };
        select_first_visible_palette_item(&catalog, &mut runtime);
        runtime.bump_render();
        let mut app = value_edit_test_app(catalog, runtime);
        app.init_resource::<RecordedUiFeedback>()
            .add_systems(Last, record_ui_feedback);
        app.update();
        take_ui_feedback(&mut app);
        app
    }

    #[test]
    fn map_editor_keys_sound_once_per_real_action_and_stay_silent_at_the_grid_edge() {
        use crate::ui::feedback::UiFeedback;
        let mut app = feedback_editor_app();
        let key = |app: &mut App, key| {
            press_editor_key(app, key, None);
            take_ui_feedback(app)
        };
        assert_eq!(
            app.world().resource::<MapMakerRuntime>().cursor,
            MapCell::new(0, 0)
        );
        assert_eq!(key(&mut app, KeyCode::ArrowLeft), Vec::new(), "left edge");
        assert_eq!(key(&mut app, KeyCode::ArrowUp), Vec::new(), "top edge");
        assert_eq!(key(&mut app, KeyCode::ArrowRight), vec![UiFeedback::Moved]);
        assert_eq!(key(&mut app, KeyCode::ArrowDown), vec![UiFeedback::Moved]);
        assert_eq!(
            app.world().resource::<MapMakerRuntime>().cursor,
            MapCell::new(1, 1)
        );

        for shortcut in [
            KeyCode::Tab,
            KeyCode::KeyR,
            KeyCode::KeyM,
            KeyCode::KeyG,
            KeyCode::KeyF,
            KeyCode::Enter,
            KeyCode::KeyZ,
            KeyCode::Delete,
            KeyCode::KeyE,
        ] {
            assert_eq!(
                key(&mut app, shortcut),
                vec![UiFeedback::Activated(None)],
                "{shortcut:?}"
            );
        }
        // The Move tool has no keyed placement, so Enter does nothing and is silent.
        assert_eq!(
            key(&mut app, KeyCode::KeyV),
            vec![UiFeedback::Activated(None)]
        );
        assert_eq!(
            app.world().resource::<MapMakerRuntime>().tool,
            EditorTool::Select
        );
        assert_eq!(key(&mut app, KeyCode::Enter), Vec::new());

        // Esc first clears an interaction, then closes Check, and only then leaves.
        app.world_mut()
            .resource_mut::<MapMakerRuntime>()
            .interaction = EditorInteraction::Erase { cells: Vec::new() };
        assert_eq!(key(&mut app, KeyCode::Escape), vec![UiFeedback::Back]);
        assert_eq!(
            app.world().resource::<MapMakerRuntime>().interaction,
            EditorInteraction::Idle
        );
        app.world_mut().resource_mut::<MapMakerRuntime>().panel = SidePanel::Check;
        assert_eq!(key(&mut app, KeyCode::Escape), vec![UiFeedback::Back]);
        assert_eq!(
            app.world().resource::<MapMakerRuntime>().panel,
            SidePanel::Collapsed
        );
        // Plain unbound keys are silent.
        assert_eq!(key(&mut app, KeyCode::KeyK), Vec::new());
        // Q switches to chrome focus (BRL-q5sm) and clicks like a control.
        assert_eq!(
            key(&mut app, KeyCode::KeyQ),
            vec![UiFeedback::Activated(None)]
        );
    }

    #[test]
    fn map_palette_search_open_commit_and_cancel_sound_and_typing_is_silent() {
        use crate::ui::{feedback::UiFeedback, surface::Surface};
        let mut app = feedback_editor_app();
        press_editor_key(&mut app, KeyCode::Slash, Some("/"));
        assert!(
            app.world()
                .resource::<MapMakerRuntime>()
                .palette_search_active
        );
        assert_eq!(
            take_ui_feedback(&mut app),
            vec![UiFeedback::Activated(None)]
        );
        press_editor_key(&mut app, KeyCode::KeyE, Some("e"));
        assert_eq!(app.world().resource::<MapMakerRuntime>().palette_query, "e");
        assert_eq!(take_ui_feedback(&mut app), Vec::new(), "typing is silent");
        press_editor_key(&mut app, KeyCode::Enter, None);
        assert!(
            !app.world()
                .resource::<MapMakerRuntime>()
                .palette_search_active
        );
        assert_eq!(
            take_ui_feedback(&mut app),
            vec![UiFeedback::Activated(Some(Surface::Primary))]
        );

        press_editor_key(&mut app, KeyCode::Slash, Some("/"));
        take_ui_feedback(&mut app);
        press_editor_key(&mut app, KeyCode::Escape, None);
        assert!(
            !app.world()
                .resource::<MapMakerRuntime>()
                .palette_search_active
        );
        assert_eq!(take_ui_feedback(&mut app), vec![UiFeedback::Back]);
    }

    #[test]
    fn map_library_name_confirm_sounds_only_when_the_commit_succeeds() {
        use crate::ui::{feedback::UiFeedback, surface::Surface};
        let storage = std::env::temp_dir().join(format!(
            "brawler-brl-yrys-map-rename-{}",
            std::process::id()
        ));
        let mut library = MapDraftLibrary::default();
        library.insert(blank_feedback_document()).unwrap();
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .add_message::<KeyboardInput>()
            .add_message::<Ime>()
            .add_message::<MouseWheel>()
            .insert_resource(ButtonInput::<KeyCode>::default())
            .insert_resource(PendingFlowActions::default())
            .init_resource::<crate::BackInput>()
            .add_message::<UiFeedback>()
            .insert_resource(MapDraftStorage(storage.clone()))
            .insert_resource(library)
            .init_resource::<SelectedGameType>()
            .init_resource::<ClientPracticeModel>()
            .init_resource::<crate::ClientMapPublishModel>()
            .init_resource::<crate::ClientPublishedMapsModel>()
            .init_resource::<SessionPurpose>()
            .init_resource::<FlowCommit>()
            .insert_resource(MapCatalogResource(
                brawler::map::MapContentCatalog::embedded().unwrap(),
            ))
            .insert_resource(MapMakerRuntime::default())
            .init_resource::<RecordedUiFeedback>()
            .insert_state(ClientFlow::MapLibrary)
            .add_systems(Update, (map_maker_input, resolve_map_maker_action).chain())
            .add_systems(Last, record_ui_feedback);
        let confirm = |app: &mut App| {
            press_editor_key(app, KeyCode::Enter, None);
            // The keyed confirm resolves after input; its sound waits for the outcome.
            app.update();
            take_ui_feedback(app)
        };

        app.world_mut().resource_mut::<MapMakerRuntime>().renaming =
            Some((MapDraftId(1), "   ".to_string()));
        assert_eq!(
            confirm(&mut app),
            Vec::new(),
            "a rejected name plays nothing"
        );
        assert!(app.world().resource::<MapMakerRuntime>().renaming.is_some());

        app.world_mut().resource_mut::<MapMakerRuntime>().renaming =
            Some((MapDraftId(1), "Renamed".to_string()));
        let renamed = confirm(&mut app);
        let _ = std::fs::remove_dir_all(&storage);
        assert_eq!(renamed, vec![UiFeedback::Activated(Some(Surface::Primary))]);
        assert!(app.world().resource::<MapMakerRuntime>().renaming.is_none());

        // No server membership: the create fails inline and plays nothing.
        app.world_mut().resource_mut::<MapMakerRuntime>().creating = Some("New".to_string());
        assert_eq!(confirm(&mut app), Vec::new());
        assert!(app.world().resource::<MapMakerRuntime>().creating.is_some());
    }
}
