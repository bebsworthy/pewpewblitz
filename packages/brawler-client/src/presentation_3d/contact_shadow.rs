//! Soft blob contact shadows that ground fighters, projectiles, and props (BRL-0152).
//!
//! A visual root that should sit on the floor carries a [`GroundContactShadow`]; when it appears,
//! [`attach_contact_shadows`] gives it one child blob: a shared soft-disc mesh scaled to the
//! object's footprint and drawn just above the floor with a dark, unlit, alpha-blended material per
//! class. As a child the blob follows the root's pose and is despawned with it, so destroyed cover,
//! despawned fighters, and unreplicated (concealed) enemies have no shadow. A locally concealed
//! ally hides its blob exactly like its outline. Static walls and indestructible blockers are
//! grounded by the baked floor darkening in `floor_tiles.rs` instead. Tuning lives in
//! `assets/catalogs/contact_shadows.ron`; everything here is client presentation outside the
//! gameplay fingerprint.

use super::{
    AuthoritativeTick, CombatVisualOwner, Controlled, Fighter, Indices, NotShadowCaster,
    NotShadowReceiver, PrimitiveTopology, RenderAssetUsages, V3FighterVisual, combat,
    environment_assets, floor_tiles, ground_position,
};
use bevy::prelude::*;
use serde::Deserialize;

const CONTACT_SHADOW_CATALOG: &str =
    include_str!("../../../../assets/catalogs/contact_shadows.ron");
const CONTACT_SHADOW_SCHEMA_VERSION: u16 = 1;
/// A shadow tint must stay dark; a bright colour would read as a glow rather than contact.
const MAX_SHADOW_CHANNEL: f32 = 0.35;
/// Above this the blob reads as a hole in the floor and hides floor-borne information.
const MAX_SHADOW_OPACITY: f32 = 0.8;
const MIN_SHADOW_SIZE: f32 = 0.5;
const MAX_SHADOW_SIZE: f32 = 2.5;
const MAX_SHADOW_CORE: f32 = 0.9;
/// Height of the blob above its root's ground: above the floor surface (0.25), below the opaque
/// relationship rings and effect overlays (1.0).
const CONTACT_SHADOW_LIFT: f32 = 0.6;
const DISC_SEGMENTS: u16 = 32;
/// Rings from the opaque core to the transparent edge; enough for a smooth gradient.
const FALLOFF_RINGS: u16 = 4;

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct ContactShadowSource {
    schema_version: u16,
    color: (f32, f32, f32),
    core: f32,
    fighter: ContactShadowClassTuning,
    projectile: ContactShadowClassTuning,
    prop: ContactShadowClassTuning,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct ContactShadowClassTuning {
    opacity: f32,
    size: f32,
}

/// Validated contact-shadow tuning, embedded from `assets/catalogs/contact_shadows.ron`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ContactShadowTuning {
    color: (f32, f32, f32),
    core: f32,
    fighter: ContactShadowClassTuning,
    projectile: ContactShadowClassTuning,
    prop: ContactShadowClassTuning,
}

fn check_range(name: &str, value: f32, min: f32, max: f32) -> Result<(), String> {
    if value.is_finite() && (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "contact shadow {name} {value} is outside [{min}, {max}]"
        ))
    }
}

impl ContactShadowTuning {
    pub(super) fn embedded() -> Result<Self, String> {
        Self::from_ron(CONTACT_SHADOW_CATALOG)
    }

    fn from_ron(source: &str) -> Result<Self, String> {
        let source: ContactShadowSource = ron::from_str(source)
            .map_err(|error| format!("contact shadow catalog parse failed: {error}"))?;
        if source.schema_version != CONTACT_SHADOW_SCHEMA_VERSION {
            return Err(format!(
                "contact shadow catalog schema {} is not {CONTACT_SHADOW_SCHEMA_VERSION}",
                source.schema_version
            ));
        }
        for channel in [source.color.0, source.color.1, source.color.2] {
            check_range("color", channel, 0.0, MAX_SHADOW_CHANNEL)?;
        }
        check_range("core", source.core, 0.0, MAX_SHADOW_CORE)?;
        for (name, class) in [
            ("fighter", source.fighter),
            ("projectile", source.projectile),
            ("prop", source.prop),
        ] {
            check_range(
                &format!("{name} opacity"),
                class.opacity,
                0.0,
                MAX_SHADOW_OPACITY,
            )?;
            check_range(
                &format!("{name} size"),
                class.size,
                MIN_SHADOW_SIZE,
                MAX_SHADOW_SIZE,
            )?;
        }
        Ok(Self {
            color: source.color,
            core: source.core,
            fighter: source.fighter,
            projectile: source.projectile,
            prop: source.prop,
        })
    }

    fn class(&self, class: ContactShadowClass) -> ContactShadowClassTuning {
        match class {
            ContactShadowClass::Fighter => self.fighter,
            ContactShadowClass::Projectile => self.projectile,
            ContactShadowClass::Prop => self.prop,
        }
    }
}

/// Which tuning a contact shadow uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ContactShadowClass {
    /// Fighters and zombies.
    Fighter,
    /// Straight projectiles in flight.
    Projectile,
    /// Map props, deployables, and objective objects.
    Prop,
}

/// Requests one soft contact shadow under this visual root.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(super) struct GroundContactShadow {
    /// Untuned half-extents on the root's local X and Z axes.
    half_extents: Vec2,
    class: ContactShadowClass,
    /// Local height of the ground below the root's origin (negative for airborne roots).
    ground_offset: f32,
}

impl GroundContactShadow {
    /// Under a fighter or zombie with the authoritative fighter radius.
    pub(super) fn fighter(radius: f32) -> Self {
        Self {
            half_extents: Vec2::splat(radius),
            class: ContactShadowClass::Fighter,
            ground_offset: 0.0,
        }
    }

    /// Under a projectile of `radius` whose root flies `height` above the ground.
    pub(super) fn projectile(radius: f32, height: f32) -> Self {
        Self {
            half_extents: Vec2::splat(radius),
            class: ContactShadowClass::Projectile,
            ground_offset: -height,
        }
    }

    /// Under a ground prop with these local half-extents.
    pub(super) fn prop(half_extents: Vec2) -> Self {
        Self {
            half_extents,
            class: ContactShadowClass::Prop,
            ground_offset: 0.0,
        }
    }

    /// Under a map prop filling `footprint` cells (unrotated; the root carries the rotation).
    pub(super) fn map_footprint(footprint: brawler::map::MapFootprint) -> Self {
        Self::prop(
            Vec2::new(f32::from(footprint.width), f32::from(footprint.height))
                * (brawler::map::MAP_CELL_SIZE_WORLD * 0.5),
        )
    }
}

/// The blob child spawned for a [`GroundContactShadow`].
#[derive(Component)]
pub(super) struct ContactShadowBlob;

/// Shared blob mesh and per-class materials; a disabled class has no material.
#[derive(Resource)]
pub(super) struct ContactShadowAssets {
    mesh: Handle<Mesh>,
    materials: [Option<Handle<StandardMaterial>>; 3],
    sizes: [f32; 3],
}

const fn class_index(class: ContactShadowClass) -> usize {
    match class {
        ContactShadowClass::Fighter => 0,
        ContactShadowClass::Projectile => 1,
        ContactShadowClass::Prop => 2,
    }
}

impl ContactShadowAssets {
    fn new(
        tuning: &ContactShadowTuning,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
    ) -> Self {
        let classes = [
            ContactShadowClass::Fighter,
            ContactShadowClass::Projectile,
            ContactShadowClass::Prop,
        ];
        Self {
            mesh: meshes.add(soft_disc_mesh(tuning.core)),
            materials: classes.map(|class| {
                let opacity = tuning.class(class).opacity;
                (opacity > 0.0).then(|| {
                    materials.add(StandardMaterial {
                        base_color: Color::srgba(
                            tuning.color.0,
                            tuning.color.1,
                            tuning.color.2,
                            opacity,
                        ),
                        alpha_mode: AlphaMode::Blend,
                        unlit: true,
                        ..default()
                    })
                })
            }),
            sizes: classes.map(|class| tuning.class(class).size),
        }
    }
}

/// Smooth falloff from 1 inside `core` to 0 at the unit radius.
fn falloff_alpha(radius: f32, core: f32) -> f32 {
    if radius <= core {
        return 1.0;
    }
    let t = ((radius - core) / (1.0 - core).max(f32::EPSILON)).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

/// Unit soft disc in the XZ plane facing +Y, with its opacity gradient in linear vertex alpha.
pub(super) fn soft_disc_mesh(core: f32) -> Mesh {
    let rings: Vec<f32> = std::iter::once(core.max(1.0e-3))
        .chain(
            (1..=FALLOFF_RINGS)
                .map(|ring| core + (1.0 - core) * f32::from(ring) / f32::from(FALLOFF_RINGS)),
        )
        .collect();
    let mut positions = vec![[0.0, 0.0, 0.0]];
    let mut colors = vec![[1.0, 1.0, 1.0, 1.0]];
    for &radius in &rings {
        let alpha = falloff_alpha(radius, core);
        for segment in 0..DISC_SEGMENTS {
            let angle = core::f32::consts::TAU * f32::from(segment) / f32::from(DISC_SEGMENTS);
            // Increasing angle runs counter-clockwise seen from +Y (simulation +y is world -z).
            positions.push([radius * angle.cos(), 0.0, -radius * angle.sin()]);
            colors.push([1.0, 1.0, 1.0, alpha]);
        }
    }
    let ring_start = |ring: u16| 1 + ring * DISC_SEGMENTS;
    let mut indices = Vec::new();
    for segment in 0..DISC_SEGMENTS {
        let next = (segment + 1) % DISC_SEGMENTS;
        indices.extend([0, ring_start(0) + segment, ring_start(0) + next]);
    }
    let ring_count = u16::try_from(rings.len()).expect("a handful of rings");
    for ring in 0..ring_count - 1 {
        for segment in 0..DISC_SEGMENTS {
            let next = (segment + 1) % DISC_SEGMENTS;
            let (inner, outer) = (ring_start(ring), ring_start(ring + 1));
            indices.extend([
                inner + segment,
                outer + segment,
                outer + next,
                inner + segment,
                outer + next,
                inner + next,
            ]);
        }
    }
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    let uvs: Vec<[f32; 2]> = positions
        .iter()
        .map(|position| [position[0] * 0.5 + 0.5, position[2] * 0.5 + 0.5])
        .collect();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_indices(Indices::U16(indices));
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh
}

/// Whether a map placement stands on the floor and is grounded by a contact shadow: every
/// feature or decoration except effect tiles, props authored as flat, and the static walls and
/// blockers that the baked floor darkening already grounds.
pub(super) fn map_prop_casts_contact_shadow(
    asset: &brawler::map::MapAssetDefinition,
    profile: &brawler::map::MapGameplayProfile,
    visual: Option<&environment_assets::MapVisualProfile>,
) -> bool {
    matches!(
        asset.slot,
        brawler::map::MapAssetSlot::Feature | brawler::map::MapAssetSlot::Decoration
    ) && profile.effect_tile_definition_id.is_none()
        && !floor_tiles::grounded_by_floor_darkening(asset, profile)
        && !visual.is_some_and(|visual| visual.flat)
}

/// Spawns a marker-owned shadow root under a static map prop that needs a contact shadow.
pub(super) fn spawn_static_prop_shadow(
    commands: &mut Commands,
    marker: brawler::map::MapPresentationMember,
    asset: &brawler::map::MapAssetDefinition,
    profile: &brawler::map::MapGameplayProfile,
    visual: Option<&environment_assets::MapVisualProfile>,
    center: Vec2,
    rotation: f32,
) {
    if !map_prop_casts_contact_shadow(asset, profile, visual) {
        return;
    }
    commands.spawn((
        marker,
        GroundContactShadow::map_footprint(asset.footprint_cells),
        Transform {
            translation: ground_position(center),
            rotation: Quat::from_rotation_y(rotation),
            ..default()
        },
        Visibility::default(),
        Name::new("static map prop contact shadow"),
    ));
}

fn prepare_contact_shadow_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let tuning =
        ContactShadowTuning::embedded().expect("the embedded contact shadow catalog must be valid");
    commands.insert_resource(ContactShadowAssets::new(
        &tuning,
        &mut meshes,
        &mut materials,
    ));
}

/// Gives every newly requested contact shadow its blob child.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system resource parameter"
)]
pub(super) fn attach_contact_shadows(
    mut commands: Commands,
    assets: Res<ContactShadowAssets>,
    roots: Query<(Entity, &GroundContactShadow), Added<GroundContactShadow>>,
) {
    for (root, shadow) in &roots {
        let index = class_index(shadow.class);
        let Some(material) = assets.materials[index].clone() else {
            continue;
        };
        let extents = shadow.half_extents * assets.sizes[index];
        commands.spawn((
            ContactShadowBlob,
            Mesh3d(assets.mesh.clone()),
            MeshMaterial3d(material),
            NotShadowCaster,
            NotShadowReceiver,
            Transform::from_xyz(0.0, shadow.ground_offset + CONTACT_SHADOW_LIFT, 0.0)
                .with_scale(Vec3::new(extents.x, 1.0, extents.y)),
            Visibility::Inherited,
            ChildOf(root),
            Name::new("Soft contact shadow"),
        ));
    }
}

/// Hides the contact shadow of a locally concealed ally (or the local fighter), like its outline,
/// so the translucent ghost still reads as concealed.
#[allow(
    clippy::type_complexity,
    reason = "the pass reads the same concealment facts as the fighter outline"
)]
pub(super) fn update_fighter_contact_shadows(
    roots: Query<(&CombatVisualOwner, &Children), With<V3FighterVisual>>,
    fighters: Query<
        (
            Option<&brawler::concealment::ConcealmentPresentationState>,
            Option<&AuthoritativeTick>,
            &brawler::combat::TeamId,
            Has<Controlled>,
        ),
        With<Fighter>,
    >,
    mut blobs: Query<&mut Visibility, With<ContactShadowBlob>>,
) {
    let controlled_team = fighters
        .iter()
        .find_map(|(_, _, team, controlled)| controlled.then_some(*team));
    for (owner, children) in &roots {
        let Ok((concealment, tick, team, _)) = fighters.get(owner.0) else {
            continue;
        };
        let concealed = controlled_team == Some(*team)
            && combat::fighter_is_visually_concealed(
                concealment,
                tick.map_or(0, |authoritative_tick| authoritative_tick.0),
                controlled_team,
            );
        let desired = if concealed {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        for child in children.iter() {
            if let Ok(mut visibility) = blobs.get_mut(child) {
                visibility.set_if_neq(desired);
            }
        }
    }
}

pub(super) fn install(app: &mut App) {
    app.add_systems(Startup, prepare_contact_shadow_assets);
}

#[cfg(test)]
mod tests;
