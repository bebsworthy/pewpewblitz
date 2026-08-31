use super::BalanceLabSnapshotV3;
use crate::{
    builds::{
        MAX_COLD_CAPACITY, MAX_FIGHTER_MOVEMENT_SPEED, MAX_REVEAL_PROXIMITY_RADIUS,
        MIN_REVEAL_PROXIMITY_RADIUS, UltimateParameters,
    },
    combat::WeaponCatalog,
    timing::{SIMULATION_TICK_HZ_F64, simulation_seconds_f64},
};
use serde::Serialize;

mod passives;
mod weapons;

pub(super) const EDITOR_SCHEMA_VERSION: u16 = 10;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(super) struct BalanceLabEditorManifest {
    schema_version: u16,
    fields: Vec<EditorFieldDescriptor>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
struct EditorFieldDescriptor {
    path: Vec<EditorPathSegment>,
    section: EditorSection,
    subject_key: String,
    subject_label: String,
    group: String,
    label: String,
    storage_kind: EditorStorageKind,
    unit: String,
    storage_scale: f64,
    minimum: f64,
    maximum: f64,
    minimum_exclusive: bool,
    step: f64,
    control: EditorControl,
    #[serde(skip_serializing_if = "Option::is_none")]
    help: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(untagged)]
enum EditorPathSegment {
    Key(String),
    Index(usize),
}

impl From<&str> for EditorPathSegment {
    fn from(value: &str) -> Self {
        Self::Key(value.to_string())
    }
}

impl From<usize> for EditorPathSegment {
    fn from(value: usize) -> Self {
        Self::Index(value)
    }
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum EditorSection {
    Global,
    Fighters,
    Weapons,
    Ultimates,
    WorldObjects,
    Modes,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum EditorStorageKind {
    Integer,
    Decimal,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum EditorControl {
    Number,
    RangeAndNumber,
}

#[derive(Clone, Copy, Debug)]
struct NumberSpec {
    storage_kind: EditorStorageKind,
    unit: &'static str,
    storage_scale: f64,
    minimum: f64,
    maximum: f64,
    minimum_exclusive: bool,
    step: f64,
    control: EditorControl,
    help: Option<&'static str>,
}

impl NumberSpec {
    fn integer(minimum: u32, maximum: u32, unit: &'static str) -> Self {
        Self {
            storage_kind: EditorStorageKind::Integer,
            unit,
            storage_scale: 1.0,
            minimum: f64::from(minimum),
            maximum: f64::from(maximum),
            minimum_exclusive: false,
            step: 1.0,
            control: EditorControl::Number,
            help: None,
        }
    }

    fn decimal(minimum: f64, maximum: f64, step: f64, unit: &'static str) -> Self {
        Self {
            storage_kind: EditorStorageKind::Decimal,
            unit,
            storage_scale: 1.0,
            minimum,
            maximum,
            minimum_exclusive: false,
            step,
            control: EditorControl::Number,
            help: None,
        }
    }

    fn positive_decimal(maximum: f64, unit: &'static str) -> Self {
        Self {
            minimum_exclusive: true,
            ..Self::decimal(0.0, maximum, 0.1, unit)
        }
    }

    fn ticks(minimum: u32, maximum: u64) -> Self {
        let maximum = u32::try_from(maximum)
            .expect("validated Balance Lab tick ceilings fit the editor representation");
        Self {
            storage_kind: EditorStorageKind::Integer,
            unit: "s",
            storage_scale: SIMULATION_TICK_HZ_F64,
            minimum: simulation_seconds_f64(minimum),
            maximum: simulation_seconds_f64(maximum),
            minimum_exclusive: false,
            step: simulation_seconds_f64(1),
            control: EditorControl::Number,
            help: Some("Enter seconds; saved to the nearest authoritative server tick."),
        }
    }

    fn milliunits(minimum: u32, maximum: u32) -> Self {
        Self {
            storage_kind: EditorStorageKind::Integer,
            unit: "world units",
            storage_scale: 1_000.0,
            minimum: f64::from(minimum) / 1_000.0,
            maximum: f64::from(maximum) / 1_000.0,
            minimum_exclusive: false,
            step: 0.001,
            control: EditorControl::Number,
            help: Some("Displayed in world units and stored to the nearest thousandth."),
        }
    }

    fn resistance_basis_points() -> Self {
        Self {
            storage_kind: EditorStorageKind::Integer,
            unit: "%",
            storage_scale: 100.0,
            minimum: 0.0,
            maximum: 60.0,
            minimum_exclusive: false,
            step: 1.0,
            control: EditorControl::RangeAndNumber,
            help: Some("Displayed as a percentage and stored in basis points."),
        }
    }

    fn basis_points(minimum: u32, maximum: u32) -> Self {
        Self {
            storage_kind: EditorStorageKind::Integer,
            unit: "%",
            storage_scale: 100.0,
            minimum: f64::from(minimum) / 100.0,
            maximum: f64::from(maximum) / 100.0,
            minimum_exclusive: false,
            step: 0.01,
            control: EditorControl::RangeAndNumber,
            help: Some("Displayed as a percentage and stored in basis points."),
        }
    }

    fn per_tick_rate(minimum: u16, maximum: u16, unit: &'static str) -> Self {
        Self {
            storage_kind: EditorStorageKind::Integer,
            unit,
            storage_scale: 1.0 / SIMULATION_TICK_HZ_F64,
            minimum: f64::from(minimum) * SIMULATION_TICK_HZ_F64,
            maximum: f64::from(maximum) * SIMULATION_TICK_HZ_F64,
            minimum_exclusive: false,
            step: SIMULATION_TICK_HZ_F64,
            control: EditorControl::Number,
            help: Some("Displayed per second and stored per authoritative server tick."),
        }
    }

    fn ranged(mut self) -> Self {
        self.control = EditorControl::RangeAndNumber;
        self
    }

    fn help(mut self, help: &'static str) -> Self {
        self.help = Some(help);
        self
    }
}

macro_rules! path {
    ($($part:expr),+ $(,)?) => {
        vec![$(EditorPathSegment::from($part)),+]
    };
}

impl BalanceLabEditorManifest {
    pub(super) fn from_catalogs(snapshot: &BalanceLabSnapshotV3, weapons: &WeaponCatalog) -> Self {
        let mut fields = Vec::new();
        add_global_fields(&mut fields);
        add_fighter_fields(&mut fields);
        for (index, weapon) in snapshot.weapons.iter().enumerate() {
            weapons::add_fields(&mut fields, index, weapon, weapons);
        }
        add_ultimate_fields(&mut fields, snapshot);
        passives::add_fields(&mut fields, snapshot);
        add_effect_tile_fields(&mut fields);
        add_world_fields(&mut fields);
        Self {
            schema_version: EDITOR_SCHEMA_VERSION,
            fields,
        }
    }
}

fn add_effect_tile_fields(fields: &mut Vec<EditorFieldDescriptor>) {
    let multiplier = |minimum_milli: u16, maximum_milli: u16| NumberSpec {
        storage_kind: EditorStorageKind::Integer,
        unit: "×",
        storage_scale: 1_000.0,
        minimum: f64::from(minimum_milli) / 1_000.0,
        maximum: f64::from(maximum_milli) / 1_000.0,
        minimum_exclusive: false,
        step: 0.001,
        control: EditorControl::RangeAndNumber,
        help: Some("Multiplies ordinary player-driven movement; Dash and knockback are unchanged."),
    };
    for (field, label, spec) in [
        (
            "speedMultiplierMilli",
            "Speed multiplier",
            multiplier(
                super::EffectTileTuning::MIN_SPEED_MULTIPLIER_MILLI,
                super::EffectTileTuning::MAX_SPEED_MULTIPLIER_MILLI,
            ),
        ),
        (
            "slowMultiplierMilli",
            "Slow multiplier",
            multiplier(
                super::EffectTileTuning::MIN_SLOW_MULTIPLIER_MILLI,
                super::EffectTileTuning::MAX_SLOW_MULTIPLIER_MILLI,
            ),
        ),
    ] {
        add_field(
            fields,
            path!["effectTiles", field],
            EditorSection::WorldObjects,
            "effect-tiles",
            "Effect tiles",
            "Movement",
            label,
            spec,
        );
    }
    add_field(
        fields,
        path!["effectTiles", "damagePerPulse"],
        EditorSection::WorldObjects,
        "effect-tiles",
        "Effect tiles",
        "Damage",
        "Damage per pulse",
        NumberSpec::integer(
            u32::from(super::EffectTileTuning::MIN_DAMAGE_PER_PULSE),
            u32::from(super::EffectTileTuning::MAX_DAMAGE_PER_PULSE),
            "health",
        ),
    );
    add_field(
        fields,
        path!["effectTiles", "intervalTicks"],
        EditorSection::WorldObjects,
        "effect-tiles",
        "Effect tiles",
        "Damage",
        "Pulse interval",
        NumberSpec::ticks(
            u32::from(super::EffectTileTuning::MIN_INTERVAL_TICKS),
            u64::from(super::EffectTileTuning::MAX_INTERVAL_TICKS),
        ),
    );
}

fn add_global_fields(fields: &mut Vec<EditorFieldDescriptor>) {
    for (field, label, spec) in [
        (
            "cold_decay_delay_ticks",
            "Buildup decay delay",
            NumberSpec::ticks(0, crate::combat::MAX_COLD_RULE_TICKS),
        ),
        (
            "cold_decay_per_tick",
            "Buildup decay rate",
            NumberSpec::per_tick_rate(1, crate::combat::MAX_COLD_DECAY_PER_TICK, "cold/s"),
        ),
        (
            "freeze_duration_ticks",
            "Freeze duration",
            NumberSpec::ticks(1, crate::combat::MAX_COLD_RULE_TICKS),
        ),
        (
            "thaw_immunity_ticks",
            "Post-thaw immunity",
            NumberSpec::ticks(0, crate::combat::MAX_COLD_RULE_TICKS),
        ),
    ] {
        add_field(
            fields,
            path!["conditionRules", field],
            EditorSection::Global,
            "cold",
            "Cold & Freeze",
            "Lifecycle",
            label,
            spec,
        );
    }
    for (field, label, maximum) in [
        ("maximum", "Maximum charge", 10_000),
        ("dealt_damage_multiplier", "Charge per damage dealt", 100),
        (
            "received_damage_multiplier",
            "Charge per damage received",
            100,
        ),
    ] {
        add_field(
            fields,
            path!["ultimateCharge", field],
            EditorSection::Global,
            "ultimate-charge",
            "Ultimate charge",
            "Combat economy",
            label,
            NumberSpec::integer(1, maximum, "charge"),
        );
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "one descriptor owns one complete editor contract"
)]
fn add_field(
    fields: &mut Vec<EditorFieldDescriptor>,
    path: Vec<EditorPathSegment>,
    section: EditorSection,
    subject_key: &str,
    subject_label: &str,
    group: &str,
    label: &str,
    spec: NumberSpec,
) {
    fields.push(EditorFieldDescriptor {
        path,
        section,
        subject_key: subject_key.to_string(),
        subject_label: subject_label.to_string(),
        group: group.to_string(),
        label: label.to_string(),
        storage_kind: spec.storage_kind,
        unit: spec.unit.to_string(),
        storage_scale: spec.storage_scale,
        minimum: spec.minimum,
        maximum: spec.maximum,
        minimum_exclusive: spec.minimum_exclusive,
        step: spec.step,
        control: spec.control,
        help: spec.help.map(str::to_string),
    });
}

fn add_fighter_fields(fields: &mut Vec<EditorFieldDescriptor>) {
    for (key, label) in [
        ("default", "Default"),
        ("lightweight", "Lightweight"),
        ("reinforced", "Reinforced"),
    ] {
        add_field(
            fields,
            path!["fighterProfiles", key, "maximum_health"],
            EditorSection::Fighters,
            key,
            label,
            "Core stats",
            "Maximum health",
            NumberSpec::integer(1, u32::from(u16::MAX), "health"),
        );
        add_field(
            fields,
            path!["fighterProfiles", key, "movement_speed"],
            EditorSection::Fighters,
            key,
            label,
            "Core stats",
            "Movement speed",
            NumberSpec::decimal(
                1.0,
                f64::from(MAX_FIGHTER_MOVEMENT_SPEED),
                1.0,
                "world units/s",
            )
            .ranged(),
        );
        add_field(
            fields,
            path!["fighterProfiles", key, "health_recovery_rate"],
            EditorSection::Fighters,
            key,
            label,
            "Recovery",
            "Health recovery rate",
            NumberSpec::integer(1, u32::from(u16::MAX), "health/s"),
        );
        add_field(
            fields,
            path!["fighterProfiles", key, "idle_attack_delay_ticks"],
            EditorSection::Fighters,
            key,
            label,
            "Recovery",
            "Attack-idle delay",
            NumberSpec::ticks(1, u64::from(u32::MAX)),
        );
        add_field(
            fields,
            path!["fighterProfiles", key, "reveal_proximity_radius"],
            EditorSection::Fighters,
            key,
            label,
            "Concealment",
            "Reveal proximity",
            NumberSpec::decimal(
                f64::from(MIN_REVEAL_PROXIMITY_RADIUS),
                f64::from(MAX_REVEAL_PROXIMITY_RADIUS),
                1.0,
                "world units",
            )
            .ranged(),
        );
        add_field(
            fields,
            path!["fighterProfiles", key, "cold_capacity"],
            EditorSection::Fighters,
            key,
            label,
            "Elemental baselines",
            "Cold capacity",
            NumberSpec::integer(1, u32::from(MAX_COLD_CAPACITY), "cold")
                .help("Target-owned buildup required to trigger Freeze."),
        );
        for (field, field_label) in [
            ("cold_resistance_basis_points", "Cold resistance"),
            ("poison_resistance_basis_points", "Poison resistance"),
            ("fire_resistance_basis_points", "Fire resistance"),
        ] {
            add_field(
                fields,
                path!["fighterProfiles", key, field],
                EditorSection::Fighters,
                key,
                label,
                "Elemental baselines",
                field_label,
                NumberSpec::resistance_basis_points(),
            );
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "explicit descriptors keep the small ultimate editor contract auditable"
)]
fn add_ultimate_fields(fields: &mut Vec<EditorFieldDescriptor>, snapshot: &BalanceLabSnapshotV3) {
    for (index, ultimate) in snapshot.ultimates.iter().enumerate() {
        match ultimate.parameters {
            UltimateParameters::Dash { .. } => {
                for (tail, group, label, spec) in [
                    (
                        "maximum_distance_milliunits",
                        "Movement",
                        "Maximum distance",
                        NumberSpec::milliunits(1, 4_096_000),
                    ),
                    (
                        "duration_ticks",
                        "Movement",
                        "Duration",
                        NumberSpec::ticks(1, 600),
                    ),
                    (
                        "damage",
                        "Impact",
                        "Damage",
                        NumberSpec::integer(1, 1_000, "health"),
                    ),
                    (
                        "knockback_speed_milliunits",
                        "Impact",
                        "Knockback speed",
                        NumberSpec::milliunits(1, 4_096_000),
                    ),
                    (
                        "knockback_duration_ticks",
                        "Impact",
                        "Knockback duration",
                        NumberSpec::ticks(1, 600),
                    ),
                    (
                        "maximum_targets",
                        "Capacity",
                        "Maximum targets",
                        NumberSpec::integer(1, 32, "targets"),
                    ),
                ] {
                    add_field(
                        fields,
                        path!["ultimates", index, "parameters", "Dash", tail],
                        EditorSection::Ultimates,
                        &ultimate.key,
                        &ultimate.display_name,
                        group,
                        label,
                        spec,
                    );
                }
            }
            UltimateParameters::Sentry { .. } => {
                for offset in 0..6 {
                    add_field(
                        fields,
                        path![
                            "ultimates",
                            index,
                            "parameters",
                            "Sentry",
                            "placement_offsets_milliunits",
                            offset
                        ],
                        EditorSection::Ultimates,
                        &ultimate.key,
                        &ultimate.display_name,
                        "Placement",
                        &format!("Placement offset {}", offset + 1),
                        NumberSpec::milliunits(1, 1_024_000),
                    );
                }
                for (tail, group, label, spec) in [
                    (
                        "body_radius_milliunits",
                        "Deployable",
                        "Body radius",
                        NumberSpec::milliunits(1, 512_000),
                    ),
                    (
                        "acquisition_range_milliunits",
                        "Targeting",
                        "Acquisition range",
                        NumberSpec::milliunits(1, 4_096_000),
                    ),
                    (
                        "acquisition_interval_ticks",
                        "Targeting",
                        "Acquisition interval",
                        NumberSpec::ticks(1, 600),
                    ),
                    (
                        "fire_interval_ticks",
                        "Firing",
                        "Fire interval",
                        NumberSpec::ticks(1, 3_600),
                    ),
                    (
                        "lifetime_ticks",
                        "Deployable",
                        "Lifetime",
                        NumberSpec::ticks(1, 36_000),
                    ),
                    (
                        "maximum_health",
                        "Deployable",
                        "Maximum health",
                        NumberSpec::integer(1, 10_000, "health"),
                    ),
                    (
                        "projectile_speed_milliunits",
                        "Projectile",
                        "Speed",
                        NumberSpec::milliunits(1, 4_096_000),
                    ),
                    (
                        "projectile_radius_milliunits",
                        "Projectile",
                        "Radius",
                        NumberSpec::milliunits(1, 512_000),
                    ),
                    (
                        "projectile_range_milliunits",
                        "Projectile",
                        "Range",
                        NumberSpec::milliunits(1, 4_096_000),
                    ),
                    (
                        "projectile_lifetime_ticks",
                        "Projectile",
                        "Flight lifetime",
                        NumberSpec::ticks(1, 600),
                    ),
                    (
                        "projectile_damage",
                        "Projectile",
                        "Damage",
                        NumberSpec::integer(1, 1_000, "health"),
                    ),
                ] {
                    add_field(
                        fields,
                        path!["ultimates", index, "parameters", "Sentry", tail],
                        EditorSection::Ultimates,
                        &ultimate.key,
                        &ultimate.display_name,
                        group,
                        label,
                        spec,
                    );
                }
            }
            UltimateParameters::SelfCloak { .. } => add_field(
                fields,
                path![
                    "ultimates",
                    index,
                    "parameters",
                    "SelfCloak",
                    "duration_ticks"
                ],
                EditorSection::Ultimates,
                &ultimate.key,
                &ultimate.display_name,
                "Timing",
                "Cloak duration",
                NumberSpec::ticks(1, 3_600),
            ),
            UltimateParameters::RevealScan { .. } => {
                add_field(
                    fields,
                    path![
                        "ultimates",
                        index,
                        "parameters",
                        "RevealScan",
                        "maximum_range_milliunits"
                    ],
                    EditorSection::Ultimates,
                    &ultimate.key,
                    &ultimate.display_name,
                    "Targeting",
                    "Maximum range",
                    NumberSpec::milliunits(1, 4_096_000),
                );
                add_field(
                    fields,
                    path![
                        "ultimates",
                        index,
                        "parameters",
                        "RevealScan",
                        "radius_milliunits"
                    ],
                    EditorSection::Ultimates,
                    &ultimate.key,
                    &ultimate.display_name,
                    "Area",
                    "Reveal radius",
                    NumberSpec::milliunits(1, 2_048_000),
                );
                add_field(
                    fields,
                    path![
                        "ultimates",
                        index,
                        "parameters",
                        "RevealScan",
                        "reveal_ticks"
                    ],
                    EditorSection::Ultimates,
                    &ultimate.key,
                    &ultimate.display_name,
                    "Timing",
                    "Reveal duration",
                    NumberSpec::ticks(1, 3_600),
                );
            }
            UltimateParameters::ConcealmentField { .. } => {
                add_field(
                    fields,
                    path![
                        "ultimates",
                        index,
                        "parameters",
                        "ConcealmentField",
                        "maximum_range_milliunits"
                    ],
                    EditorSection::Ultimates,
                    &ultimate.key,
                    &ultimate.display_name,
                    "Targeting",
                    "Maximum range",
                    NumberSpec::milliunits(1, 4_096_000),
                );
                add_field(
                    fields,
                    path![
                        "ultimates",
                        index,
                        "parameters",
                        "ConcealmentField",
                        "radius_milliunits"
                    ],
                    EditorSection::Ultimates,
                    &ultimate.key,
                    &ultimate.display_name,
                    "Area",
                    "Field radius",
                    NumberSpec::milliunits(1, 2_048_000),
                );
                add_field(
                    fields,
                    path![
                        "ultimates",
                        index,
                        "parameters",
                        "ConcealmentField",
                        "duration_ticks"
                    ],
                    EditorSection::Ultimates,
                    &ultimate.key,
                    &ultimate.display_name,
                    "Timing",
                    "Field duration",
                    NumberSpec::ticks(1, 3_600),
                );
            }
            UltimateParameters::DemolitionStrike { .. } => {
                add_field(
                    fields,
                    path![
                        "ultimates",
                        index,
                        "parameters",
                        "DemolitionStrike",
                        "maximum_range_milliunits"
                    ],
                    EditorSection::Ultimates,
                    &ultimate.key,
                    &ultimate.display_name,
                    "Targeting",
                    "Maximum range",
                    NumberSpec::milliunits(1, 4_096_000),
                );
                add_field(
                    fields,
                    path![
                        "ultimates",
                        index,
                        "parameters",
                        "DemolitionStrike",
                        "radius_milliunits"
                    ],
                    EditorSection::Ultimates,
                    &ultimate.key,
                    &ultimate.display_name,
                    "Area",
                    "Destruction radius",
                    NumberSpec::milliunits(8_000, 64_000),
                );
            }
            UltimateParameters::ElementalField { .. } => {
                for (tail, group, label, spec) in [
                    (
                        "maximum_range_milliunits",
                        "Targeting",
                        "Maximum range",
                        NumberSpec::milliunits(1, 4_096_000),
                    ),
                    (
                        "radius_milliunits",
                        "Area",
                        "Field radius",
                        NumberSpec::milliunits(1, 2_048_000),
                    ),
                    (
                        "duration_ticks",
                        "Timing",
                        "Field duration",
                        NumberSpec::ticks(1, 3_600),
                    ),
                    (
                        "pulse_interval_ticks",
                        "Timing",
                        "Pulse interval",
                        NumberSpec::ticks(1, 3_600),
                    ),
                ] {
                    add_field(
                        fields,
                        path!["ultimates", index, "parameters", "ElementalField", tail],
                        EditorSection::Ultimates,
                        &ultimate.key,
                        &ultimate.display_name,
                        group,
                        label,
                        spec,
                    );
                }
                let (effect_kind, effect_label) = match ultimate.kind {
                    crate::builds::UltimateKind::CryogenicField => ("Cold", "Cold per pulse"),
                    crate::builds::UltimateKind::FireField
                    | crate::builds::UltimateKind::PoisonField => {
                        ("DamageOverTime", "Damage per tick")
                    }
                    crate::builds::UltimateKind::RestorationField => ("Heal", "Healing"),
                    _ => continue,
                };
                let effect_tail = if effect_kind == "DamageOverTime" {
                    "damage_per_tick"
                } else {
                    "amount"
                };
                add_field(
                    fields,
                    path![
                        "ultimates",
                        index,
                        "parameters",
                        "ElementalField",
                        "effect",
                        effect_kind,
                        effect_tail
                    ],
                    EditorSection::Ultimates,
                    &ultimate.key,
                    &ultimate.display_name,
                    "Effect",
                    effect_label,
                    NumberSpec::integer(
                        1,
                        u32::from(u16::MAX),
                        if effect_kind == "Cold" {
                            "cold/pulse"
                        } else {
                            "points"
                        },
                    )
                    .help(if effect_kind == "Cold" {
                        "Applied after resistance against each target's Cold capacity."
                    } else {
                        "Applied on each authoritative field pulse."
                    }),
                );
                if effect_kind == "DamageOverTime" {
                    for (tail, label) in [
                        ("tick_interval", "Damage interval"),
                        ("duration_ticks", "Damage duration"),
                    ] {
                        add_field(
                            fields,
                            path![
                                "ultimates",
                                index,
                                "parameters",
                                "ElementalField",
                                "effect",
                                effect_kind,
                                tail
                            ],
                            EditorSection::Ultimates,
                            &ultimate.key,
                            &ultimate.display_name,
                            "Effect",
                            label,
                            NumberSpec::ticks(1, 3_600),
                        );
                    }
                }
            }
            UltimateParameters::BigBlob { .. } => {
                for (tail, group, label, spec) in [
                    (
                        "maximum_range_milliunits",
                        "Targeting",
                        "Maximum throw range",
                        NumberSpec::milliunits(1, 4_096_000),
                    ),
                    (
                        "flight_ticks",
                        "Targeting",
                        "Lob flight time",
                        NumberSpec::ticks(1, 600),
                    ),
                    (
                        "visual_arc_height_milliunits",
                        "Targeting",
                        "Visual arc height",
                        NumberSpec::milliunits(1, 2_048_000),
                    ),
                    (
                        "landing_clearance_milliunits",
                        "Targeting",
                        "Landing clearance",
                        NumberSpec::milliunits(1, 512_000),
                    ),
                    (
                        "child_speed_milliunits",
                        "Secondary blobs",
                        "Travel speed",
                        NumberSpec::milliunits(1, 4_096_000),
                    ),
                    (
                        "child_radius_milliunits",
                        "Secondary blobs",
                        "Projectile radius",
                        NumberSpec::milliunits(1, 512_000),
                    ),
                    (
                        "child_range_milliunits",
                        "Secondary blobs",
                        "Travel range",
                        NumberSpec::milliunits(1, 4_096_000),
                    ),
                    (
                        "child_lifetime_ticks",
                        "Secondary blobs",
                        "Flight lifetime",
                        NumberSpec::ticks(1, 600),
                    ),
                    (
                        "child_fuse_ticks",
                        "Explosion",
                        "Fuse delay",
                        NumberSpec::ticks(1, 3_600),
                    ),
                    (
                        "child_explosion_radius_milliunits",
                        "Explosion",
                        "Explosion radius",
                        NumberSpec::milliunits(1, 512_000),
                    ),
                    (
                        "child_damage",
                        "Explosion",
                        "Damage",
                        NumberSpec::integer(1, 1_000, "health"),
                    ),
                    (
                        "max_active_per_owner",
                        "Capacity",
                        "Maximum active blobs",
                        NumberSpec::integer(1, 16, "blobs"),
                    ),
                ] {
                    add_field(
                        fields,
                        path!["ultimates", index, "parameters", "BigBlob", tail],
                        EditorSection::Ultimates,
                        &ultimate.key,
                        &ultimate.display_name,
                        group,
                        label,
                        spec,
                    );
                }
            }
        }
    }
}

fn add_world_fields(fields: &mut Vec<EditorFieldDescriptor>) {
    add_field(
        fields,
        path!["barrel", "damageProfile", "maximum_health"],
        EditorSection::WorldObjects,
        "oil-barrel",
        "Oil barrel",
        "Durability",
        "Maximum health",
        NumberSpec::integer(1, 1_000, "health"),
    );
    for (path, label, spec) in [
        (
            path!["barrel", "explosionProfile", "damage"],
            "Explosion damage",
            NumberSpec::integer(1, u32::from(u16::MAX), "health"),
        ),
        (
            path!["barrel", "explosionProfile", "radius_world_units"],
            "Explosion radius",
            NumberSpec::integer(1, 512, "world units"),
        ),
        (
            path!["barrel", "explosionProfile", "maximum_targets"],
            "Maximum targets",
            NumberSpec::integer(1, 16, "targets"),
        ),
        (
            path!["barrel", "explosionProfile", "maximum_chain_reactions"],
            "Maximum chain reactions",
            NumberSpec::integer(1, 16, "reactions"),
        ),
    ] {
        add_field(
            fields,
            path,
            EditorSection::WorldObjects,
            "oil-barrel",
            "Oil barrel",
            "Explosion",
            label,
            spec,
        );
    }
    add_field(
        fields,
        path!["chest", "damageProfile", "maximum_health"],
        EditorSection::WorldObjects,
        "treasure-chest",
        "Treasure chest & pickup",
        "Chest",
        "Maximum health",
        NumberSpec::integer(1, 1_000, "health"),
    );
    add_field(
        fields,
        path!["chest", "pickupDefinition", "restoration"],
        EditorSection::WorldObjects,
        "treasure-chest",
        "Treasure chest & pickup",
        "Pickup",
        "Restoration",
        NumberSpec::integer(1, 1_000, "health"),
    );
    add_field(
        fields,
        path!["chest", "pickupDefinition", "collection_radius_world_units"],
        EditorSection::WorldObjects,
        "treasure-chest",
        "Treasure chest & pickup",
        "Pickup",
        "Collection radius",
        NumberSpec::integer(8, 64, "world units"),
    );
    add_field(
        fields,
        path!["chest", "pickupDefinition", "lifetime_ticks"],
        EditorSection::WorldObjects,
        "treasure-chest",
        "Treasure chest & pickup",
        "Pickup",
        "Lifetime",
        NumberSpec::ticks(60, 3_600),
    );
    add_field(
        fields,
        path!["heist", "safeMaximumHealth"],
        EditorSection::Modes,
        "heist",
        "Heist",
        "Safe",
        "Maximum health",
        NumberSpec::integer(100, 20_000, "health"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (BalanceLabSnapshotV3, WeaponCatalog) {
        let builds = crate::builds::BuildCatalog::embedded().unwrap();
        let weapons = WeaponCatalog::embedded().unwrap();
        let maps = crate::map::MapContentCatalog::embedded().unwrap();
        (
            BalanceLabSnapshotV3::from_catalogs(&builds, &weapons, &maps),
            weapons,
        )
    }

    fn path_key(path: &[EditorPathSegment]) -> String {
        path.iter()
            .map(|segment| match segment {
                EditorPathSegment::Key(value) => value.clone(),
                EditorPathSegment::Index(value) => value.to_string(),
            })
            .collect::<Vec<_>>()
            .join("/")
    }

    fn assert_f64_eq(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < f64::EPSILON,
            "expected {expected}, got {actual}"
        );
    }

    fn manifest_field_by_path<'a>(
        manifest: &'a BalanceLabEditorManifest,
        path: &str,
    ) -> &'a EditorFieldDescriptor {
        manifest
            .fields
            .iter()
            .find(|field| path_key(&field.path) == path)
            .unwrap_or_else(|| panic!("missing editor descriptor {path}"))
    }

    fn collect_numeric_leaf_paths(
        value: &serde_json::Value,
        prefix: &mut Vec<String>,
        paths: &mut std::collections::BTreeSet<String>,
    ) {
        match value {
            serde_json::Value::Number(_) => {
                assert!(paths.insert(prefix.join("/")), "duplicate numeric leaf");
            }
            serde_json::Value::Array(values) => {
                for (index, value) in values.iter().enumerate() {
                    prefix.push(index.to_string());
                    collect_numeric_leaf_paths(value, prefix, paths);
                    prefix.pop();
                }
            }
            serde_json::Value::Object(values) => {
                for (key, value) in values {
                    prefix.push(key.clone());
                    collect_numeric_leaf_paths(value, prefix, paths);
                    prefix.pop();
                }
            }
            serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::String(_) => {
            }
        }
    }

    fn assert_weapon_numeric_coverage(
        weapon: &super::super::WeaponPresetTuning,
        catalog: &WeaponCatalog,
    ) -> usize {
        let mut numeric_leaves = std::collections::BTreeSet::new();
        collect_numeric_leaf_paths(
            &serde_json::to_value(&weapon.recipe).unwrap(),
            &mut Vec::new(),
            &mut numeric_leaves,
        );
        let mut fields = Vec::new();
        weapons::add_fields(&mut fields, 0, weapon, catalog);
        let descriptor_paths: Vec<_> = fields
            .iter()
            .map(|field| match field.path.as_slice() {
                [
                    EditorPathSegment::Key(root),
                    EditorPathSegment::Index(0),
                    EditorPathSegment::Key(recipe),
                    tail @ ..,
                ] if root == "weapons" && recipe == "recipe" => path_key(tail),
                _ => panic!(
                    "{} emitted a descriptor outside weapons/0/recipe: {}",
                    weapon.key,
                    path_key(&field.path)
                ),
            })
            .collect();
        assert_eq!(
            fields.len(),
            descriptor_paths.len(),
            "{} did not account for every emitted weapon descriptor",
            weapon.key
        );
        let unique_descriptor_paths: std::collections::BTreeSet<_> =
            descriptor_paths.iter().cloned().collect();
        assert_eq!(
            descriptor_paths.len(),
            unique_descriptor_paths.len(),
            "{} has duplicate weapon descriptors",
            weapon.key
        );
        assert_eq!(
            fields.len(),
            numeric_leaves.len(),
            "{} weapon descriptor and numeric-leaf totals differ",
            weapon.key
        );
        assert_eq!(
            unique_descriptor_paths, numeric_leaves,
            "{} weapon metadata does not cover its numeric schema exactly",
            weapon.key
        );
        numeric_leaves.len()
    }

    #[test]
    fn manifest_exposes_only_the_supported_numeric_leaves() {
        let (snapshot, weapons) = fixture();
        let manifest = BalanceLabEditorManifest::from_catalogs(&snapshot, &weapons);
        assert_eq!(manifest.schema_version, EDITOR_SCHEMA_VERSION);
        assert_eq!(manifest.fields.len(), 215);
        let paths: std::collections::HashSet<_> = manifest
            .fields
            .iter()
            .map(|field| path_key(&field.path))
            .collect();
        assert_eq!(paths.len(), manifest.fields.len());
        assert!(!paths.iter().any(|path| path.contains("visual_profile_id")));
        assert!(
            !paths
                .iter()
                .any(|path| path.contains("explosion_profile_id"))
        );
        assert!(
            !paths
                .iter()
                .any(|path| path.contains("pickup_definition_id"))
        );
        for expected in [
            "ultimateCharge/maximum",
            "ultimates/0/parameters/Dash/damage",
            "ultimates/1/parameters/Sentry/projectile_damage",
            "passives/2/parameters/AdrenalResponse/duration_ticks",
            "passives/4/parameters/QuickCycle/refill_duration_basis_points",
        ] {
            assert!(
                paths.contains(expected),
                "missing authored tuning path {expected}"
            );
        }
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "explicit expected descriptors freeze the complete passive manifest contract"
    )]
    fn passive_manifest_projection_preserves_exact_descriptor_contract() {
        let (snapshot, weapons) = fixture();
        let manifest = BalanceLabEditorManifest::from_catalogs(&snapshot, &weapons);
        let actual: Vec<_> = manifest
            .fields
            .into_iter()
            .filter(|field| {
                matches!(
                    field.path.first(),
                    Some(EditorPathSegment::Key(root)) if root == "passives"
                )
            })
            .collect();
        let literal_descriptor = |path,
                                  subject_key,
                                  subject_label,
                                  group,
                                  label,
                                  unit,
                                  storage_scale,
                                  minimum,
                                  maximum,
                                  step,
                                  control,
                                  help| {
            serde_json::json!({
                "path": path,
                "section": "ultimates",
                "subjectKey": subject_key,
                "subjectLabel": subject_label,
                "group": group,
                "label": label,
                "storageKind": "integer",
                "unit": unit,
                "storageScale": storage_scale,
                "minimum": minimum,
                "maximum": maximum,
                "minimumExclusive": false,
                "step": step,
                "control": control,
                "help": help,
            })
        };
        let expected = serde_json::Value::Array(vec![
            literal_descriptor(
                serde_json::json!([
                    "passives",
                    2,
                    "parameters",
                    "AdrenalResponse",
                    "duration_ticks"
                ]),
                "adrenal-response",
                "Adrenal Response",
                "Timing",
                "Boost duration",
                "s",
                60.0,
                0.016_666_666_666_666_666,
                60.0,
                0.016_666_666_666_666_666,
                "number",
                "Enter seconds; saved to the nearest authoritative server tick.",
            ),
            literal_descriptor(
                serde_json::json!([
                    "passives",
                    2,
                    "parameters",
                    "AdrenalResponse",
                    "rearm_ticks"
                ]),
                "adrenal-response",
                "Adrenal Response",
                "Timing",
                "Rearm time",
                "s",
                60.0,
                0.016_666_666_666_666_666,
                600.0,
                0.016_666_666_666_666_666,
                "number",
                "Enter seconds; saved to the nearest authoritative server tick.",
            ),
            literal_descriptor(
                serde_json::json!([
                    "passives",
                    2,
                    "parameters",
                    "AdrenalResponse",
                    "movement_bonus_basis_points"
                ]),
                "adrenal-response",
                "Adrenal Response",
                "Effect",
                "Movement bonus",
                "%",
                100.0,
                0.01,
                100.0,
                0.01,
                "range-and-number",
                "Displayed as a percentage and stored in basis points.",
            ),
            literal_descriptor(
                serde_json::json!([
                    "passives",
                    3,
                    "parameters",
                    "CloseQuarters",
                    "near_distance_milliunits"
                ]),
                "close-quarters",
                "Close Quarters",
                "Distance",
                "Near distance",
                "world units",
                1_000.0,
                0.001,
                4_096.0,
                0.001,
                "number",
                "Displayed in world units and stored to the nearest thousandth.",
            ),
            literal_descriptor(
                serde_json::json!([
                    "passives",
                    3,
                    "parameters",
                    "CloseQuarters",
                    "far_distance_milliunits"
                ]),
                "close-quarters",
                "Close Quarters",
                "Distance",
                "Far distance",
                "world units",
                1_000.0,
                0.001,
                4_096.0,
                0.001,
                "number",
                "Displayed in world units and stored to the nearest thousandth.",
            ),
            literal_descriptor(
                serde_json::json!([
                    "passives",
                    3,
                    "parameters",
                    "CloseQuarters",
                    "near_damage_basis_points"
                ]),
                "close-quarters",
                "Close Quarters",
                "Effect",
                "Near damage",
                "%",
                100.0,
                0.01,
                300.0,
                0.01,
                "range-and-number",
                "Displayed as a percentage and stored in basis points.",
            ),
            literal_descriptor(
                serde_json::json!([
                    "passives",
                    3,
                    "parameters",
                    "CloseQuarters",
                    "far_damage_basis_points"
                ]),
                "close-quarters",
                "Close Quarters",
                "Effect",
                "Far damage",
                "%",
                100.0,
                0.01,
                300.0,
                0.01,
                "range-and-number",
                "Displayed as a percentage and stored in basis points.",
            ),
            literal_descriptor(
                serde_json::json!([
                    "passives",
                    4,
                    "parameters",
                    "QuickCycle",
                    "refill_duration_basis_points"
                ]),
                "quick-cycle",
                "Quick Cycle",
                "Effect",
                "Refill duration",
                "%",
                100.0,
                0.01,
                100.0,
                0.01,
                "range-and-number",
                "Displayed as a percentage and stored in basis points.",
            ),
            literal_descriptor(
                serde_json::json!([
                    "passives",
                    5,
                    "parameters",
                    "Tenacity",
                    "slow_duration_basis_points"
                ]),
                "tenacity",
                "Tenacity",
                "Effect",
                "Slow duration",
                "%",
                100.0,
                0.01,
                100.0,
                0.01,
                "range-and-number",
                "Displayed as a percentage and stored in basis points.",
            ),
            literal_descriptor(
                serde_json::json!([
                    "passives",
                    6,
                    "parameters",
                    "CryogenicInsulation",
                    "resistance_basis_points"
                ]),
                "cryogenic-insulation",
                "Cryogenic Insulation",
                "Effect",
                "Resistance",
                "%",
                100.0,
                0.01,
                60.0,
                0.01,
                "range-and-number",
                "Displayed as a percentage and stored in basis points.",
            ),
            literal_descriptor(
                serde_json::json!([
                    "passives",
                    7,
                    "parameters",
                    "FilteredCirculation",
                    "resistance_basis_points"
                ]),
                "filtered-circulation",
                "Filtered Circulation",
                "Effect",
                "Resistance",
                "%",
                100.0,
                0.01,
                60.0,
                0.01,
                "range-and-number",
                "Displayed as a percentage and stored in basis points.",
            ),
            literal_descriptor(
                serde_json::json!([
                    "passives",
                    8,
                    "parameters",
                    "HeatShielding",
                    "resistance_basis_points"
                ]),
                "heat-shielding",
                "Heat Shielding",
                "Effect",
                "Resistance",
                "%",
                100.0,
                0.01,
                60.0,
                0.01,
                "range-and-number",
                "Displayed as a percentage and stored in basis points.",
            ),
        ]);
        assert_eq!(serde_json::to_value(actual).unwrap(), expected);
    }

    #[test]
    fn every_passive_numeric_parameter_leaf_has_exactly_one_descriptor() {
        let (snapshot, weapons) = fixture();
        let manifest = BalanceLabEditorManifest::from_catalogs(&snapshot, &weapons);
        let mut total_numeric_leaves = 0;
        for (index, passive) in snapshot.passives.iter().enumerate() {
            let mut numeric_leaves = std::collections::BTreeSet::new();
            collect_numeric_leaf_paths(
                &serde_json::to_value(passive.parameters).unwrap(),
                &mut Vec::new(),
                &mut numeric_leaves,
            );
            let descriptor_paths: Vec<_> = manifest
                .fields
                .iter()
                .filter_map(|field| match field.path.as_slice() {
                    [
                        EditorPathSegment::Key(root),
                        EditorPathSegment::Index(field_index),
                        EditorPathSegment::Key(parameters),
                        tail @ ..,
                    ] if root == "passives"
                        && *field_index == index
                        && parameters == "parameters" =>
                    {
                        Some(path_key(tail))
                    }
                    _ => None,
                })
                .collect();
            let unique_descriptor_paths: std::collections::BTreeSet<_> =
                descriptor_paths.iter().cloned().collect();
            assert_eq!(
                descriptor_paths.len(),
                unique_descriptor_paths.len(),
                "{} has duplicate passive descriptors",
                passive.key
            );
            assert_eq!(
                unique_descriptor_paths, numeric_leaves,
                "{} passive metadata does not cover its numeric schema exactly",
                passive.key
            );
            total_numeric_leaves += numeric_leaves.len();
        }
        assert_eq!(total_numeric_leaves, 12);
    }

    #[test]
    fn every_embedded_weapon_numeric_leaf_has_exactly_one_descriptor() {
        let (snapshot, weapons) = fixture();
        let total_numeric_leaves = snapshot
            .weapons
            .iter()
            .map(|weapon| assert_weapon_numeric_coverage(weapon, &weapons))
            .sum::<usize>();
        assert_eq!(total_numeric_leaves, 87);
    }

    #[test]
    fn absent_weapon_topologies_have_exact_numeric_descriptor_coverage() {
        use crate::combat::{
            DamageOverTimeKind, DeliveryMethod, PayloadEffectDefinition, PersistentAreaShape,
            RecipientPolicy, WeaponConfiguration, WorldEffectDefinition,
        };

        let (snapshot, weapons) = fixture();
        let limits = crate::combat::EngineWeaponLimits::default();

        let mut rectangle_effects = snapshot.weapons[6].clone();
        let DeliveryMethod::Splash { shape, .. } = &mut rectangle_effects.recipe.delivery else {
            panic!("Splash fixture changed topology");
        };
        *shape = PersistentAreaShape::Rectangle {
            half_extents: [96.0, 72.0],
        };
        rectangle_effects.recipe.payload_bundles[0].effects = vec![
            PayloadEffectDefinition::Cold {
                amount: 250,
                recipients: RecipientPolicy::HostilesAndOwner { owner_scale: 0.5 },
            },
            PayloadEffectDefinition::DamageOverTime {
                kind: DamageOverTimeKind::Fire,
                damage_per_tick: 12,
                tick_interval: 30,
                duration_ticks: 90,
                recipients: RecipientPolicy::Hostiles,
            },
        ];
        WeaponConfiguration {
            recipe: rectangle_effects.recipe.clone(),
        }
        .validate(&weapons.recipe_policy, limits, None)
        .unwrap();
        assert_weapon_numeric_coverage(&rectangle_effects, &weapons);
        let mut rectangle_fields = Vec::new();
        weapons::add_fields(&mut rectangle_fields, 0, &rectangle_effects, &weapons);
        let cold = rectangle_fields
            .iter()
            .find(|field| {
                path_key(&field.path) == "weapons/0/recipe/payload_bundles/0/effects/0/Cold/amount"
            })
            .unwrap();
        assert_f64_eq(
            cold.maximum,
            f64::from(crate::combat::definitions::MAX_COLD_PAYLOAD_AMOUNT),
        );
        let owner_scale = rectangle_fields
            .iter()
            .find(|field| {
                path_key(&field.path)
                    == "weapons/0/recipe/payload_bundles/0/effects/0/Cold/recipients/HostilesAndOwner/owner_scale"
            })
            .unwrap();
        assert_f64_eq(owner_scale.minimum, 0.0);
        assert_f64_eq(owner_scale.maximum, 1.0);

        let mut destruction = snapshot.weapons[2].clone();
        destruction.recipe.world_effects = vec![WorldEffectDefinition::DestroyMap { radius: 64.0 }];
        WeaponConfiguration {
            recipe: destruction.recipe.clone(),
        }
        .validate(&weapons.recipe_policy, limits, None)
        .unwrap();
        assert_weapon_numeric_coverage(&destruction, &weapons);
        let mut destruction_fields = Vec::new();
        weapons::add_fields(&mut destruction_fields, 0, &destruction, &weapons);
        let radius = destruction_fields
            .iter()
            .find(|field| {
                path_key(&field.path) == "weapons/0/recipe/world_effects/0/DestroyMap/radius"
            })
            .unwrap();
        assert_f64_eq(
            radius.maximum,
            f64::from(crate::combat::EngineWeaponLimits::default().max_map_destruction_radius),
        );
    }

    #[test]
    fn lobbed_flight_descriptor_preserves_the_exact_serialized_contract() {
        let (snapshot, weapons) = fixture();
        let manifest = BalanceLabEditorManifest::from_catalogs(&snapshot, &weapons);

        assert_eq!(
            serde_json::to_value(manifest_field_by_path(
                &manifest,
                "weapons/2/recipe/delivery/Lobbed/max_flight_ticks"
            ))
            .unwrap(),
            serde_json::json!({
                "path": ["weapons", 2, "recipe", "delivery", "Lobbed", "max_flight_ticks"],
                "section": "weapons",
                "subjectKey": "arc-launcher",
                "subjectLabel": "Arc Launcher",
                "group": "Delivery",
                "label": "Maximum flight time",
                "storageKind": "integer",
                "unit": "s",
                "storageScale": 60.0,
                "minimum": 0.1,
                "maximum": 10.0,
                "minimumExclusive": false,
                "step": 0.016_666_666_666_666_666,
                "control": "number",
                "help": "Enter seconds; saved to the nearest authoritative server tick."
            })
        );
    }

    #[test]
    fn splash_flight_descriptor_preserves_the_exact_serialized_contract() {
        let (snapshot, weapons) = fixture();
        let manifest = BalanceLabEditorManifest::from_catalogs(&snapshot, &weapons);

        assert_eq!(
            serde_json::to_value(manifest_field_by_path(
                &manifest,
                "weapons/6/recipe/delivery/Splash/max_flight_ticks"
            ))
            .unwrap(),
            serde_json::json!({
                "path": ["weapons", 6, "recipe", "delivery", "Splash", "max_flight_ticks"],
                "section": "weapons",
                "subjectKey": "splash",
                "subjectLabel": "Splash",
                "group": "Splash",
                "label": "Maximum flight time",
                "storageKind": "integer",
                "unit": "s",
                "storageScale": 60.0,
                "minimum": 0.1,
                "maximum": 10.0,
                "minimumExclusive": false,
                "step": 0.016_666_666_666_666_666,
                "control": "number",
                "help": "Enter seconds; saved to the nearest authoritative server tick."
            })
        );
    }

    #[test]
    fn splash_heal_descriptor_preserves_the_exact_serialized_contract() {
        let (snapshot, weapons) = fixture();
        let manifest = BalanceLabEditorManifest::from_catalogs(&snapshot, &weapons);

        assert_eq!(
            serde_json::to_value(manifest_field_by_path(
                &manifest,
                "weapons/6/recipe/payload_bundles/0/effects/1/Heal/amount"
            ))
            .unwrap(),
            serde_json::json!({
                "path": [
                    "weapons", 6, "recipe", "payload_bundles", 0, "effects", 1, "Heal",
                    "amount"
                ],
                "section": "weapons",
                "subjectKey": "splash",
                "subjectLabel": "Splash",
                "group": "Payload 1",
                "label": "Healing",
                "storageKind": "integer",
                "unit": "health",
                "storageScale": 1.0,
                "minimum": 1.0,
                "maximum": 1_000.0,
                "minimumExclusive": false,
                "step": 1.0,
                "control": "number"
            })
        );
    }

    #[test]
    fn weapon_manifest_uses_authoritative_active_per_owner_bounds() {
        let (snapshot, weapons) = fixture();
        let manifest = BalanceLabEditorManifest::from_catalogs(&snapshot, &weapons);

        assert_f64_eq(
            manifest_field_by_path(
                &manifest,
                "weapons/4/recipe/delivery/StickyStraight/max_active_per_owner",
            )
            .maximum,
            f64::from(crate::combat::definitions::MAX_STICKY_ACTIVE_PER_OWNER),
        );
        assert_f64_eq(
            manifest_field_by_path(
                &manifest,
                "weapons/6/recipe/delivery/Splash/max_active_per_owner",
            )
            .maximum,
            f64::from(crate::combat::definitions::MAX_SPLASH_ACTIVE_PER_OWNER),
        );
    }

    #[test]
    fn weapon_editor_and_validator_share_lob_and_heal_boundaries() {
        use crate::combat::{DeliveryMethod, PayloadEffectDefinition, WeaponConfiguration};

        let (snapshot, weapons) = fixture();
        let limits = crate::combat::EngineWeaponLimits::default();

        let mut lobbed = snapshot.weapons[2].recipe.clone();
        if let DeliveryMethod::Lobbed {
            max_flight_ticks, ..
        } = &mut lobbed.delivery
        {
            *max_flight_ticks = 5;
        } else {
            panic!("Arc Launcher fixture changed topology");
        }
        assert!(
            WeaponConfiguration {
                recipe: lobbed.clone()
            }
            .validate(&weapons.recipe_policy, limits, None)
            .is_err()
        );
        if let DeliveryMethod::Lobbed {
            max_flight_ticks, ..
        } = &mut lobbed.delivery
        {
            *max_flight_ticks = 6;
        }
        WeaponConfiguration { recipe: lobbed }
            .validate(&weapons.recipe_policy, limits, None)
            .unwrap();

        let mut splash = snapshot.weapons[6].recipe.clone();
        if let DeliveryMethod::Splash {
            max_flight_ticks, ..
        } = &mut splash.delivery
        {
            *max_flight_ticks = 5;
        } else {
            panic!("Splash fixture changed topology");
        }
        assert!(
            WeaponConfiguration {
                recipe: splash.clone()
            }
            .validate(&weapons.recipe_policy, limits, None)
            .is_err()
        );
        if let DeliveryMethod::Splash {
            max_flight_ticks, ..
        } = &mut splash.delivery
        {
            *max_flight_ticks = 6;
        }
        WeaponConfiguration {
            recipe: splash.clone(),
        }
        .validate(&weapons.recipe_policy, limits, None)
        .unwrap();

        if let PayloadEffectDefinition::Heal { amount, .. } =
            &mut splash.payload_bundles[0].effects[1]
        {
            *amount = 1_000;
        } else {
            panic!("Splash fixture lost its healing payload");
        }
        WeaponConfiguration {
            recipe: splash.clone(),
        }
        .validate(&weapons.recipe_policy, limits, None)
        .unwrap();
        if let PayloadEffectDefinition::Heal { amount, .. } =
            &mut splash.payload_bundles[0].effects[1]
        {
            *amount = 1_001;
        }
        assert!(
            WeaponConfiguration { recipe: splash }
                .validate(&weapons.recipe_policy, limits, None)
                .is_err()
        );
    }

    #[test]
    fn lowered_weapon_damage_policy_drives_heal_editor_and_validator_maximum() {
        use crate::combat::{PayloadEffectDefinition, WeaponConfiguration};

        let (snapshot, weapons) = fixture();
        let limits = crate::combat::EngineWeaponLimits::default();
        let mut lowered = weapons;
        lowered.recipe_policy.max_damage = 600;
        let lowered_manifest = BalanceLabEditorManifest::from_catalogs(&snapshot, &lowered);
        let healing = lowered_manifest
            .fields
            .iter()
            .find(|field| {
                path_key(&field.path) == "weapons/6/recipe/payload_bundles/0/effects/1/Heal/amount"
            })
            .unwrap();
        assert_f64_eq(healing.maximum, 600.0);

        let mut lowered_recipe = snapshot.weapons[6].recipe.clone();
        if let PayloadEffectDefinition::Heal { amount, .. } =
            &mut lowered_recipe.payload_bundles[0].effects[1]
        {
            *amount = 600;
        } else {
            panic!("Splash fixture lost its healing payload");
        }
        WeaponConfiguration {
            recipe: lowered_recipe.clone(),
        }
        .validate(&lowered.recipe_policy, limits, None)
        .unwrap();
        if let PayloadEffectDefinition::Heal { amount, .. } =
            &mut lowered_recipe.payload_bundles[0].effects[1]
        {
            *amount = 601;
        }
        assert!(
            WeaponConfiguration {
                recipe: lowered_recipe,
            }
            .validate(&lowered.recipe_policy, limits, None)
            .is_err()
        );
    }

    #[test]
    fn active_per_owner_editor_caps_match_weapon_validation() {
        use crate::combat::{DeliveryMethod, WeaponConfiguration};

        let (snapshot, weapons) = fixture();
        let limits = crate::combat::EngineWeaponLimits::default();

        let mut sticky = snapshot.weapons[4].recipe.clone();
        if let DeliveryMethod::StickyStraight {
            max_active_per_owner,
            ..
        } = &mut sticky.delivery
        {
            *max_active_per_owner = crate::combat::definitions::MAX_STICKY_ACTIVE_PER_OWNER;
        } else {
            panic!("Sticky Blomb fixture changed topology");
        }
        WeaponConfiguration {
            recipe: sticky.clone(),
        }
        .validate(&weapons.recipe_policy, limits, None)
        .unwrap();
        if let DeliveryMethod::StickyStraight {
            max_active_per_owner,
            ..
        } = &mut sticky.delivery
        {
            *max_active_per_owner = crate::combat::definitions::MAX_STICKY_ACTIVE_PER_OWNER + 1;
        }
        assert!(
            WeaponConfiguration { recipe: sticky }
                .validate(&weapons.recipe_policy, limits, None)
                .is_err()
        );

        let mut splash = snapshot.weapons[6].recipe.clone();
        if let DeliveryMethod::Splash {
            max_active_per_owner,
            ..
        } = &mut splash.delivery
        {
            *max_active_per_owner = crate::combat::definitions::MAX_SPLASH_ACTIVE_PER_OWNER;
        } else {
            panic!("Splash fixture changed topology");
        }
        WeaponConfiguration {
            recipe: splash.clone(),
        }
        .validate(&weapons.recipe_policy, limits, None)
        .unwrap();
        if let DeliveryMethod::Splash {
            max_active_per_owner,
            ..
        } = &mut splash.delivery
        {
            *max_active_per_owner = crate::combat::definitions::MAX_SPLASH_ACTIVE_PER_OWNER + 1;
        }
        assert!(
            WeaponConfiguration { recipe: splash }
                .validate(&weapons.recipe_policy, limits, None)
                .is_err()
        );
    }

    #[test]
    fn global_cold_rules_use_seconds_and_per_second_editor_units() {
        let (snapshot, weapons) = fixture();
        let manifest = BalanceLabEditorManifest::from_catalogs(&snapshot, &weapons);
        let decay_delay = manifest
            .fields
            .iter()
            .find(|field| path_key(&field.path) == "conditionRules/cold_decay_delay_ticks")
            .unwrap();
        assert_eq!(decay_delay.section, EditorSection::Global);
        assert_eq!(decay_delay.subject_label, "Cold & Freeze");
        assert_eq!(decay_delay.unit, "s");
        assert!((decay_delay.storage_scale - SIMULATION_TICK_HZ_F64).abs() < f64::EPSILON);

        let decay_rate = manifest
            .fields
            .iter()
            .find(|field| path_key(&field.path) == "conditionRules/cold_decay_per_tick")
            .unwrap();
        assert_eq!(decay_rate.unit, "cold/s");
        assert!((decay_rate.storage_scale - (1.0 / SIMULATION_TICK_HZ_F64)).abs() < f64::EPSILON);
        assert!((decay_rate.minimum - SIMULATION_TICK_HZ_F64).abs() < f64::EPSILON);
        assert!((decay_rate.step - SIMULATION_TICK_HZ_F64).abs() < f64::EPSILON);
    }

    #[test]
    fn effect_tile_fields_expose_bounded_multipliers_damage_and_seconds() {
        let (snapshot, weapons) = fixture();
        let manifest = BalanceLabEditorManifest::from_catalogs(&snapshot, &weapons);
        let speed = manifest
            .fields
            .iter()
            .find(|field| path_key(&field.path) == "effectTiles/speedMultiplierMilli")
            .unwrap();
        assert_eq!(speed.section, EditorSection::WorldObjects);
        assert_eq!(speed.subject_label, "Effect tiles");
        assert_eq!(speed.unit, "×");
        assert!((speed.storage_scale - 1_000.0).abs() < f64::EPSILON);
        assert!((speed.minimum - 1.001).abs() < f64::EPSILON);
        assert!((speed.maximum - 2.0).abs() < f64::EPSILON);

        let interval = manifest
            .fields
            .iter()
            .find(|field| path_key(&field.path) == "effectTiles/intervalTicks")
            .unwrap();
        assert_eq!(interval.unit, "s");
        assert!((interval.storage_scale - SIMULATION_TICK_HZ_F64).abs() < f64::EPSILON);
        assert!((interval.minimum - 0.1).abs() < f64::EPSILON);
        assert!((interval.maximum - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fighter_bounds_follow_runtime_representation_not_balance_policy() {
        let (snapshot, weapons) = fixture();
        let manifest = BalanceLabEditorManifest::from_catalogs(&snapshot, &weapons);
        let health = manifest
            .fields
            .iter()
            .find(|field| path_key(&field.path) == "fighterProfiles/default/maximum_health")
            .unwrap();
        assert!((health.minimum - 1.0).abs() < f64::EPSILON);
        assert!((health.maximum - f64::from(u16::MAX)).abs() < f64::EPSILON);
        assert_eq!(health.control, EditorControl::Number);

        let speed = manifest
            .fields
            .iter()
            .find(|field| path_key(&field.path) == "fighterProfiles/default/movement_speed")
            .unwrap();
        assert!((speed.minimum - 1.0).abs() < f64::EPSILON);
        assert!((speed.maximum - f64::from(MAX_FIGHTER_MOVEMENT_SPEED)).abs() < f64::EPSILON);
        assert!((speed.step - 1.0).abs() < f64::EPSILON);

        let recovery = manifest
            .fields
            .iter()
            .find(|field| path_key(&field.path) == "fighterProfiles/default/health_recovery_rate")
            .unwrap();
        assert_eq!(recovery.unit, "health/s");

        let delay = manifest
            .fields
            .iter()
            .find(|field| {
                path_key(&field.path) == "fighterProfiles/default/idle_attack_delay_ticks"
            })
            .unwrap();
        assert_eq!(delay.unit, "s");
        assert!((delay.storage_scale - SIMULATION_TICK_HZ_F64).abs() < f64::EPSILON);

        let cold_capacity = manifest
            .fields
            .iter()
            .find(|field| path_key(&field.path) == "fighterProfiles/default/cold_capacity")
            .unwrap();
        assert_eq!(cold_capacity.unit, "cold");
        assert!((cold_capacity.maximum - f64::from(MAX_COLD_CAPACITY)).abs() < f64::EPSILON);

        let cold_resistance = manifest
            .fields
            .iter()
            .find(|field| {
                path_key(&field.path) == "fighterProfiles/default/cold_resistance_basis_points"
            })
            .unwrap();
        assert_eq!(cold_resistance.unit, "%");
        assert!((cold_resistance.storage_scale - 100.0).abs() < f64::EPSILON);
        assert!((cold_resistance.maximum - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ultimate_milliunits_are_described_in_world_units() {
        let (snapshot, weapons) = fixture();
        let manifest = BalanceLabEditorManifest::from_catalogs(&snapshot, &weapons);
        let range = manifest
            .fields
            .iter()
            .find(|field| {
                path_key(&field.path)
                    == "ultimates/3/parameters/RevealScan/maximum_range_milliunits"
            })
            .unwrap();
        assert_eq!(range.unit, "world units");
        assert!((range.storage_scale - 1_000.0).abs() < f64::EPSILON);
        assert!((range.minimum - 0.001).abs() < f64::EPSILON);
        assert!((range.maximum - 4_096.0).abs() < f64::EPSILON);
        assert!((range.step - 0.001).abs() < f64::EPSILON);
    }

    #[test]
    fn every_manifest_path_resolves_to_a_numeric_snapshot_leaf() {
        let (snapshot, weapons) = fixture();
        let manifest = BalanceLabEditorManifest::from_catalogs(&snapshot, &weapons);
        let value = serde_json::to_value(snapshot).unwrap();
        for field in manifest.fields {
            let mut cursor = &value;
            for segment in &field.path {
                cursor = match segment {
                    EditorPathSegment::Key(key) => cursor.get(key).unwrap_or_else(|| {
                        panic!("missing key {key} in {}", path_key(&field.path))
                    }),
                    EditorPathSegment::Index(index) => cursor.get(*index).unwrap_or_else(|| {
                        panic!("missing index {index} in {}", path_key(&field.path))
                    }),
                };
            }
            assert!(
                cursor.is_number(),
                "{} did not resolve to a number",
                path_key(&field.path)
            );
        }
    }
}
