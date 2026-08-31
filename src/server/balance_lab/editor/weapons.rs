use super::{EditorFieldDescriptor, EditorPathSegment, EditorSection, NumberSpec, add_field};
use crate::combat::definitions::{
    MAX_COLD_PAYLOAD_AMOUNT, MAX_SPLASH_ACTIVE_PER_OWNER, MAX_STICKY_ACTIVE_PER_OWNER,
};
use crate::combat::{
    DamageFalloff, DeliveryMethod, EngineWeaponLimits, FiringPattern, PayloadEffectDefinition,
    PersistentAreaShape, RecipientPolicy, TargetSelection, WeaponCatalog, WeaponEconomy,
    WorldEffectDefinition,
};

macro_rules! path {
    ($($part:expr),+ $(,)?) => {
        vec![$(EditorPathSegment::from($part)),+]
    };
}

#[allow(
    clippy::too_many_lines,
    reason = "explicit descriptors keep the complete weapon editor contract auditable"
)]
pub(super) fn add_fields(
    fields: &mut Vec<EditorFieldDescriptor>,
    index: usize,
    weapon: &super::super::WeaponPresetTuning,
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
        WeaponEconomy::Magazine {
            capacity: _,
            refill_ticks: _,
        } => {
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
        WeaponEconomy::Charges {
            capacity: _,
            recharge_ticks: _,
        } => {
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
    match weapon.recipe.firing {
        FiringPattern::Single => {}
        FiringPattern::Spread {
            delivery_count: _,
            total_angle_degrees: _,
        } => {
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
                NumberSpec::positive_decimal(f64::from(policy.max_angle_degrees), "degrees")
                    .ranged(),
            );
        }
    }
    match weapon.recipe.delivery {
        DeliveryMethod::Straight {
            speed: _,
            radius: _,
            range: _,
            lifetime_ticks: _,
            muzzle_offset: _,
        } => {
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
        DeliveryMethod::StickyStraight {
            speed: _,
            radius: _,
            range: _,
            lifetime_ticks: _,
            muzzle_offset: _,
            fuse_ticks: _,
            max_active_per_owner: _,
        } => {
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
                NumberSpec::integer(1, u32::from(MAX_STICKY_ACTIVE_PER_OWNER), "blobs"),
            );
        }
        DeliveryMethod::Lobbed {
            distance: _,
            max_flight_ticks: _,
            visual_arc_height: _,
            landing_clearance_radius: _,
            muzzle_offset: _,
        } => {
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
                NumberSpec::ticks(
                    u32::try_from(policy.minimum_lob_flight_ticks)
                        .expect("validated lob minimum fits editor representation"),
                    policy
                        .max_projectile_lifetime_ticks
                        .min(limits.max_lifetime_ticks),
                ),
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
        DeliveryMethod::MeleeArc {
            reach: _,
            angle_degrees: _,
        } => {
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
        DeliveryMethod::ConeSpray {
            propagation_speed: _,
            reach: _,
            angle_degrees: _,
            linger_ticks: _,
            pulse_interval_ticks: _,
            map_occlusion: _,
            max_targets: _,
        } => {
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
        DeliveryMethod::Splash {
            distance: _,
            max_flight_ticks: _,
            visual_arc_height: _,
            landing_clearance_radius: _,
            muzzle_offset: _,
            shape,
            duration_ticks: _,
            pulse_interval_ticks: _,
            map_occlusion: _,
            max_targets: _,
            max_active_per_owner: _,
        } => {
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
                NumberSpec::ticks(
                    u32::try_from(policy.minimum_lob_flight_ticks)
                        .expect("validated lob minimum fits editor representation"),
                    policy
                        .max_projectile_lifetime_ticks
                        .min(limits.max_lifetime_ticks),
                ),
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
                PersistentAreaShape::Circle { radius: _ } => add(
                    path!["delivery", "Splash", "shape", "Circle", "radius"],
                    "Splash area",
                    "Circle radius",
                    NumberSpec::positive_decimal(f64::from(policy.max_radius), "world units"),
                ),
                PersistentAreaShape::Rectangle { half_extents: _ } => {
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
                NumberSpec::integer(1, u32::from(MAX_SPLASH_ACTIVE_PER_OWNER), "areas"),
            );
        }
    }

    for (bundle_index, bundle) in weapon.recipe.payload_bundles.iter().enumerate() {
        let group = format!("Payload {}", bundle_index + 1);
        match bundle.target {
            TargetSelection::Direct => {}
            TargetSelection::Area {
                radius: _,
                map_occlusion: _,
                max_targets: _,
            } => {
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
                    amount: _,
                    falloff,
                    recipients,
                } => {
                    add(
                        effect_root("Damage", path!["amount"]),
                        &group,
                        "Damage",
                        NumberSpec::integer(
                            1,
                            u32::from(limits.max_damage.min(policy.max_damage)),
                            "health",
                        ),
                    );
                    match falloff {
                        DamageFalloff::None => {}
                        DamageFalloff::Linear {
                            start_distance: _,
                            end_distance: _,
                            minimum_scale: _,
                        } => {
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
                    }
                    match recipients {
                        RecipientPolicy::Hostiles
                        | RecipientPolicy::Allies
                        | RecipientPolicy::AlliesAndOwner => {}
                        RecipientPolicy::HostilesAndOwner { owner_scale: _ } => {
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
                }
                PayloadEffectDefinition::Knockback {
                    speed: _,
                    duration_ticks: _,
                    recipients,
                } => {
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
                    match recipients {
                        RecipientPolicy::Hostiles
                        | RecipientPolicy::Allies
                        | RecipientPolicy::AlliesAndOwner => {}
                        RecipientPolicy::HostilesAndOwner { owner_scale: _ } => {
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
                }
                PayloadEffectDefinition::Slow {
                    movement_multiplier: _,
                    duration_ticks: _,
                    stacking: _,
                    recipients,
                } => {
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
                    match recipients {
                        RecipientPolicy::Hostiles
                        | RecipientPolicy::Allies
                        | RecipientPolicy::AlliesAndOwner => {}
                        RecipientPolicy::HostilesAndOwner { owner_scale: _ } => {
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
                }
                PayloadEffectDefinition::Cold {
                    amount: _,
                    recipients,
                } => {
                    add(
                        effect_root("Cold", path!["amount"]),
                        &group,
                        "Cold per hit",
                        NumberSpec::integer(1, u32::from(MAX_COLD_PAYLOAD_AMOUNT), "cold")
                            .help("Applied after resistance against the target's Cold capacity."),
                    );
                    match recipients {
                        RecipientPolicy::Hostiles
                        | RecipientPolicy::Allies
                        | RecipientPolicy::AlliesAndOwner => {}
                        RecipientPolicy::HostilesAndOwner { owner_scale: _ } => {
                            add(
                                effect_root(
                                    "Cold",
                                    path!["recipients", "HostilesAndOwner", "owner_scale"],
                                ),
                                &group,
                                "Owner effect scale",
                                NumberSpec::decimal(0.0, 1.0, 0.01, "×").ranged(),
                            );
                        }
                    }
                }
                PayloadEffectDefinition::DamageOverTime {
                    kind: _,
                    damage_per_tick: _,
                    tick_interval: _,
                    duration_ticks: _,
                    recipients,
                } => {
                    add(
                        effect_root("DamageOverTime", path!["damage_per_tick"]),
                        &group,
                        "Damage per tick",
                        NumberSpec::integer(
                            1,
                            u32::from(limits.max_damage.min(policy.max_damage)),
                            "health",
                        ),
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
                    match recipients {
                        RecipientPolicy::Hostiles
                        | RecipientPolicy::Allies
                        | RecipientPolicy::AlliesAndOwner => {}
                        RecipientPolicy::HostilesAndOwner { owner_scale: _ } => {
                            add(
                                effect_root(
                                    "DamageOverTime",
                                    path!["recipients", "HostilesAndOwner", "owner_scale"],
                                ),
                                &group,
                                "Owner effect scale",
                                NumberSpec::decimal(0.0, 1.0, 0.01, "×").ranged(),
                            );
                        }
                    }
                }
                PayloadEffectDefinition::Heal {
                    amount: _,
                    recipients,
                } => {
                    add(
                        effect_root("Heal", path!["amount"]),
                        &group,
                        "Healing",
                        NumberSpec::integer(
                            1,
                            u32::from(limits.max_damage.min(policy.max_damage)),
                            "health",
                        ),
                    );
                    match recipients {
                        RecipientPolicy::Hostiles
                        | RecipientPolicy::Allies
                        | RecipientPolicy::AlliesAndOwner => {}
                        RecipientPolicy::HostilesAndOwner { owner_scale: _ } => {
                            add(
                                effect_root(
                                    "Heal",
                                    path!["recipients", "HostilesAndOwner", "owner_scale"],
                                ),
                                &group,
                                "Owner effect scale",
                                NumberSpec::decimal(0.0, 1.0, 0.01, "×").ranged(),
                            );
                        }
                    }
                }
            }
        }
    }
    for (effect_index, effect) in weapon.recipe.world_effects.iter().enumerate() {
        match effect {
            WorldEffectDefinition::DestroyMap { radius: _ } => add(
                path!["world_effects", effect_index, "DestroyMap", "radius"],
                "World effect",
                "Destruction radius",
                NumberSpec::positive_decimal(
                    f64::from(limits.max_map_destruction_radius),
                    "world units",
                )
                .help("128 world units is the current bounded terrain-event safety ceiling."),
            ),
        }
    }
}
