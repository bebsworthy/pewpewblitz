//! Bounded process diagnostics: versioned run manifests, closeout reports, and failure records.
//!
//! Everything in this module is observational. Diagnostics may read ECS and network state and
//! write local reports or overlay UI, but never mutate gameplay, validation, authority,
//! replication targets, match results, or map dynamics.

mod failure;
#[cfg(feature = "client")]
mod overlay;
mod process;

pub use failure::{
    FailureCategory, ProcessFailureRecordV1, install_panic_failure_hook, write_failure_record,
};
#[cfg(feature = "client")]
pub use overlay::ClientDiagnosticsOverlayPlugin;
#[cfg(feature = "server")]
pub(crate) use process::ProcessDiagnosticsState;
pub use process::{
    DiagnosticsSet, ProcessDiagnosticsPlugin, ProcessDiagnosticsSettings,
    ProcessExitClassification, TerminalObservationSet, TransportCounters, percentile_micros,
};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// The only closeout schema revision produced or accepted by this build. Revision 2 adds
/// the gameplay-aggregate block (match/mode, build, ability, weapon, and map dynamics
/// summaries) to revision 1's manifest, process, transport, and evidence fields, so the
/// consolidated report ties the gameplay subsystems to the run identity instead of
/// carrying process/network measurements alone.
/// The only closeout schema revision produced or accepted by this build. Revision 3 adds
/// the mode-aggregate fields (mode identity plus the typed Wipeout/Hot Zone summaries),
/// the observed-checkpoint count distinct from the declared scenario contract, the
/// map-destruction no-op brush counter, and includes the declared scenario counts in the shared
/// run identity.
pub const CLOSEOUT_SCHEMA_VERSION: u16 = 3;

/// Upper bound on manifest identity strings; rejects oversized or runaway scripted fields.
pub const MAX_IDENTITY_BYTES: usize = 96;

/// Upper bound on manifest participants; matches the engine's 24-fighter map capacity.
pub const MAX_MANIFEST_PARTICIPANTS: usize = 24;

/// Upper bound on closeout report lines before the report itself is rejected as malformed:
/// every required field exactly once plus two row lines per bounded participant.
pub const MAX_REPORT_LINES: usize = REPORT_REQUIRED_KEYS.len() + 2 * MAX_MANIFEST_PARTICIPANTS;

/// Stable process exit categories shared by failure records and closeout reports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessExitCategory {
    #[default]
    CleanExit,
    Configuration,
    EndpointStart,
    ProtocolMismatch,
    ContentMismatch,
    VerificationFailed,
    Timeout,
    Panic,
    ShutdownIncomplete,
}

impl ProcessExitCategory {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::CleanExit => "clean-exit",
            Self::Configuration => "configuration",
            Self::EndpointStart => "endpoint-start",
            Self::ProtocolMismatch => "protocol-mismatch",
            Self::ContentMismatch => "content-mismatch",
            Self::VerificationFailed => "verification-failed",
            Self::Timeout => "timeout",
            Self::Panic => "panic",
            Self::ShutdownIncomplete => "shutdown-incomplete",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value.trim() {
            "clean-exit" => Self::CleanExit,
            "configuration" => Self::Configuration,
            "endpoint-start" => Self::EndpointStart,
            "protocol-mismatch" => Self::ProtocolMismatch,
            "content-mismatch" => Self::ContentMismatch,
            "verification-failed" => Self::VerificationFailed,
            "timeout" => Self::Timeout,
            "panic" => Self::Panic,
            "shutdown-incomplete" => Self::ShutdownIncomplete,
            _ => return None,
        })
    }

    #[must_use]
    pub fn from_app_exit(exit: &AppExit) -> Self {
        if exit.is_error() {
            Self::ShutdownIncomplete
        } else {
            Self::CleanExit
        }
    }
}

/// One manifest participant row: stable player identity and selected build identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestParticipant {
    pub player_id: u64,
    pub build_identity: String,
}

/// Versioned identity of one deterministic scenario run.
///
/// The manifest names what was run; the closeout report records what was observed. Neither
/// embeds unbounded event history: scripted actions and checkpoints are referenced by count,
/// with their digests carried in the report.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunManifestV1 {
    pub schema_version: u16,
    pub scenario_id: String,
    pub scenario_revision: u32,
    pub run_id: String,
    pub build_version: String,
    pub source_revision: String,
    pub source_dirty: bool,
    pub protocol_version: u16,
    pub registry_fingerprint: u64,
    pub content_fingerprint: u64,
    pub mode: String,
    pub rules_profile: String,
    pub network_profile: String,
    pub render_profile: String,
    pub seed: u64,
    pub participants: Vec<ManifestParticipant>,
    pub scripted_action_count: u32,
    pub checkpoint_count: u32,
}

impl RunManifestV1 {
    /// Validate schema revision, identity bounds, and participant capacity.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != CLOSEOUT_SCHEMA_VERSION {
            return Err(format!(
                "unknown closeout schema revision {} (expected {}); refuse to interpret the report",
                self.schema_version, CLOSEOUT_SCHEMA_VERSION
            ));
        }
        for (label, value) in [
            ("scenario_id", &self.scenario_id),
            ("run_id", &self.run_id),
            ("build_version", &self.build_version),
            ("source_revision", &self.source_revision),
            ("mode", &self.mode),
            ("rules_profile", &self.rules_profile),
            ("network_profile", &self.network_profile),
            ("render_profile", &self.render_profile),
        ] {
            if value.trim().is_empty() {
                return Err(format!("manifest {label} must not be empty"));
            }
            if value.len() > MAX_IDENTITY_BYTES {
                return Err(format!(
                    "manifest {label} exceeds {MAX_IDENTITY_BYTES} bytes"
                ));
            }
            if value.contains(['\n', '\r', '=']) {
                return Err(format!(
                    "manifest {label} must not contain newlines or '=' separators"
                ));
            }
        }
        if self.participants.len() > MAX_MANIFEST_PARTICIPANTS {
            return Err(format!(
                "manifest declares {} participants above the {} cap",
                self.participants.len(),
                MAX_MANIFEST_PARTICIPANTS
            ));
        }
        for participant in &self.participants {
            if participant.build_identity.trim().is_empty()
                || participant.build_identity.len() > MAX_IDENTITY_BYTES
                || participant.build_identity.contains(['\n', '\r', '='])
            {
                return Err(format!(
                    "manifest participant {} has an invalid build identity",
                    participant.player_id
                ));
            }
        }
        Ok(())
    }

    /// Render the manifest as deterministic shell-readable `key=value` lines.
    ///
    /// Field order is fixed by this method; consumers may rely on it for diffs.
    #[must_use]
    pub fn to_report_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("schema_version={}", self.schema_version),
            format!("scenario_id={}", self.scenario_id),
            format!("scenario_revision={}", self.scenario_revision),
            format!("run_id={}", self.run_id),
            format!("build_version={}", self.build_version),
            format!("source_revision={}", self.source_revision),
            format!("source_dirty={}", self.source_dirty),
            format!("protocol_version={}", self.protocol_version),
            format!("registry_fingerprint={}", self.registry_fingerprint),
            format!("content_fingerprint={}", self.content_fingerprint),
            format!("mode={}", self.mode),
            format!("rules_profile={}", self.rules_profile),
            format!("network_profile={}", self.network_profile),
            format!("render_profile={}", self.render_profile),
            format!("seed={}", self.seed),
            format!("participants={}", self.participants.len()),
        ];
        for (index, participant) in self.participants.iter().enumerate() {
            lines.push(format!(
                "participant_{}_player_id={}",
                index, participant.player_id
            ));
            lines.push(format!(
                "participant_{}_build={}",
                index, participant.build_identity
            ));
        }
        lines.push(format!("scripted_actions={}", self.scripted_action_count));
        lines.push(format!("checkpoints={}", self.checkpoint_count));
        lines
    }
}

/// The gameplay-aggregate block of one closeout report: the bounded telemetry summaries
/// the process consolidated at terminal observation. The authoritative
/// match/mode/weapon/ability aggregates and build selections exist only in the server
/// process, while map-destruction aggregates exist in both roles (the client records its own
/// convergence facts), so a client legitimately reports zeros outside map dynamics.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GameplayAggregatesV1 {
    /// Completed matches recorded by this process's match telemetry.
    pub matches_completed: u32,
    /// Terminal result of the latest completed match (`none` without one), as a
    /// [`crate::matchplay::MatchResult`] report label.
    pub match_result: Option<String>,
    pub match_active_ticks: u64,
    pub match_respawns: u32,
    pub team_a_defeats: u32,
    pub team_b_defeats: u32,
    pub first_hostile_damage_tick: Option<u64>,
    /// Stable authored mode-definition ID of the latest completed match. `None` while
    /// the process completed no match.
    pub mode_definition_id: Option<u16>,
    /// Wipeout terminal scores (`None` unless the completed match was Wipeout).
    pub wipeout_final_scores: Option<[u16; 2]>,
    pub wipeout_target_score: Option<u16>,
    pub wipeout_score_margin: Option<u16>,
    /// Hot Zone terminal state and objective-behavior counters (`None` unless the
    /// completed match was Hot Zone).
    pub hot_zone_final_progress: Option<[u16; 2]>,
    pub hot_zone_target_progress_ticks: Option<u16>,
    pub hot_zone_controlled_ticks: Option<[u64; 2]>,
    pub hot_zone_contested_ticks: Option<u64>,
    pub hot_zone_control_gained_transitions: Option<[u32; 2]>,
    pub hot_zone_longest_control_ticks: Option<[u64; 2]>,
    /// Build selections accepted by this process's build telemetry.
    pub build_selections: u32,
    pub build_dropped_records: u64,
    pub ability_attempts: u64,
    pub ability_accepts: u64,
    pub dash_uses: u64,
    pub sentry_uses: u64,
    pub accepted_attacks: u64,
    pub emitted_deliveries: u64,
    pub attacks_with_hostile_contact: u64,
    pub hostile_damage: u64,
    pub map_destruction_requested: u64,
    pub map_destruction_applied: u64,
    pub map_destruction_no_ops: u64,
    pub map_destruction_rejected: u64,
    pub map_destruction_deferred: u64,
    pub map_placements_changed: u64,
}

impl GameplayAggregatesV1 {
    /// Enforce the block's semantic contract: aggregates only reference matches the
    /// process completed, a completed match carries a result label plus exactly one
    /// complete and internally consistent mode summary, weapon contact cannot exceed
    /// accepted attacks, and terminal map-destruction outcomes stay inside the
    /// submitted-brush count.
    fn validate(&self) -> Result<(), String> {
        if self.matches_completed == 0 {
            if self.match_result.is_some()
                || self.match_active_ticks != 0
                || self.match_respawns != 0
                || self.mode_definition_id.is_some()
                || self.wipeout_fields_some()
                || self.hot_zone_fields_some()
            {
                return Err(
                    "closeout gameplay aggregates reference a match the process did not complete"
                        .to_string(),
                );
            }
        } else if self
            .match_result
            .as_deref()
            .and_then(crate::matchplay::MatchResult::parse_report_label)
            .is_none()
        {
            return Err("closeout match_result is missing or not a match result label".to_string());
        }
        if self.matches_completed > 0 && self.mode_definition_id.is_none() {
            return Err("closeout mode_definition_id is missing".to_string());
        }
        self.validate_mode_summary()?;
        if self.attacks_with_hostile_contact > self.accepted_attacks {
            return Err(
                "closeout weapon aggregates carry more attacks with contact than accepted attacks"
                    .to_string(),
            );
        }
        // Deferral is a lifecycle event, not a terminal outcome: a brush deferred for
        // admission or collider budget is re-queued and later counted in exactly one of
        // the applied, no-op, or rejected terminal buckets.
        if self
            .map_destruction_applied
            .saturating_add(self.map_destruction_no_ops)
            .saturating_add(self.map_destruction_rejected)
            > self.map_destruction_requested
        {
            return Err(
                "closeout map-destruction terminal outcomes exceed the submitted requests"
                    .to_string(),
            );
        }
        Ok(())
    }

    fn wipeout_fields_some(&self) -> bool {
        self.wipeout_final_scores.is_some()
            || self.wipeout_target_score.is_some()
            || self.wipeout_score_margin.is_some()
    }

    fn hot_zone_fields_some(&self) -> bool {
        self.hot_zone_final_progress.is_some()
            || self.hot_zone_target_progress_ticks.is_some()
            || self.hot_zone_controlled_ticks.is_some()
            || self.hot_zone_contested_ticks.is_some()
            || self.hot_zone_control_gained_transitions.is_some()
            || self.hot_zone_longest_control_ticks.is_some()
    }

    /// Exactly one mode variant may carry fields, and it must be complete and internally
    /// consistent with the summary it claims to be.
    fn validate_mode_summary(&self) -> Result<(), String> {
        match self.mode_definition_id {
            Some(id) if id == crate::map::WIPEOUT_MODE_DEFINITION.0 => {
                let (Some(scores), Some(_), Some(margin)) = (
                    self.wipeout_final_scores,
                    self.wipeout_target_score,
                    self.wipeout_score_margin,
                ) else {
                    return Err("closeout wipeout mode aggregates are incomplete".to_string());
                };
                if self.hot_zone_fields_some() {
                    return Err(
                        "closeout wipeout mode aggregates carry hot-zone fields".to_string()
                    );
                }
                if margin != scores[0].abs_diff(scores[1]) {
                    return Err(
                        "closeout wipeout score margin does not match the final scores".to_string(),
                    );
                }
            }
            Some(id) if id == crate::map::HOT_ZONE_MODE_DEFINITION.0 => {
                let (Some(progress), Some(target), Some(_), Some(_), Some(_), Some(_)) = (
                    self.hot_zone_final_progress,
                    self.hot_zone_target_progress_ticks,
                    self.hot_zone_controlled_ticks,
                    self.hot_zone_contested_ticks,
                    self.hot_zone_control_gained_transitions,
                    self.hot_zone_longest_control_ticks,
                ) else {
                    return Err("closeout hot-zone mode aggregates are incomplete".to_string());
                };
                if self.wipeout_fields_some() {
                    return Err(
                        "closeout hot-zone mode aggregates carry wipeout fields".to_string()
                    );
                }
                if progress[0] > target || progress[1] > target {
                    return Err("closeout hot-zone final progress exceeds the target".to_string());
                }
            }
            None => {
                if self.wipeout_fields_some() || self.hot_zone_fields_some() {
                    return Err(
                        "closeout mode aggregates carry fields without a mode identity".to_string(),
                    );
                }
            }
            Some(_) => {
                return Err("closeout mode_definition_id is not a supported match mode".to_string());
            }
        }
        Ok(())
    }

    /// Render the block's deterministic `key=value` lines.
    #[must_use]
    fn to_report_lines(&self) -> Vec<String> {
        let render_optional_u16 = |value: Option<u16>| {
            value.map_or_else(|| "none".to_string(), |value| value.to_string())
        };
        let render_optional_u64 = |value: Option<u64>| {
            value.map_or_else(|| "none".to_string(), |value| value.to_string())
        };
        let render_u16_pair = |value: Option<[u16; 2]>| {
            value.map_or_else(|| "none".to_string(), |[a, b]| format!("{a}:{b}"))
        };
        let render_u64_pair = |value: Option<[u64; 2]>| {
            value.map_or_else(|| "none".to_string(), |[a, b]| format!("{a}:{b}"))
        };
        let render_u32_pair = |value: Option<[u32; 2]>| {
            value.map_or_else(|| "none".to_string(), |[a, b]| format!("{a}:{b}"))
        };
        vec![
            format!("matches_completed={}", self.matches_completed),
            format!(
                "match_result={}",
                self.match_result.as_deref().unwrap_or("none")
            ),
            format!(
                "mode_definition_id={}",
                render_optional_u16(self.mode_definition_id)
            ),
            format!(
                "wipeout_final_scores={}",
                render_u16_pair(self.wipeout_final_scores)
            ),
            format!(
                "wipeout_target_score={}",
                render_optional_u16(self.wipeout_target_score)
            ),
            format!(
                "wipeout_score_margin={}",
                render_optional_u16(self.wipeout_score_margin)
            ),
            format!(
                "hot_zone_final_progress={}",
                render_u16_pair(self.hot_zone_final_progress)
            ),
            format!(
                "hot_zone_target_progress_ticks={}",
                render_optional_u16(self.hot_zone_target_progress_ticks)
            ),
            format!(
                "hot_zone_controlled_ticks={}",
                render_u64_pair(self.hot_zone_controlled_ticks)
            ),
            format!(
                "hot_zone_contested_ticks={}",
                render_optional_u64(self.hot_zone_contested_ticks)
            ),
            format!(
                "hot_zone_control_gained_transitions={}",
                render_u32_pair(self.hot_zone_control_gained_transitions)
            ),
            format!(
                "hot_zone_longest_control_ticks={}",
                render_u64_pair(self.hot_zone_longest_control_ticks)
            ),
            format!("match_active_ticks={}", self.match_active_ticks),
            format!("match_respawns={}", self.match_respawns),
            format!("team_a_defeats={}", self.team_a_defeats),
            format!("team_b_defeats={}", self.team_b_defeats),
            format!(
                "first_hostile_damage_tick={}",
                self.first_hostile_damage_tick
                    .map_or_else(|| "none".to_string(), |tick| tick.to_string())
            ),
            format!("build_selections={}", self.build_selections),
            format!("build_dropped_records={}", self.build_dropped_records),
            format!("ability_attempts={}", self.ability_attempts),
            format!("ability_accepts={}", self.ability_accepts),
            format!("dash_uses={}", self.dash_uses),
            format!("sentry_uses={}", self.sentry_uses),
            format!("accepted_attacks={}", self.accepted_attacks),
            format!("emitted_deliveries={}", self.emitted_deliveries),
            format!(
                "attacks_with_hostile_contact={}",
                self.attacks_with_hostile_contact
            ),
            format!("hostile_damage={}", self.hostile_damage),
            format!(
                "map_destruction_requested={}",
                self.map_destruction_requested
            ),
            format!("map_destruction_applied={}", self.map_destruction_applied),
            format!("map_destruction_no_ops={}", self.map_destruction_no_ops),
            format!("map_destruction_rejected={}", self.map_destruction_rejected),
            format!("map_destruction_deferred={}", self.map_destruction_deferred),
            format!("map_placements_changed={}", self.map_placements_changed),
        ]
    }

    /// Reconstruct the block from already-presence-checked report lines, failing any
    /// value that does not encode its declared scalar type.
    fn from_report_lines(lines: &[(&str, &str)]) -> Result<Self, String> {
        let parse_optional_tick =
            |key: &str| -> Result<Option<u64>, String> { parse_optional_field(lines, key, "u64") };
        Ok(Self {
            matches_completed: parse_typed_field(lines, "matches_completed", "u32")?,
            match_result: match parse_report_field(lines, "match_result")
                .expect("presence was checked")
            {
                "none" => None,
                label => Some(label.to_string()),
            },
            mode_definition_id: parse_optional_field(lines, "mode_definition_id", "u16")?,
            wipeout_final_scores: parse_optional_pair(lines, "wipeout_final_scores", "u16")?,
            wipeout_target_score: parse_optional_field(lines, "wipeout_target_score", "u16")?,
            wipeout_score_margin: parse_optional_field(lines, "wipeout_score_margin", "u16")?,
            hot_zone_final_progress: parse_optional_pair(lines, "hot_zone_final_progress", "u16")?,
            hot_zone_target_progress_ticks: parse_optional_field(
                lines,
                "hot_zone_target_progress_ticks",
                "u16",
            )?,
            hot_zone_controlled_ticks: parse_optional_pair(
                lines,
                "hot_zone_controlled_ticks",
                "u64",
            )?,
            hot_zone_contested_ticks: parse_optional_field(
                lines,
                "hot_zone_contested_ticks",
                "u64",
            )?,
            hot_zone_control_gained_transitions: parse_optional_pair(
                lines,
                "hot_zone_control_gained_transitions",
                "u32",
            )?,
            hot_zone_longest_control_ticks: parse_optional_pair(
                lines,
                "hot_zone_longest_control_ticks",
                "u64",
            )?,
            match_active_ticks: parse_typed_field(lines, "match_active_ticks", "u64")?,
            match_respawns: parse_typed_field(lines, "match_respawns", "u32")?,
            team_a_defeats: parse_typed_field(lines, "team_a_defeats", "u32")?,
            team_b_defeats: parse_typed_field(lines, "team_b_defeats", "u32")?,
            first_hostile_damage_tick: parse_optional_tick("first_hostile_damage_tick")?,
            build_selections: parse_typed_field(lines, "build_selections", "u32")?,
            build_dropped_records: parse_typed_field(lines, "build_dropped_records", "u64")?,
            ability_attempts: parse_typed_field(lines, "ability_attempts", "u64")?,
            ability_accepts: parse_typed_field(lines, "ability_accepts", "u64")?,
            dash_uses: parse_typed_field(lines, "dash_uses", "u64")?,
            sentry_uses: parse_typed_field(lines, "sentry_uses", "u64")?,
            accepted_attacks: parse_typed_field(lines, "accepted_attacks", "u64")?,
            emitted_deliveries: parse_typed_field(lines, "emitted_deliveries", "u64")?,
            attacks_with_hostile_contact: parse_typed_field(
                lines,
                "attacks_with_hostile_contact",
                "u64",
            )?,
            hostile_damage: parse_typed_field(lines, "hostile_damage", "u64")?,
            map_destruction_requested: parse_typed_field(
                lines,
                "map_destruction_requested",
                "u64",
            )?,
            map_destruction_applied: parse_typed_field(lines, "map_destruction_applied", "u64")?,
            map_destruction_no_ops: parse_typed_field(lines, "map_destruction_no_ops", "u64")?,
            map_destruction_rejected: parse_typed_field(lines, "map_destruction_rejected", "u64")?,
            map_destruction_deferred: parse_typed_field(lines, "map_destruction_deferred", "u64")?,
            map_placements_changed: parse_typed_field(lines, "map_placements_changed", "u64")?,
        })
    }
}

/// Versioned consolidated observation record for one scenario run.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CloseoutReportV1 {
    pub manifest: RunManifestV1,
    pub end_reason: String,
    pub exit_category: ProcessExitCategory,
    pub started_at_unix_micros: u64,
    pub ended_at_unix_micros: u64,
    pub fixed_ticks: u64,
    pub wall_duration_micros: u64,
    pub fixed_tick_p50_micros: u32,
    pub fixed_tick_p95_micros: u32,
    pub fixed_tick_max_micros: u32,
    pub entity_high_water: u32,
    pub link_high_water: u32,
    pub terminal_entities: u32,
    pub terminal_links: u32,
    pub rtt_p50_micros: u32,
    pub rtt_p95_micros: u32,
    pub rtt_max_micros: u32,
    pub jitter_p50_micros: u32,
    pub jitter_p95_micros: u32,
    pub jitter_max_micros: u32,
    pub transport_bytes_sent: u64,
    pub transport_bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub channel_messages_sent: u64,
    pub channel_messages_received: u64,
    pub checkpoint_digest: u64,
    /// Checkpoint evidence this process actually observed. Distinct from the manifest's
    /// declared `checkpoint_count`, which preserves the scenario's expectation so the
    /// terminal gate can compare declaration against observation.
    pub checkpoints_observed: u32,
    pub first_divergence: Option<String>,
    pub dropped_messages: u64,
    pub rejected_connections: u64,
    pub error_count: u64,
    /// Consolidated build, ability, match/mode, weapon, and map dynamics summaries.
    pub gameplay: GameplayAggregatesV1,
}

impl CloseoutReportV1 {
    /// Validate the manifest plus report-level bounds before the report is trusted.
    pub fn validate(&self) -> Result<(), String> {
        self.manifest.validate()?;
        if self.end_reason.trim().is_empty() || self.end_reason.len() > MAX_IDENTITY_BYTES {
            return Err("closeout end_reason is empty or oversized".to_string());
        }
        if self.end_reason.contains(['\n', '\r', '=']) {
            return Err("closeout end_reason must not contain newlines or '='".to_string());
        }
        if let Some(divergence) = &self.first_divergence
            && (divergence.is_empty() || divergence.len() > MAX_IDENTITY_BYTES)
        {
            return Err("closeout first_divergence must be non-empty and bounded".to_string());
        }
        if self
            .first_divergence
            .as_ref()
            .is_some_and(|divergence| divergence.contains(['\n', '\r', '=']))
        {
            return Err("closeout first_divergence must not contain newlines or '='".to_string());
        }
        if self.ended_at_unix_micros < self.started_at_unix_micros {
            return Err("closeout end timestamp precedes its start timestamp".to_string());
        }
        if self.fixed_tick_p50_micros > self.fixed_tick_p95_micros
            || self.fixed_tick_p95_micros > self.fixed_tick_max_micros
        {
            return Err("closeout fixed-tick percentiles are not monotonic".to_string());
        }
        if self.rtt_p50_micros > self.rtt_p95_micros || self.rtt_p95_micros > self.rtt_max_micros {
            return Err("closeout RTT percentiles are not monotonic".to_string());
        }
        if self.jitter_p50_micros > self.jitter_p95_micros
            || self.jitter_p95_micros > self.jitter_max_micros
        {
            return Err("closeout jitter percentiles are not monotonic".to_string());
        }
        if self.terminal_entities > self.entity_high_water
            || self.terminal_links > self.link_high_water
        {
            return Err("closeout terminal counts exceed recorded high-water marks".to_string());
        }
        self.gameplay.validate()?;
        Ok(())
    }

    /// Render the report as deterministic shell-readable `key=value` lines.
    #[must_use]
    pub fn to_report_lines(&self) -> Vec<String> {
        let mut lines = self.manifest.to_report_lines();
        lines.extend([
            format!("end_reason={}", self.end_reason),
            format!("exit_category={}", self.exit_category.name()),
            format!("started_at_unix_micros={}", self.started_at_unix_micros),
            format!("ended_at_unix_micros={}", self.ended_at_unix_micros),
            format!("fixed_ticks={}", self.fixed_ticks),
            format!("wall_duration_micros={}", self.wall_duration_micros),
            format!("fixed_tick_p50_micros={}", self.fixed_tick_p50_micros),
            format!("fixed_tick_p95_micros={}", self.fixed_tick_p95_micros),
            format!("fixed_tick_max_micros={}", self.fixed_tick_max_micros),
            format!("entity_high_water={}", self.entity_high_water),
            format!("link_high_water={}", self.link_high_water),
            format!("terminal_entities={}", self.terminal_entities),
            format!("terminal_links={}", self.terminal_links),
            format!("rtt_p50_micros={}", self.rtt_p50_micros),
            format!("rtt_p95_micros={}", self.rtt_p95_micros),
            format!("rtt_max_micros={}", self.rtt_max_micros),
            format!("jitter_p50_micros={}", self.jitter_p50_micros),
            format!("jitter_p95_micros={}", self.jitter_p95_micros),
            format!("jitter_max_micros={}", self.jitter_max_micros),
            format!("transport_bytes_sent={}", self.transport_bytes_sent),
            format!("transport_bytes_received={}", self.transport_bytes_received),
            format!("packets_sent={}", self.packets_sent),
            format!("packets_received={}", self.packets_received),
            format!("channel_messages_sent={}", self.channel_messages_sent),
            format!(
                "channel_messages_received={}",
                self.channel_messages_received
            ),
            format!("checkpoint_digest={}", self.checkpoint_digest),
            format!("checkpoints_observed={}", self.checkpoints_observed),
            format!(
                "first_divergence={}",
                self.first_divergence.as_deref().unwrap_or("none")
            ),
            format!("dropped_messages={}", self.dropped_messages),
            format!("rejected_connections={}", self.rejected_connections),
            format!("error_count={}", self.error_count),
        ]);
        lines.extend(self.gameplay.to_report_lines());
        lines
    }
}

/// Parse `key=value` report lines, rejecting duplicates, unknown schema revisions, and
/// missing/oversized required fields. This is the contract verification scripts rely on.
#[must_use]
pub fn parse_report_field<'a>(lines: &'a [(&'a str, &'a str)], key: &str) -> Option<&'a str> {
    let mut found: Option<&str> = None;
    for (candidate, value) in lines {
        if *candidate == key {
            if found.is_some() {
                return None;
            }
            found = Some(value);
        }
    }
    found
}

/// Split raw report text into `(key, value)` pairs, failing on malformed lines or size overrun.
pub fn split_report_lines(contents: &str) -> Result<Vec<(&str, &str)>, String> {
    let mut lines = Vec::new();
    for line in contents.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if lines.len() >= MAX_REPORT_LINES {
            return Err(format!("closeout report exceeds {MAX_REPORT_LINES} lines"));
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("malformed report line: {line}"))?;
        if key.is_empty() || value.is_empty() {
            return Err(format!("malformed report line: {line}"));
        }
        lines.push((key, value));
    }
    Ok(lines)
}

/// Every non-participant field a schema-3 closeout report carries exactly once. The reader
/// rejects reports that drop, duplicate, or oversize any of these fields, mirroring
/// `CloseoutReportV1::to_report_lines`.
const REPORT_REQUIRED_KEYS: [&str; 82] = [
    "schema_version",
    "scenario_id",
    "scenario_revision",
    "run_id",
    "build_version",
    "source_revision",
    "source_dirty",
    "protocol_version",
    "registry_fingerprint",
    "content_fingerprint",
    "mode",
    "rules_profile",
    "network_profile",
    "render_profile",
    "seed",
    "participants",
    "scripted_actions",
    "checkpoints",
    "end_reason",
    "exit_category",
    "started_at_unix_micros",
    "ended_at_unix_micros",
    "fixed_ticks",
    "wall_duration_micros",
    "fixed_tick_p50_micros",
    "fixed_tick_p95_micros",
    "fixed_tick_max_micros",
    "entity_high_water",
    "link_high_water",
    "terminal_entities",
    "terminal_links",
    "rtt_p50_micros",
    "rtt_p95_micros",
    "rtt_max_micros",
    "jitter_p50_micros",
    "jitter_p95_micros",
    "jitter_max_micros",
    "transport_bytes_sent",
    "transport_bytes_received",
    "packets_sent",
    "packets_received",
    "channel_messages_sent",
    "channel_messages_received",
    "checkpoint_digest",
    "checkpoints_observed",
    "first_divergence",
    "dropped_messages",
    "rejected_connections",
    "error_count",
    "matches_completed",
    "match_result",
    "mode_definition_id",
    "wipeout_final_scores",
    "wipeout_target_score",
    "wipeout_score_margin",
    "hot_zone_final_progress",
    "hot_zone_target_progress_ticks",
    "hot_zone_controlled_ticks",
    "hot_zone_contested_ticks",
    "hot_zone_control_gained_transitions",
    "hot_zone_longest_control_ticks",
    "match_active_ticks",
    "match_respawns",
    "team_a_defeats",
    "team_b_defeats",
    "first_hostile_damage_tick",
    "build_selections",
    "build_dropped_records",
    "ability_attempts",
    "ability_accepts",
    "dash_uses",
    "sentry_uses",
    "accepted_attacks",
    "emitted_deliveries",
    "attacks_with_hostile_contact",
    "hostile_damage",
    "map_destruction_requested",
    "map_destruction_applied",
    "map_destruction_no_ops",
    "map_destruction_rejected",
    "map_destruction_deferred",
    "map_placements_changed",
];

/// Report fields whose values are bounded identity strings. The reader enforces the same
/// bound the writer's `validate` path enforces, so an oversized identity cannot slip into
/// a verification script through the file format.
const REPORT_IDENTITY_KEYS: [&str; 12] = [
    "scenario_id",
    "run_id",
    "build_version",
    "source_revision",
    "mode",
    "rules_profile",
    "network_profile",
    "render_profile",
    "end_reason",
    "first_divergence",
    "exit_category",
    "match_result",
];

/// Parse one required field as its declared scalar type. Presence was checked before,
/// so a failure here means the value does not encode the schema's type.
fn parse_typed_field<T: core::str::FromStr>(
    lines: &[(&str, &str)],
    key: &str,
    type_name: &str,
) -> Result<T, String> {
    let value = parse_report_field(lines, key)
        .ok_or_else(|| format!("missing or duplicated required field: {key}"))?;
    T::from_str(value).map_err(|_| format!("{key}={value} is not a {type_name}"))
}

/// Parse one required optional-scalar field: `none` maps to `None`, anything else must
/// encode the declared scalar type. Presence was checked before.
fn parse_optional_field<T: core::str::FromStr>(
    lines: &[(&str, &str)],
    key: &str,
    type_name: &str,
) -> Result<Option<T>, String> {
    match parse_report_field(lines, key).expect("presence was checked") {
        "none" => Ok(None),
        value => T::from_str(value)
            .map(Some)
            .map_err(|_| format!("{key}={value} is not a {type_name}")),
    }
}

/// Parse one required `a:b` pair field, where each half must encode the declared scalar
/// type; `none` maps to `None`. Presence was checked before.
fn parse_optional_pair<T: core::str::FromStr>(
    lines: &[(&str, &str)],
    key: &str,
    type_name: &str,
) -> Result<Option<[T; 2]>, String> {
    let value = parse_report_field(lines, key).expect("presence was checked");
    if value == "none" {
        return Ok(None);
    }
    let (left, right) = value
        .split_once(':')
        .ok_or_else(|| format!("{key}={value} is not an a:b pair"))?;
    let left = T::from_str(left).map_err(|_| format!("{key}={value} is not a {type_name} pair"))?;
    let right =
        T::from_str(right).map_err(|_| format!("{key}={value} is not a {type_name} pair"))?;
    Ok(Some([left, right]))
}

/// One already-presence-checked field value, as an owned string.
fn owned_field(lines: &[(&str, &str)], key: &str) -> String {
    parse_report_field(lines, key)
        .unwrap_or_default()
        .to_string()
}

/// Parse and validate report lines into the closeout report they must encode.
///
/// Beyond requiring every field exactly once with bounded identities, every numeric and
/// boolean field must parse as its declared type, and the reconstructed report must pass
/// [`CloseoutReportV1::validate`]. A syntactically complete but semantically malformed
/// report (non-numeric counters, inverted timestamps, non-monotonic percentiles) cannot
/// satisfy the reader, so verification gates cannot pass on stale or corrupt files that
/// merely carry the right field names.
pub fn parse_closeout_report(lines: &[(&str, &str)]) -> Result<CloseoutReportV1, String> {
    let mut seen = std::collections::HashSet::new();
    for (key, _) in lines {
        if !seen.insert(*key) {
            return Err(format!("duplicate report field: {key}"));
        }
    }
    let schema = parse_typed_field(lines, "schema_version", "u16")?;
    if schema != CLOSEOUT_SCHEMA_VERSION {
        return Err(format!(
            "unknown closeout schema revision {schema} (expected {CLOSEOUT_SCHEMA_VERSION})"
        ));
    }
    for key in REPORT_REQUIRED_KEYS {
        if parse_report_field(lines, key).is_none() {
            return Err(format!("missing or duplicated required field: {key}"));
        }
    }
    for key in REPORT_IDENTITY_KEYS {
        let value = parse_report_field(lines, key).expect("presence was checked above");
        if value.is_empty() || value.len() > MAX_IDENTITY_BYTES || value.contains('=') {
            return Err(format!(
                "report field {key} is empty, oversized, or carries an '=' separator"
            ));
        }
    }
    let exit_category = parse_report_field(lines, "exit_category").expect("presence was checked");
    let exit_category = ProcessExitCategory::parse(exit_category)
        .ok_or_else(|| format!("unknown exit_category {exit_category}"))?;
    validate_report_participant_block(lines)?;
    let manifest = parse_run_manifest(lines, schema)?;
    let report = CloseoutReportV1 {
        manifest,
        end_reason: owned_field(lines, "end_reason"),
        exit_category,
        started_at_unix_micros: parse_typed_field(lines, "started_at_unix_micros", "u64")?,
        ended_at_unix_micros: parse_typed_field(lines, "ended_at_unix_micros", "u64")?,
        fixed_ticks: parse_typed_field(lines, "fixed_ticks", "u64")?,
        wall_duration_micros: parse_typed_field(lines, "wall_duration_micros", "u64")?,
        fixed_tick_p50_micros: parse_typed_field(lines, "fixed_tick_p50_micros", "u32")?,
        fixed_tick_p95_micros: parse_typed_field(lines, "fixed_tick_p95_micros", "u32")?,
        fixed_tick_max_micros: parse_typed_field(lines, "fixed_tick_max_micros", "u32")?,
        entity_high_water: parse_typed_field(lines, "entity_high_water", "u32")?,
        link_high_water: parse_typed_field(lines, "link_high_water", "u32")?,
        terminal_entities: parse_typed_field(lines, "terminal_entities", "u32")?,
        terminal_links: parse_typed_field(lines, "terminal_links", "u32")?,
        rtt_p50_micros: parse_typed_field(lines, "rtt_p50_micros", "u32")?,
        rtt_p95_micros: parse_typed_field(lines, "rtt_p95_micros", "u32")?,
        rtt_max_micros: parse_typed_field(lines, "rtt_max_micros", "u32")?,
        jitter_p50_micros: parse_typed_field(lines, "jitter_p50_micros", "u32")?,
        jitter_p95_micros: parse_typed_field(lines, "jitter_p95_micros", "u32")?,
        jitter_max_micros: parse_typed_field(lines, "jitter_max_micros", "u32")?,
        transport_bytes_sent: parse_typed_field(lines, "transport_bytes_sent", "u64")?,
        transport_bytes_received: parse_typed_field(lines, "transport_bytes_received", "u64")?,
        packets_sent: parse_typed_field(lines, "packets_sent", "u64")?,
        packets_received: parse_typed_field(lines, "packets_received", "u64")?,
        channel_messages_sent: parse_typed_field(lines, "channel_messages_sent", "u64")?,
        channel_messages_received: parse_typed_field(lines, "channel_messages_received", "u64")?,
        checkpoint_digest: parse_typed_field(lines, "checkpoint_digest", "u64")?,
        checkpoints_observed: parse_typed_field(lines, "checkpoints_observed", "u32")?,
        first_divergence: match parse_report_field(lines, "first_divergence")
            .expect("presence was checked")
        {
            "none" => None,
            value => Some(value.to_string()),
        },
        dropped_messages: parse_typed_field(lines, "dropped_messages", "u64")?,
        rejected_connections: parse_typed_field(lines, "rejected_connections", "u64")?,
        error_count: parse_typed_field(lines, "error_count", "u64")?,
        gameplay: GameplayAggregatesV1::from_report_lines(lines)?,
    };
    report
        .validate()
        .map_err(|error| format!("reconstructed report failed validation: {error}"))?;
    Ok(report)
}

/// Reconstruct the run manifest from already-presence-checked report lines: the bounded
/// participant rows plus every identity, fingerprint, and scenario-declaration field.
fn parse_run_manifest(lines: &[(&str, &str)], schema: u16) -> Result<RunManifestV1, String> {
    let declared_participants = parse_typed_field::<u32>(lines, "participants", "u32")? as usize;
    let mut participants = Vec::with_capacity(declared_participants);
    for index in 0..declared_participants {
        participants.push(ManifestParticipant {
            player_id: parse_typed_field(lines, &format!("participant_{index}_player_id"), "u64")?,
            build_identity: owned_field(lines, &format!("participant_{index}_build")),
        });
    }
    Ok(RunManifestV1 {
        schema_version: schema,
        scenario_id: owned_field(lines, "scenario_id"),
        scenario_revision: parse_typed_field(lines, "scenario_revision", "u32")?,
        run_id: owned_field(lines, "run_id"),
        build_version: owned_field(lines, "build_version"),
        source_revision: owned_field(lines, "source_revision"),
        source_dirty: parse_typed_field(lines, "source_dirty", "bool")?,
        protocol_version: parse_typed_field(lines, "protocol_version", "u16")?,
        registry_fingerprint: parse_typed_field(lines, "registry_fingerprint", "u64")?,
        content_fingerprint: parse_typed_field(lines, "content_fingerprint", "u64")?,
        mode: owned_field(lines, "mode"),
        rules_profile: owned_field(lines, "rules_profile"),
        network_profile: owned_field(lines, "network_profile"),
        render_profile: owned_field(lines, "render_profile"),
        seed: parse_typed_field(lines, "seed", "u64")?,
        participants,
        scripted_action_count: parse_typed_field(lines, "scripted_actions", "u32")?,
        checkpoint_count: parse_typed_field(lines, "checkpoints", "u32")?,
    })
}

/// Validate the `participants=N` count against the contiguous, bounded participant rows.
fn validate_report_participant_block(lines: &[(&str, &str)]) -> Result<(), String> {
    let declared = parse_report_field(lines, "participants")
        .expect("presence was checked above")
        .parse::<u32>()
        .map_err(|_| "participants is not a u32".to_string())?;
    let count = usize::try_from(declared).unwrap_or(usize::MAX);
    if count > MAX_MANIFEST_PARTICIPANTS {
        return Err(format!(
            "report declares {declared} participants above the {MAX_MANIFEST_PARTICIPANTS} cap"
        ));
    }
    for (key, _) in lines {
        if let Some(index) = key
            .strip_prefix("participant_")
            .and_then(|rest| rest.split_once('_'))
            .and_then(|(index, _)| index.parse::<usize>().ok())
            && index >= count
        {
            return Err(format!(
                "report carries participant row {index} beyond the declared {declared}"
            ));
        }
    }
    for index in 0..count {
        for suffix in ["player_id", "build"] {
            let key = format!("participant_{index}_{suffix}");
            let value = parse_report_field(lines, &key)
                .ok_or_else(|| format!("missing or duplicated report field: {key}"))?;
            if value.is_empty() || value.len() > MAX_IDENTITY_BYTES || value.contains('=') {
                return Err(format!(
                    "report field {key} is empty, oversized, or carries an '=' separator"
                ));
            }
        }
    }
    Ok(())
}

/// The manifest identity fields one launcher run shares across every endpoint. Source
/// identity, the canonical participant/build assignment, and the declared scenario
/// contract (scripted actions and expected checkpoints) are part of the shared identity,
/// so a report from another source tree, a different roster, or a diverging scenario
/// declaration cannot satisfy this run's gate when version and fingerprints happen to
/// match. Network and render profiles stay per-endpoint (a run may mix native and
/// headless clients), so they are deliberately excluded from the agreement check.
const RUN_IDENTITY_FIELDS: [&str; 15] = [
    "scenario_id",
    "scenario_revision",
    "run_id",
    "build_version",
    "source_revision",
    "source_dirty",
    "protocol_version",
    "registry_fingerprint",
    "content_fingerprint",
    "mode",
    "rules_profile",
    "seed",
    "participants",
    "scripted_actions",
    "checkpoints",
];

/// Render one manifest's shared run identity as `(field, value)` pairs for comparison.
fn run_identity_pairs(manifest: &RunManifestV1) -> Vec<(&'static str, String)> {
    RUN_IDENTITY_FIELDS
        .iter()
        .map(|field| {
            let value = match *field {
                "scenario_id" => manifest.scenario_id.clone(),
                "scenario_revision" => manifest.scenario_revision.to_string(),
                "run_id" => manifest.run_id.clone(),
                "build_version" => manifest.build_version.clone(),
                "source_revision" => manifest.source_revision.clone(),
                "source_dirty" => manifest.source_dirty.to_string(),
                "protocol_version" => manifest.protocol_version.to_string(),
                "registry_fingerprint" => manifest.registry_fingerprint.to_string(),
                "content_fingerprint" => manifest.content_fingerprint.to_string(),
                "mode" => manifest.mode.clone(),
                "rules_profile" => manifest.rules_profile.clone(),
                "seed" => manifest.seed.to_string(),
                "participants" => manifest
                    .participants
                    .iter()
                    .map(|participant| {
                        format!("{}:{}", participant.player_id, participant.build_identity)
                    })
                    .collect::<Vec<_>>()
                    .join(";"),
                "scripted_actions" => manifest.scripted_action_count.to_string(),
                "checkpoints" => manifest.checkpoint_count.to_string(),
                _ => unreachable!("the field list is closed"),
            };
            (*field, value)
        })
        .collect()
}

/// Enforce the per-report terminal gate: a clean exit, zero dropped messages,
/// rejections, and errors, no divergence, and an actually-observed participant roster.
fn enforce_closeout_terminal_gate(
    report: &CloseoutReportV1,
    path: &std::path::Path,
) -> Result<(), String> {
    if report.exit_category != ProcessExitCategory::CleanExit {
        return Err(format!(
            "{}: unexpected exit category {}",
            path.display(),
            report.exit_category.name()
        ));
    }
    for (counter, value) in [
        ("dropped_messages", report.dropped_messages),
        ("rejected_connections", report.rejected_connections),
        ("error_count", report.error_count),
    ] {
        if value != 0 {
            return Err(format!("{}: {counter}={value}", path.display()));
        }
    }
    if let Some(divergence) = &report.first_divergence {
        return Err(format!("{}: divergence {divergence}", path.display()));
    }
    if report.manifest.participants.is_empty() {
        return Err(format!(
            "{}: manifest carries no participant rows; supervised roster runs spawn \
             fighters before the scenario completes",
            path.display()
        ));
    }
    Ok(())
}

/// Validate a finished closeout-report directory for one launcher run: exactly one report
/// per configured endpoint (`server.closeout` plus `client-1..N.closeout`), every report
/// reconstructing and validating as the current schema, one shared run identity (source
/// revision, seed, the canonical participant/build assignment, and the declared scenario
/// contract included) across endpoints with at least one observed participant row per
/// endpoint, every endpoint exiting clean with zero dropped messages, rejections, errors,
/// and divergence, and one checkpoint digest across endpoints that carries real evidence
/// exactly when `expect_checkpoint_evidence` says the run profile records checkpoints
/// (combat-assert runs do; movement, map, and match profiles do not). When
/// `declared_checkpoint_requirement` carries the asserted preset's required checkpoint
/// count, every report's declared `checkpoint_count` must equal it and its observed
/// checkpoints must cover it, so a launcher declaration that drifted from the preset
/// fails the gate instead of being overwritten by observation. Verification launchers
/// reach this through `brawler-server validate-closeout` so the terminal gate and the
/// report writer share a single schema definition instead of a launcher-side subset.
/// Returns the number of validated reports.
pub fn validate_closeout_directory(
    directory: &std::path::Path,
    client_count: u32,
    expect_checkpoint_evidence: bool,
    declared_checkpoint_requirement: Option<u32>,
) -> Result<usize, String> {
    if client_count == 0 || client_count > 8 {
        return Err(format!(
            "client count {client_count} is outside the supervised 1-8 roster"
        ));
    }
    let mut expected: Vec<String> = (1..=client_count)
        .map(|index| format!("client-{index}.closeout"))
        .collect();
    expected.push("server.closeout".to_string());
    expected.sort();
    let entries = std::fs::read_dir(directory)
        .map_err(|error| format!("{}: {error}", directory.display()))?;
    let mut present: Vec<String> = entries
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.file_name())
        .filter_map(|name| name.into_string().ok())
        .filter(|name| name.ends_with(".closeout"))
        .collect();
    present.sort();
    if present != expected {
        return Err(format!(
            "expected exactly one closeout report per configured endpoint ({}); found: {}",
            expected.join(", "),
            if present.is_empty() {
                "none".to_string()
            } else {
                present.join(", ")
            }
        ));
    }
    let mut digests: Vec<u64> = Vec::new();
    let mut reference_identity: Option<(String, Vec<(&'static str, String)>)> = None;
    for name in &expected {
        let path = directory.join(name);
        let contents = std::fs::read_to_string(&path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let lines = split_report_lines(&contents)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let report = parse_closeout_report(&lines)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        enforce_closeout_terminal_gate(&report, &path)?;
        let identity = run_identity_pairs(&report.manifest);
        if let Some((reference_name, reference_identity)) = &reference_identity {
            let mismatch = reference_identity
                .iter()
                .zip(&identity)
                .find(|((_, expected), (_, actual))| expected != actual);
            if let Some(((field, expected), (_, actual))) = mismatch {
                return Err(format!(
                    "run identity {field} diverged across endpoints: \
                     {reference_name}={expected}, {name}={actual}"
                ));
            }
        }
        reference_identity = Some((name.clone(), identity));
        if expect_checkpoint_evidence && report.checkpoint_digest == 0 {
            return Err(format!(
                "{}: checkpoint digest is zero; this run profile must carry checkpoint evidence",
                path.display()
            ));
        }
        if !expect_checkpoint_evidence && report.checkpoint_digest != 0 {
            return Err(format!(
                "{}: checkpoint digest is nonzero; this run profile records no checkpoint evidence",
                path.display()
            ));
        }
        if let Some(required) = declared_checkpoint_requirement {
            enforce_declared_checkpoint_requirement(&report, &path, required)?;
        }
        digests.push(report.checkpoint_digest);
    }
    if digests.iter().any(|digest| digest != &digests[0]) {
        return Err(format!(
            "checkpoint digests diverged across endpoints: {}",
            digests
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    Ok(expected.len())
}

/// Enforce one report's declared scenario contract against the asserted preset's
/// requirement: the declaration must equal the requirement, and observed evidence must
/// cover it. Observed may exceed the requirement when the roster fights mixed presets;
/// it must never fall short of the scenario's declared contract.
fn enforce_declared_checkpoint_requirement(
    report: &CloseoutReportV1,
    path: &std::path::Path,
    required: u32,
) -> Result<(), String> {
    if report.manifest.checkpoint_count != required {
        return Err(format!(
            "{}: declared checkpoints {} diverge from the asserted preset's required {}",
            path.display(),
            report.manifest.checkpoint_count,
            required
        ));
    }
    if report.checkpoints_observed < required {
        return Err(format!(
            "{}: observed {} of the {} checkpoints the scenario declares",
            path.display(),
            report.checkpoints_observed,
            required
        ));
    }
    Ok(())
}

/// FNV-1a digest over ordered checkpoint values; stable across runs and platforms.
#[must_use]
pub fn stable_digest(values: &[&str]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for value in values {
        for byte in value.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x1000_0000_01b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

/// Bounded ring of microsecond samples used for fixed-tick and RTT percentiles.
#[derive(Debug)]
pub(crate) struct SampleRing {
    samples: Vec<u32>,
    cursor: usize,
    filled: bool,
}

impl SampleRing {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            samples: vec![0; capacity],
            cursor: 0,
            filled: false,
        }
    }

    pub(crate) fn push(&mut self, value: u32) {
        self.samples[self.cursor] = value;
        self.cursor = (self.cursor + 1) % self.samples.len();
        if self.cursor == 0 {
            self.filled = true;
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        if self.filled {
            self.samples.len()
        } else {
            self.cursor
        }
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Copy live samples in insertion order; allocation is bounded by ring capacity.
    pub(crate) fn ordered(&self) -> Vec<u32> {
        if self.filled {
            let mut tail = self.samples[self.cursor..].to_vec();
            tail.extend_from_slice(&self.samples[..self.cursor]);
            tail
        } else {
            self.samples[..self.cursor].to_vec()
        }
    }
}

/// Read one bounded non-empty environment value for manifest construction.
#[must_use]
pub(crate) fn env_identity(key: &str, fallback: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty() && value.len() <= MAX_IDENTITY_BYTES)
        .unwrap_or_else(|| fallback.to_string())
}

/// Current wall-clock unix timestamp in microseconds, saturating on clock failure.
#[must_use]
pub(crate) fn unix_micros_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
        })
}

#[cfg(test)]
mod tests;
