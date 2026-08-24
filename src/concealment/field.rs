use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub const MAX_ACTIVE_CONCEALMENT_FIELDS: usize = brawler_routing::MAX_PARTICIPANTS;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct ConcealmentFieldId(pub u64);

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct ConcealmentFieldState {
    pub id: ConcealmentFieldId,
    pub team: crate::combat::TeamId,
    pub center: crate::combat::WorldPoint,
    pub radius_milliunits: u32,
    pub activated_at_tick: u64,
    pub expires_at_tick: u64,
}

impl ConcealmentFieldState {
    #[must_use]
    pub fn center_vec2(self) -> Vec2 {
        self.center.as_vec2()
    }

    #[must_use]
    pub fn radius(self) -> Option<f32> {
        crate::builds::world_units_from_milliunits(self.radius_milliunits)
    }
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObjectiveCarrier;

#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct AlliedConcealmentMemberships(pub Vec<ConcealmentFieldId>);

impl AlliedConcealmentMemberships {
    #[must_use]
    pub fn bounded(mut ids: Vec<ConcealmentFieldId>) -> Option<Self> {
        ids.sort_unstable();
        ids.dedup();
        (ids.len() <= MAX_ACTIVE_CONCEALMENT_FIELDS).then_some(Self(ids))
    }
}

#[cfg(feature = "server")]
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ConcealmentFieldOwner {
    pub owner_network_id: crate::protocol::NetworkEntityId,
    pub owner_generation: u64,
    pub match_id: crate::matchplay::MatchId,
}

#[cfg(feature = "server")]
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NextConcealmentFieldId(pub u64);

#[cfg(feature = "server")]
impl Default for NextConcealmentFieldId {
    fn default() -> Self {
        Self(1)
    }
}

#[cfg(feature = "server")]
impl NextConcealmentFieldId {
    pub(crate) fn allocate(&mut self) -> Option<ConcealmentFieldId> {
        let id = self.0;
        self.0 = id.checked_add(1)?;
        Some(ConcealmentFieldId(id))
    }
}

#[must_use]
pub fn field_contains(center: Vec2, radius: f32, fighter: Vec2) -> bool {
    center.is_finite()
        && fighter.is_finite()
        && radius.is_finite()
        && radius > 0.0
        && center.distance_squared(fighter) <= radius * radius
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_boundary_is_inclusive_and_non_finite_fails_closed() {
        assert!(field_contains(Vec2::ZERO, 192.0, Vec2::X * 192.0));
        assert!(!field_contains(Vec2::ZERO, 192.0, Vec2::X * 192.001));
        assert!(!field_contains(Vec2::ZERO, f32::NAN, Vec2::ZERO));
    }

    #[test]
    fn memberships_are_sorted_deduplicated_and_bounded() {
        assert_eq!(
            AlliedConcealmentMemberships::bounded(vec![
                ConcealmentFieldId(2),
                ConcealmentFieldId(1),
                ConcealmentFieldId(2),
            ])
            .unwrap()
            .0,
            vec![ConcealmentFieldId(1), ConcealmentFieldId(2)]
        );
        assert!(
            AlliedConcealmentMemberships::bounded(
                (0..=MAX_ACTIVE_CONCEALMENT_FIELDS)
                    .map(|id| ConcealmentFieldId(u64::try_from(id).unwrap()))
                    .collect(),
            )
            .is_none()
        );
    }
}
