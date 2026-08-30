//! Neutral envelope and application composition for shared authored gameplay content.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub const GAMEPLAY_CONTENT_ENVELOPE_VERSION: u16 = 26;

#[derive(Resource, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GameplayContentFingerprint(pub u64);

/// Installs the validated, build-embedded gameplay catalogs used by every process role.
///
/// This plugin is deliberately headless-safe. Client-only presentation catalogs such as audio
/// and VFX remain owned by their presentation plugins and cannot leak into the server feature
/// graph through this shared composition boundary.
pub struct GameplayContentPlugin;

impl Plugin for GameplayContentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<crate::combat::WeaponCatalogResource>()
            .init_resource::<crate::combat::CombatConditionRulesResource>()
            .add_plugins(crate::concealment::ConcealmentContentPlugin)
            .add_plugins(crate::bots::BotContentPlugin)
            .add_plugins(crate::builds::BuildContentPlugin)
            .add_plugins(crate::weapon_parts::WeaponPartContentPlugin)
            .add_plugins(crate::map::MapContentPlugin)
            .add_systems(Startup, initialize_content_fingerprint);
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(crate) fn initialize_content_fingerprint(
    weapons: Res<crate::combat::WeaponCatalogResource>,
    maps: Res<crate::map::MapCatalogResource>,
    builds: Res<crate::builds::BuildCatalogResource>,
    concealment: Res<crate::concealment::ConcealmentRulesResource>,
    bots: Res<crate::bots::BotCatalogResource>,
    mut commands: Commands,
) {
    let fingerprint = gameplay_content_fingerprint_with_shared_rules(
        &weapons.0,
        &maps.0,
        &builds.0,
        concealment.0,
        &bots.0,
    )
    .expect("embedded gameplay catalogs must fingerprint");
    commands.insert_resource(fingerprint);
}

pub fn gameplay_content_fingerprint(
    weapons: &crate::combat::WeaponCatalog,
    maps: &crate::map::MapContentCatalog,
    builds: &crate::builds::BuildCatalog,
) -> Result<GameplayContentFingerprint, String> {
    gameplay_content_fingerprint_with_shared_rules(
        weapons,
        maps,
        builds,
        crate::concealment::ConcealmentRules::embedded()?,
        &crate::bots::BotCatalog::embedded()?,
    )
}

fn gameplay_content_fingerprint_with_shared_rules(
    weapons: &crate::combat::WeaponCatalog,
    maps: &crate::map::MapContentCatalog,
    builds: &crate::builds::BuildCatalog,
    concealment: crate::concealment::ConcealmentRules,
    bots: &crate::bots::BotCatalog,
) -> Result<GameplayContentFingerprint, String> {
    builds.validate_weapon_references(weapons)?;
    let weapon_material = weapons.canonical_fingerprint_material()?;
    let map_material = maps.canonical_fingerprint_material()?;
    let build_material = builds.canonical_fingerprint_material()?;
    let part_material =
        crate::weapon_parts::WeaponPartCatalog::embedded()?.canonical_fingerprint_material()?;
    let condition_material =
        crate::combat::CombatConditionRules::embedded()?.canonical_fingerprint_material()?;
    let bot_material = bots.canonical_fingerprint_material()?;
    let concealment_material = concealment.canonical_fingerprint_material()?;
    let bytes = postcard::to_allocvec(&(
        GAMEPLAY_CONTENT_ENVELOPE_VERSION,
        weapon_material,
        map_material,
        build_material,
        part_material,
        condition_material,
        bot_material,
        concealment_material,
    ))
    .map_err(|error| format!("gameplay content envelope serialization failed: {error}"))?;
    Ok(GameplayContentFingerprint(fnv1a64(&bytes)))
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

    #[test]
    fn content_plugin_installs_catalogs_and_fingerprint_without_protocol() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, GameplayContentPlugin));

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

        app.update();

        assert!(
            app.world()
                .contains_resource::<GameplayContentFingerprint>()
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
}
