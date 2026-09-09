//! Shared fullscreen confirmations; reducers retain action and authority ownership.
use super::{
    brawlers::DeleteBrawlerConfirmationRoot,
    scroll::{clamp_scroll_offset, normalized_wheel_delta, offset_keeping_interval_visible},
    shared::{FlowButton, FlowNavigation},
};
use crate::client::{
    ClientAssetHandles, ClientProfileModel,
    flow::{
        actions::FlowUiAction,
        model::{ClientFlow, ClientOverlay},
        reducer::PendingDeletedBrawler,
    },
    ui::{
        surface::{Surface, SurfaceState},
        theme::{CREAM, INK, TEAL, font},
    },
};
use bevy::{
    input::mouse::MouseWheel,
    prelude::*,
    ui::{InteractionDisabled, ScrollPosition, UiScale},
    window::PrimaryWindow,
};

#[derive(Component, PartialEq, Clone)]
pub(in crate::client::flow) struct ConfirmationRoot {
    overlay: ClientOverlay,
    name: String,
    error: Option<String>,
    pending: bool,
    size: Vec2,
}
#[derive(Component)]
pub(in crate::client::flow) struct ConfirmationScroll;
#[derive(Component)]
pub(in crate::client::flow) struct ConfirmationAction;
#[derive(bevy::ecs::system::SystemParam)]
pub(in crate::client::flow) struct ConfirmationAssets<'w, 's> {
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    scale: Option<Res<'w, UiScale>>,
    server: Option<Res<'w, AssetServer>>,
    handles: Option<Res<'w, ClientAssetHandles>>,
}
#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "Confirmation render key and Bevy resources"
)]
pub(in crate::client::flow) fn present_confirmations(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    profile: Res<ClientProfileModel>,
    deletion: Res<PendingDeletedBrawler>,
    assets: ConfirmationAssets,
    mut navigation: ResMut<FlowNavigation>,
    roots: Query<(Entity, &ConfirmationRoot)>,
) {
    let valid = matches!(
        (*flow.get(), overlay.as_ref()),
        (
            ClientFlow::Dashboard,
            ClientOverlay::DeleteBrawlerConfirmation(_) | ClientOverlay::ChangeServerConfirmation
        ) | (ClientFlow::Match, ClientOverlay::LeaveConfirmation)
    );
    if !valid {
        for (e, _) in &roots {
            commands.entity(e).despawn();
        }
        return;
    }
    let size = assets
        .windows
        .iter()
        .next()
        .map_or(Vec2::new(1440., 900.), |w| Vec2::new(w.width(), w.height()))
        / assets.scale.as_deref().map_or(1., |s| s.0);
    let name = if let ClientOverlay::DeleteBrawlerConfirmation(id) = overlay.as_ref() {
        profile
            .snapshot()
            .and_then(|s| s.brawlers.iter().find(|b| b.id == *id))
            .map_or("Brawler", |b| b.name.as_str())
            .to_owned()
    } else {
        String::new()
    };
    let is_delete = matches!(*overlay, ClientOverlay::DeleteBrawlerConfirmation(_));
    let key = ConfirmationRoot {
        overlay: overlay.clone(),
        name,
        error: if is_delete {
            deletion.error.clone()
        } else {
            None
        },
        pending: is_delete && profile.pending(),
        size,
    };
    if roots.iter().any(|(_, old)| *old == key) {
        return;
    }
    if !roots.iter().any(|(_, old)| old.overlay == *overlay) {
        navigation.selected = 0;
    }
    for (e, _) in &roots {
        commands.entity(e).despawn();
    }
    let mut root = commands.spawn((
        key.clone(),
        DespawnOnExit(*flow.get()),
        GlobalZIndex(610),
        BackgroundColor(CREAM),
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
            flex_direction: FlexDirection::Column,
            ..default()
        },
    ));
    if is_delete {
        root.insert(DeleteBrawlerConfirmationRoot);
    }
    root.with_children(|p| {
        ConfirmationView {
            key: &key,
            assets: &assets,
        }
        .draw(p);
    });
}
struct ConfirmationView<'a, 'w, 's> {
    key: &'a ConfirmationRoot,
    assets: &'a ConfirmationAssets<'w, 's>,
}
impl ConfirmationView<'_, '_, '_> {
    fn text(&self, p: &mut ChildSpawnerCommands, value: &str, size: f32, color: Color) {
        p.spawn((
            Text::new(value),
            font(self.assets.handles.as_deref(), size),
            TextColor(color),
            TextLayout::justify(Justify::Center),
            Node {
                max_width: percent(100),
                flex_shrink: 0.,
                ..default()
            },
        ));
    }
    fn draw(&self, root: &mut ChildSpawnerCommands) {
        if let Some(server) = self.assets.server.as_deref() {
            root.spawn((
                ImageNode {
                    image_mode: NodeImageMode::Stretch,
                    ..ImageNode::new(server.load("brawler/ui/shell/game-selection-background.png"))
                },
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                ZIndex(-1),
            ));
        }
        let compact = self.key.size.x < 1000.;
        root.spawn((
            ConfirmationScroll,
            ScrollPosition::default(),
            Node {
                width: percent(100),
                height: percent(100),
                overflow: Overflow::scroll_y(),
                padding: UiRect::all(px(28)),
                ..default()
            },
        ))
        .with_children(|scroll| {
            scroll
                .spawn(Node {
                    width: percent(100),
                    min_height: percent(100),
                    flex_shrink: 0.,
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    row_gap: px(24),
                    padding: UiRect::axes(px(8), px(20)),
                    ..default()
                })
                .with_children(|body| self.content(body, compact));
        });
    }
    fn content(&self, body: &mut ChildSpawnerCommands, compact: bool) {
        let (title, copy, keep, confirm, keep_action, confirm_action) = match self.key.overlay {
            ClientOverlay::DeleteBrawlerConfirmation(_) => (
                "DELETE BRAWLER?",
                "This cannot be undone.",
                "KEEP BRAWLER",
                "DELETE",
                FlowUiAction::CancelDeleteBrawler,
                FlowUiAction::ConfirmDeleteBrawler,
            ),
            ClientOverlay::LeaveConfirmation => (
                "LEAVE MATCH?",
                "Return to the dashboard.",
                "KEEP PLAYING",
                "LEAVE MATCH",
                FlowUiAction::KeepPlaying,
                FlowUiAction::ConfirmLeaveMatch,
            ),
            _ => (
                "CHANGE SERVER?",
                "Disconnect from the current lobby.",
                "STAY CONNECTED",
                "CHANGE SERVER",
                FlowUiAction::KeepServer,
                FlowUiAction::ConfirmChangeServer,
            ),
        };
        self.icon(body);
        self.text(body, title, if compact { 44. } else { 76. }, INK);
        if !self.key.name.is_empty() {
            self.text(body, &self.key.name, if compact { 36. } else { 58. }, TEAL);
        }
        self.text(body, copy, if compact { 24. } else { 32. }, INK);
        if self.key.pending {
            self.text(body, "Deleting…", 22., INK);
        }
        if let Some(error) = &self.key.error {
            self.text(body, error, 22., INK);
        }
        body.spawn(Node {
            width: percent(100),
            max_width: px(1200),
            flex_direction: if compact {
                FlexDirection::Column
            } else {
                FlexDirection::Row
            },
            row_gap: px(20),
            column_gap: px(28),
            margin: UiRect::top(px(24)),
            flex_shrink: 0.,
            ..default()
        })
        .with_children(|buttons| {
            self.action(buttons, 0, keep_action, keep, Surface::Action, compact);
            self.action(
                buttons,
                1,
                confirm_action,
                confirm,
                Surface::Primary,
                compact,
            );
        });
    }
    fn action(
        &self,
        p: &mut ChildSpawnerCommands,
        index: usize,
        action: FlowUiAction,
        label: &str,
        role: Surface,
        compact: bool,
    ) {
        let mut e = p.spawn((
            Button,
            ConfirmationAction,
            FlowButton {
                index,
                action,
                error_action: true,
            },
            role,
            SurfaceState::default(),
            Node {
                width: if compact { percent(100) } else { px(0) },
                flex_grow: 1.,
                flex_shrink: 0.,
                min_height: px(if compact { 82. } else { 112. }),
                padding: UiRect::axes(px(44), px(22)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
        ));
        if self.key.pending {
            e.insert(InteractionDisabled);
        }
        e.with_children(|p| self.text(p, label, if compact { 26. } else { 34. }, INK));
    }
    fn icon(&self, p: &mut ChildSpawnerCommands) {
        if matches!(
            self.key.overlay,
            ClientOverlay::DeleteBrawlerConfirmation(_)
        ) {
            if let Some(server) = self.assets.server.as_deref() {
                p.spawn((
                    ImageNode::new(server.load("brawler/ui/details/trash.png")),
                    Node {
                        width: px(96),
                        height: px(96),
                        flex_shrink: 0.,
                        ..default()
                    },
                ));
            }
        } else {
            p.spawn(Node {
                width: px(112),
                height: px(88),
                flex_shrink: 0.,
                ..default()
            })
            .with_children(|icon| {
                if matches!(self.key.overlay, ClientOverlay::ChangeServerConfirmation) {
                    arrow(icon, 16., 20., false);
                    arrow(icon, 16., 64., true);
                } else {
                    exit_icon(icon);
                }
            });
        }
    }
}
fn exit_icon(p: &mut ChildSpawnerCommands) {
    for (x, y, width, height) in [(8., 4., 8., 80.), (8., 4., 44., 8.), (8., 76., 44., 8.)] {
        p.spawn((
            BackgroundColor(INK),
            Node {
                position_type: PositionType::Absolute,
                left: px(x),
                top: px(y),
                width: px(width),
                height: px(height),
                ..default()
            },
        ));
    }
    arrow(p, 32., 44., false);
}

fn arrow(p: &mut ChildSpawnerCommands, x: f32, y: f32, left: bool) {
    let tip = if left { x } else { x + 72. };
    for (cx, cy, w, angle) in [
        (x + 36., y, 72., 0.),
        (
            tip + if left { 10. } else { -10. },
            y - 10.,
            32.,
            if left { -45. } else { 45. },
        ),
        (
            tip + if left { 10. } else { -10. },
            y + 10.,
            32.,
            if left { 45. } else { -45. },
        ),
    ] {
        p.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(cx - w / 2.),
                top: px(cy - 6.),
                width: px(w),
                height: px(12),
                ..default()
            },
            BackgroundColor(INK),
            UiTransform::from_rotation(Rot2::degrees(angle)),
        ));
    }
}
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Native focus and scroll geometry"
)]
pub(in crate::client::flow) fn scroll_confirmations(
    navigation: Res<FlowNavigation>,
    mut wheel: MessageReader<MouseWheel>,
    mut areas: Query<
        (
            Entity,
            &ComputedNode,
            &UiGlobalTransform,
            &mut ScrollPosition,
        ),
        With<ConfirmationScroll>,
    >,
    buttons: Query<(&FlowButton, &ComputedNode, &UiGlobalTransform), With<ConfirmationAction>>,
    mut previous: Local<Option<(Entity, usize)>>,
) {
    let delta = normalized_wheel_delta(wheel.read(), 36.);
    let Some((entity, node, transform, mut scroll)) = areas.iter_mut().next() else {
        *previous = None;
        return;
    };
    scroll.0.y -= delta;
    let key = (entity, navigation.selected);
    if previous.as_ref() != Some(&key)
        && let Some((_, button, bt)) = buttons
            .iter()
            .find(|(b, _, _)| b.index == navigation.selected)
    {
        let (_, _, center) = transform.to_scale_angle_translation();
        let (_, _, bc) = bt.to_scale_angle_translation();
        if !node.is_empty() && !button.is_empty() {
            scroll.0.y = offset_keeping_interval_visible(
                scroll.0.y,
                center.y - node.size().y / 2. + 8. ..center.y + node.size().y / 2. - 8.,
                bc.y - button.size().y / 2. ..bc.y + button.size().y / 2.,
                node.inverse_scale_factor(),
            );
            *previous = Some(key);
        }
    }
    scroll.0.y = clamp_scroll_offset(
        scroll.0.y,
        node.content_size().y,
        node.size().y,
        node.inverse_scale_factor(),
    );
}
