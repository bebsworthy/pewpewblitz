use super::{EditorFieldDescriptor, EditorPathSegment, EditorSection, NumberSpec, add_field};
use crate::builds::{ElementalFieldEffect, ULTIMATE_PARAMETER_BOUNDS, UltimateParameters};

macro_rules! path {
    ($($part:expr),+ $(,)?) => {
        vec![$(EditorPathSegment::from($part)),+]
    };
}

macro_rules! milliunits {
    ($bounds:expr) => {
        NumberSpec::milliunits($bounds.minimum, $bounds.maximum)
    };
}

macro_rules! ticks {
    ($bounds:expr) => {
        NumberSpec::ticks(
            u32::try_from($bounds.minimum)
                .expect("ultimate tick minimum fits editor representation"),
            $bounds.maximum,
        )
    };
}

#[allow(
    clippy::too_many_lines,
    reason = "explicit descriptors keep the complete ultimate editor contract auditable"
)]
pub(super) fn add_fields(
    fields: &mut Vec<EditorFieldDescriptor>,
    snapshot: &super::super::BalanceLabSnapshotV3,
) {
    let bounds = ULTIMATE_PARAMETER_BOUNDS;
    for (index, ultimate) in snapshot.ultimates.iter().enumerate() {
        let mut add = |tail: Vec<EditorPathSegment>, group: &str, label: &str, spec: NumberSpec| {
            let mut descriptor_path = path!["ultimates", index, "parameters"];
            descriptor_path.extend(tail);
            add_field(
                fields,
                descriptor_path,
                EditorSection::Ultimates,
                &ultimate.key,
                &ultimate.display_name,
                group,
                label,
                spec,
            );
        };
        match ultimate.parameters {
            UltimateParameters::Dash {
                maximum_distance_milliunits: _,
                duration_ticks: _,
                damage: _,
                knockback_speed_milliunits: _,
                knockback_duration_ticks: _,
                maximum_targets: _,
            } => {
                for (tail, group, label, spec) in [
                    (
                        "maximum_distance_milliunits",
                        "Movement",
                        "Maximum distance",
                        milliunits!(bounds.world_distance_milliunits),
                    ),
                    (
                        "duration_ticks",
                        "Movement",
                        "Duration",
                        ticks!(bounds.short_ticks),
                    ),
                    (
                        "damage",
                        "Impact",
                        "Damage",
                        NumberSpec::integer(
                            u32::from(bounds.damage.minimum),
                            u32::from(bounds.damage.maximum),
                            "health",
                        ),
                    ),
                    (
                        "knockback_speed_milliunits",
                        "Impact",
                        "Knockback speed",
                        milliunits!(bounds.world_distance_milliunits),
                    ),
                    (
                        "knockback_duration_ticks",
                        "Impact",
                        "Knockback duration",
                        ticks!(bounds.short_ticks),
                    ),
                    (
                        "maximum_targets",
                        "Capacity",
                        "Maximum targets",
                        NumberSpec::integer(
                            u32::from(bounds.target_count.minimum),
                            u32::from(bounds.target_count.maximum),
                            "targets",
                        ),
                    ),
                ] {
                    add(path!["Dash", tail], group, label, spec);
                }
            }
            UltimateParameters::Sentry {
                placement_offsets_milliunits: _,
                body_radius_milliunits: _,
                acquisition_range_milliunits: _,
                acquisition_interval_ticks: _,
                fire_interval_ticks: _,
                lifetime_ticks: _,
                maximum_health: _,
                projectile_speed_milliunits: _,
                projectile_radius_milliunits: _,
                projectile_range_milliunits: _,
                projectile_lifetime_ticks: _,
                projectile_damage: _,
            } => {
                for offset in 0..6 {
                    add(
                        path!["Sentry", "placement_offsets_milliunits", offset],
                        "Placement",
                        &format!("Placement offset {}", offset + 1),
                        milliunits!(bounds.sentry_placement_offset_milliunits),
                    );
                }
                for (tail, group, label, spec) in [
                    (
                        "body_radius_milliunits",
                        "Deployable",
                        "Body radius",
                        milliunits!(bounds.compact_radius_milliunits),
                    ),
                    (
                        "acquisition_range_milliunits",
                        "Targeting",
                        "Acquisition range",
                        milliunits!(bounds.world_distance_milliunits),
                    ),
                    (
                        "acquisition_interval_ticks",
                        "Targeting",
                        "Acquisition interval",
                        ticks!(bounds.short_ticks),
                    ),
                    (
                        "fire_interval_ticks",
                        "Firing",
                        "Fire interval",
                        ticks!(bounds.duration_ticks),
                    ),
                    (
                        "lifetime_ticks",
                        "Deployable",
                        "Lifetime",
                        ticks!(bounds.long_lifetime_ticks),
                    ),
                    (
                        "maximum_health",
                        "Deployable",
                        "Maximum health",
                        NumberSpec::integer(
                            u32::from(bounds.health.minimum),
                            u32::from(bounds.health.maximum),
                            "health",
                        ),
                    ),
                    (
                        "projectile_speed_milliunits",
                        "Projectile",
                        "Speed",
                        milliunits!(bounds.world_distance_milliunits),
                    ),
                    (
                        "projectile_radius_milliunits",
                        "Projectile",
                        "Radius",
                        milliunits!(bounds.compact_radius_milliunits),
                    ),
                    (
                        "projectile_range_milliunits",
                        "Projectile",
                        "Range",
                        milliunits!(bounds.world_distance_milliunits),
                    ),
                    (
                        "projectile_lifetime_ticks",
                        "Projectile",
                        "Flight lifetime",
                        ticks!(bounds.short_ticks),
                    ),
                    (
                        "projectile_damage",
                        "Projectile",
                        "Damage",
                        NumberSpec::integer(
                            u32::from(bounds.damage.minimum),
                            u32::from(bounds.damage.maximum),
                            "health",
                        ),
                    ),
                ] {
                    add(path!["Sentry", tail], group, label, spec);
                }
            }
            UltimateParameters::SelfCloak { duration_ticks: _ } => add(
                path!["SelfCloak", "duration_ticks"],
                "Timing",
                "Cloak duration",
                ticks!(bounds.duration_ticks),
            ),
            UltimateParameters::RevealScan {
                maximum_range_milliunits: _,
                radius_milliunits: _,
                reveal_ticks: _,
            } => {
                add(
                    path!["RevealScan", "maximum_range_milliunits"],
                    "Targeting",
                    "Maximum range",
                    milliunits!(bounds.world_distance_milliunits),
                );
                add(
                    path!["RevealScan", "radius_milliunits"],
                    "Area",
                    "Reveal radius",
                    milliunits!(bounds.field_radius_milliunits),
                );
                add(
                    path!["RevealScan", "reveal_ticks"],
                    "Timing",
                    "Reveal duration",
                    ticks!(bounds.duration_ticks),
                );
            }
            UltimateParameters::ConcealmentField {
                maximum_range_milliunits: _,
                radius_milliunits: _,
                duration_ticks: _,
            } => {
                add(
                    path!["ConcealmentField", "maximum_range_milliunits"],
                    "Targeting",
                    "Maximum range",
                    milliunits!(bounds.world_distance_milliunits),
                );
                add(
                    path!["ConcealmentField", "radius_milliunits"],
                    "Area",
                    "Field radius",
                    milliunits!(bounds.field_radius_milliunits),
                );
                add(
                    path!["ConcealmentField", "duration_ticks"],
                    "Timing",
                    "Field duration",
                    ticks!(bounds.duration_ticks),
                );
            }
            UltimateParameters::DemolitionStrike {
                maximum_range_milliunits: _,
                radius_milliunits: _,
            } => {
                add(
                    path!["DemolitionStrike", "maximum_range_milliunits"],
                    "Targeting",
                    "Maximum range",
                    milliunits!(bounds.world_distance_milliunits),
                );
                add(
                    path!["DemolitionStrike", "radius_milliunits"],
                    "Area",
                    "Destruction radius",
                    milliunits!(bounds.demolition_radius_milliunits),
                );
            }
            UltimateParameters::ElementalField {
                maximum_range_milliunits: _,
                radius_milliunits: _,
                duration_ticks: _,
                pulse_interval_ticks: _,
                effect,
            } => {
                for (tail, group, label, spec) in [
                    (
                        "maximum_range_milliunits",
                        "Targeting",
                        "Maximum range",
                        milliunits!(bounds.world_distance_milliunits),
                    ),
                    (
                        "radius_milliunits",
                        "Area",
                        "Field radius",
                        milliunits!(bounds.field_radius_milliunits),
                    ),
                    (
                        "duration_ticks",
                        "Timing",
                        "Field duration",
                        ticks!(bounds.duration_ticks),
                    ),
                    (
                        "pulse_interval_ticks",
                        "Timing",
                        "Pulse interval",
                        ticks!(bounds.duration_ticks),
                    ),
                ] {
                    add(path!["ElementalField", tail], group, label, spec);
                }
                match effect {
                    ElementalFieldEffect::Cold { amount: _ } => add(
                        path!["ElementalField", "effect", "Cold", "amount"],
                        "Effect",
                        "Cold per pulse",
                        NumberSpec::integer(
                            u32::from(bounds.effect_amount.minimum),
                            u32::from(bounds.effect_amount.maximum),
                            "cold/pulse",
                        )
                        .help("Applied after resistance against each target's Cold capacity."),
                    ),
                    ElementalFieldEffect::DamageOverTime {
                        kind: _,
                        damage_per_tick: _,
                        tick_interval: _,
                        duration_ticks: _,
                    } => {
                        add(
                            path![
                                "ElementalField",
                                "effect",
                                "DamageOverTime",
                                "damage_per_tick"
                            ],
                            "Effect",
                            "Damage per tick",
                            NumberSpec::integer(
                                u32::from(bounds.effect_amount.minimum),
                                u32::from(bounds.effect_amount.maximum),
                                "points",
                            )
                            .help("Applied on each authoritative field pulse."),
                        );
                        add(
                            path![
                                "ElementalField",
                                "effect",
                                "DamageOverTime",
                                "tick_interval"
                            ],
                            "Effect",
                            "Damage interval",
                            ticks!(bounds.duration_ticks),
                        );
                        add(
                            path![
                                "ElementalField",
                                "effect",
                                "DamageOverTime",
                                "duration_ticks"
                            ],
                            "Effect",
                            "Damage duration",
                            ticks!(bounds.duration_ticks),
                        );
                    }
                    ElementalFieldEffect::Heal { amount: _ } => add(
                        path!["ElementalField", "effect", "Heal", "amount"],
                        "Effect",
                        "Healing",
                        NumberSpec::integer(
                            u32::from(bounds.effect_amount.minimum),
                            u32::from(bounds.effect_amount.maximum),
                            "points",
                        )
                        .help("Applied on each authoritative field pulse."),
                    ),
                }
            }
            UltimateParameters::BigBlob {
                maximum_range_milliunits: _,
                flight_ticks: _,
                visual_arc_height_milliunits: _,
                landing_clearance_milliunits: _,
                child_speed_milliunits: _,
                child_radius_milliunits: _,
                child_range_milliunits: _,
                child_lifetime_ticks: _,
                child_fuse_ticks: _,
                child_explosion_radius_milliunits: _,
                child_damage: _,
                max_active_per_owner: _,
            } => {
                for (tail, group, label, spec) in [
                    (
                        "maximum_range_milliunits",
                        "Targeting",
                        "Maximum throw range",
                        milliunits!(bounds.world_distance_milliunits),
                    ),
                    (
                        "flight_ticks",
                        "Targeting",
                        "Lob flight time",
                        ticks!(bounds.short_ticks),
                    ),
                    (
                        "visual_arc_height_milliunits",
                        "Targeting",
                        "Visual arc height",
                        milliunits!(bounds.field_radius_milliunits),
                    ),
                    (
                        "landing_clearance_milliunits",
                        "Targeting",
                        "Landing clearance",
                        milliunits!(bounds.compact_radius_milliunits),
                    ),
                    (
                        "child_speed_milliunits",
                        "Secondary blobs",
                        "Travel speed",
                        milliunits!(bounds.world_distance_milliunits),
                    ),
                    (
                        "child_radius_milliunits",
                        "Secondary blobs",
                        "Projectile radius",
                        milliunits!(bounds.compact_radius_milliunits),
                    ),
                    (
                        "child_range_milliunits",
                        "Secondary blobs",
                        "Travel range",
                        milliunits!(bounds.world_distance_milliunits),
                    ),
                    (
                        "child_lifetime_ticks",
                        "Secondary blobs",
                        "Flight lifetime",
                        ticks!(bounds.short_ticks),
                    ),
                    (
                        "child_fuse_ticks",
                        "Explosion",
                        "Fuse delay",
                        ticks!(bounds.duration_ticks),
                    ),
                    (
                        "child_explosion_radius_milliunits",
                        "Explosion",
                        "Explosion radius",
                        milliunits!(bounds.compact_radius_milliunits),
                    ),
                    (
                        "child_damage",
                        "Explosion",
                        "Damage",
                        NumberSpec::integer(
                            u32::from(bounds.damage.minimum),
                            u32::from(bounds.damage.maximum),
                            "health",
                        ),
                    ),
                    (
                        "max_active_per_owner",
                        "Capacity",
                        "Maximum active blobs",
                        NumberSpec::integer(
                            u32::from(bounds.active_count.minimum),
                            u32::from(bounds.active_count.maximum),
                            "blobs",
                        ),
                    ),
                ] {
                    add(path!["BigBlob", tail], group, label, spec);
                }
            }
        }
    }
}
