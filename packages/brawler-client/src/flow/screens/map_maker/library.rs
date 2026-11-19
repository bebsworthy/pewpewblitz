//! My Maps: the lighter-shell library (BRL-2xpc direction D). Shell Back, title and orange
//! primary actions around flat map tiles; the selected map's actions act on one tile; create and
//! delete are flat dialogs on their own layer. A filter switches to the maps this account
//! published to the connected server, each tile with its server-reported state chip, and a
//! selected pending or approved one can be withdrawn (BRL-s3dh).

use super::{
    MapMakerRoot, MapMakerRuntime, PaletteCategory,
    check::map_check,
    create_defaults,
    scroll::{MapMakerScroll, MapMakerScrollMemory, ScrollArea},
    widgets::{ButtonSpec, DANGER, Kit, MetricsParam, RADIUS, Tone, bar_fill, column, grow, row},
};
use crate::{
    ClientLobbyMembership, ClientPublishedMapsModel, MapDraftDocument, MapDraftLibrary,
    flow::{
        ClientFlow, SelectedGameType,
        actions::{FlowUiAction, PendingFlowActions},
        input::queue_ui_action,
        screens::{
            connection_shell::{spawn_action, spawn_background},
            shared::{FlowButton, FlowNavigation, FlowRoot},
        },
    },
    ui::{
        feedback::UiFeedback,
        surface::Surface,
        theme::{ACCENT, INK, PANEL, PLAY_HIGHLIGHT, TEXT, shell_background},
    },
};
use bevy::{
    prelude::*,
    ui::{FocusPolicy, InteractionDisabled},
};
use brawler::lobby::{PublishedMapId, PublishedMapListing, PublishedMapSummary};
use brawler::map::{MapCatalogResource, MapContentCatalog, MapModeAnchorKind, MapRecipe};
use lightyear::prelude::client::Client;

/// Library control indices: the keyboard/controller focus order and the burst key.
mod index {
    pub(super) const BACK: usize = 0;
    pub(super) const CREATE: usize = 1;
    pub(super) const SHOW_DEVICE: usize = 2;
    pub(super) const SHOW_PUBLISHED: usize = 3;
    pub(super) const EDIT: usize = 10;
    pub(super) const RENAME: usize = 11;
    pub(super) const DUPLICATE: usize = 12;
    pub(super) const PUBLISH: usize = 13;
    pub(super) const DELETE: usize = 14;
    pub(super) const RENAME_CANCEL: usize = 15;
    pub(super) const RENAME_SAVE: usize = 16;
    pub(super) const WITHDRAW: usize = 17;
    pub(super) const DIALOG_CANCEL: usize = 20;
    pub(super) const DIALOG_CONFIRM: usize = 21;
    pub(super) const FIRST_TILE: usize = 100;
}

/// A button on the open create or delete dialog: while a dialog is open only these take focus.
#[derive(Component)]
pub(in crate::flow) struct MapLibraryDialogButton;

/// The typed name in the create or rename field; typing retexts it without a rebuild.
#[derive(Component)]
pub(in crate::flow) struct MapNameFieldText;

#[allow(clippy::needless_pass_by_value, reason = "Bevy system parameters")]
pub(in crate::flow) fn update_map_library_values(
    runtime: Res<MapMakerRuntime>,
    mut fields: Query<&mut Text, With<MapNameFieldText>>,
) {
    if !runtime.is_changed() {
        return;
    }
    let Some(name) = runtime.editing_map_name() else {
        return;
    };
    for mut text in &mut fields {
        if text.0 != name {
            text.0 = name.to_string();
        }
    }
}

/// Keeps the library's selected map pointing at a saved draft (the first one by default).
#[allow(clippy::needless_pass_by_value, reason = "Bevy system parameters")]
pub(in crate::flow) fn keep_map_selection_valid(
    library: Res<MapDraftLibrary>,
    mut runtime: ResMut<MapMakerRuntime>,
) {
    let valid = runtime
        .selected_draft
        .filter(|id| library.drafts.contains_key(id))
        .or_else(|| library.drafts.keys().next().copied());
    if runtime.selected_draft != valid {
        runtime.selected_draft = valid;
        runtime.bump_render();
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "the library view reads independent Bevy-owned resources"
)]
pub(in crate::flow) fn present_map_library(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    library: Res<MapDraftLibrary>,
    runtime: Res<MapMakerRuntime>,
    catalog: Option<Res<MapCatalogResource>>,
    roots: Query<(Entity, &MapMakerRoot)>,
    assets: Option<Res<crate::ClientAssetHandles>>,
    server: Res<AssetServer>,
    metrics: MetricsParam,
    scroll: Res<MapMakerScrollMemory>,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    selection: Res<SelectedGameType>,
    published: Res<ClientPublishedMapsModel>,
) {
    if *flow.get() != ClientFlow::MapLibrary {
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
    let kit = Kit {
        metrics,
        server: &server,
        assets: assets.as_deref(),
    };
    let catalog = catalog.as_deref().map(|catalog| &catalog.0);
    commands
        .spawn((
            FlowRoot,
            key,
            DespawnOnExit(ClientFlow::MapLibrary),
            Node {
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(kit.pick(22.0, 20.0))),
                ..column(kit.pick(14.0, 12.0))
            },
            shell_background(),
            GlobalZIndex(420),
        ))
        .with_children(|root| {
            spawn_background(root, Some(&server));
            header(root, &kit, &library, &runtime, published.maps());
            root.spawn((
                Node {
                    width: percent(100),
                    flex_grow: 1.0,
                    min_height: px(0),
                    padding: UiRect::all(px(kit.pick(18.0, 16.0))),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(14)),
                    ..column(kit.pick(12.0, 12.0))
                },
                BackgroundColor(INK.with_alpha(0.55)),
                BorderColor::all(ACCENT.with_alpha(0.25)),
            ))
            .with_children(|body| {
                if let Some(status) = runtime.status.as_deref() {
                    status_line(body, &kit, status);
                }
                if runtime.published_view {
                    published_body(
                        body,
                        &kit,
                        &published,
                        memberships.iter().next(),
                        &runtime,
                        &scroll,
                    );
                } else if library.drafts.is_empty() {
                    empty_state(body, &kit);
                } else {
                    tiles(body, &kit, &library, catalog, &scroll);
                    let selected = runtime
                        .selected_draft
                        .and_then(|id| library.drafts.get(&id));
                    if let Some(document) = selected {
                        action_bar(body, &kit, document, &runtime);
                    }
                }
            });
        });
    if let Some(name) = runtime.creating.as_deref() {
        let defaults = memberships
            .iter()
            .next()
            .zip(catalog)
            .ok_or_else(|| "Connect to a server to create a map.".to_string())
            .and_then(|(membership, catalog)| create_defaults(membership, &selection, catalog));
        dialog(&mut commands, key, &kit, |dialog| {
            create_dialog(dialog, &kit, name, &defaults);
        });
    } else if let Some(document) = runtime
        .pending_delete
        .and_then(|id| library.drafts.get(&id))
    {
        dialog(&mut commands, key, &kit, |dialog| {
            delete_dialog(dialog, &kit, document);
        });
    } else if let Some(map) = runtime.pending_withdraw.and_then(|id| published.map(id)) {
        dialog(&mut commands, key, &kit, |dialog| {
            withdraw_dialog(dialog, &kit, map);
        });
    }
}

/// The header's two library views: drafts saved on this device, and maps published to this
/// server.
fn view_filter(
    header: &mut ChildSpawnerCommands,
    kit: &Kit,
    drafts: usize,
    published: Option<&[PublishedMapSummary]>,
) {
    let device = format!("On this device ({drafts})");
    let published = published.map_or_else(
        || "Published".to_string(),
        |maps| format!("Published ({})", maps.len()),
    );
    kit.segmented(
        header,
        false,
        vec![
            ButtonSpec::new(
                index::SHOW_DEVICE,
                FlowUiAction::ShowPublishedMaps(false),
                Tone::Quiet,
            )
            .label(&device)
            .width(px(kit.pick(200.0, 250.0))),
            ButtonSpec::new(
                index::SHOW_PUBLISHED,
                FlowUiAction::ShowPublishedMaps(true),
                Tone::Quiet,
            )
            .label(&published)
            .width(px(kit.pick(180.0, 220.0))),
        ],
    );
}

/// The Published view: one tile per submission (newest first) with its state chip, and the
/// selected map's Withdraw action.
fn published_body(
    body: &mut ChildSpawnerCommands,
    kit: &Kit,
    published: &ClientPublishedMapsModel,
    membership: Option<&ClientLobbyMembership>,
    runtime: &MapMakerRuntime,
    scroll: &MapMakerScrollMemory,
) {
    let maps = match (membership, published.maps()) {
        (None, _) => {
            published_note(
                body,
                kit,
                "Connect to a server to see the maps you published to it.",
            );
            return;
        }
        (Some(_), None) => {
            published_note(body, kit, "Loading the maps you published to this server…");
            return;
        }
        (Some(_), Some([])) => {
            published_note(
                body,
                kit,
                "Nothing published to this server yet. Select a map on this device and press Publish.",
            );
            return;
        }
        (Some(_), Some(maps)) => maps,
    };
    let games = membership.map_or(&[][..], |membership| membership.game_types.as_slice());
    let tile = kit.pick(274.0, 318.0);
    let gap = 14.0;
    body.spawn((
        MapMakerScroll(ScrollArea::Library),
        scroll.position(ScrollArea::Library),
        Node {
            width: percent(100),
            flex_grow: 1.0,
            min_height: px(0),
            flex_wrap: FlexWrap::Wrap,
            align_content: AlignContent::FlexStart,
            align_items: AlignItems::FlexStart,
            row_gap: px(gap),
            overflow: Overflow::scroll_y(),
            ..row(gap)
        },
    ))
    .with_children(|tiles| {
        for (position, map) in maps.iter().enumerate() {
            published_tile(tiles, kit, position, map, games, tile);
        }
    });
    if let Some(map) = runtime.selected_published.and_then(|id| published.map(id)) {
        published_action_bar(body, kit, map, published.pending());
    }
}

fn published_note(body: &mut ChildSpawnerCommands, kit: &Kit, copy: &str) {
    body.spawn(Node {
        width: percent(100),
        flex_grow: 1.0,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..column(kit.pick(14.0, 12.0))
    })
    .with_children(|note| {
        kit.icon(note, "arena", kit.pick(64.0, 72.0), 1.0);
        kit.label(note, copy, kit.pick(17.0, 21.0), TEXT);
    });
}

/// The chip colour of a published map's state.
fn listing_color(listing: &PublishedMapListing) -> Color {
    match listing {
        PublishedMapListing::Pending => PLAY_HIGHLIGHT,
        PublishedMapListing::Approved { in_rotation: true } => ACCENT,
        PublishedMapListing::Approved { in_rotation: false } => TEXT.with_alpha(0.8),
        PublishedMapListing::Rejected { .. } => DANGER.lighter(0.15),
        PublishedMapListing::Removed { .. } => TEXT.with_alpha(0.55),
    }
}

/// The game type a map was published for: its mode pennant and display name (which names the
/// mode already).
fn published_game(
    map: &PublishedMapSummary,
    games: &[brawler::lobby::AdvertisedGameType],
) -> (&'static str, String) {
    games
        .iter()
        .find(|game| game.id == map.game_type_id)
        .map_or_else(
            || ("arena", map.game_type_id.as_str().to_string()),
            |game| {
                let (_, pennant) = mode_identity(game.mode_definition_id);
                (pennant, game.display_name.clone())
            },
        )
}

fn published_tile(
    tiles: &mut ChildSpawnerCommands,
    kit: &Kit,
    position: usize,
    map: &PublishedMapSummary,
    games: &[brawler::lobby::AdvertisedGameType],
    width: f32,
) {
    let id = kit.button(
        tiles,
        ButtonSpec::new(
            index::FIRST_TILE + position,
            FlowUiAction::SelectPublishedMap(map.published_map_id),
            Tone::Tile,
        )
        .named(&map.name)
        .width(px(width)),
    );
    tiles
        .commands()
        .entity(id)
        .entry::<Node>()
        .and_modify(|mut node| {
            node.flex_direction = FlexDirection::Column;
            node.align_self = AlignSelf::FlexStart;
            node.flex_grow = 0.0;
            node.align_items = AlignItems::Stretch;
            node.justify_content = JustifyContent::FlexStart;
            node.padding = UiRect::all(px(12));
            node.row_gap = px(8);
            node.border = UiRect::all(px(3));
            node.border_radius = BorderRadius::all(px(10));
        });
    let (pennant, game) = published_game(map, games);
    tiles.commands().entity(id).with_children(|card| {
        status_chip(card, kit, &map.listing);
        card.spawn((
            Node {
                overflow: Overflow::clip_x(),
                ..default()
            },
            children![(
                Text::new(map.name.clone()),
                crate::ui::theme::font(kit.assets, kit.pick(22.0, 25.0)),
                TextColor(TEXT),
                crate::ui::theme::type_stroke(kit.pick(13.0, 15.0)),
                super::widgets::no_wrap(),
            )],
        ));
        card.spawn((Node {
            overflow: Overflow::clip_x(),
            ..row(6.0)
        },))
            .with_children(|line| {
                pennant_icon(line, kit, pennant, kit.pick(22.0, 26.0));
                kit.line(line, &game, kit.pick(14.0, 17.0), TEXT.with_alpha(0.85));
            });
        kit.label(
            card,
            &crate::published_map_detail(&map.listing),
            kit.pick(14.0, 17.0),
            listing_color(&map.listing),
        );
    });
}

/// A small filled chip naming the map's state.
fn status_chip(parent: &mut ChildSpawnerCommands, kit: &Kit, listing: &PublishedMapListing) {
    let color = listing_color(listing);
    parent
        .spawn((
            Node {
                align_self: AlignSelf::FlexStart,
                padding: UiRect::axes(px(10), px(3)),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(RADIUS)),
                ..default()
            },
            BackgroundColor(color.with_alpha(0.22)),
            BorderColor::all(color),
        ))
        .with_children(|chip| {
            kit.line(
                chip,
                crate::published_map_chip(listing),
                kit.pick(13.0, 16.0),
                TEXT,
            );
        });
}

fn published_action_bar(
    body: &mut ChildSpawnerCommands,
    kit: &Kit,
    map: &PublishedMapSummary,
    request_in_flight: bool,
) {
    body.spawn(Node {
        width: percent(100),
        flex_shrink: 0.0,
        min_height: px(kit.ctl()),
        ..row(kit.pick(10.0, 12.0))
    })
    .with_children(|bar| {
        bar.spawn(Node {
            flex_grow: 1.0,
            flex_shrink: 1.0,
            min_width: px(0),
            overflow: Overflow::clip_x(),
            ..column(2.0)
        })
        .with_children(|copy| {
            kit.label(copy, "SELECTED", kit.pick(12.0, 15.0), ACCENT);
            copy.spawn((
                Text::new(map.name.clone()),
                crate::ui::theme::font(kit.assets, kit.pick(22.0, 24.0)),
                TextColor(TEXT),
                crate::ui::theme::type_stroke(kit.pick(13.0, 14.0)),
                super::widgets::no_wrap(),
            ));
        });
        if map.listing.withdrawable() {
            kit.button(
                bar,
                ButtonSpec::new(
                    index::WITHDRAW,
                    FlowUiAction::WithdrawPublishedMap(map.published_map_id),
                    Tone::Danger,
                )
                .icon("trash")
                .label("Withdraw")
                .disabled(request_in_flight),
            );
        }
    });
}

fn withdraw_dialog(dialog: &mut ChildSpawnerCommands, kit: &Kit, map: &PublishedMapSummary) {
    kit.title(dialog, "WITHDRAW MAP?", kit.pick(32.0, 36.0));
    kit.label(
        dialog,
        &format!(
            "“{}” leaves review and this server's match rotation. Matches already running keep it. To offer it again, publish it again.",
            map.name
        ),
        kit.pick(16.0, 20.0),
        TEXT,
    );
    dialog_actions(
        dialog,
        kit,
        FlowUiAction::CancelWithdrawPublishedMap,
        (
            FlowUiAction::WithdrawPublishedMap(map.published_map_id),
            "Withdraw",
            Tone::Danger,
        ),
        false,
    );
}

/// The Published view's actions (BRL-s3dh). Withdraw opens a confirmation first; pressed again
/// from it, it asks the server, which alone decides whether the map is the caller's.
pub(super) fn resolve_published_map_action(
    action: &FlowUiAction,
    runtime: &mut MapMakerRuntime,
    published: &mut ClientPublishedMapsModel,
) {
    match action {
        FlowUiAction::ShowPublishedMaps(show) => {
            runtime.published_view = *show;
            runtime.pending_withdraw = None;
            runtime.pending_delete = None;
            runtime.renaming = None;
            runtime.status = None;
            // Operator decisions arrive between visits; re-list each time the view opens.
            runtime.refresh_published |= *show;
        }
        FlowUiAction::SelectPublishedMap(id) if published.map(*id).is_some() => {
            runtime.selected_published = Some(*id);
            runtime.status = None;
        }
        FlowUiAction::WithdrawPublishedMap(id)
            if published
                .map(*id)
                .is_some_and(|map| map.listing.withdrawable()) =>
        {
            if runtime.pending_withdraw == Some(*id) {
                runtime.pending_withdraw = None;
                runtime.status = Some(if published.withdraw(*id) {
                    "Withdrawing…".to_string()
                } else {
                    "Wait for the server to answer, then try again.".to_string()
                });
            } else {
                runtime.selected_published = Some(*id);
                runtime.pending_withdraw = Some(*id);
                runtime.status = None;
            }
        }
        FlowUiAction::CancelWithdrawPublishedMap => runtime.pending_withdraw = None,
        _ => return,
    }
    runtime.bump_render();
}

/// Keeps the Published listing current while My Maps is open and shows the server's answers.
pub(super) fn observe_published_maps(
    flow: ClientFlow,
    published: &mut ClientPublishedMapsModel,
    runtime: &mut MapMakerRuntime,
) {
    if flow == ClientFlow::MapLibrary
        && (published.needs_list() || runtime.refresh_published)
        && published.request_list()
    {
        runtime.refresh_published = false;
    }
    if let Some(reason) = published.take_rejection() {
        runtime.status = Some(crate::published_maps_rejection_text(reason).to_string());
        runtime.bump_render();
    }
    if let Some(id) = published.take_withdrawn() {
        let name = published
            .map(id)
            .map_or_else(|| "The map".to_string(), |map| format!("“{}”", map.name));
        runtime.status = Some(format!(
            "{name} was withdrawn. It is out of review and rotation."
        ));
    }
    if published.take_changed() {
        let valid = runtime
            .selected_published
            .filter(|id| published.map(*id).is_some())
            .or_else(|| {
                published
                    .maps()
                    .and_then(<[PublishedMapSummary]>::first)
                    .map(|map| map.published_map_id)
            });
        runtime.selected_published = valid;
        if runtime
            .pending_withdraw
            .is_some_and(|id: PublishedMapId| published.map(id).is_none())
        {
            runtime.pending_withdraw = None;
        }
        runtime.bump_render();
    }
}

fn header(
    root: &mut ChildSpawnerCommands,
    kit: &Kit,
    library: &MapDraftLibrary,
    runtime: &MapMakerRuntime,
    published: Option<&[PublishedMapSummary]>,
) {
    let published_view = runtime.published_view;
    root.spawn(Node {
        width: percent(100),
        ..row(20.0)
    })
    .with_children(|header| {
        shell_action(
            header,
            kit,
            index::BACK,
            FlowUiAction::CloseMapMaker,
            "BACK",
            Surface::Back,
            Val::Auto,
        );
        header.spawn(column(0.0)).with_children(|copy| {
            kit.title(copy, "MY MAPS", kit.pick(44.0, 46.0));
            let saved = match library.drafts.len() {
                _ if published_view => "Map Maker · published to this server".to_string(),
                0 => "Map Maker · saved on this device".to_string(),
                1 => "Map Maker · 1 map saved on this device".to_string(),
                count => format!("Map Maker · {count} maps saved on this device"),
            };
            kit.label(copy, &saved, kit.pick(15.0, 19.0), ACCENT);
        });
        header.spawn(grow());
        view_filter(header, kit, library.drafts.len(), published);
        if !library.drafts.is_empty() && !published_view {
            shell_action(
                header,
                kit,
                index::CREATE,
                FlowUiAction::CreateMap,
                "+ CREATE MAP",
                Surface::Primary,
                px(kit.pick(250.0, 290.0)),
            );
        }
    });
}

/// A shell action (Back, Create, Edit) sized for this layout.
fn shell_action(
    parent: &mut ChildSpawnerCommands,
    kit: &Kit,
    index: usize,
    action: FlowUiAction,
    label: &str,
    role: Surface,
    width: Val,
) -> Entity {
    spawn_action(
        parent,
        kit.assets,
        index,
        action,
        label,
        Some(role),
        false,
        Node {
            width,
            min_height: px(kit.pick(56.0, kit.ctl())),
            flex_shrink: 0.0,
            padding: UiRect::axes(px(kit.pick(22.0, 26.0)), px(8)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
    )
}

fn status_line(body: &mut ChildSpawnerCommands, kit: &Kit, status: &str) {
    body.spawn((
        Node {
            width: percent(100),
            padding: UiRect::axes(px(12), px(6)),
            border: UiRect::left(px(4)),
            border_radius: BorderRadius::all(px(RADIUS)),
            ..row(8.0)
        },
        BackgroundColor(bar_fill()),
        BorderColor::all(PLAY_HIGHLIGHT),
    ))
    .with_children(|line| {
        kit.icon(line, "warning", kit.pick(18.0, 26.0), 1.0);
        kit.label(line, status, kit.pick(15.0, 19.0), TEXT);
    });
}

fn tiles(
    body: &mut ChildSpawnerCommands,
    kit: &Kit,
    library: &MapDraftLibrary,
    catalog: Option<&MapContentCatalog>,
    scroll: &MapMakerScrollMemory,
) {
    let tile = kit.pick(274.0, 318.0);
    let gap = 14.0;
    body.spawn((
        MapMakerScroll(ScrollArea::Library),
        scroll.position(ScrollArea::Library),
        Node {
            width: percent(100),
            flex_grow: 1.0,
            min_height: px(0),
            flex_wrap: FlexWrap::Wrap,
            align_content: AlignContent::FlexStart,
            align_items: AlignItems::FlexStart,
            row_gap: px(gap),
            overflow: Overflow::scroll_y(),
            ..row(gap)
        },
    ))
    .with_children(|tiles| {
        for (position, document) in library.drafts.values().enumerate() {
            map_tile(tiles, kit, position, document, catalog, tile);
        }
    });
}

fn map_tile(
    tiles: &mut ChildSpawnerCommands,
    kit: &Kit,
    position: usize,
    document: &MapDraftDocument,
    catalog: Option<&MapContentCatalog>,
    width: f32,
) {
    let recipe = &document.recipe;
    let (mode, pennant) = mode_identity(recipe.mode_definition_id);
    let id = kit.button(
        tiles,
        ButtonSpec::new(
            index::FIRST_TILE + position,
            FlowUiAction::SelectMapDraft(document.id),
            Tone::Tile,
        )
        .named(&document.name)
        .width(px(width)),
    );
    tiles
        .commands()
        .entity(id)
        .entry::<Node>()
        .and_modify(|mut node| {
            node.flex_direction = FlexDirection::Column;
            node.align_self = AlignSelf::FlexStart;
            node.flex_grow = 0.0;
            node.align_items = AlignItems::Stretch;
            node.justify_content = JustifyContent::FlexStart;
            node.padding = UiRect::all(px(10));
            node.row_gap = px(6);
            node.border = UiRect::all(px(3));
            node.border_radius = BorderRadius::all(px(10));
        });
    tiles.commands().entity(id).with_children(|card| {
        overview(card, recipe, catalog, width - 26.0);
        card.spawn((
            Node {
                overflow: Overflow::clip_x(),
                ..default()
            },
            children![(
                Text::new(document.name.clone()),
                crate::ui::theme::font(kit.assets, kit.pick(22.0, 25.0)),
                TextColor(TEXT),
                crate::ui::theme::type_stroke(kit.pick(13.0, 15.0)),
                super::widgets::no_wrap(),
            )],
        ));
        card.spawn(row(6.0)).with_children(|line| {
            pennant_icon(line, kit, pennant, kit.pick(22.0, 26.0));
            kit.line(
                line,
                &format!(
                    "{mode}  ·  {} × {}",
                    recipe.dimensions.width, recipe.dimensions.height
                ),
                kit.pick(14.0, 17.0),
                TEXT.with_alpha(0.85),
            );
        });
        if let Some(catalog) = catalog {
            let check = map_check(recipe, catalog);
            let (icon, copy, color) = match check.issue_count() {
                0 if check.ready => ("check", "Ready to test".to_string(), ACCENT),
                0 | 1 => ("warning", "1 issue".to_string(), PLAY_HIGHLIGHT),
                count => ("warning", format!("{count} issues"), PLAY_HIGHLIGHT),
            };
            card.spawn(row(6.0)).with_children(|line| {
                kit.icon(line, icon, kit.pick(16.0, 20.0), 1.0);
                kit.line(line, &copy, kit.pick(14.0, 17.0), color);
            });
        }
    });
}

pub(super) fn pennant_icon(parent: &mut ChildSpawnerCommands, kit: &Kit, key: &str, height: f32) {
    parent.spawn((
        ImageNode::new(kit.server.load(crate::ui::icons::path(key))),
        Node {
            width: px(height * 250.0 / 310.0),
            height: px(height),
            flex_shrink: 0.0,
            ..default()
        },
    ));
}

/// The draft's mode name and pennant icon.
pub(super) fn mode_identity(mode: brawler::map::ModeDefinitionId) -> (&'static str, &'static str) {
    match mode {
        brawler::map::HOT_ZONE_MODE_DEFINITION => ("Hot Zone", "mode-hot-zone"),
        brawler::map::HEIST_MODE_DEFINITION => ("Heist", "mode-heist"),
        brawler::map::ZOMBIE_MODE_DEFINITION => ("Zombie", "mode-zombie"),
        _ => ("Wipeout", "mode-wipeout"),
    }
}

/// The blank-arena ground colour of an overview.
const ARENA_GROUND: Color = Color::srgb(0.722, 0.784, 0.690);
/// At most this many recipe items are drawn in one overview.
const OVERVIEW_ITEM_LIMIT: usize = 320;

/// One flat footprint in an overview, in cells.
struct OverviewMark {
    min: (u16, u16),
    size: (u16, u16),
    color: Color,
    round: bool,
}

/// The recipe's filled rectangles, placements and objectives as flat footprints in their
/// palette colours, at most `OVERVIEW_ITEM_LIMIT` of them.
fn overview_marks(recipe: &MapRecipe, catalog: &MapContentCatalog) -> Vec<OverviewMark> {
    let dimensions = recipe.dimensions;
    let filled = recipe.filled_rects.iter().map(|filled| OverviewMark {
        min: (filled.min.x, filled.min.y),
        size: (filled.width, filled.height),
        color: super::palette_color(super::PaletteItem::Asset(filled.asset_id), catalog),
        round: false,
    });
    let placements = recipe.placements.iter().filter_map(|placement| {
        let item = match placement.parameters {
            brawler::map::MapPlacementParameters::PlayerSpawn { team_slot, .. } => {
                super::PaletteItem::Spawn(team_slot)
            }
            brawler::map::MapPlacementParameters::None => {
                super::PaletteItem::Asset(placement.asset_id)
            }
        };
        let cells = catalog
            .asset(placement.asset_id)
            .and_then(|asset| brawler::map::placement_cells(dimensions, asset, placement))
            .unwrap_or_else(|| vec![placement.cell]);
        let min_x = cells.iter().map(|cell| cell.x).min()?;
        let min_y = cells.iter().map(|cell| cell.y).min()?;
        let max_x = cells.iter().map(|cell| cell.x).max()?;
        let max_y = cells.iter().map(|cell| cell.y).max()?;
        Some(OverviewMark {
            min: (min_x, min_y),
            size: (max_x - min_x + 1, max_y - min_y + 1),
            color: super::palette_color(item, catalog),
            round: matches!(item, super::PaletteItem::Spawn(_)),
        })
    });
    let anchors = recipe
        .mode_anchors
        .iter()
        .filter_map(|anchor| match anchor.kind {
            MapModeAnchorKind::HotZoneCircle {
                center_half_cell,
                radius_half_cells,
            } => {
                let radius = radius_half_cells / 2;
                Some(OverviewMark {
                    min: (
                        (center_half_cell.x / 2).saturating_sub(radius),
                        (center_half_cell.y / 2).saturating_sub(radius),
                    ),
                    size: (radius * 2, radius * 2),
                    color: PLAY_HIGHLIGHT.with_alpha(0.6),
                    round: true,
                })
            }
            MapModeAnchorKind::HeistSafe {
                team_slot,
                origin_cell,
                width_cells,
                height_cells,
                ..
            } => Some(OverviewMark {
                min: (origin_cell.x, origin_cell.y),
                size: (width_cells, height_cells),
                color: super::palette_color(super::PaletteItem::HeistSafe(team_slot), catalog),
                round: false,
            }),
            MapModeAnchorKind::ZombieIngress { .. } => None,
        });
    filled
        .chain(placements)
        .chain(anchors)
        .take(OVERVIEW_ITEM_LIMIT)
        .collect()
}

/// A recipe-derived overview: the arena ground with every placement, filled rectangle and
/// objective drawn as a flat footprint in its palette colour. A disposable presentation of the
/// saved recipe; nothing is cached or stored.
fn overview(
    parent: &mut ChildSpawnerCommands,
    recipe: &MapRecipe,
    catalog: Option<&MapContentCatalog>,
    width: f32,
) {
    let dimensions = recipe.dimensions;
    let aspect = f32::from(dimensions.width) / f32::from(dimensions.height.max(1));
    let cells_wide = f32::from(dimensions.width.max(1));
    let cells_tall = f32::from(dimensions.height.max(1));
    let marks = catalog.map_or_else(Vec::new, |catalog| overview_marks(recipe, catalog));
    parent
        .spawn((
            Node {
                width: px(width),
                height: px(width / aspect.max(0.5)),
                flex_shrink: 0.0,
                border_radius: BorderRadius::all(px(4)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(ARENA_GROUND),
        ))
        .with_children(|map| {
            for mark in marks {
                map.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: percent(f32::from(mark.min.0) * 100.0 / cells_wide),
                        top: percent(f32::from(mark.min.1) * 100.0 / cells_tall),
                        width: percent(f32::from(mark.size.0.max(1)) * 100.0 / cells_wide),
                        height: percent(f32::from(mark.size.1.max(1)) * 100.0 / cells_tall),
                        border_radius: if mark.round {
                            BorderRadius::MAX
                        } else {
                            BorderRadius::ZERO
                        },
                        ..default()
                    },
                    BackgroundColor(mark.color),
                ));
            }
        });
}

fn action_bar(
    body: &mut ChildSpawnerCommands,
    kit: &Kit,
    document: &MapDraftDocument,
    runtime: &MapMakerRuntime,
) {
    let renaming = runtime
        .renaming
        .as_ref()
        .filter(|(id, _)| *id == document.id)
        .map(|(_, name)| name.as_str());
    body.spawn(Node {
        width: percent(100),
        flex_shrink: 0.0,
        ..row(kit.pick(10.0, 12.0))
    })
    .with_children(|bar| {
        bar.spawn(Node {
            flex_grow: 1.0,
            flex_shrink: 1.0,
            min_width: px(0),
            overflow: Overflow::clip_x(),
            ..column(2.0)
        })
        .with_children(|copy| {
            if let Some(name) = renaming {
                kit.label(copy, "RENAME", kit.pick(12.0, 15.0), ACCENT);
                name_field(copy, kit, name, px(kit.pick(360.0, 470.0)));
            } else {
                kit.label(copy, "SELECTED", kit.pick(12.0, 15.0), ACCENT);
                copy.spawn((
                    Text::new(document.name.clone()),
                    crate::ui::theme::font(kit.assets, kit.pick(22.0, 24.0)),
                    TextColor(TEXT),
                    crate::ui::theme::type_stroke(kit.pick(13.0, 14.0)),
                    super::widgets::no_wrap(),
                ));
            }
        });
        if renaming.is_some() {
            rename_actions(bar, kit);
            return;
        }
        let id = document.id;
        for (index, action, icon, copy, tone) in [
            (
                index::RENAME,
                FlowUiAction::BeginRenameMapDraft(id),
                Some("edit"),
                "Rename",
                Tone::Plain,
            ),
            (
                index::DUPLICATE,
                FlowUiAction::DuplicateMapDraft(id),
                Some("duplicate"),
                "Duplicate",
                Tone::Plain,
            ),
            (
                index::PUBLISH,
                FlowUiAction::PublishMapDraft(id),
                Some("arena"),
                "Publish",
                Tone::Plain,
            ),
            (
                index::DELETE,
                FlowUiAction::DeleteMapDraft(id),
                Some("trash"),
                "Delete",
                Tone::Danger,
            ),
        ] {
            let mut spec = ButtonSpec::new(index, action, tone).label(copy);
            if let Some(icon) = icon {
                spec = spec.icon(icon);
            }
            kit.button(bar, spec);
        }
        shell_action(
            bar,
            kit,
            index::EDIT,
            FlowUiAction::OpenMapDraft(id),
            "EDIT MAP",
            Surface::Primary,
            px(kit.pick(200.0, 250.0)),
        );
    });
}

/// Cancel and Save name while the selected map is renamed in place.
fn rename_actions(bar: &mut ChildSpawnerCommands, kit: &Kit) {
    kit.button(
        bar,
        ButtonSpec::new(
            index::RENAME_CANCEL,
            FlowUiAction::CancelRenameMapDraft,
            Tone::Plain,
        )
        .label("Cancel"),
    );
    kit.button(
        bar,
        ButtonSpec::new(
            index::RENAME_SAVE,
            FlowUiAction::ConfirmRenameMapDraft,
            Tone::Primary,
        )
        .icon("check")
        .label("Save name"),
    );
}

/// A single-line text field showing the typed name and its caret.
fn name_field(parent: &mut ChildSpawnerCommands, kit: &Kit, name: &str, width: Val) {
    parent
        .spawn((
            Node {
                width,
                max_width: percent(100),
                min_height: px(kit.ctl()),
                padding: UiRect::horizontal(px(14)),
                align_items: AlignItems::Center,
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(RADIUS)),
                overflow: Overflow::clip_x(),
                ..default()
            },
            BackgroundColor(INK),
            BorderColor::all(ACCENT),
        ))
        .with_children(|field| {
            field.spawn((
                MapNameFieldText,
                Text::new(name),
                kit.sans(kit.pick(18.0, 23.0)),
                TextColor(TEXT),
                super::widgets::no_wrap(),
            ));
            field.spawn((
                Node {
                    width: px(2),
                    height: px(kit.pick(22.0, 30.0)),
                    margin: UiRect::left(px(3)),
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(ACCENT),
            ));
        });
}

fn empty_state(body: &mut ChildSpawnerCommands, kit: &Kit) {
    body.spawn(Node {
        width: percent(100),
        flex_grow: 1.0,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..column(kit.pick(14.0, 12.0))
    })
    .with_children(|empty| {
        kit.icon(empty, "arena", kit.pick(80.0, 88.0), 1.0);
        kit.title(empty, "NO MAPS YET", kit.pick(38.0, 42.0));
        kit.label(
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
                        kit.title(chip, step, kit.pick(22.0, 24.0));
                        kit.label(chip, copy, kit.pick(15.0, 18.0), TEXT);
                    });
            }
        });
        shell_action(
            empty,
            kit,
            index::CREATE,
            FlowUiAction::CreateMap,
            "+ CREATE YOUR FIRST MAP",
            Surface::Primary,
            px(kit.pick(400.0, 460.0)),
        );
    });
}

/// A dialog layer over the library: its own UI root so the library below cannot take input.
fn dialog(
    commands: &mut Commands,
    key: MapMakerRoot,
    kit: &Kit,
    content: impl FnOnce(&mut ChildSpawnerCommands),
) {
    commands
        .spawn((
            key,
            DespawnOnExit(ClientFlow::MapLibrary),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(INK.with_alpha(0.72)),
            FocusPolicy::Block,
            GlobalZIndex(460),
        ))
        .with_children(|scrim| {
            scrim
                .spawn((
                    Node {
                        width: percent(if kit.metrics.compact { 58 } else { 46 }),
                        padding: UiRect::all(px(kit.pick(24.0, 26.0))),
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(12)),
                        ..column(12.0)
                    },
                    BackgroundColor(bar_fill()),
                    BorderColor::all(ACCENT.with_alpha(0.4)),
                    FocusPolicy::Block,
                ))
                .with_children(content);
        });
}

fn dialog_actions(
    dialog: &mut ChildSpawnerCommands,
    kit: &Kit,
    cancel: FlowUiAction,
    confirm: (FlowUiAction, &str, Tone),
    confirm_disabled: bool,
) {
    dialog
        .spawn(Node {
            justify_content: JustifyContent::FlexEnd,
            margin: UiRect::top(px(6)),
            ..row(10.0)
        })
        .with_children(|actions| {
            let cancel = kit.button(
                actions,
                ButtonSpec::new(index::DIALOG_CANCEL, cancel, Tone::Plain)
                    .label("Cancel")
                    .width(px(kit.pick(110.0, 150.0))),
            );
            let (action, copy, tone) = confirm;
            let confirm = kit.button(
                actions,
                ButtonSpec::new(index::DIALOG_CONFIRM, action, tone)
                    .label(copy)
                    .width(px(kit.pick(150.0, 210.0)))
                    .disabled(confirm_disabled),
            );
            for id in [cancel, confirm] {
                actions.commands().entity(id).insert(MapLibraryDialogButton);
            }
        });
}

fn create_dialog(
    dialog: &mut ChildSpawnerCommands,
    kit: &Kit,
    name: &str,
    defaults: &Result<super::CreateDefaults, String>,
) {
    kit.title(dialog, "NEW MAP", kit.pick(32.0, 36.0));
    kit.label(dialog, "Name", kit.pick(14.0, 18.0), ACCENT);
    name_field(dialog, kit, name, percent(100));
    match defaults {
        Ok(defaults) => {
            dialog.spawn(row(24.0)).with_children(|facts| {
                let (mode, pennant) = mode_identity(defaults.mode);
                facts.spawn(row(8.0)).with_children(|fact| {
                    kit.label(fact, "Mode", kit.pick(14.0, 17.0), ACCENT);
                    pennant_icon(fact, kit, pennant, kit.pick(22.0, 26.0));
                    kit.label(fact, mode, kit.pick(16.0, 20.0), TEXT);
                });
                for (key, value) in [
                    (
                        "Size",
                        format!(
                            "{} × {}",
                            defaults.dimensions.width, defaults.dimensions.height
                        ),
                    ),
                    ("Theme", defaults.theme.clone()),
                ] {
                    facts.spawn(row(8.0)).with_children(|fact| {
                        kit.label(fact, key, kit.pick(14.0, 17.0), ACCENT);
                        kit.label(fact, &value, kit.pick(16.0, 20.0), TEXT);
                    });
                }
            });
            kit.label(
                dialog,
                &format!(
                    "Mode and size follow your selected game, {}.",
                    defaults.game
                ),
                kit.pick(13.0, 16.0),
                TEXT.with_alpha(0.75),
            );
        }
        Err(reason) => {
            kit.label(dialog, reason, kit.pick(14.0, 18.0), PLAY_HIGHLIGHT);
        }
    }
    // Nothing can be created without a server's mode defaults: the confirm stays visible but
    // disabled so the reason above is the next thing to read.
    dialog_actions(
        dialog,
        kit,
        FlowUiAction::CancelCreateMap,
        (FlowUiAction::ConfirmCreateMap, "Create map", Tone::Primary),
        defaults.is_err(),
    );
}

fn delete_dialog(dialog: &mut ChildSpawnerCommands, kit: &Kit, document: &MapDraftDocument) {
    kit.title(dialog, "DELETE MAP?", kit.pick(32.0, 36.0));
    kit.label(
        dialog,
        &format!(
            "“{}” will be removed from this device. This cannot be undone.",
            document.name
        ),
        kit.pick(16.0, 20.0),
        TEXT,
    );
    dialog_actions(
        dialog,
        kit,
        FlowUiAction::CancelDeleteMapDraft,
        (
            FlowUiAction::DeleteMapDraft(document.id),
            "Delete map",
            Tone::Danger,
        ),
        false,
    );
}

/// The library's keyboard and controller navigation: arrows and the D-pad move focus to the
/// nearest control in that direction, Enter, Space or South activates it. While a dialog is open
/// only its buttons take focus; while a name is being typed the keys belong to the field.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Bevy input system parameters"
)]
pub(in crate::flow) fn navigate_map_library(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    runtime: Res<MapMakerRuntime>,
    mut navigation: ResMut<FlowNavigation>,
    mut actions: ResMut<PendingFlowActions>,
    mut feedback: MessageWriter<UiFeedback>,
    buttons: Query<(
        &FlowButton,
        &Interaction,
        &ComputedNode,
        &UiGlobalTransform,
        &InheritedVisibility,
        Has<InteractionDisabled>,
        Has<MapLibraryDialogButton>,
        Option<&Surface>,
    )>,
) {
    if runtime.editing_map_name().is_some() && runtime.creating.is_none() {
        // Renaming in place: arrows and Enter belong to the text field.
        return;
    }
    let dialog_open = runtime.creating.is_some()
        || runtime.pending_delete.is_some()
        || runtime.pending_withdraw.is_some();
    let candidates: Vec<_> = buttons
        .iter()
        .filter(|(_, _, computed, _, visible, disabled, in_dialog, _)| {
            visible.get() && !*disabled && !computed.is_empty() && *in_dialog == dialog_open
        })
        .map(
            |(button, interaction, computed, transform, _, _, _, surface)| {
                (
                    button,
                    *interaction,
                    transform.translation * computed.inverse_scale_factor(),
                    surface.copied(),
                )
            },
        )
        .collect();
    if candidates.is_empty() {
        return;
    }
    // Focus follows a pointer press.
    if let Some((button, ..)) = candidates
        .iter()
        .find(|(_, interaction, ..)| *interaction == Interaction::Pressed)
    {
        navigation.focused = button.index;
    }
    if !candidates
        .iter()
        .any(|(button, ..)| button.index == navigation.focused)
    {
        navigation.focused = if dialog_open {
            index::DIALOG_CONFIRM
        } else {
            candidates
                .iter()
                .map(|(button, ..)| button.index)
                .find(|index| *index >= index::FIRST_TILE)
                .unwrap_or(index::BACK)
        };
    }
    let pad = |button| gamepads.iter().any(|pad| pad.just_pressed(button));
    let direction = if keyboard.just_pressed(KeyCode::ArrowLeft) || pad(GamepadButton::DPadLeft) {
        Some(Vec2::NEG_X)
    } else if keyboard.just_pressed(KeyCode::ArrowRight) || pad(GamepadButton::DPadRight) {
        Some(Vec2::X)
    } else if keyboard.just_pressed(KeyCode::ArrowUp) || pad(GamepadButton::DPadUp) {
        Some(Vec2::NEG_Y)
    } else if keyboard.just_pressed(KeyCode::ArrowDown) || pad(GamepadButton::DPadDown) {
        Some(Vec2::Y)
    } else {
        None
    };
    let positions: Vec<_> = candidates
        .iter()
        .map(|(button, _, position, _)| (button.index, *position))
        .collect();
    if let Some(direction) = direction
        && let Some(next) = spatial_neighbor(navigation.focused, direction, &positions)
        && next != navigation.focused
    {
        navigation.focused = next;
        feedback.write(UiFeedback::Moved);
    }
    // Enter confirms the create dialog's typed name in `map_maker_input`.
    let enter = keyboard.just_pressed(KeyCode::Enter) && runtime.creating.is_none();
    let space = keyboard.just_pressed(KeyCode::Space) && runtime.creating.is_none();
    if (enter || space || pad(GamepadButton::South))
        && let Some((button, _, _, surface)) = candidates
            .iter()
            .find(|(button, ..)| button.index == navigation.focused)
    {
        queue_ui_action(&mut actions, button.action.clone());
        feedback.write(UiFeedback::Activated(*surface));
    }
}

/// The control nearest `from` in `direction`: candidates must lie ahead along the direction;
/// sideways distance counts double so moves stay in their row or column.
pub(super) fn spatial_neighbor(
    from: usize,
    direction: Vec2,
    controls: &[(usize, Vec2)],
) -> Option<usize> {
    let origin = controls.iter().find(|(index, _)| *index == from)?.1;
    controls
        .iter()
        .filter(|(index, _)| *index != from)
        .filter_map(|(index, position)| {
            let offset = *position - origin;
            let ahead = offset.dot(direction);
            (ahead > 1.0).then(|| {
                let sideways = (offset - direction * ahead).length();
                (*index, ahead + sideways * 2.0)
            })
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(index, _)| index)
}

/// Theme key as words: `feature-yard` → `Feature Yard`.
pub(super) fn theme_name(
    catalog: &MapContentCatalog,
    id: brawler::map::MapPresentationThemeId,
) -> String {
    catalog
        .presentation_themes
        .iter()
        .find(|theme| theme.id == id)
        .map_or_else(
            || format!("Theme {}", id.0),
            |theme| {
                theme
                    .key
                    .split(['-', '_'])
                    .filter(|word| !word.is_empty())
                    .map(|word| {
                        let mut characters = word.chars();
                        characters.next().map_or_else(String::new, |first| {
                            first.to_uppercase().chain(characters).collect()
                        })
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            },
        )
}

impl PaletteCategory {
    /// Picker wording in sentence case.
    pub(super) const fn title(self) -> &'static str {
        match self {
            Self::Ground => "Ground",
            Self::CoverObjects => "Cover",
            Self::Vegetation => "Plants",
            Self::EffectTiles => "Effects",
            Self::Decorations => "Decor",
            Self::SpawnsObjectives => "Spawns + goals",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spatial_focus_stays_in_its_row_and_column() {
        let controls = [
            (0, Vec2::new(50.0, 40.0)),
            (1, Vec2::new(900.0, 40.0)),
            (100, Vec2::new(150.0, 300.0)),
            (101, Vec2::new(450.0, 300.0)),
            (10, Vec2::new(900.0, 600.0)),
            (11, Vec2::new(500.0, 600.0)),
        ];
        assert_eq!(spatial_neighbor(100, Vec2::X, &controls), Some(101));
        assert_eq!(spatial_neighbor(101, Vec2::NEG_X, &controls), Some(100));
        assert_eq!(spatial_neighbor(100, Vec2::NEG_Y, &controls), Some(0));
        assert_eq!(spatial_neighbor(101, Vec2::Y, &controls), Some(11));
        assert_eq!(spatial_neighbor(0, Vec2::NEG_X, &controls), None);
    }
}
