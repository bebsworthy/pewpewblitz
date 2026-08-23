//! Pure modular arena-edge and deterministic outer-dressing layout.

use super::*;

pub(super) const EDGE_MODULE: f32 = 64.0;
pub(super) const OUTER_GROUND_MARGIN: f32 = 1_152.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BorderModule {
    pub(super) position: Vec2,
    pub(super) rotation: f32,
    pub(super) corner: bool,
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
}
