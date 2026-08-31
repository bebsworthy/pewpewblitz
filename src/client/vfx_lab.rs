//! Development-only read-only viewer over the production VFX registry and materializer.

use super::{
    ArenaCamera,
    evidence_capture::{
        ClientEvidenceCapturePlugin, current_unix_millis, request_custom_evidence_capture,
    },
    hud::ClientHeistReadiness,
    presentation_3d::{
        CombatEffect3d, ImportedWorldFallbackPolicy, PendingCombatEffect,
        VfxLabLifecyclePresentationPlugin, VfxLabLifecycleSet, collect_vfx_evidence_snapshot,
        materialize_combat_effects, resolve_vfx_requests, setup_3d_foundation,
    },
    settings::persistence::ClientShellSettings,
    vfx::{
        ProductionVfxPlugin, VfxDeadline, VfxLifetime, VfxRegistry, VfxRequest,
        VfxRequestCapabilities, VfxRequestKey, VfxRequestOrder, embedded_vfx_catalog_fingerprint,
    },
};
use crate::combat::AuthoritativeTick;
use avian2d::prelude::{Position, Rotation};
use bevy::{
    app::AppExit,
    prelude::*,
    window::{PrimaryWindow, WindowResolution},
};
use core::time::Duration;
use lightyear::prelude::Controlled;
use serde_json::json;
use std::path::PathBuf;

const DEFAULT_RADIUS: u16 = 72;

/// Command-line configuration for the read-only production VFX viewer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent command-line switches are the complete bounded Lab startup contract"
)]
pub struct VfxLabConfig {
    pub effect_key: Option<String>,
    pub fixture: Option<String>,
    pub presentation_identity: Option<u64>,
    pub radius: Option<u16>,
    pub reduced: bool,
    pub primitive: bool,
    pub paused: bool,
    pub window_size: Option<(u16, u16)>,
    pub capture_dir: Option<PathBuf>,
    pub capture_after_updates: Option<u32>,
    pub exit_after_capture: bool,
}

impl VfxLabConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self
            .effect_key
            .as_deref()
            .is_some_and(|key| key.is_empty() || key.len() > 64)
        {
            return Err("VfxLab effect key must contain 1..=64 bytes".to_string());
        }
        if self
            .window_size
            .is_some_and(|(width, height)| width < 640 || height < 480)
        {
            return Err("VfxLab window must be at least 640x480".to_string());
        }
        if self
            .capture_dir
            .as_ref()
            .is_some_and(|path| path.as_os_str().is_empty())
        {
            return Err("VfxLab capture directory must not be empty".to_string());
        }
        if self
            .fixture
            .as_deref()
            .is_some_and(|fixture| VfxLabFixture::from_key(fixture).is_none())
        {
            return Err(
                "VfxLab fixture must be transient, straight, lobbed, field, cone, or status"
                    .to_string(),
            );
        }
        if self.capture_after_updates == Some(0) {
            return Err("VfxLab capture update count must be positive".to_string());
        }
        if self.presentation_identity == Some(0) {
            return Err("VfxLab presentation identity must be positive".to_string());
        }
        if self
            .radius
            .is_some_and(|radius| !(1..=256).contains(&radius))
        {
            return Err("VfxLab radius must be between 1 and 256 world units".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct VfxLabScenario {
    key: VfxRequestKey,
    producer_rank: u16,
    capabilities: VfxRequestCapabilities,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum VfxLabFixture {
    #[default]
    Transient,
    StraightProjectile,
    LobbedProjectile,
    ElementalField,
    ConeSpray,
    Status,
}

impl VfxLabFixture {
    const COUNT: u8 = 6;

    fn next(self) -> Self {
        match self {
            Self::Transient => Self::StraightProjectile,
            Self::StraightProjectile => Self::LobbedProjectile,
            Self::LobbedProjectile => Self::ElementalField,
            Self::ElementalField => Self::ConeSpray,
            Self::ConeSpray => Self::Status,
            Self::Status => Self::Transient,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Transient => "transient recipe",
            Self::StraightProjectile => "straight projectile / line",
            Self::LobbedProjectile => "lobbed projectile / arc",
            Self::ElementalField => "elemental field / area",
            Self::ConeSpray => "cone spray / propagation",
            Self::Status => "fighter status / attached",
        }
    }

    fn from_key(value: &str) -> Option<Self> {
        match value {
            "transient" => Some(Self::Transient),
            "straight" => Some(Self::StraightProjectile),
            "lobbed" => Some(Self::LobbedProjectile),
            "field" => Some(Self::ElementalField),
            "cone" => Some(Self::ConeSpray),
            "status" => Some(Self::Status),
            _ => None,
        }
    }
}

impl VfxLabScenario {
    fn request(self, event_id: u64, radius: u16) -> VfxRequest {
        let authoritative_radius = self
            .capabilities
            .authoritative_radius
            .then_some(f32::from(radius));
        let deadline = self
            .capabilities
            .authoritative_deadline
            .then_some(VfxDeadline::new(0, 300, Some(0)));
        VfxRequest::try_new(
            self.key,
            VfxRequestOrder::new(self.producer_rank, event_id),
            Vec2::ZERO,
            authoritative_radius,
            deadline,
            self.key.as_str(),
        )
        .expect("sealed production registration produces a valid lab request")
    }
}

#[derive(Resource)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent read-only viewer toggles remain explicit inspectable state"
)]
struct VfxLabState {
    scenarios: Vec<VfxLabScenario>,
    selected: usize,
    presentation_identity: u64,
    radius: u16,
    reduced: bool,
    primitive: bool,
    paused: bool,
    step_once: bool,
    capture_dir: PathBuf,
    capture_sequence: u32,
    fixture: VfxLabFixture,
    playback_tick: u64,
    tick_accumulator: Duration,
    capture_after_updates: Option<u32>,
    update_index: u32,
    exit_after_capture: bool,
    pending_capture_paths: Option<(PathBuf, PathBuf)>,
}

impl VfxLabState {
    fn selected(&self) -> VfxLabScenario {
        self.scenarios[self.selected]
    }

    fn transient_request(&self) -> Option<VfxRequest> {
        matches!(self.fixture, VfxLabFixture::Transient).then(|| {
            self.selected()
                .request(self.presentation_identity, self.radius)
        })
    }

    fn use_next_identity(&mut self) {
        self.presentation_identity = self.presentation_identity.saturating_add(1);
    }

    fn restart_playback(&mut self) {
        self.playback_tick = 30;
        self.tick_accumulator = Duration::ZERO;
        self.step_once = false;
    }

    fn select_relative(&mut self, offset: isize) {
        let len = self.scenarios.len();
        self.selected = self.selected.saturating_add_signed(offset).min(len - 1);
    }
}

#[derive(Component)]
struct VfxLabInspector;

#[derive(Component)]
struct VfxLabFixtureOwner;

#[derive(Component)]
struct VfxLabClock;

struct VfxLabPlugin {
    config: VfxLabConfig,
}

impl Plugin for VfxLabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClientShellSettings {
            reduced_combat_effects: self.config.reduced,
            ..Default::default()
        })
        .insert_resource(if self.config.primitive {
            ImportedWorldFallbackPolicy::ForcePrimitive
        } else {
            ImportedWorldFallbackPolicy::Auto
        })
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.72, 0.78, 0.9),
            brightness: 350.0,
            ..default()
        })
        .add_message::<PendingCombatEffect>()
        .add_systems(Startup, setup_3d_foundation)
        .add_systems(Startup, setup_lab_scene.after(setup_3d_foundation))
        .add_systems(
            Update,
            (
                handle_lab_input,
                resolve_vfx_requests,
                materialize_combat_effects,
                advance_lab_effects,
                update_lab_fixture_facts,
                hide_gameplay_hud,
                update_lab_inspector,
                exit_after_lab_capture,
            )
                .chain(),
        )
        .configure_sets(Update, VfxLabLifecycleSet.after(update_lab_fixture_facts))
        .add_systems(PostUpdate, request_lab_capture);
    }

    fn finish(&self, app: &mut App) {
        let registry = app.world().resource::<VfxRegistry>();
        let scenarios = registry
            .registrations()
            .iter()
            .map(|registration| VfxLabScenario {
                key: registration.key,
                producer_rank: registration.producer_rank,
                capabilities: registration.capabilities,
            })
            .collect::<Vec<_>>();
        let selected = self
            .config
            .effect_key
            .as_deref()
            .and_then(|key| {
                scenarios
                    .iter()
                    .position(|scenario| scenario.key.as_str() == key)
            })
            .unwrap_or(0);
        app.insert_resource(VfxLabState {
            scenarios,
            selected,
            presentation_identity: self.config.presentation_identity.unwrap_or(1),
            radius: self.config.radius.unwrap_or(DEFAULT_RADIUS),
            reduced: self.config.reduced,
            primitive: self.config.primitive,
            paused: self.config.paused,
            step_once: false,
            capture_dir: self
                .config
                .capture_dir
                .clone()
                .unwrap_or_else(|| PathBuf::from("target/vfx-lab")),
            capture_sequence: 0,
            fixture: self
                .config
                .fixture
                .as_deref()
                .and_then(VfxLabFixture::from_key)
                .unwrap_or_default(),
            playback_tick: 30,
            tick_accumulator: Duration::ZERO,
            capture_after_updates: self.config.capture_after_updates,
            update_index: 0,
            exit_after_capture: self.config.exit_after_capture,
            pending_capture_paths: None,
        });
    }
}

/// Build the offline, windowed `VfxLab` application.
pub fn build_vfx_lab_app(config: VfxLabConfig) -> App {
    config.validate().expect("VfxLab configuration is valid");
    let (width, height) = config.window_size.unwrap_or((1280, 720));
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: format!("{}/assets", env!("CARGO_MANIFEST_DIR")),
                ..default()
            })
            .set(ImagePlugin::default_nearest())
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Brawler VfxLab — read-only production viewer".to_string(),
                    resolution: WindowResolution::new(u32::from(width), u32::from(height)),
                    ..default()
                }),
                ..default()
            }),
    )
    .add_plugins(crate::content::GameplayContentPlugin)
    .add_message::<crate::combat::client::DeduplicatedCombatCue>()
    .add_message::<crate::matchplay::ReceivedHeistObjectiveCue>()
    .insert_resource(ClientHeistReadiness::default())
    .add_plugins(ProductionVfxPlugin)
    .add_plugins(VfxLabLifecyclePresentationPlugin)
    .add_plugins(ClientEvidenceCapturePlugin)
    .add_plugins(VfxLabPlugin { config });
    app
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "resources are Bevy system parameters owned by the scheduler"
)]
fn setup_lab_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut requests: MessageWriter<VfxRequest>,
    state: Res<VfxLabState>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(720.0, 1.0, 520.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.075, 0.085, 0.105),
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::from_xyz(0.0, -0.5, 0.0),
        Name::new("VfxLab neutral floor"),
    ));
    commands.spawn((
        VfxLabInspector,
        Text::new("VfxLab loading production catalog…"),
        TextFont::from_font_size(18.0),
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: px(16.0),
            top: px(16.0),
            padding: UiRect::all(px(12.0)),
            max_width: px(510.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.015, 0.025, 0.04, 0.94)),
        GlobalZIndex(400),
    ));
    commands.spawn((VfxLabClock, AuthoritativeTick(state.playback_tick)));
    spawn_lab_scenario(&mut commands, &state, &mut requests);
}

fn spawn_lab_scenario(
    commands: &mut Commands,
    state: &VfxLabState,
    requests: &mut MessageWriter<VfxRequest>,
) {
    if let Some(request) = state.transient_request() {
        requests.write(request);
        return;
    }
    let tick = state.playback_tick;
    let source = crate::combat::ProjectileSource {
        shot_id: crate::combat::ShotId(state.presentation_identity),
        player_id: crate::protocol::PlayerId(1),
        owner_network_entity_id: crate::protocol::NetworkEntityId(1),
        team_id: crate::combat::TeamId(0),
        weapon_definition_id: crate::combat::PULSE_SIDEARM_DEFINITION,
    };
    match state.fixture {
        VfxLabFixture::Transient => unreachable!("transient scenario returned above"),
        VfxLabFixture::StraightProjectile => spawn_straight_fixture(commands, source, tick),
        VfxLabFixture::LobbedProjectile => spawn_lobbed_fixture(commands, source, tick),
        VfxLabFixture::ElementalField => {
            spawn_field_fixture(commands, state.presentation_identity, state.radius, tick);
        }
        VfxLabFixture::ConeSpray => spawn_cone_fixture(commands, tick),
        VfxLabFixture::Status => {
            spawn_status_fixture(commands, state.presentation_identity, tick);
        }
    }
}

fn spawn_straight_fixture(
    commands: &mut Commands,
    source: crate::combat::ProjectileSource,
    tick: u64,
) {
    commands.spawn((
        VfxLabFixtureOwner,
        crate::combat::Projectile,
        source,
        Position::from_xy(-160.0, 0.0),
        Rotation::radians(0.0),
        crate::combat::StraightFlight {
            origin: crate::combat::WorldPoint::from(Vec2::new(-160.0, 0.0)),
            facing: 0.0,
            speed: 360.0,
            maximum_range: 320.0,
            launched_at_tick: tick,
        },
        crate::combat::ProjectileBody::circle(6.0),
        Name::new("VfxLab straight projectile fixture"),
    ));
}

fn spawn_lobbed_fixture(
    commands: &mut Commands,
    source: crate::combat::ProjectileSource,
    tick: u64,
) {
    commands.spawn((
        VfxLabFixtureOwner,
        crate::combat::Projectile,
        crate::combat::ProjectileSource {
            weapon_definition_id: crate::combat::ARC_LAUNCHER_DEFINITION,
            ..source
        },
        Position::from_xy(-140.0, 0.0),
        Rotation::radians(0.0),
        crate::combat::LobbedFlight {
            launch: crate::combat::WorldPoint::from(Vec2::new(-140.0, 0.0)),
            landing: crate::combat::WorldPoint::from(Vec2::new(140.0, 0.0)),
            launched_at_tick: tick,
            lands_at_tick: tick.saturating_add(90),
            visual_arc_height: 120.0,
        },
        Name::new("VfxLab lobbed projectile fixture"),
    ));
}

fn spawn_field_fixture(commands: &mut Commands, identity: u64, radius: u16, tick: u64) {
    commands.spawn((
        VfxLabFixtureOwner,
        crate::combat::ElementalFieldState {
            id: crate::combat::ElementalFieldId(identity),
            kind: crate::combat::ElementalFieldKind::Fire,
            owner_network_entity_id: crate::protocol::NetworkEntityId(1),
            team_id: crate::combat::TeamId(0),
            center: crate::combat::WorldPoint::from(Vec2::ZERO),
            radius_milliunits: u32::from(radius) * 1_000,
            activated_at_tick: tick,
            next_pulse_tick: tick.saturating_add(15),
            expires_at_tick: tick.saturating_add(300),
        },
        Name::new("VfxLab elemental field fixture"),
    ));
}

fn spawn_cone_fixture(commands: &mut Commands, tick: u64) {
    commands.spawn((
        VfxLabFixtureOwner,
        crate::combat::ConeSpray,
        crate::combat::ConeSprayState {
            origin: crate::combat::WorldPoint::from(Vec2::new(-120.0, 0.0)),
            facing: 0.0,
            propagation_speed: 480.0,
            maximum_reach: 300.0,
            angle_degrees: 70.0,
            emitted_at_tick: tick,
            full_at_tick: tick.saturating_add(38),
            expires_at_tick: tick.saturating_add(180),
            pulse_interval_ticks: 10,
            map_occlusion: false,
            max_targets: 6,
        },
        Name::new("VfxLab cone spray fixture"),
    ));
}

fn spawn_status_fixture(commands: &mut Commands, identity: u64, tick: u64) {
    commands.spawn((
        VfxLabFixtureOwner,
        VfxLabClock,
        crate::protocol::Fighter,
        Controlled,
        crate::combat::TeamId(0),
        Position::from_xy(0.0, 0.0),
        Rotation::radians(0.0),
        AuthoritativeTick(tick),
        crate::combat::ActiveEffects {
            slow: Some(crate::combat::SlowEffect {
                source_attack_id: crate::combat::AttackId(identity),
                source_network_entity_id: crate::protocol::NetworkEntityId(1),
                movement_multiplier_milli: 700,
                expires_at_tick: tick.saturating_add(300),
            }),
            ..Default::default()
        },
        Name::new("VfxLab attached status fixture"),
    ));
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "resources are Bevy system parameters owned by the scheduler"
)]
fn handle_lab_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<VfxLabState>,
    mut settings: ResMut<ClientShellSettings>,
    mut fallback: ResMut<ImportedWorldFallbackPolicy>,
    mut requests: MessageWriter<VfxRequest>,
    mut commands: Commands,
    active_effects: Query<Entity, With<CombatEffect3d>>,
    fixture_owners: Query<Entity, With<VfxLabFixtureOwner>>,
    mut clocks: Query<&mut AuthoritativeTick, With<VfxLabClock>>,
    mut exits: MessageWriter<AppExit>,
) {
    let mut restart = false;
    if matches!(state.fixture, VfxLabFixture::Transient)
        && keyboard.just_pressed(KeyCode::ArrowLeft)
    {
        state.select_relative(-1);
        restart = true;
    }
    if matches!(state.fixture, VfxLabFixture::Transient)
        && keyboard.just_pressed(KeyCode::ArrowRight)
    {
        state.select_relative(1);
        restart = true;
    }
    if keyboard.just_pressed(KeyCode::Tab) {
        state.fixture = state.fixture.next();
        restart = true;
    }
    if keyboard.just_pressed(KeyCode::Space) {
        restart = true;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        state.reduced = !state.reduced;
        settings.reduced_combat_effects = state.reduced;
        restart = true;
    }
    if keyboard.just_pressed(KeyCode::KeyP) {
        state.primitive = !state.primitive;
        *fallback = if state.primitive {
            ImportedWorldFallbackPolicy::ForcePrimitive
        } else {
            ImportedWorldFallbackPolicy::Auto
        };
        restart = true;
    }
    if keyboard.just_pressed(KeyCode::Enter) {
        state.paused = !state.paused;
    }
    if state.paused && keyboard.just_pressed(KeyCode::Period) {
        state.step_once = true;
    }
    if keyboard.just_pressed(KeyCode::KeyN) {
        state.use_next_identity();
        restart = true;
    }
    if restart {
        state.restart_playback();
        for mut tick in &mut clocks {
            tick.0 = state.playback_tick;
        }
        for entity in &active_effects {
            commands.entity(entity).try_despawn();
        }
        for entity in &fixture_owners {
            commands.entity(entity).try_despawn();
        }
        spawn_lab_scenario(&mut commands, &state, &mut requests);
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        exits.write(AppExit::Success);
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "resources are Bevy system parameters owned by the scheduler"
)]
fn advance_lab_effects(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut state: ResMut<VfxLabState>,
    mut effects: Query<(Entity, &mut CombatEffect3d)>,
    mut clocks: Query<&mut AuthoritativeTick, With<VfxLabClock>>,
) {
    let delta = if state.step_once {
        state.step_once = false;
        crate::timing::SIMULATION_TICK
    } else if state.paused {
        Duration::ZERO
    } else {
        time.delta()
    };
    if delta.is_zero() {
        return;
    }
    state.tick_accumulator = state.tick_accumulator.saturating_add(delta);
    let mut advanced_ticks = 0_u64;
    while state.tick_accumulator >= crate::timing::SIMULATION_TICK {
        state.tick_accumulator = state
            .tick_accumulator
            .saturating_sub(crate::timing::SIMULATION_TICK);
        advanced_ticks = advanced_ticks.saturating_add(1);
    }
    state.playback_tick = state.playback_tick.saturating_add(advanced_ticks);
    for mut tick in &mut clocks {
        tick.0 = state.playback_tick;
    }
    for (entity, mut effect) in &mut effects {
        if effect.advance_and_is_finished(delta, None) {
            commands.entity(entity).try_despawn();
        }
    }
}

#[allow(
    clippy::cast_precision_loss,
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "bounded Lab ticks are converted to production presentation interpolation inputs"
)]
fn update_lab_fixture_facts(
    state: Res<VfxLabState>,
    mut projectiles: Query<
        (
            &mut Position,
            Option<&crate::combat::StraightFlight>,
            Option<&crate::combat::LobbedFlight>,
        ),
        With<VfxLabFixtureOwner>,
    >,
) {
    for (mut position, straight, lobbed) in &mut projectiles {
        if let Some(flight) = straight {
            let elapsed_ticks = state.playback_tick.saturating_sub(flight.launched_at_tick);
            let elapsed_seconds = elapsed_ticks as f32 / crate::timing::SIMULATION_TICK_HZ as f32;
            let distance = (elapsed_seconds * flight.speed).min(flight.maximum_range);
            position.0 = flight.origin.as_vec2() + Vec2::from_angle(flight.facing) * distance;
        } else if let Some(flight) = lobbed {
            let duration = flight
                .lands_at_tick
                .saturating_sub(flight.launched_at_tick)
                .max(1);
            let elapsed = state
                .playback_tick
                .saturating_sub(flight.launched_at_tick)
                .min(duration);
            let progress = elapsed as f32 / duration as f32;
            position.0 = flight
                .launch
                .as_vec2()
                .lerp(flight.landing.as_vec2(), progress);
        }
    }
}

type VfxLabGameplayHudQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Visibility,
    Or<(
        With<crate::combat::CombatHudText>,
        With<crate::combat::CombatAbilityHudText>,
    )>,
>;

fn hide_gameplay_hud(mut hud: VfxLabGameplayHudQuery) {
    for mut visibility in &mut hud {
        *visibility = Visibility::Hidden;
    }
}

fn request_lab_capture(world: &mut World) {
    let keyboard_requested = world
        .get_resource::<ButtonInput<KeyCode>>()
        .is_some_and(|keyboard| keyboard.just_pressed(KeyCode::F12));
    let capture = {
        let mut state = world.resource_mut::<VfxLabState>();
        state.update_index = state.update_index.saturating_add(1);
        let scheduled = state
            .capture_after_updates
            .is_some_and(|target| state.update_index >= target);
        if !keyboard_requested && !scheduled {
            None
        } else {
            if scheduled {
                state.capture_after_updates = None;
            }
            let sequence = state.capture_sequence;
            state.capture_sequence = state.capture_sequence.saturating_add(1);
            Some((
                state.capture_dir.clone(),
                sequence,
                state.selected(),
                state.fixture,
                state.presentation_identity,
                state.reduced,
                state.primitive,
                state.paused,
                state.playback_tick,
            ))
        }
    };
    let Some((directory, sequence, scenario, fixture, identity, reduced, primitive, paused, tick)) =
        capture
    else {
        return;
    };
    let radius = world.resource::<VfxLabState>().radius;
    let request =
        matches!(fixture, VfxLabFixture::Transient).then(|| scenario.request(identity, radius));
    let profile = request.as_ref().map(|request| {
        world
            .resource::<VfxRegistry>()
            .resolve(request, reduced)
            .expect("production lab scenario resolves")
            .clone()
    });
    let unix_millis = current_unix_millis();
    let source_label = if matches!(fixture, VfxLabFixture::Transient) {
        scenario.key.as_str()
    } else {
        fixture.label()
    };
    let stem = format!(
        "vfx-lab-{}-{identity:06}-{unix_millis}-{sequence:03}",
        source_label
            .replace(['.', ' ', '/'], "-")
            .to_ascii_lowercase()
    );
    {
        let mut state = world.resource_mut::<VfxLabState>();
        if state.exit_after_capture {
            state.pending_capture_paths = Some((
                directory.join(format!("{stem}.png")),
                directory.join(format!("{stem}.json")),
            ));
        }
    }
    let snapshot = json!({
        "schema_version": 1,
        "capture_kind": "vfx-lab",
        "build_version": crate::VERSION,
        "captured_unix_millis": unix_millis,
        "catalog_fingerprint": embedded_vfx_catalog_fingerprint(),
        "lab": {
            "fixture": fixture.label(),
            "request_key": request.as_ref().map(|_| scenario.key.as_str()),
            "producer_rank": request.as_ref().map(|_| scenario.producer_rank),
            "presentation_identity": identity,
            "radius": radius,
            "playback_tick": tick,
            "authoritative_radius": request.as_ref().and_then(|request| request.authoritative_radius),
            "authoritative_deadline": request.as_ref().and_then(|request| request.deadline).map(|deadline| json!({
                "activated_at_tick": deadline.activated_at_tick,
                "expires_at_tick": deadline.expires_at_tick,
                "observed_at_tick": deadline.observed_at_tick,
            })),
            "reduced": reduced,
            "primitive": primitive,
            "paused": paused,
            "resolved_profile": profile,
        },
        "vfx": collect_vfx_evidence_snapshot(world),
    });
    request_custom_evidence_capture(world, &directory, &stem, &snapshot);
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "resources are Bevy system parameters owned by the scheduler"
)]
fn exit_after_lab_capture(state: Res<VfxLabState>, mut exits: MessageWriter<AppExit>) {
    if state
        .pending_capture_paths
        .as_ref()
        .is_some_and(|(png, json)| png.is_file() && json.is_file())
    {
        exits.write(AppExit::Success);
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "resources are Bevy system parameters owned by the scheduler"
)]
fn update_lab_inspector(
    state: Res<VfxLabState>,
    registry: Res<VfxRegistry>,
    effects: Query<(), With<CombatEffect3d>>,
    fixtures: Query<(), With<VfxLabFixtureOwner>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut inspector: Query<&mut Text, With<VfxLabInspector>>,
    cameras: Query<(), With<ArenaCamera>>,
) {
    let scenario = state.selected();
    let scenario_details = if let Some(request) = state.transient_request() {
        let profile = registry
            .resolve(&request, state.reduced)
            .expect("production lab scenario resolves");
        let lifetime = match profile.lifetime {
            VfxLifetime::Millis(value) => format!("{value} ms"),
            VfxLifetime::AuthoritativeDeadline => "authoritative deadline".to_string(),
        };
        format!(
            "Mapping  {}/{}  {}\nProfile {}\nRenderer {:?} - material {:?}\nScale {:?} - anchor {:?}\nLifetime {} - cap {}\nFallback {}",
            state.selected + 1,
            state.scenarios.len(),
            scenario.key.as_str(),
            profile.id,
            profile.renderer,
            profile.material,
            profile.scale,
            profile.anchor,
            lifetime,
            profile.concurrency_cap,
            profile.fallback_profile,
        )
    } else {
        format!(
            "Fixture  {}/{}  {}\nSource   production lifecycle facts + materializer\nTick     {}",
            fixture_index(state.fixture),
            VfxLabFixture::COUNT,
            state.fixture.label(),
            state.playback_tick,
        )
    };
    let window = windows.single().ok();
    let dimensions = window.map_or_else(
        || "window unavailable".to_string(),
        |window| format!("{:.0}x{:.0}", window.width(), window.height()),
    );
    let text = format!(
        "VFXLAB - READ-ONLY PRODUCTION VIEWER\n\n\
         {}\n\n\
         Variant  {}{}\n\
         Playback {} - identity {}\n\
         Radius   {} world units\n\
         Capture  {}\n\
         Catalog  {:016x} - sealed/valid\n\
         Active transient {} - fixture facts {} - cameras {} - {}\n\n\
         Tab fixture   <-/-> mapping   Space restart   Enter pause   . step\n\
         N identity   R reduced   P primitive   F12 capture   Esc quit",
        scenario_details,
        if state.reduced { "reduced" } else { "normal" },
        if state.primitive { " + primitive" } else { "" },
        if state.paused { "paused" } else { "running" },
        state.presentation_identity,
        state.radius,
        state.capture_dir.display(),
        embedded_vfx_catalog_fingerprint(),
        effects.iter().count(),
        fixtures.iter().count(),
        cameras.iter().count(),
        dimensions,
    );
    for mut inspector in &mut inspector {
        (**inspector).clone_from(&text);
    }
}

const fn fixture_index(fixture: VfxLabFixture) -> u8 {
    match fixture {
        VfxLabFixture::Transient => 1,
        VfxLabFixture::StraightProjectile => 2,
        VfxLabFixture::LobbedProjectile => 3,
        VfxLabFixture::ElementalField => 4,
        VfxLabFixture::ConeSpray => 5,
        VfxLabFixture::Status => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_rejects_undersized_windows_and_oversized_keys() {
        assert!(
            VfxLabConfig {
                window_size: Some((639, 480)),
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            VfxLabConfig {
                effect_key: Some("x".repeat(65)),
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            VfxLabConfig {
                fixture: Some("unknown".to_string()),
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            VfxLabConfig {
                presentation_identity: Some(0),
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            VfxLabConfig {
                radius: Some(257),
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn scenario_selection_is_bounded_and_replay_identity_is_explicit() {
        let scenario = VfxLabScenario {
            key: VfxRequestKey::new("combat.muzzle"),
            producer_rank: 100,
            capabilities: VfxRequestCapabilities::NONE,
        };
        let mut state = VfxLabState {
            scenarios: vec![scenario, scenario],
            selected: 0,
            presentation_identity: 7,
            radius: DEFAULT_RADIUS,
            reduced: false,
            primitive: false,
            paused: false,
            step_once: false,
            capture_dir: PathBuf::from("target/vfx-lab"),
            capture_sequence: 0,
            fixture: VfxLabFixture::Transient,
            playback_tick: 30,
            tick_accumulator: Duration::ZERO,
            capture_after_updates: None,
            update_index: 0,
            exit_after_capture: false,
            pending_capture_paths: None,
        };
        state.select_relative(-1);
        assert_eq!(state.selected, 0);
        state.select_relative(9);
        assert_eq!(state.selected, 1);
        assert_eq!(state.transient_request().unwrap().order.event_id, 7);
        assert_eq!(state.transient_request().unwrap().order.event_id, 7);
        state.use_next_identity();
        assert_eq!(state.transient_request().unwrap().order.event_id, 8);
    }

    #[test]
    fn fixture_names_and_cycle_are_bounded() {
        let expected = [
            ("transient", VfxLabFixture::Transient),
            ("straight", VfxLabFixture::StraightProjectile),
            ("lobbed", VfxLabFixture::LobbedProjectile),
            ("field", VfxLabFixture::ElementalField),
            ("cone", VfxLabFixture::ConeSpray),
            ("status", VfxLabFixture::Status),
        ];
        for (name, fixture) in expected {
            assert_eq!(VfxLabFixture::from_key(name), Some(fixture));
        }

        let mut fixture = VfxLabFixture::Transient;
        for _ in 0..VfxLabFixture::COUNT {
            fixture = fixture.next();
        }
        assert_eq!(fixture, VfxLabFixture::Transient);
    }
}
