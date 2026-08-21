//! Pure modular arena-edge and deterministic outer-dressing layout.

use super::*;

pub(super) const EDGE_MODULE: f32 = 64.0;
pub(super) const DRESSING_BAND: f32 = 320.0;
pub(super) const OUTER_GROUND_MARGIN: f32 = 1_152.0;
const DRESSING_PLACEMENT_COUNT: u64 = 64;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BorderModule {
    pub(super) position: Vec2,
    pub(super) rotation: f32,
    pub(super) corner: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct DressingPlacement {
    pub(super) position: Vec2,
    pub(super) rotation: f32,
    pub(super) variant: crate::map::MapVisualVariantId,
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "validated positive map bounds cap module counts far below u16"
)]
pub(super) fn border_modules(bounds: crate::map::AxisAlignedMapRect) -> Vec<BorderModule> {
    let size = bounds.size();
    let horizontal = (size.x / EDGE_MODULE).floor().max(1.0) as u16;
    let vertical = (size.y / EDGE_MODULE).floor().max(1.0) as u16;
    let mut result = Vec::with_capacity(usize::from((horizontal + vertical) * 2 + 4));
    for index in 0..horizontal {
        let fraction = (f32::from(index) + 0.5) / f32::from(horizontal);
        let x = bounds.min.x + size.x * fraction;
        result.push(BorderModule {
            position: Vec2::new(x, bounds.min.y),
            rotation: 0.0,
            corner: false,
        });
        result.push(BorderModule {
            position: Vec2::new(x, bounds.max.y),
            rotation: core::f32::consts::PI,
            corner: false,
        });
    }
    for index in 0..vertical {
        let fraction = (f32::from(index) + 0.5) / f32::from(vertical);
        let y = bounds.min.y + size.y * fraction;
        result.push(BorderModule {
            position: Vec2::new(bounds.min.x, y),
            rotation: core::f32::consts::FRAC_PI_2,
            corner: false,
        });
        result.push(BorderModule {
            position: Vec2::new(bounds.max.x, y),
            rotation: -core::f32::consts::FRAC_PI_2,
            corner: false,
        });
    }
    for (position, rotation) in [
        (bounds.min, 0.0),
        (
            Vec2::new(bounds.max.x, bounds.min.y),
            core::f32::consts::FRAC_PI_2,
        ),
        (bounds.max, core::f32::consts::PI),
        (
            Vec2::new(bounds.min.x, bounds.max.y),
            -core::f32::consts::FRAC_PI_2,
        ),
    ] {
        result.push(BorderModule {
            position,
            rotation,
            corner: true,
        });
    }
    result
}

pub(super) fn dressing_plan(
    bounds: crate::map::AxisAlignedMapRect,
    seed: u64,
    variants: &[crate::map::MapVisualVariantId],
) -> Vec<DressingPlacement> {
    if variants.is_empty() {
        return Vec::new();
    }
    let variant_count = u64::try_from(variants.len()).expect("variant count fits u64");
    let prominent = variants
        .iter()
        .copied()
        .filter(|variant| matches!(variant.0, 4..=6))
        .collect::<Vec<_>>();
    let prominent_count = u64::try_from(prominent.len()).expect("prominent count fits u64");
    let mut result = Vec::with_capacity(
        usize::try_from(DRESSING_PLACEMENT_COUNT).expect("placement count fits usize"),
    );
    let center = bounds.center();
    let half = bounds.size() * 0.5;
    for index in 0_u64..DRESSING_PLACEMENT_COUNT {
        let mixed = splitmix64(seed.wrapping_add(index));
        let cluster = index / 4;
        let member = index % 4;
        let side = cluster % 4;
        let lane = cluster / 4;
        let anchor_along =
            [-0.78, -0.26, 0.26, 0.78][usize::try_from(lane).expect("dressing lane fits usize")];
        let anchor_depth = if lane % 2 == 0 { 104.0 } else { 196.0 };
        let along = anchor_along + (unit(mixed.rotate_left(13)) - 0.5) * 0.10;
        let member_offset = Vec2::new(
            (unit(mixed.rotate_left(23)) - 0.5) * 76.0,
            (unit(mixed.rotate_left(31)) - 0.5) * 62.0,
        );
        let position = match side {
            0 => {
                center
                    + Vec2::new(along * (half.x + DRESSING_BAND), -half.y - anchor_depth)
                    + member_offset
            }
            1 => {
                center
                    + Vec2::new(half.x + anchor_depth, along * (half.y + DRESSING_BAND))
                    + Vec2::new(member_offset.y, member_offset.x)
            }
            2 => {
                center
                    + Vec2::new(along * (half.x + DRESSING_BAND), half.y + anchor_depth)
                    + member_offset
            }
            _ => {
                center
                    + Vec2::new(-half.x - anchor_depth, along * (half.y + DRESSING_BAND))
                    + Vec2::new(member_offset.y, member_offset.x)
            }
        };
        result.push(DressingPlacement {
            position,
            rotation: unit(mixed.rotate_left(41)) * core::f32::consts::TAU,
            variant: if member == 0 && prominent_count > 0 {
                prominent[usize::try_from(cluster % prominent_count)
                    .expect("prominent variant index fits usize")]
            } else {
                variants[usize::try_from(mixed % variant_count).expect("variant index fits usize")]
            },
        });
    }
    result
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "the low pseudorandom word is intentionally normalized for visual placement"
)]
fn unit(value: u64) -> f32 {
    (value as u32) as f32 / u32::MAX as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds() -> crate::map::AxisAlignedMapRect {
        crate::map::AxisAlignedMapRect {
            min: Vec2::new(-896.0, -576.0),
            max: Vec2::new(896.0, 576.0),
        }
    }

    #[test]
    fn border_has_complete_sides_and_dedicated_corners() {
        let modules = border_modules(bounds());
        assert_eq!(modules.iter().filter(|module| module.corner).count(), 4);
        assert_eq!(modules.len(), 96);
    }

    #[test]
    fn dressing_is_deterministic_bounded_and_outside_play() {
        let variants = [
            crate::map::MapVisualVariantId(5),
            crate::map::MapVisualVariantId(6),
        ];
        let first = dressing_plan(bounds(), 42, &variants);
        assert_eq!(first, dressing_plan(bounds(), 42, &variants));
        assert_eq!(first.len(), 64);
        assert!(
            first
                .iter()
                .all(|placement| !bounds().contains(placement.position))
        );
    }
}
