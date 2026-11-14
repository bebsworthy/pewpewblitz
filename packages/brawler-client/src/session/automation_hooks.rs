//! The product session systems' only view of automation. With the `automation` feature it reads
//! `ClientAutomationConfig` and the runtime `HeadlessAutomation`; in the player build every
//! question has a constant answer and every report is a no-op, so no automation field, exit rule,
//! or timing marker is compiled into the player client (BRL-3p70).

use bevy::ecs::system::SystemParam;
#[cfg(feature = "automation")]
use bevy::prelude::Res;

#[derive(SystemParam)]
pub(super) struct AutomationHooks<'w> {
    #[cfg(feature = "automation")]
    config: Option<Res<'w, crate::ClientAutomationConfig>>,
    #[cfg(feature = "automation")]
    runtime: Option<Res<'w, brawler::config::HeadlessAutomation>>,
    #[cfg(not(feature = "automation"))]
    player: core::marker::PhantomData<&'w ()>,
}

#[cfg(feature = "automation")]
impl AutomationHooks<'_> {
    fn config(&self) -> Option<&crate::ClientAutomationConfig> {
        self.config.as_deref()
    }

    /// Automation readies and restarts direct-UDP matches without player input.
    pub(super) fn automatic_match_commands(&self, roster_count: usize) -> bool {
        self.config().is_some_and(|config| {
            crate::automation::automatic_match_command_enabled(config, roster_count)
        })
    }

    /// Scripted runs re-arm the Waiting ready command when a countdown departs.
    pub(super) fn rearms_match_commands(&self) -> bool {
        self.config()
            .is_some_and(|config| config.headless_simulation_ticks.is_some())
    }

    pub(super) fn exits_after_lobby_welcome(&self) -> bool {
        self.config()
            .is_some_and(|config| config.exit_after_lobby_welcome)
    }

    pub(super) fn exits_after_lobby_return(&self) -> bool {
        self.config()
            .is_some_and(|config| config.exit_after_lobby_return)
    }

    pub(super) fn roster_exit_target(&self) -> Option<usize> {
        self.config().and_then(|config| config.exit_after_roster)
    }

    /// Whether the scripted simulation budget (if any) has elapsed.
    pub(super) fn run_complete(&self) -> bool {
        self.runtime.as_deref().is_none_or(|runtime| {
            runtime
                .simulation_ticks
                .is_none_or(|limit| runtime.elapsed_ticks >= limit)
        })
    }

    fn reports_render_timing(&self) -> bool {
        self.config()
            .is_some_and(|config| config.render_measurement.is_some())
    }

    /// Render-evidence timing marker for the first Ready check-in of one loading correlation.
    pub(super) fn report_match_ready(&self, client_id: u64, request_id: u64) {
        if self.reports_render_timing() {
            eprintln!(
                "brawler-client timing match-ready client_id={client_id} request_id={request_id} ts_ms={}",
                brawler::diagnostics::unix_micros_now() / 1_000
            );
        }
    }

    /// Render-evidence timing marker for an accepted match join.
    pub(super) fn report_match_accepted(&self, client_id: u64, player_id: u64) {
        if self.reports_render_timing() {
            eprintln!(
                "brawler-client timing match-accepted client_id={client_id} player_id={player_id} ts_ms={}",
                brawler::diagnostics::unix_micros_now() / 1_000
            );
        }
    }

    /// Routed-evidence marker (`scripts/network-routed-evidence.py`) at the Lightyear `Connected`
    /// boundary of a fresh match session. It carries only stable correlation IDs; capabilities,
    /// player identities, and manifests are never logged.
    #[allow(
        clippy::unused_self,
        reason = "the player build's no-op twin keeps one call shape for both builds"
    )]
    pub(super) fn report_handoff_connected(&self, client_id: u64, request_id: u64) {
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis());
        let marker = format!(
            "brawler-client timing handoff-connected client_id={client_id} request_id={request_id} ts_ms={timestamp_ms}\n"
        );
        // Both verification clients inherit one stderr file descriptor. Format the whole bounded
        // marker first and issue one write so process output cannot splice stable IDs into an
        // unparsable half-line.
        let _ = std::io::Write::write_all(&mut std::io::stderr().lock(), marker.as_bytes());
    }
}

#[cfg(not(feature = "automation"))]
#[allow(
    clippy::unused_self,
    reason = "the player build answers every automation question with a constant"
)]
impl AutomationHooks<'_> {
    pub(super) const fn automatic_match_commands(&self, _roster_count: usize) -> bool {
        false
    }

    pub(super) const fn rearms_match_commands(&self) -> bool {
        false
    }

    pub(super) const fn exits_after_lobby_welcome(&self) -> bool {
        false
    }

    pub(super) const fn exits_after_lobby_return(&self) -> bool {
        false
    }

    pub(super) const fn roster_exit_target(&self) -> Option<usize> {
        None
    }

    pub(super) const fn run_complete(&self) -> bool {
        true
    }

    pub(super) const fn report_match_ready(&self, _client_id: u64, _request_id: u64) {}

    pub(super) const fn report_match_accepted(&self, _client_id: u64, _player_id: u64) {}

    pub(super) const fn report_handoff_connected(&self, _client_id: u64, _request_id: u64) {}
}
