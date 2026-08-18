//! Process-level observation: fixed-tick timing, entity/link high-water marks, optional
//! Lightyear transport counters, and closeout report finalization.

use super::{
    CloseoutReportV1, GameplayAggregatesV1, ManifestParticipant, ProcessExitCategory,
    RunManifestV1, SampleRing, env_identity, unix_micros_now,
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
}

impl Default for ProcessDiagnosticsSettings {
    fn default() -> Self {
        Self {
            report_path: std::env::var_os("BRAWLER_DIAGNOSTICS_CLOSEOUT_FILE").map(PathBuf::from),
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

/// Observational state; nothing here is gameplay state.
#[derive(Resource, Debug)]
pub(crate) struct ProcessDiagnosticsState {
    pub(crate) enabled: bool,
    tick_started_at: Option<Instant>,
    fixed_tick_samples: SampleRing,
    pub(crate) fixed_ticks: u64,
    rtt_samples: SampleRing,
    jitter_samples: SampleRing,
    entity_high_water: u32,
    link_high_water: u32,
    terminal_entities: Option<u32>,
    terminal_links: Option<u32>,
    pub(crate) transport: TransportCounters,
    rejected_connections: u64,
    error_count: u64,
    /// Manifest participant rows cached while fighters were live. Finalization runs after
    /// the role shutdown chain may have despawned replicated fighters, so the roster is
    /// observed during the run and the terminal report reads this cache, not the world.
    pub(crate) manifest_participants: Vec<ManifestParticipant>,
    /// Gameplay aggregates consolidated by `observe_gameplay_aggregates` at terminal
    /// observation; the finalizer only copies them into the report.
    pub(crate) gameplay: GameplayAggregatesV1,
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
            enabled: settings.report_path.is_some(),
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
            app.add_systems(FixedFirst, begin_fixed_tick_observation)
                .add_systems(FixedLast, finish_fixed_tick_observation)
                .add_systems(
                    Last,
                    (
                        observe_process_counts,
                        sample_link_stats,
                        observe_manifest_participants,
                        observe_gameplay_aggregates,
                    )
                        .in_set(TerminalObservationSet),
                )
                .add_systems(Last, finalize_closeout_report.in_set(DiagnosticsSet));
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
                sample_lightyear_metrics
                    .in_set(TerminalObservationSet)
                    .before(lightyear::metrics::prelude::ClearBucketsSystem),
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
    mut exits: MessageReader<AppExit>,
    entities: &bevy::ecs::entity::Entities,
    links: Query<(), With<Link>>,
    mut state: ResMut<ProcessDiagnosticsState>,
) {
    // Every observed error exit is one failed check or lifecycle path; the closeout report
    // uses this count to prove zero errors rather than merely carrying the field.
    for exit in exits.read() {
        if exit.is_error() {
            state.error_count = state.error_count.saturating_add(1);
        }
    }
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

/// Cache the manifest participant rows while fighters are live. The observation runs every
/// frame of a diagnostics-enabled process, so build replacements update their row in place
/// and the cache stays sorted by stable player identity; finalization then reads the cache
/// even after the role shutdown chain has despawned every replicated fighter.
pub(super) fn observe_manifest_participants(
    mut state: ResMut<ProcessDiagnosticsState>,
    fighters: Query<
        (&crate::protocol::PlayerId, &crate::builds::SelectedBuild),
        With<crate::protocol::Fighter>,
    >,
) {
    let mut participants = std::mem::take(&mut state.manifest_participants);
    for (player, build) in &fighters {
        let row = ManifestParticipant {
            player_id: player.0,
            build_identity: participant_build_identity(build),
        };
        match participants.binary_search_by_key(&player.0, |row| row.player_id) {
            Ok(index) => participants[index] = row,
            Err(index) if participants.len() < super::MAX_MANIFEST_PARTICIPANTS => {
                participants.insert(index, row);
            }
            _ => {}
        }
    }
    state.manifest_participants = participants;
}

/// Consolidate the bounded gameplay telemetry summaries into the terminal observation
/// state, so the consolidated report ties the run to its build, ability, match/mode, and
/// terrain observations instead of process/network measurements alone. The authoritative
/// match, build, and ability telemetry resources exist only in the server process; both
/// roles own terrain telemetry (the client records its convergence facts). Reading them
/// here — while the process still owns the resources — keeps report finalization free of
/// gameplay-query parameters.
#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn observe_gameplay_aggregates(
    mut state: ResMut<ProcessDiagnosticsState>,
    match_telemetry: Option<Res<crate::matchplay::MatchTelemetry>>,
    #[cfg(feature = "server")] build_telemetry: Option<Res<crate::builds::BuildTelemetry>>,
    terrain_telemetry: Option<Res<crate::terrain::telemetry::TerrainTelemetry>>,
) {
    let mut gameplay = GameplayAggregatesV1::default();
    #[cfg(feature = "server")]
    if let Some(builds) = build_telemetry.as_deref() {
        // Process-lifetime totals count evicted records too: once a bounded queue starts
        // dropping, retained length alone would freeze the reported total while its
        // dropped counter keeps climbing.
        gameplay.build_selections = u32::try_from(
            u64::try_from(builds.selections.len())
                .unwrap_or(u64::MAX)
                .saturating_add(builds.dropped_records),
        )
        .unwrap_or(u32::MAX);
        gameplay.build_dropped_records = builds.dropped_records;
    }
    if let Some(matches) = match_telemetry.as_deref() {
        gameplay.matches_completed = u32::try_from(
            u64::try_from(matches.summaries.len())
                .unwrap_or(u64::MAX)
                .saturating_add(matches.dropped_summaries),
        )
        .unwrap_or(u32::MAX);
        if let Some(summary) = matches.summaries.back() {
            consolidate_match_summary(summary, &mut gameplay);
        }
    }
    if let Some(terrain) = terrain_telemetry.as_deref() {
        let aggregates = &terrain.aggregates;
        gameplay.terrain_requested_brushes = aggregates.requested_brushes;
        gameplay.terrain_applied_brushes = aggregates.applied_brushes;
        gameplay.terrain_no_op_brushes = aggregates.no_op_brushes;
        gameplay.terrain_rejected_brushes = aggregates.rejected_brushes;
        gameplay.terrain_deferred_brushes = aggregates.deferred_brushes;
        gameplay.terrain_cells_erased = aggregates.cells_erased;
    }
    state.gameplay = gameplay;
}

/// Fold the latest completed match summary's mode, ability, and weapon aggregates into
/// the gameplay block. The mode identity and its typed summary ride along so closeouts
/// carry the objective evidence (Wipeout scores, Hot Zone terminal state) the match
/// telemetry already owns. Weapon aggregates are summed across every preset that fought.
fn consolidate_match_summary(
    summary: &crate::matchplay::MatchSummary,
    gameplay: &mut GameplayAggregatesV1,
) {
    use crate::matchplay::ModeSummary;
    gameplay.match_result = Some(summary.result.report_label());
    gameplay.mode_definition_id = Some(summary.mode_definition_id.0);
    match summary.mode_summary {
        ModeSummary::Wipeout(wipeout) => {
            gameplay.wipeout_final_scores = Some(wipeout.final_scores);
            gameplay.wipeout_target_score = Some(wipeout.target_score);
            gameplay.wipeout_score_margin = Some(wipeout.score_margin);
        }
        ModeSummary::HotZone(hot_zone) => {
            gameplay.hot_zone_final_progress = Some(hot_zone.final_progress_ticks);
            gameplay.hot_zone_target_progress_ticks = Some(hot_zone.target_progress_ticks);
            gameplay.hot_zone_controlled_ticks = Some(hot_zone.controlled_ticks_by_team);
            gameplay.hot_zone_contested_ticks = Some(hot_zone.contested_ticks);
            gameplay.hot_zone_control_gained_transitions =
                Some(hot_zone.control_gained_transitions_by_team);
            gameplay.hot_zone_longest_control_ticks =
                Some(hot_zone.longest_consecutive_control_ticks_by_team);
        }
    }
    gameplay.match_active_ticks = summary.active_duration_ticks;
    gameplay.match_respawns = summary.respawns;
    gameplay.team_a_defeats = summary.credited_defeats_by_team[0];
    gameplay.team_b_defeats = summary.credited_defeats_by_team[1];
    gameplay.first_hostile_damage_tick = summary.time_to_first_hostile_damage_ticks;
    gameplay.ability_attempts = summary.ability_telemetry.attempts;
    gameplay.ability_accepts = summary.ability_telemetry.accepts;
    gameplay.dash_uses = summary.ability_telemetry.dash_uses;
    gameplay.sentry_uses = summary.ability_telemetry.sentry_uses;
    for (_, aggregate) in &summary.weapon_aggregates {
        gameplay.accepted_attacks = gameplay
            .accepted_attacks
            .saturating_add(aggregate.accepted_attacks);
        gameplay.emitted_deliveries = gameplay
            .emitted_deliveries
            .saturating_add(aggregate.emitted_deliveries);
        gameplay.attacks_with_hostile_contact = gameplay
            .attacks_with_hostile_contact
            .saturating_add(aggregate.attacks_with_hostile_contact);
        gameplay.hostile_damage = gameplay
            .hostile_damage
            .saturating_add(aggregate.hostile_damage);
    }
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
    // Lightyear records cumulative byte totals as gauges, not counters.
    fn byte_gauge(
        registry: &lightyear::metrics::prelude::MetricsRegistry,
        name: &'static str,
    ) -> Option<u64> {
        registry
            .get_gauge_value(&Key::from_static_name(name))
            .map(|value| value.max(0.0) as u64)
    }
    state.transport.bytes_sent =
        byte_gauge(&registry, "transport/send_bytes").unwrap_or(state.transport.bytes_sent);
    state.transport.bytes_received =
        byte_gauge(&registry, "transport/recv_bytes").unwrap_or(state.transport.bytes_received);
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
pub(super) fn checkpoint_evidence_digest(
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
    (super::stable_digest(&refs), count)
}

// Bevy system parameters are owned by the scheduling runtime; `Res` cannot be borrowed here.
// The role-gated evidence parameters are how one finalizer serves both role lanes.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    clippy::too_many_arguments,
    reason = "every parameter is a Bevy system parameter owned by the schedule runtime; the role-gated evidence reads keep one finalization phase instead of duplicated per-role systems"
)]
fn finalize_closeout_report(
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
    state.report_written = true;
    let exit_category = classification.classified_category(exit);
    let mut manifest = settings.manifest.clone();
    if let Some(protocol) = protocol {
        manifest.registry_fingerprint = protocol.0;
    }
    if let Some(content) = content {
        manifest.content_fingerprint = content.0;
    }
    // Checkpoint convergence evidence comes from the process's own recorded scenario
    // checkpoints, so the digest, observation count, and divergence fields reflect
    // observed gameplay rather than schema presence. The manifest's declared
    // `checkpoint_count` stays untouched: it is the scenario's expectation, and the
    // terminal gate compares declaration against observation instead of overwriting
    // one with the other. Each App registers only its own role's evidence resources,
    // so sequential role resolution stays correct even in both-features test builds.
    #[cfg_attr(not(any(feature = "server", feature = "client")), allow(unused_mut))]
    let mut evidence = CloseoutEvidence::default();
    #[cfg(feature = "server")]
    if let Some(server) = server_evidence.as_deref() {
        evidence = server_closeout_evidence(server, combat_telemetry.as_deref());
    }
    #[cfg(feature = "client")]
    if let Some(observation) = client_observation.as_deref() {
        evidence = client_closeout_evidence(observation);
    }
    // Participant rows come from the during-run cache: by the terminal frame the role
    // shutdown chain may already have despawned every replicated fighter.
    if manifest.participants.is_empty() && !state.manifest_participants.is_empty() {
        manifest
            .participants
            .clone_from(&state.manifest_participants);
    }
    let gameplay = std::mem::take(&mut state.gameplay);
    let report = assemble_closeout_report(
        manifest,
        &settings,
        exit_category,
        &state,
        evidence,
        gameplay,
    );
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
/// fields because the manifest validator rejects `=` and newlines inside identity values;
/// the worst case (`u64` fingerprint, `u16` ids) stays far under the identity bound.
pub(super) fn participant_build_identity(build: &crate::builds::SelectedBuild) -> String {
    format!(
        "preset:{} fingerprint:{} revision:{}",
        build.source_build_preset_id.map_or(0, |id| u64::from(id.0)),
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
