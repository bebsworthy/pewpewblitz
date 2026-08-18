//! Bounded structured local failure records for process diagnosis.
//!
//! Failure records are development diagnostics only. They never replace the process exit
//! code, never leave the local filesystem, and never include secrets, network keys, full
//! paths other than an explicitly selected report path, or component dumps.

use super::{ProcessExitCategory, env_identity, unix_micros_now};
use serde::{Deserialize, Serialize};

/// The schema revision of the failure record contract.
pub const FAILURE_SCHEMA_VERSION: u16 = 1;

/// Bounded message length; longer diagnostics are truncated deterministically.
pub const MAX_FAILURE_MESSAGE_BYTES: usize = 512;

/// Stable runtime failure category. Configuration/argument errors remain process exit code 2
/// and carry the dedicated `Configuration` category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureCategory {
    Configuration,
    EndpointStart,
    ProtocolMismatch,
    ContentMismatch,
    VerificationFailed,
    Timeout,
    Panic,
    ShutdownIncomplete,
}

impl FailureCategory {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Configuration => "configuration",
            Self::EndpointStart => "endpoint_start",
            Self::ProtocolMismatch => "protocol_mismatch",
            Self::ContentMismatch => "content_mismatch",
            Self::VerificationFailed => "verification_failed",
            Self::Timeout => "timeout",
            Self::Panic => "panic",
            Self::ShutdownIncomplete => "shutdown_incomplete",
        }
    }
}

impl From<FailureCategory> for ProcessExitCategory {
    fn from(category: FailureCategory) -> Self {
        match category {
            FailureCategory::Configuration => ProcessExitCategory::Configuration,
            FailureCategory::EndpointStart => ProcessExitCategory::EndpointStart,
            FailureCategory::ProtocolMismatch => ProcessExitCategory::ProtocolMismatch,
            FailureCategory::ContentMismatch => ProcessExitCategory::ContentMismatch,
            FailureCategory::VerificationFailed => ProcessExitCategory::VerificationFailed,
            FailureCategory::Timeout => ProcessExitCategory::Timeout,
            FailureCategory::Panic => ProcessExitCategory::Panic,
            FailureCategory::ShutdownIncomplete => ProcessExitCategory::ShutdownIncomplete,
        }
    }
}

/// One bounded local failure observation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessFailureRecordV1 {
    pub schema_version: u16,
    pub category: FailureCategory,
    pub message: String,
    pub run_id: String,
    pub build_version: String,
    pub source_revision: String,
    pub protocol_version: u16,
    pub unix_micros: u64,
}

/// Percent-encode the characters that would corrupt single-line `key=value` report fields.
/// The encoding is deterministic and reversible, and never introduces a raw separator.
fn sanitize_message_value(message: &str) -> String {
    let mut sanitized = String::with_capacity(message.len());
    for character in message.chars() {
        match character {
            '%' => sanitized.push_str("%25"),
            '=' => sanitized.push_str("%3D"),
            '\n' => sanitized.push_str("%0A"),
            '\r' => sanitized.push_str("%0D"),
            control if u32::from(control) < 0x20 || control == '\u{7f}' => sanitized.push(' '),
            other => sanitized.push(other),
        }
    }
    sanitized
}

/// Truncate to the byte bound on a valid UTF-8 boundary, reserving room for the ellipsis so
/// the final message never exceeds `MAX_FAILURE_MESSAGE_BYTES`.
fn truncate_message_bytes(message: String) -> String {
    if message.len() <= MAX_FAILURE_MESSAGE_BYTES {
        return message;
    }
    let mut end = MAX_FAILURE_MESSAGE_BYTES - 3;
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &message[..end])
}

impl ProcessFailureRecordV1 {
    #[must_use]
    pub fn new(category: FailureCategory, message: impl Into<String>) -> Self {
        let message = truncate_message_bytes(sanitize_message_value(&message.into()));
        Self {
            schema_version: FAILURE_SCHEMA_VERSION,
            category,
            message,
            run_id: env_identity("BRAWLER_NETWORK_RUN_ID", "unknown"),
            build_version: crate::VERSION.to_string(),
            source_revision: env_identity("BRAWLER_SOURCE_REVISION", "unknown"),
            protocol_version: crate::protocol::SUPPORTED_PROTOCOL_VERSION,
            unix_micros: unix_micros_now(),
        }
    }

    /// Render the record as deterministic shell-readable `key=value` lines.
    #[must_use]
    pub fn to_report_lines(&self) -> Vec<String> {
        vec![
            format!("schema_version={}", self.schema_version),
            format!("category={}", self.category.name()),
            format!("message={}", self.message),
            format!("run_id={}", self.run_id),
            format!("build_version={}", self.build_version),
            format!("source_revision={}", self.source_revision),
            format!("protocol_version={}", self.protocol_version),
            format!("unix_micros={}", self.unix_micros),
        ]
    }
}

/// Append one failure record to `path` as `key=value` lines.
///
/// Appending (not truncating) preserves earlier records from repeated failures inside one
/// scripted scenario; callers keep the file bounded by using one file per run.
pub fn write_failure_record(path: &std::path::Path, record: &ProcessFailureRecordV1) {
    use std::io::Write;
    let mut contents = record.to_report_lines().join("\n") + "\n\n";
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return;
    };
    let _ = file.write_all(contents.as_bytes());
    contents.clear();
}

/// Install a minimal panic hook that appends a bounded local failure record before
/// delegating to the normal hook. A panic still terminates the process; this is a
/// development diagnostic, not panic recovery.
pub fn install_panic_failure_hook(report_path: std::path::PathBuf) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = format!("{info}");
        write_failure_record(
            &report_path,
            &ProcessFailureRecordV1::new(FailureCategory::Panic, message),
        );
        previous(info);
    }));
}
