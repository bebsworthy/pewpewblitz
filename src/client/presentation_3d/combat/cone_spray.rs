use super::super::*;
use super::common::{CombatVfxContext, GROUND_EFFECT_HEIGHT};
use crate::combat::client::{
    AimTraceBlockerClass, AimTraceBlockerIndex, AimTraceDynamicBlocker, MAX_CONE_SPRAY_SEGMENTS,
    PreviewGeometry, cone_spray_primitives,
};
use bevy::{reflect::TypePath, render::render_resource::AsBindGroup, shader::ShaderRef};
use std::collections::{HashMap, HashSet};

const CONE_SPRAY_SHADER_PATH: &str = "shaders/cone_spray.wgsl";

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(in crate::client) struct ConeSprayMaterial {
    #[uniform(0)]
    params: Vec4,
}

impl ConeSprayMaterial {
    pub(in crate::client) const fn normal() -> Self {
        Self {
            params: Vec4::new(0.0, 0.0, 0.0, 0.0),
        }
    }

    pub(in crate::client) const fn reduced() -> Self {
        Self {
            params: Vec4::new(0.0, 1.0, 0.0, 0.0),
        }
    }

    pub(in crate::client) const fn primitive() -> Self {
        Self {
            params: Vec4::new(0.0, 2.0, 0.0, 0.0),
        }
    }
}

impl Material for ConeSprayMaterial {
    fn fragment_shader() -> ShaderRef {
        CONE_SPRAY_SHADER_PATH.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn depth_bias(&self) -> f32 {
        10.0
    }

    fn enable_shadows() -> bool {
        false
    }
}

pub(in crate::client) struct ConeSprayMaterialPlugin;

impl Plugin for ConeSprayMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<ConeSprayMaterial>::default());
    }
}

#[derive(Component)]
pub(in super::super) struct ConeSprayVisual3d;

#[derive(Component)]
pub(in super::super) struct ConeSpraySurfaceVisual3d {
    owner: Entity,
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "cone reconciliation joins one bounded authoritative carrier tuple with its visual root"
)]
pub(in super::super) fn reconcile_cone_spray_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<Material3dAssets>,
    vfx: CombatVfxContext,
    sprays: Query<
        (
            Entity,
            &crate::combat::ConeSprayState,
            Option<&crate::combat::ReplicatedAttackSource>,
            Option<&crate::combat::AttackDelivery>,
        ),
        With<crate::combat::ConeSpray>,
    >,
    visuals: Query<(Entity, &CombatVisualOwner), With<ConeSprayVisual3d>>,
    surface_meshes: Query<(&ConeSpraySurfaceVisual3d, &Mesh3d)>,
) {
    let roots = super::common::unique_roots(&mut commands, &visuals);
    for (owner, state, source, delivery) in &sprays {
        if roots.contains(&owner) {
            continue;
        }
        let geometry = vfx::VfxGeometry::Cone {
            origin: state.origin.as_vec2().to_array(),
            facing: state.facing,
            reach: state.maximum_reach,
            angle_degrees: state.angle_degrees,
        };
        let timing = vfx::VfxAuthoritativeTiming::persistent(
            state.emitted_at_tick,
            None,
            state.expires_at_tick,
        );
        let signature = source.map_or_else(
            || vfx::CombatVisualSignature {
                carrier: vfx::VfxCarrier::Cone,
                phase: vfx::VfxPhase::Active,
                accents: [Some(vfx::VfxAccent::Neutral), None],
                relationship: vfx::VfxRelationship::Neutral,
                context: None,
                geometry,
                identity: vfx::VfxPresentationIdentity::Neutral(state.emitted_at_tick),
                timing,
                variant: vfx.variant(),
            },
            |source| {
                vfx.cache().signature_for_source(
                    source.attack,
                    delivery.map_or(0, |delivery| delivery.delivery_index),
                    vfx::VfxCarrier::Cone,
                    vfx::VfxPhase::Active,
                    geometry,
                    timing,
                    vfx.variant(),
                )
            },
        );
        let root = commands
            .spawn((
                CombatVisualOwner(owner),
                ConeSprayVisual3d,
                signature,
                Transform::default(),
                Visibility::default(),
                Name::new("Spray gas visual root"),
            ))
            .id();
        let spray_material = cone_spray_material(signature.variant, &materials);
        commands.entity(root).with_children(|parent| {
            parent.spawn((
                ConeSpraySurfaceVisual3d { owner },
                Mesh3d(meshes.add(cone_spray_surface_mesh(state.origin.as_vec2(), &[]))),
                MeshMaterial3d(spray_material.clone()),
                NotShadowCaster,
                NotShadowReceiver,
                Transform::default(),
                Visibility::Hidden,
                Name::new("Spray gas clipped surface"),
            ));
        });
    }
    for (root, owner) in &visuals {
        if sprays.get(owner.0).is_err() {
            for (surface, mesh) in &surface_meshes {
                if surface.owner == owner.0 {
                    meshes.remove(mesh.0.id());
                }
            }
            commands.entity(root).despawn();
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "spray presentation reconstructs authoritative time against the current observed map and safe blockers"
)]
pub(in super::super) fn update_cone_spray_visuals(
    mut meshes: ResMut<Assets<Mesh>>,
    mut index: ResMut<AimTraceBlockerIndex>,
    catalog: Res<crate::map::MapCatalogResource>,
    maps: Query<
        (
            &crate::map::ResolvedMapSnapshot,
            &crate::map::MapDynamicState,
        ),
        With<crate::map::MapRoot>,
    >,
    authoritative_ticks: Query<&AuthoritativeTick>,
    sprays: Query<&crate::combat::ConeSprayState>,
    safes: Query<(
        &Position,
        &Rotation,
        &crate::matchplay::HeistSafe,
        &crate::combat::CurrentHealth,
    )>,
    mut surfaces: Query<(&ConeSpraySurfaceVisual3d, &mut Visibility, &Mesh3d)>,
) {
    if let Some((map, state)) = maps
        .iter()
        .max_by_key(|(snapshot, _)| snapshot.identity.instance_id)
    {
        index.refresh(map, state, &catalog.0);
    }
    let mut blockers = safes
        .iter()
        .filter(|(_, _, _, health)| health.0 > 0)
        .map(|(position, rotation, safe, _)| AimTraceDynamicBlocker {
            class: AimTraceBlockerClass::HeistSafe,
            stable_id: (safe.match_id.0 << 32) | u128::from(safe.anchor_id.0),
            position: position.0,
            rotation: rotation.as_radians(),
            shape: crate::map::MapShape::Rectangle {
                half_extents: crate::matchplay::HEIST_SAFE_HALF_EXTENTS,
            },
        })
        .collect::<Vec<_>>();
    blockers.sort_by_key(|blocker| blocker.stable_id);
    let observed_tick = authoritative_ticks.iter().map(|tick| tick.0).max();
    let owners = surfaces
        .iter()
        .map(|(surface, ..)| surface.owner)
        .collect::<HashSet<_>>();
    let primitives_by_owner = owners
        .into_iter()
        .filter_map(|owner| {
            sprays.get(owner).ok().map(|state| {
                let tick = observed_tick.unwrap_or(state.emitted_at_tick);
                let primitives = cone_spray_primitives(*state, tick, &index, &blockers);
                (owner, (state.origin.as_vec2(), primitives))
            })
        })
        .collect::<HashMap<_, _>>();
    for (surface, mut visibility, mesh) in &mut surfaces {
        let Some((origin, primitives_for_spray)) = primitives_by_owner.get(&surface.owner) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Some(mut surface_mesh) = meshes.get_mut(&mesh.0) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *surface_mesh = cone_spray_surface_mesh(*origin, primitives_for_spray);
        *visibility = Visibility::Inherited;
    }
}

fn cone_spray_surface_mesh(
    origin: Vec2,
    primitives: &[crate::combat::client::PreviewPrimitive],
) -> Mesh {
    const RAY_COUNT: usize = MAX_CONE_SPRAY_SEGMENTS.div_ceil(2);

    let mut positions = Vec::with_capacity(RAY_COUNT + 1);
    positions.push(ground_position(origin).to_array());
    for primitive in primitives.iter().take(RAY_COUNT) {
        let PreviewGeometry::Corridor {
            center,
            angle,
            length,
            ..
        } = primitive.geometry
        else {
            continue;
        };
        let endpoint = center + Vec2::from_angle(angle) * (length * 0.5);
        positions.push(ground_position(endpoint).to_array());
    }
    for position in &mut positions {
        position[1] = GROUND_EFFECT_HEIGHT;
    }

    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    let mut uvs = Vec::with_capacity(positions.len());
    uvs.push([0.0, 0.5]);
    let endpoint_count = positions.len().saturating_sub(1).max(1);
    let endpoint_divisor = u16::try_from(endpoint_count.saturating_sub(1).max(1))
        .map(f32::from)
        .expect("cone surface endpoint count fits u16");
    for endpoint in 0..positions.len().saturating_sub(1) {
        let endpoint = u16::try_from(endpoint)
            .map(f32::from)
            .expect("cone surface endpoint index fits u16");
        uvs.push([1.0, endpoint / endpoint_divisor]);
    }
    let triangle_count = positions.len().saturating_sub(2);
    let mut indices = Vec::with_capacity(triangle_count * 3);
    for segment in 0..triangle_count {
        let first = u16::try_from(segment + 1).expect("cone surface vertex count fits u16");
        indices.extend([0, first, first + 1]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U16(indices));
    mesh
}

const fn cone_spray_material(
    variant: vfx::VfxPresentationVariant,
    materials: &Material3dAssets,
) -> &Handle<ConeSprayMaterial> {
    match variant {
        vfx::VfxPresentationVariant::Normal => &materials.spray_gas,
        vfx::VfxPresentationVariant::Reduced => &materials.spray_gas_reduced,
        vfx::VfxPresentationVariant::Primitive | vfx::VfxPresentationVariant::ReducedPrimitive => {
            &materials.spray_gas_primitive
        }
    }
}

pub(in crate::client) fn animate_cone_spray_materials(
    ticks: Query<&AuthoritativeTick>,
    handles: Option<Res<Material3dAssets>>,
    mut materials: ResMut<Assets<ConeSprayMaterial>>,
) {
    const VISUAL_CYCLE_SECONDS: u64 = 900;

    let Some(handles) = handles else {
        return;
    };
    let tick = ticks.iter().map(|tick| tick.0).max().unwrap_or_default();
    let cycle_ticks = crate::timing::SIMULATION_TICK_HZ * VISUAL_CYCLE_SECONDS;
    let visual_tick = u16::try_from(tick % cycle_ticks)
        .expect("the fifteen-minute cone spray visual cycle fits u16");
    let tick_hz = u16::try_from(crate::timing::SIMULATION_TICK_HZ)
        .expect("the simulation tick rate fits u16");
    let time = f32::from(visual_tick) / f32::from(tick_hz);
    for handle in [
        &handles.spray_gas,
        &handles.spray_gas_reduced,
        &handles.spray_gas_primitive,
    ] {
        if let Some(mut material) = materials.get_mut(handle) {
            material.params.x = time;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cone_material_variants_encode_bounded_quality_levels() {
        assert!(ConeSprayMaterial::normal().params.y.abs() < f32::EPSILON);
        assert!((ConeSprayMaterial::reduced().params.y - 1.0).abs() < f32::EPSILON);
        assert!((ConeSprayMaterial::primitive().params.y - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn production_retained_material_schedule_advances_cone_time() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<super::super::elemental_style::FireAreaMaterial>>()
            .init_resource::<Assets<super::super::elemental_style::ElementalSupportAreaMaterial>>()
            .init_resource::<Assets<ConeSprayMaterial>>()
            .init_resource::<crate::builds::BuildCatalogResource>()
            .add_systems(Startup, setup_3d_foundation);
        configure_world_presentation_schedule(&mut app);
        super::super::add_retained_material_animation_systems(
            &mut app,
            WorldPresentationSet::Animate,
        );
        app.world_mut()
            .spawn(AuthoritativeTick(crate::timing::SIMULATION_TICK_HZ));

        app.update();

        let handle = app.world().resource::<Material3dAssets>().spray_gas.clone();
        let material = app
            .world()
            .resource::<Assets<ConeSprayMaterial>>()
            .get(&handle)
            .expect("startup prepares the production cone material");
        assert!((material.params.x - 1.0).abs() < f32::EPSILON);
    }
}
