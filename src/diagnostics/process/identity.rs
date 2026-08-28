//! Environment-derived diagnostics run identity.

use super::RunManifestV1;
use crate::diagnostics::env_identity;

impl RunManifestV1 {
    /// Derive the manifest identity from bounded environment controls.
    ///
    /// These are development verification controls, not a worker manifest or IPC contract.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            schema_version: crate::diagnostics::CLOSEOUT_SCHEMA_VERSION,
            scenario_id: env_identity("BRAWLER_DIAGNOSTICS_SCENARIO_ID", "ad-hoc"),
            scenario_revision: std::env::var("BRAWLER_DIAGNOSTICS_SCENARIO_REVISION")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(1),
            run_id: env_identity("BRAWLER_NETWORK_RUN_ID", "unknown"),
            build_version: crate::VERSION.to_string(),
            source_revision: env_identity("BRAWLER_SOURCE_REVISION", "unknown"),
            source_dirty: std::env::var("BRAWLER_SOURCE_DIRTY").as_deref() == Ok("1"),
            protocol_version: crate::protocol::SUPPORTED_PROTOCOL_VERSION,
            registry_fingerprint: 0,
            content_fingerprint: 0,
            mode: env_identity("BRAWLER_DIAGNOSTICS_MODE", "wipeout"),
            rules_profile: env_identity("BRAWLER_DIAGNOSTICS_RULES_PROFILE", "production"),
            network_profile: env_identity("BRAWLER_NETWORK_PROFILE", "local"),
            render_profile: env_identity("BRAWLER_RENDER_PROFILE", "native"),
            seed: std::env::var("BRAWLER_DIAGNOSTICS_SEED")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0),
            participants: Vec::new(),
            scripted_action_count: std::env::var("BRAWLER_DIAGNOSTICS_SCRIPTED_ACTIONS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0),
            checkpoint_count: std::env::var("BRAWLER_DIAGNOSTICS_CHECKPOINTS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0),
        }
    }
}
