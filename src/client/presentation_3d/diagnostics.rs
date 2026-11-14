//! Opt-in, bounded native render evidence for V3 closeout.

#[allow(
    clippy::wildcard_imports,
    reason = "the world-renderer submodule consumes the presentation_3d composition surface"
)]
use super::*;
use atomic_write_file::AtomicWriteFile;
use bevy::ecs::system::SystemParam;
use bevy::{
    diagnostic::{
        DiagnosticsStore, FrameTimeDiagnosticsPlugin, SystemInfo,
        SystemInformationDiagnosticsPlugin,
    },
    platform::time::Instant,
    render::renderer::RenderAdapterInfo,
};
#[cfg(test)]
use std::env;
use std::{collections::HashSet, io::Write, time::Duration};

const SAMPLE_CAPACITY: usize = 8_192;
const REPORT_FIELDS: &[&str] = &[
    "schema",
    "version",
    "commit",
    "release",
    "os",
    "cpu",
    "adapter",
    "backend",
    "window_width",
    "window_height",
    "render_profile",
    "measurement_context",
    "fallback",
    "reduced_effects",
    "warmup_seconds",
    "measurement_seconds",
    "sample_count",
    "frame_p50_ms",
    "frame_p95_ms",
    "frame_p99_ms",
    "frame_max_ms",
    "frames_over_25_ms",
    "frames_over_50_ms",
    "frames_over_100_ms",
    "entity_high_water",
    "entity_terminal",
    "mesh_entity_high_water",
    "mesh_entity_terminal",
    "visual_root_high_water",
    "visual_root_terminal",
    "effect_high_water",
    "effect_terminal",
    "map_member_high_water",
    "map_member_terminal",
    "dynamic_map_visuals_high_water",
    "dynamic_map_visuals_terminal",
    "fighter_high_water",
    "fighter_terminal",
    "projectile_high_water",
    "projectile_terminal",
    "sentry_high_water",
    "sentry_terminal",
    "mesh_asset_high_water",
    "mesh_asset_terminal",
    "material_asset_high_water",
    "material_asset_terminal",
    "image_asset_high_water",
    "image_asset_terminal",
    "dashboard_owned_high_water",
    "dashboard_owned_terminal",
    "map_instance_id",
    "map_recipe_id",
    "mode_definition_id",
    "presentation_theme_id",
    "owner_moving_frames",
    "owner_stalled_frames",
    "owner_speed_ratio_p05",
    "owner_speed_ratio_p50",
    "owner_speed_ratio_p95",
    "owner_pullback_frames",
    "owner_pullback_max",
    "result",
    "first_failure",
];

#[derive(Clone, Copy, Default)]
struct HighWater {
    entities: usize,
    mesh_entities: usize,
    visual_roots: usize,
    effects: usize,
    map_members: usize,
    dynamic_map_visuals: usize,
    fighters: usize,
    projectiles: usize,
    sentries: usize,
    meshes: usize,
    materials: usize,
    images: usize,
    dashboard_owned: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum RenderMeasurementContext {
    Dashboard,
    #[default]
    Gameplay,
}

impl RenderMeasurementContext {
    const fn name(self) -> &'static str {
        match self {
            Self::Dashboard => "dashboard",
            Self::Gameplay => "gameplay",
        }
    }
}

type MeasuredMap = (
    u64,
    crate::map::MapRecipeId,
    crate::map::ModeDefinitionId,
    crate::map::MapPresentationThemeId,
);

fn measured_map(map: &crate::map::ResolvedMapSnapshot) -> MeasuredMap {
    (
        map.identity.instance_id.0,
        map.identity.recipe_id,
        map.mode_definition_id,
        map.presentation_theme_id,
    )
}

#[derive(Resource)]
struct RenderMeasurementState {
    config: crate::config::RenderMeasurementConfig,
    ready_at: Option<Duration>,
    last_measurement: Option<Instant>,
    samples: Vec<f64>,
    high: HighWater,
    current: HighWater,
    measured_map: Option<MeasuredMap>,
    context: Option<RenderMeasurementContext>,
    owner_motion: OwnerMotionSamples,
    written: bool,
}

/// Per-frame presented owner motion during the measurement window (BRL-0169).
///
/// For each rendered frame whose predicted step is moving, the presented planar speed is divided
/// by the predicted step speed. Fixed-tick stepping on a high-refresh display shows as stalled
/// frames (ratio near 0) alternating with catch-up frames (ratio near 2 at 120 Hz); smooth
/// presentation keeps the ratio near 1. Independently of the predicted step, a frame whose
/// presented displacement reverses the previous frame's direction by more than
/// [`OWNER_PULLBACK_DISTANCE`] counts as a pull-back (wall-contact overshoot snapping back).
#[derive(Default)]
struct OwnerMotionSamples {
    previous: Option<(Entity, Vec2)>,
    /// Direction of the newest non-negligible presented displacement.
    previous_direction: Option<Vec2>,
    stalled: usize,
    ratios: Vec<f32>,
    pullbacks: usize,
    max_pullback: f32,
}

/// Presented-to-predicted speed ratio below which a moving frame counts as stalled.
const OWNER_STALLED_RATIO: f32 = 0.05;
/// Predicted step speed below which the owner is treated as standing still.
const OWNER_MOVING_SPEED: f32 = 1.0;
/// Backward presented travel in one frame, in world units, that counts as a pull-back.
const OWNER_PULLBACK_DISTANCE: f32 = 0.05;

impl OwnerMotionSamples {
    fn sample(&mut self, owner: Option<(Entity, Vec2, Vec2)>, delta_seconds: f32) {
        let Some((entity, presented, step_velocity)) = owner else {
            self.previous = None;
            self.previous_direction = None;
            return;
        };
        let previous = self.previous.replace((entity, presented));
        let Some((previous_entity, previous_position)) = previous else {
            return;
        };
        if previous_entity != entity {
            self.previous_direction = None;
            return;
        }
        let displacement = presented - previous_position;
        if let Some(direction) = self.previous_direction {
            let backward = -displacement.dot(direction);
            if backward > OWNER_PULLBACK_DISTANCE {
                self.pullbacks += 1;
                self.max_pullback = self.max_pullback.max(backward);
            }
        }
        if displacement.length() > OWNER_PULLBACK_DISTANCE {
            self.previous_direction = Some(displacement.normalize());
        }
        let expected = step_velocity.length();
        if delta_seconds <= 0.0 || expected < OWNER_MOVING_SPEED {
            return;
        }
        let ratio = presented.distance(previous_position) / delta_seconds / expected;
        if self.ratios.len() < SAMPLE_CAPACITY {
            if ratio < OWNER_STALLED_RATIO {
                self.stalled += 1;
            }
            self.ratios.push(ratio);
        }
    }

    fn percentiles(&self) -> [f64; 3] {
        let mut sorted = self
            .ratios
            .iter()
            .map(|ratio| f64::from(*ratio))
            .collect::<Vec<_>>();
        sorted.sort_by(f64::total_cmp);
        [5, 50, 95].map(|percentage| percentile(&sorted, percentage))
    }
}

type DashboardOwnedFilter = Or<(With<flow::DashboardRoot>, With<dashboard::DashboardOwned>)>;

#[derive(SystemParam)]
struct RenderEvidenceQueries<'w, 's> {
    all: Query<'w, 's, Entity>,
    mesh_entities: Query<'w, 's, (), With<Mesh3d>>,
    visual_roots: Query<'w, 's, (), With<CombatVisualOwner>>,
    effects: Query<'w, 's, (), With<CombatEffect3d>>,
    map_members: Query<'w, 's, (), With<crate::map::MapPresentationMember>>,
    dynamic_map_visuals: Query<'w, 's, (), With<DynamicMapVisual>>,
    fighters: Query<'w, 's, (), With<Fighter>>,
    projectiles: Query<'w, 's, (), With<crate::combat::Projectile>>,
    sentries: Query<'w, 's, (), With<crate::abilities::Sentry>>,
    meshes: Res<'w, Assets<Mesh>>,
    materials: Res<'w, Assets<StandardMaterial>>,
    images: Res<'w, Assets<Image>>,
    dashboard_owned: Query<'w, 's, (), DashboardOwnedFilter>,
}

pub(super) struct RenderMeasurementPlugin(pub(crate) crate::config::RenderMeasurementConfig);

impl Plugin for RenderMeasurementPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            FrameTimeDiagnosticsPlugin::new(SAMPLE_CAPACITY),
            SystemInformationDiagnosticsPlugin,
        ))
        .insert_resource(RenderMeasurementState {
            config: self.0.clone(),
            ready_at: None,
            last_measurement: None,
            samples: Vec::with_capacity(SAMPLE_CAPACITY),
            high: HighWater::default(),
            current: HighWater::default(),
            measured_map: None,
            context: None,
            owner_motion: OwnerMotionSamples::default(),
            written: false,
        })
        .add_systems(
            Last,
            (sample_owner_motion, sample_and_finalize_render_measurement).chain(),
        );
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "the opt-in closeout sample records one bounded snapshot of distinct presentation owners"
)]
fn sample_and_finalize_render_measurement(
    mut state: ResMut<RenderMeasurementState>,
    time: Res<Time<Real>>,
    diagnostics: Res<DiagnosticsStore>,
    assets: Option<Res<assets::ClientAssetReadiness>>,
    map_readiness: Option<Res<crate::map::ClientMapReadiness>>,
    flow: Option<Res<State<ClientFlow>>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    adapter: Option<Res<RenderAdapterInfo>>,
    system: Option<Res<SystemInfo>>,
    evidence: RenderEvidenceQueries,
    fallback: Res<ImportedWorldFallbackPolicy>,
    settings: Option<Res<ClientShellSettings>>,
    maps: Query<&crate::map::ResolvedMapSnapshot, With<crate::map::MapRoot>>,
    mut exits: MessageWriter<AppExit>,
) {
    if state.written {
        return;
    }
    let assets_ready =
        assets.is_some_and(|value| !matches!(*value, assets::ClientAssetReadiness::Loading));
    let dashboard_ready = flow
        .as_deref()
        .is_some_and(|flow| *flow.get() == ClientFlow::Dashboard);
    let gameplay_ready =
        map_readiness.is_some_and(|value| matches!(*value, crate::map::ClientMapReadiness::Ready));
    let ready = assets_ready && (dashboard_ready || gameplay_ready);
    // Readiness starts the bounded window; later match teardown must not restart it. Keeping the
    // original anchor makes the report cover the product match and its routed return-to-lobby
    // lifecycle instead of waiting forever once the measured match completes.
    if state.ready_at.is_none() && !ready {
        return;
    }
    let now = time.elapsed();
    if state.context.is_none() && ready {
        state.context = Some(if dashboard_ready {
            RenderMeasurementContext::Dashboard
        } else {
            RenderMeasurementContext::Gameplay
        });
    }
    let ready_at = *state.ready_at.get_or_insert(now);
    if let Some(map) = maps.iter().max_by_key(|map| map.identity.instance_id) {
        state.measured_map = Some(measured_map(map));
    }
    if now.saturating_sub(ready_at) < state.config.warmup {
        return;
    }

    state.current = HighWater {
        entities: evidence.all.iter().count(),
        mesh_entities: evidence.mesh_entities.iter().count(),
        visual_roots: evidence.visual_roots.iter().count(),
        effects: evidence.effects.iter().count(),
        map_members: evidence.map_members.iter().count(),
        dynamic_map_visuals: evidence.dynamic_map_visuals.iter().count(),
        fighters: evidence.fighters.iter().count(),
        projectiles: evidence.projectiles.iter().count(),
        sentries: evidence.sentries.iter().count(),
        meshes: evidence.meshes.len(),
        materials: evidence.materials.len(),
        images: evidence.images.len(),
        dashboard_owned: evidence.dashboard_owned.iter().count(),
    };
    let current = state.current;
    update_high_water(&mut state.high, current);
    if let Some(measurement) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|value| value.measurement())
        && state.last_measurement != Some(measurement.time)
    {
        state.last_measurement = Some(measurement.time);
        if state.samples.len() < SAMPLE_CAPACITY {
            state.samples.push(measurement.value);
        }
    }
    if now.saturating_sub(ready_at) < state.config.warmup + state.config.measurement {
        return;
    }

    let window = windows.iter().next();
    let map = maps.iter().max_by_key(|map| map.identity.instance_id);
    let report = compose_report(
        &state,
        window,
        adapter.as_deref(),
        system.as_deref(),
        *fallback,
        settings.is_some_and(|value| value.reduced_combat_effects),
        map,
    );
    let write_result = validate_report(&report)
        .and_then(|()| write_report(&state.config.report_path, report.as_bytes()));
    match write_result {
        Ok(()) => {
            info!(path = %state.config.report_path.display(), "V3 render report written");
            state.written = true;
            exits.write(AppExit::Success);
        }
        Err(error) => {
            error!(path = %state.config.report_path.display(), %error, "V3 render report failed");
            state.written = true;
            exits.write(AppExit::error());
        }
    }
}

/// Sample the presented owner pose once per rendered frame inside the measurement window.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn sample_owner_motion(
    mut state: ResMut<RenderMeasurementState>,
    real: Res<Time<Real>>,
    time: Res<Time>,
    owners: Query<
        (Entity, &Position, &prediction::OwnerPrediction),
        (With<Fighter>, With<Controlled>),
    >,
) {
    let Some(ready_at) = state.ready_at else {
        return;
    };
    if state.written || real.elapsed().saturating_sub(ready_at) < state.config.warmup {
        return;
    }
    let owner = owners
        .iter()
        .next()
        .map(|(entity, position, prediction)| (entity, position.0, prediction.step_velocity()));
    state.owner_motion.sample(owner, time.delta_secs());
}

fn update_high_water(high: &mut HighWater, current: HighWater) {
    high.entities = high.entities.max(current.entities);
    high.mesh_entities = high.mesh_entities.max(current.mesh_entities);
    high.visual_roots = high.visual_roots.max(current.visual_roots);
    high.effects = high.effects.max(current.effects);
    high.map_members = high.map_members.max(current.map_members);
    high.dynamic_map_visuals = high.dynamic_map_visuals.max(current.dynamic_map_visuals);
    high.fighters = high.fighters.max(current.fighters);
    high.projectiles = high.projectiles.max(current.projectiles);
    high.sentries = high.sentries.max(current.sentries);
    high.meshes = high.meshes.max(current.meshes);
    high.materials = high.materials.max(current.materials);
    high.images = high.images.max(current.images);
    high.dashboard_owned = high.dashboard_owned.max(current.dashboard_owned);
}

fn percentile(sorted: &[f64], percentage: usize) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let index = ((sorted.len() - 1) * percentage).div_ceil(100);
    sorted[index.min(sorted.len() - 1)]
}

fn render_measurement_failure(
    sample_count: usize,
    p95: f64,
    p99: f64,
    over_25: usize,
    over_100: usize,
) -> Option<&'static str> {
    if sample_count < 1_200 {
        Some("sample_count")
    } else if p95 > 18.5 {
        Some("frame_p95_ms")
    } else if p99 > 25.0 {
        Some("frame_p99_ms")
    } else if over_100 > 0 {
        Some("frames_over_100_ms")
    } else if over_25.saturating_mul(100) > sample_count {
        Some("frames_over_25_ms")
    } else {
        None
    }
}

fn compose_report(
    state: &RenderMeasurementState,
    window: Option<&Window>,
    adapter: Option<&RenderAdapterInfo>,
    system: Option<&SystemInfo>,
    fallback: ImportedWorldFallbackPolicy,
    reduced_effects: bool,
    map: Option<&crate::map::ResolvedMapSnapshot>,
) -> String {
    let mut samples = state.samples.clone();
    samples.sort_by(f64::total_cmp);
    let p50 = percentile(&samples, 50);
    let p95 = percentile(&samples, 95);
    let p99 = percentile(&samples, 99);
    let maximum = samples.last().copied().unwrap_or_default();
    let over_25 = samples.iter().filter(|value| **value > 25.0).count();
    let over_50 = samples.iter().filter(|value| **value > 50.0).count();
    let over_100 = samples.iter().filter(|value| **value > 100.0).count();
    let failed = render_measurement_failure(samples.len(), p95, p99, over_25, over_100);
    let (width, height) = window.map_or((0, 0), |window| {
        (window.physical_width(), window.physical_height())
    });
    let adapter_name = adapter.map_or("unknown", |value| value.name.as_str());
    let backend = adapter.map_or_else(
        || "unknown".to_string(),
        |value| format!("{:?}", value.backend),
    );
    let cpu = system.map_or("unknown", |value| value.cpu.as_str());
    let os = system.map_or("unknown", |value| value.os.as_str());
    let measured_map = map.map(measured_map).or(state.measured_map);
    let map_id = measured_map.map_or(0, |value| value.0);
    let recipe_id = measured_map.map_or(0, |value| value.1.0);
    let mode_id =
        measured_map.map_or_else(|| "unknown".to_string(), |value| format!("{:?}", value.2));
    let theme_id = measured_map.map_or(0, |value| value.3.0);
    let context = state.context.unwrap_or_default().name();
    let owner_ratios = state.owner_motion.percentiles();
    format!(
        "schema=5\nversion={}\ncommit={}\nrelease={}\nos={}\ncpu={}\nadapter={}\nbackend={}\nwindow_width={}\nwindow_height={}\nrender_profile={}\nmeasurement_context={}\nfallback={}\nreduced_effects={}\nwarmup_seconds={}\nmeasurement_seconds={}\nsample_count={}\nframe_p50_ms={:.3}\nframe_p95_ms={:.3}\nframe_p99_ms={:.3}\nframe_max_ms={:.3}\nframes_over_25_ms={}\nframes_over_50_ms={}\nframes_over_100_ms={}\nentity_high_water={}\nentity_terminal={}\nmesh_entity_high_water={}\nmesh_entity_terminal={}\nvisual_root_high_water={}\nvisual_root_terminal={}\neffect_high_water={}\neffect_terminal={}\nmap_member_high_water={}\nmap_member_terminal={}\ndynamic_map_visuals_high_water={}\ndynamic_map_visuals_terminal={}\nfighter_high_water={}\nfighter_terminal={}\nprojectile_high_water={}\nprojectile_terminal={}\nsentry_high_water={}\nsentry_terminal={}\nmesh_asset_high_water={}\nmesh_asset_terminal={}\nmaterial_asset_high_water={}\nmaterial_asset_terminal={}\nimage_asset_high_water={}\nimage_asset_terminal={}\ndashboard_owned_high_water={}\ndashboard_owned_terminal={}\nmap_instance_id={}\nmap_recipe_id={}\nmode_definition_id={}\npresentation_theme_id={}\nowner_moving_frames={}\nowner_stalled_frames={}\nowner_speed_ratio_p05={:.3}\nowner_speed_ratio_p50={:.3}\nowner_speed_ratio_p95={:.3}\nowner_pullback_frames={}\nowner_pullback_max={:.3}\nresult={}\nfirst_failure={}\n",
        VERSION,
        option_env!("BRAWLER_GIT_COMMIT").unwrap_or("unknown"),
        !cfg!(debug_assertions),
        os,
        cpu,
        adapter_name,
        backend,
        width,
        height,
        RenderProfile::from_env().name(),
        context,
        if fallback == ImportedWorldFallbackPolicy::ForcePrimitive {
            "primitive"
        } else {
            "imported-auto"
        },
        reduced_effects,
        state.config.warmup.as_secs(),
        state.config.measurement.as_secs(),
        samples.len(),
        p50,
        p95,
        p99,
        maximum,
        over_25,
        over_50,
        over_100,
        state.high.entities,
        state.current.entities,
        state.high.mesh_entities,
        state.current.mesh_entities,
        state.high.visual_roots,
        state.current.visual_roots,
        state.high.effects,
        state.current.effects,
        state.high.map_members,
        state.current.map_members,
        state.high.dynamic_map_visuals,
        state.current.dynamic_map_visuals,
        state.high.fighters,
        state.current.fighters,
        state.high.projectiles,
        state.current.projectiles,
        state.high.sentries,
        state.current.sentries,
        state.high.meshes,
        state.current.meshes,
        state.high.materials,
        state.current.materials,
        state.high.images,
        state.current.images,
        state.high.dashboard_owned,
        state.current.dashboard_owned,
        map_id,
        recipe_id,
        mode_id,
        theme_id,
        state.owner_motion.ratios.len(),
        state.owner_motion.stalled,
        owner_ratios[0],
        owner_ratios[1],
        owner_ratios[2],
        state.owner_motion.pullbacks,
        state.owner_motion.max_pullback,
        if failed.is_none() { "pass" } else { "fail" },
        failed.unwrap_or("none"),
    )
}

fn validate_report(report: &str) -> Result<(), String> {
    let mut seen = HashSet::with_capacity(REPORT_FIELDS.len());
    for line in report.lines() {
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("invalid report line: {line}"))?;
        if key.is_empty() || value.is_empty() {
            return Err(format!("empty report field: {key}"));
        }
        if !seen.insert(key) {
            return Err(format!("duplicate report field: {key}"));
        }
    }
    for required in REPORT_FIELDS {
        if !seen.contains(required) {
            return Err(format!("missing report field: {required}"));
        }
    }
    if seen.len() != REPORT_FIELDS.len() {
        return Err("report contains an unknown field".to_string());
    }
    Ok(())
}

fn write_report(path: &std::path::Path, contents: &[u8]) -> Result<(), String> {
    if path.exists() {
        return Err("report path already exists".to_string());
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut file = AtomicWriteFile::open(path).map_err(|error| error.to_string())?;
    file.write_all(contents)
        .map_err(|error| error.to_string())?;
    file.commit().map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_uses_bounded_nearest_rank() {
        assert!(percentile(&[], 95).abs() < f64::EPSILON);
        assert!((percentile(&[1.0, 2.0, 3.0, 4.0], 50) - 3.0).abs() < f64::EPSILON);
        assert!((percentile(&[1.0, 2.0, 3.0, 4.0], 95) - 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn owner_motion_separates_tick_stepping_from_smooth_frames() {
        let owner = Entity::from_raw_u32(7).expect("valid entity index");
        let velocity = Vec2::new(120.0, 0.0);
        let frame = 1.0 / 120.0;
        // 60 Hz stepping on a 120 Hz display: every other frame repeats the pose.
        let mut stepped = OwnerMotionSamples::default();
        let mut x = 0.0;
        for frame_index in 0..9 {
            if frame_index % 2 == 0 {
                x += 2.0;
            }
            stepped.sample(Some((owner, Vec2::new(x, 0.0), velocity)), frame);
        }
        assert_eq!(stepped.ratios.len(), 8);
        assert_eq!(stepped.stalled, 4);
        let [p05, _, p95] = stepped.percentiles();
        assert!(p05 < 0.01 && (p95 - 2.0).abs() < 1e-3, "{p05} {p95}");
        // Smooth presentation advances every frame by one frame of travel.
        let mut smooth = OwnerMotionSamples::default();
        for frame_index in 0..9_u8 {
            let x = f32::from(frame_index) * velocity.x * frame;
            smooth.sample(Some((owner, Vec2::new(x, 0.0), velocity)), frame);
        }
        assert_eq!(smooth.stalled, 0);
        assert_eq!(smooth.pullbacks, 0);
        let [p05, p50, p95] = smooth.percentiles();
        assert!((p05 - 1.0).abs() < 1e-3 && (p50 - 1.0).abs() < 1e-3 && (p95 - 1.0).abs() < 1e-3);
        // Standing still or losing the owner records nothing.
        smooth.sample(Some((owner, Vec2::ZERO, Vec2::ZERO)), frame);
        smooth.sample(None, frame);
        assert_eq!(smooth.ratios.len(), 8);
        assert_eq!(stepped.pullbacks, 0, "tick stepping never moves backward");
    }

    #[test]
    fn owner_motion_counts_wall_contact_pull_backs() {
        let owner = Entity::from_raw_u32(7).expect("valid entity index");
        let velocity = Vec2::new(120.0, 0.0);
        let frame = 1.0 / 120.0;
        let mut samples = OwnerMotionSamples::default();
        // Walking forward, overshooting by 0.5, then snapping back to the contact and holding.
        for x in [0.0, 1.0, 2.0, 2.5, 2.0, 2.0, 2.0] {
            samples.sample(Some((owner, Vec2::new(x, 0.0), velocity)), frame);
        }
        assert_eq!(samples.pullbacks, 1);
        assert!((samples.max_pullback - 0.5).abs() < 1e-5);
    }

    #[test]
    fn report_writer_refuses_overwrite() {
        let path =
            env::temp_dir().join(format!("brawler-render-report-{}.txt", std::process::id()));
        let _ = std::fs::remove_file(&path);
        write_report(&path, b"first").unwrap();
        assert!(write_report(&path, b"second").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"first");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn report_validation_rejects_missing_and_duplicate_fields() {
        let complete = REPORT_FIELDS
            .iter()
            .map(|field| format!("{field}=value"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(validate_report(&complete).is_ok());
        assert!(validate_report("schema=1\nschema=1").is_err());
        assert!(validate_report("schema=1").is_err());
    }
}
