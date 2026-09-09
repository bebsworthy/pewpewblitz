//! Creation draft and fullscreen catalog choices; profile reducer owns submission.
use super::{
    BrawlerBrowserArt, BrawlerCreationDraft, BrawlerCreationRoot, BrawlerListScrollArea,
    BrawlerScreenUi, CreationPicker, advertised_fighter_name, advertised_weapon_name,
    brawler_screen_layout, browser_columns, details, ultimate_name,
};
use crate::client::flow::screens::game_select::GameSelectionColumns;
use crate::client::flow::{
    actions::FlowUiAction,
    model::{ClientFlow, ClientOverlay},
};
use crate::client::ui::surface::Surface;
use crate::client::ui::theme::{CREAM, INK, font};
use bevy::{prelude::*, ui::ScrollPosition};

#[derive(bevy::ecs::system::SystemParam)]
pub(in crate::client::flow) struct CreationAssets<'w> {
    server: Option<Res<'w, AssetServer>>,
    handles: Option<Res<'w, crate::client::ClientAssetHandles>>,
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "Screen-owned render key and Bevy resources"
)]
pub(in crate::client::flow) fn present_brawler_creation(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    draft: Res<BrawlerCreationDraft>,
    profile: Res<crate::client::ClientProfileModel>,
    screen: BrawlerScreenUi,
    resources: CreationAssets,
    roots: Query<(Entity, &BrawlerCreationRoot)>,
    scroll_positions: Query<&ScrollPosition, With<BrawlerListScrollArea>>,
) {
    if !matches!(*overlay, ClientOverlay::BrawlerCreation) || *flow.get() != ClientFlow::Dashboard {
        for (e, _) in &roots {
            commands.entity(e).despawn();
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
    let key = BrawlerCreationRoot {
        draft: draft.clone(),
        layout: brawler_screen_layout(&screen),
        viewport: (size.x.to_bits(), size.y.to_bits()),
        pending: profile.pending(),
    };
    if roots.iter().any(|(_, old)| *old == key) {
        return;
    }
    for (e, _) in &roots {
        commands.entity(e).despawn();
    }
    let scroll = if roots
        .iter()
        .any(|(_, old)| old.draft.picker == draft.picker)
    {
        scroll_positions.iter().next().cloned().unwrap_or_default()
    } else {
        ScrollPosition::default()
    };
    let view = CreationView {
        scroll,
        draft: &draft,
        catalog,
        server: resources.server.as_deref(),
        handles: resources.handles.as_deref(),
        width: size.x,
        height: size.y,
        pending: profile.pending(),
    };
    commands
        .spawn((
            key,
            DespawnOnExit(ClientFlow::Dashboard),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(24)),
                row_gap: px(20),
                ..default()
            },
            BackgroundColor(CREAM),
            GlobalZIndex(510),
        ))
        .with_children(|root| view.draw(root));
}

struct CreationView<'a> {
    draft: &'a BrawlerCreationDraft,
    catalog: &'a crate::profiles::AdvertisedBrawlerCatalog,
    server: Option<&'a AssetServer>,
    handles: Option<&'a crate::client::ClientAssetHandles>,
    width: f32,
    height: f32,
    pending: bool,
    scroll: ScrollPosition,
}
impl CreationView<'_> {
    fn text(
        &self,
        p: &mut ChildSpawnerCommands,
        label: impl Into<String>,
        size: f32,
        color: Color,
    ) {
        p.spawn((
            Text::new(label),
            font(self.handles, size),
            TextColor(color),
            TextLayout::justify(Justify::Center),
            Node {
                max_width: percent(100),
                flex_shrink: 0.,
                ..default()
            },
        ));
    }
    fn button(
        &self,
        p: &mut ChildSpawnerCommands,
        index: usize,
        action: FlowUiAction,
        role: Surface,
        node: Node,
    ) -> Entity {
        let e = BrawlerBrowserArt::button(p, index, action, false, self.pending, node);
        p.commands().entity(e).insert(role);
        e
    }
    fn draw(&self, root: &mut ChildSpawnerCommands) {
        self.header(root);
        let columns = if self.draft.picker.is_some() {
            browser_columns(self.width)
        } else if self.width < 1000. {
            1
        } else {
            3
        };
        root.spawn((
            BrawlerListScrollArea,
            GameSelectionColumns(columns),
            self.scroll.clone(),
            Node {
                width: percent(100),
                min_height: px(0),
                flex_grow: 1.,
                overflow: Overflow::scroll_y(),
                flex_direction: FlexDirection::Column,
                row_gap: px(18),
                padding: UiRect::all(px(8)),
                ..default()
            },
        ))
        .with_children(|body| {
            body.spawn(Node {
                width: percent(100),
                flex_wrap: FlexWrap::Wrap,
                column_gap: px(20),
                row_gap: px(20),
                flex_shrink: 0.,
                ..default()
            })
            .with_children(|grid| {
                if let Some(picker) = self.draft.picker {
                    self.choices(grid, picker, columns);
                } else {
                    let height = if columns == 1 {
                        300.
                    } else {
                        (self.height - 330.).clamp(300., 500.)
                    };
                    self.card(
                        grid,
                        0,
                        FlowUiAction::OpenCreationFighter,
                        "FIGHTER",
                        advertised_fighter_name(self.catalog, self.draft.fighter_profile_id),
                        "Health · Speed · Reveal range",
                        CreationPicker::Fighter,
                        None,
                        columns,
                        height,
                    );
                    self.card(
                        grid,
                        1,
                        FlowUiAction::OpenCreationWeapon,
                        "WEAPON",
                        advertised_weapon_name(self.catalog, self.draft.weapon_base_id),
                        "Customizable parts",
                        CreationPicker::Weapon,
                        None,
                        columns,
                        height,
                    );
                    self.card(
                        grid,
                        2,
                        FlowUiAction::OpenCreationUltimate,
                        "ULTIMATE",
                        ultimate_name(self.catalog, self.draft.ultimate),
                        "Change anytime",
                        CreationPicker::Ultimate,
                        self.catalog.ultimate(self.draft.ultimate).map(|u| u.kind),
                        columns,
                        height,
                    );
                }
            });
            if self.draft.picker.is_none() {
                self.text(
                    body,
                    "Fighter and weapon are fixed after creation.",
                    18.,
                    INK,
                );
                self.footer(body);
            }
            if let Some(error) = &self.draft.inline_error {
                self.text(body, error, 18., INK);
            }
        });
    }
    fn header(&self, root: &mut ChildSpawnerCommands) {
        if let Some(server) = self.server {
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
        let title = match self.draft.picker {
            None => "CREATE BRAWLER",
            Some(CreationPicker::Fighter) => "CHOOSE FIGHTER",
            Some(CreationPicker::Weapon) => "CHOOSE WEAPON",
            Some(CreationPicker::Ultimate) => "CHOOSE ULTIMATE",
        };
        root.spawn(Node {
            width: percent(100),
            min_height: px(64),
            flex_shrink: 0.,
            align_items: AlignItems::Center,
            column_gap: px(24),
            ..default()
        })
        .with_children(|header| {
            let e = self.button(
                header,
                1000,
                FlowUiAction::CancelCreateBrawler,
                Surface::Back,
                Node {
                    height: px(54),
                    width: px(230),
                    flex_shrink: 0.,
                    ..default()
                },
            );
            header.commands().entity(e).with_children(|p| {
                self.text(
                    p,
                    if self.draft.picker.is_some() {
                        "‹ CREATE BRAWLER"
                    } else {
                        "‹ BRAWLERS"
                    },
                    22.,
                    INK,
                );
            });
            self.text(
                header,
                title,
                if self.width < 1000. { 28. } else { 42. },
                INK,
            );
        });
    }
    #[allow(
        clippy::too_many_arguments,
        reason = "Creation choice content and geometry"
    )]
    fn card(
        &self,
        p: &mut ChildSpawnerCommands,
        index: usize,
        action: FlowUiAction,
        category: &str,
        name: &str,
        description: &str,
        kind: CreationPicker,
        ultimate: Option<crate::builds::UltimateKind>,
        columns: usize,
        height: f32,
    ) {
        use super::choice_cards::{ChoiceArt, ChoiceCards};
        ChoiceCards {
            server: self.server,
            handles: self.handles,
            width: self.width,
            pending: self.pending,
            overview: self.draft.picker.is_none(),
        }
        .card(
            p,
            index,
            action,
            category,
            name,
            description,
            match kind {
                CreationPicker::Fighter => ChoiceArt::Fighter,
                CreationPicker::Weapon => ChoiceArt::Weapon,
                CreationPicker::Ultimate => {
                    ChoiceArt::Icon(ultimate.map_or("pulse", details::art::ultimate_icon))
                }
            },
            columns,
            height,
        );
    }
    fn choices(&self, p: &mut ChildSpawnerCommands, kind: CreationPicker, columns: usize) {
        match kind {
            CreationPicker::Fighter => {
                for (i, f) in self.catalog.fighter_profiles.iter().enumerate() {
                    self.card(
                        p,
                        i,
                        FlowUiAction::ChooseCreationFighter(f.id),
                        "",
                        &f.display_name,
                        &format!(
                            "Health {} · Speed {:.0} · Reveal {:.0}",
                            f.stats.maximum_health,
                            f.stats.movement_speed,
                            f.stats.reveal_proximity_radius
                        ),
                        kind,
                        None,
                        columns,
                        300.,
                    );
                }
            }
            CreationPicker::Weapon => {
                for (i, w) in self.catalog.weapon_bases.iter().enumerate() {
                    self.card(
                        p,
                        i,
                        FlowUiAction::ChooseCreationWeapon(w.id),
                        "",
                        &w.display_name,
                        weapon_description(&w.configuration),
                        kind,
                        None,
                        columns,
                        300.,
                    );
                }
            }
            CreationPicker::Ultimate => {
                for (i, u) in self.catalog.ultimates.iter().enumerate() {
                    self.card(
                        p,
                        i,
                        FlowUiAction::ChooseCreationUltimate(u.id),
                        "",
                        &u.display_name,
                        ultimate_description(u.kind),
                        kind,
                        Some(u.kind),
                        columns,
                        300.,
                    );
                }
            }
        }
    }
    fn footer(&self, p: &mut ChildSpawnerCommands) {
        p.spawn(Node {
            width: percent(100),
            flex_direction: if self.width < 1000. {
                FlexDirection::Column
            } else {
                FlexDirection::Row
            },
            column_gap: px(24),
            row_gap: px(14),
            flex_shrink: 0.,
            ..default()
        })
        .with_children(|row| {
            let e = self.button(
                row,
                3,
                FlowUiAction::EditCreationName,
                Surface::Field,
                Node {
                    min_height: px(76),
                    width: if self.width < 1000. {
                        percent(100)
                    } else {
                        percent(58)
                    },
                    align_items: AlignItems::Center,
                    ..default()
                },
            );
            row.commands().entity(e).with_children(|p| {
                let mut name = self.draft.name.clone();
                if self.draft.editing_name {
                    name.insert(self.draft.name_caret, '|');
                }
                self.text(p, format!("NAME   {name}"), 24., INK);
            });
            let e = self.button(
                row,
                4,
                FlowUiAction::ConfirmCreateBrawler,
                Surface::Primary,
                Node {
                    min_height: px(76),
                    flex_grow: 1.,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::all(px(18)),
                    ..default()
                },
            );
            row.commands().entity(e).with_children(|p| {
                self.text(
                    p,
                    if self.pending {
                        "CREATING…"
                    } else {
                        "CREATE BRAWLER"
                    },
                    26.,
                    INK,
                );
            });
        });
    }
}

fn weapon_description(config: &crate::combat::WeaponConfiguration) -> &'static str {
    use crate::combat::{DeliveryMethod as D, FiringPattern};
    match config.recipe.delivery {
        D::Straight { .. } => {
            if matches!(config.recipe.firing, FiringPattern::Spread { .. }) {
                "Projectile spread."
            } else {
                "Single straight shots."
            }
        }
        D::StickyStraight { .. } => "Sticks, then explodes.",
        D::Lobbed { .. } => "Lobbed impact.",
        D::MeleeArc { .. } => "Close-range melee sweep.",
        D::ConeSpray { .. } => "Spray in a wide cone.",
        D::Splash { .. } => "Lingering area effects.",
    }
}
pub(super) fn ultimate_description(kind: crate::builds::UltimateKind) -> &'static str {
    use crate::builds::UltimateKind as U;
    match kind {
        U::Dash => "Burst forward.",
        U::Sentry => "Deploy a turret.",
        U::SelfCloak => "Hide yourself briefly.",
        U::RevealScan => "Reveal hidden enemies.",
        U::ConcealmentField => "Create a concealment zone.",
        U::DemolitionStrike => "Strike a targeted area.",
        U::CryogenicField => "Create a freezing zone.",
        U::FireField => "Create a burning zone.",
        U::PoisonField => "Create a poison zone.",
        U::RestorationField => "Create a healing zone.",
        U::BigBlob => "Launch a bouncing blob.",
    }
}
