use super::BalanceLabSnapshotV3;
use crate::{
    builds::{
        MAX_COLD_CAPACITY, MAX_FIGHTER_MOVEMENT_SPEED, MAX_REVEAL_PROXIMITY_RADIUS,
        MIN_REVEAL_PROXIMITY_RADIUS, PassiveParameters, UltimateParameters,
    },
    combat::{
        DamageFalloff, DeliveryMethod, EngineWeaponLimits, FiringPattern, PayloadEffectDefinition,
        RecipientPolicy, TargetSelection, WeaponCatalog, WeaponEconomy, WorldEffectDefinition,
    },
    timing::{SIMULATION_TICK_HZ_F64, simulation_seconds_f64},
};
use serde::Serialize;

pub(super) const EDITOR_SCHEMA_VERSION: u16 = 9;

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
            minimum: simulation_seconds_f64(u64::from(minimum)),
            maximum: simulation_seconds_f64(u64::from(maximum)),
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
            add_weapon_fields(&mut fields, index, weapon, weapons);
        }
        add_ultimate_fields(&mut fields, snapshot);
        add_passive_fields(&mut fields, snapshot);
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
    reason = "explicit descriptors keep the complete weapon editor contract auditable"
)]
fn add_weapon_fields(
    fields: &mut Vec<EditorFieldDescriptor>,
    index: usize,
    weapon: &super::WeaponPresetTuning,
    catalog: &WeaponCatalog,
) {
    let policy = &catalog.recipe_policy;
    let limits = EngineWeaponLimits::default();
    let root = |tail: Vec<EditorPathSegment>| {
        let mut path = path!["weapons", index, "recipe"];
        path.extend(tail);
        path
    };
    let mut add = |tail: Vec<EditorPathSegment>, group: &str, label: &str, spec: NumberSpec| {
        add_field(
            fields,
            root(tail),
            EditorSection::Weapons,
            &weapon.key,
            &weapon.display_name,
            group,
            label,
            spec,
        );
    };

    match weapon.recipe.economy {
        WeaponEconomy::Magazine { .. } => {
            add(
                path!["economy", "Magazine", "capacity"],
                "Economy",
                "Magazine capacity",
                NumberSpec::integer(1, u32::from(policy.max_capacity), "shots"),
            );
            add(
                path!["economy", "Magazine", "refill_ticks"],
                "Economy",
                "Ammo recovery per round",
                NumberSpec::ticks(1, u64::from(u32::MAX)),
            );
        }
        WeaponEconomy::Charges { .. } => {
            add(
                path!["economy", "Charges", "capacity"],
                "Economy",
                "Charge capacity",
                NumberSpec::integer(1, u32::from(policy.max_capacity), "charges"),
            );
            add(
                path!["economy", "Charges", "recharge_ticks"],
                "Economy",
                "Ammo recovery per charge",
                NumberSpec::ticks(1, u64::from(u32::MAX)),
            );
        }
    }
    add(
        path!["fire_cooldown_ticks"],
        "Firing",
        "Fire cooldown",
        NumberSpec::ticks(1, policy.max_fire_cooldown_ticks),
    );
    if let FiringPattern::Spread { .. } = weapon.recipe.firing {
        add(
            path!["firing", "Spread", "delivery_count"],
            "Firing",
            "Projectile count",
            NumberSpec::integer(
                2,
                u32::from(policy.max_deliveries_per_attack),
                "projectiles",
            ),
        );
        add(
            path!["firing", "Spread", "total_angle_degrees"],
            "Firing",
            "Total spread angle",
            NumberSpec::positive_decimal(f64::from(policy.max_angle_degrees), "degrees").ranged(),
        );
    }
    match weapon.recipe.delivery {
        DeliveryMethod::Straight { .. } => {
            add(
                path!["delivery", "Straight", "speed"],
                "Delivery",
                "Projectile speed",
                NumberSpec::positive_decimal(f64::from(policy.max_speed), "world units/s"),
            );
            add(
                path!["delivery", "Straight", "radius"],
                "Delivery",
                "Projectile radius",
                NumberSpec::positive_decimal(f64::from(policy.max_radius), "world units"),
            );
            add(
                path!["delivery", "Straight", "range"],
                "Delivery",
                "Maximum range",
                NumberSpec::positive_decimal(f64::from(policy.max_distance), "world units"),
            );
            add(
                path!["delivery", "Straight", "lifetime_ticks"],
                "Delivery",
                "Projectile lifetime",
                NumberSpec::ticks(1, policy.max_projectile_lifetime_ticks),
            );
            add(
                path!["delivery", "Straight", "muzzle_offset"],
                "Delivery",
                "Muzzle offset",
                NumberSpec::positive_decimal(f64::from(limits.max_world_field), "world units"),
            );
        }
        DeliveryMethod::StickyStraight { .. } => {
            add(
                path!["delivery", "StickyStraight", "speed"],
                "Delivery",
                "Projectile speed",
                NumberSpec::positive_decimal(f64::from(policy.max_speed), "world units/s"),
            );
            add(
                path!["delivery", "StickyStraight", "radius"],
                "Delivery",
                "Projectile radius",
                NumberSpec::positive_decimal(f64::from(policy.max_radius), "world units"),
            );
            add(
                path!["delivery", "StickyStraight", "range"],
                "Delivery",
                "Maximum range",
                NumberSpec::positive_decimal(f64::from(policy.max_distance), "world units"),
            );
            add(
                path!["delivery", "StickyStraight", "lifetime_ticks"],
                "Delivery",
                "Projectile lifetime",
                NumberSpec::ticks(1, policy.max_projectile_lifetime_ticks),
            );
            add(
                path!["delivery", "StickyStraight", "muzzle_offset"],
                "Delivery",
                "Muzzle offset",
                NumberSpec::positive_decimal(f64::from(limits.max_world_field), "world units"),
            );
            add(
                path!["delivery", "StickyStraight", "fuse_ticks"],
                "Sticky",
                "Explosion delay",
                NumberSpec::ticks(1, limits.max_deadline_ticks),
            );
            add(
                path!["delivery", "StickyStraight", "max_active_per_owner"],
                "Sticky",
                "Maximum active blobs",
                NumberSpec::integer(1, 16, "blobs"),
            );
        }
        DeliveryMethod::Lobbed { .. } => {
            add(
                path!["delivery", "Lobbed", "distance"],
                "Delivery",
                "Maximum distance",
                NumberSpec::positive_decimal(f64::from(policy.max_distance), "world units"),
            );
            add(
                path!["delivery", "Lobbed", "max_flight_ticks"],
                "Delivery",
                "Maximum flight time",
                NumberSpec::ticks(1, policy.max_projectile_lifetime_ticks),
            );
            add(
                path!["delivery", "Lobbed", "visual_arc_height"],
                "Delivery",
                "Visual arc height",
                NumberSpec::decimal(0.0, f64::from(policy.max_distance), 0.1, "world units"),
            );
            add(
                path!["delivery", "Lobbed", "landing_clearance_radius"],
                "Delivery",
                "Landing clearance",
                NumberSpec::positive_decimal(f64::from(policy.max_radius), "world units"),
            );
            add(
                path!["delivery", "Lobbed", "muzzle_offset"],
                "Delivery",
                "Muzzle offset",
                NumberSpec::positive_decimal(f64::from(limits.max_world_field), "world units"),
            );
        }
        DeliveryMethod::MeleeArc { .. } => {
            add(
                path!["delivery", "MeleeArc", "reach"],
                "Delivery",
                "Reach",
                NumberSpec::positive_decimal(f64::from(policy.max_distance), "world units"),
            );
            add(
                path!["delivery", "MeleeArc", "angle_degrees"],
                "Delivery",
                "Arc angle",
                NumberSpec::positive_decimal(f64::from(policy.max_angle_degrees), "degrees")
                    .ranged(),
            );
        }
        DeliveryMethod::ConeSpray { .. } => {
            add(
                path!["delivery", "ConeSpray", "propagation_speed"],
                "Spray",
                "Gas propagation speed",
                NumberSpec::positive_decimal(f64::from(policy.max_speed), "world units/s"),
            );
            add(
                path!["delivery", "ConeSpray", "reach"],
                "Spray",
                "Maximum reach",
                NumberSpec::positive_decimal(f64::from(policy.max_distance), "world units"),
            );
            add(
                path!["delivery", "ConeSpray", "angle_degrees"],
                "Spray",
                "Cone angle",
                NumberSpec::positive_decimal(f64::from(policy.max_angle_degrees), "degrees")
                    .ranged(),
            );
            add(
                path!["delivery", "ConeSpray", "linger_ticks"],
                "Spray",
                "Full-cone linger",
                NumberSpec::ticks(1, limits.max_deadline_ticks),
            );
            add(
                path!["delivery", "ConeSpray", "pulse_interval_ticks"],
                "Spray",
                "Damage pulse interval",
                NumberSpec::ticks(1, limits.max_deadline_ticks),
            );
            add(
                path!["delivery", "ConeSpray", "max_targets"],
                "Spray",
                "Maximum targets per pulse",
                NumberSpec::integer(1, u32::from(policy.max_targets_per_delivery), "targets"),
            );
        }
        DeliveryMethod::Splash { shape, .. } => {
            add(
                path!["delivery", "Splash", "distance"],
                "Splash",
                "Maximum placement distance",
                NumberSpec::positive_decimal(f64::from(policy.max_distance), "world units"),
            );
            add(
                path!["delivery", "Splash", "max_flight_ticks"],
                "Splash",
                "Maximum flight time",
                NumberSpec::ticks(1, policy.max_projectile_lifetime_ticks),
            );
            add(
                path!["delivery", "Splash", "visual_arc_height"],
                "Splash",
                "Visual arc height",
                NumberSpec::decimal(0.0, f64::from(policy.max_distance), 0.1, "world units"),
            );
            add(
                path!["delivery", "Splash", "landing_clearance_radius"],
                "Splash",
                "Landing clearance",
                NumberSpec::positive_decimal(f64::from(policy.max_radius), "world units"),
            );
            add(
                path!["delivery", "Splash", "muzzle_offset"],
                "Splash",
                "Muzzle offset",
                NumberSpec::positive_decimal(f64::from(limits.max_world_field), "world units"),
            );
            match shape {
                crate::combat::PersistentAreaShape::Circle { .. } => add(
                    path!["delivery", "Splash", "shape", "Circle", "radius"],
                    "Splash area",
                    "Circle radius",
                    NumberSpec::positive_decimal(f64::from(policy.max_radius), "world units"),
                ),
                crate::combat::PersistentAreaShape::Rectangle { .. } => {
                    add(
                        path![
                            "delivery",
                            "Splash",
                            "shape",
                            "Rectangle",
                            "half_extents",
                            0
                        ],
                        "Splash area",
                        "Rectangle half-width",
                        NumberSpec::positive_decimal(f64::from(policy.max_radius), "world units"),
                    );
                    add(
                        path![
                            "delivery",
                            "Splash",
                            "shape",
                            "Rectangle",
                            "half_extents",
                            1
                        ],
                        "Splash area",
                        "Rectangle half-depth",
                        NumberSpec::positive_decimal(f64::from(policy.max_radius), "world units"),
                    );
                }
            }
            add(
                path!["delivery", "Splash", "duration_ticks"],
                "Splash area",
                "Area duration",
                NumberSpec::ticks(1, policy.max_effect_duration_ticks),
            );
            add(
                path!["delivery", "Splash", "pulse_interval_ticks"],
                "Splash area",
                "Pulse interval",
                NumberSpec::ticks(1, policy.max_effect_duration_ticks),
            );
            add(
                path!["delivery", "Splash", "max_targets"],
                "Splash area",
                "Maximum targets per pulse",
                NumberSpec::integer(1, u32::from(policy.max_targets_per_delivery), "targets"),
            );
            add(
                path!["delivery", "Splash", "max_active_per_owner"],
                "Splash area",
                "Maximum active areas",
                NumberSpec::integer(1, 8, "areas"),
            );
        }
    }

    for (bundle_index, bundle) in weapon.recipe.payload_bundles.iter().enumerate() {
        let group = format!("Payload {}", bundle_index + 1);
        if let TargetSelection::Area { .. } = bundle.target {
            add(
                path!["payload_bundles", bundle_index, "target", "Area", "radius"],
                &group,
                "Area radius",
                NumberSpec::positive_decimal(f64::from(policy.max_radius), "world units"),
            );
            add(
                path![
                    "payload_bundles",
                    bundle_index,
                    "target",
                    "Area",
                    "max_targets"
                ],
                &group,
                "Maximum targets",
                NumberSpec::integer(1, u32::from(policy.max_targets_per_delivery), "targets"),
            );
        }
        for (effect_index, effect) in bundle.effects.iter().enumerate() {
            let effect_root = |kind: &str, tail: Vec<EditorPathSegment>| {
                let mut path = path![
                    "payload_bundles",
                    bundle_index,
                    "effects",
                    effect_index,
                    kind
                ];
                path.extend(tail);
                path
            };
            match effect {
                PayloadEffectDefinition::Damage {
                    falloff,
                    recipients,
                    ..
                } => {
                    add(
                        effect_root("Damage", path!["amount"]),
                        &group,
                        "Damage",
                        NumberSpec::integer(1, u32::from(policy.max_damage), "health"),
                    );
                    if let DamageFalloff::Linear { .. } = falloff {
                        add(
                            effect_root("Damage", path!["falloff", "Linear", "start_distance"]),
                            &group,
                            "Falloff start",
                            NumberSpec::decimal(
                                0.0,
                                f64::from(policy.max_distance),
                                0.1,
                                "world units",
                            ),
                        );
                        add(
                            effect_root("Damage", path!["falloff", "Linear", "end_distance"]),
                            &group,
                            "Falloff end",
                            NumberSpec::decimal(
                                0.0,
                                f64::from(policy.max_distance),
                                0.1,
                                "world units",
                            )
                            .help("Must be greater than falloff start."),
                        );
                        add(
                            effect_root("Damage", path!["falloff", "Linear", "minimum_scale"]),
                            &group,
                            "Minimum damage scale",
                            NumberSpec {
                                minimum_exclusive: true,
                                ..NumberSpec::decimal(0.0, 1.0, 0.01, "×").ranged()
                            },
                        );
                    }
                    if let RecipientPolicy::HostilesAndOwner { .. } = recipients {
                        add(
                            effect_root(
                                "Damage",
                                path!["recipients", "HostilesAndOwner", "owner_scale"],
                            ),
                            &group,
                            "Owner damage scale",
                            NumberSpec::decimal(0.0, 1.0, 0.01, "×").ranged(),
                        );
                    }
                }
                PayloadEffectDefinition::Knockback { recipients, .. } => {
                    add(
                        effect_root("Knockback", path!["speed"]),
                        &group,
                        "Knockback speed",
                        NumberSpec::decimal(
                            0.0,
                            f64::from(policy.max_knockback_speed),
                            0.1,
                            "world units/s",
                        ),
                    );
                    add(
                        effect_root("Knockback", path!["duration_ticks"]),
                        &group,
                        "Knockback duration",
                        NumberSpec::ticks(1, policy.max_effect_duration_ticks),
                    );
                    if let RecipientPolicy::HostilesAndOwner { .. } = recipients {
                        add(
                            effect_root(
                                "Knockback",
                                path!["recipients", "HostilesAndOwner", "owner_scale"],
                            ),
                            &group,
                            "Owner effect scale",
                            NumberSpec::decimal(0.0, 1.0, 0.01, "×").ranged(),
                        );
                    }
                }
                PayloadEffectDefinition::Slow { recipients, .. } => {
                    add(
                        effect_root("Slow", path!["movement_multiplier"]),
                        &group,
                        "Movement multiplier",
                        NumberSpec {
                            minimum_exclusive: true,
                            ..NumberSpec::decimal(0.0, 1.0, 0.01, "×").ranged()
                        },
                    );
                    add(
                        effect_root("Slow", path!["duration_ticks"]),
                        &group,
                        "Slow duration",
                        NumberSpec::ticks(1, policy.max_effect_duration_ticks),
                    );
                    if let RecipientPolicy::HostilesAndOwner { .. } = recipients {
                        add(
                            effect_root(
                                "Slow",
                                path!["recipients", "HostilesAndOwner", "owner_scale"],
                            ),
                            &group,
                            "Owner effect scale",
                            NumberSpec::decimal(0.0, 1.0, 0.01, "×").ranged(),
                        );
                    }
                }
                PayloadEffectDefinition::Cold { .. } => add(
                    effect_root("Cold", path!["amount"]),
                    &group,
                    "Cold per hit",
                    NumberSpec::integer(1, u32::from(u16::MAX), "cold")
                        .help("Applied after resistance against the target's Cold capacity."),
                ),
                PayloadEffectDefinition::DamageOverTime { .. } => {
                    add(
                        effect_root("DamageOverTime", path!["damage_per_tick"]),
                        &group,
                        "Damage per tick",
                        NumberSpec::integer(1, u32::from(policy.max_damage), "health"),
                    );
                    add(
                        effect_root("DamageOverTime", path!["tick_interval"]),
                        &group,
                        "Tick interval",
                        NumberSpec::ticks(1, policy.max_effect_duration_ticks),
                    );
                    add(
                        effect_root("DamageOverTime", path!["duration_ticks"]),
                        &group,
                        "Duration",
                        NumberSpec::ticks(1, policy.max_effect_duration_ticks),
                    );
                }
                PayloadEffectDefinition::Heal { .. } => add(
                    effect_root("Heal", path!["amount"]),
                    &group,
                    "Healing",
                    NumberSpec::integer(1, u32::from(u16::MAX), "health"),
                ),
            }
        }
    }
    for (effect_index, effect) in weapon.recipe.world_effects.iter().enumerate() {
        match effect {
            WorldEffectDefinition::DestroyMap { .. } => add(
                path!["world_effects", effect_index, "DestroyMap", "radius"],
                "World effect",
                "Destruction radius",
                NumberSpec::positive_decimal(128.0, "world units")
                    .help("128 world units is the current bounded terrain-event safety ceiling."),
            ),
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

#[allow(
    clippy::too_many_lines,
    reason = "explicit descriptors keep the small passive editor contract auditable"
)]
fn add_passive_fields(fields: &mut Vec<EditorFieldDescriptor>, snapshot: &BalanceLabSnapshotV3) {
    for (index, passive) in snapshot.passives.iter().enumerate() {
        let variant = match passive.parameters {
            PassiveParameters::LightweightFrame | PassiveParameters::ReinforcedFrame => continue,
            PassiveParameters::AdrenalResponse { .. } => "AdrenalResponse",
            PassiveParameters::CloseQuarters { .. } => "CloseQuarters",
            PassiveParameters::QuickCycle { .. } => "QuickCycle",
            PassiveParameters::Tenacity { .. } => "Tenacity",
            PassiveParameters::CryogenicInsulation { .. } => "CryogenicInsulation",
            PassiveParameters::FilteredCirculation { .. } => "FilteredCirculation",
            PassiveParameters::HeatShielding { .. } => "HeatShielding",
        };
        let descriptors: &[(&str, &str, &str, NumberSpec)] = match passive.parameters {
            PassiveParameters::AdrenalResponse { .. } => &[
                (
                    "duration_ticks",
                    "Timing",
                    "Boost duration",
                    NumberSpec::ticks(1, 3_600),
                ),
                (
                    "rearm_ticks",
                    "Timing",
                    "Rearm time",
                    NumberSpec::ticks(1, 36_000),
                ),
                (
                    "movement_bonus_basis_points",
                    "Effect",
                    "Movement bonus",
                    NumberSpec::basis_points(1, 10_000),
                ),
            ],
            PassiveParameters::CloseQuarters { .. } => &[
                (
                    "near_distance_milliunits",
                    "Distance",
                    "Near distance",
                    NumberSpec::milliunits(1, 4_096_000),
                ),
                (
                    "far_distance_milliunits",
                    "Distance",
                    "Far distance",
                    NumberSpec::milliunits(1, 4_096_000),
                ),
                (
                    "near_damage_basis_points",
                    "Effect",
                    "Near damage",
                    NumberSpec::basis_points(1, 30_000),
                ),
                (
                    "far_damage_basis_points",
                    "Effect",
                    "Far damage",
                    NumberSpec::basis_points(1, 30_000),
                ),
            ],
            PassiveParameters::QuickCycle { .. } => &[(
                "refill_duration_basis_points",
                "Effect",
                "Refill duration",
                NumberSpec::basis_points(1, 10_000),
            )],
            PassiveParameters::Tenacity { .. } => &[(
                "slow_duration_basis_points",
                "Effect",
                "Slow duration",
                NumberSpec::basis_points(1, 10_000),
            )],
            PassiveParameters::CryogenicInsulation { .. }
            | PassiveParameters::FilteredCirculation { .. }
            | PassiveParameters::HeatShielding { .. } => &[(
                "resistance_basis_points",
                "Effect",
                "Resistance",
                NumberSpec::basis_points(1, 6_000),
            )],
            PassiveParameters::LightweightFrame | PassiveParameters::ReinforcedFrame => &[],
        };
        for (tail, group, label, spec) in descriptors {
            add_field(
                fields,
                path!["passives", index, "parameters", variant, *tail],
                EditorSection::Ultimates,
                &passive.key,
                &passive.display_name,
                group,
                label,
                *spec,
            );
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
