//! Process-level observation: fixed-tick timing, entity/link high-water marks, optional
//! Lightyear transport counters, and closeout report finalization.

use super::{
    CloseoutReportV1, ProcessExitCategory, RunManifestV1, SampleRing, env_identity, unix_micros_now,
};
use bevy::app::AppExit;
use bevy::prelude::*;
use lightyear::prelude::Link;
use std::{
    path::PathBuf,
    time::{Duration, Instant, SystemTime},
};

/// How many recent fixed-tick/RTT samples feed report percentiles.
const SAMPLE_RING_CAPACITY: usize = 4096;

/// Diagnostics configuration. The plugin stays inert without a report path.
#[derive(Resource, Clone, Debug)]
pub struct ProcessDiagnosticsSettings {
    pub report_path: Option<PathBuf>,
    pub manifest: RunManifestV1,
    pub started_at: SystemTime,
    pub started_instant: Instant,
    pub end_reason: String,
    pub rejected_connections: u64,
}

impl Default for ProcessDiagnosticsSettings {
    fn default() -> Self {
        Self {
            report_path: std::env::var_os("BRAWLER_DIAGNOSTICS_CLOSEOUT_FILE").map(PathBuf::from),
            manifest: RunManifestV1::from_env(),
            started_at: SystemTime::now(),
            started_instant: Instant::now(),
            end_reason: "app-exit".to_string(),
            rejected_connections: 0,
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

/// Observational state; nothing here is gameplay state.
#[derive(Resource, Debug)]
pub(crate) struct ProcessDiagnosticsState {
    pub(crate) enabled: bool,
    tick_started_at: Option<Instant>,
    fixed_tick_samples: SampleRing,
    fixed_ticks: u64,
    rtt_samples: SampleRing,
    jitter_samples: SampleRing,
    entity_high_water: u32,
    link_high_water: u32,
    terminal_entities: Option<u32>,
    terminal_links: Option<u32>,
    pub(crate) transport: TransportCounters,
    dropped_messages: u64,
    error_count: u64,
    report_written: bool,
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
            dropped_messages: 0,
            error_count: 0,
            report_written: false,
        }
    }
}

/// Ordering anchor for report finalization. The dedicated server orders its shutdown chain
/// before this set so the report observes the final terminal exit.
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
            enabled: settings.report_path.is_some(),
            ..ProcessDiagnosticsState::default()
        };
        #[cfg(feature = "process-metrics")]
        let metrics_enabled = state.enabled;
        app.insert_resource(state);
        app.add_systems(FixedFirst, begin_fixed_tick_observation)
            .add_systems(FixedLast, finish_fixed_tick_observation)
            .add_systems(Last, (observe_process_counts, sample_link_stats))
            .add_systems(Last, finalize_closeout_report.in_set(DiagnosticsSet));

        #[cfg(feature = "process-metrics")]
        if metrics_enabled {
            // Lightyear's metrics registry is process-global; install it once and only in
            // dedicated measurement processes that opted in through the feature flag.
            app.add_plugins(lightyear::metrics::prelude::MetricsPlugin::default());
            app.add_systems(
                Last,
                sample_lightyear_metrics.before(lightyear::metrics::prelude::ClearBucketsSystem),
            );
        }
    }
}

fn begin_fixed_tick_observation(mut state: ResMut<ProcessDiagnosticsState>) {
    state.tick_started_at = Some(Instant::now());
}

fn finish_fixed_tick_observation(mut state: ResMut<ProcessDiagnosticsState>) {
    if let Some(started) = state.tick_started_at.take() {
        let elapsed = started.elapsed();
        let micros = duration_to_micros(elapsed);
        state.fixed_tick_samples.push(micros);
        state.fixed_ticks = state.fixed_ticks.saturating_add(1);
    }
}

fn observe_process_counts(
    entities: &bevy::ecs::entity::Entities,
    links: Query<(), With<Link>>,
    mut state: ResMut<ProcessDiagnosticsState>,
) {
    let entity_count = entities.len();
    let link_count = u32::try_from(links.iter().count()).unwrap_or(u32::MAX);
    state.entity_high_water = state.entity_high_water.max(entity_count);
    state.link_high_water = state.link_high_water.max(link_count);
    state.terminal_entities = Some(entity_count);
    state.terminal_links = Some(link_count);
}

fn sample_link_stats(links: Query<&Link>, mut state: ResMut<ProcessDiagnosticsState>) {
    let Some(worst) = links.iter().max_by_key(|link| link.stats.rtt) else {
        return;
    };
    state.rtt_samples.push(duration_to_micros(worst.stats.rtt));
    state
        .jitter_samples
        .push(duration_to_micros(worst.stats.jitter));
}

#[cfg(feature = "process-metrics")]
fn sample_lightyear_metrics(
    registry: Option<Res<lightyear::metrics::prelude::MetricsRegistry>>,
    mut state: ResMut<ProcessDiagnosticsState>,
) {
    use lightyear::metrics::metrics::Key;
    let Some(registry) = registry else {
        return;
    };
    fn counter(
        registry: &lightyear::metrics::prelude::MetricsRegistry,
        name: &'static str,
    ) -> Option<u64> {
        registry
            .get_counter_value(&Key::from_static_name(name))
            .map(|value| value as u64)
    }
    state.transport.bytes_sent =
        counter(&registry, "transport/send_bytes").unwrap_or(state.transport.bytes_sent);
    state.transport.bytes_received =
        counter(&registry, "transport/recv_bytes").unwrap_or(state.transport.bytes_received);
    state.transport.packets_sent =
        counter(&registry, "packets/send").unwrap_or(state.transport.packets_sent);
    state.transport.packets_received =
        counter(&registry, "packets/received").unwrap_or(state.transport.packets_received);
    state.transport.channel_messages_sent = counter(&registry, "channel/send_messages")
        .unwrap_or(state.transport.channel_messages_sent);
    state.transport.channel_messages_received = counter(&registry, "channel/recv_messages")
        .unwrap_or(state.transport.channel_messages_received);
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

fn duration_to_micros(duration: Duration) -> u32 {
    u32::try_from(duration.as_micros()).unwrap_or(u32::MAX)
}

// Bevy system parameters are owned by the scheduling runtime; `Res` cannot be borrowed here.
// The fighter read observes only stable wire identities for the manifest participant rows.
#[allow(clippy::needless_pass_by_value, clippy::type_complexity)]
fn finalize_closeout_report(
    mut exits: MessageReader<AppExit>,
    settings: Res<ProcessDiagnosticsSettings>,
    mut state: ResMut<ProcessDiagnosticsState>,
    protocol: Option<Res<crate::protocol::ProtocolFingerprint>>,
    content: Option<Res<crate::content::GameplayContentFingerprint>>,
    fighters: Option<
        Query<
            (&crate::protocol::PlayerId, &crate::builds::SelectedBuild),
            With<crate::protocol::Fighter>,
        >,
    >,
) {
    if !state.enabled || state.report_written {
        return;
    }
    let Some(exit) = exits.read().next() else {
        return;
    };
    state.report_written = true;
    let exit_category = ProcessExitCategory::from_app_exit(exit);
    let fixed = state.fixed_tick_samples.ordered();
    let rtt = state.rtt_samples.ordered();
    let jitter = state.jitter_samples.ordered();
    let mut manifest = settings.manifest.clone();
    if let Some(protocol) = protocol {
        manifest.registry_fingerprint = protocol.0;
    }
    if let Some(content) = content {
        manifest.content_fingerprint = content.0;
    }
    if let Some(fighters) = fighters
        && manifest.participants.is_empty()
    {
        let mut participants: Vec<_> = fighters
            .iter()
            .map(|(player, build)| super::ManifestParticipant {
                player_id: player.0,
                build_identity: format!(
                    "preset={:?} fingerprint={} revision={}",
                    build.source_build_preset_id.map_or(0, |id| id.0),
                    build.recipe_fingerprint.0,
                    build.revision.0
                ),
            })
            .collect();
        participants.sort_unstable_by_key(|participant| participant.player_id);
        participants.truncate(super::MAX_MANIFEST_PARTICIPANTS);
        manifest.participants = participants;
    }
    let report = CloseoutReportV1 {
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
        checkpoint_digest: 0,
        first_divergence: None,
        dropped_messages: state.dropped_messages,
        rejected_connections: settings.rejected_connections,
        error_count: state.error_count,
    };
    if let Err(error) = report.validate() {
        bevy::log::error!(?error, "closeout report failed validation; not written");
        return;
    }
    if let Some(path) = &settings.report_path {
        let contents = report.to_report_lines().join("\n") + "\n";
        if let Err(error) = std::fs::write(path, contents.as_bytes()) {
            bevy::log::error!(path = %path.display(), ?error, "closeout report write failed");
        } else {
            bevy::log::info!(path = %path.display(), "closeout report written");
        }
    }
}

fn system_time_micros(when: SystemTime) -> u64 {
    when.duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
        })
}

impl RunManifestV1 {
    /// Derive the manifest identity from bounded environment controls.
    ///
    /// These are development verification controls, not a worker manifest or IPC contract.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            schema_version: super::CLOSEOUT_SCHEMA_VERSION,
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
