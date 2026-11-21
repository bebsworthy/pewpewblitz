//! Per-character roster tuning (BRL-5v3c): stats, signature weapon recipe, and option lists.

use crate::{
    builds::{BuildCatalog, PassiveDefinitionId, ResolvedFighterStats, UltimateDefinitionId},
    characters::{CHARACTER_OPTION_COUNT, CharacterCatalog, CharacterWeaponRecipe},
    combat::{WeaponCatalog, WeaponPresetId},
    weapon_parts::{WEAPON_PART_SLOT_COUNT, WeaponPartCatalog, WeaponPartDefinitionId},
};
use serde::{Deserialize, Serialize};

/// One roster character's tunable kit. Identity is locked; the weapon parts are fixed slots where
/// `0` is an empty slot, so the snapshot topology never changes during a tuning transaction.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(super) struct CharacterTuning {
    pub(super) id: u16,
    pub(super) key: String,
    pub(super) display_name: String,
    pub(super) stats: ResolvedFighterStats,
    pub(super) weapon_base: u16,
    pub(super) weapon_parts: [u16; WEAPON_PART_SLOT_COUNT],
    pub(super) ultimate_options: [u16; CHARACTER_OPTION_COUNT],
    pub(super) passive_options: [u16; CHARACTER_OPTION_COUNT],
}

pub(super) fn from_catalog(characters: &CharacterCatalog) -> Vec<CharacterTuning> {
    characters
        .characters
        .iter()
        .map(|character| {
            let mut weapon_parts = [0; WEAPON_PART_SLOT_COUNT];
            for (slot, part) in weapon_parts.iter_mut().zip(&character.weapon.parts) {
                *slot = part.0;
            }
            CharacterTuning {
                id: character.id.0,
                key: character.key.clone(),
                display_name: character.display_name.clone(),
                stats: character.stats,
                weapon_base: character.weapon.base.0,
                weapon_parts,
                ultimate_options: character.ultimate_options.map(|id| id.0),
                passive_options: character.passive_options.map(|id| id.0),
            }
        })
        .collect()
}

/// Build the revised roster from the canonical catalog and a candidate, rejecting unknown
/// references with path-qualified errors, then require every option combination to resolve.
pub(super) fn revise(
    candidate: &[CharacterTuning],
    baseline: &[CharacterTuning],
    canonical: &CharacterCatalog,
    builds: &BuildCatalog,
    weapons: &WeaponCatalog,
    parts: &WeaponPartCatalog,
) -> Result<CharacterCatalog, String> {
    if candidate.len() != baseline.len() || canonical.characters.len() != baseline.len() {
        return Err("character roster topology changed".into());
    }
    let mut next = canonical.clone();
    for (index, ((supplied, expected), character)) in candidate
        .iter()
        .zip(baseline)
        .zip(&mut next.characters)
        .enumerate()
    {
        if supplied.id != expected.id
            || supplied.key != expected.key
            || supplied.display_name != expected.display_name
            || character.id.0 != expected.id
        {
            return Err("character identity changed".into());
        }
        let field =
            |leaf: &str, message: &str| format!("field /characters/{index}/{leaf}: {message}");
        crate::builds::validate_fighter_stats(&supplied.stats)
            .map_err(|error| format!("character {}: {error}", supplied.key))?;
        if weapons
            .preset(WeaponPresetId(supplied.weapon_base))
            .is_none()
        {
            return Err(field("weaponBase", "unknown weapon base"));
        }
        for (slot, part) in supplied.weapon_parts.iter().enumerate() {
            if *part != 0 && parts.definition(WeaponPartDefinitionId(*part)).is_none() {
                return Err(field(&format!("weaponParts/{slot}"), "unknown weapon part"));
            }
        }
        for (slot, ultimate) in supplied.ultimate_options.iter().enumerate() {
            if builds.ultimate(UltimateDefinitionId(*ultimate)).is_none() {
                return Err(field(
                    &format!("ultimateOptions/{slot}"),
                    "unknown ultimate",
                ));
            }
        }
        if supplied.ultimate_options[0] == supplied.ultimate_options[1] {
            return Err(field(
                "ultimateOptions/1",
                "the two ultimate options must differ",
            ));
        }
        for (slot, passive) in supplied.passive_options.iter().enumerate() {
            if builds.passive(PassiveDefinitionId(*passive)).is_none() {
                return Err(field(&format!("passiveOptions/{slot}"), "unknown passive"));
            }
        }
        if supplied.passive_options[0] == supplied.passive_options[1] {
            return Err(field(
                "passiveOptions/1",
                "the two passive options must differ",
            ));
        }
        character.stats = supplied.stats;
        character.weapon = CharacterWeaponRecipe {
            base: WeaponPresetId(supplied.weapon_base),
            parts: supplied
                .weapon_parts
                .iter()
                .filter(|part| **part != 0)
                .map(|part| WeaponPartDefinitionId(*part))
                .collect(),
        };
        character.ultimate_options = supplied.ultimate_options.map(UltimateDefinitionId);
        character.passive_options = supplied.passive_options.map(PassiveDefinitionId);
    }
    next.validate_option_combinations(builds, weapons, parts)
        .map_err(|error| format!("revised catalogs break a roster character: {error}"))?;
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        canonical: CharacterCatalog,
        builds: BuildCatalog,
        weapons: WeaponCatalog,
        parts: WeaponPartCatalog,
        baseline: Vec<CharacterTuning>,
    }

    fn fixture() -> Fixture {
        let canonical = CharacterCatalog::embedded().unwrap();
        let baseline = from_catalog(&canonical);
        Fixture {
            canonical,
            builds: BuildCatalog::embedded().unwrap(),
            weapons: WeaponCatalog::embedded().unwrap(),
            parts: WeaponPartCatalog::embedded().unwrap(),
            baseline,
        }
    }

    fn revise_with(
        fixture: &Fixture,
        candidate: &[CharacterTuning],
    ) -> Result<CharacterCatalog, String> {
        revise(
            candidate,
            &fixture.baseline,
            &fixture.canonical,
            &fixture.builds,
            &fixture.weapons,
            &fixture.parts,
        )
    }

    #[test]
    fn canonical_tuning_round_trips_to_the_canonical_catalog() {
        let fixture = fixture();
        assert_eq!(fixture.baseline.len(), fixture.canonical.characters.len());
        assert_eq!(
            revise_with(&fixture, &fixture.baseline).unwrap(),
            fixture.canonical
        );
    }

    #[test]
    fn stats_weapon_parts_and_options_install_into_the_revised_roster() {
        let fixture = fixture();
        let mut candidate = fixture.baseline.clone();
        candidate[0].stats.maximum_health = 1_337;
        candidate[0].stats.movement_speed = 81.5;
        candidate[0].weapon_base = 3;
        candidate[0].weapon_parts = [0, 3, 0, 5];
        candidate[0].ultimate_options = [11, 2];
        candidate[0].passive_options = [9, 1];

        let revised = revise_with(&fixture, &candidate).unwrap();
        let riff = &revised.characters[0];
        assert_eq!(riff.stats.maximum_health, 1_337);
        assert!((riff.stats.movement_speed - 81.5).abs() < f32::EPSILON);
        assert_eq!(riff.weapon.base, WeaponPresetId(3));
        assert_eq!(
            riff.weapon.parts,
            vec![WeaponPartDefinitionId(3), WeaponPartDefinitionId(5)]
        );
        assert_eq!(
            riff.ultimate_options,
            [UltimateDefinitionId(11), UltimateDefinitionId(2)]
        );
        assert_eq!(
            riff.passive_options,
            [PassiveDefinitionId(9), PassiveDefinitionId(1)]
        );
        assert_eq!(revised.characters[1..], fixture.canonical.characters[1..]);
    }

    #[test]
    fn unknown_or_duplicate_references_reject_with_their_field_path() {
        let fixture = fixture();
        for (mutate, expected) in [
            (
                (|tuning: &mut CharacterTuning| tuning.weapon_base = 99)
                    as fn(&mut CharacterTuning),
                "field /characters/2/weaponBase:",
            ),
            (
                |tuning| tuning.weapon_parts[3] = 99,
                "field /characters/2/weaponParts/3:",
            ),
            (
                |tuning| tuning.ultimate_options[1] = 99,
                "field /characters/2/ultimateOptions/1:",
            ),
            (
                |tuning| tuning.ultimate_options[1] = tuning.ultimate_options[0],
                "field /characters/2/ultimateOptions/1:",
            ),
            (
                |tuning| tuning.passive_options[0] = 0,
                "field /characters/2/passiveOptions/0:",
            ),
            (
                |tuning| tuning.passive_options[1] = tuning.passive_options[0],
                "field /characters/2/passiveOptions/1:",
            ),
        ] {
            let mut candidate = fixture.baseline.clone();
            mutate(&mut candidate[2]);
            let error = revise_with(&fixture, &candidate).unwrap_err();
            assert!(error.starts_with(expected), "{error}");
        }
    }

    #[test]
    fn identity_topology_and_stat_bounds_are_locked() {
        let fixture = fixture();
        let mut renamed = fixture.baseline.clone();
        renamed[0].display_name = "Not Riff".into();
        assert!(revise_with(&fixture, &renamed).is_err());

        let mut shortened = fixture.baseline.clone();
        shortened.pop();
        assert!(revise_with(&fixture, &shortened).is_err());

        let mut stalled = fixture.baseline.clone();
        stalled[0].stats.movement_speed = 0.0;
        assert!(revise_with(&fixture, &stalled).is_err());
    }
}
