//! Neutral envelope and application composition for shared authored gameplay content.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const GAMEPLAY_CONTENT_ENVELOPE_VERSION: u16 = 27;

const MAX_GAMEPLAY_FINGERPRINT_CONTRIBUTORS: usize = 16;

pub(crate) const BOTS_FINGERPRINT_DOMAIN: &str = "bots.practice";
pub(crate) const BUILDS_FINGERPRINT_DOMAIN: &str = "builds.catalog";
pub(crate) const COMBAT_CONDITIONS_FINGERPRINT_DOMAIN: &str = "combat.conditions";
pub(crate) const COMBAT_WEAPONS_FINGERPRINT_DOMAIN: &str = "combat.weapons";
pub(crate) const CONCEALMENT_FINGERPRINT_DOMAIN: &str = "concealment.rules";
pub(crate) const MAP_FINGERPRINT_DOMAIN: &str = "map.catalog";
pub(crate) const WEAPON_PARTS_FINGERPRINT_DOMAIN: &str = "weapon-parts.catalog";

const REQUIRED_GAMEPLAY_FINGERPRINT_DOMAINS: [&str; 7] = [
    BOTS_FINGERPRINT_DOMAIN,
    BUILDS_FINGERPRINT_DOMAIN,
    COMBAT_CONDITIONS_FINGERPRINT_DOMAIN,
    COMBAT_WEAPONS_FINGERPRINT_DOMAIN,
    CONCEALMENT_FINGERPRINT_DOMAIN,
    MAP_FINGERPRINT_DOMAIN,
    WEAPON_PARTS_FINGERPRINT_DOMAIN,
];

type GameplayFingerprintMaterialFn = fn(&World) -> Result<Vec<u8>, String>;

#[derive(Clone, Copy)]
struct GameplayFingerprintRegistration {
    domain_id: &'static str,
    domain_schema_version: u16,
    material: GameplayFingerprintMaterialFn,
}

#[derive(Resource, Default)]
struct GameplayFingerprintRegistry {
    registrations: Vec<GameplayFingerprintRegistration>,
}

#[derive(Resource, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GameplayContentFingerprint(pub u64);

/// Register one shared gameplay-content domain during plugin construction.
///
/// The registry remains private so runtime systems cannot mutate compatibility identity. Domain
/// plugins own their material callback while this module owns bounds, ordering, and framing.
pub(crate) fn register_gameplay_fingerprint_contributor(
    app: &mut App,
    domain_id: &'static str,
    domain_schema_version: u16,
    material: GameplayFingerprintMaterialFn,
) {
    app.init_resource::<GameplayFingerprintRegistry>();
    app.world_mut()
        .resource_mut::<GameplayFingerprintRegistry>()
        .registrations
        .push(GameplayFingerprintRegistration {
            domain_id,
            domain_schema_version,
            material,
        });
}

/// Installs the validated, build-embedded gameplay catalogs used by every process role.
///
/// This plugin is deliberately headless-safe. Client-only presentation catalogs such as audio
/// and VFX remain owned by their presentation plugins and cannot leak into the server feature
/// graph through this shared composition boundary.
pub struct GameplayContentPlugin;

impl Plugin for GameplayContentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameplayFingerprintRegistry>()
            .add_plugins(crate::combat::CombatContentPlugin)
            .add_plugins(crate::concealment::ConcealmentContentPlugin)
            .add_plugins(crate::bots::BotContentPlugin)
            .add_plugins(crate::builds::BuildContentPlugin)
            .add_plugins(crate::weapon_parts::WeaponPartContentPlugin)
            .add_plugins(crate::map::MapContentPlugin)
            .add_systems(Startup, initialize_content_fingerprint);
    }
}

/// Evaluate registered shared-content domains against their current World resources.
///
/// This path intentionally works before Startup so routed process and worker builders can use the
/// same contributor set as the production Startup finalizer.
pub fn gameplay_content_fingerprint_from_world(
    world: &World,
) -> Result<GameplayContentFingerprint, String> {
    let registry = world
        .get_resource::<GameplayFingerprintRegistry>()
        .ok_or_else(|| "gameplay fingerprint registry is not installed".to_owned())?;
    evaluate_gameplay_fingerprint(world, &registry.registrations)
}

fn evaluate_gameplay_fingerprint(
    world: &World,
    registrations: &[GameplayFingerprintRegistration],
) -> Result<GameplayContentFingerprint, String> {
    if registrations.len() > MAX_GAMEPLAY_FINGERPRINT_CONTRIBUTORS {
        return Err(format!(
            "gameplay fingerprint contributor capacity exceeded: {} > {MAX_GAMEPLAY_FINGERPRINT_CONTRIBUTORS}",
            registrations.len()
        ));
    }

    let mut domain_ids = BTreeSet::new();
    for registration in registrations {
        if !valid_fingerprint_domain_id(registration.domain_id) {
            return Err(format!(
                "invalid gameplay fingerprint domain id: {}",
                registration.domain_id
            ));
        }
        if !domain_ids.insert(registration.domain_id) {
            return Err(format!(
                "duplicate gameplay fingerprint domain id: {}",
                registration.domain_id
            ));
        }
    }
    for required in REQUIRED_GAMEPLAY_FINGERPRINT_DOMAINS {
        if !domain_ids.contains(required) {
            return Err(format!(
                "missing required gameplay fingerprint domain: {required}"
            ));
        }
    }

    let mut contributions = registrations
        .iter()
        .map(|registration| {
            (registration.material)(world).map(|material| {
                (
                    registration.domain_id,
                    registration.domain_schema_version,
                    material,
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    contributions.sort_unstable_by_key(|(domain_id, _, _)| *domain_id);
    let bytes = postcard::to_allocvec(&(GAMEPLAY_CONTENT_ENVELOPE_VERSION, contributions))
        .map_err(|error| format!("gameplay content envelope serialization failed: {error}"))?;
    Ok(GameplayContentFingerprint(fnv1a64(&bytes)))
}

fn valid_fingerprint_domain_id(domain_id: &str) -> bool {
    !domain_id.is_empty()
        && domain_id.len() <= 64
        && domain_id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
        })
}

pub(crate) fn initialize_content_fingerprint(world: &mut World) {
    let fingerprint = gameplay_content_fingerprint_from_world(world)
        .expect("embedded gameplay catalogs must fingerprint");
    world.insert_resource(fingerprint);
}

/// Fingerprint caller-supplied primary catalogs with the other shared embedded catalogs.
///
/// Production process identity uses [`gameplay_content_fingerprint_from_world`]. This convenience
/// API remains for pure catalog callers while delegating to the same plugin registrations and
/// deterministic evaluator rather than maintaining another central domain list.
pub fn gameplay_content_fingerprint(
    weapons: &crate::combat::WeaponCatalog,
    maps: &crate::map::MapContentCatalog,
    builds: &crate::builds::BuildCatalog,
) -> Result<GameplayContentFingerprint, String> {
    let mut app = App::new();
    app.add_plugins(GameplayContentPlugin)
        .insert_resource(crate::combat::WeaponCatalogResource(weapons.clone()))
        .insert_resource(crate::map::MapCatalogResource(maps.clone()))
        .insert_resource(crate::builds::BuildCatalogResource(builds.clone()));
    gameplay_content_fingerprint_from_world(app.world())
}

#[must_use]
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    fnv1a64_seeded(0xcbf2_9ce4_8422_2325, bytes)
}

/// Continue one FNV-1a hash with more material instead of starting a new digest.
#[must_use]
pub fn fnv1a64_seeded(seed: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(seed, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(
        clippy::unnecessary_wraps,
        reason = "test callback deliberately implements the fallible contributor contract"
    )]
    fn synthetic_material(_: &World) -> Result<Vec<u8>, String> {
        Ok(vec![1, 2, 3])
    }

    #[allow(
        clippy::unnecessary_wraps,
        reason = "test callback deliberately implements the fallible contributor contract"
    )]
    fn alternate_synthetic_material(_: &World) -> Result<Vec<u8>, String> {
        Ok(vec![1, 2, 4])
    }

    fn app_with_synthetic(
        domain_id: &'static str,
        domain_schema_version: u16,
        material: GameplayFingerprintMaterialFn,
    ) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, GameplayContentPlugin));
        register_gameplay_fingerprint_contributor(
            &mut app,
            domain_id,
            domain_schema_version,
            material,
        );
        app
    }

    #[test]
    fn content_plugin_installs_catalogs_and_fingerprint_without_protocol() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, GameplayContentPlugin));

        assert_eq!(GAMEPLAY_CONTENT_ENVELOPE_VERSION, 27);
        assert!(
            app.world()
                .contains_resource::<crate::combat::WeaponCatalogResource>()
        );
        assert!(
            app.world()
                .contains_resource::<crate::combat::CombatConditionRulesResource>()
        );
        assert!(
            app.world()
                .contains_resource::<crate::concealment::ConcealmentRulesResource>()
        );
        assert!(
            app.world()
                .contains_resource::<crate::bots::BotCatalogResource>()
        );
        assert!(
            app.world()
                .contains_resource::<crate::builds::BuildCatalogResource>()
        );
        assert!(
            app.world()
                .contains_resource::<crate::weapon_parts::WeaponPartCatalogResource>()
        );
        assert!(
            app.world()
                .contains_resource::<crate::map::MapCatalogResource>()
        );
        assert!(!app.is_plugin_added::<crate::protocol::ProtocolPlugin>());
        assert!(
            !app.world()
                .contains_resource::<GameplayContentFingerprint>()
        );
        let before_startup = gameplay_content_fingerprint_from_world(app.world()).unwrap();

        app.update();

        assert_eq!(
            *app.world().resource::<GameplayContentFingerprint>(),
            before_startup
        );
    }

    #[test]
    fn contributor_order_is_irrelevant_and_duplicate_or_missing_domains_fail() {
        let mut app = app_with_synthetic("test.synthetic", 1, synthetic_material);
        let baseline = gameplay_content_fingerprint_from_world(app.world()).unwrap();
        app.world_mut()
            .resource_mut::<GameplayFingerprintRegistry>()
            .registrations
            .reverse();
        assert_eq!(
            gameplay_content_fingerprint_from_world(app.world()).unwrap(),
            baseline
        );

        register_gameplay_fingerprint_contributor(
            &mut app,
            "test.synthetic",
            1,
            synthetic_material,
        );
        assert!(
            gameplay_content_fingerprint_from_world(app.world())
                .unwrap_err()
                .contains("duplicate")
        );

        let mut missing = App::new();
        register_gameplay_fingerprint_contributor(
            &mut missing,
            "test.synthetic",
            1,
            synthetic_material,
        );
        assert!(
            gameplay_content_fingerprint_from_world(missing.world())
                .unwrap_err()
                .contains("missing required")
        );
    }

    #[test]
    fn contributor_capacity_is_bounded() {
        let mut app = App::new();
        app.add_plugins(GameplayContentPlugin);
        let registration = GameplayFingerprintRegistration {
            domain_id: "test.capacity",
            domain_schema_version: 1,
            material: synthetic_material,
        };
        app.world_mut()
            .resource_mut::<GameplayFingerprintRegistry>()
            .registrations
            .extend(std::iter::repeat_n(
                registration,
                MAX_GAMEPLAY_FINGERPRINT_CONTRIBUTORS,
            ));
        assert!(
            gameplay_content_fingerprint_from_world(app.world())
                .unwrap_err()
                .contains("capacity")
        );
    }

    #[test]
    fn domain_identity_schema_and_material_affect_fingerprint() {
        let baseline = gameplay_content_fingerprint_from_world(
            app_with_synthetic("test.synthetic-a", 1, synthetic_material).world(),
        )
        .unwrap();
        assert_ne!(
            gameplay_content_fingerprint_from_world(
                app_with_synthetic("test.synthetic-b", 1, synthetic_material).world()
            )
            .unwrap(),
            baseline
        );
        assert_ne!(
            gameplay_content_fingerprint_from_world(
                app_with_synthetic("test.synthetic-a", 1, alternate_synthetic_material).world()
            )
            .unwrap(),
            baseline
        );

        assert_ne!(
            gameplay_content_fingerprint_from_world(
                app_with_synthetic("test.synthetic-a", 2, synthetic_material).world()
            )
            .unwrap(),
            baseline
        );
    }

    #[test]
    fn live_weapon_part_and_condition_resources_affect_fingerprint() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, GameplayContentPlugin));
        let baseline = gameplay_content_fingerprint_from_world(app.world()).unwrap();

        app.world_mut()
            .resource_mut::<crate::weapon_parts::WeaponPartCatalogResource>()
            .0
            .starter_set_revision += 1;
        assert_ne!(
            gameplay_content_fingerprint_from_world(app.world()).unwrap(),
            baseline
        );

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, GameplayContentPlugin));
        app.world_mut()
            .resource_mut::<crate::combat::CombatConditionRulesResource>()
            .0
            .cold_decay_delay_ticks += 1;
        assert_ne!(
            gameplay_content_fingerprint_from_world(app.world()).unwrap(),
            baseline
        );
    }

    #[test]
    fn global_fingerprint_includes_authored_concealment_rules() {
        let fingerprint_for_rules = |rules: crate::concealment::ConcealmentRules| {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, GameplayContentPlugin));
            app.world_mut()
                .insert_resource(crate::concealment::ConcealmentRulesResource(rules));
            app.update();
            *app.world().resource::<GameplayContentFingerprint>()
        };

        let authored = crate::concealment::ConcealmentRules::embedded().unwrap();
        let authored_fingerprint = fingerprint_for_rules(authored);
        assert_ne!(
            authored_fingerprint,
            fingerprint_for_rules(crate::concealment::ConcealmentRules {
                attack_reveal_ticks: authored.attack_reveal_ticks + 1,
                ..authored
            })
        );
        assert_ne!(
            authored_fingerprint,
            fingerprint_for_rules(crate::concealment::ConcealmentRules {
                damage_reveal_ticks: authored.damage_reveal_ticks + 1,
                ..authored
            })
        );
    }

    #[test]
    fn global_fingerprint_includes_authored_direct_diagnostic_policy() {
        let weapons = crate::combat::WeaponCatalog::embedded().unwrap();
        let maps = crate::map::MapContentCatalog::embedded().unwrap();
        let builds = crate::builds::BuildCatalog::embedded().unwrap();
        let baseline = gameplay_content_fingerprint(&weapons, &maps, &builds).unwrap();

        let mut changed = builds;
        changed.direct_diagnostic.weapon_base_ids.rotate_left(1);
        changed.validate_weapon_references(&weapons).unwrap();

        assert_ne!(
            gameplay_content_fingerprint(&weapons, &maps, &changed).unwrap(),
            baseline
        );
    }

    #[test]
    fn invalid_live_build_weapon_reference_fails_fingerprinting() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, GameplayContentPlugin));
        app.world_mut()
            .resource_mut::<crate::builds::BuildCatalogResource>()
            .0
            .direct_diagnostic
            .weapon_base_ids[0] = crate::profiles::WeaponBaseId(u16::MAX);
        assert!(gameplay_content_fingerprint_from_world(app.world()).is_err());
    }
}
