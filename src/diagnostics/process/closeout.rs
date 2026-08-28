//! Role-aware terminal evidence selection, report assembly, validation, and one-shot output.

use super::{
    GameplayAggregatesV1, ProcessDiagnosticsSettings, ProcessDiagnosticsState, ProcessExitCategory,
    ProcessExitClassification, RunManifestV1, percentile_micros,
};
use crate::diagnostics::{CloseoutReportV1, unix_micros_now};
use bevy::app::AppExit;
use bevy::prelude::*;
use std::{path::Path, time::SystemTime};

/// Convergence evidence derived from the process's own checkpoint and drop telemetry. The
/// defaults are what a run with no recorded scenario checkpoints reports.
#[derive(Default)]
struct CloseoutEvidence {
    checkpoint_digest: u64,
    observed_checkpoints: usize,
    first_divergence: Option<String>,
    dropped_messages: u64,
}

/// The server is authoritative: it digests the checkpoints it recorded and sums its own
/// bounded-evidence drop counters. It reports no divergence of its own.
#[cfg(feature = "server")]
fn server_closeout_evidence(
    evidence: &crate::combat::CombatEvidenceSnapshots,
    telemetry: Option<&crate::combat::CombatTelemetry>,
) -> CloseoutEvidence {
    let (checkpoint_digest, observed_checkpoints) =
        checkpoint_evidence_digest(&evidence.checkpoints);
    let dropped_messages = telemetry.map_or(0, |telemetry| {
        telemetry
            .dropped_cues
            .saturating_add(telemetry.dropped_records)
            .saturating_add(telemetry.dropped_accepted_shot_timestamps)
    });
    CloseoutEvidence {
        checkpoint_digest,
        observed_checkpoints,
        first_divergence: None,
        dropped_messages,
    }
}

/// The client is the converging endpoint: it digests the checkpoints it reproduced, sums
/// its dropped cue counters, and labels the first expected checkpoint it never matched.
#[cfg(feature = "client")]
fn client_closeout_evidence(
    observation: &crate::combat::client::ClientCombatObservation,
) -> CloseoutEvidence {
    let (checkpoint_digest, observed_checkpoints) =
        checkpoint_evidence_digest(&observation.checkpoints);
    let dropped_messages = observation
        .dropped_cue_stream
        .saturating_add(observation.dropped_cue_timestamps);
    let first_divergence = observation.expected_checkpoints.first().map(|unmatched| {
        format!(
            "checkpoint {} unmatched at tick {}",
            unmatched.checkpoint.as_str(),
            unmatched.snapshot.authoritative_tick
        )
    });
    CloseoutEvidence {
        checkpoint_digest,
        observed_checkpoints,
        first_divergence,
        dropped_messages,
    }
}

/// Digest over a process's recorded checkpoint evidence: ordered `name:encoded-snapshot`
/// pairs, so equal digests across endpoints prove both checkpoint sets and payloads agree.
pub(in crate::diagnostics) fn checkpoint_evidence_digest(
    checkpoints: &std::collections::BTreeMap<String, crate::combat::CombatStateSnapshot>,
) -> (u64, usize) {
    if checkpoints.is_empty() {
        // Zero is reserved for "no checkpoint evidence observed", so idle runs never carry
        // the digest of an empty set.
        return (0, 0);
    }
    let mut values = Vec::with_capacity(checkpoints.len());
    for (name, snapshot) in checkpoints {
        let encoded = crate::combat::encode_state_snapshot(snapshot).unwrap_or_default();
        values.push(format!("{name}:{encoded}"));
    }
    let count = values.len();
    let refs: Vec<&str> = values.iter().map(String::as_str).collect();
    (crate::diagnostics::stable_digest(&refs), count)
}

// Bevy system parameters are owned by the scheduling runtime; `Res` cannot be borrowed here.
// The role-gated evidence parameters are how one finalizer serves both role lanes.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    clippy::too_many_arguments,
    reason = "every parameter is a Bevy system parameter owned by the schedule runtime; the role-gated evidence reads keep one finalization phase instead of duplicated per-role systems"
)]
pub(super) fn finalize_closeout_report(
    mut exits: MessageReader<AppExit>,
    settings: Res<ProcessDiagnosticsSettings>,
    mut state: ResMut<ProcessDiagnosticsState>,
    classification: Res<ProcessExitClassification>,
    protocol: Option<Res<crate::protocol::ProtocolFingerprint>>,
    content: Option<Res<crate::content::GameplayContentFingerprint>>,
    #[cfg(feature = "server")] server_evidence: Option<Res<crate::combat::CombatEvidenceSnapshots>>,
    #[cfg(feature = "server")] combat_telemetry: Option<Res<crate::combat::CombatTelemetry>>,
    #[cfg(feature = "client")] client_observation: Option<
        Res<crate::combat::client::ClientCombatObservation>,
    >,
) {
    if !state.enabled || state.report_written {
        return;
    }
    let Some(exit) = exits.read().next() else {
        return;
    };
    // Finalization is one-shot even when validation or local I/O fails. Re-entering on a later
    // AppExit would mix a different terminal frame into the same report identity.
    state.report_written = true;
    let exit_category = classification.classified_category(exit);
    let manifest = completed_manifest(&settings, &state, protocol.as_deref(), content.as_deref());
    let evidence = resolve_closeout_evidence(
        #[cfg(feature = "server")]
        server_evidence.as_deref(),
        #[cfg(feature = "server")]
        combat_telemetry.as_deref(),
        #[cfg(feature = "client")]
        client_observation.as_deref(),
    );
    let gameplay = std::mem::take(&mut state.gameplay);
    let report = assemble_closeout_report(
        manifest,
        &settings,
        exit_category,
        &state,
        evidence,
        gameplay,
    );
    write_closeout_report(settings.report_path.as_deref(), &report);
}

fn completed_manifest(
    settings: &ProcessDiagnosticsSettings,
    state: &ProcessDiagnosticsState,
    protocol: Option<&crate::protocol::ProtocolFingerprint>,
    content: Option<&crate::content::GameplayContentFingerprint>,
) -> RunManifestV1 {
    let mut manifest = settings.manifest.clone();
    if let Some(protocol) = protocol {
        manifest.registry_fingerprint = protocol.0;
    }
    if let Some(content) = content {
        manifest.content_fingerprint = content.0;
    }
    if manifest.participants.is_empty() && !state.manifest_participants.is_empty() {
        manifest
            .participants
            .clone_from(&state.manifest_participants);
    }
    manifest
}

fn resolve_closeout_evidence(
    #[cfg(feature = "server")] server_evidence: Option<&crate::combat::CombatEvidenceSnapshots>,
    #[cfg(feature = "server")] combat_telemetry: Option<&crate::combat::CombatTelemetry>,
    #[cfg(feature = "client")] client_observation: Option<
        &crate::combat::client::ClientCombatObservation,
    >,
) -> CloseoutEvidence {
    #[cfg_attr(not(any(feature = "server", feature = "client")), allow(unused_mut))]
    let mut evidence = CloseoutEvidence::default();
    #[cfg(feature = "server")]
    if let Some(server) = server_evidence {
        evidence = server_closeout_evidence(server, combat_telemetry);
    }
    #[cfg(feature = "client")]
    if let Some(observation) = client_observation {
        evidence = client_closeout_evidence(observation);
    }
    evidence
}

fn write_closeout_report(path: Option<&Path>, report: &CloseoutReportV1) {
    if let Err(error) = report.validate() {
        bevy::log::error!(?error, "closeout report failed validation; not written");
        return;
    }
    let Some(path) = path else {
        return;
    };
    let contents = report.to_report_lines().join("\n") + "\n";
    if let Err(error) = std::fs::write(path, contents.as_bytes()) {
        bevy::log::error!(path = %path.display(), ?error, "closeout report write failed");
    } else {
        bevy::log::info!(path = %path.display(), "closeout report written");
    }
}

/// Assemble the terminal report from the finalized manifest, the observed process
/// samples, and the consolidated evidence/gameplay blocks. Pure assembly: every value is
/// read from already-owned observation state, so the finalizer stays a lifecycle system.
#[allow(
    clippy::needless_pass_by_value,
    reason = "evidence and gameplay are consumed by the assembled report"
)]
fn assemble_closeout_report(
    manifest: RunManifestV1,
    settings: &ProcessDiagnosticsSettings,
    exit_category: ProcessExitCategory,
    state: &ProcessDiagnosticsState,
    evidence: CloseoutEvidence,
    gameplay: GameplayAggregatesV1,
) -> CloseoutReportV1 {
    let fixed = state.fixed_tick_samples.ordered();
    let rtt = state.rtt_samples.ordered();
    let jitter = state.jitter_samples.ordered();
    CloseoutReportV1 {
        manifest,
        end_reason: settings.end_reason.clone(),
        exit_category,
        started_at_unix_micros: system_time_micros(settings.started_at),
        ended_at_unix_micros: unix_micros_now(),
        fixed_ticks: state.fixed_ticks,
        wall_duration_micros: u64::try_from(settings.started_instant.elapsed().as_micros())
            .unwrap_or(u64::MAX),
        fixed_tick_p50_micros: percentile_micros(&fixed, 0.50),
        fixed_tick_p95_micros: percentile_micros(&fixed, 0.95),
        fixed_tick_max_micros: fixed.iter().copied().max().unwrap_or(0),
        entity_high_water: state.entity_high_water,
        link_high_water: state.link_high_water,
        terminal_entities: state.terminal_entities.unwrap_or(0),
        terminal_links: state.terminal_links.unwrap_or(0),
        rtt_p50_micros: percentile_micros(&rtt, 0.50),
        rtt_p95_micros: percentile_micros(&rtt, 0.95),
        rtt_max_micros: rtt.iter().copied().max().unwrap_or(0),
        jitter_p50_micros: percentile_micros(&jitter, 0.50),
        jitter_p95_micros: percentile_micros(&jitter, 0.95),
        jitter_max_micros: jitter.iter().copied().max().unwrap_or(0),
        transport_bytes_sent: state.transport.bytes_sent,
        transport_bytes_received: state.transport.bytes_received,
        packets_sent: state.transport.packets_sent,
        packets_received: state.transport.packets_received,
        channel_messages_sent: state.transport.channel_messages_sent,
        channel_messages_received: state.transport.channel_messages_received,
        checkpoint_digest: evidence.checkpoint_digest,
        checkpoints_observed: u32::try_from(evidence.observed_checkpoints).unwrap_or(u32::MAX),
        first_divergence: evidence.first_divergence,
        dropped_messages: evidence.dropped_messages,
        rejected_connections: state.rejected_connections,
        error_count: state.error_count,
        gameplay,
    }
}

/// Bounded, separator-free identity for one manifest participant row. `:` separates the
/// fields because the manifest validator rejects `=` and newlines inside identity values.
pub(in crate::diagnostics) fn participant_build_identity(
    build: &crate::builds::SelectedBuild,
) -> String {
    format!(
        "fingerprint:{} revision:{}",
        build.recipe_fingerprint.0,
        u64::from(build.revision.0)
    )
}

fn system_time_micros(when: SystemTime) -> u64 {
    when.duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
        })
}
