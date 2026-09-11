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
            surface::{Surface, SurfaceState},
            theme::{CREAM, INK, ORANGE, TEAL, font},
        },
    },
    map::{
        AuthoredMapCandidate, MapAssetId, MapAssetSlot, MapCatalogResource, MapCell, MapSymmetry,
        authorable_assets,
    },
};
use bevy::{
    input::{
        ButtonState,
        keyboard::KeyboardInput,
        mouse::{AccumulatedMouseMotion, MouseWheel},
    },
    prelude::*,
    window::PrimaryWindow,
};
use lightyear::prelude::client::Client;
use std::collections::BTreeSet;

const VISIBLE_PALETTE_ITEMS: usize = 7;
const PALETTE_PAGE_DELTA: i8 = 7;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum EditorTool {
    #[default]
    Place,
    Erase,
    Move,
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
    paint_stroke: Vec<MapCell>,
    stroke_asset: Option<MapAssetId>,
    selection: BTreeSet<crate::map::MapPlacementId>,
    selection_anchor: Option<MapCell>,
    move_anchor: Option<MapCell>,
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
            paint_stroke: Vec::new(),
            stroke_asset: None,
            selection: BTreeSet::new(),
            selection_anchor: None,
            move_anchor: None,
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
        let items = palette(&catalog.0, session.document.recipe.mode_definition_id);
        let item = items
            .get(runtime.palette_index % items.len().max(1))
            .copied();
        let result = if let Some(asset_id) = runtime.stroke_asset {
            preview_session.paint_stroke(
                &catalog.0,
                asset_id,
                runtime.paint_stroke.iter().copied(),
                runtime.quarter_turns,
            )
        } else {
            item.map_or(Ok(()), |item| {
                preview_place_item(
                    &mut preview_session,
                    &catalog.0,
                    item,
                    runtime.cursor,
                    runtime.quarter_turns,
                )
            })
        };
        valid = result.is_ok();
    } else if runtime.tool == EditorTool::Move
        && let Some(anchor) = runtime.move_anchor
    {
        valid = preview_session
            .move_placements_by(
                &catalog.0,
                &runtime.selection,
                anchor,
                i32::from(runtime.cursor.x) - i32::from(anchor.x),
                i32::from(runtime.cursor.y) - i32::from(anchor.y),
            )
            .is_ok();
    }
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
                EditorTool::Move => crate::client::presentation_3d::MapEditorPointerMode::Move,
            },
            pointer_valid: valid,
            selected: runtime.selection.iter().copied().collect(),
            selection_rect: runtime
                .selection_anchor
                .map(|anchor| (anchor, runtime.cursor)),
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
            runtime.quarter_turns = (runtime.quarter_turns + 1) % 4;
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
        FlowUiAction::MapMakerUsePlaceTool => {
            runtime.tool = EditorTool::Place;
            runtime.selection.clear();
            runtime.bump_render();
        }
        FlowUiAction::MapMakerUseEraseTool => {
            runtime.tool = EditorTool::Erase;
            runtime.selection.clear();
            runtime.bump_render();
        }
        FlowUiAction::MapMakerUseMoveTool => {
            runtime.tool = EditorTool::Move;
            runtime.bump_render();
        }
        FlowUiAction::MapMakerSelectRect(first, second) => {
            if let Some(session) = runtime.session.as_ref() {
                let selected = session.placements_in_rect(&catalog.0, first, second);
                runtime.selection = session.symmetry_selection(&catalog.0, &selected);
                runtime.status = Some(format!("{} item(s) selected.", runtime.selection.len()));
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
            | FlowUiAction::MapMakerUsePlaceTool
            | FlowUiAction::MapMakerUseEraseTool
            | FlowUiAction::MapMakerUseMoveTool
            | FlowUiAction::MapMakerSelectRect(_, _)
            | FlowUiAction::MapMakerMoveSelection(_, _)
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
            runtime.palette_index = 0;
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
            FlowUiAction::MapMakerUseMoveTool => self.tool == EditorTool::Move,
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
    let count = palette(catalog, session.document.recipe.mode_definition_id).len();
    if count > 0 {
        runtime.palette_index = cycled_palette_index(runtime.palette_index, count, delta);
    }
    runtime.bump_render();
}

fn cycled_palette_index(current: usize, count: usize, delta: i8) -> usize {
    if count == 0 {
        return 0;
    }
    if usize::from(delta.unsigned_abs()) == VISIBLE_PALETTE_ITEMS {
        let page_count = count.div_ceil(VISIBLE_PALETTE_ITEMS);
        let page = (current / VISIBLE_PALETTE_ITEMS).min(page_count - 1);
        let next_page = if delta.is_negative() {
            (page + page_count - 1) % page_count
        } else {
            (page + 1) % page_count
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
    let Some(session) = runtime.session.as_mut() else {
        return;
    };
    let items = palette(catalog, session.document.recipe.mode_definition_id);
    let Some(item) = items
        .get(runtime.palette_index % items.len().max(1))
        .copied()
    else {
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
    let Some(session) = runtime.session.as_mut() else {
        return;
    };
    let items = palette(catalog, session.document.recipe.mode_definition_id);
    let Some(PaletteItem::Asset(asset_id)) = items
        .get(runtime.palette_index % items.len().max(1))
        .copied()
    else {
        runtime.status = Some("Drag painting is available for map assets only.".to_string());
        runtime.bump_render();
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
            || ((keyboard.pressed(KeyCode::Space)
                || keyboard.pressed(KeyCode::ShiftLeft)
                || keyboard.pressed(KeyCode::ShiftRight))
                && mouse.pressed(MouseButton::Left))
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
        runtime.paint_stroke.clear();
    }
    for event in mouse_wheel.read() {
        if !ui_owns_pointer {
            runtime.camera_zoom = (runtime.camera_zoom * (-event.y * 0.10).exp()).clamp(0.5, 3.0);
        }
    }
    if mouse_just_pressed && pointer_over_world && !ui_owns_pointer && !panning {
        let cursor = runtime.cursor;
        match runtime.tool {
            EditorTool::Place => {
                runtime.paint_stroke.clear();
                runtime.paint_stroke.push(cursor);
                runtime.stroke_asset = runtime
                    .session
                    .as_ref()
                    .zip(catalog.as_deref())
                    .and_then(|(session, catalog)| {
                        palette(&catalog.0, session.document.recipe.mode_definition_id)
                            .get(runtime.palette_index)
                            .copied()
                    })
                    .and_then(|item| match item {
                        PaletteItem::Asset(id) => Some(id),
                        _ => None,
                    });
            }
            EditorTool::Erase => {
                runtime.paint_stroke.clear();
                runtime.paint_stroke.push(cursor);
            }
            EditorTool::Move => {
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
                    runtime.move_anchor = Some(cursor);
                    runtime.selection_anchor = None;
                } else {
                    runtime.selection_anchor = Some(cursor);
                    runtime.move_anchor = None;
                }
            }
        }
    } else if mouse_pressed
        && pointer_over_world
        && !ui_owns_pointer
        && !panning
        && runtime.tool == EditorTool::Place
        && runtime.stroke_asset.is_some()
        && runtime.paint_stroke.last() != Some(&runtime.cursor)
    {
        let cursor = runtime.cursor;
        runtime.paint_stroke.push(cursor);
    }
    if mouse_just_released && !panning {
        match runtime.tool {
            EditorTool::Place if !runtime.paint_stroke.is_empty() => {
                let stroke = std::mem::take(&mut runtime.paint_stroke);
                if runtime.stroke_asset.take().is_some() {
                    queue_ui_action(&mut actions, FlowUiAction::MapMakerPaintStroke(stroke));
                } else {
                    queue_ui_action(&mut actions, FlowUiAction::MapMakerPlace);
                }
            }
            EditorTool::Erase if !runtime.paint_stroke.is_empty() => {
                runtime.paint_stroke.clear();
                queue_ui_action(&mut actions, FlowUiAction::MapMakerErase);
            }
            EditorTool::Move => {
                if let Some(anchor) = runtime.move_anchor.take() {
                    queue_ui_action(
                        &mut actions,
                        FlowUiAction::MapMakerMoveSelection(anchor, runtime.cursor),
                    );
                } else if let Some(anchor) = runtime.selection_anchor.take() {
                    queue_ui_action(
                        &mut actions,
                        FlowUiAction::MapMakerSelectRect(anchor, runtime.cursor),
                    );
                }
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
            EditorTool::Move => {}
        }
    }
    if keyboard.just_pressed(KeyCode::Delete)
        || keyboard.just_pressed(KeyCode::Backspace)
        || pad(GamepadButton::RightTrigger)
    {
        queue_ui_action(&mut actions, FlowUiAction::MapMakerErase);
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
    if keyboard.just_pressed(KeyCode::KeyF) {
        queue_ui_action(&mut actions, FlowUiAction::MapMakerFitView);
    }
    if keyboard.just_pressed(KeyCode::Escape) || pad(GamepadButton::East) {
        queue_ui_action(&mut actions, FlowUiAction::BackToMapLibrary);
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

#[allow(clippy::needless_pass_by_value)]
pub(in crate::client::flow) fn present_map_editor(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    runtime: Res<MapMakerRuntime>,
    catalog: Option<Res<MapCatalogResource>>,
    roots: Query<(Entity, &MapMakerRoot)>,
    assets: Option<Res<crate::client::ClientAssetHandles>>,
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
    let recipe = &session.document.recipe;
    let items = palette(&catalog.0, recipe.mode_definition_id);
    let selected = items
        .get(runtime.palette_index % items.len().max(1))
        .copied();
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
            editor_identity_panel(root, assets.as_deref(), session);
            editor_mode_panel(root, assets.as_deref(), &runtime, selected, &catalog.0);
            editor_left_rail(root, assets.as_deref(), &runtime);
            editor_right_rail(root, assets.as_deref(), &runtime);
            editor_palette(root, assets.as_deref(), &runtime, &items, &catalog.0);
            if let Some(status) = runtime.status.as_deref() {
                root.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: percent(25),
                        right: percent(25),
                        bottom: px(112),
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

fn editor_identity_panel(
    root: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    session: &MapEditorSession,
) {
    let recipe = &session.document.recipe;
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(18),
            top: px(18),
            min_width: px(220),
            flex_direction: FlexDirection::Column,
            padding: UiRect::axes(px(15), px(10)),
            border: UiRect::all(px(1)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.04, 0.07, 0.08, 0.93)),
        BorderColor::all(Color::srgba(0.32, 0.64, 0.62, 0.35)),
    ))
    .with_children(|panel| {
        panel.spawn((
            Text::new(session.document.name.to_uppercase()),
            font(assets, 20.0),
            TextColor(CREAM),
        ));
        panel.spawn((
            Text::new(format!(
                "{} × {}  •  r{}  •  {}",
                recipe.dimensions.width,
                recipe.dimensions.height,
                recipe.revision,
                if session.dirty() { "UNSAVED" } else { "SAVED" }
            )),
            font(assets, 12.0),
            TextColor(if session.dirty() { ORANGE } else { TEAL }),
        ));
    });
}

fn editor_mode_panel(
    root: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    runtime: &MapMakerRuntime,
    selected: Option<PaletteItem>,
    catalog: &crate::map::MapContentCatalog,
) {
    let symmetry = runtime
        .session
        .as_ref()
        .map_or(MapSymmetry::None, |session| session.symmetry);
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: percent(26),
            right: percent(24),
            top: px(18),
            min_height: px(66),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: px(10),
            padding: UiRect::axes(px(16), px(8)),
            border: UiRect::all(px(1)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.04, 0.07, 0.08, 0.93)),
        BorderColor::all(Color::srgba(0.32, 0.64, 0.62, 0.35)),
    ))
    .with_children(|panel| {
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Column,
                min_width: px(250),
                row_gap: px(2),
                ..default()
            })
            .with_children(|state| {
                state.spawn((
                    Text::new(format!(
                        "{}  ·  {}",
                        tool_label(runtime.tool),
                        palette_label(selected, catalog)
                    )),
                    font(assets, 17.0),
                    TextColor(CREAM),
                ));
                state.spawn((
                    MapMakerCursorCopy,
                    Text::new(format!(
                        "{}°  ·  {}  ·  CELL {},{}",
                        rotation_degrees(runtime.quarter_turns),
                        symmetry_label(symmetry),
                        runtime.cursor.x,
                        runtime.cursor.y
                    )),
                    font(assets, 12.0),
                    TextColor(TEAL),
                ));
                state.spawn((
                    Text::new("SPACE / SHIFT + DRAG TO PAN"),
                    font(assets, 10.0),
                    TextColor(Color::srgba(0.98, 0.92, 0.81, 0.68)),
                ));
            });
        compact_action_button(
            panel,
            assets,
            3,
            FlowUiAction::MapMakerRotate,
            &format!("ROTATE\n{}°", rotation_degrees(runtime.quarter_turns)),
            TEAL,
            false,
        );
        compact_action_button(
            panel,
            assets,
            4,
            FlowUiAction::MapMakerCycleSymmetry,
            &format!("SYMMETRY\n{}", symmetry_label(symmetry)),
            TEAL,
            false,
        );
    });
}

fn editor_left_rail(
    root: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    runtime: &MapMakerRuntime,
) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(18),
            top: percent(31),
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            padding: UiRect::all(px(6)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.04, 0.07, 0.08, 0.88)),
    ))
    .with_children(|rail| {
        compact_action_button(
            rail,
            assets,
            30,
            FlowUiAction::MapMakerUsePlaceTool,
            "PLACE",
            ORANGE,
            runtime.tool == EditorTool::Place,
        );
        compact_action_button(
            rail,
            assets,
            31,
            FlowUiAction::MapMakerUseEraseTool,
            "ERASE",
            Color::srgb(0.75, 0.18, 0.15),
            runtime.tool == EditorTool::Erase,
        );
        compact_action_button(
            rail,
            assets,
            32,
            FlowUiAction::MapMakerUseMoveTool,
            if runtime.tool == EditorTool::Move {
                "MOVE / SELECT"
            } else {
                "MOVE"
            },
            TEAL,
            runtime.tool == EditorTool::Move,
        );
        compact_action_button(
            rail,
            assets,
            7,
            FlowUiAction::MapMakerUndo,
            "UNDO",
            TEAL,
            false,
        );
        compact_action_button(
            rail,
            assets,
            8,
            FlowUiAction::MapMakerRedo,
            "REDO",
            TEAL,
            false,
        );
        compact_action_button(
            rail,
            assets,
            9,
            FlowUiAction::MapMakerClear,
            if runtime.clear_armed {
                "! CLEAR?"
            } else {
                "CLEAR"
            },
            Color::srgb(0.75, 0.18, 0.15),
            runtime.clear_armed,
        );
    });
}

fn editor_right_rail(
    root: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    runtime: &MapMakerRuntime,
) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: px(18),
            top: px(18),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
            row_gap: px(6),
            padding: UiRect::all(px(6)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.04, 0.07, 0.08, 0.88)),
    ))
    .with_children(|rail| {
        compact_action_button(
            rail,
            assets,
            20,
            FlowUiAction::BackToMapLibrary,
            "MAPS",
            TEAL,
            false,
        );
        compact_action_button(
            rail,
            assets,
            22,
            FlowUiAction::TestAuthoredMap,
            "TEST MAP",
            ORANGE,
            true,
        );
        compact_action_button(
            rail,
            assets,
            21,
            FlowUiAction::SaveMapDraft,
            "SAVE",
            TEAL,
            false,
        );
        compact_action_button(
            rail,
            assets,
            10,
            FlowUiAction::MapMakerToggleGrid,
            if runtime.grid_visible {
                "GRID ON"
            } else {
                "GRID OFF"
            },
            TEAL,
            runtime.grid_visible,
        );
        compact_action_button(
            rail,
            assets,
            11,
            FlowUiAction::MapMakerFitView,
            "FIT MAP",
            TEAL,
            false,
        );
    });
}

fn editor_palette(
    root: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    runtime: &MapMakerRuntime,
    items: &[PaletteItem],
    catalog: &crate::map::MapContentCatalog,
) {
    let theme = runtime
        .session
        .as_ref()
        .map_or(crate::map::MapPresentationThemeId(1), |session| {
            session.document.recipe.presentation_theme_id
        });
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(112),
            right: px(112),
            bottom: px(12),
            min_height: px(118),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: px(8),
            padding: UiRect::axes(px(14), px(10)),
            border: UiRect::all(px(1)),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(Color::srgba(0.04, 0.07, 0.08, 0.94)),
        BorderColor::all(Color::srgba(0.32, 0.64, 0.62, 0.35)),
    ))
    .with_children(|tray| {
        compact_action_button(
            tray,
            assets,
            1,
            FlowUiAction::MapMakerCycleAsset(-PALETTE_PAGE_DELTA),
            "PREV PAGE",
            TEAL,
            false,
        );
        let start = (runtime.palette_index / VISIBLE_PALETTE_ITEMS) * VISIBLE_PALETTE_ITEMS;
        for (index, item) in items
            .iter()
            .copied()
            .enumerate()
            .skip(start)
            .take(VISIBLE_PALETTE_ITEMS)
        {
            let selected = runtime.palette_index == index;
            palette_item_button(
                tray,
                assets,
                100 + index,
                FlowUiAction::MapMakerSelectAsset(index),
                &palette_short_label(item, catalog),
                MapPalettePreviewHost {
                    slot: index - start,
                    visual: item.into(),
                    theme,
                },
                selected,
            );
        }
        let page_count = items.len().div_ceil(VISIBLE_PALETTE_ITEMS).max(1);
        tray.spawn((
            Text::new(format!(
                "PAGE {} / {}\nITEM {} / {}",
                runtime.palette_index / VISIBLE_PALETTE_ITEMS + 1,
                page_count,
                runtime.palette_index.saturating_add(1),
                items.len()
            )),
            font(assets, 11.0),
            TextLayout::justify(Justify::Center),
            TextColor(ORANGE),
        ));
        compact_action_button(
            tray,
            assets,
            2,
            FlowUiAction::MapMakerCycleAsset(PALETTE_PAGE_DELTA),
            "NEXT PAGE",
            TEAL,
            false,
        );
    });
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
                width: px(116),
                min_height: px(98),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(px(5), px(6)),
                row_gap: px(3),
                ..default()
            },
            Surface::Card,
            SurfaceState {
                selected,
                ..default()
            },
        ))
        .with_children(|button| {
            button.spawn((
                preview,
                Node {
                    width: px(106),
                    height: px(66),
                    ..default()
                },
            ));
            button.spawn((
                Text::new(label.replace('\n', " · ")),
                font(assets, 11.0),
                TextLayout::justify(Justify::Center),
                TextColor(INK),
            ));
        });
}

fn palette_short_label(item: PaletteItem, catalog: &crate::map::MapContentCatalog) -> String {
    match item {
        PaletteItem::Asset(id) => catalog.asset(id).map_or_else(
            || "? ASSET".to_string(),
            |asset| {
                let category = match asset.slot {
                    MapAssetSlot::Surface => "SURFACE",
                    MapAssetSlot::Feature => "WALL",
                    MapAssetSlot::Decoration => "PROP",
                    MapAssetSlot::Marker => "MARKER",
                };
                format!("{category}\n{}", asset.display_name)
            },
        ),
        PaletteItem::Spawn(0) => "SPAWN\nTEAM A".to_string(),
        PaletteItem::Spawn(_) => "SPAWN\nTEAM B".to_string(),
        PaletteItem::HotZone => "OBJECTIVE\nZONE".to_string(),
        PaletteItem::HeistSafe(0) => "OBJECTIVE\nA SAFE".to_string(),
        PaletteItem::HeistSafe(_) => "OBJECTIVE\nB SAFE".to_string(),
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
    let role = surface_role(&action);
    parent
        .spawn((
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
            SurfaceState {
                selected,
                ..default()
            },
        ))
        .with_child((
            Text::new(label),
            font(assets, 12.0),
            TextLayout::justify(Justify::Center),
            TextColor(INK),
        ));
}

const fn tool_label(tool: EditorTool) -> &'static str {
    match tool {
        EditorTool::Place => "PLACE",
        EditorTool::Erase => "ERASE",
        EditorTool::Move => "MOVE / SELECT",
    }
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
    fn palette_page_controls_wrap_between_page_starts() {
        assert_eq!(cycled_palette_index(0, 34, -PALETTE_PAGE_DELTA), 28);
        assert_eq!(cycled_palette_index(28, 34, PALETTE_PAGE_DELTA), 0);
        assert_eq!(cycled_palette_index(7, 34, -PALETTE_PAGE_DELTA), 0);
        assert_eq!(cycled_palette_index(0, 34, -1), 33);
    }
}
