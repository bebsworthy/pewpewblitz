//! Catalog cards shared by creation and saved-brawler ability selection.
use super::{BrawlerBrowserArt, BrawlerCardPreview};
use crate::client::flow::actions::FlowUiAction;
use crate::client::ui::{
    surface::Surface,
    theme::{INK, TEAL, font},
};
use bevy::prelude::*;

#[derive(Clone, Copy)]
pub(super) enum ChoiceArt {
    Fighter,
    Weapon,
    Icon(&'static str),
}
pub(super) struct ChoiceCards<'a> {
    pub server: Option<&'a AssetServer>,
    pub handles: Option<&'a crate::client::ClientAssetHandles>,
    pub width: f32,
    pub pending: bool,
    pub overview: bool,
}
impl ChoiceCards<'_> {
    pub(super) fn text(
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
    pub(super) fn button(
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
    #[allow(
        clippy::too_many_arguments,
        reason = "Choice card content and geometry"
    )]
    pub(super) fn card(
        &self,
        p: &mut ChildSpawnerCommands,
        index: usize,
        action: FlowUiAction,
        category: &str,
        name: &str,
        description: &str,
        illustration: ChoiceArt,
        columns: usize,
        height: f32,
    ) {
        let width =
            (self.width - 84. - 20. * (f32::from(u16::try_from(columns).unwrap_or(5)) - 1.))
                / f32::from(u16::try_from(columns).unwrap_or(5));
        let e = self.button(
            p,
            index,
            action,
            Surface::Card,
            Node {
                width: px(width),
                height: px(height),
                padding: UiRect::axes(px(28), px(22)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                flex_shrink: 0.,
                row_gap: px(8),
                ..default()
            },
        );
        p.commands().entity(e).with_children(|card| {
            if self.overview {
                card.spawn((
                    Text::new("›"),
                    font(self.handles, 42.),
                    TextColor(INK),
                    Node {
                        position_type: PositionType::Absolute,
                        right: px(32),
                        top: percent(45),
                        ..default()
                    },
                ));
            }
            if !category.is_empty() {
                self.text(card, category, 28., TEAL);
            }
            self.illustration(card, illustration, height);
            self.text(
                card,
                name,
                if self.overview && height > 320. {
                    36.
                } else if columns > 3 {
                    22.
                } else {
                    26.
                },
                INK,
            );
            if !description.is_empty() {
                self.text(
                    card,
                    description,
                    if self.overview && height > 320. {
                        22.
                    } else {
                        18.
                    },
                    INK,
                );
            }
        });
    }
    fn illustration(&self, card: &mut ChildSpawnerCommands, illustration: ChoiceArt, height: f32) {
        card.spawn(Node {
            width: percent(100),
            height: px(0),
            min_height: px(0),
            flex_grow: 1.,
            flex_shrink: 1.,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            overflow: Overflow::clip(),
            ..default()
        })
        .with_children(|art| match illustration {
            ChoiceArt::Fighter => {
                art.spawn((
                    BrawlerCardPreview,
                    Node {
                        width: px(0),
                        height: px(0),
                        ..default()
                    },
                ));
            }
            ChoiceArt::Weapon => {
                art.spawn((
                    crate::client::flow::WeaponPreviewHost,
                    Node {
                        width: percent(85),
                        height: percent(100),
                        ..default()
                    },
                ));
            }
            ChoiceArt::Icon(icon) => {
                if let Some(server) = self.server {
                    art.spawn((
                        ImageNode::new(server.load(format!("brawler/ui/details/{icon}.png"))),
                        Node {
                            width: Val::Auto,
                            aspect_ratio: Some(1.),
                            height: percent(100),
                            max_height: px((height - 150.).clamp(72., 240.)),
                            ..default()
                        },
                    ));
                }
            }
        });
    }
}
