//! Automation-only session drivers: the windowed synthetic-controller demo and the headless
//! product-match smoke exit. Compiled only with the `automation` feature.

use super::{
    AppExit, Client, ClientJoinPhase, ClientJoinStatus, Commands, Component, Controlled, Fighter,
    Gamepad, GamepadAxis, GamepadButton, MatchPhase, MatchRoot, MatchState, MessageWriter,
    NetworkEntityId, Position, Query, Res, RoutedClientSession, RoutedClientSessionKind, Vec2,
    With, info,
};

/// Marker for the reproducible, windowed controller smoke path.
#[derive(Component)]
pub(super) struct ControllerDemoGamepad;

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn spawn_controller_demo_gamepad(
    mut commands: Commands,
    config: Res<crate::ClientAutomationConfig>,
) {
    if config.windowed_controller_demo.is_some() {
        commands.spawn((Gamepad::default(), ControllerDemoGamepad));
        info!("windowed synthetic controller demo enabled");
    }
}

/// Keep the synthetic controller aimed at the server-owned neutral dummy while preserving the
/// normal gamepad sampling path. This is only a visual/input smoke aid; it is not gameplay logic.
#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn update_controller_demo_gamepad(
    config: Res<crate::ClientAutomationConfig>,
    mut gamepads: Query<&mut Gamepad, With<ControllerDemoGamepad>>,
    controlled: Query<&Position, (With<Fighter>, With<Controlled>)>,
    fighters: Query<(&NetworkEntityId, &Position), With<Fighter>>,
) {
    if config.windowed_controller_demo.is_none() {
        return;
    }
    let aim = controlled
        .iter()
        .next()
        .and_then(|controlled| {
            fighters
                .iter()
                .find(|(network_id, _)| network_id.0 == 0)
                .map(|(_, dummy)| dummy.0 - controlled.0)
        })
        .filter(|delta| delta.is_finite() && delta.length_squared() > f32::EPSILON)
        .map_or(Vec2::X, Vec2::normalize);

    for mut gamepad in &mut gamepads {
        gamepad.analog_mut().set(GamepadAxis::LeftStickX, 0.0);
        gamepad.analog_mut().set(GamepadAxis::LeftStickY, 0.0);
        gamepad.analog_mut().set(GamepadAxis::RightStickX, aim.x);
        gamepad.analog_mut().set(GamepadAxis::RightStickY, aim.y);
        gamepad.analog_mut().set(GamepadButton::RightTrigger2, 1.0);
    }
}

/// Headless M05 evidence: exit successfully once the joined product match reaches Active.
#[allow(clippy::needless_pass_by_value)]
pub(super) fn finish_product_match_smoke(
    config: Res<crate::ClientAutomationConfig>,
    roots: Query<&MatchState, With<MatchRoot>>,
    mut exit: MessageWriter<AppExit>,
) {
    if config.product_match_smoke
        && config.render_measurement.is_none()
        && !config.product_requeue_smoke
        && !config.product_network_change_smoke
        && roots
            .iter()
            .any(|state| matches!(state.phase, MatchPhase::Active { .. }))
    {
        exit.write(AppExit::Success);
    }
}

/// Headless BRL-0162 network-change rehearsal phases. Durations are wall-clock because the
/// supervisor's capability idle bound and Netcode's keep-alive bound are wall-clock too.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum NetworkChangeStage {
    #[default]
    AwaitingActive,
    Settling {
        until: core::time::Duration,
    },
    Outage {
        until: core::time::Duration,
        generation: u64,
    },
    Recovering {
        deadline: core::time::Duration,
        generation: u64,
        tick_at_rebind: u64,
    },
    Finished,
}

#[derive(bevy::prelude::Resource, Debug, Default)]
pub(super) struct NetworkChangeSmoke {
    stage: NetworkChangeStage,
}

/// Settle time after Active before the rehearsed outage starts.
const NETWORK_CHANGE_SETTLE: core::time::Duration = core::time::Duration::from_secs(1);
/// Rehearsed outage: long enough to exceed every sub-second retransmission and the old 3 s
/// server-side default, comfortably inside the 10 s session bound.
const NETWORK_CHANGE_OUTAGE: core::time::Duration = core::time::Duration::from_secs(4);
/// Authoritative ticks the resumed session must deliver after the source-port change (1 s).
const NETWORK_CHANGE_RESUMED_TICKS: u64 = brawler::timing::SIMULATION_TICK_HZ;

/// Drive one mid-match outage plus source-port change and require the same routed match
/// session to resume: the session generation must not change, the client must stay Active, and
/// fresh authoritative ticks must arrive from the server after the rebind.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    clippy::too_many_arguments,
    reason = "one automation coordinator reads the schedule-owned session state it verifies"
)]
pub(super) fn drive_network_change_smoke(
    config: Res<crate::ClientAutomationConfig>,
    time: Res<bevy::prelude::Time<bevy::prelude::Real>>,
    mut smoke: bevy::prelude::ResMut<NetworkChangeSmoke>,
    lifecycle: Res<crate::RoutedClientLifecycle>,
    roots: Query<&MatchState, With<MatchRoot>>,
    ticks: Query<&brawler::combat::AuthoritativeTick>,
    mut clients: Query<
        (
            &RoutedClientSession,
            &ClientJoinStatus,
            &mut crate::routed_udp::RoutedUdpIo,
            &lightyear::prelude::LocalAddr,
        ),
        With<Client>,
    >,
    mut exit: MessageWriter<AppExit>,
) {
    if !config.product_network_change_smoke {
        return;
    }
    let now = time.elapsed();
    let newest_tick = ticks.iter().map(|tick| tick.0).max().unwrap_or(0);
    let Some((session, status, mut io, local_addr)) = clients.iter_mut().next() else {
        if !matches!(smoke.stage, NetworkChangeStage::AwaitingActive) {
            bevy::log::error!("network-change smoke lost its routed client entity");
            exit.write(AppExit::error());
        }
        return;
    };
    let match_session = session.kind == RoutedClientSessionKind::Match
        && lifecycle.phase == crate::RoutedClientPhase::Match;
    match smoke.stage {
        NetworkChangeStage::AwaitingActive => {
            if match_session
                && roots
                    .iter()
                    .any(|state| matches!(state.phase, MatchPhase::Active { .. }))
            {
                smoke.stage = NetworkChangeStage::Settling {
                    until: now.saturating_add(NETWORK_CHANGE_SETTLE),
                };
            }
        }
        NetworkChangeStage::Settling { until } if now >= until => {
            io.set_simulated_outage(true);
            info!(
                generation = session.generation,
                outage_ms = NETWORK_CHANGE_OUTAGE.as_millis(),
                "network-change smoke: dropping all routed traffic"
            );
            smoke.stage = NetworkChangeStage::Outage {
                until: now.saturating_add(NETWORK_CHANGE_OUTAGE),
                generation: session.generation,
            };
        }
        NetworkChangeStage::Outage { until, generation } if now >= until => {
            let previous = io.local_addr();
            match io.rebind_socket(local_addr.0) {
                Ok(rebound) => {
                    io.set_simulated_outage(false);
                    info!(
                        ?previous,
                        %rebound,
                        "network-change smoke: traffic restored from a new source port"
                    );
                    smoke.stage = NetworkChangeStage::Recovering {
                        deadline: now.saturating_add(super::ROUTED_SESSION_TIMEOUT),
                        generation,
                        tick_at_rebind: newest_tick,
                    };
                }
                Err(error) => {
                    bevy::log::error!(%error, "network-change smoke could not rebind its socket");
                    exit.write(AppExit::error());
                    smoke.stage = NetworkChangeStage::Finished;
                }
            }
        }
        NetworkChangeStage::Recovering {
            deadline,
            generation,
            tick_at_rebind,
        } => {
            let same_session = match_session
                && session.generation == generation
                && matches!(status.phase, ClientJoinPhase::Active { .. });
            if !same_session {
                bevy::log::error!(
                    generation = session.generation,
                    ?status.phase,
                    "network-change smoke: the match session did not survive"
                );
                exit.write(AppExit::error());
                smoke.stage = NetworkChangeStage::Finished;
            } else if newest_tick >= tick_at_rebind.saturating_add(NETWORK_CHANGE_RESUMED_TICKS) {
                info!(
                    generation,
                    tick_at_rebind,
                    newest_tick,
                    "network-change smoke: the same match session resumed after the source change"
                );
                exit.write(AppExit::Success);
                smoke.stage = NetworkChangeStage::Finished;
            } else if now >= deadline {
                bevy::log::error!(
                    tick_at_rebind,
                    newest_tick,
                    "network-change smoke: no authoritative progress after the source change"
                );
                exit.write(AppExit::error());
                smoke.stage = NetworkChangeStage::Finished;
            }
        }
        NetworkChangeStage::Settling { .. }
        | NetworkChangeStage::Outage { .. }
        | NetworkChangeStage::Finished => {}
    }
}

/// Finish the routed process smoke only after the match session has completed, its routes have
/// been revoked, and the client has authenticated one fresh lobby generation. Generation one is
/// the initial lobby and generation two is the match; a generation of three or greater therefore
/// cannot be satisfied by an initial lobby retry.
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn observe_fresh_lobby_return(
    config: Res<crate::ClientAutomationConfig>,
    mut app_exit: MessageWriter<AppExit>,
    query: Query<(&RoutedClientSession, &ClientJoinStatus), With<Client>>,
) {
    if !config.exit_after_lobby_return {
        return;
    }
    if query.iter().any(|(session, status)| {
        session.kind == RoutedClientSessionKind::Lobby
            && session.generation >= 3
            && matches!(status.phase, ClientJoinPhase::LobbyActive { .. })
    }) {
        info!("brawler client authenticated a fresh lobby after match completion");
        app_exit.write(AppExit::Success);
    }
}
