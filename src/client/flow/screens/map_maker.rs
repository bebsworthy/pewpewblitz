//! Native Map Maker library, editor presentation, and input ownership.

use super::shared::{FlowButton, FlowRoot, surface_role};
use crate::{
    client::{
        ArenaCamera, ClientLobbyMembership, ClientPracticeModel, MapDraftDocument, MapDraftId,
        MapDraftLibrary, MapEditorSession,
        flow::{
            ClientFlow, SelectedGameType, SessionPurpose,
            actions::{FlowCommit, FlowUiAction, OverlayCommit, PendingFlowActions},
            input::queue_ui_action,
        },
        map_maker::{MapDraftStorage, delete_map_draft, load_map_drafts, save_map_draft},
        ui::{
            surface::{Surface, SurfaceState, SurfaceStyle},
            theme::{CREAM, CYAN, INK, ORANGE, TEAL, font},
        },
    },
    map::{
        AuthoredMapCandidate, MapAssetId, MapAssetSlot, MapCatalogResource, MapCell, MapSymmetry,
        authorable_assets, placement_cells,
    },
};
use bevy::{
    input::{
        ButtonState,
        keyboard::KeyboardInput,
        mouse::{AccumulatedMouseMotion, MouseWheel},
    },
    prelude::*,
    ui::InteractionDisabled,
    window::PrimaryWindow,
};
use lightyear::prelude::client::Client;
use std::collections::BTreeSet;

const VISIBLE_PALETTE_ITEMS: usize = 6;
const PALETTE_PAGE_DELTA: i8 = 6;

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

    const fn label(self) -> &'static str {
        match self {
            Self::Ground => "GROUND",
            Self::CoverObjects => "COVER",
            Self::Vegetation => "VEGETATION",
            Self::EffectTiles => "EFFECTS",
            Self::Decorations => "DECOR",
            Self::SpawnsObjectives => "SPAWNS + GOALS",
        }
    }

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
pub(in crate::client::flow) struct MapMakerRuntime {
    pub session: Option<MapEditorSession>,
    cursor: MapCell,
    palette_index: usize,
    quarter_turns: u8,
    status: Option<String>,
    pending_delete: Option<MapDraftId>,
    renaming: Option<(MapDraftId, String)>,
    creating: Option<String>,
    clear_armed: bool,
    grid_visible: bool,
    tool: EditorTool,
    interaction: EditorInteraction,
    selection: BTreeSet<crate::map::MapPlacementId>,
    palette_category: PaletteCategory,
    palette_query: String,
    palette_search_active: bool,
    assets_open: bool,
    validation_open: bool,
    render_revision: u64,
    return_to_editor: bool,
    camera_center: Vec2,
    camera_zoom: f32,
    ui_press_latched: bool,
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
            clear_armed: false,
            grid_visible: true,
            tool: EditorTool::Place,
            interaction: EditorInteraction::Idle,
            selection: BTreeSet::new(),
            palette_category: PaletteCategory::CoverObjects,
            palette_query: String::new(),
            palette_search_active: false,
            assets_open: true,
            validation_open: false,
            render_revision: 0,
            return_to_editor: false,
            camera_center: Vec2::ZERO,
            camera_zoom: 1.0,
            ui_press_latched: false,
        }
    }
}

#[derive(Component)]
pub(in crate::client::flow) struct MapMakerRoot(u64);

#[derive(Component)]
pub(in crate::client::flow) struct MapMakerCursorCopy;

#[derive(Component, Clone, Copy)]
enum PaletteItem {
    Asset(MapAssetId),
    Spawn(u8),
    HotZone,
    HeistSafe(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::client) enum MapPaletteVisual {
    Asset(MapAssetId),
    Spawn(u8),
    HotZone,
    HeistSafe(u8),
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::client) struct MapPalettePreviewHost {
    pub(in crate::client) slot: usize,
    pub(in crate::client) visual: MapPaletteVisual,
    pub(in crate::client) theme: crate::map::MapPresentationThemeId,
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
pub(in crate::client::flow) fn load_map_library(
    mut commands: Commands,
    storage: Res<MapDraftStorage>,
) {
    commands.insert_resource(load_map_drafts(&storage));
}

#[allow(clippy::needless_pass_by_value)]
pub(in crate::client::flow) fn sync_map_editor_preview(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    runtime: Res<MapMakerRuntime>,
    catalog: Option<Res<MapCatalogResource>>,
    current: Option<Res<crate::client::presentation_3d::MapEditorPreview>>,
) {
    if *flow.get() != ClientFlow::MapEditor {
        if current.is_some() {
            commands.remove_resource::<crate::client::presentation_3d::MapEditorPreview>();
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
    let preview = crate::client::presentation_3d::MapEditorPreview::from_recipe(
        session.document.id,
        &preview_session.document.recipe,
        &catalog.0,
        crate::client::presentation_3d::MapEditorPreviewOptions {
            cursor: runtime.cursor,
            grid_visible: runtime.grid_visible,
            camera_center: runtime.camera_center,
            camera_zoom: runtime.camera_zoom,
            symmetry: session.symmetry,
            pointer_mode: match runtime.tool {
                EditorTool::Place => crate::client::presentation_3d::MapEditorPointerMode::Place,
                EditorTool::Erase => crate::client::presentation_3d::MapEditorPointerMode::Erase,
                EditorTool::Select => crate::client::presentation_3d::MapEditorPointerMode::Move,
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
    catalog: &crate::map::MapContentCatalog,
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

#[allow(clippy::needless_pass_by_value)]
pub(in crate::client::flow) fn update_map_editor_cursor_copy(
    runtime: Res<MapMakerRuntime>,
    mut copies: Query<&mut Text, With<MapMakerCursorCopy>>,
) {
    if !runtime.is_changed() {
        return;
    }
    let symmetry = runtime
        .session
        .as_ref()
        .map_or(MapSymmetry::None, |session| session.symmetry);
    for mut copy in &mut copies {
        **copy = format!(
            "{}°  ·  {}  ·  CELL {},{}",
            rotation_degrees(runtime.quarter_turns),
            symmetry_label(symmetry),
            runtime.cursor.x,
            runtime.cursor.y
        );
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]
pub(in crate::client::flow) fn resolve_map_maker_action(
    mut actions: ResMut<PendingFlowActions>,
    flow: Res<State<ClientFlow>>,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    catalog: Option<Res<MapCatalogResource>>,
    storage: Res<MapDraftStorage>,
    mut library: ResMut<MapDraftLibrary>,
    mut runtime: ResMut<MapMakerRuntime>,
    mut selection: ResMut<SelectedGameType>,
    mut practice: ResMut<ClientPracticeModel>,
    mut purpose: ResMut<SessionPurpose>,
    mut commit: ResMut<FlowCommit>,
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
        }
        FlowUiAction::CancelCreateMap if *flow.get() == ClientFlow::MapLibrary => {
            runtime.creating = None;
            runtime.status = None;
            runtime.bump_render();
        }
        FlowUiAction::OpenMapDraft(id) if *flow.get() == ClientFlow::MapLibrary => {
            if let Some(document) = library.drafts.get(&id).cloned() {
                runtime.session = Some(MapEditorSession::open(document));
                select_first_visible_palette_item(&catalog.0, &mut runtime);
                runtime.status = None;
                runtime.bump_render();
                commit.next_flow = Some(ClientFlow::MapEditor);
            }
        }
        FlowUiAction::BeginRenameMapDraft(id) if *flow.get() == ClientFlow::MapLibrary => {
            if let Some(document) = library.drafts.get(&id) {
                runtime.renaming = Some((id, document.name.clone()));
                runtime.pending_delete = None;
                runtime.status = Some("Type a new local map name, then confirm.".to_string());
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
                    library.insert(duplicate)
                });
            runtime.status = result.err().or_else(|| Some("Map duplicated.".to_string()));
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
                runtime.pending_delete = Some(id);
                runtime.status =
                    Some("Press DELETE again to confirm. This cannot be undone.".to_string());
            }
            runtime.bump_render();
        }
        FlowUiAction::BackToMapLibrary if *flow.get() == ClientFlow::MapEditor => {
            if runtime
                .session
                .as_ref()
                .is_some_and(MapEditorSession::dirty)
            {
                runtime.status = Some("Unsaved changes: Save before leaving, or use Clear/Undo to restore the saved revision.".to_string());
                runtime.bump_render();
            } else {
                runtime.session = None;
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
            } else {
                runtime.clear_armed = true;
                runtime.status =
                    Some("Press CLEAR again to remove all placements and anchors.".to_string());
            }
            runtime.bump_render();
        }
        FlowUiAction::MapMakerCycleAsset(delta) => {
            cycle_palette(&catalog.0, &mut runtime, delta);
        }
        FlowUiAction::MapMakerSelectAsset(index) => {
            let count = runtime.session.as_ref().map_or(0, |session| {
                palette(&catalog.0, session.document.recipe.mode_definition_id).len()
            });
            if index < count {
                runtime.palette_index = index;
                runtime.bump_render();
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
            }
            runtime.bump_render();
        }
        FlowUiAction::MapMakerRotate => {
            if runtime.tool == EditorTool::Select && !runtime.selection.is_empty() {
                let selected = runtime.selection.clone();
                if let Some(session) = runtime.session.as_mut() {
                    runtime.status = session.rotate_placements(&catalog.0, &selected).err();
                }
            } else {
                runtime.quarter_turns = (runtime.quarter_turns + 1) % 4;
            }
            runtime.bump_render();
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
                runtime.palette_category = category;
                select_first_visible_palette_item(&catalog.0, &mut runtime);
                runtime.bump_render();
            }
        }
        FlowUiAction::MapMakerToggleSearch => {
            runtime.palette_search_active = !runtime.palette_search_active;
            runtime.bump_render();
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
            runtime.validation_open = !runtime.validation_open;
            runtime.bump_render();
        }
        FlowUiAction::MapMakerToggleGrid => {
            runtime.grid_visible = !runtime.grid_visible;
            runtime.bump_render();
        }
        FlowUiAction::MapMakerFitView => {
            runtime.camera_center = Vec2::ZERO;
            runtime.camera_zoom = 1.0;
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
            | FlowUiAction::BeginRenameMapDraft(_)
            | FlowUiAction::ConfirmRenameMapDraft
            | FlowUiAction::CancelRenameMapDraft
            | FlowUiAction::DuplicateMapDraft(_)
            | FlowUiAction::DeleteMapDraft(_)
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
    )
}

fn create_default_document(
    memberships: &Query<&ClientLobbyMembership, With<Client>>,
    selection: &SelectedGameType,
    catalog: &crate::map::MapContentCatalog,
    library: &mut MapDraftLibrary,
    runtime: &mut MapMakerRuntime,
    commit: &mut FlowCommit,
    name: &str,
) {
    let Some(membership) = memberships.iter().next() else {
        return;
    };
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
        });
    let Some(game) = game else {
        runtime.status = Some("This server does not advertise a Map Maker mode.".to_string());
        return;
    };
    let Some(preset) = game
        .practice_map_preset_ids
        .first()
        .and_then(|id| catalog.preset(*id))
    else {
        runtime.status = Some("No compatible map defaults are available.".to_string());
        return;
    };
    let id = library.next_id();
    match MapDraftDocument::blank(
        id,
        name,
        game.mode_definition_id,
        preset.recipe.dimensions,
        preset.recipe.presentation_theme_id,
        preset.recipe.default_surface_asset_id,
    ) {
        Ok(mut document) => {
            document.intended_game_type_id = Some(game.id.clone());
            if let Err(error) = library.insert(document.clone()) {
                runtime.status = Some(error);
                return;
            }
            runtime.session = Some(MapEditorSession::open(document));
            runtime.creating = None;
            runtime.cursor = MapCell::new(
                preset.recipe.dimensions.width / 2,
                preset.recipe.dimensions.height / 2,
            );
            select_first_visible_palette_item(catalog, runtime);
            runtime.status = Some(
                "Draft created. Add spawns and the selected mode objective before testing."
                    .to_string(),
            );
            runtime.bump_render();
            commit.next_flow = Some(ClientFlow::MapEditor);
        }
        Err(error) => runtime.status = Some(error),
    }
}

fn supported_mode(mode: crate::map::ModeDefinitionId) -> bool {
    matches!(
        mode,
        crate::map::WIPEOUT_MODE_DEFINITION
            | crate::map::HOT_ZONE_MODE_DEFINITION
            | crate::map::HEIST_MODE_DEFINITION
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
    catalog: &crate::map::MapContentCatalog,
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
        runtime.bump_render();
        return;
    };
    match crate::map::validate_authored_recipe(
        &session.document.recipe,
        game.mode_definition_id,
        catalog,
        crate::map::MapInstanceId(1),
    ) {
        Ok(_) => {}
        Err(error) => {
            runtime.status = Some(error.to_string());
            runtime.bump_render();
            return;
        }
    }
    if let Err(error) = save_map_draft(storage, &session.document)
        .and_then(|()| library.insert(session.document.clone()))
    {
        runtime.status = Some(error);
        runtime.bump_render();
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
    let selected_brawler = membership.profile.selected_brawler_id.and_then(|id| {
        membership
            .profile
            .brawlers
            .iter()
            .find(|brawler| brawler.id == id)
    });
    if let Some(brawler) = selected_brawler {
        *purpose = SessionPurpose::Practice;
        if practice.start(selection, brawler.id, brawler.revision) {
            runtime.return_to_editor = true;
            runtime.status = Some("Submitting this saved revision to the server…".to_string());
        }
    } else {
        runtime.status = Some("Select or create a brawler before testing.".to_string());
    }
    runtime.bump_render();
}

impl MapMakerRuntime {
    fn bump_render(&mut self) {
        self.render_revision = self.render_revision.wrapping_add(1).max(1);
    }

    pub(in crate::client::flow) fn action_selected(&self, action: &FlowUiAction) -> bool {
        match action {
            FlowUiAction::MapMakerSelectAsset(index) => self.palette_index == *index,
            FlowUiAction::MapMakerUsePlaceTool => self.tool == EditorTool::Place,
            FlowUiAction::MapMakerUseEraseTool => self.tool == EditorTool::Erase,
            FlowUiAction::MapMakerUseMoveTool => self.tool == EditorTool::Select,
            FlowUiAction::MapMakerSelectCategory(category) => {
                PaletteCategory::from_u8(*category) == Some(self.palette_category)
            }
            FlowUiAction::MapMakerToggleSearch => self.palette_search_active,
            FlowUiAction::MapMakerToggleAssets => self.assets_open,
            FlowUiAction::MapMakerToggleValidation => self.validation_open,
            FlowUiAction::MapMakerCycleSymmetry => self
                .session
                .as_ref()
                .is_some_and(|session| session.symmetry != MapSymmetry::None),
            FlowUiAction::MapMakerToggleGrid => self.grid_visible,
            FlowUiAction::MapMakerClear => self.clear_armed,
            _ => false,
        }
    }
}

fn activate_editor_tool(runtime: &mut MapMakerRuntime, tool: EditorTool) {
    runtime.tool = tool;
    runtime.interaction = EditorInteraction::Idle;
    runtime.validation_open = false;
    runtime.assets_open = tool == EditorTool::Place;
    if tool != EditorTool::Place {
        runtime.palette_search_active = false;
    }
    if tool != EditorTool::Select {
        runtime.selection.clear();
    }
}

fn toggle_assets_drawer(runtime: &mut MapMakerRuntime) {
    if runtime.tool == EditorTool::Place {
        runtime.assets_open = !runtime.assets_open;
        runtime.validation_open = false;
        if !runtime.assets_open {
            runtime.palette_search_active = false;
        }
    } else {
        activate_editor_tool(runtime, EditorTool::Place);
    }
}

fn palette(
    catalog: &crate::map::MapContentCatalog,
    mode: crate::map::ModeDefinitionId,
) -> Vec<PaletteItem> {
    let mut items = authorable_assets(catalog)
        .into_iter()
        .map(|asset| PaletteItem::Asset(asset.id))
        .collect::<Vec<_>>();
    items.extend([PaletteItem::Spawn(0), PaletteItem::Spawn(1)]);
    if mode == crate::map::HOT_ZONE_MODE_DEFINITION {
        items.push(PaletteItem::HotZone);
    }
    if mode == crate::map::HEIST_MODE_DEFINITION {
        items.extend([PaletteItem::HeistSafe(0), PaletteItem::HeistSafe(1)]);
    }
    items
}

fn cycle_palette(
    catalog: &crate::map::MapContentCatalog,
    runtime: &mut MapMakerRuntime,
    delta: i8,
) {
    let Some(session) = runtime.session.as_ref() else {
        return;
    };
    let visible =
        visible_palette_entries(catalog, runtime, session.document.recipe.mode_definition_id);
    if !visible.is_empty() {
        let current = visible
            .iter()
            .position(|(index, _)| *index == runtime.palette_index)
            .unwrap_or(0);
        let next = cycled_palette_index(current, visible.len(), delta);
        runtime.palette_index = visible[next].0;
    }
    runtime.bump_render();
}

fn select_first_visible_palette_item(
    catalog: &crate::map::MapContentCatalog,
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
    catalog: &crate::map::MapContentCatalog,
    runtime: &MapMakerRuntime,
    mode: crate::map::ModeDefinitionId,
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

fn selected_palette_item(
    catalog: &crate::map::MapContentCatalog,
    runtime: &MapMakerRuntime,
    mode: crate::map::ModeDefinitionId,
) -> Option<PaletteItem> {
    visible_palette_entries(catalog, runtime, mode)
        .into_iter()
        .find_map(|(index, item)| (index == runtime.palette_index).then_some(item))
}

fn palette_category(item: PaletteItem, catalog: &crate::map::MapContentCatalog) -> PaletteCategory {
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
            } else if profile.concealment == crate::map::MapConcealmentBehavior::HideOccupants {
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
    if usize::from(delta.unsigned_abs()) == VISIBLE_PALETTE_ITEMS {
        let page_count = count.div_ceil(VISIBLE_PALETTE_ITEMS);
        let page = (current / VISIBLE_PALETTE_ITEMS).min(page_count - 1);
        let next_page = if delta.is_negative() {
            page.saturating_sub(1)
        } else {
            (page + 1).min(page_count - 1)
        };
        return next_page * VISIBLE_PALETTE_ITEMS;
    }
    let amount = usize::from(delta.unsigned_abs()) % count;
    if delta.is_negative() {
        (current + count - amount) % count
    } else {
        (current + amount) % count
    }
}

fn place_current(catalog: &crate::map::MapContentCatalog, runtime: &mut MapMakerRuntime) {
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
        runtime.bump_render();
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
    catalog: &crate::map::MapContentCatalog,
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
        runtime.bump_render();
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

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]
pub(in crate::client::flow) fn map_maker_input(
    flow: Res<State<ClientFlow>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut keyboard_events: MessageReader<KeyboardInput>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    mouse_motion: Option<Res<AccumulatedMouseMotion>>,
    mut mouse_wheel: MessageReader<MouseWheel>,
    gamepads: Query<&Gamepad>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<ArenaCamera>>,
    catalog: Option<Res<MapCatalogResource>>,
    buttons: Query<(&FlowButton, Ref<Interaction>)>,
    mut runtime: ResMut<MapMakerRuntime>,
    mut actions: ResMut<PendingFlowActions>,
) {
    if !matches!(*flow.get(), ClientFlow::MapLibrary | ClientFlow::MapEditor) {
        return;
    }
    let pointer_is_down = mouse
        .as_deref()
        .is_some_and(|mouse| mouse.pressed(MouseButton::Left));
    if !pointer_is_down {
        runtime.ui_press_latched = false;
    }
    let mut ui_activated = false;
    for (button, interaction) in &buttons {
        if is_map_maker_action(&button.action)
            && accept_button_press(
                interaction.is_changed(),
                *interaction,
                pointer_is_down,
                &mut runtime.ui_press_latched,
            )
        {
            queue_ui_action(&mut actions, button.action.clone());
            ui_activated = true;
        }
    }
    let validation_trigger_owns_pointer = buttons.iter().any(|(button, interaction)| {
        button.action == FlowUiAction::MapMakerToggleValidation
            && matches!(*interaction, Interaction::Hovered | Interaction::Pressed)
    });
    if runtime.validation_open
        && mouse
            .as_deref()
            .is_some_and(|mouse| mouse.just_pressed(MouseButton::Left))
        && !validation_trigger_owns_pointer
    {
        runtime.validation_open = false;
        runtime.bump_render();
    }
    if *flow.get() == ClientFlow::MapLibrary {
        if let Some(name) = runtime.creating.as_mut() {
            for event in keyboard_events.read() {
                if event.state != ButtonState::Pressed {
                    continue;
                }
                if event.key_code == KeyCode::Backspace {
                    name.pop();
                } else if let Some(text) = event.text.as_deref() {
                    for character in text.chars().filter(|character| !character.is_control()) {
                        if name.len() + character.len_utf8()
                            <= crate::map::authoring::MAX_AUTHORED_MAP_NAME_BYTES
                        {
                            name.push(character);
                        }
                    }
                }
            }
            if keyboard.just_pressed(KeyCode::Enter) {
                queue_ui_action(&mut actions, FlowUiAction::ConfirmCreateMap);
            } else if keyboard.just_pressed(KeyCode::Escape) {
                queue_ui_action(&mut actions, FlowUiAction::CancelCreateMap);
            }
            return;
        }
        if let Some((_, name)) = runtime.renaming.as_mut() {
            for event in keyboard_events.read() {
                if event.state != ButtonState::Pressed {
                    continue;
                }
                if event.key_code == KeyCode::Backspace {
                    name.pop();
                } else if let Some(text) = event.text.as_deref() {
                    for character in text.chars().filter(|character| !character.is_control()) {
                        if name.len() + character.len_utf8()
                            <= crate::map::authoring::MAX_AUTHORED_MAP_NAME_BYTES
                        {
                            name.push(character);
                        }
                    }
                }
            }
            if keyboard.just_pressed(KeyCode::Enter) {
                queue_ui_action(&mut actions, FlowUiAction::ConfirmRenameMapDraft);
            } else if keyboard.just_pressed(KeyCode::Escape) {
                queue_ui_action(&mut actions, FlowUiAction::CancelRenameMapDraft);
            }
            return;
        }
        if keyboard.just_pressed(KeyCode::Escape)
            || gamepads
                .iter()
                .any(|pad| pad.just_pressed(GamepadButton::East))
        {
            queue_ui_action(&mut actions, FlowUiAction::CloseMapMaker);
        }
        return;
    }
    if runtime.palette_search_active {
        let mut changed = false;
        for event in keyboard_events.read() {
            if event.state != ButtonState::Pressed {
                continue;
            }
            if event.key_code == KeyCode::Slash && runtime.palette_query.is_empty() {
                continue;
            }
            if event.key_code == KeyCode::Backspace {
                changed |= runtime.palette_query.pop().is_some();
            } else if let Some(text) = event.text.as_deref() {
                for character in text.chars().filter(|character| !character.is_control()) {
                    if runtime.palette_query.len() + character.len_utf8() <= 48 {
                        runtime.palette_query.push(character);
                        changed = true;
                    }
                }
            }
        }
        if keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::Escape) {
            runtime.palette_search_active = false;
            changed = true;
        }
        if changed {
            if let Some(catalog) = catalog.as_deref() {
                select_first_visible_palette_item(&catalog.0, &mut runtime);
            }
            runtime.bump_render();
        }
        return;
    }
    if keyboard.just_pressed(KeyCode::Slash) {
        activate_editor_tool(&mut runtime, EditorTool::Place);
        runtime.palette_search_active = true;
        runtime.bump_render();
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
                crate::client::presentation_3d::cursor_ground_point(camera, transform, cursor)
            })
        });
    if let (Some(dimensions), Some(world)) = (dimensions, world_cursor)
        && let Some(cell) = dimensions.world_cell(world)
    {
        runtime.cursor = cell;
    }
    let pad = |button| gamepads.iter().any(|gamepad| gamepad.just_pressed(button));
    if let Some(dimensions) = dimensions {
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
    if keyboard.just_pressed(KeyCode::Tab) || pad(GamepadButton::RightTrigger2) {
        queue_ui_action(&mut actions, FlowUiAction::MapMakerCycleAsset(1));
    }
    if pad(GamepadButton::LeftTrigger2) {
        queue_ui_action(&mut actions, FlowUiAction::MapMakerCycleAsset(-1));
    }
    if keyboard.just_pressed(KeyCode::KeyR) || pad(GamepadButton::West) {
        queue_ui_action(&mut actions, FlowUiAction::MapMakerRotate);
    }
    if keyboard.just_pressed(KeyCode::KeyM) || pad(GamepadButton::North) {
        queue_ui_action(&mut actions, FlowUiAction::MapMakerCycleSymmetry);
    }
    if keyboard.just_pressed(KeyCode::KeyP) {
        queue_ui_action(&mut actions, FlowUiAction::MapMakerUsePlaceTool);
    }
    if keyboard.just_pressed(KeyCode::KeyE) {
        queue_ui_action(&mut actions, FlowUiAction::MapMakerUseEraseTool);
    }
    if keyboard.just_pressed(KeyCode::KeyV) {
        queue_ui_action(&mut actions, FlowUiAction::MapMakerUseMoveTool);
    }
    let ui_owns_pointer = ui_activated
        || buttons.iter().any(|(_, interaction)| {
            matches!(*interaction, Interaction::Hovered | Interaction::Pressed)
        });
    let palette_owns_pointer = buttons.iter().any(|(button, interaction)| {
        is_palette_action(&button.action)
            && matches!(*interaction, Interaction::Hovered | Interaction::Pressed)
    });
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
            if motion.delta != Vec2::ZERO {
                runtime.bump_render();
            }
        }
        runtime.interaction = EditorInteraction::Idle;
    }
    for event in mouse_wheel.read() {
        if palette_owns_pointer {
            let direction = if event.y > 0.0 || event.x < 0.0 {
                -1
            } else {
                1
            };
            queue_ui_action(&mut actions, FlowUiAction::MapMakerCycleAsset(direction));
        } else if !ui_owns_pointer {
            runtime.camera_zoom = (runtime.camera_zoom * (-event.y * 0.10).exp()).clamp(0.5, 3.0);
            runtime.bump_render();
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
    if keyboard.just_pressed(KeyCode::Enter)
        || (keyboard.just_pressed(KeyCode::Space) && !mouse_pressed)
        || pad(GamepadButton::South)
    {
        match runtime.tool {
            EditorTool::Place => queue_ui_action(&mut actions, FlowUiAction::MapMakerPlace),
            EditorTool::Erase => queue_ui_action(&mut actions, FlowUiAction::MapMakerErase),
            EditorTool::Select => {}
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
    }
    if keyboard.just_pressed(KeyCode::KeyZ) {
        queue_ui_action(&mut actions, FlowUiAction::MapMakerUndo);
    }
    if keyboard.just_pressed(KeyCode::KeyY) {
        queue_ui_action(&mut actions, FlowUiAction::MapMakerRedo);
    }
    if keyboard.just_pressed(KeyCode::KeyS) {
        queue_ui_action(&mut actions, FlowUiAction::SaveMapDraft);
    }
    if keyboard.just_pressed(KeyCode::KeyT) {
        queue_ui_action(&mut actions, FlowUiAction::TestAuthoredMap);
    }
    if keyboard.just_pressed(KeyCode::KeyG) {
        queue_ui_action(&mut actions, FlowUiAction::MapMakerToggleGrid);
    }
    if keyboard.just_pressed(KeyCode::KeyC) {
        queue_ui_action(&mut actions, FlowUiAction::MapMakerClear);
    }
    if keyboard.just_pressed(KeyCode::KeyF) {
        queue_ui_action(&mut actions, FlowUiAction::MapMakerFitView);
    }
    if keyboard.just_pressed(KeyCode::Escape) || pad(GamepadButton::East) {
        if runtime.validation_open {
            runtime.validation_open = false;
            runtime.bump_render();
        } else if runtime.interaction != EditorInteraction::Idle || !runtime.selection.is_empty() {
            runtime.interaction = EditorInteraction::Idle;
            runtime.selection.clear();
            runtime.status = None;
            runtime.bump_render();
        } else {
            queue_ui_action(&mut actions, FlowUiAction::BackToMapLibrary);
        }
    }
}

fn is_palette_action(action: &FlowUiAction) -> bool {
    matches!(
        action,
        FlowUiAction::MapMakerCycleAsset(_)
            | FlowUiAction::MapMakerSelectAsset(_)
            | FlowUiAction::MapMakerSelectCategory(_)
            | FlowUiAction::MapMakerToggleSearch
            | FlowUiAction::MapMakerClearSearch
            | FlowUiAction::MapMakerToggleAssets
    )
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

fn accept_button_press(
    interaction_changed: bool,
    interaction: Interaction,
    pointer_is_down: bool,
    latched: &mut bool,
) -> bool {
    if !pointer_is_down {
        *latched = false;
    }
    if !interaction_changed || interaction != Interaction::Pressed || *latched {
        return false;
    }
    *latched = pointer_is_down;
    true
}

#[allow(clippy::needless_pass_by_value)]
pub(in crate::client::flow) fn present_map_library(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    library: Res<MapDraftLibrary>,
    runtime: Res<MapMakerRuntime>,
    catalog: Option<Res<MapCatalogResource>>,
    roots: Query<(Entity, &MapMakerRoot)>,
    assets: Option<Res<crate::client::ClientAssetHandles>>,
) {
    if *flow.get() != ClientFlow::MapLibrary {
        return;
    }
    if roots
        .iter()
        .any(|(_, root)| root.0 == runtime.render_revision)
    {
        return;
    }
    for (entity, _) in &roots {
        commands.entity(entity).try_despawn();
    }
    commands
        .spawn((
            FlowRoot,
            MapMakerRoot(runtime.render_revision),
            DespawnOnExit(ClientFlow::MapLibrary),
            root_node(),
            BackgroundColor(INK),
            GlobalZIndex(420),
        ))
        .with_children(|root| {
            header(
                root,
                assets.as_deref(),
                "MY MAPS",
                "Create, reopen, duplicate, or delete local drafts.",
            );
            if let Some(status) = runtime.status.as_deref() {
                root.spawn((
                    Text::new(status),
                    font(assets.as_deref(), 18.0),
                    TextColor(ORANGE),
                ));
            }
            if let Some((_, name)) = runtime.renaming.as_ref() {
                rename_panel(root, assets.as_deref(), name);
            }
            if let Some(name) = runtime.creating.as_deref() {
                create_map_panel(root, assets.as_deref(), name, &runtime, catalog.as_deref());
            }
            if library.drafts.is_empty() {
                root.spawn((
                    Text::new("No maps yet. Create a bounded draft to begin."),
                    font(assets.as_deref(), 22.0),
                    TextColor(CREAM),
                ));
            }
            root.spawn((Node {
                width: percent(100),
                flex_wrap: FlexWrap::Wrap,
                column_gap: px(16),
                row_gap: px(16),
                justify_content: JustifyContent::Center,
                ..default()
            },))
                .with_children(|cards| {
                    for document in library.drafts.values() {
                        map_card(
                            cards,
                            assets.as_deref(),
                            document,
                            runtime.pending_delete == Some(document.id),
                            catalog.as_deref().map(|catalog| &catalog.0),
                        );
                    }
                });
            action_button(
                root,
                assets.as_deref(),
                9000,
                FlowUiAction::CreateMap,
                "CREATE MAP",
                ORANGE,
            );
            action_button(
                root,
                assets.as_deref(),
                9001,
                FlowUiAction::CloseMapMaker,
                "BACK",
                TEAL,
            );
        });
}

fn create_map_panel(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    name: &str,
    _runtime: &MapMakerRuntime,
    catalog: Option<&MapCatalogResource>,
) {
    let preview = catalog
        .and_then(|catalog| catalog.0.presets.first())
        .map_or("SERVER DEFAULT THEME".to_string(), |preset| {
            format!(
                "{} × {}  •  THEME {}",
                preset.recipe.dimensions.width,
                preset.recipe.dimensions.height,
                preset.recipe.presentation_theme_id.0
            )
        });
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: percent(24),
                right: percent(24),
                top: percent(18),
                bottom: percent(18),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                justify_content: JustifyContent::Center,
                row_gap: px(16),
                padding: UiRect::all(px(28)),
                border: UiRect::all(px(3)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.07, 0.08, 0.98)),
            BorderColor::all(TEAL),
            GlobalZIndex(460),
        ))
        .with_children(|modal| {
            modal.spawn((
                Text::new("CREATE MAP"),
                font(assets, 32.0),
                TextColor(CREAM),
            ));
            modal.spawn((Text::new("MAP NAME"), font(assets, 14.0), TextColor(TEAL)));
            modal
                .spawn((
                    Node {
                        min_height: px(54),
                        align_items: AlignItems::Center,
                        padding: UiRect::axes(px(14), px(8)),
                        border: UiRect::all(px(2)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.09, 0.14, 0.15)),
                    BorderColor::all(ORANGE),
                ))
                .with_child((
                    Text::new(format!("{name}|")),
                    font(assets, 21.0),
                    TextColor(CREAM),
                ));
            modal
                .spawn((
                    Node {
                        min_height: px(150),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border: UiRect::all(px(2)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.12, 0.28, 0.25)),
                    BorderColor::all(TEAL),
                ))
                .with_child((Text::new(preview), font(assets, 19.0), TextColor(CREAM)));
            modal
                .spawn((Node {
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::FlexEnd,
                    column_gap: px(12),
                    ..default()
                },))
                .with_children(|actions| {
                    action_button(
                        actions,
                        assets,
                        8_980,
                        FlowUiAction::CancelCreateMap,
                        "CANCEL",
                        TEAL,
                    );
                    action_button(
                        actions,
                        assets,
                        8_981,
                        FlowUiAction::ConfirmCreateMap,
                        "CREATE",
                        ORANGE,
                    );
                });
        });
}

fn rename_panel(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    name: &str,
) {
    parent.spawn((
        Text::new(format!("RENAME: {name}|")),
        font(assets, 22.0),
        TextColor(CREAM),
    ));
    parent
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            column_gap: px(10),
            ..default()
        },))
        .with_children(|actions| {
            action_button(
                actions,
                assets,
                8990,
                FlowUiAction::ConfirmRenameMapDraft,
                "CONFIRM RENAME",
                ORANGE,
            );
            action_button(
                actions,
                assets,
                8991,
                FlowUiAction::CancelRenameMapDraft,
                "CANCEL",
                TEAL,
            );
        });
}

fn map_card(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    document: &MapDraftDocument,
    confirming_delete: bool,
    catalog: Option<&crate::map::MapContentCatalog>,
) {
    let button_base = usize::try_from(document.id.0)
        .unwrap_or(usize::MAX / 10)
        .saturating_mul(10);
    parent
        .spawn((
            Node {
                width: px(300),
                min_height: px(190),
                flex_direction: FlexDirection::Column,
                row_gap: px(8),
                padding: UiRect::all(px(14)),
                border: UiRect::all(px(2)),
                ..default()
            },
            BackgroundColor(CREAM),
            BorderColor::all(TEAL),
        ))
        .with_children(|card| {
            card.spawn((
                Text::new(document.name.clone()),
                font(assets, 25.0),
                TextColor(INK),
            ));
            if let Some(catalog) = catalog {
                let known = document
                    .recipe
                    .placements
                    .iter()
                    .filter(|placement| catalog.asset(placement.asset_id).is_some())
                    .count();
                card.spawn((
                    Text::new(format!(
                        "THEME {}  •  {known} rendered assets",
                        document.recipe.presentation_theme_id.0
                    )),
                    font(assets, 14.0),
                    TextColor(TEAL),
                ));
            }
            card.spawn((
                Text::new(format!(
                    "{} × {} cells  •  {} placements",
                    document.recipe.dimensions.width,
                    document.recipe.dimensions.height,
                    document.recipe.placements.len()
                )),
                font(assets, 16.0),
                TextColor(INK),
            ));
            action_button(
                card,
                assets,
                button_base,
                FlowUiAction::OpenMapDraft(document.id),
                "OPEN",
                ORANGE,
            );
            action_button(
                card,
                assets,
                button_base.saturating_add(1),
                FlowUiAction::BeginRenameMapDraft(document.id),
                "RENAME",
                TEAL,
            );
            action_button(
                card,
                assets,
                button_base.saturating_add(2),
                FlowUiAction::DuplicateMapDraft(document.id),
                "DUPLICATE",
                TEAL,
            );
            action_button(
                card,
                assets,
                button_base.saturating_add(3),
                FlowUiAction::DeleteMapDraft(document.id),
                if confirming_delete {
                    "CONFIRM DELETE"
                } else {
                    "DELETE"
                },
                Color::srgb(0.75, 0.18, 0.15),
            );
        });
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "the editor shell consumes independent Bevy-owned resources and queries"
)]
pub(in crate::client::flow) fn present_map_editor(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    runtime: Res<MapMakerRuntime>,
    catalog: Option<Res<MapCatalogResource>>,
    roots: Query<(Entity, &MapMakerRoot)>,
    assets: Option<Res<crate::client::ClientAssetHandles>>,
    asset_server: Res<AssetServer>,
    cameras: Query<(&Camera, &GlobalTransform), With<ArenaCamera>>,
) {
    if *flow.get() != ClientFlow::MapEditor {
        return;
    }
    if roots
        .iter()
        .any(|(_, root)| root.0 == runtime.render_revision)
    {
        return;
    }
    for (entity, _) in &roots {
        commands.entity(entity).try_despawn();
    }
    let (Some(session), Some(catalog)) = (runtime.session.as_ref(), catalog.as_deref()) else {
        return;
    };
    let validation_error = local_validation_error(session, &catalog.0);
    let selection_toolbar_position = selection_toolbar_position(
        session,
        &catalog.0,
        &runtime.selection,
        cameras.iter().next(),
    );
    commands
        .spawn((
            FlowRoot,
            MapMakerRoot(runtime.render_revision),
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
            editor_document_bar(
                root,
                assets.as_deref(),
                session,
                validation_error.as_deref(),
                runtime.validation_open,
                &asset_server,
            );
            editor_tool_rail(root, assets.as_deref(), &runtime, &asset_server);
            editor_canvas_controls(root, assets.as_deref(), &runtime, &asset_server);
            editor_selection_toolbar(
                root,
                assets.as_deref(),
                &runtime,
                &asset_server,
                selection_toolbar_position,
            );
            editor_validation_popover(
                root,
                assets.as_deref(),
                &runtime,
                validation_error.as_deref(),
                &asset_server,
            );
            editor_palette(root, assets.as_deref(), &runtime, &catalog.0, &asset_server);
            if let Some(status) = runtime.status.as_deref() {
                root.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: percent(25),
                        right: percent(25),
                        bottom: px(
                            if runtime.tool == EditorTool::Place && runtime.assets_open {
                                302
                            } else {
                                34
                            },
                        ),
                        padding: UiRect::axes(px(16), px(9)),
                        justify_content: JustifyContent::Center,
                        border: UiRect::all(px(2)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.04, 0.07, 0.08, 0.92)),
                    BorderColor::all(ORANGE),
                ))
                .with_child((
                    Text::new(status),
                    font(assets.as_deref(), 15.0),
                    TextColor(CREAM),
                ));
            }
        });
}

fn local_validation_error(
    session: &MapEditorSession,
    catalog: &crate::map::MapContentCatalog,
) -> Option<String> {
    crate::map::validate_authored_recipe(
        &session.document.recipe,
        session.document.recipe.mode_definition_id,
        catalog,
        crate::map::MapInstanceId(1),
    )
    .err()
    .map(|error| error.to_string())
}

fn selection_toolbar_position(
    session: &MapEditorSession,
    catalog: &crate::map::MapContentCatalog,
    selection: &BTreeSet<crate::map::MapPlacementId>,
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
                .and_then(|asset| placement_cells(recipe.dimensions, asset, placement))
                .unwrap_or_else(|| vec![placement.cell])
        })
        .collect::<Vec<_>>();
    cells.extend(
        recipe
            .mode_anchors
            .iter()
            .filter(|anchor| selection.contains(&anchor.placement_id))
            .map(|anchor| match anchor.kind {
                crate::map::MapModeAnchorKind::HotZoneCircle {
                    center_half_cell, ..
                }
                | crate::map::MapModeAnchorKind::ZombieIngress { center_half_cell } => {
                    MapCell::new(center_half_cell.x / 2, center_half_cell.y / 2)
                }
                crate::map::MapModeAnchorKind::HeistSafe { origin_cell, .. } => origin_cell,
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

#[allow(
    clippy::too_many_lines,
    reason = "the document bar declares one cohesive Bevy UI row and its child controls"
)]
fn editor_document_bar(
    root: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    session: &MapEditorSession,
    validation_error: Option<&str>,
    validation_open: bool,
    asset_server: &AssetServer,
) {
    let recipe = &session.document.recipe;
    let ready_to_test = validation_error.is_none();
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(14),
            right: px(14),
            top: px(12),
            height: px(70),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(10),
            padding: UiRect::axes(px(18), px(7)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(14)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.04, 0.07, 0.08, 0.93)),
        BorderColor::all(Color::srgba(0.32, 0.64, 0.62, 0.35)),
    ))
    .with_children(|panel| {
        panel
            .spawn(Node {
                width: px(330),
                flex_direction: FlexDirection::Column,
                ..default()
            })
            .with_children(|identity| {
                identity.spawn((
                    Text::new(session.document.name.to_uppercase()),
                    font(assets, 19.0),
                    TextColor(CREAM),
                ));
                identity
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: px(5),
                        ..default()
                    })
                    .with_children(|status| {
                        if !session.dirty() {
                            status.spawn((
                                ImageNode::new(
                                    asset_server.load("brawler/ui/details/map-saved.png"),
                                ),
                                Node {
                                    width: px(17),
                                    height: px(17),
                                    ..default()
                                },
                            ));
                        }
                        status.spawn((
                            Text::new(format!(
                                "{} × {}  •  r{}  •  {}",
                                recipe.dimensions.width,
                                recipe.dimensions.height,
                                recipe.revision,
                                if session.dirty() { "UNSAVED" } else { "SAVED" }
                            )),
                            font(assets, 11.0),
                            TextColor(if ready_to_test && !session.dirty() {
                                CYAN
                            } else {
                                ORANGE
                            }),
                        ));
                    });
            });
        panel.spawn(Node {
            flex_grow: 1.0,
            ..default()
        });
        compact_action_button_disabled(
            panel,
            assets,
            7,
            FlowUiAction::MapMakerUndo,
            "UNDO",
            false,
            !session.can_undo(),
        );
        compact_action_button_disabled(
            panel,
            assets,
            8,
            FlowUiAction::MapMakerRedo,
            "REDO",
            false,
            !session.can_redo(),
        );
        panel.spawn(Node {
            width: px(18),
            ..default()
        });
        compact_action_button(
            panel,
            assets,
            21,
            FlowUiAction::SaveMapDraft,
            "SAVE",
            TEAL,
            false,
        );
        compact_icon_action_button_disabled(
            panel,
            assets,
            asset_server,
            22,
            FlowUiAction::TestAuthoredMap,
            "TEST MAP",
            "brawler/ui/details/map-play.png",
            126.0,
            true,
            !ready_to_test,
        );
        compact_icon_action_button_disabled(
            panel,
            assets,
            asset_server,
            23,
            FlowUiAction::MapMakerToggleValidation,
            if ready_to_test { "READY" } else { "1 ISSUE" },
            if ready_to_test {
                "brawler/ui/details/map-saved.png"
            } else {
                "brawler/ui/details/map-warning.png"
            },
            112.0,
            validation_open,
            ready_to_test,
        );
        compact_action_button(
            panel,
            assets,
            20,
            FlowUiAction::BackToMapLibrary,
            "MAPS",
            TEAL,
            false,
        );
    });
}

fn editor_tool_rail(
    root: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    runtime: &MapMakerRuntime,
    asset_server: &AssetServer,
) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(14),
            top: px(94),
            width: px(116),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(12),
            padding: UiRect::all(px(10)),
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::all(px(18)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.04, 0.07, 0.08, 0.93)),
        BorderColor::all(Color::srgba(0.32, 0.64, 0.62, 0.35)),
    ))
    .with_children(|panel| {
        tool_button(
            panel,
            assets,
            30,
            FlowUiAction::MapMakerUsePlaceTool,
            "PLACE",
            asset_server.load("brawler/ui/details/map-place.png"),
            runtime.tool == EditorTool::Place,
        );
        tool_button(
            panel,
            assets,
            32,
            FlowUiAction::MapMakerUseMoveTool,
            "SELECT",
            asset_server.load("brawler/ui/details/map-select.png"),
            runtime.tool == EditorTool::Select,
        );
        tool_button(
            panel,
            assets,
            31,
            FlowUiAction::MapMakerUseEraseTool,
            "ERASE",
            asset_server.load("brawler/ui/details/map-erase.png"),
            runtime.tool == EditorTool::Erase,
        );
    });
}

fn tool_button(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    index: usize,
    action: FlowUiAction,
    label: &str,
    icon: Handle<Image>,
    selected: bool,
) {
    parent
        .spawn((
            Button,
            FlowButton {
                index,
                action,
                error_action: false,
            },
            Node {
                width: px(94),
                height: px(104),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(2),
                ..default()
            },
            Surface::Card,
            SurfaceStyle::MAP_TOOL,
            SurfaceState {
                selected,
                ..default()
            },
        ))
        .with_children(|button| {
            button.spawn((
                ImageNode::new(icon),
                Node {
                    width: px(46),
                    height: px(46),
                    ..default()
                },
            ));
            button.spawn((
                Text::new(label),
                font(assets, 17.0),
                TextColor(INK),
                TextLayout::justify(Justify::Center),
            ));
        });
}

fn editor_canvas_controls(
    root: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    runtime: &MapMakerRuntime,
    asset_server: &AssetServer,
) {
    let symmetry = runtime
        .session
        .as_ref()
        .map_or(MapSymmetry::None, |session| session.symmetry);
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: px(20),
            top: px(96),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(5),
            padding: UiRect::all(px(5)),
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::all(px(14)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.04, 0.07, 0.08, 0.92)),
        BorderColor::all(Color::srgba(0.32, 0.64, 0.62, 0.35)),
    ))
    .with_children(|panel| {
        compact_icon_action_button_disabled(
            panel,
            assets,
            asset_server,
            10,
            FlowUiAction::MapMakerToggleGrid,
            if runtime.grid_visible {
                "GRID\nON"
            } else {
                "GRID\nOFF"
            },
            "brawler/ui/details/map-grid.png",
            96.0,
            runtime.grid_visible,
            false,
        );
        compact_icon_action_button_disabled(
            panel,
            assets,
            asset_server,
            4,
            FlowUiAction::MapMakerCycleSymmetry,
            &format!("SYMMETRY\n{}", symmetry_label(symmetry)),
            "brawler/ui/details/map-symmetry.png",
            146.0,
            false,
            false,
        );
        compact_icon_action_button_disabled(
            panel,
            assets,
            asset_server,
            11,
            FlowUiAction::MapMakerFitView,
            "FIT",
            "brawler/ui/details/map-fit.png",
            78.0,
            false,
            false,
        );
    });
}

fn editor_selection_toolbar(
    root: &mut ChildSpawnerCommands,
    _assets: Option<&crate::client::ClientAssetHandles>,
    runtime: &MapMakerRuntime,
    asset_server: &AssetServer,
    position: Option<Vec2>,
) {
    if runtime.tool != EditorTool::Select || runtime.selection.is_empty() {
        return;
    }
    let position = position.unwrap_or(Vec2::new(700.0, 220.0));
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px((position.x - 102.0).max(126.0)),
            top: px((position.y - 88.0).max(92.0)),
            flex_direction: FlexDirection::Row,
            column_gap: px(5),
            padding: UiRect::all(px(5)),
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::all(px(10)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.04, 0.07, 0.08, 0.94)),
        BorderColor::all(Color::srgba(0.32, 0.64, 0.62, 0.35)),
    ))
    .with_children(|panel| {
        icon_image_button(
            panel,
            40,
            FlowUiAction::MapMakerRotate,
            asset_server.load("brawler/ui/details/map-rotate.png"),
            false,
        );
        icon_image_button(
            panel,
            41,
            FlowUiAction::MapMakerDuplicateSelection,
            asset_server.load("brawler/ui/details/map-duplicate.png"),
            false,
        );
        icon_image_button(
            panel,
            42,
            FlowUiAction::MapMakerDeleteSelection,
            asset_server.load("brawler/ui/details/trash.png"),
            true,
        );
        icon_image_button(
            panel,
            43,
            FlowUiAction::MapMakerClearSelection,
            asset_server.load("brawler/ui/details/map-close.png"),
            false,
        );
    });
}

fn icon_image_button(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    action: FlowUiAction,
    icon: Handle<Image>,
    danger: bool,
) {
    let label = match &action {
        FlowUiAction::MapMakerRotate => "Rotate selection 90 degrees",
        FlowUiAction::MapMakerDuplicateSelection => "Duplicate selection",
        FlowUiAction::MapMakerDeleteSelection => "Delete selection",
        FlowUiAction::MapMakerClearSelection => "Clear selection",
        FlowUiAction::MapMakerClearSearch => "Clear asset search",
        _ => "Selection action",
    };
    parent
        .spawn((
            Button,
            AccessibleLabel::new(label),
            FlowButton {
                index,
                action,
                error_action: false,
            },
            Node {
                width: px(50),
                height: px(46),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            if danger {
                Surface::Danger
            } else {
                Surface::Action
            },
            SurfaceStyle::MAP_ACTION,
            SurfaceState::default(),
        ))
        .with_child((
            ImageNode {
                color: if danger { CREAM } else { INK },
                ..ImageNode::new(icon)
            },
            Node {
                width: px(25),
                height: px(25),
                ..default()
            },
        ));
}

fn editor_validation_popover(
    root: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    runtime: &MapMakerRuntime,
    validation_error: Option<&str>,
    asset_server: &AssetServer,
) {
    let Some(error) = validation_popover_error(runtime, validation_error) else {
        return;
    };
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: px(146),
            top: px(88),
            width: px(310),
            flex_direction: FlexDirection::Column,
            row_gap: px(7),
            padding: UiRect::all(px(14)),
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::all(px(14)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.04, 0.07, 0.08, 0.97)),
        BorderColor::all(ORANGE),
    ))
    .with_children(|panel| {
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(8),
                ..default()
            })
            .with_children(|title| {
                title.spawn((
                    ImageNode::new(asset_server.load("brawler/ui/details/map-warning.png")),
                    Node {
                        width: px(23),
                        height: px(23),
                        ..default()
                    },
                ));
                title.spawn((
                    Text::new("MAP CHECK  •  1 ISSUE"),
                    font(assets, 17.0),
                    TextColor(ORANGE),
                ));
            });
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexStart,
                column_gap: px(9),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    Node {
                        width: px(18),
                        height: px(18),
                        flex_shrink: 0.0,
                        border: UiRect::all(px(2)),
                        ..default()
                    },
                    BorderColor::all(CREAM),
                ));
                row.spawn((
                    Text::new(error),
                    font(assets, 12.0),
                    TextColor(CREAM),
                    Node {
                        max_width: px(250),
                        ..default()
                    },
                ));
            });
    });
}

fn validation_popover_error<'a>(
    runtime: &MapMakerRuntime,
    validation_error: Option<&'a str>,
) -> Option<&'a str> {
    runtime
        .validation_open
        .then_some(validation_error)
        .flatten()
}

#[allow(
    clippy::too_many_lines,
    reason = "the asset browser declares its category and visible-card rows together"
)]
fn editor_palette(
    root: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    runtime: &MapMakerRuntime,
    catalog: &crate::map::MapContentCatalog,
    asset_server: &AssetServer,
) {
    let Some(session) = runtime.session.as_ref() else {
        return;
    };
    if runtime.tool != EditorTool::Place || !runtime.assets_open {
        root.spawn((
            Button,
            FlowButton {
                index: 60,
                action: FlowUiAction::MapMakerToggleAssets,
                error_action: false,
            },
            Node {
                position_type: PositionType::Absolute,
                left: px(14),
                bottom: px(8),
                width: px(184),
                height: px(58),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            Surface::Dark,
            SurfaceStyle::MAP_ACTION,
            SurfaceState::default(),
        ))
        .with_children(|button| {
            button.spawn((Text::new("ASSETS"), font(assets, 20.0), TextColor(CREAM)));
            button.spawn((
                ImageNode {
                    color: CREAM,
                    ..ImageNode::new(asset_server.load("brawler/ui/details/map-chevron-up.png"))
                },
                Node {
                    width: px(20),
                    height: px(20),
                    margin: UiRect::left(px(10)),
                    ..default()
                },
            ));
        });
        root.spawn((
            Text::new(match runtime.tool {
                EditorTool::Select => {
                    "Drag empty space to select  •  Shift-click to add  •  Delete to remove  •  Space-drag to pan"
                }
                EditorTool::Erase => {
                    "Click or drag to erase  •  Esc cancels  •  Space-drag to pan  •  Wheel to zoom"
                }
                EditorTool::Place => {
                    "Click or drag to place  •  R rotates  •  Esc cancels  •  Space-drag to pan"
                }
            }),
            font(assets, 11.0),
            TextColor(Color::srgba(0.52, 0.85, 0.82, 0.78)),
            Node {
                position_type: PositionType::Absolute,
                left: percent(28),
                right: percent(20),
                bottom: px(15),
                ..default()
            },
            TextLayout::justify(Justify::Center),
        ));
        return;
    }
    let theme = session.document.recipe.presentation_theme_id;
    let entries =
        visible_palette_entries(catalog, runtime, session.document.recipe.mode_definition_id);
    let selected_position = entries
        .iter()
        .position(|(index, _)| *index == runtime.palette_index)
        .unwrap_or(0);
    let (start, end) = palette_window(selected_position, entries.len());
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(14),
            right: px(14),
            bottom: px(8),
            height: px(282),
            flex_direction: FlexDirection::Column,
            row_gap: px(8),
            padding: UiRect::axes(px(12), px(9)),
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::all(px(18)),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(Color::srgba(0.04, 0.07, 0.08, 0.94)),
        BorderColor::all(Color::srgba(0.32, 0.64, 0.62, 0.35)),
    ))
    .with_children(|tray| {
        tray.spawn(Node {
            width: percent(100),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(7),
            ..default()
        })
        .with_children(|header| {
            header
                .spawn((
                    Button,
                    FlowButton {
                        index: 60,
                        action: FlowUiAction::MapMakerToggleAssets,
                        error_action: false,
                    },
                    Node {
                        width: px(164),
                        height: px(42),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    Surface::Dark,
                    SurfaceStyle::MAP_ACTION,
                    SurfaceState::default(),
                ))
                .with_children(|button| {
                    button.spawn((Text::new("ASSETS"), font(assets, 18.0), TextColor(CREAM)));
                    button.spawn((
                        ImageNode {
                            color: CREAM,
                            ..ImageNode::new(
                                asset_server.load("brawler/ui/details/map-chevron-down.png"),
                            )
                        },
                        Node {
                            width: px(18),
                            height: px(18),
                            margin: UiRect::left(px(8)),
                            ..default()
                        },
                    ));
                });
            for (index, category) in PaletteCategory::ALL.into_iter().enumerate() {
                palette_category_button(
                    header,
                    assets,
                    50 + index,
                    FlowUiAction::MapMakerSelectCategory(category as u8),
                    category.label(),
                    runtime.palette_category == category,
                );
            }
            header.spawn(Node {
                flex_grow: 1.0,
                ..default()
            });
            let search = if runtime.palette_query.is_empty() {
                if runtime.palette_search_active {
                    "_".to_string()
                } else {
                    "SEARCH ASSETS".to_string()
                }
            } else {
                format!(
                    "{}{}",
                    runtime.palette_query,
                    if runtime.palette_search_active {
                        "_"
                    } else {
                        ""
                    }
                )
            };
            search_button(
                header,
                assets,
                asset_server,
                56,
                FlowUiAction::MapMakerToggleSearch,
                &search,
                runtime.palette_search_active,
            );
            if !runtime.palette_query.is_empty() {
                icon_image_button(
                    header,
                    57,
                    FlowUiAction::MapMakerClearSearch,
                    asset_server.load("brawler/ui/details/map-close.png"),
                    false,
                );
            }
        });
        tray.spawn(Node {
            width: percent(100),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: px(8),
            ..default()
        })
        .with_children(|items| {
            palette_arrow_button(
                items,
                assets,
                asset_server,
                1,
                FlowUiAction::MapMakerCycleAsset(-PALETTE_PAGE_DELTA),
                "‹",
                start == 0,
            );
            for (slot, (index, item)) in entries
                .iter()
                .copied()
                .enumerate()
                .skip(start)
                .take(VISIBLE_PALETTE_ITEMS)
            {
                palette_item_button(
                    items,
                    assets,
                    100 + index,
                    FlowUiAction::MapMakerSelectAsset(index),
                    &palette_short_label(item, catalog),
                    MapPalettePreviewHost {
                        slot: slot - start,
                        visual: item.into(),
                        theme,
                    },
                    runtime.palette_index == index,
                );
            }
            if entries.is_empty() {
                items.spawn((
                    Text::new("NO ASSETS MATCH THIS CATEGORY AND SEARCH"),
                    font(assets, 13.0),
                    TextColor(CREAM),
                    Node {
                        width: px(812),
                        ..default()
                    },
                    TextLayout::justify(Justify::Center),
                ));
            }
            palette_arrow_button(
                items,
                assets,
                asset_server,
                2,
                FlowUiAction::MapMakerCycleAsset(PALETTE_PAGE_DELTA),
                "›",
                end >= entries.len(),
            );
            items.spawn((
                Text::new(if entries.is_empty() {
                    "0 OF 0".to_string()
                } else {
                    format!("{}–{} OF {}", start + 1, end, entries.len())
                }),
                font(assets, 12.0),
                TextLayout::justify(Justify::Center),
                TextColor(TEAL),
                Node {
                    width: px(82),
                    ..default()
                },
            ));
        });
        tray.spawn((
            Text::new("Click or drag to place  •  R rotates  •  Esc cancels  •  Space-drag to pan"),
            font(assets, 10.0),
            TextColor(Color::srgba(0.52, 0.85, 0.82, 0.78)),
            Node {
                width: percent(100),
                ..default()
            },
            TextLayout::justify(Justify::Center),
        ));
    });
}

fn palette_window(selected_position: usize, entry_count: usize) -> (usize, usize) {
    let start = (selected_position / VISIBLE_PALETTE_ITEMS) * VISIBLE_PALETTE_ITEMS;
    (start, (start + VISIBLE_PALETTE_ITEMS).min(entry_count))
}

fn search_button(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    asset_server: &AssetServer,
    index: usize,
    action: FlowUiAction,
    label: &str,
    selected: bool,
) {
    parent
        .spawn((
            Button,
            FlowButton {
                index,
                action,
                error_action: false,
            },
            Node {
                width: px(286),
                height: px(40),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(9),
                padding: UiRect::axes(px(14), px(6)),
                ..default()
            },
            Surface::Dark,
            SurfaceStyle::MAP_ASSET,
            SurfaceState {
                selected,
                ..default()
            },
        ))
        .with_children(|button| {
            button.spawn((
                ImageNode {
                    color: CREAM,
                    ..ImageNode::new(asset_server.load("brawler/ui/details/map-search.png"))
                },
                Node {
                    width: px(24),
                    height: px(24),
                    ..default()
                },
            ));
            button.spawn((Text::new(label), font(assets, 13.0), TextColor(CREAM)));
        });
}

fn palette_arrow_button(
    parent: &mut ChildSpawnerCommands,
    _assets: Option<&crate::client::ClientAssetHandles>,
    asset_server: &AssetServer,
    index: usize,
    action: FlowUiAction,
    glyph: &str,
    disabled: bool,
) {
    let mut button = parent.spawn((
        Button,
        FlowButton {
            index,
            action,
            error_action: false,
        },
        Node {
            width: px(64),
            height: px(76),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        Surface::Dark,
        SurfaceStyle::MAP_ACTION,
        SurfaceState {
            selected: !disabled,
            disabled,
            ..default()
        },
    ));
    if disabled {
        button.insert(InteractionDisabled);
    }
    button.with_child((
        ImageNode {
            color: if disabled {
                Color::srgba(0.50, 0.63, 0.64, 0.55)
            } else {
                INK
            },
            ..ImageNode::new(asset_server.load("brawler/ui/details/map-chevron-down.png"))
        },
        Node {
            width: px(28),
            height: px(28),
            ..default()
        },
        UiTransform::from_rotation(Rot2::radians(if glyph == "‹" {
            core::f32::consts::FRAC_PI_2
        } else {
            -core::f32::consts::FRAC_PI_2
        })),
    ));
}

fn palette_category_button(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    index: usize,
    action: FlowUiAction,
    label: &str,
    selected: bool,
) {
    parent
        .spawn((
            Button,
            FlowButton {
                index,
                action,
                error_action: false,
            },
            Node {
                width: px(if label == "SPAWNS + GOALS" { 152 } else { 112 }),
                height: px(40),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(px(6), px(5)),
                ..default()
            },
            Surface::Dark,
            SurfaceStyle::MAP_TAB,
            SurfaceState {
                selected,
                ..default()
            },
        ))
        .with_child((
            Text::new(label),
            font(assets, 10.0),
            TextLayout::justify(Justify::Center),
            TextColor(if selected { INK } else { CREAM }),
        ));
}

#[allow(clippy::too_many_arguments)]
fn palette_item_button(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    index: usize,
    action: FlowUiAction,
    label: &str,
    preview: MapPalettePreviewHost,
    selected: bool,
) {
    parent
        .spawn((
            Button,
            FlowButton {
                index,
                action,
                error_action: false,
            },
            Node {
                width: px(180),
                height: px(174),
                position_type: PositionType::Relative,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(px(5), px(6)),
                row_gap: px(3),
                ..default()
            },
            Surface::Dark,
            SurfaceStyle::MAP_ASSET,
            SurfaceState {
                selected,
                ..default()
            },
        ))
        .with_children(|button| {
            if selected {
                button.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(0),
                        right: px(0),
                        top: px(0),
                        bottom: px(0),
                        border: UiRect::all(px(3)),
                        ..default()
                    },
                    BorderColor::all(Color::srgb(0.02, 0.94, 0.90)),
                ));
                palette_selection_badge(button);
            }
            button.spawn((
                preview,
                Node {
                    width: px(168),
                    height: px(124),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.08, 0.20, 0.22)),
            ));
            button.spawn((
                Text::new(label.replace('\n', " · ")),
                font(assets, 11.0),
                TextLayout::justify(Justify::Center),
                TextColor(CREAM),
            ));
        });
}

fn palette_selection_badge(button: &mut ChildSpawnerCommands) {
    button
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(7),
                top: px(7),
                width: px(30),
                height: px(30),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BackgroundColor(Color::srgb(0.05, 0.92, 0.84)),
            BorderColor::all(INK),
            ZIndex(4),
        ))
        .with_children(|badge| {
            for (left, top, width, angle) in [(7, 15, 9, 0.72), (12, 12, 13, -0.78)] {
                badge.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(left),
                        top: px(top),
                        width: px(width),
                        height: px(3),
                        ..default()
                    },
                    BackgroundColor(INK),
                    UiTransform::from_rotation(Rot2::radians(angle)),
                ));
            }
        });
}

fn palette_short_label(item: PaletteItem, catalog: &crate::map::MapContentCatalog) -> String {
    match item {
        PaletteItem::Asset(id) => catalog.asset(id).map_or_else(
            || "UNAVAILABLE".to_string(),
            |asset| asset.display_name.to_uppercase(),
        ),
        PaletteItem::Spawn(0) => "TEAM A SPAWN".to_string(),
        PaletteItem::Spawn(_) => "TEAM B SPAWN".to_string(),
        PaletteItem::HotZone => "CAPTURE ZONE".to_string(),
        PaletteItem::HeistSafe(0) => "TEAM A SAFE".to_string(),
        PaletteItem::HeistSafe(_) => "TEAM B SAFE".to_string(),
    }
}

#[allow(clippy::too_many_arguments)]
fn compact_action_button(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    index: usize,
    action: FlowUiAction,
    label: &str,
    _color: Color,
    selected: bool,
) {
    compact_action_button_disabled(parent, assets, index, action, label, selected, false);
}

#[allow(clippy::too_many_arguments)]
fn compact_icon_action_button_disabled(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    asset_server: &AssetServer,
    index: usize,
    action: FlowUiAction,
    label: &str,
    icon_path: &'static str,
    min_width: f32,
    selected: bool,
    disabled: bool,
) {
    let role = if matches!(
        action,
        FlowUiAction::MapMakerToggleGrid
            | FlowUiAction::MapMakerCycleSymmetry
            | FlowUiAction::MapMakerFitView
    ) {
        Surface::Dark
    } else {
        surface_role(&action)
    };
    let content_color = if role == Surface::Dark && !selected {
        CREAM
    } else {
        INK
    };
    let mut button = parent.spawn((
        Button,
        FlowButton {
            index,
            action,
            error_action: false,
        },
        Node {
            min_width: px(min_width),
            min_height: px(50),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: px(7),
            padding: UiRect::axes(px(10), px(6)),
            ..default()
        },
        role,
        SurfaceStyle::MAP_ACTION,
        SurfaceState {
            selected,
            disabled,
            ..default()
        },
    ));
    if disabled {
        button.insert(InteractionDisabled);
    }
    button.with_children(|button| {
        button.spawn((
            ImageNode {
                color: if role == Surface::Dark {
                    content_color
                } else {
                    Color::WHITE
                },
                ..ImageNode::new(asset_server.load(icon_path))
            },
            Node {
                width: px(23),
                height: px(23),
                flex_shrink: 0.0,
                ..default()
            },
        ));
        button.spawn((
            Text::new(label),
            font(assets, 12.0),
            TextLayout::justify(Justify::Center),
            TextColor(content_color),
        ));
    });
}

#[allow(clippy::too_many_arguments)]
fn compact_action_button_disabled(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    index: usize,
    action: FlowUiAction,
    label: &str,
    selected: bool,
    disabled: bool,
) {
    let role = surface_role(&action);
    let mut button = parent.spawn((
        Button,
        FlowButton {
            index,
            action,
            error_action: false,
        },
        Node {
            min_width: px(94),
            min_height: px(50),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::axes(px(9), px(6)),
            ..default()
        },
        role,
        SurfaceStyle::MAP_ACTION,
        SurfaceState {
            selected,
            disabled,
            ..default()
        },
    ));
    if disabled {
        button.insert(InteractionDisabled);
    }
    button.with_child((
        Text::new(label),
        font(assets, 12.0),
        TextLayout::justify(Justify::Center),
        TextColor(INK),
    ));
}

const fn symmetry_label(symmetry: MapSymmetry) -> &'static str {
    match symmetry {
        MapSymmetry::None => "NONE",
        MapSymmetry::LeftRight => "LEFT / RIGHT",
        MapSymmetry::TopBottom => "TOP / BOTTOM",
        MapSymmetry::Rotate180 => "ROTATE 180°",
        MapSymmetry::FourWay => "FOUR-WAY",
    }
}

fn palette_label(item: Option<PaletteItem>, catalog: &crate::map::MapContentCatalog) -> String {
    match item {
        Some(PaletteItem::Asset(id)) => catalog
            .asset(id)
            .map_or("Unavailable asset".to_string(), |asset| {
                asset.display_name.clone()
            }),
        Some(PaletteItem::Spawn(0)) => "Team A Spawn".to_string(),
        Some(PaletteItem::Spawn(_)) => "Team B Spawn".to_string(),
        Some(PaletteItem::HotZone) => "Capture Circle".to_string(),
        Some(PaletteItem::HeistSafe(0)) => "Team A Safe".to_string(),
        Some(PaletteItem::HeistSafe(_)) => "Team B Safe".to_string(),
        None => "No available asset".to_string(),
    }
}

fn rotation_degrees(quarter_turns: u8) -> u16 {
    u16::from(quarter_turns % 4) * 90
}

fn root_node() -> Node {
    Node {
        width: percent(100),
        height: percent(100),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        row_gap: px(10),
        padding: UiRect::axes(px(22), px(14)),
        ..default()
    }
}

fn header(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    title: &str,
    subtitle: &str,
) {
    parent.spawn((Text::new(title), font(assets, 38.0), TextColor(CREAM)));
    parent.spawn((Text::new(subtitle), font(assets, 17.0), TextColor(TEAL)));
}

fn action_button(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    index: usize,
    action: FlowUiAction,
    label: &str,
    color: Color,
) {
    parent
        .spawn((
            Button,
            FlowButton {
                index,
                action,
                error_action: false,
            },
            Node {
                min_width: px(130),
                min_height: px(38),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(px(12), px(6)),
                border: UiRect::all(px(2)),
                ..default()
            },
            BackgroundColor(color),
            BorderColor::all(CREAM),
        ))
        .with_child((Text::new(label), font(assets, 16.0), TextColor(INK)));
}

pub(in crate::client::flow) fn return_to_editor_after_practice(
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

    #[test]
    fn held_pointer_cannot_reactivate_rebuilt_button() {
        let mut latched = false;
        assert!(accept_button_press(
            true,
            Interaction::Pressed,
            true,
            &mut latched
        ));
        assert!(latched);
        assert!(!accept_button_press(
            true,
            Interaction::Pressed,
            true,
            &mut latched
        ));
        assert!(!accept_button_press(
            true,
            Interaction::None,
            false,
            &mut latched
        ));
        assert!(!latched);
        assert!(accept_button_press(
            true,
            Interaction::Pressed,
            true,
            &mut latched
        ));
    }

    #[test]
    fn unchanged_pressed_interaction_does_not_emit_an_action() {
        let mut latched = false;
        assert!(!accept_button_press(
            false,
            Interaction::Pressed,
            true,
            &mut latched
        ));
    }

    #[test]
    fn valid_three_quarter_turn_rotation_displays_without_u8_overflow() {
        assert_eq!(rotation_degrees(3), 270);
        assert_eq!(rotation_degrees(u8::MAX), 270);
    }

    #[test]
    fn editor_control_selection_is_owned_by_runtime_state() {
        let runtime = MapMakerRuntime::default();
        assert!(runtime.action_selected(&FlowUiAction::MapMakerUsePlaceTool));
        assert!(runtime.action_selected(&FlowUiAction::MapMakerSelectAsset(0)));
        assert!(runtime.action_selected(&FlowUiAction::MapMakerToggleGrid));
        assert!(!runtime.action_selected(&FlowUiAction::MapMakerUseEraseTool));
        assert!(!runtime.action_selected(&FlowUiAction::MapMakerCycleSymmetry));
        assert_eq!(
            usize::from(PALETTE_PAGE_DELTA.unsigned_abs()),
            VISIBLE_PALETTE_ITEMS
        );
    }

    #[test]
    fn palette_page_controls_stop_at_horizontal_range_edges() {
        assert_eq!(cycled_palette_index(0, 34, -PALETTE_PAGE_DELTA), 0);
        assert_eq!(cycled_palette_index(30, 34, PALETTE_PAGE_DELTA), 30);
        assert_eq!(cycled_palette_index(7, 34, -PALETTE_PAGE_DELTA), 0);
        assert_eq!(cycled_palette_index(0, 34, -1), 33);
    }

    #[test]
    fn approved_tool_states_drive_assets_visibility() {
        let mut runtime = MapMakerRuntime::default();
        assert!(runtime.assets_open);

        activate_editor_tool(&mut runtime, EditorTool::Select);
        assert!(!runtime.assets_open);
        assert_eq!(runtime.tool, EditorTool::Select);

        toggle_assets_drawer(&mut runtime);
        assert!(runtime.assets_open);
        assert_eq!(runtime.tool, EditorTool::Place);

        toggle_assets_drawer(&mut runtime);
        assert!(!runtime.assets_open);
    }

    #[test]
    fn map_check_details_are_visible_only_while_requested() {
        let mut runtime = MapMakerRuntime::default();
        assert_eq!(
            validation_popover_error(&runtime, Some("missing spawn")),
            None
        );
        runtime.validation_open = true;
        assert_eq!(
            validation_popover_error(&runtime, Some("missing spawn")),
            Some("missing spawn")
        );
        assert_eq!(validation_popover_error(&runtime, None), None);
    }

    #[test]
    fn asset_range_uses_direct_item_windows() {
        assert_eq!(palette_window(0, 18), (0, 6));
        assert_eq!(palette_window(7, 18), (6, 12));
        assert_eq!(palette_window(17, 18), (12, 18));
        assert_eq!(palette_window(0, 3), (0, 3));
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
        let catalog = crate::map::MapContentCatalog::embedded().unwrap();
        assert_eq!(
            palette_category(PaletteItem::Asset(crate::map::GROUND_ASSET), &catalog),
            PaletteCategory::Ground
        );
        assert_eq!(
            palette_category(PaletteItem::Asset(crate::map::TALL_GRASS_ASSET), &catalog),
            PaletteCategory::Vegetation
        );
        assert_eq!(
            palette_category(PaletteItem::Asset(crate::map::SPEED_TILE_ASSET), &catalog),
            PaletteCategory::EffectTiles
        );
        assert_eq!(
            palette_category(PaletteItem::Spawn(0), &catalog),
            PaletteCategory::SpawnsObjectives
        );
    }

    #[test]
    fn hidden_palette_selection_cannot_be_placed() {
        let catalog = crate::map::MapContentCatalog::embedded().unwrap();
        let mut runtime = MapMakerRuntime::default();
        runtime.palette_query = "no asset has this name".to_string();

        assert!(
            selected_palette_item(&catalog, &runtime, crate::map::WIPEOUT_MODE_DEFINITION,)
                .is_none()
        );
    }
}
