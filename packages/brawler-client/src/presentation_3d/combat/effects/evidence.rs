//! Presentation-owned VFX evidence snapshot for the paired PNG/JSON capture.
//!
//! Kept beside the renderer it observes but out of the lifecycle code: it only reads renderer
//! ownership, catalog, and retained-asset state and never mutates presentation.

#[allow(
    clippy::wildcard_imports,
    reason = "the world-renderer submodule consumes the presentation_3d composition surface"
)]
use super::super::super::*;
use super::super::{MAX_ACTIVE_PARTICLE_EMITTERS, RetainedProjectileSmoke3d};
use super::{
    CombatEffect3d, CombatEffectLayer3d, MAX_EFFECTS, VfxParticleDiagnostics, VfxParticleEmitter3d,
    VfxTransientDiagnostics,
};
use crate::vfx::{CombatVisualSignature, VfxRegistry, VfxSignatureCache};
use bevy_hanabi::EffectAsset;
use serde_json::{Value, json};
use std::collections::HashMap;

/// Collects bounded presentation-owned VFX state for a screenshot evidence pair.
///
/// The snapshot intentionally reports renderer ownership rather than gameplay outcomes. Durable
/// gameplay facts (projectiles, fields, fighters, and authoritative ticks) are captured by the
/// ordinary evidence path alongside this renderer view.
#[allow(
    clippy::too_many_lines,
    reason = "one evidence snapshot keeps the transient, durable-signature, registry, and cache views atomic"
)]
pub(crate) fn collect_vfx_evidence_snapshot(world: &mut World) -> Value {
    let mut layer_query = world.query::<&CombatEffectLayer3d>();
    let mut layers_by_root = HashMap::<Entity, Vec<(u8, Value)>>::new();
    for layer in layer_query.iter(world) {
        layers_by_root.entry(layer.root).or_default().push((
            layer.layer_index,
            json!({
                "index": layer.layer_index,
                "id": layer.layer_id,
                "readability": layer.readability,
                "variant_sample": layer.variant_sample,
            }),
        ));
    }
    let mut particle_query = world.query::<&VfxParticleEmitter3d>();
    let mut particle_layers_by_root = HashMap::<Entity, Vec<Value>>::new();
    for emitter in particle_query.iter(world) {
        particle_layers_by_root
            .entry(emitter.root)
            .or_default()
            .push(json!({
                "id": emitter.layer_id,
                "profile": emitter.profile,
                "capacity": emitter.capacity,
                "spawn": emitter.spawn,
                "seed": emitter.seed,
            }));
    }
    let mut transient_query = world.query::<(
        Entity,
        &CombatEffect3d,
        &CombatVisualSignature,
        Option<&Name>,
    )>();
    let mut transient_effects = transient_query
        .iter(world)
        .map(|(entity, effect, signature, name)| {
            json!({
                "entity": entity.to_bits(),
                "recipe_id": effect.recipe_id,
                "signature": signature,
                "layers": layers_by_root.remove(&entity).map(|mut layers| {
                    layers.sort_unstable_by_key(|(index, _)| *index);
                    layers.into_iter().map(|(_, layer)| layer).collect::<Vec<_>>()
                }).unwrap_or_default(),
                "particle_layers": particle_layers_by_root.remove(&entity).unwrap_or_default(),
                "order": effect.order,
                "label": name.map(Name::as_str),
                "elapsed_seconds": effect.timer.elapsed_secs(),
                "duration_seconds": effect.timer.duration().as_secs_f32(),
                "authoritative_expires_at_tick": effect.expires_at_tick,
            })
        })
        .collect::<Vec<_>>();
    transient_effects.sort_by_key(|effect| effect["order"].as_u64().unwrap_or(u64::MAX));

    let reduced_effects = world
        .get_resource::<ClientShellSettings>()
        .is_some_and(|settings| settings.reduced_combat_effects);
    let primitive_world = world
        .get_resource::<ImportedWorldFallbackPolicy>()
        .is_some_and(|policy| matches!(policy, ImportedWorldFallbackPolicy::ForcePrimitive));
    let mesh_assets = world.get_resource::<Assets<Mesh>>().map_or(0, Assets::len);
    let material_assets = world
        .get_resource::<Assets<StandardMaterial>>()
        .map_or(0, Assets::len);
    let fire_area_material_assets = world
        .get_resource::<Assets<super::super::elemental_style::FireAreaMaterial>>()
        .map_or(0, Assets::len);
    let elemental_support_area_material_assets = world
        .get_resource::<Assets<super::super::elemental_style::ElementalSupportAreaMaterial>>()
        .map_or(0, Assets::len);
    let cone_spray_material_assets = world
        .get_resource::<Assets<super::super::cone_spray::ConeSprayMaterial>>()
        .map_or(0, Assets::len);
    let straight_projectile_trail_material_assets = world
        .get_resource::<Assets<super::super::straight_projectile_style::StraightProjectileTrailMaterial>>()
        .map_or(0, Assets::len);
    let dash_trail_material_assets = world
        .get_resource::<Assets<super::super::dash_style::DashTrailMaterial>>()
        .map_or(0, Assets::len);
    let splash_area_material_assets = world
        .get_resource::<Assets<super::super::splash_style::SplashAreaMaterial>>()
        .map_or(0, Assets::len);
    let particle_effect_assets = world
        .get_resource::<Assets<EffectAsset>>()
        .map_or(0, Assets::len);
    let image_assets = world.get_resource::<Assets<Image>>().map_or(0, Assets::len);
    let signature_cache = world
        .get_resource::<VfxSignatureCache>()
        .map(VfxSignatureCache::diagnostics);
    let catalog_diagnostics = world
        .get_resource::<VfxRegistry>()
        .map(VfxRegistry::catalog_diagnostics);
    let transient_diagnostics = world
        .get_resource::<VfxTransientDiagnostics>()
        .copied()
        .unwrap_or_default();
    let particle_diagnostics = world
        .get_resource::<VfxParticleDiagnostics>()
        .copied()
        .unwrap_or_default();
    let mut signature_query = world.query::<(
        Entity,
        &CombatVisualSignature,
        Option<&CombatVisualOwner>,
        Option<&Name>,
    )>();
    let mut signatures = signature_query
        .iter(world)
        .map(|(entity, signature, owner, name)| {
            json!({
                "entity": entity.to_bits(),
                "owner": owner.map(|owner| owner.0.to_bits()),
                "name": name.map(Name::as_str),
                "signature": signature,
            })
        })
        .collect::<Vec<_>>();
    signatures.sort_by_key(|entry| entry["entity"].as_u64().unwrap_or(u64::MAX));
    let elemental_fields =
        serialized_component_snapshots::<brawler::combat::ElementalFieldState>(world);
    let concealment_fields =
        serialized_component_snapshots::<brawler::concealment::ConcealmentFieldState>(world);
    let persistent_splashes =
        serialized_component_snapshots::<brawler::combat::PersistentSplashState>(world);
    let cone_sprays = serialized_component_snapshots::<brawler::combat::ConeSprayState>(world);
    let sticky_blobs = sticky_blob_snapshots(world);

    json!({
        "catalog_fingerprint": vfx::embedded_vfx_catalog_fingerprint(),
        "catalog": catalog_diagnostics,
        "reduced_effects": reduced_effects,
        "primitive_world": primitive_world,
        "active": {
            "visual_roots": component_count::<CombatVisualOwner>(world),
            "transient_roots": transient_effects.len(),
            "transient_root_capacity": MAX_EFFECTS,
            "transient_root_high_water": transient_diagnostics.root_high_water,
            "transient_mesh_layers": component_count::<CombatEffectLayer3d>(world),
            "transient_mesh_layer_high_water": transient_diagnostics.mesh_layer_high_water,
            "transient_roots_evicted": transient_diagnostics.evicted_roots,
            "projectile_roots": component_count::<V3ProjectileVisual>(world),
            "lobbed_projectile_aids": component_count::<super::super::entities::LobbedProjectileAid3d>(world),
            "sentry_roots": component_count::<super::super::entities::SentryVisual3d>(world),
            "concealment_field_roots": component_count::<super::super::entities::ConcealmentFieldVisual3d>(world),
            "elemental_field_roots": component_count::<super::super::entities::ElementalFieldVisual3d>(world),
            "persistent_splash_roots": component_count::<super::super::entities::PersistentSplashVisual3d>(world),
            "sticky_blob_roots": component_count::<super::super::entities::StickyBlobVisual3d>(world),
            "cone_spray_roots": component_count::<super::super::cone_spray::ConeSprayVisual3d>(world),
            "status_roots": component_count::<super::super::fighter_feedback::StatusVisual3d>(world),
            "dash_trail_roots": component_count::<super::super::fighter_feedback::DashTrailVisual3d>(world),
            "elemental_motion_accents": component_count::<super::super::elemental_style::ElementalMotionAccent3d>(world),
            "animated_fire_areas": component_count::<super::super::elemental_style::AnimatedFireArea3d>(world),
            "fire_area_underlays": component_count::<super::super::elemental_style::FireAreaUnderlay3d>(world),
            "particle_emitters": component_count::<VfxParticleEmitter3d>(world),
            "retained_projectile_smoke_emitters": component_count::<RetainedProjectileSmoke3d>(world),
            "particle_emitter_capacity": MAX_ACTIVE_PARTICLE_EMITTERS,
            "particle_emitter_high_water": particle_diagnostics.high_water,
            "particle_emitters_suppressed": particle_diagnostics.suppressed,
        },
        "assets": {
            "meshes": mesh_assets,
            "standard_materials": material_assets,
            "fire_area_materials": fire_area_material_assets,
            "elemental_support_area_materials": elemental_support_area_material_assets,
            "cone_spray_materials": cone_spray_material_assets,
            "straight_projectile_trail_materials": straight_projectile_trail_material_assets,
            "dash_trail_materials": dash_trail_material_assets,
            "splash_area_materials": splash_area_material_assets,
            "particle_effects": particle_effect_assets,
            "images": image_assets,
        },
        "signature_cache": signature_cache,
        "signatures": signatures,
        "transient_effects": transient_effects,
        "persistent_facts": {
            "elemental_fields": elemental_fields,
            "concealment_fields": concealment_fields,
            "persistent_splashes": persistent_splashes,
            "cone_sprays": cone_sprays,
            "sticky_blobs": sticky_blobs,
        },
    })
}

pub(super) fn component_count<T: Component>(world: &mut World) -> usize {
    world
        .query_filtered::<Entity, With<T>>()
        .iter(world)
        .count()
}

fn serialized_component_snapshots<T>(world: &mut World) -> Vec<Value>
where
    T: Component + serde::Serialize,
{
    let mut query = world.query::<(Entity, &T)>();
    let mut snapshots = query
        .iter(world)
        .map(|(entity, state)| {
            json!({
                "entity": entity.to_bits(),
                "state": state,
            })
        })
        .collect::<Vec<_>>();
    snapshots.sort_by_key(|snapshot| snapshot["entity"].as_u64().unwrap_or(u64::MAX));
    snapshots
}

fn sticky_blob_snapshots(world: &mut World) -> Vec<Value> {
    let mut query = world.query::<(
        Entity,
        &brawler::combat::StickyBlobState,
        Option<&Position>,
        Option<&brawler::combat::ReplicatedAttackSource>,
    )>();
    let mut snapshots = query
        .iter(world)
        .map(|(entity, state, position, source)| {
            json!({
                "entity": entity.to_bits(),
                "position": position.map(|position| [position.x, position.y]),
                "state": state,
                "source": source,
            })
        })
        .collect::<Vec<_>>();
    snapshots.sort_by_key(|snapshot| snapshot["entity"].as_u64().unwrap_or(u64::MAX));
    snapshots
}

#[cfg(test)]
mod tests {
    use super::super::tests::registry;
    use super::*;
    use crate::vfx::VfxPresentationVariant;

    #[test]
    fn evidence_snapshot_names_catalog_variant_and_active_lifecycle_owners() {
        let mut world = World::new();
        world.insert_resource(ClientShellSettings {
            reduced_combat_effects: true,
            ..Default::default()
        });
        world.insert_resource(ImportedWorldFallbackPolicy::ForcePrimitive);
        world.insert_resource(registry());
        world.insert_resource(VfxTransientDiagnostics {
            root_high_water: 7,
            mesh_layer_high_water: 21,
            evicted_roots: 2,
        });
        world.spawn((
            CombatEffect3d {
                timer: Timer::new(Duration::from_millis(180), TimerMode::Once),
                expires_at_tick: None,
                order: 7,
                recipe_id: "impact".to_string(),
                mesh_layers: 0,
                particle_emitters: 0,
            },
            CombatVisualSignature::neutral_point(7, Vec2::ZERO)
                .with_variant(VfxPresentationVariant::ReducedPrimitive),
            Name::new("Impact evidence fixture"),
        ));
        let owner = world.spawn_empty().id();
        world.spawn((
            CombatVisualOwner(owner),
            super::super::super::entities::ElementalFieldVisual3d,
            CombatVisualSignature::neutral_point(11, Vec2::new(2.0, 3.0)),
        ));
        world.spawn(super::super::super::elemental_style::AnimatedFireArea3d);
        world.spawn(super::super::super::elemental_style::FireAreaUnderlay3d);
        world.spawn(brawler::combat::ElementalFieldState {
            id: brawler::combat::ElementalFieldId(11),
            kind: brawler::combat::ElementalFieldKind::Fire,
            owner_network_entity_id: NetworkEntityId(12),
            team_id: brawler::combat::TeamId(1),
            center: brawler::combat::WorldPoint { x: 2.0, y: 3.0 },
            radius_milliunits: 64_000,
            activated_at_tick: 20,
            next_pulse_tick: 30,
            expires_at_tick: 80,
        });

        let snapshot = collect_vfx_evidence_snapshot(&mut world);

        assert_eq!(snapshot["reduced_effects"], true);
        assert_eq!(snapshot["primitive_world"], true);
        assert_eq!(snapshot["catalog"]["request_mappings"], 17);
        assert_eq!(snapshot["catalog"]["mapping_capacity"], 64);
        assert!(snapshot["catalog"]["recipes"].as_u64().unwrap() > 17);
        assert!(
            snapshot["catalog"]["maximum_recipe_mesh_layers"]
                .as_u64()
                .unwrap()
                <= 8
        );
        assert_eq!(snapshot["active"]["transient_roots"], 1);
        assert_eq!(snapshot["active"]["transient_root_capacity"], 96);
        assert_eq!(snapshot["active"]["transient_root_high_water"], 7);
        assert_eq!(snapshot["active"]["transient_mesh_layer_high_water"], 21);
        assert_eq!(snapshot["active"]["transient_roots_evicted"], 2);
        assert_eq!(snapshot["active"]["elemental_field_roots"], 1);
        assert_eq!(snapshot["active"]["animated_fire_areas"], 1);
        assert_eq!(snapshot["active"]["fire_area_underlays"], 1);
        assert_eq!(snapshot["transient_effects"][0]["recipe_id"], "impact");
        assert_eq!(snapshot["transient_effects"][0]["order"], 7);
        assert_eq!(
            snapshot["persistent_facts"]["elemental_fields"][0]["state"]["kind"],
            "Fire"
        );
        assert_ne!(snapshot["catalog_fingerprint"], 0);
    }
}
