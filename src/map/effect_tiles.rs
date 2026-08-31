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
    pub(crate) const fn capabilities(self) -> EffectTileCapabilities {
        match self {
            Self::None => EffectTileCapabilities::NONE,
            Self::Speed {
                movement_multiplier_milli,
            } => EffectTileCapabilities {
                movement: Some(MovementTileEffect {
                    multiplier_milli: movement_multiplier_milli,
                }),
                periodic_damage: None,
                blocks_healing: false,
                traversal: EffectTileTraversal::Movement,
                spawn_clearance: EffectTileSpawnClearance::OccupiedCell,
                presentation: Some(EffectTilePresentation::Speed),
            },
            Self::Slow {
                movement_multiplier_milli,
            } => EffectTileCapabilities {
                movement: Some(MovementTileEffect {
                    multiplier_milli: movement_multiplier_milli,
                }),
                periodic_damage: None,
                blocks_healing: false,
                traversal: EffectTileTraversal::Movement,
                spawn_clearance: EffectTileSpawnClearance::OccupiedCell,
                presentation: Some(EffectTilePresentation::Slow),
            },
            Self::Damage {
                damage,
                interval_ticks,
            } => EffectTileCapabilities {
                movement: None,
                periodic_damage: Some(PeriodicDamageTileEffect {
                    damage,
                    interval_ticks,
                }),
                blocks_healing: true,
                traversal: EffectTileTraversal::Hazard,
                spawn_clearance: EffectTileSpawnClearance::AdjacentCells,
                presentation: Some(EffectTilePresentation::Damage),
            },
        }
    }

    #[must_use]
    pub const fn kind(self) -> Option<EffectTileKind> {
        match self.capabilities().presentation {
            Some(presentation) => Some(presentation.kind()),
            None => None,
        }
    }

    #[must_use]
    pub const fn movement_multiplier_milli(self) -> u16 {
        self.capabilities().movement_multiplier_milli()
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
pub(crate) struct MovementTileEffect {
    pub multiplier_milli: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PeriodicDamageTileEffect {
    pub damage: u16,
    pub interval_ticks: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum EffectTileTraversal {
    #[default]
    Neutral,
    Movement,
    Hazard,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum EffectTileSpawnClearance {
    #[default]
    None,
    OccupiedCell,
    AdjacentCells,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EffectTilePresentation {
    Speed,
    Slow,
    Damage,
}

impl EffectTilePresentation {
    const fn kind(self) -> EffectTileKind {
        match self {
            Self::Speed => EffectTileKind::Speed,
            Self::Slow => EffectTileKind::Slow,
            Self::Damage => EffectTileKind::Damage,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct EffectTileCapabilities {
    pub movement: Option<MovementTileEffect>,
    pub periodic_damage: Option<PeriodicDamageTileEffect>,
    pub blocks_healing: bool,
    pub traversal: EffectTileTraversal,
    pub spawn_clearance: EffectTileSpawnClearance,
    pub presentation: Option<EffectTilePresentation>,
}

impl EffectTileCapabilities {
    pub const NONE: Self = Self {
        movement: None,
        periodic_damage: None,
        blocks_healing: false,
        traversal: EffectTileTraversal::Neutral,
        spawn_clearance: EffectTileSpawnClearance::None,
        presentation: None,
    };

    #[must_use]
    pub const fn is_effect_tile(self) -> bool {
        !matches!(self.spawn_clearance, EffectTileSpawnClearance::None)
    }

    #[must_use]
    pub const fn movement_multiplier_milli(self) -> u16 {
        match self.movement {
            Some(effect) => effect.multiplier_milli,
            None => 1_000,
        }
    }

    #[must_use]
    #[cfg(any(feature = "server", test))]
    pub(crate) const fn traversal_cost_milli(self, hazard_cost_milli: u16) -> u16 {
        match self.traversal {
            EffectTileTraversal::Neutral => 1_000,
            EffectTileTraversal::Movement => {
                movement_terrain_cost_milli(self.movement_multiplier_milli())
            }
            EffectTileTraversal::Hazard => hazard_cost_milli,
        }
    }

    #[must_use]
    pub(crate) const fn violates_spawn_clearance(self, tile: MapCell, spawn: MapCell) -> bool {
        let dx = tile.x.abs_diff(spawn.x);
        let dy = tile.y.abs_diff(spawn.y);
        match self.spawn_clearance {
            EffectTileSpawnClearance::None => false,
            EffectTileSpawnClearance::OccupiedCell => tile.x == spawn.x && tile.y == spawn.y,
            EffectTileSpawnClearance::AdjacentCells => dx <= 1 && dy <= 1,
        }
    }
}

#[cfg(any(feature = "server", test))]
#[allow(
    clippy::cast_possible_truncation,
    reason = "the explicit upper-bound branch proves the reciprocal fits in u16"
)]
const fn movement_terrain_cost_milli(multiplier_milli: u16) -> u16 {
    let multiplier = if multiplier_milli == 0 {
        1
    } else {
        multiplier_milli as u32
    };
    let reciprocal = 1_000_000_u32.div_ceil(multiplier);
    if reciprocal > u16::MAX as u32 {
        u16::MAX
    } else {
        reciprocal as u16
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedEffectTile {
    pub placement_id: MapPlacementId,
    pub cell: MapCell,
    pub behavior: MapEffectTileBehavior,
}

impl ResolvedEffectTile {
    #[must_use]
    #[cfg(feature = "server")]
    pub(crate) const fn capabilities(self) -> EffectTileCapabilities {
        self.behavior.capabilities()
    }
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
    #[must_use]
    pub(crate) const fn capabilities(&self) -> EffectTileCapabilities {
        self.behavior.capabilities()
    }

    /// Whether this authoritative occupancy suppresses positive-health gameplay effects.
    #[must_use]
    pub const fn blocks_healing(&self) -> bool {
        self.capabilities().blocks_healing
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

    #[test]
    fn composite_capabilities_remain_orthogonal_for_every_consumer() {
        let capabilities = EffectTileCapabilities {
            movement: Some(MovementTileEffect {
                multiplier_milli: 1_250,
            }),
            periodic_damage: Some(PeriodicDamageTileEffect {
                damage: 7,
                interval_ticks: 45,
            }),
            blocks_healing: true,
            traversal: EffectTileTraversal::Hazard,
            spawn_clearance: EffectTileSpawnClearance::AdjacentCells,
            presentation: Some(EffectTilePresentation::Damage),
        };

        assert_eq!(capabilities.movement_multiplier_milli(), 1_250);
        assert_eq!(
            capabilities.periodic_damage,
            Some(PeriodicDamageTileEffect {
                damage: 7,
                interval_ticks: 45,
            })
        );
        assert!(capabilities.blocks_healing);
        assert_eq!(capabilities.traversal_cost_milli(3_500), 3_500);
        assert!(capabilities.violates_spawn_clearance(MapCell::new(4, 4), MapCell::new(5, 5)));
        assert_eq!(
            capabilities.presentation,
            Some(EffectTilePresentation::Damage)
        );
    }
}
