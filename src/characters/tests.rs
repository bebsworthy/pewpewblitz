use super::*;
use crate::builds::BuildCatalog;
use crate::combat::WeaponCatalog;
use crate::weapon_parts::WeaponPartCatalog;
use std::collections::BTreeSet;

fn embedded() -> (
    CharacterCatalog,
    BuildCatalog,
    WeaponCatalog,
    WeaponPartCatalog,
) {
    (
        CharacterCatalog::embedded().unwrap(),
        BuildCatalog::embedded().unwrap(),
        WeaponCatalog::embedded().unwrap(),
        WeaponPartCatalog::embedded().unwrap(),
    )
}

#[test]
fn launch_roster_is_nine_characters_with_stable_ids_and_references() {
    let (characters, builds, weapons, parts) = embedded();
    characters
        .validate_references(&builds, &weapons, &parts)
        .unwrap();
    let keys: Vec<_> = characters
        .characters
        .iter()
        .map(|character| (character.id.0, character.key.as_str()))
        .collect();
    assert_eq!(
        keys,
        [
            (1, "riff"),
            (2, "torque"),
            (3, "cobble"),
            (4, "plunk"),
            (5, "pollen"),
            (6, "orbit"),
            (7, "sumi"),
            (8, "prism"),
            (9, "bumper"),
        ]
    );
}

#[test]
fn launch_roster_covers_every_weapon_base_ultimate_and_passive() {
    let (characters, builds, weapons, _) = embedded();
    let used_weapons: BTreeSet<_> = characters
        .characters
        .iter()
        .map(|character| character.weapon.base)
        .collect();
    let all_weapons: BTreeSet<_> = weapons.presets.iter().map(|preset| preset.id).collect();
    assert_eq!(used_weapons, all_weapons);
    let used_ultimates: BTreeSet<_> = characters
        .characters
        .iter()
        .flat_map(|character| character.ultimate_options)
        .collect();
    let all_ultimates: BTreeSet<_> = builds
        .ultimates
        .iter()
        .map(|ultimate| ultimate.id)
        .collect();
    assert_eq!(used_ultimates, all_ultimates);
    let used_passives: BTreeSet<_> = characters
        .characters
        .iter()
        .flat_map(|character| character.passive_options)
        .collect();
    let all_passives: BTreeSet<_> = builds.passives.iter().map(|passive| passive.id).collect();
    assert_eq!(used_passives, all_passives);
}

#[test]
fn catalog_rejects_duplicate_options_unknown_references_and_unsorted_ids() {
    let (characters, builds, weapons, parts) = embedded();
    let mut duplicate = characters.clone();
    duplicate.characters[0].passive_options[1] = duplicate.characters[0].passive_options[0];
    assert!(duplicate.validate().is_err());
    let mut unsorted = characters.clone();
    unsorted.characters.swap(0, 1);
    assert!(unsorted.validate().is_err());
    let mut unknown = characters.clone();
    unknown.characters[0].ultimate_options[0] = crate::builds::UltimateDefinitionId(999);
    assert!(unknown.validate().is_ok());
    assert!(
        unknown
            .validate_references(&builds, &weapons, &parts)
            .is_err()
    );
    let mut too_many_parts = characters;
    too_many_parts.characters[0].weapon.parts =
        vec![crate::weapon_parts::WeaponPartDefinitionId(1); 5];
    assert!(too_many_parts.validate().is_err());
}
