//! Brawler card Riff previews: one fixed-aspect offscreen target per weapon family on screen, so
//! each brawler card, the details portrait, and the creation fighter card show Riff holding that
//! brawler's own primary weapon family (BRL-y6zs). The Dashboard Riff keeps the selected
//! brawler's family on its own layer (`DASHBOARD_RENDER_LAYER`).
use crate::flow::BrawlerCardPreview;
use crate::presentation_3d::{
    DASHBOARD_RIFF_SCALE, Imported3dAssets, WeaponFamily, WorldLookCatalog, find_socket_and_player,
    player_preview_toon_scene, riff_scene_transform, weapon_attachment,
};
use bevy::{
    asset::RenderAssetUsages,
    camera::{ClearColorConfig, RenderTarget, visibility::RenderLayers},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
    world_serialization::{WorldAssetRoot, WorldInstanceReady},
};
use core::time::Duration;

/// One private render layer per family, clear of the map palette previews (18..=24), the weapon
/// previews (28, 30, 32, 33), the Dashboard (29), and the world HUD (31).
const LAYERS: [usize; 4] = [34, 35, 36, 37];

/// Card frames vary per screen, so the target keeps one fixed square aspect.
const TARGET_SIZE: u32 = 768;

/// One family's card camera, light, and Riff scene root.
#[derive(Component)]
pub(super) struct CardOwned(WeaponFamily);

/// The offscreen camera of one family's card target.
#[derive(Component)]
pub(super) struct BrawlerCardCamera;

/// The scene root under which the imported Riff spawns once the imported assets exist.
#[derive(Component)]
pub(super) struct CardRoot(WeaponFamily);

#[derive(Component)]
pub(super) struct CardRiffSpawned;

/// An imported Riff or weapon scene root; its descendants join the family's layer when ready.
#[derive(Component)]
struct CardScene(WeaponFamily);

struct Target {
    image: Handle<Image>,
    camera: Entity,
}

/// The live card target of each family, indexed by `WeaponFamily::index`.
#[derive(Resource, Default)]
pub(super) struct Targets([Option<Target>; 4]);

pub(super) fn install(app: &mut App) {
    app.init_resource::<Targets>().add_observer(ready);
}

fn layer(family: WeaponFamily) -> RenderLayers {
    RenderLayers::layer(LAYERS[family.index()])
}

/// Creates a family's target while a host of that family exists and releases it when none remain.
#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "Bevy preview hosts and render target ownership"
)]
pub(super) fn ensure(
    mut commands: Commands,
    hosts: Query<&BrawlerCardPreview>,
    mut targets: ResMut<Targets>,
    owned: Query<(Entity, &CardOwned)>,
    roots: Query<(Entity, &CardRoot), Without<CardRiffSpawned>>,
    mut images: ResMut<Assets<Image>>,
    imported: Option<Res<Imported3dAssets>>,
    look: Res<WorldLookCatalog>,
) {
    for family in WeaponFamily::ALL {
        let wanted = hosts.iter().any(|host| host.0 == family);
        let slot = &mut targets.0[family.index()];
        match (slot.is_some(), wanted) {
            (true, false) => {
                for (entity, owner) in &owned {
                    if owner.0 == family {
                        commands.entity(entity).try_despawn();
                    }
                }
                if let Some(target) = slot.take() {
                    images.remove(target.image.id());
                }
            }
            (false, true) => {
                *slot = Some(spawn_target(&mut commands, &mut images, &look, family));
            }
            _ => {}
        }
    }
    let Some(imported) = imported else {
        return;
    };
    for (root, scene) in &roots {
        if targets.0[scene.0.index()].is_none() {
            continue;
        }
        commands.entity(root).insert(CardRiffSpawned).with_child((
            CardScene(scene.0),
            player_preview_toon_scene(),
            WorldAssetRoot(imported.character_scene.clone()),
            riff_scene_transform(-0.12, DASHBOARD_RIFF_SCALE),
            layer(scene.0),
            Name::new(format!("Brawler card Riff {:?}", scene.0)),
        ));
    }
}

fn spawn_target(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    look: &WorldLookCatalog,
    family: WeaponFamily,
) -> Target {
    let mut image = Image::new_uninit(
        Extent3d {
            width: TARGET_SIZE,
            height: TARGET_SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::all(),
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    let image = images.add(image);
    let camera = commands
        .spawn((
            CardOwned(family),
            BrawlerCardCamera,
            crate::presentation_3d::render_quality::PreviewCamera,
            Camera3d::default(),
            Camera {
                order: -3,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
            RenderTarget::Image(image.clone().into()),
            Projection::Perspective(PerspectiveProjection {
                fov: 0.42,
                ..default()
            }),
            look.preview_camera_look(),
            look.preview_ambient_light(),
            Transform::from_xyz(205.0, 72.0, 125.0).looking_at(Vec3::new(0.0, 34.0, 0.0), Vec3::Y),
            layer(family),
        ))
        .id();
    // The Dashboard preview rig: one catalog key light (the toon material bands directional
    // light and ambient only, BRL-0179).
    commands.spawn((
        CardOwned(family),
        look.preview_key_light(),
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.65, -0.55, 0.0)),
        layer(family),
    ));
    commands.spawn((
        CardOwned(family),
        CardRoot(family),
        Transform::default(),
        Visibility::default(),
        layer(family),
    ));
    Target { image, camera }
}

/// Fits each host's family image inside its card frame after layout.
#[allow(
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "fit per-family render targets inside each card after layout"
)]
pub(super) fn bind(
    mut commands: Commands,
    mut hosts: Query<(
        Entity,
        &BrawlerCardPreview,
        &ChildOf,
        &mut Node,
        Option<&ImageNode>,
    )>,
    frames: Query<&ComputedNode>,
    images: Res<Assets<Image>>,
    targets: Res<Targets>,
) {
    for (host, preview, parent, mut node, image_node) in &mut hosts {
        let Some(target) = targets.0[preview.0.index()].as_ref() else {
            continue;
        };
        let Some(image) = images.get(&target.image) else {
            continue;
        };
        if let Ok(frame) = frames.get(parent.parent()) {
            let bounds = frame.size() * frame.inverse_scale_factor();
            let size = super::contained_preview_size(image.size().as_vec2(), bounds);
            let (width, height) = (px(size.x), px(size.y));
            // Only write on change so a settled card does not dirty layout every frame.
            if node.width != width || node.height != height {
                node.width = width;
                node.height = height;
            }
        }
        if image_node.is_none_or(|current| current.image != target.image) {
            commands
                .entity(host)
                .insert(ImageNode::new(target.image.clone()));
        }
    }
}

/// Releases every family's card target, camera, light, and scene.
pub(super) fn release(
    mut commands: Commands,
    mut targets: ResMut<Targets>,
    owned: Query<Entity, With<CardOwned>>,
    mut images: ResMut<Assets<Image>>,
) {
    for entity in &owned {
        commands.entity(entity).try_despawn();
    }
    for target in targets.0.iter_mut().filter_map(Option::take) {
        commands.entity(target.camera).try_despawn();
        images.remove(target.image.id());
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "the scene-ready boundary validates and binds one imported card hierarchy"
)]
fn ready(
    event: On<WorldInstanceReady>,
    mut commands: Commands,
    imported: Option<Res<Imported3dAssets>>,
    scenes: Query<&CardScene>,
    children: Query<&Children>,
    names: Query<&Name>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let Ok(scene) = scenes.get(event.entity) else {
        return;
    };
    let family = scene.0;
    let descendants = children.iter_descendants(event.entity).collect::<Vec<_>>();
    for entity in &descendants {
        commands.entity(*entity).insert(layer(family));
    }
    let Some(imported) = imported else {
        return;
    };
    // Only the Riff scene has the weapon socket; the held weapon scene stops here.
    let Some((socket, player)) = find_socket_and_player(&descendants, &names, &players) else {
        return;
    };
    commands.spawn((
        CardScene(family),
        player_preview_toon_scene(),
        weapon_attachment(&imported, family),
        layer(family),
        ChildOf(socket),
    ));
    if let Ok(mut animation_player) = players.get_mut(player) {
        let mut transitions = AnimationTransitions::new();
        transitions
            .play(&mut animation_player, imported.clips.idle, Duration::ZERO)
            .repeat();
        commands
            .entity(player)
            .insert(AnimationGraphHandle(imported.animation_graph.clone()))
            .insert(transitions);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<Targets>()
            .insert_resource(WorldLookCatalog::embedded().expect("embedded world look"))
            .add_systems(Update, (ensure, bind).chain());
        app
    }

    fn card(app: &mut App, family: WeaponFamily) -> Entity {
        let frame = app.world_mut().spawn(ComputedNode::default()).id();
        app.world_mut()
            .spawn((BrawlerCardPreview(family), Node::default(), ChildOf(frame)))
            .id()
    }

    fn image(app: &App, host: Entity) -> Handle<Image> {
        app.world()
            .get::<ImageNode>(host)
            .expect("bound card image")
            .image
            .clone()
    }

    fn live_targets(app: &mut App) -> (usize, usize) {
        let targets = app.world().resource::<Targets>().0.iter().flatten().count();
        let cameras = app
            .world_mut()
            .query_filtered::<(), With<BrawlerCardCamera>>()
            .iter(app.world())
            .count();
        (targets, cameras)
    }

    #[test]
    fn card_layers_are_distinct_and_clear_of_shell_layers() {
        let mut layers = LAYERS.to_vec();
        layers.sort_unstable();
        layers.dedup();
        assert_eq!(layers.len(), WeaponFamily::ALL.len());
        for layer in LAYERS {
            assert!(!(18..=24).contains(&layer), "map palette preview layers");
            assert!(![28, 30, 32, 33].contains(&layer), "weapon preview layers");
            assert_ne!(layer, super::super::DASHBOARD_RENDER_LAYER);
            assert_ne!(layer, 31, "world HUD camera layer");
        }
    }

    #[test]
    fn cards_of_different_families_bind_different_images_and_same_family_shares() {
        let mut app = app();
        let first = card(&mut app, WeaponFamily::OneHanded);
        let second = card(&mut app, WeaponFamily::OneHanded);
        let heavy = card(&mut app, WeaponFamily::Heavy);
        app.update();
        app.update();
        assert_eq!(image(&app, first), image(&app, second));
        assert_ne!(image(&app, first), image(&app, heavy));
        assert_eq!(live_targets(&mut app), (2, 2));
    }

    #[test]
    fn card_targets_never_exceed_one_per_family() {
        let mut app = app();
        for family in WeaponFamily::ALL.into_iter().cycle().take(12) {
            card(&mut app, family);
        }
        for _ in 0..3 {
            app.update();
            assert_eq!(live_targets(&mut app), (4, 4));
        }
        let mut hosts = app.world_mut().query::<(&BrawlerCardPreview, &ImageNode)>();
        let mut images = hosts
            .iter(app.world())
            .map(|(_, node)| node.image.id())
            .collect::<Vec<_>>();
        images.sort_unstable();
        images.dedup();
        assert_eq!(images.len(), WeaponFamily::ALL.len());
    }

    #[test]
    fn card_targets_return_to_baseline_after_the_list_closes() {
        let mut app = app();
        let image_baseline = app.world().resource::<Assets<Image>>().len();
        for _ in 0..10 {
            let hosts = [
                card(&mut app, WeaponFamily::Melee),
                card(&mut app, WeaponFamily::Throwable),
            ];
            app.update();
            assert_eq!(live_targets(&mut app), (2, 2));
            assert_eq!(
                app.world().resource::<Assets<Image>>().len(),
                image_baseline + 2
            );
            // Leaving one family releases only that family.
            app.world_mut().despawn(hosts[0]);
            app.update();
            assert_eq!(live_targets(&mut app), (1, 1));
            app.world_mut().despawn(hosts[1]);
            app.update();
            assert_eq!(live_targets(&mut app), (0, 0));
            assert_eq!(
                app.world().resource::<Assets<Image>>().len(),
                image_baseline
            );
            assert_eq!(
                app.world_mut()
                    .query_filtered::<(), With<CardOwned>>()
                    .iter(app.world())
                    .count(),
                0
            );
        }
        // Leaving the Dashboard with cards still on screen releases everything.
        card(&mut app, WeaponFamily::Heavy);
        app.update();
        app.world_mut().run_system_once(release).unwrap();
        assert_eq!(live_targets(&mut app), (0, 0));
        assert_eq!(
            app.world().resource::<Assets<Image>>().len(),
            image_baseline
        );
    }
}
