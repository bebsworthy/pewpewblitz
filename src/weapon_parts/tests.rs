use super::*;
use crate::combat::{FighterDefinitions, WeaponCatalog, WeaponPresetId};

#[test]
fn embedded_catalog_is_valid_and_has_twelve_sidegrades() {
    let catalog = WeaponPartCatalog::embedded().unwrap();
    assert_eq!(catalog.definitions.len(), 12);
    assert_ne!(catalog.fingerprint().unwrap().0, 0);
}

#[test]
fn slot_permutation_has_the_same_modifiers_and_weapon_fingerprint() {
    let parts = WeaponPartCatalog::embedded().unwrap();
    let first: Vec<_> = parts
        .definitions
        .iter()
        .take(4)
        .flat_map(|definition| definition.effects.iter().copied())
        .collect();
    let mut reversed = first.clone();
    reversed.reverse();
    let first = aggregate_weapon_part_effects(first).unwrap();
    let reversed = aggregate_weapon_part_effects(reversed).unwrap();
    assert_eq!(first, reversed);

    let weapons = WeaponCatalog::embedded().unwrap();
    let fighters = FighterDefinitions::default();
    let fighter = &fighters.entries[0];
    assert_eq!(
        resolve_weapon_parts(&weapons, fighter, WeaponPresetId(1), first)
            .unwrap()
            .recipe_fingerprint,
        resolve_weapon_parts(&weapons, fighter, WeaponPresetId(1), reversed)
            .unwrap()
            .recipe_fingerprint
    );
}

#[test]
fn every_legacy_part_resolves_on_every_weapon_base() {
    let parts = WeaponPartCatalog::embedded().unwrap();
    let weapons = WeaponCatalog::embedded().unwrap();
    let fighters = FighterDefinitions::default();
    let fighter = &fighters.entries[0];
    for definition in parts.definitions.iter().take(8) {
        let modifiers = aggregate_weapon_part_effects(definition.effects.iter().copied()).unwrap();
        for base in 1..=6 {
            resolve_weapon_parts(&weapons, fighter, WeaponPresetId(base), modifiers).unwrap();
        }
        let splash = resolve_weapon_parts(&weapons, fighter, WeaponPresetId(7), modifiers);
        if definition.id == WeaponPartDefinitionId(7) {
            assert_eq!(
                splash.unwrap_err(),
                WeaponPartModelError::IncompatibleWeapon
            );
        } else {
            splash.unwrap();
        }
    }
}

#[test]
fn every_legal_zero_to_four_part_combination_resolves_on_its_compatible_bases() {
    let parts = WeaponPartCatalog::embedded().unwrap();
    let weapons = WeaponCatalog::embedded().unwrap();
    let fighters = FighterDefinitions::default();
    let fighter = &fighters.entries[0];
    for mask in 0_u16..(1_u16 << parts.definitions.len()) {
        if mask.count_ones() > 4 {
            continue;
        }
        let effects = parts
            .definitions
            .iter()
            .enumerate()
            .filter(|(index, _)| mask & (1 << index) != 0)
            .flat_map(|(_, definition)| definition.effects.iter().copied());
        let Ok(modifiers) = aggregate_weapon_part_effects(effects) else {
            continue;
        };
        for base in 1..=7 {
            let _ = resolve_weapon_parts(&weapons, fighter, WeaponPresetId(base), modifiers);
        }
    }
}
