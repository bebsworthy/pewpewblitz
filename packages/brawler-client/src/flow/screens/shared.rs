//! Shared flow-screen roots, buttons, navigation, and interaction presentation.

use super::{dashboard::DashboardButtonStyle, game_select::GameTypeSelectionDraft};
use crate::ui::surface::{Surface, SurfaceBurst, SurfaceState};
use crate::{
    ClientLobbyMembership,
    flow::{actions::FlowUiAction, input::overlay_allows_button, model::ClientOverlay},
};
use bevy::{prelude::*, ui::InteractionDisabled};
use lightyear::prelude::client::Client;

#[derive(Resource, Default)]
pub(in crate::flow) struct FlowNavigation {
    pub(in crate::flow) selected: usize,
}

#[derive(Component)]
pub(in crate::flow) struct FlowRoot;

/// Presentation-only request to burst the action that confirmed an accepted change (BRL-51zd).
/// The owning screen may rebuild after the commit, so the request stays open briefly and bursts
/// the matching action on whichever screen entity carries it. Navigation never waits for it.
#[derive(Resource, Default)]
pub(in crate::flow) struct FlowBurst {
    index: Option<usize>,
    since: Option<f32>,
}

impl FlowBurst {
    pub(in crate::flow) fn request(&mut self, index: usize) {
        self.index = Some(index);
        self.since = None;
    }

    #[cfg(test)]
    pub(in crate::flow) fn requested(&self) -> Option<usize> {
        self.index
    }
}

/// Long enough to cover the commit-frame rebuild, short enough that later rebuilds stay still.
const FLOW_BURST_WINDOW_SECONDS: f32 = 0.25;

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Bevy presentation system parameters"
)]
pub(in crate::flow) fn apply_flow_burst(
    mut commands: Commands,
    time: Res<Time>,
    overlay: Res<ClientOverlay>,
    mut burst: ResMut<FlowBurst>,
    buttons: Query<(Entity, &FlowButton), (With<Surface>, Without<SurfaceBurst>)>,
) {
    let Some(index) = burst.index else {
        return;
    };
    let now = time.elapsed_secs();
    let since = *burst.since.get_or_insert(now);
    if now - since > FLOW_BURST_WINDOW_SECONDS {
        *burst = FlowBurst::default();
        return;
    }
    for (entity, button) in &buttons {
        if button.index == index && overlay_allows_button(&overlay, button) {
            commands.entity(entity).insert(SurfaceBurst::default());
        }
    }
}

#[derive(Component, Clone, Debug)]
#[require(crate::ui::press::ReleaseActivation)]
pub(in crate::flow) struct FlowButton {
    pub(in crate::flow) index: usize,
    pub(in crate::flow) action: FlowUiAction,
    pub(in crate::flow) error_action: bool,
}

pub(in crate::flow) fn flow_root_node() -> Node {
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

pub(in crate::flow) fn spawn_heading(parent: &mut ChildSpawnerCommands, text: &str) {
    parent.spawn((
        Text::new(text),
        TextFont::from_font_size(38.0),
        TextColor(crate::ui::theme::ACCENT),
    ));
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
pub(in crate::flow) fn update_flow_button_chrome(
    mut commands: Commands,
    navigation: Res<FlowNavigation>,
    overlay: Res<ClientOverlay>,
    game_draft: Res<GameTypeSelectionDraft>,
    equipment: Option<Res<super::equipment::WeaponEquipmentDraft>>,
    creation: Option<Res<super::brawlers::BrawlerCreationDraft>>,
    abilities: Option<Res<super::brawlers::BrawlerEditDraft>>,
    map_maker: Option<Res<super::map_maker::MapMakerRuntime>>,
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
            FlowUiAction::MapMakerSelectAsset(_)
            | FlowUiAction::MapMakerSelectCategory(_)
            | FlowUiAction::MapMakerToggleSearch
            | FlowUiAction::MapMakerUsePlaceTool
            | FlowUiAction::MapMakerUseEraseTool
            | FlowUiAction::MapMakerUseMoveTool
            | FlowUiAction::MapMakerCycleSymmetry
            | FlowUiAction::MapMakerToggleGrid
            | FlowUiAction::MapMakerClear => map_maker
                .as_deref()
                .is_some_and(|runtime| runtime.action_selected(&button.action)),
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

pub(in crate::flow) fn surface_role(action: &FlowUiAction) -> Surface {
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
        | A::TestAuthoredMap
        | A::MapMakerToggleValidation
        | A::OpenDashboardMenu => Surface::Primary,
        A::OpenPracticeMaps | A::OpenBrawlerList | A::OpenMapMaker | A::SaveMapDraft => {
            Surface::Secondary
        }
        A::OpenBrawlerDetails(_)
        | A::CreateBrawler
        | A::SelectEquipmentSlot(_)
        | A::EquipWeaponPart(_)
        | A::UnequipWeaponPart
        | A::SelectGameTypeDraft(_)
        | A::MapMakerSelectAsset(_)
        | A::MapMakerSelectCategory(_)
        | A::MapMakerToggleSearch
        | A::MapMakerClearSearch => Surface::Card,
        A::OpenWeaponEquipment(_) | A::OpenBrawlerEditor(_) | A::OpenGameTypeSelect => {
            Surface::Panel
        }
        A::EditAddress | A::EditName => Surface::Field,
        A::JoinSaved(_) => Surface::Row,
        A::BeginBrawlerNameEdit | A::ConfirmBrawlerEdit => Surface::Quiet,
        A::MapMakerClear | A::MapMakerDeleteSelection => Surface::Danger,
        _ => Surface::Action,
    }
}
