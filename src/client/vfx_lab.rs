//! Development-only read-only viewer over the production VFX registry and materializer.

use super::{
    ArenaCamera,
    evidence_capture::{
        ClientEvidenceCapturePlugin, current_unix_millis, request_custom_evidence_capture,
    },
    hud::ClientHeistReadiness,
    presentation_3d::{
        CombatEffect3d, ImportedWorldFallbackPolicy, MAX_ACTIVE_PARTICLE_EMITTERS,
        Material3dAssets, PendingCombatEffect, Primitive3dAssets,
        VfxLabLifecyclePresentationPlugin, VfxLabLifecycleSet, VfxParticleAdapterPlugin,
        VfxParticleDiagnostics, VfxParticleEmitter3d, VfxTransientDiagnostics,
        animate_combat_effect_layers, collect_vfx_evidence_snapshot, materialize_combat_effects,
        resolve_vfx_requests, setup_3d_foundation,
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
use bevy_hanabi::{EffectSimulation, EffectSimulationTime};
use core::time::Duration;
use lightyear::prelude::Controlled;
use serde_json::json;
use std::path::PathBuf;

const DEFAULT_RADIUS: u16 = 72;
const MAX_INITIAL_SAMPLE_TICKS: u16 = 600;
const VFX_LAB_DASH_START: Vec2 = Vec2::new(-70.0, 0.0);
const VFX_LAB_DASH_END: Vec2 = Vec2::new(290.0, 0.0);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VfxLabAccent {
    #[default]
    Neutral,
    Cold,
    Fire,
    Poison,
    Restoration,
}

impl VfxLabAccent {
    #[must_use]
    pub fn from_key(value: &str) -> Option<Self> {
        match value {
            "neutral" => Some(Self::Neutral),
            "cold" => Some(Self::Cold),
            "fire" => Some(Self::Fire),
            "poison" => Some(Self::Poison),
            "restoration" => Some(Self::Restoration),
            _ => None,
        }
    }

    const fn next(self) -> Self {
        match self {
            Self::Neutral => Self::Cold,
            Self::Cold => Self::Fire,
            Self::Fire => Self::Poison,
            Self::Poison => Self::Restoration,
            Self::Restoration => Self::Neutral,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Cold => "cold",
            Self::Fire => "fire",
            Self::Poison => "poison",
            Self::Restoration => "restoration",
        }
    }

    const fn signature_accent(self) -> crate::client::vfx::VfxAccent {
        match self {
            Self::Neutral => crate::client::vfx::VfxAccent::Neutral,
            Self::Cold => crate::client::vfx::VfxAccent::Cold,
            Self::Fire => crate::client::vfx::VfxAccent::Fire,
            Self::Poison => crate::client::vfx::VfxAccent::Poison,
            Self::Restoration => crate::client::vfx::VfxAccent::Restoration,
        }
    }
}

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
    pub sample_ticks: Option<u16>,
    pub accent: VfxLabAccent,
    pub lobbed_impact: bool,
    pub ultimate: bool,
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
                "VfxLab fixture must be transient, straight, melee, lobbed, sticky, splash, field, cone, dash, sentry, or status"
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
        if self
            .sample_ticks
            .is_some_and(|ticks| ticks > MAX_INITIAL_SAMPLE_TICKS)
        {
            return Err(format!(
                "VfxLab initial sample must be at most {MAX_INITIAL_SAMPLE_TICKS} ticks"
            ));
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
    Melee,
    LobbedProjectile,
    StickyPayload,
    PersistentSplash,
    ElementalField,
    ConcealmentField,
    ConeSpray,
    Dash,
    Sentry,
    Status,
}

impl VfxLabFixture {
    const COUNT: u8 = 12;

    fn next(self) -> Self {
        match self {
            Self::Transient => Self::StraightProjectile,
            Self::StraightProjectile => Self::Melee,
            Self::Melee => Self::LobbedProjectile,
            Self::LobbedProjectile => Self::StickyPayload,
            Self::StickyPayload => Self::PersistentSplash,
            Self::PersistentSplash => Self::ElementalField,
            Self::ElementalField => Self::ConcealmentField,
            Self::ConcealmentField => Self::ConeSpray,
            Self::ConeSpray => Self::Dash,
            Self::Dash => Self::Sentry,
            Self::Sentry => Self::Status,
            Self::Status => Self::Transient,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Transient => "transient recipe",
            Self::StraightProjectile => "straight projectile / line",
            Self::Melee => "melee / authored sweep and contact",
            Self::LobbedProjectile => "lobbed projectile / arc",
            Self::StickyPayload => "armed sticky payload / fuse",
            Self::PersistentSplash => "persistent Splash / dual-purpose fluid area",
            Self::ElementalField => "elemental field / area",
            Self::ConcealmentField => "concealment field / veil area",
            Self::ConeSpray => "cone spray / propagation",
            Self::Dash => "dash / retained wake",
            Self::Sentry => "sentry / retained deployable",
            Self::Status => "fighter status / attached",
        }
    }

    fn from_key(value: &str) -> Option<Self> {
        match value {
            "transient" => Some(Self::Transient),
            "straight" => Some(Self::StraightProjectile),
            "melee" => Some(Self::Melee),
            "lobbed" => Some(Self::LobbedProjectile),
            "sticky" => Some(Self::StickyPayload),
            "splash" => Some(Self::PersistentSplash),
            "field" => Some(Self::ElementalField),
            "concealment" => Some(Self::ConcealmentField),
            "cone" => Some(Self::ConeSpray),
            "dash" => Some(Self::Dash),
            "sentry" => Some(Self::Sentry),
            "status" => Some(Self::Status),
            _ => None,
        }
    }
}

const fn fixture_has_fixed_accent(fixture: VfxLabFixture) -> bool {
    matches!(
        fixture,
        VfxLabFixture::Melee
            | VfxLabFixture::PersistentSplash
            | VfxLabFixture::ConcealmentField
            | VfxLabFixture::Sentry
    )
}

impl VfxLabScenario {
    fn request(
        self,
        event_id: u64,
        radius: u16,
        accent: VfxLabAccent,
        lobbed_impact: bool,
        ultimate: bool,
    ) -> VfxRequest {
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
        .and_then(|request| {
            let mut signature = request.signature;
            signature.identity = crate::client::vfx::VfxPresentationIdentity::Explicit(event_id);
            signature.accents = [Some(accent.signature_accent()), None];
            if matches!(
                self.key,
                crate::client::vfx::REVEAL_SCAN_VFX | crate::client::vfx::DEMOLITION_STRIKE_VFX
            ) {
                signature.carrier = crate::client::vfx::VfxCarrier::PersistentArea;
                signature.phase = crate::client::vfx::VfxPhase::Activation;
                signature.geometry =
                    crate::client::vfx::VfxGeometry::circle(request.position, f32::from(radius));
            }
            if self.key == crate::client::vfx::REVEAL_SCAN_VFX {
                signature.accents = [Some(crate::client::vfx::VfxAccent::Reveal), None];
            }
            if self.key == crate::client::vfx::DEMOLITION_STRIKE_VFX {
                signature.accents = [Some(crate::client::vfx::VfxAccent::Demolition), None];
            }
            apply_attached_transient_signature(self.key, ultimate, &mut signature);
            if self.key == crate::client::vfx::COMBAT_MUZZLE_VFX {
                signature.carrier = crate::client::vfx::VfxCarrier::StraightProjectile;
                signature.phase = crate::client::vfx::VfxPhase::Activation;
            }
            if self.key == crate::client::vfx::COMBAT_IMPACT_VFX {
                signature.carrier = crate::client::vfx::VfxCarrier::StraightProjectile;
                signature.phase = crate::client::vfx::VfxPhase::Impact;
                if lobbed_impact {
                    signature.carrier = crate::client::vfx::VfxCarrier::LobbedProjectile;
                    signature.geometry = crate::client::vfx::VfxGeometry::circle(
                        request.position,
                        f32::from(radius),
                    );
                }
            }
            signature.context = if matches!(
                self.key,
                crate::client::vfx::SELF_CLOAK_VFX
                    | crate::client::vfx::FORCED_REVEAL_VFX
                    | crate::client::vfx::COMBAT_DAMAGE_VFX
                    | crate::client::vfx::COMBAT_RESET_VFX
            ) {
                None
            } else if self.key == crate::client::vfx::REVEAL_SCAN_VFX {
                Some(crate::client::vfx::VfxContextOverride::RevealScan)
            } else if self.key == crate::client::vfx::DEMOLITION_STRIKE_VFX {
                Some(crate::client::vfx::VfxContextOverride::DemolitionStrike)
            } else if self.key == crate::client::vfx::ELEMENTAL_FIELD_VFX {
                Some(crate::client::vfx::VfxContextOverride::ElementalField(
                    match accent {
                        VfxLabAccent::Cold => crate::combat::ElementalFieldKind::Cryogenic,
                        VfxLabAccent::Poison => crate::combat::ElementalFieldKind::Poison,
                        VfxLabAccent::Restoration => crate::combat::ElementalFieldKind::Restoration,
                        VfxLabAccent::Fire | VfxLabAccent::Neutral => {
                            crate::combat::ElementalFieldKind::Fire
                        }
                    },
                ))
            } else if ultimate {
                Some(crate::client::vfx::VfxContextOverride::Ultimate(
                    crate::builds::UltimateDefinitionId(1),
                ))
            } else {
                None
            };
            request.with_signature(signature)
        })
        .expect("sealed production registration produces a valid lab request")
    }
}

fn apply_attached_transient_signature(
    key: VfxRequestKey,
    alternate: bool,
    signature: &mut crate::client::vfx::CombatVisualSignature,
) {
    let (phase, accent) = if key == crate::client::vfx::SELF_CLOAK_VFX {
        (
            if alternate {
                crate::client::vfx::VfxPhase::Interruption
            } else {
                crate::client::vfx::VfxPhase::Activation
            },
            crate::client::vfx::VfxAccent::Concealment,
        )
    } else if key == crate::client::vfx::FORCED_REVEAL_VFX {
        (
            crate::client::vfx::VfxPhase::Activation,
            crate::client::vfx::VfxAccent::Reveal,
        )
    } else if key == crate::client::vfx::COMBAT_DAMAGE_VFX {
        (
            if alternate {
                crate::client::vfx::VfxPhase::Interruption
            } else {
                crate::client::vfx::VfxPhase::Impact
            },
            crate::client::vfx::VfxAccent::Neutral,
        )
    } else if key == crate::client::vfx::COMBAT_RESET_VFX {
        (
            crate::client::vfx::VfxPhase::Expiry,
            crate::client::vfx::VfxAccent::Neutral,
        )
    } else {
        return;
    };
    signature.carrier = crate::client::vfx::VfxCarrier::AttachedStatus;
    signature.phase = phase;
    signature.accents = [Some(accent), None];
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
    accent: VfxLabAccent,
    lobbed_impact: bool,
    ultimate: bool,
    reduced: bool,
    primitive: bool,
    show_diagnostics: bool,
    paused: bool,
    step_once: bool,
    capture_dir: PathBuf,
    capture_sequence: u32,
    fixture: VfxLabFixture,
    playback_tick: u64,
    initial_sample_ticks: u16,
    pending_sample_ticks: u16,
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
            self.selected().request(
                self.presentation_identity,
                self.radius,
                self.accent,
                self.lobbed_impact,
                self.ultimate,
            )
        })
    }

    fn use_next_identity(&mut self) {
        self.presentation_identity = self.presentation_identity.saturating_add(1);
    }

    fn restart_playback(&mut self) {
        self.playback_tick = 30;
        self.pending_sample_ticks = self.initial_sample_ticks;
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

#[derive(Component)]
struct VfxLabDashFixture {
    started_at_tick: u64,
    ends_at_tick: u64,
}

#[derive(Component)]
struct VfxLabReference(u8);

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
        .init_resource::<VfxTransientDiagnostics>()
        .add_systems(Startup, setup_3d_foundation)
        .add_systems(Startup, setup_lab_scene.after(setup_3d_foundation))
        .add_systems(
            Update,
            (
                handle_lab_input,
                update_lab_reference_positions,
                sync_lab_particle_time,
                resolve_vfx_requests,
                materialize_combat_effects,
                advance_lab_effects,
                animate_combat_effect_layers,
                update_lab_fixture_facts,
                update_lab_dash_fixture,
                hide_gameplay_hud,
                update_lab_inspector,
                exit_after_lab_capture,
            )
                .chain(),
        )
        .configure_sets(
            Update,
            crate::client::vfx::VfxSignatureSet
                .after(handle_lab_input)
                .before(VfxLabLifecycleSet),
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
        let fixture = self
            .config
            .fixture
            .as_deref()
            .and_then(VfxLabFixture::from_key)
            .unwrap_or_default();
        app.insert_resource(VfxLabState {
            scenarios,
            selected,
            presentation_identity: self.config.presentation_identity.unwrap_or(1),
            radius: self.config.radius.unwrap_or(DEFAULT_RADIUS),
            accent: if fixture_has_fixed_accent(fixture) {
                VfxLabAccent::Neutral
            } else {
                self.config.accent
            },
            lobbed_impact: self.config.lobbed_impact,
            ultimate: self.config.ultimate,
            reduced: self.config.reduced,
            primitive: self.config.primitive,
            show_diagnostics: false,
            paused: self.config.paused,
            step_once: false,
            capture_dir: self
                .config
                .capture_dir
                .clone()
                .unwrap_or_else(|| PathBuf::from("target/vfx-lab")),
            capture_sequence: 0,
            fixture,
            playback_tick: 30,
            initial_sample_ticks: self.config.sample_ticks.unwrap_or_default(),
            pending_sample_ticks: self.config.sample_ticks.unwrap_or_default(),
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
    let initially_paused = config.paused;
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
    .add_plugins(VfxParticleAdapterPlugin)
    .add_plugins(VfxLabLifecyclePresentationPlugin)
    .add_plugins(ClientEvidenceCapturePlugin)
    .add_plugins(VfxLabPlugin { config });
    if initially_paused {
        app.world_mut()
            .resource_mut::<Time<EffectSimulation>>()
            .pause();
    }
    app
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "resources are Bevy system parameters owned by the scheduler"
)]
fn sync_lab_particle_time(
    state: Res<VfxLabState>,
    mut particle_time: ResMut<Time<EffectSimulation>>,
) {
    if state.paused && !state.step_once && state.pending_sample_ticks == 0 {
        particle_time.pause();
    } else {
        particle_time.unpause();
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "resources are Bevy system parameters owned by the scheduler"
)]
fn setup_lab_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    primitives: Res<Primitive3dAssets>,
    presentation_materials: Res<Material3dAssets>,
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
    let reference_positions = lab_reference_positions(state.fixture);
    for (index, x) in reference_positions.into_iter().enumerate() {
        commands.spawn((
            VfxLabReference(u8::try_from(index).expect("two reference indices fit u8")),
            Mesh3d(primitives.fighter.clone()),
            MeshMaterial3d(presentation_materials.neutral.clone()),
            Transform::from_xyz(x, 14.0, 0.0),
            Name::new(format!("VfxLab fighter-size reference {}", index + 1)),
        ));
    }
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

#[allow(
    clippy::needless_pass_by_value,
    reason = "the read-only state is a Bevy resource system parameter"
)]
fn update_lab_reference_positions(
    state: Res<VfxLabState>,
    mut references: Query<(&VfxLabReference, &mut Transform)>,
) {
    let positions = lab_reference_positions(state.fixture);
    for (reference, mut transform) in &mut references {
        transform.translation.x = positions[usize::from(reference.0)];
    }
}

const fn lab_reference_positions(fixture: VfxLabFixture) -> [f32; 2] {
    match fixture {
        VfxLabFixture::Transient | VfxLabFixture::StickyPayload => [-64.0, 96.0],
        VfxLabFixture::LobbedProjectile => [-80.0, 80.0],
        VfxLabFixture::Melee => [0.0, 108.0],
        VfxLabFixture::StraightProjectile
        | VfxLabFixture::ElementalField
        | VfxLabFixture::ConcealmentField
        | VfxLabFixture::PersistentSplash
        | VfxLabFixture::ConeSpray
        | VfxLabFixture::Status => [0.0, 96.0],
        VfxLabFixture::Sentry => [-120.0, 210.0],
        VfxLabFixture::Dash => [-250.0, 380.0],
    }
}

fn spawn_lab_scenario(
    commands: &mut Commands,
    state: &VfxLabState,
    requests: &mut MessageWriter<VfxRequest>,
) {
    if state.fixture == VfxLabFixture::Melee {
        spawn_melee_fixture(state, requests);
        return;
    }
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
    if matches!(
        state.fixture,
        VfxLabFixture::StraightProjectile
            | VfxLabFixture::LobbedProjectile
            | VfxLabFixture::ConeSpray
    ) {
        spawn_lab_style_owner(
            commands,
            state.accent,
            state.presentation_identity,
            source.owner_network_entity_id,
        );
    }
    match state.fixture {
        VfxLabFixture::Transient => unreachable!("transient scenario returned above"),
        VfxLabFixture::StraightProjectile => spawn_straight_fixture(commands, source, tick),
        VfxLabFixture::Melee => unreachable!("melee requests return before lifecycle fixtures"),
        VfxLabFixture::LobbedProjectile => spawn_lobbed_fixture(commands, source, tick),
        VfxLabFixture::StickyPayload => spawn_sticky_fixture(
            commands,
            state.presentation_identity,
            state.radius,
            state.ultimate,
            tick,
        ),
        VfxLabFixture::PersistentSplash => {
            spawn_splash_fixture(commands, state.presentation_identity, state.radius, tick);
        }
        VfxLabFixture::ElementalField => {
            spawn_field_fixture(
                commands,
                state.presentation_identity,
                state.radius,
                state.accent,
                tick,
            );
        }
        VfxLabFixture::ConcealmentField => {
            spawn_concealment_fixture(commands, state.presentation_identity, state.radius, tick);
        }
        VfxLabFixture::ConeSpray => spawn_cone_fixture(commands, state.presentation_identity, tick),
        VfxLabFixture::Dash => spawn_dash_fixture(commands, state.presentation_identity, tick),
        VfxLabFixture::Sentry => {
            spawn_sentry_fixture(commands, state.presentation_identity, tick);
            spawn_sentry_transients(state, requests);
        }
        VfxLabFixture::Status => {
            spawn_status_fixture(
                commands,
                state.presentation_identity,
                state.accent,
                state.ultimate,
                tick,
            );
        }
    }
}

fn spawn_sticky_fixture(
    commands: &mut Commands,
    identity: u64,
    radius: u16,
    ultimate: bool,
    tick: u64,
) {
    commands.spawn((
        VfxLabFixtureOwner,
        Position::from_xy(0.0, 0.0),
        crate::combat::StickyBlobState {
            kind: if ultimate {
                crate::combat::StickyBlobKind::UltimateSecondary
            } else {
                crate::combat::StickyBlobKind::Primary
            },
            attached_to: None,
            armed_at_tick: tick,
            detonates_at_tick: tick.saturating_add(180),
            explosion_radius: f32::from(radius),
        },
        Name::new(format!("VfxLab sticky payload fixture {identity}")),
    ));
}

fn spawn_melee_fixture(state: &VfxLabState, requests: &mut MessageWriter<VfxRequest>) {
    const REACH: f32 = 120.0;
    const ANGLE_DEGREES: f32 = 100.0;
    let origin = Vec2::ZERO;
    let contact = Vec2::new(108.0, 0.0);
    let scenario = |key| {
        state
            .scenarios
            .iter()
            .copied()
            .find(|scenario| scenario.key == key)
            .expect("production VFX registry contains the melee transient key")
    };

    let mut swing = scenario(crate::client::vfx::COMBAT_MUZZLE_VFX).request(
        state.presentation_identity,
        state.radius,
        VfxLabAccent::Neutral,
        false,
        false,
    );
    swing.position = origin;
    swing.signature.carrier = crate::client::vfx::VfxCarrier::Melee;
    swing.signature.phase = crate::client::vfx::VfxPhase::Activation;
    swing.signature.geometry = crate::client::vfx::VfxGeometry::Cone {
        origin: origin.to_array(),
        facing: 0.0,
        reach: REACH,
        angle_degrees: ANGLE_DEGREES,
    };
    swing.signature.timing =
        crate::client::vfx::VfxAuthoritativeTiming::instant(state.playback_tick);
    requests.write(swing);

    let mut impact = scenario(crate::client::vfx::COMBAT_IMPACT_VFX).request(
        state.presentation_identity.saturating_add(1),
        state.radius,
        VfxLabAccent::Neutral,
        false,
        false,
    );
    impact.position = contact;
    impact.signature.carrier = crate::client::vfx::VfxCarrier::Melee;
    impact.signature.phase = crate::client::vfx::VfxPhase::Impact;
    impact.signature.geometry = crate::client::vfx::VfxGeometry::point(contact);
    impact.signature.timing =
        crate::client::vfx::VfxAuthoritativeTiming::instant(state.playback_tick);
    requests.write(impact);
}

fn spawn_straight_fixture(
    commands: &mut Commands,
    source: crate::combat::ProjectileSource,
    tick: u64,
) {
    let attack = lab_attack_source(source, Vec2::new(-160.0, 0.0));
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
        crate::combat::ProjectileDeadline {
            expires_at_tick: tick.saturating_add(120),
        },
        crate::combat::ReplicatedAttackSource { attack },
        crate::combat::AttackDelivery {
            attack_id: attack.attack_id,
            delivery_index: 0,
        },
        Name::new("VfxLab straight projectile fixture"),
    ));
}

fn spawn_lobbed_fixture(
    commands: &mut Commands,
    source: crate::combat::ProjectileSource,
    tick: u64,
) {
    let source = crate::combat::ProjectileSource {
        weapon_definition_id: crate::combat::ARC_LAUNCHER_DEFINITION,
        ..source
    };
    let attack = lab_attack_source(source, Vec2::new(-140.0, 0.0));
    commands.spawn((
        VfxLabFixtureOwner,
        crate::combat::Projectile,
        source,
        Position::from_xy(-140.0, 0.0),
        Rotation::radians(0.0),
        crate::combat::LobbedFlight {
            launch: crate::combat::WorldPoint::from(Vec2::new(-140.0, 0.0)),
            landing: crate::combat::WorldPoint::from(Vec2::new(140.0, 0.0)),
            launched_at_tick: tick,
            lands_at_tick: tick.saturating_add(90),
            visual_arc_height: 120.0,
        },
        crate::combat::ReplicatedAttackSource { attack },
        crate::combat::AttackDelivery {
            attack_id: attack.attack_id,
            delivery_index: 0,
        },
        Name::new("VfxLab lobbed projectile fixture"),
    ));
}

fn spawn_field_fixture(
    commands: &mut Commands,
    identity: u64,
    radius: u16,
    accent: VfxLabAccent,
    tick: u64,
) {
    commands.spawn((
        VfxLabFixtureOwner,
        crate::combat::ElementalFieldState {
            id: crate::combat::ElementalFieldId(identity),
            kind: match accent {
                VfxLabAccent::Cold => crate::combat::ElementalFieldKind::Cryogenic,
                VfxLabAccent::Poison => crate::combat::ElementalFieldKind::Poison,
                VfxLabAccent::Restoration => crate::combat::ElementalFieldKind::Restoration,
                VfxLabAccent::Neutral | VfxLabAccent::Fire => {
                    crate::combat::ElementalFieldKind::Fire
                }
            },
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

fn spawn_splash_fixture(commands: &mut Commands, identity: u64, radius: u16, tick: u64) {
    commands.spawn((
        VfxLabFixtureOwner,
        crate::combat::PersistentSplash,
        crate::combat::PersistentSplashState {
            center: crate::combat::WorldPoint::from(Vec2::ZERO),
            facing: 0.0,
            shape: crate::combat::PersistentAreaShape::Circle {
                radius: f32::from(radius),
            },
            activated_at_tick: tick,
            next_pulse_tick: tick.saturating_add(30),
            expires_at_tick: tick.saturating_add(240),
            pulse_interval_ticks: 30,
            map_occlusion: true,
            max_targets: 6,
            effects: [
                Some(crate::combat::PayloadEffectDefinition::Damage {
                    amount: 36,
                    falloff: crate::combat::DamageFalloff::None,
                    recipients: crate::combat::RecipientPolicy::Hostiles,
                }),
                Some(crate::combat::PayloadEffectDefinition::Heal {
                    amount: 24,
                    recipients: crate::combat::RecipientPolicy::AlliesAndOwner,
                }),
            ],
        },
        Name::new(format!("VfxLab Persistent Splash fixture {identity}")),
    ));
}

fn spawn_cone_fixture(commands: &mut Commands, identity: u64, tick: u64) {
    let source = crate::combat::ProjectileSource {
        shot_id: crate::combat::ShotId(identity),
        player_id: crate::protocol::PlayerId(1),
        owner_network_entity_id: crate::protocol::NetworkEntityId(1),
        team_id: crate::combat::TeamId(0),
        weapon_definition_id: crate::combat::SPRAY_DEFINITION,
    };
    let attack = lab_attack_source(source, Vec2::new(-120.0, 0.0));
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
        crate::combat::ReplicatedAttackSource { attack },
        crate::combat::AttackDelivery {
            attack_id: attack.attack_id,
            delivery_index: 0,
        },
        Name::new("VfxLab cone spray fixture"),
    ));
}

fn spawn_concealment_fixture(commands: &mut Commands, identity: u64, radius: u16, tick: u64) {
    commands.spawn((
        VfxLabFixtureOwner,
        VfxLabClock,
        AuthoritativeTick(tick),
        crate::concealment::ConcealmentFieldState {
            id: crate::concealment::ConcealmentFieldId(identity),
            team: crate::combat::TeamId(0),
            center: crate::combat::WorldPoint::from(Vec2::ZERO),
            radius_milliunits: u32::from(radius).saturating_mul(1_000),
            activated_at_tick: tick,
            expires_at_tick: tick.saturating_add(600),
        },
        Name::new("VfxLab replicated Concealment Field fixture"),
    ));
}

fn spawn_status_fixture(
    commands: &mut Commands,
    identity: u64,
    accent: VfxLabAccent,
    alternate: bool,
    tick: u64,
) {
    let expires_at_tick = tick.saturating_add(300);
    let source = crate::combat::ConditionSource {
        action_id: crate::combat::AttackId(identity),
        kind: crate::combat::CombatSourceKind::PrimaryWeapon,
        player_id: crate::protocol::PlayerId(1),
        network_entity_id: crate::protocol::NetworkEntityId(1),
        team_id: crate::combat::TeamId(0),
        source_preset_id: None,
        recipe_fingerprint: None,
    };
    let mut fixture = commands.spawn((
        VfxLabFixtureOwner,
        VfxLabClock,
        crate::protocol::Fighter,
        Controlled,
        crate::protocol::NetworkEntityId(1),
        crate::combat::TeamId(0),
        Position::from_xy(0.0, 0.0),
        Rotation::radians(0.0),
        AuthoritativeTick(tick),
        Name::new("VfxLab attached status fixture"),
    ));
    match (accent, alternate) {
        (VfxLabAccent::Neutral, false) => {
            fixture.insert(crate::combat::ActiveEffects {
                slow: Some(crate::combat::SlowEffect {
                    source_attack_id: crate::combat::AttackId(identity),
                    source_network_entity_id: crate::protocol::NetworkEntityId(1),
                    movement_multiplier_milli: 700,
                    expires_at_tick,
                }),
                ..Default::default()
            });
        }
        (VfxLabAccent::Neutral, true) => {
            fixture.insert(crate::combat::KnockbackFeedback {
                velocity: crate::combat::WorldPoint::from(Vec2::new(220.0, 0.0)),
                expires_at_tick,
            });
        }
        (VfxLabAccent::Cold, frozen) => {
            fixture.insert(crate::combat::ActiveEffects {
                cold: crate::combat::ColdState {
                    meter: if frozen { 1_000 } else { 500 },
                    last_contribution_tick: tick,
                    frozen_until_tick: frozen.then_some(expires_at_tick),
                    source: Some(source),
                    ..Default::default()
                },
                ..Default::default()
            });
        }
        (VfxLabAccent::Fire, _) => {
            fixture.insert(crate::combat::ActiveEffects {
                fire: Some(lab_damage_over_time(source, tick, expires_at_tick)),
                ..Default::default()
            });
        }
        (VfxLabAccent::Poison, _) => {
            fixture.insert(crate::combat::ActiveEffects {
                poison: Some(lab_damage_over_time(source, tick, expires_at_tick)),
                ..Default::default()
            });
        }
        (VfxLabAccent::Restoration, _) => {
            fixture.insert(crate::concealment::ConcealmentPresentationState {
                forced_reveals: vec![crate::concealment::TeamRevealDeadline {
                    team: crate::combat::TeamId(0),
                    expires_at_tick,
                }],
                ..Default::default()
            });
        }
    }
}

const fn lab_damage_over_time(
    source: crate::combat::ConditionSource,
    tick: u64,
    expires_at_tick: u64,
) -> crate::combat::DamageOverTime {
    crate::combat::DamageOverTime {
        source,
        damage_per_tick: 8,
        tick_interval: 30,
        next_tick: tick.saturating_add(30),
        expires_at_tick,
    }
}

fn spawn_dash_fixture(commands: &mut Commands, identity: u64, tick: u64) {
    let ends_at_tick = tick.saturating_add(18);
    commands.spawn((
        VfxLabFixtureOwner,
        VfxLabClock,
        VfxLabDashFixture {
            started_at_tick: tick,
            ends_at_tick,
        },
        crate::protocol::Fighter,
        Controlled,
        crate::protocol::NetworkEntityId(1),
        crate::combat::TeamId(0),
        Position(VFX_LAB_DASH_START),
        Rotation::radians(0.0),
        AuthoritativeTick(tick),
        lab_style_loadout(VfxLabAccent::Neutral, identity),
        crate::builds::AbilityState {
            charge: 0,
            phase: crate::builds::AbilityPhase::Dashing { ends_at_tick },
        },
        Name::new("VfxLab Dash lifecycle fixture"),
    ));
}

fn spawn_sentry_fixture(commands: &mut Commands, identity: u64, tick: u64) {
    commands.spawn((
        VfxLabFixtureOwner,
        VfxLabClock,
        crate::abilities::Sentry,
        crate::abilities::SentryIdentity {
            deployable_id: crate::builds::DeployableId(identity),
            owner_player_id: crate::protocol::PlayerId(1),
            owner_network_id: crate::protocol::NetworkEntityId(1),
            team_id: crate::combat::TeamId(0),
            ultimate_id: crate::builds::UltimateDefinitionId(2),
            match_id: crate::matchplay::MatchId(1),
        },
        crate::abilities::SentryDeadline {
            expires_at_tick: tick.saturating_add(720),
        },
        crate::combat::TeamId(0),
        Position::from_xy(0.0, 0.0),
        Rotation::radians(0.0),
        AuthoritativeTick(tick),
        Name::new("VfxLab replicated Sentry fixture"),
    ));
}

fn spawn_sentry_transients(state: &VfxLabState, requests: &mut MessageWriter<VfxRequest>) {
    let scenario = |key| {
        state
            .scenarios
            .iter()
            .copied()
            .find(|scenario| scenario.key == key)
            .expect("production VFX registry contains the Sentry transient key")
    };
    let signature = |position, phase, identity| crate::client::vfx::CombatVisualSignature {
        carrier: crate::client::vfx::VfxCarrier::Deployable,
        phase,
        accents: [Some(crate::client::vfx::VfxAccent::Neutral), None],
        relationship: crate::client::vfx::VfxRelationship::SelfOwned,
        context: Some(crate::client::vfx::VfxContextOverride::Sentry),
        geometry: crate::client::vfx::VfxGeometry::point(position),
        identity: crate::client::vfx::VfxPresentationIdentity::Deployable(identity),
        timing: crate::client::vfx::VfxAuthoritativeTiming::instant(state.playback_tick),
        variant: crate::client::vfx::VfxPresentationVariant::Normal,
    };

    let fire_position = Vec2::new(crate::client::vfx::SENTRY_MUZZLE_PRESENTATION_OFFSET, 0.0);
    let mut fire = scenario(crate::client::vfx::COMBAT_MUZZLE_VFX).request(
        state.presentation_identity,
        state.radius,
        VfxLabAccent::Neutral,
        false,
        true,
    );
    fire.position = fire_position;
    fire.signature = signature(
        fire_position,
        crate::client::vfx::VfxPhase::Activation,
        state.presentation_identity,
    );
    requests.write(fire);

    let removal_position = Vec2::new(145.0, 0.0);
    let removal_identity = state.presentation_identity.saturating_add(1);
    let removal_phase = if state.ultimate {
        crate::client::vfx::VfxPhase::Interruption
    } else {
        crate::client::vfx::VfxPhase::Expiry
    };
    let mut removal = scenario(crate::client::vfx::COMBAT_IMPACT_VFX).request(
        removal_identity,
        state.radius,
        VfxLabAccent::Neutral,
        false,
        true,
    );
    removal.position = removal_position;
    removal.signature = signature(removal_position, removal_phase, removal_identity);
    requests.write(removal);
}

fn lab_attack_source(
    source: crate::combat::ProjectileSource,
    origin: Vec2,
) -> crate::combat::AttackSource {
    crate::combat::AttackSource {
        kind: crate::combat::CombatSourceKind::PrimaryWeapon,
        attack_id: crate::combat::AttackId(source.shot_id.0),
        player_id: source.player_id,
        owner_network_entity_id: source.owner_network_entity_id,
        team_id: source.team_id,
        recipe_fingerprint: crate::combat::WeaponRecipeFingerprint(source.shot_id.0),
        legacy_compatibility: false,
        source_preset_id: None,
        origin: crate::combat::WorldPoint::from(origin),
        facing: 0.0,
    }
}

fn spawn_lab_style_owner(
    commands: &mut Commands,
    accent: VfxLabAccent,
    fingerprint: u64,
    network_id: crate::protocol::NetworkEntityId,
) {
    commands.spawn((
        VfxLabFixtureOwner,
        crate::protocol::Fighter,
        network_id,
        crate::combat::TeamId(0),
        lab_style_loadout(accent, fingerprint),
        Name::new("VfxLab signature-only loadout owner"),
    ));
}

fn lab_style_loadout(
    accent: VfxLabAccent,
    fingerprint: u64,
) -> crate::builds::ResolvedMatchLoadout {
    let mut loadout = crate::builds::resolve_build_recipe(
        &crate::builds::BuildCatalog::embedded().expect("embedded build catalog is valid"),
        &crate::combat::WeaponCatalog::embedded().expect("embedded weapon catalog is valid"),
        crate::builds::BrawlerBuildRecipe {
            weapon: crate::builds::WeaponChoice::Preset(crate::combat::WeaponPresetId(1)),
            ultimate: crate::builds::UltimateDefinitionId(1),
            passives: [
                crate::builds::PassiveDefinitionId(1),
                crate::builds::PassiveDefinitionId(6),
            ],
        },
    )
    .expect("built-in VfxLab loadout resolves");
    loadout.primary_weapon.recipe_fingerprint = crate::combat::WeaponRecipeFingerprint(fingerprint);
    loadout.primary_weapon.recipe.payload_bundles[0].effects = vec![match accent {
        VfxLabAccent::Cold => crate::combat::PayloadEffectDefinition::Cold {
            amount: 10,
            recipients: crate::combat::RecipientPolicy::Hostiles,
        },
        VfxLabAccent::Fire => crate::combat::PayloadEffectDefinition::DamageOverTime {
            kind: crate::combat::DamageOverTimeKind::Fire,
            damage_per_tick: 2,
            tick_interval: 10,
            duration_ticks: 30,
            recipients: crate::combat::RecipientPolicy::Hostiles,
        },
        VfxLabAccent::Poison => crate::combat::PayloadEffectDefinition::DamageOverTime {
            kind: crate::combat::DamageOverTimeKind::Poison,
            damage_per_tick: 2,
            tick_interval: 10,
            duration_ticks: 60,
            recipients: crate::combat::RecipientPolicy::Hostiles,
        },
        VfxLabAccent::Restoration => crate::combat::PayloadEffectDefinition::Heal {
            amount: 10,
            recipients: crate::combat::RecipientPolicy::Allies,
        },
        VfxLabAccent::Neutral => crate::combat::PayloadEffectDefinition::Damage {
            amount: 10,
            falloff: crate::combat::DamageFalloff::None,
            recipients: crate::combat::RecipientPolicy::Hostiles,
        },
    }];
    loadout
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
        if fixture_has_fixed_accent(state.fixture) {
            state.accent = VfxLabAccent::Neutral;
        }
        restart = true;
    }
    if keyboard.just_pressed(KeyCode::Space) {
        restart = true;
    }
    if keyboard.just_pressed(KeyCode::KeyC) && !state_has_fixed_accent(&state) {
        state.accent = state.accent.next();
        restart = true;
    }
    if keyboard.just_pressed(KeyCode::KeyU) {
        state.ultimate = !state.ultimate;
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
    if keyboard.just_pressed(KeyCode::KeyI) {
        state.show_diagnostics = !state.show_diagnostics;
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

fn state_has_fixed_accent(state: &VfxLabState) -> bool {
    fixture_has_fixed_accent(state.fixture)
        || (state.fixture == VfxLabFixture::Transient
            && matches!(
                state.selected().key,
                crate::client::vfx::REVEAL_SCAN_VFX
                    | crate::client::vfx::DEMOLITION_STRIKE_VFX
                    | crate::client::vfx::SELF_CLOAK_VFX
                    | crate::client::vfx::FORCED_REVEAL_VFX
            ))
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
    // Materialize retained lifecycle roots once at their activation facts before applying a
    // requested sample jump. This keeps origin-relative production visuals (notably Dash) honest.
    let delta = if state.pending_sample_ticks > 0 && state.update_index > 0 {
        let ticks = state.pending_sample_ticks;
        state.pending_sample_ticks = 0;
        crate::timing::SIMULATION_TICK.saturating_mul(u32::from(ticks))
    } else if state.step_once {
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

#[allow(
    clippy::cast_precision_loss,
    clippy::needless_pass_by_value,
    reason = "the bounded Dash fixture projects authoritative ticks into production movement facts"
)]
fn update_lab_dash_fixture(
    state: Res<VfxLabState>,
    mut fighters: Query<
        (
            &mut Position,
            &mut crate::builds::AbilityState,
            &VfxLabDashFixture,
        ),
        With<VfxLabFixtureOwner>,
    >,
) {
    for (mut position, mut ability, dash) in &mut fighters {
        let duration = dash
            .ends_at_tick
            .saturating_sub(dash.started_at_tick)
            .max(1);
        let elapsed = state
            .playback_tick
            .saturating_sub(dash.started_at_tick)
            .min(duration);
        let progress = elapsed as f32 / duration as f32;
        position.0 = VFX_LAB_DASH_START.lerp(VFX_LAB_DASH_END, progress);
        ability.phase = if state.playback_tick <= dash.ends_at_tick {
            crate::builds::AbilityPhase::Dashing {
                ends_at_tick: dash.ends_at_tick,
            }
        } else {
            crate::builds::AbilityPhase::Charging
        };
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

#[allow(
    clippy::too_many_lines,
    reason = "paired Lab capture keeps the selected style, recipe, lifecycle snapshot, and paths atomic"
)]
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
                state.accent,
                state.lobbed_impact,
                state.ultimate,
                state.reduced,
                state.primitive,
                state.paused,
                state.playback_tick,
            ))
        }
    };
    let Some((
        directory,
        sequence,
        scenario,
        fixture,
        identity,
        accent,
        lobbed_impact,
        ultimate,
        reduced,
        primitive,
        paused,
        tick,
    )) = capture
    else {
        return;
    };
    let radius = world.resource::<VfxLabState>().radius;
    let request = matches!(fixture, VfxLabFixture::Transient)
        .then(|| scenario.request(identity, radius, accent, lobbed_impact, ultimate));
    let recipe = request.as_ref().map(|request| {
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
            "accent": accent.label(),
            "lobbed_impact": lobbed_impact,
            "ultimate": ultimate,
            "radius": radius,
            "playback_tick": tick,
            "initial_sample_ticks": world.resource::<VfxLabState>().initial_sample_ticks,
            "authoritative_radius": request.as_ref().and_then(|request| request.authoritative_radius),
            "authoritative_deadline": request.as_ref().and_then(|request| request.deadline).map(|deadline| json!({
                "activated_at_tick": deadline.activated_at_tick,
                "expires_at_tick": deadline.expires_at_tick,
                "observed_at_tick": deadline.observed_at_tick,
            })),
            "reduced": reduced,
            "primitive": primitive,
            "paused": paused,
            "resolved_recipe": recipe,
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
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the read-only inspector presents one atomic view of lab, registry, signature, fixture, and camera state"
)]
fn update_lab_inspector(
    state: Res<VfxLabState>,
    registry: Res<VfxRegistry>,
    signature_cache: Res<crate::client::vfx::VfxSignatureCache>,
    effects: Query<(), With<CombatEffect3d>>,
    emitters: Query<(), With<VfxParticleEmitter3d>>,
    particle_diagnostics: Res<VfxParticleDiagnostics>,
    signatures: Query<(&crate::client::vfx::CombatVisualSignature, Option<&Name>)>,
    fixtures: Query<(), With<VfxLabFixtureOwner>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut inspector: Query<&mut Text, With<VfxLabInspector>>,
    cameras: Query<(), With<ArenaCamera>>,
) {
    let scenario = state.selected();
    let scenario_details = if let Some(request) = state.transient_request() {
        let recipe = registry
            .resolve(&request, state.reduced)
            .expect("production lab scenario resolves");
        let mut layers = recipe
            .layers
            .iter()
            .enumerate()
            .map(|(index, layer)| {
                let lifetime = match layer.timeline.duration {
                    VfxLifetime::Millis(value) => format!("{value} ms"),
                    VfxLifetime::AuthoritativeDeadline => "deadline".to_string(),
                };
                format!(
                    "{}. {} {:?}/{:?} {:?} {}",
                    index + 1,
                    layer.id,
                    layer.renderer,
                    layer.material,
                    layer.readability,
                    lifetime,
                )
            })
            .collect::<Vec<_>>();
        layers.extend(
            recipe
                .particle_layers
                .iter()
                .enumerate()
                .map(|(index, layer)| {
                    format!(
                        "P{}. {} {:?} Decorative {:?} / {} ms",
                        index + 1,
                        layer.id,
                        layer.profile,
                        layer.spawn,
                        layer.lifetime_millis,
                    )
                }),
        );
        let layers = layers.join("\n");
        format!(
            "Mapping  {}/{}  {}\nRecipe  {} - {} mesh + {} particle layer(s) - cap {}\n{}\nFallback {}",
            state.selected + 1,
            state.scenarios.len(),
            scenario.key.as_str(),
            recipe.id,
            recipe.layers.len(),
            recipe.particle_layers.len(),
            recipe.concurrency_cap,
            layers,
            recipe.fallback_recipe,
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
    let mut signature_rows = signatures
        .iter()
        .map(|(signature, name)| {
            (
                name.map_or("unnamed", Name::as_str).to_string(),
                format!(
                    "{:?}/{:?} {:?} {:?} {:?}",
                    signature.carrier,
                    signature.phase,
                    signature.accents,
                    signature.relationship,
                    signature.identity,
                ),
            )
        })
        .collect::<Vec<_>>();
    signature_rows.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    let signature_summary = signature_rows.first().map_or_else(
        || "none".to_string(),
        |(name, signature)| format!("{name}: {signature}"),
    );
    let cache = signature_cache.diagnostics();
    let window = windows.single().ok();
    let dimensions = window.map_or_else(
        || "window unavailable".to_string(),
        |window| format!("{:.0}x{:.0}", window.width(), window.height()),
    );
    let diagnostic_text = format!(
        "VFXLAB - READ-ONLY PRODUCTION VIEWER\n\n\
         {}\n\n\
         Variant  {}{}\n\
         Style    {}{}\n\
         Playback {} - identity {}\n\
         Sample   {} initial ticks\n\
         Radius   {} world units\n\
         Capture  {}\n\
         Catalog  {:016x} - sealed/valid\n\
         Signature {} active - {}\n\
         Cache     {}/{} attacks - {} FIFO evictions\n\
         Active transient {} - particles {}/{} (high {}, suppressed {}) - fixture facts {} - cameras {} - {}\n\n\
         Tab fixture   <-/-> mapping   Space restart   Enter pause   . step\n\
         N identity   C accent   U ultimate   R reduced   P primitive   I guide   F12 capture   Esc quit",
        scenario_details,
        if state.reduced { "reduced" } else { "normal" },
        if state.primitive { " + primitive" } else { "" },
        state.accent.label(),
        if state.ultimate { " + ultimate" } else { "" },
        if state.paused { "paused" } else { "running" },
        state.presentation_identity,
        state.initial_sample_ticks,
        state.radius,
        state.capture_dir.display(),
        embedded_vfx_catalog_fingerprint(),
        signature_rows.len(),
        signature_summary,
        cache.entries,
        cache.capacity,
        cache.evictions,
        effects.iter().count(),
        emitters.iter().count(),
        MAX_ACTIVE_PARTICLE_EMITTERS,
        particle_diagnostics.high_water,
        particle_diagnostics.suppressed,
        fixtures.iter().count(),
        cameras.iter().count(),
        dimensions,
    );
    let text = if state.show_diagnostics {
        diagnostic_text
    } else {
        lab_guide_text(&state, scenario, &dimensions)
    };
    for mut inspector in &mut inspector {
        (**inspector).clone_from(&text);
    }
}

fn lab_guide_text(state: &VfxLabState, scenario: VfxLabScenario, dimensions: &str) -> String {
    let style_label = match (state.fixture, scenario.key) {
        (VfxLabFixture::Transient, crate::client::vfx::REVEAL_SCAN_VFX) => "REVEAL".to_string(),
        (VfxLabFixture::Transient, crate::client::vfx::DEMOLITION_STRIKE_VFX) => {
            "DEMOLITION".to_string()
        }
        (VfxLabFixture::Transient, crate::client::vfx::SELF_CLOAK_VFX) => "SELF CLOAK".to_string(),
        (VfxLabFixture::Transient, crate::client::vfx::FORCED_REVEAL_VFX) => {
            "FORCED REVEAL".to_string()
        }
        (VfxLabFixture::Transient, crate::client::vfx::COMBAT_DAMAGE_VFX) => "DAMAGE".to_string(),
        (VfxLabFixture::Transient, crate::client::vfx::COMBAT_RESET_VFX) => "RESET".to_string(),
        (VfxLabFixture::PersistentSplash, _) => "SPLASH DUAL".to_string(),
        (VfxLabFixture::ConcealmentField, _) => "CONCEALMENT".to_string(),
        (VfxLabFixture::Sentry, _) => "DEPLOYABLE NEUTRAL".to_string(),
        (VfxLabFixture::Status, _) => status_fixture_label(state).to_string(),
        _ => state.accent.label().to_uppercase(),
    };
    let subject = match state.fixture {
        VfxLabFixture::Transient => match scenario.key.as_str() {
            "combat.muzzle" => "WEAPON FIRING FLASH",
            "combat.impact" => "WEAPON HIT / IMPACT",
            "ability.reveal-scan" => "REVEAL SCAN ACTIVATION",
            "ability.elemental-field" => "ULTIMATE FIELD ACTIVATION",
            "ability.demolition-strike" => "DEMOLITION STRIKE MAP BREACH",
            "ability.self-cloak" => "SELF CLOAK ATTACHED LIFECYCLE CUE",
            "ability.forced-reveal" => "FORCED REVEAL TARGET ACQUISITION",
            "combat.damage" => "FIGHTER HIT / DEFEAT OUTCOME",
            "combat.reset" => "FIGHTER RESET OUTCOME",
            _ => "ONE-SHOT GAMEPLAY EFFECT",
        },
        VfxLabFixture::StraightProjectile => "STRAIGHT PROJECTILE IN FLIGHT",
        VfxLabFixture::Melee => "IMPACT BLADE SWEEP AND CONTACT",
        VfxLabFixture::LobbedProjectile => "LOBBED PROJECTILE IN FLIGHT",
        VfxLabFixture::StickyPayload => "ARMED STICKY PAYLOAD",
        VfxLabFixture::PersistentSplash => "PERSISTENT DUAL-PURPOSE SPLASH AREA",
        VfxLabFixture::ElementalField => "PERSISTENT ELEMENTAL AREA",
        VfxLabFixture::ConcealmentField => "PERSISTENT CONCEALMENT VEIL",
        VfxLabFixture::ConeSpray => "PROPAGATING ATTACK CONE",
        VfxLabFixture::Dash => "FIGHTER DASH WAKE",
        VfxLabFixture::Sentry => "SENTRY DEPLOYABLE AND LIFECYCLE CUES",
        VfxLabFixture::Status => "STATUS ATTACHED TO A FIGHTER",
    };
    let meaning = lab_fixture_meaning(state, scenario);
    let style = lab_guide_style(state);
    let style_control = if state_has_fixed_accent(state) {
        "C fixed style  "
    } else {
        "C compare style"
    };
    let (state_suffix, ultimate_control) = lab_mode_labels(state);
    format!(
        "VFXLAB - WHAT YOU ARE LOOKING AT\n\n\
         {subject}\n\
         Style: {}{}   Variant: {}{}\n\n\
         {meaning}\n\n\
         {style}\n\n\
         GRAY SPHERES ARE FIGHTER-SIZE REFERENCES.\n\
         They are not part of the effect.\n\n\
         {style_control}    R reduced    P primitive\n\
         Space restart      Enter pause  . next tick\n\
         Tab fixture        <-/-> effect {ultimate_control}\n\
         I technical details             Esc quit\n\n\
        Window {dimensions}",
        style_label,
        state_suffix,
        if state.reduced { "reduced" } else { "normal" },
        if state.primitive { " + primitive" } else { "" },
    )
}

fn lab_fixture_meaning(state: &VfxLabState, scenario: VfxLabScenario) -> &'static str {
    match state.fixture {
        VfxLabFixture::Transient => {
            if scenario.key == crate::client::vfx::REVEAL_SCAN_VFX {
                "The thin outer ring is the exact 192-unit scan radius and remains for the authoritative reveal deadline. The concentric waves are the one-time activation sweep. Fighters found by that scan retain separate attached Reveal markers; this area does not scan repeatedly."
            } else if scenario.key == crate::client::vfx::DEMOLITION_STRIKE_VFX {
                "The thin outer ring is the exact 64-unit map-destruction radius. The fracture flash, hot core, expanding shockwave, sparks, and smoke present an authoritative destruction event that has already resolved; this is not a pending danger telegraph."
            } else if scenario.key == crate::client::vfx::SELF_CLOAK_VFX {
                "This compact veil is attached to the authoritative fighter position. Activation seals inward as retained fighter translucency begins; press U to compare the sharp interruption used when attacking, taking damage, or being defeated."
            } else if scenario.key == crate::client::vfx::FORCED_REVEAL_VFX {
                "This compact magenta acquisition burst appears on the revealed fighter. The separately retained Reveal status owns the authoritative deadline; this transient only marks the moment the reveal is applied."
            } else if scenario.key == crate::client::vfx::COMBAT_DAMAGE_VFX {
                if state.ultimate {
                    "This outward fracture marks authoritative defeat at the fighter position. The broad break and release ring distinguish it from an ordinary hit; press U to compare the compact damage response."
                } else {
                    "This compact flash and ground slash mark an authoritative damage result at the fighter position. Press U to compare the larger outward defeat response."
                }
            } else if scenario.key == crate::client::vfx::COMBAT_RESET_VFX {
                "This clean restoration pulse marks an authoritative fighter reset. Its ordered green-white rise is intentionally distinct from both incoming damage and defeat."
            } else if state.lobbed_impact {
                "This is a lobbed landing. The thin outer ring is the exact payload radius; the ground flash, central burst, expanding shockwave, sparks, and smoke are staged presentation. Use . to move one tick or restart with another --sample-ticks value."
            } else {
                "The effect is centered between the two gray scale references. Flash bodies communicate the event; expanding rings communicate its ground position. Use . to move one tick or restart with another --sample-ticks value."
            }
        }
        VfxLabFixture::StraightProjectile => {
            "The exact collision footprint remains beneath the bright leading nucleus. Neutral wraps a tapered animated energy body around it; Cold adds rigid crystal fins, Fire adds soft trailing lobes, Poison adds an unstable bubbling wake, and Restoration forms an ordered vitality spark."
        }
        VfxLabFixture::Melee => {
            "The layered crescents inhabit the authored 120-unit, 100-degree melee envelope and face outward from the fighter reference at its origin. The flash at the far reference is a separate contact event; gameplay still resolves targets and damage authoritatively."
        }
        VfxLabFixture::LobbedProjectile => {
            "The moving body follows the production launch/landing arc and rotates along its current tangent. The dark disc tracks its ground projection, the orange ring marks the fixed landing point, and emitted smoke remains behind to expand and fade."
        }
        VfxLabFixture::StickyPayload => {
            "The viscous body is the armed payload. Its bright crawling veins and orbiting droplets accelerate with the authoritative fuse; the exact ground boundary and subtle growing fill communicate the future explosion radius and fuse progression. Press U to compare the smaller Big Blob ultimate secondary."
        }
        VfxLabFixture::PersistentSplash => {
            "The animated fluid is the exact persistent Splash area. Cyan impact energy communicates hostile damage while teal vitality motion communicates allied healing; the thin outer ring remains the exact gameplay boundary."
        }
        VfxLabFixture::ElementalField => {
            "The subtly treated center is the affected ground area. The thin bright ring is the exact gameplay boundary. Crystals, living flame, cellular contamination, or ordered vitality arcs identify the element without hiding fighters."
        }
        VfxLabFixture::ConcealmentField => {
            "The subtle center is the allied concealment volume and the thin bright ring is its exact authoritative radius. Drifting veil wisps communicate concealment without turning the area into an opaque fog bank. Fighter visibility and membership remain server-authoritative."
        }
        VfxLabFixture::ConeSpray => {
            "The wedge shows the currently reached portion of the authoritative cone. Its outer silhouette, not its opacity, communicates attack coverage."
        }
        VfxLabFixture::Dash => {
            "The colored fighter follows the built-in authoritative Dash duration. Its wake grows from the activation point to the current position, then lingers briefly and dissipates after movement ends. The wake is presentation only."
        }
        VfxLabFixture::Sentry => {
            "The grounded turret is the real replicated Sentry presentation. The compact discharge at its muzzle is the fire cue; the separate effect to the right is cleanup. Press U to compare orderly natural expiry with interrupted electrical collapse. Projectile delivery and cleanup remain authoritative."
        }
        VfxLabFixture::Status => {
            "The exact boundary and compact motif are attached presentation for replicated fighter state. Slow/Cold orbit, Frozen locks into a shard crown, Poison bubbles, Fire flickers, Reveal scans, and Knockback pulses outward. The whole motif follows the fighter and disappears at the authoritative deadline."
        }
    }
}

fn lab_mode_labels(state: &VfxLabState) -> (&'static str, &'static str) {
    if state.fixture == VfxLabFixture::Transient
        && state.selected().key == crate::client::vfx::SELF_CLOAK_VFX
    {
        return (
            if state.ultimate {
                " + INTERRUPTED"
            } else {
                " + ACTIVATION"
            },
            "U lifecycle mode",
        );
    }
    if state.fixture == VfxLabFixture::Transient
        && state.selected().key == crate::client::vfx::COMBAT_DAMAGE_VFX
    {
        return (
            if state.ultimate {
                " + DEFEAT"
            } else {
                " + HIT"
            },
            "U outcome mode",
        );
    }
    if state.fixture == VfxLabFixture::Transient
        && state.selected().key == crate::client::vfx::COMBAT_RESET_VFX
    {
        return ("", "U no alternate");
    }
    if state.fixture == VfxLabFixture::Sentry {
        return (
            if state.ultimate {
                " + INTERRUPTED"
            } else {
                " + EXPIRY"
            },
            "U cleanup mode",
        );
    }
    if state.fixture == VfxLabFixture::Status {
        return ("", "U alternate status");
    }
    (
        if state.ultimate { " + ULTIMATE" } else { "" },
        "U ultimate",
    )
}

const fn status_fixture_label(state: &VfxLabState) -> &'static str {
    match (state.accent, state.ultimate) {
        (VfxLabAccent::Neutral, false) => "SLOW",
        (VfxLabAccent::Neutral, true) => "KNOCKBACK",
        (VfxLabAccent::Cold, false) => "COLD",
        (VfxLabAccent::Cold, true) => "FROZEN",
        (VfxLabAccent::Fire, _) => "FIRE",
        (VfxLabAccent::Poison, _) => "POISON",
        (VfxLabAccent::Restoration, _) => "REVEAL",
    }
}

fn lab_guide_style(state: &VfxLabState) -> &'static str {
    if let Some(style) = fighter_outcome_guide_style(state) {
        return style;
    }
    if state.fixture == VfxLabFixture::Transient
        && state.selected().key == crate::client::vfx::SELF_CLOAK_VFX
    {
        return if state.ultimate {
            "INTERRUPTED CLOAK fractures the veil with a compact core flash, sharp ground break, and fast release rings; it reads as a broken state rather than a gentle timeout."
        } else {
            "SELF CLOAK pulls a low cyan veil inward around the fighter, briefly enclosing the body without becoming the lasting visibility state."
        };
    }
    if state.fixture == VfxLabFixture::Transient
        && state.selected().key == crate::client::vfx::FORCED_REVEAL_VFX
    {
        return "FORCED REVEAL uses the same magenta electronic detection language as Reveal Scan, compressed into a target-sized reticle and acquisition pulse.";
    }
    if state.fixture == VfxLabFixture::ConcealmentField {
        return "CONCEALMENT uses a low, breathing energy veil with sparse drifting wisps. It is intentionally distinct from elemental hazard motion and preserves map readability; broader ownership colors remain tracked by BRL-0107.";
    }
    if state.fixture == VfxLabFixture::Status {
        return match (state.accent, state.ultimate) {
            (VfxLabAccent::Neutral, false) => {
                "SLOW uses a restrained cyan drag boundary with orbiting snowflake motes. Press U for Knockback or C for Cold."
            }
            (VfxLabAccent::Neutral, true) => {
                "KNOCKBACK uses orange impulse bars that repeatedly drive outward from the fighter. Press U for Slow or C for Cold."
            }
            (VfxLabAccent::Cold, false) => {
                "COLD uses orbiting vertical ice splinters around its related cyan boundary. Press U to compare the locked Frozen state."
            }
            (VfxLabAccent::Cold, true) => {
                "FROZEN intensifies Cold into a six-shard locking crown with a sharper deterministic shiver. Press U to return to Cold."
            }
            (VfxLabAccent::Fire, _) => {
                "FIRE uses a hot red boundary and three independently flickering flame tongues. Press C to compare Poison."
            }
            (VfxLabAccent::Poison, _) => {
                "POISON uses an acidic boundary with uneven bubbles that linger and bob without hiding the fighter. Press C to compare Reveal."
            }
            (VfxLabAccent::Restoration, _) => {
                "REVEAL uses the magenta detection language: a wider boundary and four fast rotating sensor sparks. Press C to return to Slow."
            }
        };
    }
    if state.fixture == VfxLabFixture::Transient
        && state.selected().key == crate::client::vfx::REVEAL_SCAN_VFX
    {
        return "REVEAL uses a precise magenta sensor language: a restrained exact boundary, fast concentric acquisition waves, and a compact origin reticle. It is deliberately geometric and electronic rather than elemental.";
    }
    if state.fixture == VfxLabFixture::Transient
        && state.selected().key == crate::client::vfx::DEMOLITION_STRIKE_VFX
    {
        return "DEMOLITION uses a hard orange breach language: an exact destruction boundary, a radial fracture motif, a compact white-hot core, an expanding shock front, and ballistic debris with smoke. The blast is intentionally distinct from elemental fire.";
    }
    match state.accent {
        VfxLabAccent::Neutral if state.fixture == VfxLabFixture::StraightProjectile => {
            "NEUTRAL is the reusable Pulse/Scatter comet language: a dimensional team-colored nucleus embedded inside a deterministic wrapping energy wake. Press C to compare each elemental identity."
        }
        VfxLabAccent::Neutral if state.fixture == VfxLabFixture::Melee => {
            "IMPACT BLADE uses a fast cool-white energy sweep: a tapered cutting crescent, a brighter inner blade, a delayed afterimage, and a compact contact burst. Its reach, angle, and facing come from signature geometry rather than a weapon-ID branch."
        }
        VfxLabAccent::Neutral if state.fixture == VfxLabFixture::Transient => {
            "NEUTRAL straight shots use a cool-white energy language: a compressed launch flash and a layered impact that dissipates through a ground wave and deterministic sparks. Press C to compare each elemental identity."
        }
        VfxLabAccent::Neutral if state.fixture == VfxLabFixture::StickyPayload => {
            "NEUTRAL sticky payloads use a volatile violet language shared by Sticky Blomb and Big Blob: a dark dimensional core, living emissive veins, detached viscous droplets, and an exact delayed-blast telegraph."
        }
        VfxLabAccent::Neutral if state.fixture == VfxLabFixture::PersistentSplash => {
            "SPLASH is intentionally dual-purpose rather than purely elemental: traveling ripples, bright caustics, and alternating fluid droplets keep hostile impact and allied restoration legible together."
        }
        VfxLabAccent::Neutral if state.fixture == VfxLabFixture::Dash => {
            "NEUTRAL Dash uses a cool-white high-velocity wake: a hot central lane, turbulent cyan filaments, and sparse speed cuts that dissipate after the authoritative movement ends."
        }
        VfxLabAccent::Neutral if state.fixture == VfxLabFixture::Sentry => {
            "SENTRY uses a compact machine-energy language: planted armor, a contained charge discharge, an orderly contracting shutdown, and a sharper electrical collapse for interruption. Team and ownership language remain separate from this neutral deployable identity."
        }
        VfxLabAccent::Neutral => {
            "NEUTRAL is the safe fallback. Press C to compare each authored elemental identity."
        }
        VfxLabAccent::Cold => {
            "COLD should feel slow, crisp, and crystalline. Press C once to compare Fire."
        }
        VfxLabAccent::Fire => {
            "FIRE should feel turbulent, hot-edged, and continuously alive. Press C once to compare Poison."
        }
        VfxLabAccent::Poison => {
            "POISON should feel unstable, bubbling, and contaminating without becoming an opaque green fill. Press C once to compare Restoration."
        }
        VfxLabAccent::Restoration => {
            "RESTORATION should feel ordered, buoyant, and life-giving through a rotating vitality motif. Press C to return to Neutral."
        }
    }
}

fn fighter_outcome_guide_style(state: &VfxLabState) -> Option<&'static str> {
    if state.fixture != VfxLabFixture::Transient {
        return None;
    }
    if state.selected().key == crate::client::vfx::COMBAT_DAMAGE_VFX {
        return Some(if state.ultimate {
            "DEFEAT expands into a broad red fracture and release ring, giving the terminal outcome more visual weight than an ordinary hit."
        } else {
            "DAMAGE stays compact and immediate: a red contact core, directional ground slash, short echo, and bounded sparks."
        });
    }
    (state.selected().key == crate::client::vfx::COMBAT_RESET_VFX).then_some(
        "RESET uses ordered restoration geometry, a rising green-white core, and a clean expanding ring to read as recovery rather than impact.",
    )
}

const fn fixture_index(fixture: VfxLabFixture) -> u8 {
    match fixture {
        VfxLabFixture::Transient => 1,
        VfxLabFixture::StraightProjectile => 2,
        VfxLabFixture::Melee => 3,
        VfxLabFixture::LobbedProjectile => 4,
        VfxLabFixture::StickyPayload => 5,
        VfxLabFixture::PersistentSplash => 6,
        VfxLabFixture::ElementalField => 7,
        VfxLabFixture::ConcealmentField => 8,
        VfxLabFixture::ConeSpray => 9,
        VfxLabFixture::Dash => 10,
        VfxLabFixture::Sentry => 11,
        VfxLabFixture::Status => 12,
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
        assert!(
            VfxLabConfig {
                sample_ticks: Some(MAX_INITIAL_SAMPLE_TICKS + 1),
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
            accent: VfxLabAccent::Neutral,
            lobbed_impact: false,
            ultimate: false,
            reduced: false,
            primitive: false,
            show_diagnostics: false,
            paused: false,
            step_once: false,
            capture_dir: PathBuf::from("target/vfx-lab"),
            capture_sequence: 0,
            fixture: VfxLabFixture::Transient,
            playback_tick: 30,
            initial_sample_ticks: 0,
            pending_sample_ticks: 0,
            tick_accumulator: Duration::ZERO,
            capture_after_updates: None,
            update_index: 0,
            exit_after_capture: false,
            pending_capture_paths: None,
        };
        let guide = lab_guide_text(&state, scenario, "1280x720");
        assert!(guide.contains("WHAT YOU ARE LOOKING AT"));
        assert!(guide.contains("FIGHTER-SIZE REFERENCES"));
        assert!(guide.contains("Press C"));
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
    fn lobbed_impact_preview_uses_the_production_carrier_and_radius() {
        let scenario = VfxLabScenario {
            key: crate::client::vfx::COMBAT_IMPACT_VFX,
            producer_rank: crate::client::vfx::COMBAT_VFX_PRODUCER_RANK,
            capabilities: VfxRequestCapabilities::RADIUS,
        };
        let request = scenario.request(9, 150, VfxLabAccent::Fire, true, false);
        assert_eq!(
            request.signature.carrier,
            crate::client::vfx::VfxCarrier::LobbedProjectile
        );
        assert_eq!(request.authoritative_radius, Some(150.0));
        assert_eq!(
            request.signature.geometry,
            crate::client::vfx::VfxGeometry::circle(Vec2::ZERO, 150.0)
        );
    }

    #[test]
    fn neutral_transient_preview_uses_straight_production_signatures() {
        let muzzle = VfxLabScenario {
            key: crate::client::vfx::COMBAT_MUZZLE_VFX,
            producer_rank: crate::client::vfx::COMBAT_VFX_PRODUCER_RANK,
            capabilities: VfxRequestCapabilities::NONE,
        }
        .request(10, DEFAULT_RADIUS, VfxLabAccent::Neutral, false, false);
        assert_eq!(
            muzzle.signature.carrier,
            crate::client::vfx::VfxCarrier::StraightProjectile
        );
        assert_eq!(
            muzzle.signature.phase,
            crate::client::vfx::VfxPhase::Activation
        );

        let impact = VfxLabScenario {
            key: crate::client::vfx::COMBAT_IMPACT_VFX,
            producer_rank: crate::client::vfx::COMBAT_VFX_PRODUCER_RANK,
            capabilities: VfxRequestCapabilities::NONE,
        }
        .request(11, DEFAULT_RADIUS, VfxLabAccent::Neutral, false, false);
        assert_eq!(
            impact.signature.carrier,
            crate::client::vfx::VfxCarrier::StraightProjectile
        );
        assert_eq!(impact.signature.phase, crate::client::vfx::VfxPhase::Impact);
    }

    #[test]
    fn fighter_outcome_previews_use_production_lifecycle_signatures() {
        let damage_scenario = VfxLabScenario {
            key: crate::client::vfx::COMBAT_DAMAGE_VFX,
            producer_rank: crate::client::vfx::COMBAT_VFX_PRODUCER_RANK,
            capabilities: VfxRequestCapabilities::NONE,
        };
        let hit = damage_scenario.request(12, DEFAULT_RADIUS, VfxLabAccent::Fire, false, false);
        let defeat = damage_scenario.request(13, DEFAULT_RADIUS, VfxLabAccent::Fire, false, true);
        assert_eq!(
            hit.signature.carrier,
            crate::client::vfx::VfxCarrier::AttachedStatus
        );
        assert_eq!(hit.signature.phase, crate::client::vfx::VfxPhase::Impact);
        assert_eq!(
            defeat.signature.phase,
            crate::client::vfx::VfxPhase::Interruption
        );
        assert_eq!(
            defeat.signature.accents,
            [Some(crate::client::vfx::VfxAccent::Neutral), None]
        );
        assert_eq!(defeat.signature.context, None);

        let reset = VfxLabScenario {
            key: crate::client::vfx::COMBAT_RESET_VFX,
            producer_rank: crate::client::vfx::COMBAT_VFX_PRODUCER_RANK,
            capabilities: VfxRequestCapabilities::NONE,
        }
        .request(14, DEFAULT_RADIUS, VfxLabAccent::Fire, false, false);
        assert_eq!(
            reset.signature.carrier,
            crate::client::vfx::VfxCarrier::AttachedStatus
        );
        assert_eq!(reset.signature.phase, crate::client::vfx::VfxPhase::Expiry);
        assert_eq!(
            reset.signature.accents,
            [Some(crate::client::vfx::VfxAccent::Neutral), None]
        );
        assert_eq!(reset.signature.context, None);
    }

    #[test]
    fn demolition_preview_uses_production_semantics_and_authored_radius() {
        let scenario = VfxLabScenario {
            key: crate::client::vfx::DEMOLITION_STRIKE_VFX,
            producer_rank: crate::client::vfx::COMBAT_VFX_PRODUCER_RANK,
            capabilities: VfxRequestCapabilities::RADIUS,
        };
        let request = scenario.request(12, 64, VfxLabAccent::Cold, false, false);
        assert_eq!(request.authoritative_radius, Some(64.0));
        assert_eq!(
            request.signature.carrier,
            crate::client::vfx::VfxCarrier::PersistentArea
        );
        assert_eq!(
            request.signature.phase,
            crate::client::vfx::VfxPhase::Activation
        );
        assert_eq!(
            request.signature.accents,
            [Some(crate::client::vfx::VfxAccent::Demolition), None]
        );
        assert_eq!(
            request.signature.context,
            Some(crate::client::vfx::VfxContextOverride::DemolitionStrike)
        );
        assert_eq!(
            request.signature.geometry,
            crate::client::vfx::VfxGeometry::circle(Vec2::ZERO, 64.0)
        );
    }

    #[test]
    fn reveal_preview_uses_production_semantics_radius_and_deadline() {
        let scenario = VfxLabScenario {
            key: crate::client::vfx::REVEAL_SCAN_VFX,
            producer_rank: crate::client::vfx::COMBAT_VFX_PRODUCER_RANK,
            capabilities: VfxRequestCapabilities::RADIUS_AND_DEADLINE,
        };
        let request = scenario.request(13, 192, VfxLabAccent::Fire, false, false);
        assert_eq!(request.authoritative_radius, Some(192.0));
        assert_eq!(request.deadline, Some(VfxDeadline::new(0, 300, Some(0))));
        assert_eq!(
            request.signature.carrier,
            crate::client::vfx::VfxCarrier::PersistentArea
        );
        assert_eq!(
            request.signature.accents,
            [Some(crate::client::vfx::VfxAccent::Reveal), None]
        );
        assert_eq!(
            request.signature.context,
            Some(crate::client::vfx::VfxContextOverride::RevealScan)
        );
        assert_eq!(
            request.signature.geometry,
            crate::client::vfx::VfxGeometry::circle(Vec2::ZERO, 192.0)
        );
    }

    #[test]
    fn fixture_names_and_cycle_are_bounded() {
        let expected = [
            ("transient", VfxLabFixture::Transient),
            ("straight", VfxLabFixture::StraightProjectile),
            ("melee", VfxLabFixture::Melee),
            ("lobbed", VfxLabFixture::LobbedProjectile),
            ("sticky", VfxLabFixture::StickyPayload),
            ("splash", VfxLabFixture::PersistentSplash),
            ("field", VfxLabFixture::ElementalField),
            ("concealment", VfxLabFixture::ConcealmentField),
            ("cone", VfxLabFixture::ConeSpray),
            ("dash", VfxLabFixture::Dash),
            ("sentry", VfxLabFixture::Sentry),
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

    #[test]
    fn accent_names_and_cycle_cover_all_authored_elements() {
        let expected = [
            ("neutral", VfxLabAccent::Neutral),
            ("cold", VfxLabAccent::Cold),
            ("fire", VfxLabAccent::Fire),
            ("poison", VfxLabAccent::Poison),
            ("restoration", VfxLabAccent::Restoration),
        ];
        for (name, accent) in expected {
            assert_eq!(VfxLabAccent::from_key(name), Some(accent));
        }

        let mut accent = VfxLabAccent::Neutral;
        for _ in 0..expected.len() {
            accent = accent.next();
        }
        assert_eq!(accent, VfxLabAccent::Neutral);
    }
}
