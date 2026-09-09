//! Ability overview and fullscreen catalog pickers; edits remain server-owned.
use super::{
    AbilityPicker, BrawlerEditDraft, BrawlerEditorRoot, BrawlerListScrollArea, BrawlerScreenUi,
    browser_columns,
    choice_cards::{ChoiceArt, ChoiceCards},
    creation::ultimate_description,
    details::art,
};
use crate::client::flow::{
    actions::FlowUiAction,
    model::{ClientFlow, ClientOverlay},
    screens::game_select::GameSelectionColumns,
};
use crate::client::ui::{
    surface::Surface,
    theme::{CREAM, INK, TEAL},
};
use bevy::{prelude::*, ui::ScrollPosition};

#[derive(bevy::ecs::system::SystemParam)]
pub(in crate::client::flow) struct AbilityAssets<'w> {
    server: Option<Res<'w, AssetServer>>,
    handles: Option<Res<'w, crate::client::ClientAssetHandles>>,
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "Screen-owned render key and Bevy resources"
)]
pub(in crate::client::flow) fn present_brawler_editor(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    draft: Res<BrawlerEditDraft>,
    profile: Res<crate::client::ClientProfileModel>,
    screen: BrawlerScreenUi,
    assets: AbilityAssets,
    roots: Query<(Entity, &BrawlerEditorRoot)>,
    scroll_positions: Query<&ScrollPosition, With<BrawlerListScrollArea>>,
) {
    if *flow.get() != ClientFlow::Dashboard || !matches!(*overlay, ClientOverlay::BrawlerEditor) {
        for (entity, _) in &roots {
            commands.entity(entity).despawn();
        }
        return;
    }
    let Some(catalog) = profile.catalog() else {
        return;
    };
    let scale = screen.scale.as_deref().map_or(1., |s| s.0);
    let size = screen
        .windows
        .iter()
        .next()
        .map_or(Vec2::new(1440., 900.), |w| {
            Vec2::new(w.width(), w.height()) / scale
        });
    let key = BrawlerEditorRoot {
        draft: draft.clone(),
        viewport: (size.x.to_bits(), size.y.to_bits()),
        pending: profile.pending(),
    };
    if roots.iter().any(|(_, old)| *old == key) {
        return;
    }
    let scroll = if roots
        .iter()
        .any(|(_, old)| old.draft.picker == draft.picker)
    {
        scroll_positions.iter().next().cloned().unwrap_or_default()
    } else {
        ScrollPosition::default()
    };
    for (entity, _) in &roots {
        commands.entity(entity).despawn();
    }
    let view = AbilityView {
        cards: ChoiceCards {
            server: assets.server.as_deref(),
            handles: assets.handles.as_deref(),
            width: size.x,
            pending: profile.pending(),
            overview: draft.picker.is_none(),
        },
        draft: &draft,
        catalog,
        height: size.y,
    };
    commands
        .spawn((
            key,
            DespawnOnExit(ClientFlow::Dashboard),
            GlobalZIndex(510),
            BackgroundColor(CREAM),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                bottom: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(24)),
                row_gap: px(20),
                ..default()
            },
        ))
        .with_children(|root| view.draw(root, scroll));
}

struct AbilityView<'a> {
    cards: ChoiceCards<'a>,
    draft: &'a BrawlerEditDraft,
    catalog: &'a crate::profiles::AdvertisedBrawlerCatalog,
    height: f32,
}
impl AbilityView<'_> {
    fn draw(&self, root: &mut ChildSpawnerCommands, scroll: ScrollPosition) {
        if let Some(server) = self.cards.server {
            root.spawn((
                ImageNode {
                    image_mode: NodeImageMode::Stretch,
                    ..ImageNode::new(server.load("brawler/ui/shell/game-selection-background.png"))
                },
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    top: px(0),
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                ZIndex(-1),
            ));
        }
        self.header(root);
        let columns = if self.draft.picker.is_some() {
            browser_columns(self.cards.width)
        } else if self.cards.width < 1000. {
            1
        } else {
            3
        };
        root.spawn((
            BrawlerListScrollArea,
            GameSelectionColumns(columns),
            scroll,
            Node {
                width: percent(100),
                min_height: px(0),
                flex_grow: 1.,
                overflow: Overflow::scroll_y(),
                flex_direction: FlexDirection::Column,
                justify_content: if columns == 3 && self.draft.picker.is_none() {
                    JustifyContent::Center
                } else {
                    JustifyContent::Start
                },
                padding: UiRect::all(px(8)),
                row_gap: px(18),
                ..default()
            },
        ))
        .with_children(|body| {
            if let Some(error) = &self.draft.inline_error {
                self.cards.text(body, error, 18., INK);
            }
            if self.cards.pending {
                self.cards.text(body, "Updating ability…", 18., INK);
            }
            body.spawn(Node {
                width: percent(100),
                flex_wrap: FlexWrap::Wrap,
                column_gap: px(20),
                row_gap: px(20),
                flex_shrink: 0.,
                ..default()
            })
            .with_children(|grid| self.grid(grid, columns));
        });
    }
    fn header(&self, root: &mut ChildSpawnerCommands) {
        root.spawn(Node {
            width: percent(100),
            min_height: px(64),
            flex_shrink: 0.,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            column_gap: px(24),
            ..default()
        })
        .with_children(|header| {
            let e = self.cards.button(
                header,
                1000,
                FlowUiAction::CancelBrawlerEdit,
                Surface::Back,
                Node {
                    height: px(54),
                    width: px(230),
                    flex_shrink: 0.,
                    ..default()
                },
            );
            header.commands().entity(e).with_children(|p| {
                self.cards.text(
                    p,
                    if self.draft.picker.is_some() {
                        "‹ ABILITIES"
                    } else {
                        "‹ BRAWLER"
                    },
                    22.,
                    INK,
                );
            });
            self.cards.text(header, &self.draft.name, 22., TEAL);
        });
        root.spawn(Node {
            width: percent(100),
            flex_shrink: 0.,
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|title| {
            self.cards.text(
                title,
                match self.draft.picker {
                    None => "CUSTOMIZE ABILITIES",
                    Some(AbilityPicker::Ultimate) => "CHOOSE ULTIMATE",
                    Some(AbilityPicker::Passive(_)) => "CHOOSE PASSIVE",
                },
                if self.cards.width < 1000. { 28. } else { 48. },
                INK,
            );
        });
    }
    fn grid(&self, grid: &mut ChildSpawnerCommands, columns: usize) {
        match self.draft.picker {
            None => self.overview(grid, columns),
            Some(AbilityPicker::Ultimate) => {
                for (index, u) in self.catalog.ultimates.iter().enumerate() {
                    self.cards.card(
                        grid,
                        index,
                        FlowUiAction::ChooseBrawlerUltimate(u.id),
                        "",
                        &u.display_name,
                        ultimate_description(u.kind),
                        ChoiceArt::Icon(art::ultimate_icon(u.kind)),
                        columns,
                        300.,
                    );
                }
            }
            Some(AbilityPicker::Passive(slot)) => {
                for (index, p) in self.catalog.passives.iter().enumerate() {
                    let unavailable = if !p.saved_brawler_selectable {
                        Some("Fixed by fighter profile.")
                    } else if self.draft.passive_ids[1 - slot] == p.id {
                        Some("Used in the other slot.")
                    } else {
                        None
                    };
                    let cards = ChoiceCards {
                        pending: self.cards.pending || unavailable.is_some(),
                        ..self.cards
                    };
                    cards.card(
                        grid,
                        index,
                        FlowUiAction::ChooseBrawlerPassive(p.id),
                        "",
                        &p.display_name,
                        unavailable.unwrap_or_else(|| passive_description(p.kind)),
                        ChoiceArt::Icon(art::passive_icon(p.kind)),
                        columns,
                        300.,
                    );
                }
            }
        }
    }
    fn overview(&self, grid: &mut ChildSpawnerCommands, columns: usize) {
        let height = if columns == 1 {
            300.
        } else {
            (self.height * 0.55).clamp(320., 500.)
        };
        if let Some(u) = self.catalog.ultimate(self.draft.ultimate_id) {
            self.cards.card(
                grid,
                0,
                FlowUiAction::OpenBrawlerUltimate,
                "ULTIMATE",
                &u.display_name,
                ultimate_description(u.kind),
                ChoiceArt::Icon(art::ultimate_icon(u.kind)),
                columns,
                height,
            );
        }
        for (slot, id) in self.draft.passive_ids.iter().enumerate() {
            if let Some(p) = self.catalog.passive(*id) {
                self.cards.card(
                    grid,
                    slot + 1,
                    FlowUiAction::OpenBrawlerPassive(slot),
                    if slot == 0 { "PASSIVE 1" } else { "PASSIVE 2" },
                    &p.display_name,
                    passive_description(p.kind),
                    ChoiceArt::Icon(art::passive_icon(p.kind)),
                    columns,
                    height,
                );
            }
        }
    }
}
fn passive_description(kind: crate::builds::PassiveKind) -> &'static str {
    use crate::builds::PassiveKind as P;
    match kind {
        P::LightweightFrame => "Faster movement, less health.",
        P::ReinforcedFrame => "More health, slower movement.",
        P::Tenacity => "Shorter slows.",
        P::AdrenalResponse => "Speed burst after enemy primary-weapon damage.",
        P::CloseQuarters => "More damage at close range.",
        P::QuickCycle => "Faster refill after defeating an enemy.",
        P::CryogenicInsulation => "Resist freezing effects.",
        P::FilteredCirculation => "Resist poison effects.",
        P::HeatShielding => "Resist burning effects.",
    }
}
