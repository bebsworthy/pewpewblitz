//! Development-only read-only viewer over the production VFX registry and materializer.

use super::{
    ArenaCamera,
    hud::ClientHeistReadiness,
    presentation_3d::{
        CombatEffect3d, ImportedWorldFallbackPolicy, PendingCombatEffect, cleanup_combat_effects,
        materialize_combat_effects, resolve_vfx_requests, setup_3d_foundation,
    },
    settings::persistence::ClientShellSettings,
    vfx::{
        ProductionVfxPlugin, VfxDeadline, VfxLifetime, VfxRegistry, VfxRequest,
        VfxRequestCapabilities, VfxRequestKey, VfxRequestOrder, embedded_vfx_catalog_fingerprint,
    },
};
use bevy::{
    app::AppExit,
    prelude::*,
    window::{PrimaryWindow, WindowResolution},
};

const DEFAULT_RADIUS: f32 = 72.0;

/// Command-line configuration for the read-only production VFX viewer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VfxLabConfig {
    pub effect_key: Option<String>,
    pub reduced: bool,
    pub primitive: bool,
    pub window_size: Option<(u16, u16)>,
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
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct VfxLabScenario {
    key: VfxRequestKey,
    producer_rank: u16,
    capabilities: VfxRequestCapabilities,
}

impl VfxLabScenario {
    fn request(self, event_id: u64) -> VfxRequest {
        let authoritative_radius = self
            .capabilities
            .authoritative_radius
            .then_some(DEFAULT_RADIUS);
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
struct VfxLabState {
    scenarios: Vec<VfxLabScenario>,
    selected: usize,
    next_event_id: u64,
    reduced: bool,
    primitive: bool,
}

impl VfxLabState {
    fn selected(&self) -> VfxLabScenario {
        self.scenarios[self.selected]
    }

    fn replay(&mut self) -> VfxRequest {
        self.next_event_id = self.next_event_id.saturating_add(1);
        self.selected().request(self.next_event_id)
    }

    fn select_relative(&mut self, offset: isize) {
        let len = self.scenarios.len();
        self.selected = self.selected.saturating_add_signed(offset).min(len - 1);
    }
}

#[derive(Component)]
struct VfxLabInspector;

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
                cleanup_combat_effects,
                update_lab_inspector,
            )
                .chain(),
        );
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
            next_event_id: 0,
            reduced: self.config.reduced,
            primitive: self.config.primitive,
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
    .add_plugins(VfxLabPlugin { config });
    app
}

fn setup_lab_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut requests: MessageWriter<VfxRequest>,
    mut state: ResMut<VfxLabState>,
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
    requests.write(state.replay());
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "resources are Bevy system parameters owned by the scheduler"
)]
fn handle_lab_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<VfxLabState>,
    mut settings: ResMut<ClientShellSettings>,
    mut fallback: ResMut<ImportedWorldFallbackPolicy>,
    mut requests: MessageWriter<VfxRequest>,
    mut exits: MessageWriter<AppExit>,
) {
    let mut replay = false;
    if keyboard.just_pressed(KeyCode::ArrowLeft) {
        state.select_relative(-1);
        replay = true;
    }
    if keyboard.just_pressed(KeyCode::ArrowRight) {
        state.select_relative(1);
        replay = true;
    }
    if keyboard.just_pressed(KeyCode::Space) {
        replay = true;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        state.reduced = !state.reduced;
        settings.reduced_combat_effects = state.reduced;
        replay = true;
    }
    if keyboard.just_pressed(KeyCode::KeyP) {
        state.primitive = !state.primitive;
        *fallback = if state.primitive {
            ImportedWorldFallbackPolicy::ForcePrimitive
        } else {
            ImportedWorldFallbackPolicy::Auto
        };
        replay = true;
    }
    if replay {
        requests.write(state.replay());
    }
    if keyboard.just_pressed(KeyCode::Escape) {
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
    windows: Query<&Window, With<PrimaryWindow>>,
    mut inspector: Query<&mut Text, With<VfxLabInspector>>,
    cameras: Query<(), With<ArenaCamera>>,
) {
    let scenario = state.selected();
    let request = scenario.request(state.next_event_id.max(1));
    let profile = registry
        .resolve(&request, state.reduced)
        .expect("production lab scenario resolves");
    let lifetime = match profile.lifetime {
        VfxLifetime::Millis(value) => format!("{value} ms"),
        VfxLifetime::AuthoritativeDeadline => "authoritative deadline".to_string(),
    };
    let window = windows.single().ok();
    let dimensions = window.map_or_else(
        || "window unavailable".to_string(),
        |window| format!("{:.0}×{:.0}", window.width(), window.height()),
    );
    let text = format!(
        "VFXLAB · READ-ONLY PRODUCTION VIEWER\n\n\
         Effect  {}/{}  {}\n\
         Profile {}\n\
         Renderer {:?} · material {:?}\n\
         Scale {:?} · anchor {:?}\n\
         Lifetime {} · cap {}\n\
         Fallback {}\n\n\
         Variant  {}{}\n\
         Catalog  {:016x}\n\
         Active roots {} · cameras {} · {}\n\n\
         ←/→ select   Space replay   R reduced   P primitive   Esc quit",
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
        if state.reduced { "reduced" } else { "normal" },
        if state.primitive { " + primitive" } else { "" },
        embedded_vfx_catalog_fingerprint(),
        effects.iter().count(),
        cameras.iter().count(),
        dimensions,
    );
    for mut inspector in &mut inspector {
        (**inspector).clone_from(&text);
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
    }

    #[test]
    fn scenario_selection_is_bounded_and_replay_identity_is_monotonic() {
        let scenario = VfxLabScenario {
            key: VfxRequestKey::new("combat.muzzle"),
            producer_rank: 100,
            capabilities: VfxRequestCapabilities::NONE,
        };
        let mut state = VfxLabState {
            scenarios: vec![scenario, scenario],
            selected: 0,
            next_event_id: 0,
            reduced: false,
            primitive: false,
        };
        state.select_relative(-1);
        assert_eq!(state.selected, 0);
        state.select_relative(9);
        assert_eq!(state.selected, 1);
        assert_eq!(state.replay().order.event_id, 1);
        assert_eq!(state.replay().order.event_id, 2);
    }
}
