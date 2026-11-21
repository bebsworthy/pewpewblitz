//! Roster character editor projection: one comparison column per character (BRL-5v3c).

use super::{
    EditorFieldDescriptor, EditorOption, EditorPathSegment, EditorSection, NumberSpec,
    add_choice_field, add_field,
};
use crate::builds::{
    MAX_COLD_CAPACITY, MAX_FIGHTER_MOVEMENT_SPEED, MAX_REVEAL_PROXIMITY_RADIUS,
    MIN_REVEAL_PROXIMITY_RADIUS,
};

fn stat_fields() -> [(&'static str, &'static str, &'static str, NumberSpec); 9] {
    [
        (
            "maximum_health",
            "Core stats",
            "Maximum health",
            NumberSpec::integer(1, u32::from(u16::MAX), "health"),
        ),
        (
            "movement_speed",
            "Core stats",
            "Movement speed",
            NumberSpec::decimal(
                1.0,
                f64::from(MAX_FIGHTER_MOVEMENT_SPEED),
                1.0,
                "world units/s",
            )
            .ranged(),
        ),
        (
            "health_recovery_rate",
            "Recovery",
            "Health recovery rate",
            NumberSpec::integer(1, u32::from(u16::MAX), "health/s"),
        ),
        (
            "idle_attack_delay_ticks",
            "Recovery",
            "Attack-idle delay",
            NumberSpec::ticks(1, u64::from(u32::MAX)),
        ),
        (
            "reveal_proximity_radius",
            "Concealment",
            "Reveal proximity",
            NumberSpec::decimal(
                f64::from(MIN_REVEAL_PROXIMITY_RADIUS),
                f64::from(MAX_REVEAL_PROXIMITY_RADIUS),
                1.0,
                "world units",
            )
            .ranged(),
        ),
        (
            "cold_capacity",
            "Elemental baselines",
            "Cold capacity",
            NumberSpec::integer(1, u32::from(MAX_COLD_CAPACITY), "cold")
                .help("Target-owned buildup required to trigger Freeze."),
        ),
        (
            "cold_resistance_basis_points",
            "Elemental baselines",
            "Cold resistance",
            NumberSpec::basis_points(0, 6_000),
        ),
        (
            "poison_resistance_basis_points",
            "Elemental baselines",
            "Poison resistance",
            NumberSpec::basis_points(0, 6_000),
        ),
        (
            "fire_resistance_basis_points",
            "Elemental baselines",
            "Fire resistance",
            NumberSpec::basis_points(0, 6_000),
        ),
    ]
}

fn options<'a>(entries: impl Iterator<Item = (u16, &'a str)>) -> Vec<EditorOption> {
    entries
        .map(|(value, label)| EditorOption {
            value,
            label: label.to_string(),
        })
        .collect()
}

pub(super) fn add_fields(
    fields: &mut Vec<EditorFieldDescriptor>,
    snapshot: &super::super::BalanceLabSnapshotV3,
) {
    let weapon_options = options(
        snapshot
            .weapons
            .iter()
            .map(|weapon| (weapon.id, weapon.display_name.as_str())),
    );
    let parts = crate::weapon_parts::WeaponPartCatalog::embedded()
        .expect("embedded weapon-part catalog is valid");
    let part_options = options(
        std::iter::once((0, "None")).chain(
            parts
                .definitions
                .iter()
                .map(|part| (part.id.0, part.display_name.as_str())),
        ),
    );
    let ultimate_options = options(
        snapshot
            .ultimates
            .iter()
            .map(|ultimate| (ultimate.id, ultimate.display_name.as_str())),
    );
    let passive_options = options(
        snapshot
            .passives
            .iter()
            .map(|passive| (passive.id, passive.display_name.as_str())),
    );
    for (index, character) in snapshot.characters.iter().enumerate() {
        let subject = (character.key.as_str(), character.display_name.as_str());
        for (leaf, group, label, spec) in stat_fields() {
            add_field(
                fields,
                path!["characters", index, "stats", leaf],
                EditorSection::Characters,
                leaf,
                subject.0,
                subject.1,
                group,
                label,
                spec,
            );
        }
        add_choice_field(
            fields,
            path!["characters", index, "weaponBase"],
            EditorSection::Characters,
            "weaponBase",
            subject.0,
            subject.1,
            "Signature weapon",
            "Weapon base",
            weapon_options.clone(),
            "The base recipe this character fires; tune its numbers under Weapons.",
        );
        for slot in 0..character.weapon_parts.len() {
            add_choice_field(
                fields,
                path!["characters", index, "weaponParts", slot],
                EditorSection::Characters,
                &format!("weaponParts/{slot}"),
                subject.0,
                subject.1,
                "Signature weapon",
                &format!("Authored part {}", slot + 1),
                part_options.clone(),
                "An authored weapon part folded into the signature weapon; None leaves the slot empty.",
            );
        }
        for (key, label, list, help) in [
            (
                "ultimateOptions",
                "Ultimate option",
                &ultimate_options,
                "One of the two ultimates players choose between; the two must differ.",
            ),
            (
                "passiveOptions",
                "Passive option",
                &passive_options,
                "One of the two passives players choose between; the two must differ.",
            ),
        ] {
            for slot in 0..crate::characters::CHARACTER_OPTION_COUNT {
                add_choice_field(
                    fields,
                    path!["characters", index, key, slot],
                    EditorSection::Characters,
                    &format!("{key}/{slot}"),
                    subject.0,
                    subject.1,
                    "Options",
                    &format!("{label} {}", slot + 1),
                    list.clone(),
                    help,
                );
            }
        }
    }
}
