//! Sparse, low-contrast toy motifs over the shell background (BRL-51zd).
//!
//! Owners mark their full-screen background node with [`ShellBackdrop`]; this module adds one
//! motif layer child drawn by `shell_motifs.wgsl`. Motif positions, shapes and opacity live in that
//! shader beside the Dashboard plane values. Motifs drift with the Dashboard planes and freeze under
//! reduced motion/effects; the Low render tier drops the layer (`render_quality.ron`). The layer
//! is decoration only: it has no interaction and never owns layout.

use super::surface::SurfaceMotionPolicy;
use bevy::{
    prelude::*, reflect::TypePath, render::render_resource::AsBindGroup, shader::ShaderRef,
};

/// Where the owning screen leaves room for motifs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MotifLayout {
    /// Clear of the Dashboard logo, hero, status bar and the right-hand action column.
    Dashboard,
    /// Outer margins only, for the content-dense screens over `game-selection-background`.
    Edges,
}

/// Marks a full-screen shell background node that carries the motif layer.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct ShellBackdrop(pub MotifLayout);

#[derive(Component)]
struct MotifLayer;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct ShellMotifMaterial {
    /// x: seconds, y: drift amount (0 freezes motifs), z: layout (0 Dashboard, 1 edges).
    #[uniform(0)]
    motion: Vec4,
    /// Motif color; alpha is the authored opacity.
    #[uniform(1)]
    tint: Vec4,
}

impl UiMaterial for ShellMotifMaterial {
    fn fragment_shader() -> ShaderRef {
        "brawler/shaders/shell_motifs.wgsl".into()
    }
}

/// One shared material per layout, so a frame updates at most two uniforms.
#[derive(Resource)]
struct MotifMaterials {
    dashboard: Handle<ShellMotifMaterial>,
    edges: Handle<ShellMotifMaterial>,
}

/// Time at which reduced motion freezes the motif pose, matching the Dashboard planes.
const FROZEN_SECONDS: f32 = 5.0;
/// Motifs are white ink on the blue gradient; opacity is authored in the shader.
const MOTIF_TINT: Vec4 = Vec4::new(1.0, 1.0, 1.0, 1.0);

pub(crate) struct ShellMotifPlugin;

impl Plugin for ShellMotifPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiMaterialPlugin::<ShellMotifMaterial>::default())
            .add_systems(Update, (attach_motif_layers, update_motif_layers).chain());
    }
}

fn layout_value(layout: MotifLayout) -> f32 {
    match layout {
        MotifLayout::Dashboard => 0.0,
        MotifLayout::Edges => 1.0,
    }
}

/// The `(seconds, drift)` the shader receives: drift stops entirely under reduced motion.
fn motif_motion(seconds: f32, reduced: bool) -> (f32, f32) {
    if reduced {
        (FROZEN_SECONDS, 0.0)
    } else {
        (seconds, 1.0)
    }
}

fn layer_visibility(policy: SurfaceMotionPolicy) -> Visibility {
    if policy.motifs {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy owns presentation resources as system parameters"
)]
fn attach_motif_layers(
    mut commands: Commands,
    policy: Res<SurfaceMotionPolicy>,
    shared: Option<Res<MotifMaterials>>,
    mut materials: ResMut<Assets<ShellMotifMaterial>>,
    backdrops: Query<(Entity, &ShellBackdrop), Added<ShellBackdrop>>,
) {
    if backdrops.is_empty() {
        return;
    }
    let shared = if let Some(shared) = shared {
        MotifMaterials {
            dashboard: shared.dashboard.clone(),
            edges: shared.edges.clone(),
        }
    } else {
        let (seconds, drift) = motif_motion(FROZEN_SECONDS, policy.reduced);
        let mut add = |layout| {
            materials.add(ShellMotifMaterial {
                motion: Vec4::new(seconds, drift, layout_value(layout), 0.0),
                tint: MOTIF_TINT,
            })
        };
        let created = MotifMaterials {
            dashboard: add(MotifLayout::Dashboard),
            edges: add(MotifLayout::Edges),
        };
        commands.insert_resource(MotifMaterials {
            dashboard: created.dashboard.clone(),
            edges: created.edges.clone(),
        });
        created
    };
    for (entity, backdrop) in &backdrops {
        let material = match backdrop.0 {
            MotifLayout::Dashboard => shared.dashboard.clone(),
            MotifLayout::Edges => shared.edges.clone(),
        };
        commands.entity(entity).with_child((
            MotifLayer,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                ..default()
            },
            MaterialNode(material),
            layer_visibility(*policy),
        ));
    }
}

/// Advances drift while motion is allowed and hides the layer when the tier drops motifs.
/// Frozen motifs are not rewritten, so reduced motion costs no per-frame uniform upload.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy owns presentation resources as system parameters"
)]
fn update_motif_layers(
    time: Res<Time>,
    policy: Res<SurfaceMotionPolicy>,
    shared: Option<Res<MotifMaterials>>,
    mut materials: ResMut<Assets<ShellMotifMaterial>>,
    mut layers: Query<&mut Visibility, With<MotifLayer>>,
) {
    let visibility = layer_visibility(*policy);
    for mut layer in &mut layers {
        layer.set_if_neq(visibility);
    }
    let Some(shared) = shared else {
        return;
    };
    if !policy.motifs || layers.is_empty() {
        return;
    }
    let (seconds, drift) = motif_motion(time.elapsed_secs(), policy.reduced);
    for handle in [&shared.dashboard, &shared.edges] {
        let unchanged = materials
            .get(handle)
            .is_none_or(|material| material.motion.xy() == Vec2::new(seconds, drift));
        if unchanged {
            continue;
        }
        if let Some(mut material) = materials.get_mut(handle) {
            material.motion.x = seconds;
            material.motion.y = drift;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use core::time::Duration;

    fn motif_app(policy: SurfaceMotionPolicy) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<ShellMotifMaterial>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                50,
            )))
            .insert_resource(policy)
            .add_systems(Update, (attach_motif_layers, update_motif_layers).chain());
        let backdrop = app
            .world_mut()
            .spawn((Node::default(), ShellBackdrop(MotifLayout::Edges)))
            .id();
        app.update();
        app.update();
        (app, backdrop)
    }

    fn edges_motion(app: &App) -> Vec4 {
        let shared = app.world().resource::<MotifMaterials>();
        app.world()
            .resource::<Assets<ShellMotifMaterial>>()
            .get(&shared.edges)
            .unwrap()
            .motion
    }

    fn layer(app: &mut App) -> Entity {
        app.world_mut()
            .query_filtered::<Entity, With<MotifLayer>>()
            .single(app.world())
            .unwrap()
    }

    #[test]
    fn backdrop_gets_one_decorative_layer_that_drifts() {
        let (mut app, backdrop) = motif_app(SurfaceMotionPolicy::default());
        let layer = layer(&mut app);
        assert_eq!(
            app.world().get::<ChildOf>(layer).unwrap().parent(),
            backdrop
        );
        assert!(app.world().get::<Interaction>(layer).is_none());
        assert_eq!(
            app.world().get::<Visibility>(layer),
            Some(&Visibility::Inherited)
        );
        let before = edges_motion(&app);
        app.update();
        let after = edges_motion(&app);
        assert!((after.y - 1.0).abs() < f32::EPSILON);
        assert!(after.x > before.x, "drift advances with time");
    }

    #[test]
    fn reduced_motion_freezes_motifs_in_place() {
        let (mut app, _) = motif_app(SurfaceMotionPolicy {
            reduced: true,
            motifs: true,
        });
        for _ in 0..5 {
            app.update();
            let motion = edges_motion(&app);
            assert!(motion.y.abs() < f32::EPSILON, "no drift");
            assert!(
                (motion.x - FROZEN_SECONDS).abs() < f32::EPSILON,
                "fixed pose"
            );
        }
    }

    #[test]
    fn low_tier_hides_the_layer_and_restores_it() {
        let (mut app, _) = motif_app(SurfaceMotionPolicy {
            reduced: false,
            motifs: false,
        });
        let layer = layer(&mut app);
        assert_eq!(
            app.world().get::<Visibility>(layer),
            Some(&Visibility::Hidden)
        );
        app.world_mut().resource_mut::<SurfaceMotionPolicy>().motifs = true;
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(layer),
            Some(&Visibility::Inherited)
        );
    }

    /// BRL-0196: the layer fills its backdrop at the phone viewport and adds no controls.
    #[test]
    fn layer_fills_the_backdrop_at_phone_size_without_controls() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<SurfaceMotionPolicy>();
        super::super::layout_harness::add_layout(&mut app, 914, 411);
        app.init_asset::<ShellMotifMaterial>()
            .add_systems(Update, (attach_motif_layers, update_motif_layers).chain());
        super::super::layout_harness::apply_viewport_fit(&mut app);
        let backdrop = app
            .world_mut()
            .spawn((
                Node {
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                ShellBackdrop(MotifLayout::Dashboard),
            ))
            .id();
        for _ in 0..3 {
            app.update();
        }
        let layer = layer(&mut app);
        let outer = super::super::layout_harness::node_rect(app.world(), backdrop).unwrap();
        let inner = super::super::layout_harness::node_rect(app.world(), layer).unwrap();
        assert!((outer.size() - inner.size()).abs().max_element() < 0.5);
        assert!((inner.width() - 914.0).abs() < 0.5, "{inner:?}");
        assert!(app.world().get::<Button>(layer).is_none());
    }
}
