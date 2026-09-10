//! Shared flow-screen roots, buttons, navigation, and interaction presentation.

use super::{dashboard::DashboardButtonStyle, game_select::GameTypeSelectionDraft};
use crate::client::ui::surface::{Surface, SurfaceState};
use crate::client::{
    ClientLobbyMembership,
    flow::{actions::FlowUiAction, input::overlay_allows_button, model::ClientOverlay},
};
use bevy::{prelude::*, ui::InteractionDisabled};
use lightyear::prelude::client::Client;

#[derive(Resource, Default)]
pub(in crate::client::flow) struct FlowNavigation {
    pub(in crate::client::flow) selected: usize,
}

#[derive(Component)]
pub(in crate::client::flow) struct FlowRoot;

#[derive(Component, Clone, Debug)]
pub(in crate::client::flow) struct FlowButton {
    pub(in crate::client::flow) index: usize,
    pub(in crate::client::flow) action: FlowUiAction,
    pub(in crate::client::flow) error_action: bool,
}

pub(in crate::client::flow) fn flow_root_node() -> Node {
    Node {
        width: percent(100),
        height: percent(100),
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        row_gap: px(10),
        padding: UiRect::all(px(20)),
        overflow: Overflow::scroll_y(),
        ..default()
    }
}

pub(in crate::client::flow) fn spawn_heading(parent: &mut ChildSpawnerCommands, text: &str) {
    parent.spawn((
        Text::new(text),
        TextFont::from_font_size(38.0),
        TextColor(Color::srgb(0.25, 0.9, 1.0)),
    ));
}

pub(in crate::client::flow) fn spawn_flow_button(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    action: FlowUiAction,
    label: &str,
) {
    spawn_flow_button_disabled(parent, index, action, label, false);
}

pub(in crate::client::flow) fn spawn_flow_button_disabled(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    action: FlowUiAction,
    label: &str,
    disabled: bool,
) {
    let mut entity = parent.spawn((
        Button,
        FlowButton {
            index,
            action,
            error_action: false,
        },
        Node {
            width: percent(88),
            max_width: px(820),
            min_height: px(42),
            padding: UiRect::axes(px(12), px(8)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,

            ..default()
        },
    ));
    if disabled {
        entity.insert(InteractionDisabled);
    }
    entity.with_child((Text::new(label), TextFont::from_font_size(18.0)));
}

pub(in crate::client::flow) fn spawn_flow_error_button(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    action: FlowUiAction,
    label: &str,
) {
    spawn_flow_error_button_disabled(parent, index, action, label, false);
}

pub(in crate::client::flow) fn spawn_flow_error_button_disabled(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    action: FlowUiAction,
    label: &str,
    disabled: bool,
) {
    let mut entity = parent.spawn((
        Button,
        FlowButton {
            index,
            action,
            error_action: true,
        },
        Node {
            width: percent(92),
            min_height: px(44),
            padding: UiRect::axes(px(12), px(8)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,

            ..default()
        },
    ));
    if disabled {
        entity.insert(InteractionDisabled);
    }
    entity.with_child((Text::new(label), TextFont::from_font_size(18.0)));
}

// Flow owns action semantics and input scope; Surface owns all visual policy.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "screen-owned state adapter"
)]
#[allow(
    clippy::too_many_arguments,
    reason = "Screen-owned selection resources feed shared visual states"
)]
pub(in crate::client::flow) fn update_flow_button_chrome(
    mut commands: Commands,
    navigation: Res<FlowNavigation>,
    overlay: Res<ClientOverlay>,
    game_draft: Res<GameTypeSelectionDraft>,
    equipment: Option<Res<super::equipment::WeaponEquipmentDraft>>,
    creation: Option<Res<super::brawlers::BrawlerCreationDraft>>,
    abilities: Option<Res<super::brawlers::BrawlerEditDraft>>,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    mut buttons: Query<(
        Entity,
        &FlowButton,
        Has<InteractionDisabled>,
        Option<&DashboardButtonStyle>,
        Option<&mut SurfaceState>,
        Option<&Surface>,
    )>,
) {
    let selected_brawler = memberships
        .iter()
        .next()
        .and_then(|m| m.profile.selected_brawler_id);
    for (entity, button, disabled, dashboard, state, role) in &mut buttons {
        if matches!(dashboard, Some(DashboardButtonStyle::Preview)) {
            continue;
        }
        let selected = match button.action {
            FlowUiAction::SelectGameTypeDraft(index) => game_draft.selected_index == Some(index),
            FlowUiAction::OpenBrawlerDetails(id) => selected_brawler == Some(id),
            FlowUiAction::SelectEquipmentSlot(slot) => equipment
                .as_deref()
                .is_some_and(|d| d.selected_slot == slot),
            FlowUiAction::EquipWeaponPart(id) => equipment
                .as_deref()
                .is_some_and(|d| d.equipped_part_ids[d.selected_slot] == Some(id)),
            FlowUiAction::UnequipWeaponPart => equipment
                .as_deref()
                .is_some_and(|d| d.equipped_part_ids[d.selected_slot].is_none()),
            FlowUiAction::ChooseCreationFighter(id) => creation
                .as_deref()
                .is_some_and(|d| d.fighter_profile_id == id),
            FlowUiAction::ChooseCreationWeapon(id) => {
                creation.as_deref().is_some_and(|d| d.weapon_base_id == id)
            }
            FlowUiAction::ChooseCreationUltimate(id) => {
                creation.as_deref().is_some_and(|d| d.ultimate == id)
            }
            FlowUiAction::ChooseBrawlerUltimate(id) => {
                abilities.as_deref().is_some_and(|d| d.ultimate_id == id)
            }
            FlowUiAction::ChooseBrawlerPassive(id) => abilities.as_deref().is_some_and(|d| {
                matches!(d.picker,
                Some(super::brawlers::AbilityPicker::Passive(slot)) if d.passive_ids[slot] == id)
            }),
            _ => false,
        };
        let next = SurfaceState {
            focused: overlay_allows_button(&overlay, button) && navigation.selected == button.index,
            selected,
            inactive: !overlay_allows_button(&overlay, button),
            disabled,
        };
        if let Some(mut state) = state {
            *state = next;
        } else {
            commands.entity(entity).insert(next);
        }
        if role.is_none() {
            commands.entity(entity).insert(surface_role(&button.action));
        }
    }
}

pub(in crate::client::flow) fn surface_role(action: &FlowUiAction) -> Surface {
    use FlowUiAction as A;
    match action {
        A::Back
        | A::CloseBrawlerList
        | A::BackToBrawlerList
        | A::CancelWeaponEquipment
        | A::CancelBrawlerEdit
        | A::CancelCreateBrawler
        | A::CancelGameType
        | A::CancelPracticeMaps
        | A::CloseMapMaker
        | A::BackToMapLibrary
        | A::ReturnToDashboard
        | A::CloseDashboardMenu => Surface::Back,
        A::JoinQueue
        | A::StartPractice
        | A::SelectBrawler(_)
        | A::ConfirmGameType
        | A::ConfirmCreateBrawler
        | A::Connect
        | A::QueueAgain
        | A::OpenDashboardMenu => Surface::Primary,
        A::OpenPracticeMaps | A::OpenBrawlerList | A::OpenMapMaker => Surface::Secondary,
        A::OpenBrawlerDetails(_)
        | A::CreateBrawler
        | A::SelectEquipmentSlot(_)
        | A::EquipWeaponPart(_)
        | A::UnequipWeaponPart
        | A::SelectGameTypeDraft(_) => Surface::Card,
        A::OpenWeaponEquipment(_) | A::OpenBrawlerEditor(_) | A::OpenGameTypeSelect => {
            Surface::Panel
        }
        A::EditAddress | A::EditName => Surface::Field,
        A::JoinSaved(_) => Surface::Row,
        A::BeginBrawlerNameEdit | A::ConfirmBrawlerEdit => Surface::Quiet,
        _ => Surface::Action,
    }
}
