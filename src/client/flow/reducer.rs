//! Deferred flow commit and session teardown ownership.

use super::{
    ClientFlow, ClientNetworkConfig, ClientOverlay, ConnectionGeneration, FlowCommit, FlowError,
    FlowErrorAction, FlowErrorKind, FlowNavigation, OverlayCommit, PendingConnection,
    ResolverState, RoutedClientLifecycle, RoutedClientPhase, RoutedClientSession,
    begin_connection_target, spawn_current_candidate,
};
use bevy::prelude::*;
use lightyear::prelude::{Unlink, UnlinkReason, client::Disconnect};

pub(super) fn favorite_focus_after_removal(
    removed_index: Option<usize>,
    remaining: usize,
) -> usize {
    removed_index.map_or(0, |index| {
        if index < remaining {
            3 + index * 2
        } else if index > 0 {
            3 + (index - 1) * 2
        } else {
            0
        }
    })
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(super) fn teardown_session(
    mut commands: Commands,
    commit: Res<FlowCommit>,
    clients: Query<Entity, With<RoutedClientSession>>,
    mut routed: ResMut<RoutedClientLifecycle>,
) {
    if !commit.teardown {
        return;
    }
    for entity in &clients {
        commands.trigger(Disconnect { entity });
        commands.trigger(Unlink {
            entity,
            reason: UnlinkReason::UserRequested(None),
        });
        commands.entity(entity).despawn();
    }
    routed.phase = RoutedClientPhase::Disabled;
}

#[allow(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    reason = "the flow commit phase coordinates runtime-owned Bevy resources"
)]
pub(super) fn commit_flow(
    mut commands: Commands,
    time: Res<Time<Real>>,
    config: Res<ClientNetworkConfig>,
    mut generation: ResMut<ConnectionGeneration>,
    mut resolver: ResMut<ResolverState>,
    mut routed: ResMut<RoutedClientLifecycle>,
    pending: Option<ResMut<PendingConnection>>,
    commit: Res<FlowCommit>,
    mut next_flow: ResMut<NextState<ClientFlow>>,
    mut overlay: ResMut<ClientOverlay>,
    mut navigation: ResMut<FlowNavigation>,
) {
    if let Some(error) = &commit.error {
        *overlay = ClientOverlay::Error(error.clone());
    } else if let Some(overlay_commit) = commit.overlay {
        *overlay = match overlay_commit {
            OverlayCommit::Clear => ClientOverlay::None,
            OverlayCommit::Settings => ClientOverlay::Settings,
            OverlayCommit::Credits => ClientOverlay::Credits,
            OverlayCommit::DashboardMenu => ClientOverlay::DashboardMenu,
            OverlayCommit::BrawlerList => ClientOverlay::BrawlerList,
            OverlayCommit::BrawlerDetails(value) => ClientOverlay::BrawlerDetails(value),
            OverlayCommit::BrawlerCreation => ClientOverlay::BrawlerCreation,
            OverlayCommit::BrawlerEditor => ClientOverlay::BrawlerEditor,
            OverlayCommit::WeaponEquipment => ClientOverlay::WeaponEquipment,
            OverlayCommit::DeleteBrawlerConfirmation(value) => {
                ClientOverlay::DeleteBrawlerConfirmation(value)
            }
            OverlayCommit::Confirmation(value) => ClientOverlay::Confirmation(value),
            OverlayCommit::ChangeServerConfirmation => ClientOverlay::ChangeServerConfirmation,
        };
    }
    if let Some(index) = commit.focus_index {
        navigation.selected = index;
    }
    if let Some(target) = commit.start_target.clone()
        && let Err(error) = begin_connection_target(
            &mut commands,
            &config,
            time.elapsed(),
            &mut generation,
            &mut resolver,
            &mut routed,
            target,
        )
    {
        *overlay = ClientOverlay::Error(FlowError {
            kind: FlowErrorKind::Connection,
            message: error,
            return_flow: ClientFlow::ServerSelect,
            actions: [
                Some(FlowErrorAction::RetryConnection),
                Some(FlowErrorAction::Back),
            ],
        });
        next_flow.set(ClientFlow::ServerSelect);
        return;
    } else if commit.advance_candidate
        && let Some(mut pending) = pending
    {
        if pending.current_entity.is_some() {
            pending.current_candidate = pending.current_candidate.saturating_add(1);
        }
        pending.current_entity = None;
        spawn_current_candidate(
            &mut commands,
            &config,
            time.elapsed(),
            &mut routed,
            &mut pending,
        );
    }
    if let Some(flow) = commit.next_flow {
        next_flow.set(flow);
        if flow != ClientFlow::Connecting {
            commands.remove_resource::<PendingConnection>();
        }
    }
}
