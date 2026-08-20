//! Opt-in, bounded native render evidence for V3 closeout.

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
    "terrain_chunk_high_water",
    "terrain_chunk_terminal",
    "debris_high_water",
    "debris_terminal",
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
    "map_instance_id",
    "mode_definition_id",
    "result",
    "first_failure",
];

#[derive(Clone, Copy, Default)]
struct HighWater {
    entities: usize,
    mesh_entities: usize,
    visual_roots: usize,
    effects: usize,
    terrain_chunks: usize,
    debris: usize,
    fighters: usize,
    projectiles: usize,
    sentries: usize,
    meshes: usize,
    materials: usize,
}

#[derive(Resource)]
struct RenderMeasurementState {
    config: crate::config::RenderMeasurementConfig,
    ready_at: Option<Duration>,
    last_measurement: Option<Instant>,
    samples: Vec<f64>,
    high: HighWater,
    current: HighWater,
    measured_map: Option<(u64, crate::map::ModeDefinitionId)>,
    written: bool,
}

#[derive(SystemParam)]
struct RenderEvidenceQueries<'w, 's> {
    all: Query<'w, 's, Entity>,
    mesh_entities: Query<'w, 's, (), With<Mesh3d>>,
    visual_roots: Query<'w, 's, (), With<CombatVisualOwner>>,
    effects: Query<'w, 's, (), With<combat::CombatEffect3d>>,
    terrain_chunks: Query<'w, 's, (), With<crate::terrain::client::TerrainChunkVisual>>,
    debris: Query<'w, 's, (), With<crate::terrain::client::presentation::TerrainDebris>>,
    fighters: Query<'w, 's, (), With<Fighter>>,
    projectiles: Query<'w, 's, (), With<crate::combat::Projectile>>,
    sentries: Query<'w, 's, (), With<crate::abilities::Sentry>>,
    meshes: Res<'w, Assets<Mesh>>,
    materials: Res<'w, Assets<StandardMaterial>>,
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
            written: false,
        })
        .add_systems(Last, sample_and_finalize_render_measurement);
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
    let ready = assets
        .is_some_and(|value| !matches!(*value, assets::ClientAssetReadiness::Loading))
        && map_readiness
            .is_some_and(|value| matches!(*value, crate::map::ClientMapReadiness::Ready));
    // Readiness starts the bounded window; later match teardown must not restart it. Keeping the
    // original anchor makes the report cover the product match and its routed return-to-lobby
    // lifecycle instead of waiting forever once the measured match completes.
    if state.ready_at.is_none() && !ready {
        return;
    }
    let now = time.elapsed();
    let ready_at = *state.ready_at.get_or_insert(now);
    if let Some(map) = maps.iter().max_by_key(|map| map.identity.instance_id) {
        state.measured_map = Some((map.identity.instance_id.0, map.mode_definition_id));
    }
    if now.saturating_sub(ready_at) < state.config.warmup {
        return;
    }

    state.current = HighWater {
        entities: evidence.all.iter().count(),
        mesh_entities: evidence.mesh_entities.iter().count(),
        visual_roots: evidence.visual_roots.iter().count(),
        effects: evidence.effects.iter().count(),
        terrain_chunks: evidence.terrain_chunks.iter().count(),
        debris: evidence.debris.iter().count(),
        fighters: evidence.fighters.iter().count(),
        projectiles: evidence.projectiles.iter().count(),
        sentries: evidence.sentries.iter().count(),
        meshes: evidence.meshes.len(),
        materials: evidence.materials.len(),
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

fn update_high_water(high: &mut HighWater, current: HighWater) {
    high.entities = high.entities.max(current.entities);
    high.mesh_entities = high.mesh_entities.max(current.mesh_entities);
    high.visual_roots = high.visual_roots.max(current.visual_roots);
    high.effects = high.effects.max(current.effects);
    high.terrain_chunks = high.terrain_chunks.max(current.terrain_chunks);
    high.debris = high.debris.max(current.debris);
    high.fighters = high.fighters.max(current.fighters);
    high.projectiles = high.projectiles.max(current.projectiles);
    high.sentries = high.sentries.max(current.sentries);
    high.meshes = high.meshes.max(current.meshes);
    high.materials = high.materials.max(current.materials);
}

fn percentile(sorted: &[f64], percentage: usize) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let index = ((sorted.len() - 1) * percentage).div_ceil(100);
    sorted[index.min(sorted.len() - 1)]
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
    let failed = if samples.len() < 1_200 {
        Some("sample_count")
    } else if p95 > 18.5 {
        Some("frame_p95_ms")
    } else if p99 > 25.0 {
        Some("frame_p99_ms")
    } else if over_100 > 0 {
        Some("frames_over_100_ms")
    } else if over_25.saturating_mul(100) > samples.len() {
        Some("frames_over_25_ms")
    } else {
        None
    };
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
    let measured_map = map
        .map(|value| (value.identity.instance_id.0, value.mode_definition_id))
        .or(state.measured_map);
    let map_id = measured_map.map_or(0, |value| value.0);
    let mode_id =
        measured_map.map_or_else(|| "unknown".to_string(), |value| format!("{:?}", value.1));
    format!(
        "schema=1\nversion={}\ncommit={}\nrelease={}\nos={}\ncpu={}\nadapter={}\nbackend={}\nwindow_width={}\nwindow_height={}\nrender_profile={}\nfallback={}\nreduced_effects={}\nwarmup_seconds={}\nmeasurement_seconds={}\nsample_count={}\nframe_p50_ms={:.3}\nframe_p95_ms={:.3}\nframe_p99_ms={:.3}\nframe_max_ms={:.3}\nframes_over_25_ms={}\nframes_over_50_ms={}\nframes_over_100_ms={}\nentity_high_water={}\nentity_terminal={}\nmesh_entity_high_water={}\nmesh_entity_terminal={}\nvisual_root_high_water={}\nvisual_root_terminal={}\neffect_high_water={}\neffect_terminal={}\nterrain_chunk_high_water={}\nterrain_chunk_terminal={}\ndebris_high_water={}\ndebris_terminal={}\nfighter_high_water={}\nfighter_terminal={}\nprojectile_high_water={}\nprojectile_terminal={}\nsentry_high_water={}\nsentry_terminal={}\nmesh_asset_high_water={}\nmesh_asset_terminal={}\nmaterial_asset_high_water={}\nmaterial_asset_terminal={}\nmap_instance_id={}\nmode_definition_id={}\nresult={}\nfirst_failure={}\n",
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
        state.high.terrain_chunks,
        state.current.terrain_chunks,
        state.high.debris,
        state.current.debris,
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
        map_id,
        mode_id,
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
