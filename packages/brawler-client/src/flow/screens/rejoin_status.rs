//! "Rejoining match" status shown while a lost match session is re-established (BRL-yyeg).
//!
//! The routed lifecycle is the only source: the label exists exactly while the Match screen is
//! presented and an automatic rejoin is in progress. It never decides navigation.

use crate::{
    RoutedClientLifecycle,
    flow::ClientFlow,
    ui::theme::{TEXT, font},
};
use bevy::prelude::*;

pub(in crate::flow) const REJOINING_MATCH_COPY: &str = "Rejoining match…";

#[derive(Component)]
pub(in crate::flow) struct RejoinStatusLabel;

/// Whether the rejoin status should be visible this frame.
pub(in crate::flow) fn rejoin_status_visible(
    flow: ClientFlow,
    lifecycle: &RoutedClientLifecycle,
) -> bool {
    flow == ClientFlow::Match && lifecycle.rejoin_in_progress()
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are owned by the schedule"
)]
pub(in crate::flow) fn present_rejoin_status(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    lifecycle: Res<RoutedClientLifecycle>,
    assets: Option<Res<crate::ClientAssetHandles>>,
    labels: Query<Entity, With<RejoinStatusLabel>>,
) {
    let visible = rejoin_status_visible(*flow.get(), &lifecycle);
    match (visible, labels.iter().next()) {
        (true, None) => {
            commands
                .spawn((
                    RejoinStatusLabel,
                    DespawnOnExit(ClientFlow::Match),
                    GlobalZIndex(420),
                    Node {
                        position_type: PositionType::Absolute,
                        width: percent(100),
                        top: percent(42),
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                ))
                .with_children(|root| {
                    root.spawn((
                        Text::new(REJOINING_MATCH_COPY),
                        font(assets.as_deref(), 34.0),
                        TextColor(TEXT),
                    ));
                });
        }
        (false, Some(entity)) => commands.entity(entity).despawn(),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_shows_only_on_the_match_screen_during_a_rejoin() {
        let mut lifecycle = RoutedClientLifecycle::default();
        assert!(!rejoin_status_visible(ClientFlow::Match, &lifecycle));
        lifecycle.rejoin = crate::RoutedRejoin::AwaitingGrant;
        assert!(rejoin_status_visible(ClientFlow::Match, &lifecycle));
        assert!(!rejoin_status_visible(ClientFlow::Dashboard, &lifecycle));
        lifecycle.rejoin = crate::RoutedRejoin::Connecting;
        assert!(rejoin_status_visible(ClientFlow::Match, &lifecycle));
    }
}
