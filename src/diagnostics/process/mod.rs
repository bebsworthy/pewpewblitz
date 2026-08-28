//! Process diagnostics composition, shared state, and explicit schedule ownership.

pub(super) mod closeout;
pub(super) mod common_window;
mod identity;
pub(super) mod sampling;

use super::{
    GameplayAggregatesV1, ManifestParticipant, ProcessExitCategory, RunManifestV1, SampleRing,
};
use bevy::app::AppExit;
use bevy::prelude::*;
use std::{
    path::PathBuf,
    time::{Instant, SystemTime},
};

pub(super) use closeout::participant_build_identity;
pub use common_window::percentile_micros;

/// How many recent fixed-tick/RTT samples feed report percentiles.
const SAMPLE_RING_CAPACITY: usize = 4096;

/// Diagnostics configuration. The plugin stays inert without a report or window path.
#[derive(Resource, Clone, Debug)]
pub struct ProcessDiagnosticsSettings {
    pub report_path: Option<PathBuf>,
    /// Optional bounded marker for a comparable authoritative match observation window.
    /// This is separate from the closeout schema because it is an opt-in measurement seam,
    /// not a gameplay or process-report field.
    pub window_path: Option<PathBuf>,
    pub manifest: RunManifestV1,
    pub started_at: SystemTime,
    pub started_instant: Instant,
    pub end_reason: String,
}

impl Default for ProcessDiagnosticsSettings {
    fn default() -> Self {
        Self {
            report_path: std::env::var_os("BRAWLER_DIAGNOSTICS_CLOSEOUT_FILE").map(PathBuf::from),
            window_path: std::env::var_os("BRAWLER_DIAGNOSTICS_WINDOW_FILE").map(PathBuf::from),
            manifest: RunManifestV1::from_env(),
            started_at: SystemTime::now(),
            started_instant: Instant::now(),
            end_reason: "app-exit".to_string(),
        }
    }
}

impl ProcessDiagnosticsSettings {
    /// Enable report writing to `path`, keeping the environment-derived manifest identity.
    #[must_use]
    pub fn with_report_path(mut self, path: PathBuf) -> Self {
        self.report_path = Some(path);
        self
    }

    /// Local failure-record path selected by the `BRAWLER_FAILURE_REPORT` control, if any.
    #[must_use]
    pub fn failure_record_path(&self) -> Option<PathBuf> {
        std::env::var_os("BRAWLER_FAILURE_REPORT").map(PathBuf::from)
    }
}

/// Cumulative Lightyear transport observations (zero without the `process-metrics` feature).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TransportCounters {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub channel_messages_sent: u64,
    pub channel_messages_received: u64,
}

/// One bounded, authoritative match interval. The marker is emitted only after the first
/// completed match, so a launcher cannot accidentally compare a direct run's second match with
/// a routed run's first match. Counters use the same `Last` app-frame observation boundary on
/// both topologies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct CommonWindowObservation {
    pub(super) start_tick: Option<u64>,
    pub(super) end_tick: Option<u64>,
    pub(super) start_transport: TransportCounters,
    pub(super) end_transport: TransportCounters,
    pub(super) written: bool,
}

/// Observational state; nothing here is gameplay state.
#[derive(Resource, Debug)]
pub(crate) struct ProcessDiagnosticsState {
    pub(crate) enabled: bool,
    pub(super) tick_started_at: Option<Instant>,
    pub(super) fixed_tick_samples: SampleRing,
    pub(crate) fixed_ticks: u64,
    pub(super) rtt_samples: SampleRing,
    pub(super) jitter_samples: SampleRing,
    pub(super) entity_high_water: u32,
    pub(super) link_high_water: u32,
    pub(super) terminal_entities: Option<u32>,
    pub(super) terminal_links: Option<u32>,
    pub(crate) transport: TransportCounters,
    pub(super) common_window: CommonWindowObservation,
    pub(super) rejected_connections: u64,
    pub(super) error_count: u64,
    /// Manifest participant rows cached while fighters were live. Finalization runs after
    /// the role shutdown chain may have despawned replicated fighters, so the roster is
    /// observed during the run and the terminal report reads this cache, not the world.
    pub(crate) manifest_participants: Vec<ManifestParticipant>,
    /// Gameplay aggregates consolidated by `observe_gameplay_aggregates` at terminal
    /// observation; the finalizer only copies them into the report.
    pub(crate) gameplay: GameplayAggregatesV1,
    pub(super) report_written: bool,
}

impl Default for ProcessDiagnosticsState {
    fn default() -> Self {
        Self {
            enabled: false,
            tick_started_at: None,
            fixed_tick_samples: SampleRing::with_capacity(SAMPLE_RING_CAPACITY),
            fixed_ticks: 0,
            rtt_samples: SampleRing::with_capacity(SAMPLE_RING_CAPACITY),
            jitter_samples: SampleRing::with_capacity(SAMPLE_RING_CAPACITY),
            entity_high_water: 0,
            link_high_water: 0,
            terminal_entities: None,
            terminal_links: None,
            transport: TransportCounters::default(),
            common_window: CommonWindowObservation::default(),
            rejected_connections: 0,
            error_count: 0,
            manifest_participants: Vec::new(),
            gameplay: GameplayAggregatesV1::default(),
            report_written: false,
        }
    }
}

impl ProcessDiagnosticsState {
    /// Count one refused session so the closeout report can prove zero rejections instead
    /// of merely carrying the field. Called from the server's rejection paths only.
    #[cfg(feature = "server")]
    pub(crate) fn record_rejected_connection(&mut self) {
        self.rejected_connections = self.rejected_connections.saturating_add(1);
    }
}

/// Structured exit classification recorded by whichever system requests an error exit.
/// `AppExit` cannot carry a category, so closeout finalization reads this resource to
/// report the true failure class instead of the undifferentiated error mapping.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProcessExitClassification {
    category: Option<ProcessExitCategory>,
}

impl ProcessExitClassification {
    /// Record the category of an error exit being requested now. The first recorded
    /// category wins, so a shutdown storm cannot overwrite the root cause.
    pub(crate) fn record_error_exit(&mut self, category: ProcessExitCategory) {
        if self.category.is_none() {
            self.category = Some(category);
        }
    }

    /// Resolve the closeout category for `exit`: a classified error exit reports its
    /// recorded category, an unclassified error exit stays `ShutdownIncomplete`, and a
    /// success exit stays `CleanExit`.
    #[must_use]
    pub fn classified_category(&self, exit: &AppExit) -> ProcessExitCategory {
        if exit.is_error() {
            self.category
                .unwrap_or(ProcessExitCategory::ShutdownIncomplete)
        } else {
            ProcessExitCategory::CleanExit
        }
    }
}

/// Ordering anchor for terminal observations: final process counts, link statistics, and
/// the Lightyear transport sample. Role shutdown chains order before this set, so its
/// systems observe post-shutdown entity/link counts and the re-emitted terminal exit.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalObservationSet;

/// Ordering anchor for report finalization. Terminal observations in
/// [`TerminalObservationSet`] must complete first so the exit-frame report carries the
/// final error count, transport sample, and post-shutdown entity/link counts.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DiagnosticsSet;

/// Installs observational process diagnostics in either role application.
///
/// The plugin never mutates gameplay components; it only reads clocks, entity/link counts,
/// Lightyear counters, and the terminal `AppExit`.
pub struct ProcessDiagnosticsPlugin;

impl Plugin for ProcessDiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        let settings = app
            .world_mut()
            .get_resource_or_insert_with(ProcessDiagnosticsSettings::default)
            .clone();
        let state = ProcessDiagnosticsState {
            enabled: settings.report_path.is_some() || settings.window_path.is_some(),
            ..ProcessDiagnosticsState::default()
        };
        let enabled = state.enabled;
        #[cfg(feature = "process-metrics")]
        let metrics_enabled = enabled;
        app.insert_resource(state);
        app.init_resource::<ProcessExitClassification>();
        // The observation sets stay configured in every build because the role shutdown
        // chains order against them; without a report path the set holds no systems and
        // the process keeps its zero-cost inert behavior.
        app.configure_sets(Last, TerminalObservationSet.before(DiagnosticsSet));
        if enabled {
            app.add_systems(FixedFirst, sampling::begin_fixed_tick_observation)
                .add_systems(FixedLast, sampling::finish_fixed_tick_observation)
                .add_systems(
                    Last,
                    (
                        sampling::observe_process_counts,
                        sampling::sample_link_stats,
                        sampling::observe_manifest_participants,
                        sampling::observe_gameplay_aggregates,
                    )
                        .in_set(TerminalObservationSet),
                );
            // The server owns the authoritative match lifecycle. Capture the interval
            // inside its fixed-post transaction, after the mode outcome has committed and
            // before `SimulationTick` advances, so both boundaries name the exact
            // authoritative ticks rather than whichever app frame happens to observe them.
            #[cfg(feature = "server")]
            app.add_systems(
                FixedPostUpdate,
                common_window::observe_common_window_fixed
                    .after(crate::matchplay::MatchSet::Outcomes)
                    .before(crate::gameplay::advance_simulation_tick),
            );
            // A client has no authoritative outcome transaction. Keep its opt-in marker
            // observational and app-frame based; paired M01 comparison uses the server
            // and routed match-worker markers above.
            #[cfg(not(feature = "server"))]
            app.add_systems(
                Last,
                common_window::observe_common_window_client
                    .in_set(TerminalObservationSet)
                    .before(common_window::finalize_common_window),
            );
            app.add_systems(
                Last,
                common_window::finalize_common_window.in_set(TerminalObservationSet),
            );
            app.add_systems(
                Last,
                closeout::finalize_closeout_report.in_set(DiagnosticsSet),
            );
        }

        #[cfg(feature = "process-metrics")]
        if metrics_enabled {
            // Lightyear's metrics registry is process-global; install it once and only in
            // dedicated measurement processes that opted in through the feature flag. The
            // sampler stays inside the terminal observation set so the closeout report sees
            // the final bucket values before Lightyear clears them.
            app.add_plugins(lightyear::metrics::prelude::MetricsPlugin::default());
            app.add_systems(
                Last,
                sampling::sample_lightyear_metrics
                    .in_set(TerminalObservationSet)
                    .before(common_window::finalize_common_window)
                    .before(lightyear::metrics::prelude::ClearBucketsSystem),
            );
        }
    }
}
