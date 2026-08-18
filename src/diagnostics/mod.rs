//! Bounded process diagnostics: versioned run manifests, closeout reports, and failure records.
//!
//! Everything in this module is observational. Diagnostics may read ECS and network state and
//! write local reports or overlay UI, but never mutate gameplay, validation, authority,
//! replication targets, match results, or terrain.

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

/// The only closeout schema revision produced or accepted by this build.
pub const CLOSEOUT_SCHEMA_VERSION: u16 = 1;

/// Upper bound on manifest identity strings; rejects oversized or runaway scripted fields.
pub const MAX_IDENTITY_BYTES: usize = 96;

/// Upper bound on manifest participants; matches the engine's 24-fighter terrain capacity.
pub const MAX_MANIFEST_PARTICIPANTS: usize = 24;

/// Upper bound on closeout report lines before the report itself is rejected as malformed.
pub const MAX_REPORT_LINES: usize = 96;

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
    pub first_divergence: Option<String>,
    pub dropped_messages: u64,
    pub rejected_connections: u64,
    pub error_count: u64,
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
        if self.terminal_entities > self.entity_high_water
            || self.terminal_links > self.link_high_water
        {
            return Err("closeout terminal counts exceed recorded high-water marks".to_string());
        }
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
            format!(
                "first_divergence={}",
                self.first_divergence.as_deref().unwrap_or("none")
            ),
            format!("dropped_messages={}", self.dropped_messages),
            format!("rejected_connections={}", self.rejected_connections),
            format!("error_count={}", self.error_count),
        ]);
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

/// Every non-participant field a schema-1 closeout report carries exactly once. The reader
/// rejects reports that drop, duplicate, or oversize any of these fields, mirroring
/// `CloseoutReportV1::to_report_lines`.
const REPORT_REQUIRED_KEYS: [&str; 48] = [
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
    "first_divergence",
    "dropped_messages",
    "rejected_connections",
    "error_count",
];

/// Report fields whose values are bounded identity strings. The reader enforces the same
/// bound the writer's `validate` path enforces, so an oversized identity cannot slip into
/// a verification script through the file format.
const REPORT_IDENTITY_KEYS: [&str; 11] = [
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

/// One already-presence-checked field value, as an owned string.
fn owned_field(lines: &[(&str, &str)], key: &str) -> String {
    parse_report_field(lines, key)
        .unwrap_or_default()
        .to_string()
}

/// Parse and validate report lines into the schema-v1 closeout report they must encode.
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
    let declared_participants = parse_typed_field::<u32>(lines, "participants", "u32")? as usize;
    let mut participants = Vec::with_capacity(declared_participants);
    for index in 0..declared_participants {
        participants.push(ManifestParticipant {
            player_id: parse_typed_field(lines, &format!("participant_{index}_player_id"), "u64")?,
            build_identity: owned_field(lines, &format!("participant_{index}_build")),
        });
    }
    let manifest = RunManifestV1 {
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
    };
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
        first_divergence: match parse_report_field(lines, "first_divergence")
            .expect("presence was checked")
        {
            "none" => None,
            value => Some(value.to_string()),
        },
        dropped_messages: parse_typed_field(lines, "dropped_messages", "u64")?,
        rejected_connections: parse_typed_field(lines, "rejected_connections", "u64")?,
        error_count: parse_typed_field(lines, "error_count", "u64")?,
    };
    report
        .validate()
        .map_err(|error| format!("reconstructed report failed validation: {error}"))?;
    Ok(report)
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

/// The manifest identity fields one launcher run shares across every endpoint. Network
/// and render profiles stay per-endpoint (a run may mix native and headless clients),
/// so they are deliberately excluded from the agreement check.
const RUN_IDENTITY_FIELDS: [&str; 10] = [
    "scenario_id",
    "scenario_revision",
    "run_id",
    "build_version",
    "protocol_version",
    "registry_fingerprint",
    "content_fingerprint",
    "mode",
    "rules_profile",
    "seed",
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
                "protocol_version" => manifest.protocol_version.to_string(),
                "registry_fingerprint" => manifest.registry_fingerprint.to_string(),
                "content_fingerprint" => manifest.content_fingerprint.to_string(),
                "mode" => manifest.mode.clone(),
                "rules_profile" => manifest.rules_profile.clone(),
                "seed" => manifest.seed.to_string(),
                _ => unreachable!("the field list is closed"),
            };
            (*field, value)
        })
        .collect()
}

/// Validate a finished closeout-report directory for one launcher run: exactly one report
/// per configured endpoint (`server.closeout` plus `client-1..N.closeout`), every report
/// reconstructing and validating as schema v1, one shared run identity across endpoints,
/// every endpoint exiting clean with zero dropped messages, rejections, errors, and
/// divergence, and one checkpoint digest across endpoints that carries real evidence
/// exactly when `expect_checkpoint_evidence` says the run profile records checkpoints
/// (combat-assert runs do; movement, terrain, and match profiles do not). Verification
/// launchers reach this through `brawler-server validate-closeout` so the terminal gate
/// and the report writer share a single schema definition instead of a launcher-side
/// subset. Returns the number of validated reports.
pub fn validate_closeout_directory(
    directory: &std::path::Path,
    client_count: u32,
    expect_checkpoint_evidence: bool,
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
