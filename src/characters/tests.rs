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

#[test]
fn practice_bot_rosters_cover_every_character_weapon_and_ultimate_without_repeats() {
    let (characters, builds, weapons, _) = embedded();
    let roster_seeds = 0..u64::try_from(2 * characters.characters.len()).unwrap();
    // Bot seats for the routed 1v1, 2v2 and 3v3 Practice compositions.
    let compositions = [1_usize, 3, 5];
    let mut picked = BTreeSet::new();
    for bots in compositions {
        let mut single_seat = BTreeSet::new();
        for seed in roster_seeds.clone() {
            let roster: Vec<_> = (0..bots)
                .map(|ordinal| characters.practice_bot_choice(seed, ordinal))
                .collect();
            assert!(roster.iter().all(|choice| characters.accepts(*choice)));
            let distinct: BTreeSet<_> = roster.iter().map(|choice| choice.character_id).collect();
            assert_eq!(distinct.len(), bots, "a roster never repeats a character");
            assert_eq!(
                roster,
                (0..bots)
                    .map(|ordinal| characters.practice_bot_choice(seed, ordinal))
                    .collect::<Vec<_>>(),
                "picks are deterministic"
            );
            single_seat.insert((roster[0].character_id, roster[0].ultimate_id));
            picked.extend(roster);
        }
        assert_eq!(
            single_seat.len(),
            2 * characters.characters.len(),
            "consecutive starts give the first seat every character with both ultimates"
        );
    }
    let picked_weapons: BTreeSet<_> = picked
        .iter()
        .map(|choice| {
            characters
                .character(choice.character_id)
                .unwrap()
                .weapon
                .base
        })
        .collect();
    let all_weapons: BTreeSet<_> = weapons.presets.iter().map(|preset| preset.id).collect();
    assert_eq!(picked_weapons, all_weapons);
    let picked_ultimates: BTreeSet<_> = picked.iter().map(|choice| choice.ultimate_id).collect();
    let all_ultimates: BTreeSet<_> = builds
        .ultimates
        .iter()
        .map(|ultimate| ultimate.id)
        .collect();
    assert_eq!(picked_ultimates, all_ultimates);
    let picked_passives: BTreeSet<_> = picked.iter().map(|choice| choice.passive_id).collect();
    let all_passives: BTreeSet<_> = builds.passives.iter().map(|passive| passive.id).collect();
    assert_eq!(picked_passives, all_passives);
}

#[test]
fn a_choice_carried_onto_revised_options_keeps_its_slot() {
    let (characters, ..) = embedded();
    let original = &characters.characters[0];
    let mut revised = original.clone();
    let choice = CharacterChoice {
        character_id: original.id,
        ultimate_id: original.ultimate_options[0],
        passive_id: original.passive_options[1],
    };
    assert_eq!(choice.carried_by_slot(original, &revised), choice);

    revised.ultimate_options = [
        crate::builds::UltimateDefinitionId(11),
        original.ultimate_options[1],
    ];
    revised.passive_options = [
        original.passive_options[1],
        crate::builds::PassiveDefinitionId(9),
    ];
    let carried = choice.carried_by_slot(original, &revised);
    // The replaced ultimate takes its slot's new option; the passive is still offered (moved).
    assert_eq!(carried.ultimate_id, crate::builds::UltimateDefinitionId(11));
    assert_eq!(carried.passive_id, original.passive_options[1]);
    assert!(carried.is_offered_by(&revised));
}
