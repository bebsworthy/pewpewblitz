//! Saved build overview: presentation only; profile reducer owns every mutation.
use super::{
    BrawlerBrowserArt, BrawlerCardPreview, BrawlerDetailsLayout, BrawlerDetailsRoot,
    BrawlerDetailsScrollArea, BrawlerEditDraft, BrawlerScreenUi, advertised_weapon_name,
    brawler_screen_layout, passive_name, ultimate_name,
};
use crate::client::ui::surface::Surface;
use crate::client::{
    flow::{
        actions::FlowUiAction,
        model::{ClientFlow, ClientOverlay},
    },
    ui::theme::{CREAM, INK, TEAL, font},
};
use bevy::{prelude::*, ui::ScrollPosition};
mod art;

#[derive(bevy::ecs::system::SystemParam)]
pub(in crate::client::flow) struct DetailsAssets<'w> {
    server: Option<Res<'w, AssetServer>>,
    handles: Option<Res<'w, crate::client::ClientAssetHandles>>,
    parts: Res<'w, crate::weapon_parts::WeaponPartCatalogResource>,
    draft: Res<'w, BrawlerEditDraft>,
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "Bevy screen resources and owned render key"
)]
pub(in crate::client::flow) fn present_brawler_details(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    profile: Res<crate::client::ClientProfileModel>,
    mut screen: BrawlerScreenUi,
    resources: DetailsAssets,
    roots: Query<(Entity, &BrawlerDetailsRoot)>,
) {
    let (id, confirmation) = match *overlay {
        ClientOverlay::BrawlerDetails(id) => (id, false),
        ClientOverlay::DeleteBrawlerConfirmation(id) => (id, true),
        _ => {
            for (entity, _) in &roots {
                commands.entity(entity).despawn();
            }
            return;
        }
    };
    if *flow.get() != ClientFlow::Dashboard {
        return;
    }
    let (Some(snapshot), Some(catalog)) = (profile.snapshot(), profile.catalog()) else {
        return;
    };
    let Some(brawler) = snapshot.brawlers.iter().find(|b| b.id == id) else {
        return;
    };
    let rename = (resources.draft.inline_rename && resources.draft.brawler_id == Some(id))
        .then(|| resources.draft.clone());
    let ui_scale = screen.scale.as_deref().map_or(1., |s| s.0);
    let size = screen
        .windows
        .iter()
        .next()
        .map_or(Vec2::new(1672., 941.), |w| {
            Vec2::new(w.width(), w.height()) / ui_scale
        });
    let factor = (size.x / 1672.).min(size.y / 941.).clamp(0.65, 2.5);
    let key = BrawlerDetailsRoot {
        viewport: (size.x.to_bits(), size.y.to_bits()),
        brawler_id: id,
        profile_revision: snapshot.revision,
        pending: profile.pending(),
        contextual_confirmation: confirmation,
        layout: brawler_screen_layout(&screen),
        rename,
    };
    if roots.iter().any(|(_, old)| *old == key) {
        return;
    }
    if roots.is_empty() {
        screen.navigation.selected = 2;
    }
    for (entity, _) in &roots {
        commands.entity(entity).despawn();
    }
    let view = DetailsView {
        factor,
        body_height: (size.y - 174. * factor).max(520. * factor),
        art: BrawlerBrowserArt {
            card_height: 0.,
            server: resources.server.as_deref(),
            assets: resources.handles.as_deref(),
        },
        brawler,
        snapshot,
        catalog,
        parts: &resources.parts,
        compact: key.layout == BrawlerDetailsLayout::Compact,
        disabled: profile.pending() || confirmation,
        selecting: profile.selection_pending(id),
        rename: key.rename.as_ref(),
    };
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(28. * factor)),
                flex_direction: FlexDirection::Column,
                row_gap: px(16. * factor),
                ..default()
            },
            BackgroundColor(CREAM),
            GlobalZIndex(510),
            DespawnOnExit(ClientFlow::Dashboard),
        ))
        .with_children(|root| view.draw(root))
        .insert(key);
}

struct DetailsView<'a> {
    art: BrawlerBrowserArt<'a>,
    brawler: &'a crate::profiles::SavedBrawler,
    snapshot: &'a crate::profiles::ProfileSnapshot,
    catalog: &'a crate::profiles::AdvertisedBrawlerCatalog,
    parts: &'a crate::weapon_parts::WeaponPartCatalogResource,
    factor: f32,
    body_height: f32,
    compact: bool,
    disabled: bool,
    selecting: bool,
    rename: Option<&'a BrawlerEditDraft>,
}

impl DetailsView<'_> {
    fn label(
        &self,
        parent: &mut ChildSpawnerCommands,
        text: impl Into<String>,
        size: f32,
        color: Color,
    ) {
        self.label_aligned(parent, text, size, color, Justify::Left);
    }

    fn label_aligned(
        &self,
        parent: &mut ChildSpawnerCommands,
        text: impl Into<String>,
        size: f32,
        color: Color,
        justify: Justify,
    ) {
        parent.spawn((
            Text::new(text),
            font(self.art.assets, size * self.factor),
            TextColor(color),
            TextLayout::justify(justify),
            Node {
                width: percent(100),
                min_width: px(0),
                flex_shrink: 0.,
                ..default()
            },
        ));
    }

    fn panel(
        &self,
        parent: &mut ChildSpawnerCommands,
        index: usize,
        action: FlowUiAction,
        height: f32,
        primary: bool,
    ) -> Entity {
        let id = BrawlerBrowserArt::button(
            parent,
            index,
            action,
            false,
            self.disabled,
            Node {
                width: percent(100),
                min_width: px(0),
                height: px(height),
                flex_shrink: 0.,
                padding: UiRect::axes(px(34. * self.factor), px(24. * self.factor)),
                flex_direction: FlexDirection::Column,
                row_gap: px(8. * self.factor),
                ..default()
            },
        );
        parent.commands().entity(id).insert(if index == 4 {
            Surface::Back
        } else if primary {
            Surface::Primary
        } else {
            Surface::Panel
        });
        id
    }

    fn draw(&self, root: &mut ChildSpawnerCommands) {
        if let Some(server) = self.art.server {
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
        root.spawn((
            BrawlerDetailsScrollArea,
            ScrollPosition::default(),
            Node {
                width: percent(100),
                min_height: px(0),
                flex_grow: 1.,
                overflow: Overflow::scroll_y(),
                flex_direction: if self.compact {
                    FlexDirection::Column
                } else {
                    FlexDirection::Row
                },
                column_gap: px(24. * self.factor),
                row_gap: px(20. * self.factor),
                padding: UiRect::all(px(6. * self.factor)),
                ..default()
            },
        ))
        .with_children(|body| {
            self.identity(body);
            body.spawn(Node {
                width: if self.compact {
                    percent(100)
                } else {
                    percent(53)
                },
                min_width: px(0),
                flex_grow: 1.,
                flex_shrink: 0.,
                flex_direction: FlexDirection::Column,
                row_gap: px(16. * self.factor),
                ..default()
            })
            .with_children(|right| {
                let usable = self.body_height - 44. * self.factor;
                self.weapon(
                    right,
                    if self.compact {
                        280. * self.factor
                    } else {
                        usable * 0.40
                    },
                );
                self.abilities(
                    right,
                    if self.compact {
                        294. * self.factor
                    } else {
                        usable * 0.40
                    },
                );
                self.select_action(right, usable);
            });
        });
    }

    fn select_action(&self, right: &mut ChildSpawnerCommands, usable: f32) {
        let select = self.panel(
            right,
            0,
            FlowUiAction::SelectBrawler(self.brawler.id),
            if self.compact {
                104. * self.factor
            } else {
                usable * 0.20
            },
            true,
        );
        right
            .commands()
            .entity(select)
            .insert(Node {
                width: percent(100),
                height: px(if self.compact {
                    104. * self.factor
                } else {
                    usable * 0.20
                }),
                flex_shrink: 0.,
                padding: UiRect::all(px(16. * self.factor)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|b| {
                self.label_aligned(
                    b,
                    if self.selecting {
                        "SELECTING..."
                    } else {
                        "SELECT FOR PLAY"
                    },
                    if self.compact { 42. } else { 62. },
                    INK,
                    Justify::Center,
                );
            });
    }

    fn identity(&self, body: &mut ChildSpawnerCommands) {
        body.spawn(Node {
            width: if self.compact {
                percent(100)
            } else {
                percent(44)
            },
            flex_shrink: 0.,
            height: if self.compact {
                Val::Auto
            } else {
                px(self.body_height - 12. * self.factor)
            },
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .with_children(|left| {
            left.spawn(Node {
                width: percent(100),
                height: if self.compact {
                    px(320. * self.factor)
                } else {
                    px(self.body_height - 88. * self.factor)
                },
                flex_shrink: 0.,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                overflow: Overflow::clip(),
                ..default()
            })
            .with_children(|frame| {
                frame.spawn((
                    BrawlerCardPreview,
                    UiTransform::from_scale(Vec2::splat(1.18)),
                    Node {
                        width: px(0),
                        height: px(0),
                        flex_shrink: 0.,
                        ..default()
                    },
                ));
            });
            left.spawn(Node {
                width: px(264. * self.factor),
                margin: UiRect::top(px(12. * self.factor)),
                ..default()
            })
            .with_children(|wrap| {
                let id = self.panel(
                    wrap,
                    3,
                    FlowUiAction::DeleteBrawler(self.brawler.id),
                    64. * self.factor,
                    false,
                );
                wrap.commands()
                    .entity(id)
                    .insert(Node {
                        width: percent(100),
                        height: px(64. * self.factor),
                        flex_shrink: 0.,
                        padding: UiRect::axes(px(16. * self.factor), px(10. * self.factor)),
                        align_items: AlignItems::Center,
                        column_gap: px(8. * self.factor),
                        ..default()
                    })
                    .with_children(|b| {
                        self.icon(b, "trash", 32., 32.);
                        self.label(b, "DELETE BRAWLER", 21., INK);
                    });
            });
        });
    }

    fn header(&self, root: &mut ChildSpawnerCommands) {
        root.spawn(Node {
            width: percent(100),
            height: px(98. * self.factor),
            flex_shrink: 0.,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|header| {
            header
                .spawn(Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    top: px(0),
                    width: px(if self.compact { 205. } else { 330. } * self.factor),
                    ..default()
                })
                .with_children(|wrap| {
                    let id = self.panel(
                        wrap,
                        4,
                        FlowUiAction::BackToBrawlerList,
                        72. * self.factor,
                        true,
                    );
                    wrap.commands()
                        .entity(id)
                        .insert(Node {
                            width: percent(100),
                            height: px(72. * self.factor),
                            padding: UiRect::axes(px(18. * self.factor), px(10. * self.factor)),
                            align_items: AlignItems::Center,
                            column_gap: px(12. * self.factor),
                            ..default()
                        })
                        .with_children(|b| {
                            self.icon(b, "back", 38., 38.);
                            self.label(
                                b,
                                if self.compact {
                                    "BRAWLERS"
                                } else {
                                    "BACK TO BRAWLERS"
                                },
                                26.,
                                INK,
                            );
                        });
                });
            self.title(header);
        });
    }

    fn title(&self, header: &mut ChildSpawnerCommands) {
        let editing = self.rename.is_some_and(|r| r.editing_name);
        let id = BrawlerBrowserArt::button(
            header,
            5,
            if editing {
                FlowUiAction::ConfirmBrawlerEdit
            } else {
                FlowUiAction::BeginBrawlerNameEdit
            },
            false,
            self.disabled,
            Node {
                width: percent(if self.compact { 65. } else { 55. }),
                height: percent(100),
                margin: UiRect::left(if self.compact {
                    px(145. * self.factor)
                } else {
                    px(0)
                }),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: px(16. * self.factor),
                ..default()
            },
        );
        let text = if let Some(draft) = self.rename.filter(|r| r.editing_name) {
            let caret = draft.name_caret.min(draft.name.len());
            format!("{}|{}", &draft.name[..caret], &draft.name[caret..])
        } else {
            self.brawler.name.to_uppercase()
        };
        let size = if self.compact {
            46.
        } else if text.chars().count() > 16 {
            60.
        } else {
            88.
        };
        header
            .commands()
            .entity(id)
            .remove::<BoxShadow>()
            .insert((AccessibleLabel::new("Rename brawler"), Surface::Quiet))
            .with_children(|name| {
                name.spawn(Node {
                    width: px(text.chars().fold(0., |width, _| width + size * 0.60) * self.factor),
                    max_width: percent(82),
                    min_width: px(0),
                    ..default()
                })
                .with_children(|copy| self.label_aligned(copy, text, size, INK, Justify::Center));
                self.icon(name, "edit", 48., 48.);
                if let Some(error) = self.rename.and_then(|r| r.inline_error.as_deref()) {
                    self.label(name, error, 16., INK);
                }
            });
    }

    fn weapon(&self, parent: &mut ChildSpawnerCommands, height: f32) {
        let id = self.panel(
            parent,
            2,
            FlowUiAction::OpenWeaponEquipment(self.brawler.id),
            height,
            false,
        );
        parent.commands().entity(id).with_children(|panel| {
            self.chevron(panel);
            panel
                .spawn(Node {
                    position_type: PositionType::Absolute,
                    right: px(66. * self.factor),
                    top: percent(7),
                    width: percent(37),
                    height: percent(82),
                    ..default()
                })
                .with_children(|frame| {
                    frame.spawn((
                        crate::client::flow::WeaponPreviewHost,
                        Node {
                            width: percent(100),
                            height: percent(100),
                            ..default()
                        },
                    ));
                });
            panel
                .spawn(Node {
                    width: percent(60),
                    height: percent(100),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::SpaceBetween,
                    row_gap: px(8. * self.factor),
                    ..default()
                })
                .with_children(|copy| {
                    copy.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        ..default()
                    })
                    .with_children(|heading| {
                        self.label(heading, "WEAPON", 26., TEAL);
                        self.label(
                            heading,
                            advertised_weapon_name(self.catalog, self.brawler.weapon_base_id)
                                .to_uppercase(),
                            if self.compact { 38. } else { 48. },
                            INK,
                        );
                    });
                    self.slots(copy);
                });
        });
    }

    fn slots(&self, parent: &mut ChildSpawnerCommands) {
        parent
            .spawn(Node {
                width: percent(100),
                column_gap: px(10. * self.factor),
                ..default()
            })
            .with_children(|slots| {
                for part_id in self.brawler.equipped_part_ids {
                    let part =
                        part_id.and_then(|id| self.snapshot.inventory.iter().find(|p| p.id == id));
                    slots
                        .spawn((
                            Node {
                                width: if self.compact {
                                    px(110. * self.factor)
                                } else {
                                    percent(23)
                                },
                                aspect_ratio: Some(1.),
                                min_width: px(0),
                                border: UiRect::all(px(if part.is_some() {
                                    3. * self.factor
                                } else {
                                    0.
                                })),
                                border_radius: BorderRadius::all(px(10. * self.factor)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            BorderColor::all(TEAL),
                        ))
                        .with_children(|slot| {
                            if let (Some(part), Some(server)) = (part, self.art.server) {
                                let family = self
                                    .parts
                                    .0
                                    .definition(part.definition_id)
                                    .map_or("chamber", |d| d.presentation_type.as_str());
                                slot.spawn((
                                    ImageNode::new(server.load(format!(
                                        "brawler/ui/shell/part-{}.png",
                                        family.to_lowercase()
                                    ))),
                                    Node {
                                        width: percent(90),
                                        height: percent(80),
                                        ..default()
                                    },
                                    AccessibleLabel::new(part.display_name.clone()),
                                ));
                            } else if let Some(server) = self.art.server {
                                slot.spawn((
                                    ImageNode::new(server.load("brawler/ui/details/empty.png")),
                                    Node {
                                        width: percent(100),
                                        height: percent(100),
                                        ..default()
                                    },
                                ));
                            }
                        });
                }
            });
    }

    fn abilities(&self, parent: &mut ChildSpawnerCommands, height: f32) {
        let id = self.panel(
            parent,
            1,
            FlowUiAction::OpenBrawlerEditor(self.brawler.id),
            height,
            false,
        );
        parent.commands().entity(id).with_children(|panel| {
            self.chevron(panel);
            self.label(panel, "ABILITIES", 40., INK);
            panel
                .spawn(Node {
                    width: percent(91),
                    flex_grow: 1.,
                    min_height: px(0),
                    column_gap: px(20. * self.factor),
                    ..default()
                })
                .with_children(|groups| {
                    groups
                        .spawn(Node {
                            width: percent(42),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|ultimate| {
                            self.category(ultimate, "ULTIMATE");
                            let icon = self
                                .catalog
                                .ultimate(self.brawler.ultimate_id)
                                .map_or("blast", |a| art::ultimate_icon(a.kind));
                            self.badge(
                                ultimate,
                                icon,
                                ultimate_name(self.catalog, self.brawler.ultimate_id),
                                true,
                            );
                        });
                    groups.spawn((
                        Node {
                            width: px(2. * self.factor),
                            height: percent(90),
                            ..default()
                        },
                        BackgroundColor(TEAL),
                    ));
                    groups
                        .spawn(Node {
                            flex_grow: 1.,
                            min_width: px(0),
                            flex_direction: FlexDirection::Column,
                            ..default()
                        })
                        .with_children(|passives| {
                            self.category(passives, "PASSIVES");
                            passives
                                .spawn(Node {
                                    width: percent(100),
                                    flex_grow: 1.,
                                    min_height: px(0),
                                    column_gap: px(12. * self.factor),
                                    ..default()
                                })
                                .with_children(|pair| {
                                    for id in self.brawler.passive_ids {
                                        pair.spawn(Node {
                                            flex_grow: 1.,
                                            flex_basis: px(0),
                                            min_width: px(0),
                                            align_items: AlignItems::Center,
                                            flex_direction: FlexDirection::Column,
                                            ..default()
                                        })
                                        .with_children(
                                            |p| {
                                                let icon = self
                                                    .catalog
                                                    .passive(id)
                                                    .map_or("shield", |a| {
                                                        art::passive_icon(a.kind)
                                                    });
                                                self.badge(
                                                    p,
                                                    icon,
                                                    passive_name(self.catalog, id),
                                                    false,
                                                );
                                            },
                                        );
                                    }
                                });
                        });
                });
        });
    }

    fn category(&self, parent: &mut ChildSpawnerCommands, text: &str) {
        parent
            .spawn(Node {
                width: percent(100),
                align_items: AlignItems::Center,
                column_gap: px(10. * self.factor),
                margin: UiRect::bottom(px(6. * self.factor)),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    Node {
                        flex_grow: 1.,
                        height: px(2. * self.factor),
                        ..default()
                    },
                    BackgroundColor(TEAL),
                ));
                row.spawn(Node {
                    width: px(112. * self.factor),
                    flex_shrink: 0.,
                    ..default()
                })
                .with_children(|copy| self.label_aligned(copy, text, 21., TEAL, Justify::Center));
                row.spawn((
                    Node {
                        flex_grow: 1.,
                        height: px(2. * self.factor),
                        ..default()
                    },
                    BackgroundColor(TEAL),
                ));
            });
    }

    fn badge(&self, parent: &mut ChildSpawnerCommands, icon: &str, name: &str, ultimate: bool) {
        self.icon(
            parent,
            icon,
            if ultimate { 164. } else { 126. },
            if ultimate { 126. } else { 110. },
        );
        parent
            .spawn(Node {
                width: percent(100),
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|label| {
                label.spawn((
                    Text::new(name.to_uppercase()),
                    font(
                        self.art.assets,
                        if ultimate { 27. } else { 23. } * self.factor,
                    ),
                    TextColor(INK),
                    TextLayout::justify(Justify::Center),
                    Node {
                        width: percent(100),
                        min_width: px(0),
                        ..default()
                    },
                ));
            });
    }

    fn icon(&self, parent: &mut ChildSpawnerCommands, name: &str, width: f32, height: f32) {
        if let Some(server) = self.art.server {
            parent.spawn((
                ImageNode::new(server.load(format!("brawler/ui/details/{name}.png"))),
                Node {
                    width: px(width * self.factor),
                    height: px(height * self.factor),
                    flex_shrink: 0.,
                    ..default()
                },
            ));
        }
    }

    fn chevron(&self, parent: &mut ChildSpawnerCommands) {
        parent
            .spawn(Node {
                position_type: PositionType::Absolute,
                right: px(20. * self.factor),
                top: percent(38),
                ..default()
            })
            .with_children(|edge| self.icon(edge, "chevron", 58., 58.));
    }
}
