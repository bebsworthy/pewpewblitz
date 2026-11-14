//! Mobile app suspension (BRL-0162). While the OS suspends the app, Bevy stops updating and its
//! clocks clamp the gap, so Netcode would only notice a long-dead session a full keep-alive bound
//! after resume while the player watches a frozen match. When the app resumes after a suspension
//! at least as long as the routed session bound, the server has already timed the session out;
//! disconnect it immediately so the ordinary loss path (fresh lobby, explanation) runs at once.

use super::{
    Commands, Disconnect, Disconnected, Entity, Query, ResMut, Resource, With, Without, info,
};
use bevy::{prelude::MessageReader, window::AppLifecycle};
use std::time::{Duration, Instant};

/// Wall-clock start of the current suspension, if any.
#[derive(Resource, Debug, Default)]
pub(super) struct SuspendedSince(Option<Instant>);

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn expire_sessions_after_long_suspension(
    mut commands: Commands,
    mut lifecycle: MessageReader<AppLifecycle>,
    mut suspended: ResMut<SuspendedSince>,
    clients: Query<Entity, (With<super::RoutedClientSession>, Without<Disconnected>)>,
) {
    for event in lifecycle.read() {
        match event {
            AppLifecycle::Suspended => {
                suspended.0.get_or_insert_with(Instant::now);
            }
            AppLifecycle::Running => {
                let Some(since) = suspended.0.take() else {
                    continue;
                };
                expire_if_beyond_bound(&mut commands, since.elapsed(), &clients);
            }
            AppLifecycle::Idle | AppLifecycle::WillSuspend | AppLifecycle::WillResume => {}
        }
    }
}

fn expire_if_beyond_bound(
    commands: &mut Commands,
    suspended_for: Duration,
    clients: &Query<Entity, (With<super::RoutedClientSession>, Without<Disconnected>)>,
) {
    if suspended_for < super::ROUTED_SESSION_TIMEOUT {
        return;
    }
    for entity in clients {
        info!(
            suspended_ms = suspended_for.as_millis(),
            "routed session outlived by app suspension; disconnecting"
        );
        commands.trigger(Disconnect { entity });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{ecs::system::RunSystemOnce, prelude::*};

    #[derive(Resource, Default)]
    struct DisconnectCount(usize);

    fn world_suspended_for(duration: Duration) -> World {
        let mut world = World::new();
        world.init_resource::<Messages<AppLifecycle>>();
        world.init_resource::<DisconnectCount>();
        world.add_observer(|_: On<Disconnect>, mut count: ResMut<DisconnectCount>| count.0 += 1);
        world.insert_resource(SuspendedSince(Instant::now().checked_sub(duration)));
        world.spawn(super::super::RoutedClientSession {
            generation: 2,
            kind: super::super::RoutedClientSessionKind::Match,
        });
        world.write_message(AppLifecycle::Running);
        world
            .run_system_once(expire_sessions_after_long_suspension)
            .unwrap();
        world.flush();
        world
    }

    #[test]
    fn resume_after_the_session_bound_disconnects_routed_sessions_at_once() {
        let world =
            world_suspended_for(super::super::ROUTED_SESSION_TIMEOUT + Duration::from_secs(1));
        assert_eq!(world.resource::<DisconnectCount>().0, 1);
        assert!(world.resource::<SuspendedSince>().0.is_none());
    }

    #[test]
    fn short_suspension_keeps_the_session() {
        let world = world_suspended_for(Duration::from_secs(2));
        assert_eq!(world.resource::<DisconnectCount>().0, 0);
    }
}
