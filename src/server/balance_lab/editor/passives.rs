use super::{EditorFieldDescriptor, EditorPathSegment, EditorSection, NumberSpec, add_field};
use crate::builds::{
    PASSIVE_ADRENAL_DURATION_TICKS_BOUNDS, PASSIVE_ADRENAL_MOVEMENT_BONUS_BASIS_POINTS_BOUNDS,
    PASSIVE_ADRENAL_REARM_TICKS_BOUNDS, PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS,
    PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS,
    PASSIVE_ELEMENTAL_RESISTANCE_BASIS_POINTS_BOUNDS,
    PASSIVE_QUICK_CYCLE_REFILL_BASIS_POINTS_BOUNDS, PASSIVE_TENACITY_SLOW_BASIS_POINTS_BOUNDS,
    PassiveParameters,
};

type FieldDescriptor = (&'static str, &'static str, &'static str, NumberSpec);

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive family projection makes passive schema additions compile-visible"
)]
pub(super) fn add_fields(
    fields: &mut Vec<EditorFieldDescriptor>,
    snapshot: &super::super::BalanceLabSnapshotV3,
) {
    for (index, passive) in snapshot.passives.iter().enumerate() {
        let resistance_descriptors = [(
            "resistance_basis_points",
            "Effect",
            "Resistance",
            NumberSpec::basis_points(
                u32::from(PASSIVE_ELEMENTAL_RESISTANCE_BASIS_POINTS_BOUNDS.minimum),
                u32::from(PASSIVE_ELEMENTAL_RESISTANCE_BASIS_POINTS_BOUNDS.maximum),
            ),
        )];
        let (variant, descriptors): (&str, &[FieldDescriptor]) = match passive.parameters {
            PassiveParameters::LightweightFrame | PassiveParameters::ReinforcedFrame => continue,
            PassiveParameters::AdrenalResponse {
                duration_ticks: _,
                rearm_ticks: _,
                movement_bonus_basis_points: _,
            } => (
                "AdrenalResponse",
                &[
                    (
                        "duration_ticks",
                        "Timing",
                        "Boost duration",
                        NumberSpec::ticks(
                            u32::try_from(PASSIVE_ADRENAL_DURATION_TICKS_BOUNDS.minimum)
                                .expect("passive tick minimum fits editor representation"),
                            PASSIVE_ADRENAL_DURATION_TICKS_BOUNDS.maximum,
                        ),
                    ),
                    (
                        "rearm_ticks",
                        "Timing",
                        "Rearm time",
                        NumberSpec::ticks(
                            u32::try_from(PASSIVE_ADRENAL_REARM_TICKS_BOUNDS.minimum)
                                .expect("passive tick minimum fits editor representation"),
                            PASSIVE_ADRENAL_REARM_TICKS_BOUNDS.maximum,
                        ),
                    ),
                    (
                        "movement_bonus_basis_points",
                        "Effect",
                        "Movement bonus",
                        NumberSpec::basis_points(
                            u32::from(PASSIVE_ADRENAL_MOVEMENT_BONUS_BASIS_POINTS_BOUNDS.minimum),
                            u32::from(PASSIVE_ADRENAL_MOVEMENT_BONUS_BASIS_POINTS_BOUNDS.maximum),
                        ),
                    ),
                ],
            ),
            PassiveParameters::CloseQuarters {
                near_distance_milliunits: _,
                far_distance_milliunits: _,
                near_damage_basis_points: _,
                far_damage_basis_points: _,
            } => (
                "CloseQuarters",
                &[
                    (
                        "near_distance_milliunits",
                        "Distance",
                        "Near distance",
                        NumberSpec::milliunits(
                            PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.minimum,
                            PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.maximum,
                        ),
                    ),
                    (
                        "far_distance_milliunits",
                        "Distance",
                        "Far distance",
                        NumberSpec::milliunits(
                            PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.minimum,
                            PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.maximum,
                        ),
                    ),
                    (
                        "near_damage_basis_points",
                        "Effect",
                        "Near damage",
                        NumberSpec::basis_points(
                            u32::from(PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.minimum),
                            u32::from(PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.maximum),
                        ),
                    ),
                    (
                        "far_damage_basis_points",
                        "Effect",
                        "Far damage",
                        NumberSpec::basis_points(
                            u32::from(PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.minimum),
                            u32::from(PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.maximum),
                        ),
                    ),
                ],
            ),
            PassiveParameters::QuickCycle {
                refill_duration_basis_points: _,
            } => (
                "QuickCycle",
                &[(
                    "refill_duration_basis_points",
                    "Effect",
                    "Refill duration",
                    NumberSpec::basis_points(
                        u32::from(PASSIVE_QUICK_CYCLE_REFILL_BASIS_POINTS_BOUNDS.minimum),
                        u32::from(PASSIVE_QUICK_CYCLE_REFILL_BASIS_POINTS_BOUNDS.maximum),
                    ),
                )],
            ),
            PassiveParameters::Tenacity {
                slow_duration_basis_points: _,
            } => (
                "Tenacity",
                &[(
                    "slow_duration_basis_points",
                    "Effect",
                    "Slow duration",
                    NumberSpec::basis_points(
                        u32::from(PASSIVE_TENACITY_SLOW_BASIS_POINTS_BOUNDS.minimum),
                        u32::from(PASSIVE_TENACITY_SLOW_BASIS_POINTS_BOUNDS.maximum),
                    ),
                )],
            ),
            PassiveParameters::CryogenicInsulation {
                resistance_basis_points: _,
            } => ("CryogenicInsulation", &resistance_descriptors),
            PassiveParameters::FilteredCirculation {
                resistance_basis_points: _,
            } => ("FilteredCirculation", &resistance_descriptors),
            PassiveParameters::HeatShielding {
                resistance_basis_points: _,
            } => ("HeatShielding", &resistance_descriptors),
        };
        for (tail, group, label, spec) in descriptors {
            add_field(
                fields,
                passive_path(index, variant, tail),
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

fn passive_path(index: usize, variant: &str, tail: &str) -> Vec<EditorPathSegment> {
    vec![
        EditorPathSegment::from("passives"),
        EditorPathSegment::from(index),
        EditorPathSegment::from("parameters"),
        EditorPathSegment::from(variant),
        EditorPathSegment::from(tail),
    ]
}
