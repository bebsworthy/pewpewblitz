//! Authored effect-tile behavior and replicated fighter occupancy.

use super::{MapCell, MapDynamicGeneration, MapPlacementId};
use bevy::prelude::Component;
use serde::{Deserialize, Serialize};

pub const MAX_EFFECT_TILE_PLACEMENTS: usize = 4_096;
pub(crate) const MAX_EFFECT_TILE_MOVEMENT_MULTIPLIER_MILLI: u16 = 2_000;
pub(crate) const MIN_SLOW_TILE_MOVEMENT_MULTIPLIER_MILLI: u16 = 100;
pub(crate) const MAX_EFFECT_TILE_DAMAGE: u16 = 100;
pub(crate) const MIN_EFFECT_TILE_INTERVAL_TICKS: u16 = 6;
pub(crate) const MAX_EFFECT_TILE_INTERVAL_TICKS: u16 = 600;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MapEffectTileBehavior {
    #[default]
    None,
    Speed {
        movement_multiplier_milli: u16,
    },
    Slow {
        movement_multiplier_milli: u16,
    },
    Damage {
        damage: u16,
        interval_ticks: u16,
    },
}

impl MapEffectTileBehavior {
    #[must_use]
    pub const fn kind(self) -> Option<EffectTileKind> {
        match self {
            Self::None => None,
            Self::Speed { .. } => Some(EffectTileKind::Speed),
            Self::Slow { .. } => Some(EffectTileKind::Slow),
            Self::Damage { .. } => Some(EffectTileKind::Damage),
        }
    }

    #[must_use]
    pub const fn movement_multiplier_milli(self) -> u16 {
        match self {
            Self::Speed {
                movement_multiplier_milli,
            }
            | Self::Slow {
                movement_multiplier_milli,
            } => movement_multiplier_milli,
            Self::None | Self::Damage { .. } => 1_000,
        }
    }

    pub(crate) fn validate(self) -> Result<(), String> {
        match self {
            Self::None
            | Self::Speed {
                movement_multiplier_milli: 1_001..=MAX_EFFECT_TILE_MOVEMENT_MULTIPLIER_MILLI,
            }
            | Self::Slow {
                movement_multiplier_milli: MIN_SLOW_TILE_MOVEMENT_MULTIPLIER_MILLI..=999,
            }
            | Self::Damage {
                damage: 1..=MAX_EFFECT_TILE_DAMAGE,
                interval_ticks: MIN_EFFECT_TILE_INTERVAL_TICKS..=MAX_EFFECT_TILE_INTERVAL_TICKS,
            } => Ok(()),
            _ => Err("effect tile behavior exceeds engine bounds".into()),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum EffectTileKind {
    Speed,
    Slow,
    Damage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedEffectTile {
    pub placement_id: MapPlacementId,
    pub cell: MapCell,
    pub behavior: MapEffectTileBehavior,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct EffectTileOccupancy {
    pub generation: MapDynamicGeneration,
    pub placement_id: MapPlacementId,
    pub behavior: MapEffectTileBehavior,
    pub entered_at_tick: u64,
    pub next_pulse_at_tick: Option<u64>,
}

impl EffectTileOccupancy {
    /// Whether this authoritative occupancy suppresses positive-health gameplay effects.
    #[must_use]
    pub const fn blocks_healing(&self) -> bool {
        matches!(self.behavior, MapEffectTileBehavior::Damage { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn occupancy(behavior: MapEffectTileBehavior) -> EffectTileOccupancy {
        EffectTileOccupancy {
            generation: MapDynamicGeneration {
                map_instance_id: crate::map::MapInstanceId(1),
                generation: 1,
            },
            placement_id: MapPlacementId(1),
            behavior,
            entered_at_tick: 0,
            next_pulse_at_tick: None,
        }
    }

    #[test]
    fn only_damage_occupancy_blocks_healing() {
        assert!(
            occupancy(MapEffectTileBehavior::Damage {
                damage: 10,
                interval_ticks: 30,
            })
            .blocks_healing()
        );
        assert!(
            !occupancy(MapEffectTileBehavior::Speed {
                movement_multiplier_milli: 1_250,
            })
            .blocks_healing()
        );
        assert!(
            !occupancy(MapEffectTileBehavior::Slow {
                movement_multiplier_milli: 700,
            })
            .blocks_healing()
        );
    }
}
