//! Authoritative common-window boundary capture, validation, encoding, and one-shot output.

use super::{ProcessDiagnosticsSettings, ProcessDiagnosticsState, TransportCounters};
use bevy::prelude::*;
use std::path::Path;

/// Capture the authoritative match boundaries in the fixed-post transaction. The lifecycle
/// outcome has committed by this point, while `SimulationTick` still names the tick that just
/// ran. This avoids an app-frame-dependent `Last` observation shifting one topology's interval
/// by one tick when its render/update cadence differs from the other.
#[cfg(feature = "server")]
#[allow(clippy::needless_pass_by_value)] // Bevy systems receive `Res<T>` by value.
pub(in crate::diagnostics) fn observe_common_window_fixed(
    settings: Res<ProcessDiagnosticsSettings>,
    tick: Res<crate::timing::SimulationTick>,
    roots: Query<&crate::matchplay::MatchState, With<crate::matchplay::MatchRoot>>,
    mut state: ResMut<ProcessDiagnosticsState>,
) {
    if settings.window_path.is_none() {
        return;
    }
    let Ok(match_state) = roots.single() else {
        return;
    };
    let current_tick = tick.0;
    if state.common_window.start_tick.is_none()
        && matches!(
            match_state.phase,
            crate::matchplay::MatchPhase::Active { .. }
        )
    {
        state.common_window.start_tick = Some(current_tick);
        state.common_window.start_transport = state.transport;
    }
    if state.common_window.end_tick.is_none()
        && state.common_window.start_tick.is_some()
        && matches!(
            match_state.phase,
            crate::matchplay::MatchPhase::Completed { .. }
        )
    {
        state.common_window.end_tick = Some(current_tick);
        state.common_window.end_transport = state.transport;
    }
}

/// Client-side fallback for an opt-in marker. Clients do not own the authoritative outcome
/// transaction, so they retain the old observational behavior; paired M01 comparisons consume
/// only the server and routed match-worker markers produced by [`observe_common_window_fixed`].
#[cfg(not(feature = "server"))]
#[allow(clippy::needless_pass_by_value)] // Bevy systems receive `Res<T>` by value.
pub(super) fn observe_common_window_client(
    settings: Res<ProcessDiagnosticsSettings>,
    tick: Option<Res<crate::timing::SimulationTick>>,
    roots: Query<&crate::matchplay::MatchState, With<crate::matchplay::MatchRoot>>,
    mut state: ResMut<ProcessDiagnosticsState>,
) {
    if settings.window_path.is_none() {
        return;
    }
    let Some(tick) = tick else {
        return;
    };
    let Ok(match_state) = roots.single() else {
        return;
    };
    let current_tick = tick.0;
    if state.common_window.start_tick.is_none()
        && matches!(
            match_state.phase,
            crate::matchplay::MatchPhase::Active { .. }
        )
    {
        state.common_window.start_tick = Some(current_tick);
        state.common_window.start_transport = state.transport;
    }
    if state.common_window.end_tick.is_none()
        && state.common_window.start_tick.is_some()
        && matches!(
            match_state.phase,
            crate::matchplay::MatchPhase::Completed { .. }
        )
    {
        state.common_window.end_tick = Some(current_tick);
        state.common_window.end_transport = state.transport;
    }
}

/// Finalize one captured interval after the terminal app-frame transport sample. Tick bounds
/// were captured by the fixed authoritative boundary on the server; only the marker's stable
/// identity and transport deltas are assembled here.
#[allow(clippy::needless_pass_by_value)] // Bevy systems receive `Res<T>` by value.
pub(in crate::diagnostics) fn finalize_common_window(
    settings: Res<ProcessDiagnosticsSettings>,
    roots: Query<&crate::matchplay::MatchState, With<crate::matchplay::MatchRoot>>,
    participants: Query<&crate::matchplay::MatchParticipant>,
    protocol: Option<Res<crate::protocol::ProtocolFingerprint>>,
    content: Option<Res<crate::content::GameplayContentFingerprint>>,
    mut state: ResMut<ProcessDiagnosticsState>,
) {
    let Some(path) = settings.window_path.as_deref() else {
        return;
    };
    let Ok(match_state) = roots.single() else {
        return;
    };
    let (Some(start_tick), Some(end_tick)) =
        (state.common_window.start_tick, state.common_window.end_tick)
    else {
        return;
    };
    if state.common_window.written {
        return;
    }
    let Some((protocol, content)) =
        validated_window_fingerprints(path, protocol.as_deref(), content.as_deref())
    else {
        return;
    };
    let Some(bounds) = finalize_transport_bounds(&mut state, path, start_tick, end_tick) else {
        return;
    };
    let Some(result) = completed_result_summary(match_state) else {
        return;
    };
    let role = std::env::var("BRAWLER_DIAGNOSTICS_ROLE").unwrap_or_else(|_| "unknown".into());
    let participant_count = participants
        .iter()
        .filter(|participant| participant.match_id == match_state.match_id)
        .count();
    let contents = encode_common_window_marker(&CommonWindowMarker {
        settings: &settings,
        match_state,
        role: &role,
        participant_count,
        result_kind: result.kind,
        result_team_a: result.team_a,
        result_team_b: result.team_b,
        start_tick: bounds.start_tick,
        end_tick: bounds.end_tick,
        start: bounds.start,
        end: bounds.end,
        protocol,
        content,
    });
    if write_common_window_marker(path, &contents, bounds) {
        state.common_window.written = true;
    }
}

#[derive(Clone, Copy)]
struct CommonWindowBounds {
    start_tick: u64,
    end_tick: u64,
    start: TransportCounters,
    end: TransportCounters,
}

#[derive(Clone, Copy)]
struct CommonWindowResult {
    kind: &'static str,
    team_a: u8,
    team_b: u8,
}

fn validated_window_fingerprints(
    path: &Path,
    protocol: Option<&crate::protocol::ProtocolFingerprint>,
    content: Option<&crate::content::GameplayContentFingerprint>,
) -> Option<(u64, u64)> {
    let Some(protocol) = protocol.filter(|fingerprint| fingerprint.0 != 0) else {
        bevy::log::error!(path = %path.display(), "common authoritative measurement window requires a non-zero protocol fingerprint");
        return None;
    };
    let Some(content) = content.filter(|fingerprint| fingerprint.0 != 0) else {
        bevy::log::error!(path = %path.display(), "common authoritative measurement window requires a non-zero content fingerprint");
        return None;
    };
    Some((protocol.0, content.0))
}

fn finalize_transport_bounds(
    state: &mut ProcessDiagnosticsState,
    path: &Path,
    start_tick: u64,
    end_tick: u64,
) -> Option<CommonWindowBounds> {
    state.common_window.end_transport = state.transport;
    let bounds = CommonWindowBounds {
        start_tick,
        end_tick,
        start: state.common_window.start_transport,
        end: state.common_window.end_transport,
    };
    if bounds.end_tick < bounds.start_tick
        || bounds.end.bytes_sent < bounds.start.bytes_sent
        || bounds.end.bytes_received < bounds.start.bytes_received
        || bounds.end.packets_sent < bounds.start.packets_sent
        || bounds.end.packets_received < bounds.start.packets_received
    {
        bevy::log::error!(path = %path.display(), start_tick, end_tick, "common authoritative measurement window was not monotonic");
        None
    } else {
        Some(bounds)
    }
}

fn completed_result_summary(
    match_state: &crate::matchplay::MatchState,
) -> Option<CommonWindowResult> {
    match match_state.phase {
        crate::matchplay::MatchPhase::Completed {
            result: crate::matchplay::MatchResult::TeamVictory { team },
            ..
        } => Some(CommonWindowResult {
            kind: "team-victory",
            team_a: team.0,
            team_b: 0,
        }),
        crate::matchplay::MatchPhase::Completed {
            result: crate::matchplay::MatchResult::Draw,
            ..
        } => Some(CommonWindowResult {
            kind: "draw",
            team_a: 0,
            team_b: 0,
        }),
        crate::matchplay::MatchPhase::Completed {
            result:
                crate::matchplay::MatchResult::Forfeit {
                    winner,
                    departed_team,
                },
            ..
        } => Some(CommonWindowResult {
            kind: "forfeit",
            team_a: winner.0,
            team_b: departed_team.0,
        }),
        _ => None,
    }
}

fn write_common_window_marker(path: &Path, contents: &str, bounds: CommonWindowBounds) -> bool {
    match std::fs::write(path, contents) {
        Ok(()) => {
            bevy::log::info!(path = %path.display(), start_tick = bounds.start_tick, end_tick = bounds.end_tick, "common authoritative measurement window written");
            true
        }
        Err(error) => {
            bevy::log::error!(path = %path.display(), ?error, "common authoritative measurement window write failed");
            false
        }
    }
}

struct CommonWindowMarker<'a> {
    settings: &'a ProcessDiagnosticsSettings,
    match_state: &'a crate::matchplay::MatchState,
    role: &'a str,
    participant_count: usize,
    result_kind: &'a str,
    result_team_a: u8,
    result_team_b: u8,
    start_tick: u64,
    end_tick: u64,
    start: TransportCounters,
    end: TransportCounters,
    protocol: u64,
    content: u64,
}

fn encode_common_window_marker(marker: &CommonWindowMarker<'_>) -> String {
    format!(
        "schema=brawler-common-window-v1\nstatus=complete\nrole={role}\nrun_id={}\nscenario_id={}\nscenario_revision={}\nmode={}\nrules_profile={}\nnetwork_profile={}\nprotocol_version={}\nregistry_fingerprint={}\ncontent_fingerprint={}\nmode_definition_id={}\nrules_revision={}\nparticipant_count={participant_count}\nresult_kind={result_kind}\nresult_team_a={result_team_a}\nresult_team_b={result_team_b}\nstart_tick={start_tick}\nend_tick={end_tick}\ntick_count={}\ntransport_bytes_sent_start={}\ntransport_bytes_sent_end={}\ntransport_bytes_received_start={}\ntransport_bytes_received_end={}\npackets_sent_start={}\npackets_sent_end={}\npackets_received_start={}\npackets_received_end={}\n",
        marker.settings.manifest.run_id,
        marker.settings.manifest.scenario_id,
        marker.settings.manifest.scenario_revision,
        marker.settings.manifest.mode,
        marker.settings.manifest.rules_profile,
        marker.settings.manifest.network_profile,
        marker.settings.manifest.protocol_version,
        marker.protocol,
        marker.content,
        marker.match_state.mode_definition_id.0,
        marker.match_state.rules_revision,
        marker.end_tick.saturating_sub(marker.start_tick),
        marker.start.bytes_sent,
        marker.end.bytes_sent,
        marker.start.bytes_received,
        marker.end.bytes_received,
        marker.start.packets_sent,
        marker.end.packets_sent,
        marker.start.packets_received,
        marker.end.packets_received,
        role = marker.role,
        participant_count = marker.participant_count,
        result_kind = marker.result_kind,
        result_team_a = marker.result_team_a,
        result_team_b = marker.result_team_b,
        start_tick = marker.start_tick,
        end_tick = marker.end_tick,
    )
}

/// Compute the p`percentile` sample of a bounded microsecond sample list.
#[must_use]
pub fn percentile_micros(samples: &[u32], percentile: f32) -> u32 {
    if samples.is_empty() {
        return 0;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]
    let index = ((sorted.len() as f32 - 1.0) * percentile.clamp(0.0, 1.0)).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}
