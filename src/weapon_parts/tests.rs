use super::*;
use crate::combat::{WeaponCatalog, WeaponPresetId};

#[test]
fn embedded_catalog_is_valid_and_has_twelve_sidegrades() {
    let catalog = WeaponPartCatalog::embedded().unwrap();
    assert_eq!(catalog.definitions.len(), 12);
    assert_ne!(catalog.fingerprint().unwrap().0, 0);
}

#[test]
fn catalog_accepts_an_additive_part_definition() {
    let mut catalog = WeaponPartCatalog::embedded().unwrap();
    let mut thirteenth = catalog.definitions.last().unwrap().clone();
    thirteenth.id = WeaponPartDefinitionId(13);
    thirteenth.key = "thirteenth-part".into();
    thirteenth.display_name = "Thirteenth Part".into();
    catalog.starter_set_revision += 1;
    catalog.definitions.push(thirteenth);

    assert!(catalog.validate().is_ok());
    assert!(catalog.fingerprint().is_ok());
}

#[test]
fn additive_part_catalog_rejects_duplicate_keys_and_inventory_overrun() {
    let embedded = WeaponPartCatalog::embedded().unwrap();
    let mut duplicate = embedded.clone();
    duplicate.definitions[1].key = duplicate.definitions[0].key.clone();
    assert!(duplicate.validate().is_err());

    let mut regressed_revision = embedded.clone();
    regressed_revision.starter_set_revision = 1;
    assert!(regressed_revision.validate().is_err());

    let mut oversized = embedded;
    let template = oversized.definitions.last().unwrap().clone();
    for id in 13..=u16::try_from(MAX_WEAPON_PARTS_PER_PROFILE).unwrap() {
        let mut definition = template.clone();
        definition.id = WeaponPartDefinitionId(id);
        definition.key = format!("part-{id}");
        definition.display_name = format!("Part {id}");
        oversized.definitions.push(definition);
    }
    assert!(oversized.validate().is_ok());

    let id = u16::try_from(MAX_WEAPON_PARTS_PER_PROFILE + 1).unwrap();
    let mut definition = template;
    definition.id = WeaponPartDefinitionId(id);
    definition.key = format!("part-{id}");
    definition.display_name = format!("Part {id}");
    oversized.definitions.push(definition);
    assert!(oversized.validate().is_err());
}

#[test]
fn cold_part_uses_the_combat_payload_amount_boundary() {
    let maximum = crate::combat::definitions::MAX_COLD_PAYLOAD_AMOUNT;
    assert!(
        WeaponPartEffect::Cold { amount: maximum }
            .validate()
            .is_ok()
    );
    assert!(
        WeaponPartEffect::Cold {
            amount: maximum + 1,
        }
        .validate()
        .is_err()
    );
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
    let body = crate::builds::BuildCatalog::embedded()
        .unwrap()
        .fighter_body;
    assert_eq!(
        resolve_weapon_parts(&weapons, body, WeaponPresetId(1), first)
            .unwrap()
            .recipe_fingerprint,
        resolve_weapon_parts(&weapons, body, WeaponPresetId(1), reversed)
            .unwrap()
            .recipe_fingerprint
    );
}

#[test]
fn every_legacy_part_resolves_on_every_weapon_base() {
    let parts = WeaponPartCatalog::embedded().unwrap();
    let weapons = WeaponCatalog::embedded().unwrap();
    let body = crate::builds::BuildCatalog::embedded()
        .unwrap()
        .fighter_body;
    for definition in parts.definitions.iter().take(8) {
        let modifiers = aggregate_weapon_part_effects(definition.effects.iter().copied()).unwrap();
        for base in 1..=6 {
            resolve_weapon_parts(&weapons, body, WeaponPresetId(base), modifiers).unwrap();
        }
        let splash = resolve_weapon_parts(&weapons, body, WeaponPresetId(7), modifiers);
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
    let body = crate::builds::BuildCatalog::embedded()
        .unwrap()
        .fighter_body;
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
            let _ = resolve_weapon_parts(&weapons, body, WeaponPresetId(base), modifiers);
        }
    }
}
