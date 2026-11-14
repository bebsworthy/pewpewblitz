//! Neutral measurement of the same post-platform-handler samples consumed by gameplay.
use super::ClientInputSettings;
use bevy::prelude::Vec2;

const SETTLE_SECONDS: f32 = 0.5;
const SAMPLE_SECONDS: f32 = 2.5;
const MAX_REST_MAGNITUDE: f32 = 0.25;
const MARGIN: f32 = 0.02;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Recommendation {
    pub(crate) movement: f32,
    pub(crate) aim: f32,
}
impl Recommendation {
    pub(crate) fn apply(self, settings: &mut ClientInputSettings) {
        settings.move_deadzone = self.movement;
        settings.aim_deadzone = self.aim;
        settings.aim_commit_threshold = settings.aim_commit_threshold.max(self.aim);
        settings.revision = settings.revision.saturating_add(1);
    }
}

#[derive(Clone, Default, Debug)]
pub(crate) struct NeutralMeasurement {
    elapsed: f32,
    samples: u32,
    peak: Vec2,
    invalid: bool,
}
impl NeutralMeasurement {
    pub(crate) fn progress(&self) -> f32 {
        (self.elapsed / (SETTLE_SECONDS + SAMPLE_SECONDS)).clamp(0.0, 1.0)
    }
    pub(crate) fn sample(
        &mut self,
        delta: f32,
        movement: Vec2,
        aim: Vec2,
    ) -> Option<Result<Recommendation, &'static str>> {
        self.elapsed += delta.clamp(0.0, 0.1);
        if self.elapsed <= SETTLE_SECONDS {
            return None;
        }
        let magnitudes = Vec2::new(movement.length(), aim.length());
        self.invalid |= !magnitudes.is_finite() || magnitudes.max_element() > MAX_REST_MAGNITUDE;
        if magnitudes.is_finite() {
            self.peak = self.peak.max(magnitudes);
        }
        self.samples = self.samples.saturating_add(1);
        if self.elapsed < SETTLE_SECONDS + SAMPLE_SECONDS {
            return None;
        }
        Some(if self.invalid {
            Err(
                "The sticks moved too far to measure rest drift. Release both sticks and measure again.",
            )
        } else if self.samples < 30 {
            Err("Not enough samples. Measure again with the game window focused.")
        } else {
            let threshold = |value: f32| ((value + MARGIN) * 100.0).ceil().min(50.0) / 100.0;
            Ok(Recommendation {
                movement: threshold(self.peak.x),
                aim: threshold(self.peak.y),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recommendation_is_staged_and_preserves_continuous_targeting() {
        let mut measurement = NeutralMeasurement::default();
        let mut result = None;
        for _ in 0..200 {
            if result.is_none() {
                result = measurement.sample(1.0 / 60.0, Vec2::X * 0.025, Vec2::Y * 0.045);
            }
        }
        let suggestion = result.unwrap().unwrap();
        let mut settings = ClientInputSettings::default();
        assert!(settings.shape_targeting_axis(Vec2::Y * 0.045).is_some());
        suggestion.apply(&mut settings);
        assert!(settings.validate().is_ok());
        assert!(settings.shape_targeting_axis(Vec2::Y * 0.045).is_none());
        let near = settings
            .shape_targeting_axis(Vec2::X * (suggestion.aim + 0.002))
            .unwrap();
        assert!(near.length() > 0.0 && near.length() < 0.01);
        assert!((settings.shape_targeting_axis(Vec2::X).unwrap().length() - 1.0).abs() < 0.0001);
    }
    #[test]
    fn intentional_motion_and_invalid_samples_never_produce_a_calibration() {
        for input in [Vec2::X * 0.8, Vec2::new(f32::NAN, 0.0)] {
            let mut measurement = NeutralMeasurement::default();
            let mut result = None;
            for _ in 0..200 {
                if result.is_none() {
                    result = measurement.sample(1.0 / 60.0, Vec2::ZERO, input);
                }
            }
            assert!(result.unwrap().is_err());
        }
    }
}
