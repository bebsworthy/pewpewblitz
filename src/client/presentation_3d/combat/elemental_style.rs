//! Retained primitive Cold/Fire styling shared by durable projectile and area roots.

use super::super::*;
use bevy::{reflect::TypePath, render::render_resource::AsBindGroup, shader::ShaderRef};

const FIRE_AREA_SHADER_PATH: &str = "shaders/fire_area.wgsl";

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(in crate::client) struct FireAreaMaterial {
    #[uniform(0)]
    params: Vec4,
}

impl Default for FireAreaMaterial {
    fn default() -> Self {
        Self {
            params: Vec4::new(0.0, 1.0 / 1.08, 0.0, 0.0),
        }
    }
}

impl Material for FireAreaMaterial {
    fn fragment_shader() -> ShaderRef {
        FIRE_AREA_SHADER_PATH.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn depth_bias(&self) -> f32 {
        12.0
    }

    fn enable_shadows() -> bool {
        false
    }
}

pub(in crate::client) struct FireAreaMaterialPlugin;

impl Plugin for FireAreaMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<FireAreaMaterial>::default());
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub(in crate::client::presentation_3d) struct ElementalMotionAccent3d {
    motion: ElementalAccentMotion,
    seed: u64,
    base_translation: Vec3,
    base_rotation: Quat,
    base_scale: Vec3,
}

#[derive(Component)]
pub(in crate::client::presentation_3d) struct AnimatedFireArea3d;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ElementalAccentMotion {
    ColdCrystal,
    FireProjectileLobe,
    FireAreaTongue,
}

pub(super) fn elemental_materials(
    signature: vfx::CombatVisualSignature,
    materials: &Material3dAssets,
) -> Option<(Handle<StandardMaterial>, Handle<StandardMaterial>)> {
    match primary_element(signature) {
        Some(vfx::VfxAccent::Cold) => Some((
            materials.elemental_cold.clone(),
            materials.elemental_cold_bright.clone(),
        )),
        Some(vfx::VfxAccent::Fire) => Some((
            materials.elemental_fire.clone(),
            materials.elemental_fire_bright.clone(),
        )),
        _ => None,
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the style helper receives one complete retained projectile materialization profile"
)]
pub(super) fn spawn_elemental_projectile_style(
    parent: &mut ChildSpawnerCommands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    signature: vfx::CombatVisualSignature,
    exact_mesh: Handle<Mesh>,
    exact_transform: Transform,
    fallback_relationship_material: Handle<StandardMaterial>,
    visual_radius: f32,
) -> bool {
    let Some(accent) = primary_element(signature) else {
        return false;
    };
    let Some((soft, bright)) = elemental_materials(signature, materials) else {
        return false;
    };
    parent.spawn((
        Mesh3d(exact_mesh.clone()),
        MeshMaterial3d(bright.clone()),
        NotShadowCaster,
        NotShadowReceiver,
        exact_transform,
        Name::new("Elemental projectile exact geometry"),
    ));
    let mut relationship_transform = exact_transform;
    relationship_transform.scale *= 0.48;
    parent.spawn((
        Mesh3d(exact_mesh),
        MeshMaterial3d(relationship_material(
            signature.relationship,
            materials,
            fallback_relationship_material,
        )),
        NotShadowCaster,
        NotShadowReceiver,
        relationship_transform,
        Name::new("Elemental projectile relationship core"),
    ));

    let reduced = is_reduced(signature.variant);
    let seed = signature.stable_seed();
    match accent {
        vfx::VfxAccent::Cold => {
            let count: u8 = if reduced { 1 } else { 3 };
            for slot in 0..count {
                let angle =
                    core::f32::consts::FRAC_PI_4 + f32::from(slot) * core::f32::consts::FRAC_PI_3;
                let transform = Transform {
                    translation: Vec3::Y * 0.35,
                    rotation: Quat::from_rotation_y(angle),
                    scale: Vec3::new(visual_radius * 1.45, 1.15, visual_radius * 0.16),
                };
                parent.spawn((
                    ElementalMotionAccent3d::new(
                        ElementalAccentMotion::ColdCrystal,
                        seed_for(seed, slot),
                        transform,
                    ),
                    Mesh3d(primitives.unit_cuboid.clone()),
                    MeshMaterial3d(if slot == 0 {
                        bright.clone()
                    } else {
                        soft.clone()
                    }),
                    NotShadowCaster,
                    NotShadowReceiver,
                    transform,
                    Name::new("Cold projectile crystal fin"),
                ));
            }
        }
        vfx::VfxAccent::Fire => {
            let count: u8 = if reduced { 1 } else { 3 };
            for slot in 0..count {
                let distance = visual_radius * (1.0 + f32::from(slot) * 0.75);
                let scale = visual_radius * (0.62 - f32::from(slot) * 0.13);
                let transform = Transform::from_translation(Vec3::new(-distance, 0.4, 0.0))
                    .with_scale(Vec3::splat(scale.max(0.5)));
                parent.spawn((
                    ElementalMotionAccent3d::new(
                        ElementalAccentMotion::FireProjectileLobe,
                        seed_for(seed, slot),
                        transform,
                    ),
                    Mesh3d(primitives.effect_sphere.clone()),
                    MeshMaterial3d(if slot == 0 {
                        bright.clone()
                    } else {
                        soft.clone()
                    }),
                    NotShadowCaster,
                    NotShadowReceiver,
                    transform,
                    Name::new("Fire projectile trailing lobe"),
                ));
            }
        }
        _ => {}
    }
    true
}

pub(super) fn spawn_elemental_area_style(
    parent: &mut ChildSpawnerCommands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    signature: vfx::CombatVisualSignature,
    radius: f32,
) -> bool {
    let Some(accent) = primary_element(signature) else {
        return false;
    };
    let Some((soft, bright)) = elemental_materials(signature, materials) else {
        return false;
    };
    ElementalAreaStyle {
        primitives,
        materials,
        signature,
        accent,
        soft,
        bright,
        radius,
    }
    .spawn(parent);
    true
}

struct ElementalAreaStyle<'a> {
    primitives: &'a Primitive3dAssets,
    materials: &'a Material3dAssets,
    signature: vfx::CombatVisualSignature,
    accent: vfx::VfxAccent,
    soft: Handle<StandardMaterial>,
    bright: Handle<StandardMaterial>,
    radius: f32,
}

impl ElementalAreaStyle<'_> {
    fn spawn(&self, parent: &mut ChildSpawnerCommands) {
        self.spawn_fill(parent);
        self.spawn_boundaries(parent);
        self.spawn_relationship(parent);
        self.spawn_accents(parent);
    }

    fn is_normal_fire(&self) -> bool {
        self.accent == vfx::VfxAccent::Fire
            && self.signature.variant == vfx::VfxPresentationVariant::Normal
    }

    fn ground_rotation() -> Quat {
        Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2)
    }

    fn spawn_fill(&self, parent: &mut ChildSpawnerCommands) {
        let fill_scale = if self.is_normal_fire() {
            self.radius * 1.08
        } else {
            self.radius
        };
        let fill_transform =
            Transform::from_xyz(0.0, super::common::GROUND_EFFECT_HEIGHT + 0.25, 0.0)
                .with_rotation(Self::ground_rotation())
                .with_scale(Vec3::splat(fill_scale));
        if self.is_normal_fire() {
            parent.spawn((
                AnimatedFireArea3d,
                Mesh3d(self.primitives.area_disc.clone()),
                MeshMaterial3d(self.materials.elemental_fire_area.clone()),
                NotShadowCaster,
                NotShadowReceiver,
                fill_transform,
                Name::new("Fire area animated circumference"),
            ));
            return;
        }
        let fill = if self.accent == vfx::VfxAccent::Fire {
            self.materials.elemental_fire_fill.clone()
        } else {
            self.soft.clone()
        };
        parent.spawn((
            Mesh3d(self.primitives.area_disc.clone()),
            MeshMaterial3d(fill),
            NotShadowCaster,
            NotShadowReceiver,
            fill_transform,
            Name::new("Elemental area subtle fill"),
        ));
    }

    fn spawn_boundaries(&self, parent: &mut ChildSpawnerCommands) {
        let boundary_mesh = if self.is_normal_fire() {
            self.primitives.area_thin_ring.clone()
        } else {
            self.primitives.area_ring.clone()
        };
        let boundary = if self.accent == vfx::VfxAccent::Fire {
            self.materials.elemental_fire_bright.clone()
        } else {
            self.bright.clone()
        };
        parent.spawn((
            Mesh3d(boundary_mesh),
            MeshMaterial3d(boundary),
            NotShadowCaster,
            NotShadowReceiver,
            Transform::from_xyz(0.0, super::common::GROUND_EFFECT_HEIGHT + 0.65, 0.0)
                .with_rotation(Self::ground_rotation())
                .with_scale(Vec3::splat(self.radius)),
            Name::new("Elemental area exact bright boundary"),
        ));
        if self.accent == vfx::VfxAccent::Fire && !self.is_normal_fire() {
            parent.spawn((
                Mesh3d(self.primitives.area_ring.clone()),
                MeshMaterial3d(self.bright.clone()),
                NotShadowCaster,
                NotShadowReceiver,
                Transform::from_xyz(0.0, super::common::GROUND_EFFECT_HEIGHT + 0.48, 0.0)
                    .with_rotation(Self::ground_rotation())
                    .with_scale(Vec3::splat(self.radius * 0.90)),
                Name::new("Fire area soft inner boundary"),
            ));
        }
    }

    fn spawn_relationship(&self, parent: &mut ChildSpawnerCommands) {
        if self.signature.relationship == vfx::VfxRelationship::Neutral {
            return;
        }
        parent.spawn((
            Mesh3d(self.primitives.area_ring.clone()),
            MeshMaterial3d(relationship_material(
                self.signature.relationship,
                self.materials,
                self.materials.neutral.clone(),
            )),
            NotShadowCaster,
            NotShadowReceiver,
            Transform::from_xyz(0.0, super::common::GROUND_EFFECT_HEIGHT + 0.9, 0.0)
                .with_rotation(Self::ground_rotation())
                .with_scale(Vec3::splat(self.radius * 0.9)),
            Name::new("Elemental area relationship inset"),
        ));
    }

    fn spawn_accents(&self, parent: &mut ChildSpawnerCommands) {
        let count = area_accent_count(self.accent, self.signature.variant);
        let seed = self.signature.stable_seed();
        let offset = unit_sample(seed_for(seed, 31)) * core::f32::consts::TAU;
        for slot in 0..count {
            let angle = offset + f32::from(slot) * core::f32::consts::TAU / f32::from(count);
            let slot_seed = seed_for(seed, slot);
            let transform = self.accent_transform(seed, slot, angle, count);
            let motion = if self.accent == vfx::VfxAccent::Cold {
                ElementalAccentMotion::ColdCrystal
            } else {
                ElementalAccentMotion::FireAreaTongue
            };
            let mesh = if self.accent == vfx::VfxAccent::Cold {
                self.primitives.unit_cuboid.clone()
            } else {
                self.primitives.fire_tongue.clone()
            };
            let material = self.accent_material(slot);
            parent.spawn((
                ElementalMotionAccent3d::new(motion, slot_seed, transform),
                Mesh3d(mesh),
                MeshMaterial3d(material),
                NotShadowCaster,
                NotShadowReceiver,
                transform,
                Name::new(if self.accent == vfx::VfxAccent::Cold {
                    "Cold area crystal accent"
                } else {
                    "Fire area flame accent"
                }),
            ));
        }
    }

    fn accent_transform(&self, seed: u64, slot: u8, angle: f32, count: u8) -> Transform {
        let radial_sample = unit_sample(seed_for(seed, slot.wrapping_add(41)));
        if self.accent == vfx::VfxAccent::Cold {
            let radial = self.radius * (0.76 + radial_sample * 0.12);
            return Transform {
                translation: Vec3::new(angle.cos() * radial, 3.2, angle.sin() * radial),
                rotation: Quat::from_rotation_y(-angle),
                scale: Vec3::new(self.radius * 0.11, 2.0, self.radius * 0.035),
            };
        }
        let radial = self.radius * (0.92 + (radial_sample - 0.5) * 0.035);
        let circumference_slot = core::f32::consts::TAU * radial / f32::from(count);
        let length_sample = unit_sample(seed_for(seed, slot.wrapping_add(83)));
        Transform {
            translation: Vec3::new(
                angle.cos() * radial,
                super::common::GROUND_EFFECT_HEIGHT + 1.1,
                angle.sin() * radial,
            ),
            rotation: Quat::from_rotation_y(core::f32::consts::FRAC_PI_2 - angle)
                * Self::ground_rotation(),
            scale: Vec3::new(
                circumference_slot * 1.18,
                self.radius * (0.10 + length_sample * 0.14),
                1.0,
            ),
        }
    }

    fn accent_material(&self, slot: u8) -> Handle<StandardMaterial> {
        if self.accent == vfx::VfxAccent::Cold {
            return if slot.is_multiple_of(2) {
                self.bright.clone()
            } else {
                self.soft.clone()
            };
        }
        match slot % 3 {
            0 => self.materials.elemental_fire_hot.clone(),
            1 => self.bright.clone(),
            _ => self.soft.clone(),
        }
    }
}

pub(in crate::client::presentation_3d) fn animate_elemental_motion_accents(
    ticks: Query<&AuthoritativeTick>,
    mut accents: Query<(&ElementalMotionAccent3d, &mut Transform)>,
) {
    let tick = ticks.iter().map(|tick| tick.0).max().unwrap_or_default();
    for (accent, mut transform) in &mut accents {
        let period = match accent.motion {
            ElementalAccentMotion::ColdCrystal => 120,
            ElementalAccentMotion::FireProjectileLobe => 24,
            ElementalAccentMotion::FireAreaTongue => 18 + accent.seed % 15,
        };
        let phase_tick = u16::try_from((tick + accent.seed % period) % period)
            .expect("elemental accent phase is bounded by its short period");
        let period = u16::try_from(period).expect("elemental accent period fits u16");
        let progress = f32::from(phase_tick) / f32::from(period);
        match accent.motion {
            ElementalAccentMotion::ColdCrystal => {
                let wave = (progress * core::f32::consts::TAU).sin();
                transform.translation = accent.base_translation + Vec3::Y * (wave * 1.1);
                transform.rotation = accent.base_rotation * Quat::from_rotation_y(wave * 0.12);
                transform.scale = accent.base_scale * (0.94 + wave * 0.06);
            }
            ElementalAccentMotion::FireProjectileLobe => {
                transform.translation = accent.base_translation + Vec3::Y * (progress * 5.0);
                transform.rotation = accent.base_rotation;
                transform.scale = accent.base_scale * (1.15 - progress * 0.55);
            }
            ElementalAccentMotion::FireAreaTongue => {
                let wave = (progress * core::f32::consts::TAU).sin();
                let flicker = wave.mul_add(0.18, 0.82);
                transform.translation = accent.base_translation + Vec3::Y * ((wave + 1.0) * 0.28);
                transform.rotation = accent.base_rotation * Quat::from_rotation_z(wave * 0.055);
                transform.scale = Vec3::new(
                    accent.base_scale.x * wave.mul_add(0.05, 0.95),
                    accent.base_scale.y * flicker,
                    accent.base_scale.z,
                );
            }
        }
    }
}

pub(in crate::client) fn animate_fire_area_material(
    ticks: Query<&AuthoritativeTick>,
    handles: Option<Res<Material3dAssets>>,
    mut materials: ResMut<Assets<FireAreaMaterial>>,
) {
    let Some(handles) = handles else {
        return;
    };
    let tick = ticks.iter().map(|tick| tick.0).max().unwrap_or_default();
    if let Some(mut material) = materials.get_mut(&handles.elemental_fire_area) {
        const VISUAL_CYCLE_SECONDS: u64 = 900;
        let cycle_ticks = crate::timing::SIMULATION_TICK_HZ * VISUAL_CYCLE_SECONDS;
        let visual_tick = u16::try_from(tick % cycle_ticks)
            .expect("the fifteen-minute Fire visual cycle fits u16");
        let tick_hz = u16::try_from(crate::timing::SIMULATION_TICK_HZ)
            .expect("the simulation tick rate fits u16");
        material.params.x = f32::from(visual_tick) / f32::from(tick_hz);
    }
}

impl ElementalMotionAccent3d {
    const fn new(motion: ElementalAccentMotion, seed: u64, transform: Transform) -> Self {
        Self {
            motion,
            seed,
            base_translation: transform.translation,
            base_rotation: transform.rotation,
            base_scale: transform.scale,
        }
    }
}

fn primary_element(signature: vfx::CombatVisualSignature) -> Option<vfx::VfxAccent> {
    signature
        .accents
        .into_iter()
        .flatten()
        .find(|accent| matches!(accent, vfx::VfxAccent::Cold | vfx::VfxAccent::Fire))
}

const fn is_reduced(variant: vfx::VfxPresentationVariant) -> bool {
    matches!(
        variant,
        vfx::VfxPresentationVariant::Reduced | vfx::VfxPresentationVariant::ReducedPrimitive
    )
}

const fn area_accent_count(accent: vfx::VfxAccent, variant: vfx::VfxPresentationVariant) -> u8 {
    match (accent, variant) {
        (
            vfx::VfxAccent::Cold,
            vfx::VfxPresentationVariant::Normal | vfx::VfxPresentationVariant::Primitive,
        ) => 6,
        (vfx::VfxAccent::Cold, _) => 3,
        (vfx::VfxAccent::Fire, vfx::VfxPresentationVariant::Normal) => 0,
        (vfx::VfxAccent::Fire, vfx::VfxPresentationVariant::Primitive) => 36,
        (vfx::VfxAccent::Fire, _) => 18,
        _ => 0,
    }
}

fn relationship_material(
    relationship: vfx::VfxRelationship,
    materials: &Material3dAssets,
    neutral_fallback: Handle<StandardMaterial>,
) -> Handle<StandardMaterial> {
    match relationship {
        vfx::VfxRelationship::SelfOwned => materials.marker_local.clone(),
        vfx::VfxRelationship::Ally => materials.marker_ally.clone(),
        vfx::VfxRelationship::Enemy => materials.marker_enemy.clone(),
        vfx::VfxRelationship::Neutral => neutral_fallback,
    }
}

fn seed_for(seed: u64, slot: u8) -> u64 {
    vfx::vfx_counter_sample(
        seed,
        "durable.elemental",
        "mesh-accent",
        slot,
        u32::from(slot),
    )
}

fn unit_sample(seed: u64) -> f32 {
    let sample = u16::try_from(seed & 0xffff).expect("sample is masked to sixteen bits");
    f32::from(sample) / f32::from(u16::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_identity_drives_repeatable_bounded_offsets() {
        let seed = seed_for(77, 3);
        assert_eq!(seed, seed_for(77, 3));
        assert_ne!(seed, seed_for(78, 3));
        assert!((0.0..=1.0).contains(&unit_sample(seed)));
    }

    #[test]
    fn reduced_variants_preserve_a_bounded_elemental_identity_channel() {
        use vfx::VfxPresentationVariant::{Normal, Primitive, Reduced, ReducedPrimitive};

        assert_eq!(area_accent_count(vfx::VfxAccent::Cold, Normal), 6);
        assert_eq!(area_accent_count(vfx::VfxAccent::Cold, Primitive), 6);
        assert_eq!(area_accent_count(vfx::VfxAccent::Cold, Reduced), 3);
        assert_eq!(area_accent_count(vfx::VfxAccent::Cold, ReducedPrimitive), 3);
        assert_eq!(area_accent_count(vfx::VfxAccent::Fire, Normal), 0);
        assert_eq!(area_accent_count(vfx::VfxAccent::Fire, Primitive), 36);
        assert_eq!(area_accent_count(vfx::VfxAccent::Fire, Reduced), 18);
        assert_eq!(area_accent_count(vfx::VfxAccent::Neutral, Normal), 0);
    }
}
