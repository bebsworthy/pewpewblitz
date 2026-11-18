//! Shell weapon previews: one offscreen camera and render layer per weapon family on screen, so
//! each preview host shows its own primary weapon family's model (BRL-q29d).
use crate::flow::WeaponPreviewHost;
use crate::presentation_3d::{
    FighterWeaponCatalog, Imported3dAssets, ToonScene, WeaponFamily, WorldLookCatalog,
};
use bevy::{
    asset::RenderAssetUsages,
    camera::{ClearColorConfig, RenderTarget, visibility::RenderLayers},
    prelude::*,
    render::render_resource::{TextureDimension, TextureFormat, TextureUsages},
    ui::widget::ViewportNode,
    world_serialization::{WorldAssetRoot, WorldInstanceReady},
};

/// One private render layer per family, clear of the dashboard (29), HUD (31), and map palette
/// preview (18..=24) layers.
const LAYERS: [usize; 4] = [28, 30, 32, 33];

/// The preview model root; its scene descendants join the family's render layer when ready.
#[derive(Component)]
struct WeaponScene(WeaponFamily);

/// One family's preview camera, light, and model.
#[derive(Component)]
struct WeaponOwned(WeaponFamily);

struct Target {
    image: Handle<Image>,
    camera: Entity,
    host: Entity,
}

/// The live preview target of each family, indexed by `WeaponFamily::index`.
#[derive(Resource, Default)]
struct Targets([Option<Target>; 4]);

pub(super) fn install(app: &mut App) {
    app.init_resource::<Targets>()
        .add_systems(Update, update)
        .add_observer(ready);
}

fn layer(family: WeaponFamily) -> RenderLayers {
    RenderLayers::layer(LAYERS[family.index()])
}

#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "Bevy preview hosts and render target ownership"
)]
fn update(
    mut commands: Commands,
    hosts: Query<(Entity, &WeaponPreviewHost)>,
    thumbnails: Query<(), (Without<ImageNode>, Without<ViewportNode>)>,
    mut targets: ResMut<Targets>,
    owned: Query<(Entity, &WeaponOwned)>,
    mut images: ResMut<Assets<Image>>,
    imported: Option<Res<Imported3dAssets>>,
    weapons: Res<FighterWeaponCatalog>,
    look: Res<WorldLookCatalog>,
) {
    for family in WeaponFamily::ALL {
        let mut family_hosts = hosts
            .iter()
            .filter(|(_, host)| host.0 == family)
            .map(|(entity, _)| entity);
        let host = family_hosts.next();
        let slot = &mut targets.0[family.index()];
        match (slot.as_mut(), host) {
            (Some(target), Some(host)) => {
                // The first host shows the live viewport; any other host of the same family
                // shows a copy of its image.
                for other in family_hosts.filter(|entity| thumbnails.contains(*entity)) {
                    commands
                        .entity(other)
                        .insert(ImageNode::new(target.image.clone()));
                }
                if host != target.host {
                    commands
                        .entity(host)
                        .insert(ViewportNode::new(target.camera));
                    target.host = host;
                }
            }
            (Some(_), None) => {
                for (entity, owner) in &owned {
                    if owner.0 == family {
                        commands.entity(entity).despawn();
                    }
                }
                if let Some(target) = slot.take() {
                    images.remove(target.image.id());
                }
            }
            (None, Some(host)) => {
                if let Some(imported) = imported.as_deref() {
                    *slot = Some(spawn_preview(
                        &mut commands,
                        &mut images,
                        imported,
                        &weapons,
                        &look,
                        family,
                        host,
                    ));
                }
            }
            (None, None) => {}
        }
    }
}

fn spawn_preview(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    imported: &Imported3dAssets,
    weapons: &FighterWeaponCatalog,
    look: &WorldLookCatalog,
    family: WeaponFamily,
    host: Entity,
) -> Target {
    let mut image = Image::new_uninit(
        default(),
        TextureDimension::D2,
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::all(),
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    let image = images.add(image);
    let camera = commands
        .spawn((
            WeaponOwned(family),
            Camera3d::default(),
            crate::presentation_3d::render_quality::PreviewCamera,
            Camera {
                order: -3,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
            RenderTarget::Image(image.clone().into()),
            Projection::Perspective(PerspectiveProjection {
                fov: 0.5,
                ..default()
            }),
            look.preview_camera_look(),
            look.preview_ambient_light(),
            Transform::from_xyz(0.8, 0.32, 0.9).looking_at(Vec3::new(0.0, -0.05, 0.0), Vec3::Y),
            layer(family),
        ))
        .id();
    commands.entity(host).insert(ViewportNode::new(camera));
    commands.spawn((
        WeaponOwned(family),
        WeaponScene(family),
        // Arena toon material without a relationship rim: at this close-up a side rim reads as a
        // coloured fringe on the weapon rather than a silhouette accent (BRL-0179 captures).
        ToonScene,
        WorldAssetRoot(imported.weapon_scenes[family.index()].clone()),
        weapons.preview_transform(family),
        layer(family),
    ));
    // One catalog key light: the toon material bands directional light, so a back fill would put
    // the whole weapon in the lit band (BRL-0179).
    commands.spawn((
        WeaponOwned(family),
        look.preview_key_light(),
        Transform::from_xyz(2.0, 4.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
        layer(family),
    ));
    Target {
        image,
        camera,
        host,
    }
}

#[allow(clippy::needless_pass_by_value)]
fn ready(
    event: On<WorldInstanceReady>,
    mut commands: Commands,
    roots: Query<&WeaponScene>,
    children: Query<&Children>,
) {
    let Ok(scene) = roots.get(event.entity) else {
        return;
    };
    for e in children.iter_descendants(event.entity) {
        commands.entity(e).insert(layer(scene.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_preview_layers_are_distinct_and_clear_of_shell_layers() {
        let mut layers = LAYERS.to_vec();
        layers.sort_unstable();
        layers.dedup();
        assert_eq!(layers.len(), WeaponFamily::ALL.len());
        for layer in LAYERS {
            assert!(!(18..=24).contains(&layer), "map palette preview layers");
            assert_ne!(layer, super::super::DASHBOARD_RENDER_LAYER);
            assert_ne!(layer, 31, "world HUD camera layer");
        }
    }
}
