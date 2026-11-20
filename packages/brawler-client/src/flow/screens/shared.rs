//! Shared flow-screen roots, buttons, navigation, and interaction presentation.

use super::{dashboard::DashboardButtonStyle, game_select::GameTypeSelectionDraft};
use crate::flow::{actions::FlowUiAction, input::overlay_allows_button, model::ClientOverlay};
use crate::ui::surface::{Surface, SurfaceBurst, SurfaceState};
use bevy::{prelude::*, ui::InteractionDisabled};

#[derive(Resource, Default)]
pub(in crate::flow) struct FlowNavigation {
    /// Index of the focused `FlowButton`: where keyboard, gamepad or pointer navigation is.
    pub(in crate::flow) focused: usize,
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
#[require(
    crate::ui::press::ReleaseActivation,
    crate::ui::touch_target::TouchTarget
)]
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
    reason = "Screen-owned choice resources feed shared visual states"
)]
pub(in crate::flow) fn update_flow_button_chrome(
    mut commands: Commands,
    navigation: Res<FlowNavigation>,
    overlay: Res<ClientOverlay>,
    game_draft: Res<GameTypeSelectionDraft>,
    profile: Option<Res<crate::ClientProfileModel>>,
    map_maker: Option<Res<super::map_maker::MapMakerRuntime>>,
    mut buttons: Query<(
        Entity,
        &FlowButton,
        Has<InteractionDisabled>,
        Option<&DashboardButtonStyle>,
        Option<&mut SurfaceState>,
        Option<&Surface>,
        Has<super::map_maker::FlatControl>,
    )>,
) {
    // The pick being saved shows as current at once; the server's answer confirms or reverts it.
    let shown_pick = profile.as_deref().and_then(|profile| {
        profile.pending_choice().or_else(|| {
            profile
                .snapshot()
                .map(brawler::profiles::ProfileSnapshot::selection)
        })
    });
    for (entity, button, disabled, dashboard, state, role, flat) in &mut buttons {
        if matches!(dashboard, Some(DashboardButtonStyle::Preview)) {
            continue;
        }
        let current = match button.action {
            FlowUiAction::SelectGameTypeDraft(index) => game_draft.selected_index == Some(index),
            FlowUiAction::PickCharacter(id) => {
                shown_pick.is_some_and(|pick| pick.character_id == id)
            }
            FlowUiAction::PickUltimate(id) => shown_pick.is_some_and(|pick| pick.ultimate_id == id),
            FlowUiAction::PickPassive(id) => shown_pick.is_some_and(|pick| pick.passive_id == id),
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
                .is_some_and(|runtime| runtime.action_is_current(&button.action)),
            _ => false,
        };
        let next = SurfaceState {
            focused: overlay_allows_button(&overlay, button) && navigation.focused == button.index,
            current,
            inactive: !overlay_allows_button(&overlay, button),
            disabled,
        };
        if let Some(mut state) = state {
            *state = next;
        } else {
            commands.entity(entity).insert(next);
        }
        // Map Maker's flat controls draw themselves (BRL-2xpc); they are not shell surfaces.
        if role.is_none() && !flat {
            commands.entity(entity).insert(surface_role(&button.action));
        }
    }
}

pub(in crate::flow) fn surface_role(action: &FlowUiAction) -> Surface {
    use FlowUiAction as A;
    match action {
        A::Back
        | A::CloseBrawlers
        | A::CancelGameType
        | A::CancelPracticeMaps
        | A::CloseMapMaker
        | A::BackToMapLibrary
        | A::ReturnToDashboard
        | A::CloseDashboardMenu => Surface::Back,
        A::JoinQueue
        | A::StartPractice
        | A::ConfirmGameType
        | A::Connect
        | A::QueueAgain
        | A::TestAuthoredMap
        | A::MapMakerToggleValidation
        | A::OpenDashboardMenu => Surface::Primary,
        A::OpenPracticeMaps | A::OpenBrawlers | A::OpenMapMaker | A::SaveMapDraft => {
            Surface::Secondary
        }
        A::PickCharacter(_)
        | A::PickUltimate(_)
        | A::PickPassive(_)
        | A::SelectGameTypeDraft(_)
        | A::MapMakerSelectAsset(_)
        | A::MapMakerSelectCategory(_)
        | A::MapMakerToggleSearch
        | A::MapMakerClearSearch => Surface::Card,
        A::OpenGameTypeSelect => Surface::Panel,
        A::EditAddress | A::EditName => Surface::Field,
        A::JoinSaved(_) => Surface::Row,
        A::MapMakerClear | A::MapMakerDeleteSelection => Surface::Danger,
        _ => Surface::Action,
    }
}
