//! Dark silhouette outline around fighters (BRL-0151).
//!
//! Each toon mesh under a fighter visual root gets one child hull: a smoothed-normal copy of its
//! mesh drawn with [`FighterOutlineMaterial`] (front faces culled, extruded in screen space by
//! `assets/shaders/fighter_outline.wgsl`). The hull is a child of the body mesh, so it inherits
//! the body's transform, animation, and visibility, and it is despawned with the fighter visual,
//! which exists only while the observer may see the replicated fighter. A locally concealed ally
//! (translucent body) hides its hull so the ghost reads as concealed. The hull colour comes from
//! the relationship tag the toon rim already writes. Shell previews (Dashboard, brawler cards)
//! mark their Riff scene root with [`PreviewOutline`] and get the same hulls on the preview's
//! render layer at a preview width (BRL-y3hb). Tuning lives in
//! `assets/catalogs/fighter_outline.ron`; everything here is client presentation outside the
//! gameplay fingerprint.

use super::super::{
    AuthoritativeTick, CombatVisualOwner, Controlled, Fighter, NotShadowCaster, NotShadowReceiver,
    PrimitiveTopology, RenderAssetUsages, RenderLayers, RimRelation, ToonMaterial, ToonShading,
    V3FighterVisual,
};
use super::fighter_feedback::fighter_is_visually_concealed;
use bevy::prelude::*;
use bevy::{
    camera::visibility::NoFrustumCulling,
    mesh::{MeshTag, MeshVertexBufferLayoutRef, VertexAttributeValues, skinning::SkinnedMesh},
    pbr::{MaterialPipeline, MaterialPipelineKey},
    render::render_resource::{
        AsBindGroup, Face, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
    },
    shader::ShaderRef,
};
use serde::Deserialize;
use std::collections::{HashMap, hash_map::Entry};

const FIGHTER_OUTLINE_CATALOG: &str =
    include_str!("../../../../../assets/catalogs/fighter_outline.ron");
const FIGHTER_OUTLINE_SCHEMA_VERSION: u16 = 2;
const FIGHTER_OUTLINE_SHADER_PATH: &str = "shaders/fighter_outline.wgsl";
/// Outline width bounds in world units (a fighter is about 43 units tall); thicker outlines read
/// as a black border rather than a silhouette accent.
const MIN_OUTLINE_WIDTH_WORLD: f32 = 0.3;
const MAX_OUTLINE_WIDTH_WORLD: f32 = 6.0;
/// The outline must stay dark against every floor; a bright colour would read as a highlight.
const MAX_OUTLINE_CHANNEL: f32 = 0.35;
/// Distinct fighter mesh assets that get a hull copy; past the ceiling a mesh stays unoutlined.
const MAX_OUTLINE_HULL_MESHES: usize = 64;
/// World units the arena hull is pushed away from the camera, so where body parts meet (neck,
/// hands on the blaster) the shell stays behind the touching surfaces instead of poking dark
/// specks through.
const HULL_DEPTH_PUSH_WORLD: f32 = 2.0;
/// The preview Riff is scaled about 1.8x an arena fighter, so its face details and pockets stand
/// proportionally further off the body; a deeper push keeps their hulls behind the head and
/// torso, so only the silhouette and real overlaps (ears, goggles, arms) are outlined.
const PREVIEW_HULL_DEPTH_PUSH_WORLD: f32 = 6.0;
/// Positions closer than this (in mesh units) share one smoothed outline normal.
const HULL_WELD_QUANTUM: f32 = 1.0e-4;

/// Flat-colour inverted-hull outline material, shared by every fighter hull.
#[derive(Asset, AsBindGroup, TypePath, Clone, Copy, Debug, PartialEq)]
pub(crate) struct FighterOutlineMaterial {
    #[uniform(0)]
    settings: FighterOutlineSettings,
}

#[derive(ShaderType, Clone, Copy, Debug, PartialEq)]
struct FighterOutlineSettings {
    /// Linear outline colour per mesh tag: untagged, local, ally, enemy.
    colors: [Vec4; 4],
    /// Width in world units at the fighter's depth (x) and the hull's push away from the camera
    /// in world units (y).
    width: Vec4,
}

impl Material for FighterOutlineMaterial {
    fn vertex_shader() -> ShaderRef {
        FIGHTER_OUTLINE_SHADER_PATH.into()
    }

    fn fragment_shader() -> ShaderRef {
        FIGHTER_OUTLINE_SHADER_PATH.into()
    }

    // The hull never casts or receives shadows and never writes the prepass (BRL-0140 input).
    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let mut attributes = vec![
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
        ];
        // The mesh pipeline already defines `SKINNED` for a jointed layout; the shader then
        // skins the hull with the body's joint matrices.
        if layout.0.contains(Mesh::ATTRIBUTE_JOINT_INDEX)
            && layout.0.contains(Mesh::ATTRIBUTE_JOINT_WEIGHT)
        {
            attributes.push(Mesh::ATTRIBUTE_JOINT_INDEX.at_shader_location(2));
            attributes.push(Mesh::ATTRIBUTE_JOINT_WEIGHT.at_shader_location(3));
        }
        descriptor.vertex.buffers = vec![layout.0.get_layout(&attributes)?];
        // Inverted hull: only the far side of the extruded shell is drawn, so the body covers it
        // everywhere except the silhouette band.
        descriptor.primitive.cull_mode = Some(Face::Front);
        Ok(())
    }
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct FighterOutlineSource {
    schema_version: u16,
    width_world: f32,
    preview_width_world: f32,
    color: (f32, f32, f32),
    relationship_tint: f32,
}

/// Validated outline tuning, embedded from `assets/catalogs/fighter_outline.ron`.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub(crate) struct FighterOutlineTuning {
    width_world: f32,
    preview_width_world: f32,
    color: Srgba,
    relationship_tint: f32,
}

impl FighterOutlineTuning {
    pub(crate) fn embedded() -> Result<Self, String> {
        Self::from_ron(FIGHTER_OUTLINE_CATALOG)
    }

    fn from_ron(source: &str) -> Result<Self, String> {
        let source: FighterOutlineSource = ron::from_str(source)
            .map_err(|error| format!("fighter outline catalog parse failed: {error}"))?;
        if source.schema_version != FIGHTER_OUTLINE_SCHEMA_VERSION {
            return Err(format!(
                "fighter outline catalog schema {} is not {FIGHTER_OUTLINE_SCHEMA_VERSION}",
                source.schema_version
            ));
        }
        check_range(
            "width_world",
            source.width_world,
            MIN_OUTLINE_WIDTH_WORLD,
            MAX_OUTLINE_WIDTH_WORLD,
        )?;
        check_range(
            "preview_width_world",
            source.preview_width_world,
            MIN_OUTLINE_WIDTH_WORLD,
            MAX_OUTLINE_WIDTH_WORLD,
        )?;
        for channel in [source.color.0, source.color.1, source.color.2] {
            check_range("color", channel, 0.0, MAX_OUTLINE_CHANNEL)?;
        }
        check_range("relationship_tint", source.relationship_tint, 0.0, 1.0)?;
        Ok(Self {
            width_world: source.width_world,
            preview_width_world: source.preview_width_world,
            color: Srgba::rgb(source.color.0, source.color.1, source.color.2),
            relationship_tint: source.relationship_tint,
        })
    }

    /// The arena outline material. With [`Self::preview_material`], the single place outline
    /// shading is chosen, so a later quality tier (BRL-0161) can thin or drop outlines here.
    pub(crate) fn material(&self, shading: &ToonShading) -> FighterOutlineMaterial {
        self.material_at(shading, self.width_world, HULL_DEPTH_PUSH_WORLD)
    }

    /// The shell preview outline: the same colours at the preview width, since the preview Riff
    /// is scaled up and framed far closer than an arena fighter.
    pub(crate) fn preview_material(&self, shading: &ToonShading) -> FighterOutlineMaterial {
        self.material_at(
            shading,
            self.preview_width_world,
            PREVIEW_HULL_DEPTH_PUSH_WORLD,
        )
    }

    fn material_at(
        &self,
        shading: &ToonShading,
        width_world: f32,
        depth_push_world: f32,
    ) -> FighterOutlineMaterial {
        let base = LinearRgba::from(self.color);
        let tinted = |relation: RimRelation| {
            let rim = shading.rim_rgb(relation);
            let rim = LinearRgba::from(Srgba::rgb(rim[0], rim[1], rim[2]));
            base.mix(&rim, self.relationship_tint).to_vec4().with_w(1.0)
        };
        FighterOutlineMaterial {
            settings: FighterOutlineSettings {
                colors: [
                    base.to_vec4().with_w(1.0),
                    tinted(RimRelation::Local),
                    tinted(RimRelation::Ally),
                    tinted(RimRelation::Enemy),
                ],
                width: Vec4::new(width_world, depth_push_world, 0.0, 0.0),
            },
        }
    }
}

fn check_range(name: &str, value: f32, min: f32, max: f32) -> Result<(), String> {
    if value.is_finite() && (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "fighter outline {name} = {value} is outside {min}..={max}"
        ))
    }
}

/// The shared outline materials and the hull mesh made for each fighter mesh asset.
#[derive(Resource, Default)]
pub(crate) struct FighterOutlineAssets {
    material: Handle<FighterOutlineMaterial>,
    preview_material: Handle<FighterOutlineMaterial>,
    /// `None` records a mesh that cannot carry a hull (not a triangle list, skinned, no normals).
    hulls: HashMap<AssetId<Mesh>, Option<Handle<Mesh>>>,
}

/// Marks a body mesh that already owns its outline hull child.
#[derive(Component)]
pub(crate) struct FighterOutlined;

/// The outline hull child of one fighter body mesh.
#[derive(Component)]
pub(crate) struct FighterOutlineHull;

/// The root of a shell preview fighter scene (Dashboard or brawler-card Riff) whose toon meshes,
/// held weapon included, get preview outline hulls. The root's `RenderLayers` is the preview's
/// private layer; hulls carry it explicitly because render layers are not inherited.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct PreviewOutline;

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are passed by value"
)]
pub(crate) fn prepare_fighter_outline_material(
    tuning: Res<FighterOutlineTuning>,
    shading: Res<ToonShading>,
    mut materials: ResMut<Assets<FighterOutlineMaterial>>,
    mut assets: ResMut<FighterOutlineAssets>,
) {
    assets.material = materials.add(tuning.material(&shading));
    assets.preview_material = materials.add(tuning.preview_material(&shading));
}

/// A copy of `source` with only positions, one smoothed normal per welded position, the joint
/// attributes of a skinned mesh, and the original indices. Flat-shaded meshes have split normals at
/// every hard edge; extruding along those would crack the hull at each corner.
fn smoothed_hull_mesh(source: &Mesh) -> Option<Mesh> {
    if source.primitive_topology() != PrimitiveTopology::TriangleList {
        return None;
    }
    let Some(VertexAttributeValues::Float32x3(positions)) =
        source.try_attribute_option(Mesh::ATTRIBUTE_POSITION).ok()?
    else {
        return None;
    };
    let Some(VertexAttributeValues::Float32x3(normals)) =
        source.try_attribute_option(Mesh::ATTRIBUTE_NORMAL).ok()?
    else {
        return None;
    };
    let weld = |position: &[f32; 3]| {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "fighter mesh coordinates are a few units; the quantised key fits i32"
        )]
        position.map(|axis| (axis / HULL_WELD_QUANTUM).round() as i32)
    };
    let mut welded = HashMap::<[i32; 3], Vec3>::with_capacity(positions.len());
    for (position, normal) in positions.iter().zip(normals) {
        *welded.entry(weld(position)).or_default() += Vec3::from(*normal);
    }
    let smoothed = positions
        .iter()
        .zip(normals)
        .map(|(position, normal)| {
            welded[&weld(position)]
                .try_normalize()
                .unwrap_or_else(|| Vec3::from(*normal))
                .to_array()
        })
        .collect::<Vec<_>>();
    let mut hull = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, smoothed);
    let joints = (
        source
            .try_attribute_option(Mesh::ATTRIBUTE_JOINT_INDEX)
            .ok()?,
        source
            .try_attribute_option(Mesh::ATTRIBUTE_JOINT_WEIGHT)
            .ok()?,
    );
    match joints {
        (Some(indices), Some(weights)) => {
            hull.insert_attribute(Mesh::ATTRIBUTE_JOINT_INDEX, indices.clone());
            hull.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, weights.clone());
        }
        (None, None) => {}
        _ => return None,
    }
    if let Some(indices) = source.try_indices_option().ok()? {
        hull.insert_indices(indices.clone());
    }
    Some(hull)
}

fn hull_for(
    source: &Handle<Mesh>,
    meshes: &mut Assets<Mesh>,
    assets: &mut FighterOutlineAssets,
) -> Option<Handle<Mesh>> {
    let at_capacity = assets.hulls.len() >= MAX_OUTLINE_HULL_MESHES;
    match assets.hulls.entry(source.id()) {
        Entry::Occupied(entry) => entry.get().clone(),
        Entry::Vacant(entry) => {
            if at_capacity {
                return None;
            }
            // A mesh that is not loaded yet is retried on a later frame.
            let hull = smoothed_hull_mesh(meshes.get(source)?).map(|hull| meshes.add(hull));
            entry.insert(hull).clone()
        }
    }
}

/// Gives every toon body mesh under a fighter visual root its outline hull, and hides the hulls
/// of a locally concealed ally.
#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "the pass discovers body meshes across imported and fallback fighter hierarchies and reads the same concealment facts as the body-material writer"
)]
pub(in super::super) fn attach_fighter_outlines(
    mut commands: Commands,
    roots: Query<(Entity, &CombatVisualOwner), With<V3FighterVisual>>,
    children: Query<&Children>,
    fighters: Query<
        (
            Option<&brawler::concealment::ConcealmentPresentationState>,
            Option<&AuthoritativeTick>,
            &brawler::combat::TeamId,
            Has<Controlled>,
        ),
        With<Fighter>,
    >,
    bodies: Query<
        (&Mesh3d, Option<&SkinnedMesh>),
        (With<MeshMaterial3d<ToonMaterial>>, Without<FighterOutlined>),
    >,
    mut hulls: Query<&mut Visibility, With<FighterOutlineHull>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut assets: ResMut<FighterOutlineAssets>,
) {
    let controlled_team = fighters
        .iter()
        .find_map(|(_, _, team, controlled)| controlled.then_some(*team));
    for (root, owner) in &roots {
        let Ok((concealment, tick, team, _)) = fighters.get(owner.0) else {
            continue;
        };
        let concealed = controlled_team == Some(*team)
            && fighter_is_visually_concealed(
                concealment,
                tick.map_or(0, |authoritative_tick| authoritative_tick.0),
                controlled_team,
            );
        let desired = if concealed {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        for entity in children.iter_descendants(root) {
            if let Ok(mut visibility) = hulls.get_mut(entity) {
                visibility.set_if_neq(desired);
                continue;
            }
            let Ok((body, skin)) = bodies.get(entity) else {
                continue;
            };
            let Some(hull) = hull_for(&body.0, &mut meshes, &mut assets) else {
                if assets.hulls.contains_key(&body.0.id()) {
                    commands.entity(entity).try_insert(FighterOutlined);
                }
                continue;
            };
            spawn_hull(
                &mut commands,
                entity,
                hull,
                assets.material.clone(),
                skin,
                desired,
            );
        }
    }
}

/// Marks `body` outlined and spawns its hull child.
fn spawn_hull<'a>(
    commands: &'a mut Commands,
    body: Entity,
    hull: Handle<Mesh>,
    material: Handle<FighterOutlineMaterial>,
    skin: Option<&SkinnedMesh>,
    visibility: Visibility,
) -> EntityCommands<'a> {
    commands.entity(body).try_insert(FighterOutlined);
    let mut hull_entity = commands.spawn((
        FighterOutlineHull,
        Mesh3d(hull),
        MeshMaterial3d(material),
        NotShadowCaster,
        NotShadowReceiver,
        Transform::IDENTITY,
        visibility,
        ChildOf(body),
        Name::new("Fighter silhouette outline hull"),
    ));
    if let Some(skin) = skin {
        // The hull is skinned by the body's joints; its bind-pose bounds would cull it while the
        // animated body is still on screen.
        hull_entity.insert((skin.clone(), NoFrustumCulling));
    }
    hull_entity
}

/// Gives every toon mesh under a [`PreviewOutline`] root (body and held weapon) its preview hull
/// on the root's render layer. Runs in every flow; with no preview on screen it visits nothing.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Bevy system parameters are passed by value; the body query names its filters"
)]
pub(in super::super) fn attach_preview_outlines(
    mut commands: Commands,
    roots: Query<(Entity, &RenderLayers), With<PreviewOutline>>,
    children: Query<&Children>,
    bodies: Query<
        (&Mesh3d, Option<&SkinnedMesh>, Option<&MeshTag>),
        (With<MeshMaterial3d<ToonMaterial>>, Without<FighterOutlined>),
    >,
    mut meshes: ResMut<Assets<Mesh>>,
    mut assets: ResMut<FighterOutlineAssets>,
) {
    for (root, layers) in &roots {
        for entity in children.iter_descendants(root) {
            let Ok((body, skin, tag)) = bodies.get(entity) else {
                continue;
            };
            let Some(hull) = hull_for(&body.0, &mut meshes, &mut assets) else {
                if assets.hulls.contains_key(&body.0.id()) {
                    commands.entity(entity).try_insert(FighterOutlined);
                }
                continue;
            };
            let material = assets.preview_material.clone();
            let mut hull = spawn_hull(
                &mut commands,
                entity,
                hull,
                material,
                skin,
                Visibility::Inherited,
            );
            hull.insert(layers.clone());
            // The fixed preview rim tag, so an optional relationship tint matches the body rim.
            if let Some(tag) = tag {
                hull.insert(tag.clone());
            }
        }
    }
}

pub(in super::super) fn install(app: &mut App) {
    app.add_plugins(MaterialPlugin::<FighterOutlineMaterial>::default())
        .insert_resource(
            FighterOutlineTuning::embedded()
                .expect("the embedded fighter outline catalog must be valid"),
        )
        .init_resource::<FighterOutlineAssets>()
        .add_systems(Startup, prepare_fighter_outline_material);
}

#[cfg(test)]
mod tests;
