//! Character select (BRL-egn6): the roster grid and the selected character's two ultimates and
//! two passives. Presentation only; the profile reducer sends every pick.
//!
//! Focus indices form a four-column grid for keyboard and gamepad parity: the roster fills the
//! first three columns row by row, the option column is the fourth (ultimates in rows 0-1,
//! passives in rows 2-3), and Back sits above at [`BRAWLERS_BACK_INDEX`].

use super::{
    dashboard::{DashboardLayoutClass, dashboard_layout_class},
    game_select::GameSelectionColumns,
    shared::{FlowButton, FlowNavigation},
};
use crate::flow::{
    actions::FlowUiAction,
    model::{ClientFlow, ClientOverlay},
};
use crate::ui::theme::{ACCENT, PANEL_RAISED, TEXT, font, shell_background};
use bevy::{
    prelude::*,
    ui::{InteractionDisabled, UiScale},
    window::PrimaryWindow,
};
use brawler::characters::{CharacterChoice, CharacterId};
use brawler::profiles::{AdvertisedCharacter, AdvertisedCharacterCatalog};

mod art;

const ROSTER_COLUMNS: usize = 3;
/// The navigation grid: the roster columns plus the option column.
const GRID_COLUMNS: usize = ROSTER_COLUMNS + 1;
pub(in crate::flow) const BRAWLERS_BACK_INDEX: usize = 1000;

/// The last refused pick's message, shown on the screen until the next pick or reopen.
#[derive(Resource, Default)]
pub(in crate::flow) struct BrawlersNotice(pub(in crate::flow) Option<String>);

/// Shows the Dashboard's live preview (the selected character) while the screen is open.
#[derive(Component)]
pub(crate) struct BrawlersPreviewHost;

#[derive(Component, Clone, Debug, PartialEq)]
pub(in crate::flow) struct BrawlersRoot {
    viewport: (u32, u32),
    compact: bool,
    selection: CharacterChoice,
    pending: bool,
    notice: Option<String>,
}

/// The roster tile's focus index for the character at `position` in catalog order.
const fn tile_index(position: usize) -> usize {
    (position / ROSTER_COLUMNS) * GRID_COLUMNS + position % ROSTER_COLUMNS
}

/// The option column's focus index for `row` (0-1 ultimates, 2-3 passives).
const fn option_index(row: usize) -> usize {
    row * GRID_COLUMNS + ROSTER_COLUMNS
}

/// The focus index of a character's roster tile (the first tile when it is not advertised).
pub(in crate::flow) fn focus_index_for_character(
    catalog: &AdvertisedCharacterCatalog,
    id: CharacterId,
) -> usize {
    tile_index(
        catalog
            .characters
            .iter()
            .position(|character| character.id == id)
            .unwrap_or(0),
    )
}

#[derive(bevy::ecs::system::SystemParam)]
pub(in crate::flow) struct BrawlersScreenUi<'w, 's> {
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    scale: Option<Res<'w, UiScale>>,
    server: Option<Res<'w, AssetServer>>,
    handles: Option<Res<'w, crate::ClientAssetHandles>>,
    notice: Res<'w, BrawlersNotice>,
    navigation: ResMut<'w, FlowNavigation>,
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy screen resources and owned render key"
)]
pub(in crate::flow) fn present_brawlers(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    profile: Res<crate::ClientProfileModel>,
    mut screen: BrawlersScreenUi,
    roots: Query<(Entity, &BrawlersRoot)>,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::Brawlers) || *flow.get() != ClientFlow::Dashboard
    {
        for (entity, _) in &roots {
            commands.entity(entity).despawn();
        }
        return;
    }
    let (Some(snapshot), Some(catalog)) = (profile.snapshot(), profile.catalog()) else {
        return;
    };
    let ui_scale = screen.scale.as_deref().map_or(1.0, |scale| scale.0);
    let window = screen.windows.iter().next();
    let size = window.map_or(Vec2::new(1280.0, 720.0), |window| {
        Vec2::new(window.width(), window.height())
    });
    let compact = window.is_some_and(|window| {
        dashboard_layout_class(
            window.resolution.width(),
            window.resolution.height(),
            ui_scale,
        ) == DashboardLayoutClass::Compact
    });
    let selection = profile
        .pending_choice()
        .unwrap_or_else(|| snapshot.selection());
    let key = BrawlersRoot {
        viewport: (size.x.to_bits(), size.y.to_bits()),
        compact,
        selection,
        pending: profile.pending(),
        notice: screen.notice.0.clone(),
    };
    if roots.iter().any(|(_, old)| *old == key) {
        return;
    }
    if roots.is_empty() {
        screen.navigation.focused = focus_index_for_character(catalog, selection.character_id);
    }
    for (entity, _) in &roots {
        commands.entity(entity).despawn();
    }
    let Some(character) = catalog.character(selection.character_id) else {
        return;
    };
    let view = BrawlersView {
        catalog,
        character,
        selection,
        disabled: key.pending,
        compact,
        server: screen.server.as_deref(),
        handles: screen.handles.as_deref(),
    };
    commands
        .spawn((
            key.clone(),
            GameSelectionColumns(GRID_COLUMNS),
            DespawnOnExit(ClientFlow::Dashboard),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(24)),
                row_gap: px(16),
                ..default()
            },
            shell_background(),
            GlobalZIndex(510),
        ))
        .with_children(|root| view.draw(root, key.notice.as_deref()));
}

struct BrawlersView<'a> {
    catalog: &'a AdvertisedCharacterCatalog,
    character: &'a AdvertisedCharacter,
    selection: CharacterChoice,
    disabled: bool,
    compact: bool,
    server: Option<&'a AssetServer>,
    handles: Option<&'a crate::ClientAssetHandles>,
}

impl BrawlersView<'_> {
    fn label(&self, parent: &mut ChildSpawnerCommands, text: impl Into<String>, size: f32) {
        self.label_colored(parent, text, size, TEXT, Justify::Center);
    }

    fn label_colored(
        &self,
        parent: &mut ChildSpawnerCommands,
        text: impl Into<String>,
        size: f32,
        color: Color,
        justify: Justify,
    ) {
        parent.spawn((
            Text::new(text),
            font(self.handles, size),
            TextColor(color),
            TextLayout::justify(justify),
            Node {
                width: percent(100),
                min_width: px(0),
                flex_shrink: 0.0,
                ..default()
            },
        ));
    }

    fn icon(&self, parent: &mut ChildSpawnerCommands, name: &str, size: f32) {
        if let Some(server) = self.server {
            parent.spawn((
                ImageNode::new(server.load(crate::ui::icons::path(name))),
                Node {
                    width: px(size),
                    height: px(size),
                    flex_shrink: 0.0,
                    ..default()
                },
            ));
        }
    }

    fn button(
        &self,
        parent: &mut ChildSpawnerCommands,
        index: usize,
        action: FlowUiAction,
        node: Node,
    ) -> Entity {
        let mut entity = parent.spawn((
            Button,
            FlowButton {
                index,
                action,
                error_action: true,
            },
            node,
        ));
        if self.disabled && index != BRAWLERS_BACK_INDEX {
            entity.insert(InteractionDisabled);
        }
        entity.id()
    }

    fn draw(&self, root: &mut ChildSpawnerCommands, notice: Option<&str>) {
        if let Some(server) = self.server {
            root.spawn((
                crate::ui::motifs::ShellBackdrop(crate::ui::motifs::MotifLayout::Edges),
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
        self.header(root, notice);
        root.spawn(Node {
            width: percent(100),
            min_height: px(0),
            flex_grow: 1.0,
            column_gap: px(20),
            ..default()
        })
        .with_children(|body| {
            if !self.compact {
                self.portrait(body);
            }
            self.roster(body);
            self.options(body);
        });
    }

    fn header(&self, root: &mut ChildSpawnerCommands, notice: Option<&str>) {
        root.spawn(Node {
            width: percent(100),
            height: px(72),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|header| {
            let back = self.button(
                header,
                BRAWLERS_BACK_INDEX,
                FlowUiAction::CloseBrawlers,
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    top: px(4),
                    width: px(150),
                    height: px(60),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::axes(px(12), px(8)),
                    ..default()
                },
            );
            header
                .commands()
                .entity(back)
                .insert(AccessibleLabel::new("Back to Dashboard"))
                .with_children(|b| self.label(b, "‹ BACK", 24.0));
            header
                .spawn(Node {
                    width: percent(50),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|title| {
                    self.label(title, "BRAWLERS", 46.0);
                    if let Some(notice) = notice {
                        self.label(title, notice, 18.0);
                    }
                });
        });
    }

    /// The selected character: the Dashboard's live preview, name, role and kit facts.
    fn portrait(&self, body: &mut ChildSpawnerCommands) {
        body.spawn(Node {
            width: percent(27),
            min_width: px(0),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(4),
            ..default()
        })
        .with_children(|portrait| {
            portrait.spawn((
                BrawlersPreviewHost,
                Node {
                    width: percent(100),
                    min_height: px(0),
                    flex_grow: 1.0,
                    ..default()
                },
            ));
            self.label(portrait, self.character.display_name.to_uppercase(), 48.0);
            self.label_colored(
                portrait,
                format!(
                    "{} · {}",
                    self.character.role.label().to_uppercase(),
                    self.character.body.label().to_uppercase()
                ),
                22.0,
                ACCENT,
                Justify::Center,
            );
            self.label(portrait, self.weapon_name(self.character), 24.0);
            self.label(
                portrait,
                format!(
                    "{} HEALTH · SPEED {:.0}",
                    self.character.stats.maximum_health, self.character.stats.movement_speed
                ),
                18.0,
            );
        });
    }

    fn weapon_name(&self, character: &AdvertisedCharacter) -> String {
        self.catalog.weapon(character.weapon_base).map_or_else(
            || "Unknown weapon".into(),
            |weapon| weapon.display_name.clone(),
        )
    }

    fn roster(&self, body: &mut ChildSpawnerCommands) {
        let rows = self.catalog.characters.len().div_ceil(ROSTER_COLUMNS);
        body.spawn(Node {
            min_width: px(0),
            flex_grow: 1.0,
            flex_basis: px(0),
            display: Display::Grid,
            grid_template_columns: RepeatedGridTrack::flex(
                u16::try_from(ROSTER_COLUMNS).unwrap_or(3),
                1.0,
            ),
            grid_template_rows: RepeatedGridTrack::flex(u16::try_from(rows).unwrap_or(3), 1.0),
            column_gap: px(12),
            row_gap: px(12),
            ..default()
        })
        .with_children(|grid| {
            for (position, character) in self.catalog.characters.iter().enumerate() {
                self.tile(grid, position, character);
            }
        });
    }

    fn tile(
        &self,
        grid: &mut ChildSpawnerCommands,
        position: usize,
        character: &AdvertisedCharacter,
    ) {
        let selected = character.id == self.selection.character_id;
        let weapon = self.weapon_name(character);
        let tile = self.button(
            grid,
            tile_index(position),
            FlowUiAction::PickCharacter(character.id),
            Node {
                min_width: px(0),
                min_height: px(76),
                padding: UiRect::axes(px(10), px(6)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(2),
                ..default()
            },
        );
        grid.commands()
            .entity(tile)
            .insert(AccessibleLabel::new(format!(
                "{}: {} {}, {weapon}{}",
                character.display_name,
                character.role.label(),
                character.body.label(),
                if selected { ", selected" } else { "" }
            )))
            .with_children(|card| {
                self.label(card, character.display_name.to_uppercase(), 30.0);
                self.label_colored(
                    card,
                    character.role.label().to_uppercase(),
                    16.0,
                    ACCENT,
                    Justify::Center,
                );
                self.label(card, weapon, 17.0);
            });
    }

    /// The selected character's two ultimates and two passives.
    fn options(&self, body: &mut ChildSpawnerCommands) {
        body.spawn(Node {
            width: percent(if self.compact { 34.0 } else { 26.0 }),
            min_width: px(0),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
            ..default()
        })
        .with_children(|column| {
            self.category(column, "ULTIMATE");
            for (slot, id) in self.character.ultimate_options.into_iter().enumerate() {
                let (name, icon) =
                    self.catalog
                        .ultimate(id)
                        .map_or(("Unknown", "blast"), |ultimate| {
                            (
                                ultimate.display_name.as_str(),
                                art::ultimate_icon(ultimate.kind),
                            )
                        });
                self.option(
                    column,
                    option_index(slot),
                    FlowUiAction::PickUltimate(id),
                    name,
                    icon,
                    id == self.selection.ultimate_id,
                );
            }
            self.category(column, "PASSIVE");
            for (slot, id) in self.character.passive_options.into_iter().enumerate() {
                let (name, icon) =
                    self.catalog
                        .passive(id)
                        .map_or(("Unknown", "shield"), |passive| {
                            (
                                passive.display_name.as_str(),
                                art::passive_icon(passive.kind),
                            )
                        });
                self.option(
                    column,
                    option_index(2 + slot),
                    FlowUiAction::PickPassive(id),
                    name,
                    icon,
                    id == self.selection.passive_id,
                );
            }
        });
    }

    fn category(&self, column: &mut ChildSpawnerCommands, text: &str) {
        column
            .spawn(Node {
                width: percent(100),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                column_gap: px(10),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    Node {
                        flex_grow: 1.0,
                        height: px(2),
                        ..default()
                    },
                    BackgroundColor(PANEL_RAISED),
                ));
                row.spawn(Node {
                    width: px(120),
                    flex_shrink: 0.0,
                    ..default()
                })
                .with_children(|copy| {
                    self.label_colored(copy, text, 22.0, ACCENT, Justify::Center);
                });
                row.spawn((
                    Node {
                        flex_grow: 1.0,
                        height: px(2),
                        ..default()
                    },
                    BackgroundColor(PANEL_RAISED),
                ));
            });
    }

    fn option(
        &self,
        column: &mut ChildSpawnerCommands,
        index: usize,
        action: FlowUiAction,
        name: &str,
        icon: &str,
        chosen: bool,
    ) {
        let option = self.button(
            column,
            index,
            action,
            Node {
                width: percent(100),
                min_height: px(76),
                flex_grow: 1.0,
                flex_basis: px(0),
                padding: UiRect::axes(px(14), px(6)),
                align_items: AlignItems::Center,
                column_gap: px(12),
                ..default()
            },
        );
        column
            .commands()
            .entity(option)
            .insert(AccessibleLabel::new(format!(
                "{name}{}",
                if chosen { ", chosen" } else { "" }
            )))
            .with_children(|row| {
                self.icon(row, icon, 56.0);
                row.spawn(Node {
                    min_width: px(0),
                    flex_grow: 1.0,
                    ..default()
                })
                .with_children(|copy| {
                    self.label_colored(copy, name.to_uppercase(), 22.0, TEXT, Justify::Left);
                });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roster_and_option_indices_form_one_four_column_grid() {
        let catalog = AdvertisedCharacterCatalog::embedded().unwrap();
        let tiles: Vec<_> = (0..catalog.characters.len()).map(tile_index).collect();
        let options: Vec<_> = (0..4).map(option_index).collect();
        assert_eq!(tiles, [0, 1, 2, 4, 5, 6, 8, 9, 10]);
        assert_eq!(options, [3, 7, 11, 15]);
        assert!(tiles.iter().all(|index| !options.contains(index)));
        assert_eq!(focus_index_for_character(&catalog, CharacterId(9)), 10);
        assert_eq!(focus_index_for_character(&catalog, CharacterId(999)), 0);
    }
}
