//! Shared server-owned boost pickups for the modes that grant per-life boost stacks (BRL-5ss9).
//!
//! Zombie and Last Stand share, unchanged: the match-scoped pickup ID allocator, the replicated
//! pickup spawn, once-per-match detection of destroyed treasure chests, and the collection
//! transaction (nearest-by-network-ID individual grant, or every eligible teammate for a team
//! grant). Each mode keeps its own drop policy: which chests grant which scope, ambient Zombie
//! drops, and Last Stand death drops.

use super::{
    BOOST_PICKUP_NETWORK_NAMESPACE, BoostGrantScope, BoostPickup, BoostPickupAvailableAtTick,
    BoostPickupIdentity, BoostPickupSource, BoostStacks, MatchId, MatchParticipant, MatchPhase,
    MatchRoot, MatchState, effective_maximum_health,
};
use crate::{
    combat::{CurrentHealth, Defeated},
    map::{MapDynamicState, MapPlacementId, ResolvedMap},
    matchplay::ActiveCombatant,
    protocol::{Fighter, NetworkEntityId},
    timing::SimulationTick,
};
use avian2d::prelude::Position;
use bevy::prelude::*;
use lightyear::prelude::{NetworkTarget, Replicate};
use std::collections::{BTreeMap, BTreeSet};

/// The installed mode's collection reach and per-life stack ceiling.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub(crate) struct BoostPickupRules {
    pub(crate) pickup_radius: f32,
    pub(crate) stack_limit: u8,
}

/// Match-scoped pickup identity allocator; restart clears it.
#[derive(Resource, Default)]
pub(crate) struct NextBoostPickupId(u32);

impl NextBoostPickupId {
    fn next(&mut self) -> Option<u32> {
        self.0 = self.0.checked_add(1)?;
        Some(self.0)
    }

    pub(crate) fn clear(&mut self) {
        self.0 = 0;
    }
}

/// Treasure chests whose terminal transition already produced a pickup this match.
#[derive(Resource, Default)]
pub(crate) struct ProcessedChests {
    match_id: Option<MatchId>,
    placements: BTreeSet<MapPlacementId>,
}

impl ProcessedChests {
    /// Chests destroyed since the last call this match, in placement order, with their
    /// cell-centre positions. Each chest is reported once per match.
    pub(crate) fn newly_destroyed(
        &mut self,
        match_id: MatchId,
        resolved: &ResolvedMap,
        map_state: Option<&MapDynamicState>,
    ) -> Vec<(MapPlacementId, Vec2)> {
        if self.match_id != Some(match_id) {
            self.match_id = Some(match_id);
            self.placements.clear();
        }
        let Some(map_state) = map_state else {
            return Vec::new();
        };
        let terminal = map_state
            .terminal_states
            .iter()
            .map(|transition| transition.placement_id)
            .collect::<BTreeSet<_>>();
        resolved
            .snapshot
            .placements
            .iter()
            .filter(|placement| {
                placement.asset_id == crate::map::TREASURE_CHEST_ASSET
                    && terminal.contains(&placement.placement_id)
                    && self.placements.insert(placement.placement_id)
            })
            .map(|placement| {
                (
                    placement.placement_id,
                    resolved.snapshot.dimensions.cell_center(placement.cell),
                )
            })
            .collect()
    }
}

/// Spawn one replicated pickup; `false` when the match's pickup identities are exhausted.
pub(crate) fn spawn_boost_pickup(
    commands: &mut Commands,
    ids: &mut NextBoostPickupId,
    match_id: MatchId,
    source: BoostPickupSource,
    scope: BoostGrantScope,
    position: Vec2,
    tick: u64,
) -> bool {
    let Some(pickup_id) = ids.next() else {
        return false;
    };
    let network_id = NetworkEntityId(BOOST_PICKUP_NETWORK_NAMESPACE | u64::from(pickup_id));
    commands.spawn((
        BoostPickup,
        BoostPickupIdentity {
            match_id,
            pickup_id,
        },
        source,
        scope,
        BoostPickupAvailableAtTick(tick.saturating_add(1)),
        network_id,
        Position::from_xy(position.x, position.y),
        Transform::from_translation(position.extend(0.0)),
        Replicate::to_clients(NetworkTarget::All),
    ));
    true
}

/// Raise one fighter's stacks by one up to `limit`, adding the gained maximum health.
fn grant_boost_stack(
    health: &mut CurrentHealth,
    stacks: &mut BoostStacks,
    base_maximum_health: u16,
    limit: u8,
) {
    let previous_maximum = effective_maximum_health(base_maximum_health, *stacks);
    stacks.0 = stacks.0.saturating_add(1).min(limit);
    let new_maximum = effective_maximum_health(base_maximum_health, *stacks);
    health.0 = health
        .0
        .saturating_add(new_maximum.saturating_sub(previous_maximum))
        .min(new_maximum);
}

#[allow(
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "the Bevy system parameters declare the complete deterministic pickup transaction view"
)]
pub(crate) fn collect_boost_pickups(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    rules: Res<BoostPickupRules>,
    roots: Query<&MatchState, With<MatchRoot>>,
    pickups: Query<
        (
            Entity,
            &BoostPickupIdentity,
            &BoostGrantScope,
            &BoostPickupAvailableAtTick,
            &Position,
        ),
        With<BoostPickup>,
    >,
    mut fighters: ParamSet<(
        Query<
            (
                Entity,
                &NetworkEntityId,
                &Position,
                &BoostStacks,
                &MatchParticipant,
            ),
            (With<Fighter>, With<ActiveCombatant>, Without<Defeated>),
        >,
        Query<
            (
                &mut CurrentHealth,
                &mut BoostStacks,
                &crate::builds::ResolvedFighterStats,
                &MatchParticipant,
            ),
            With<Fighter>,
        >,
    )>,
) {
    let Ok(state) = roots.single() else { return };
    if !matches!(state.phase, MatchPhase::Active { .. }) {
        return;
    }
    let mut ordered = pickups.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|(_, identity, ..)| identity.pickup_id);
    for (pickup_entity, identity, scope, available, position) in ordered {
        if identity.match_id != state.match_id || tick.0 < available.0 {
            continue;
        }
        let (nearby, recipients_to_apply) = {
            let candidates = fighters.p0();
            let mut nearby = candidates
                .iter()
                .filter(|(_, _, candidate_position, _, participant)| {
                    participant.match_id == state.match_id
                        && candidate_position.0.distance_squared(position.0)
                            <= rules.pickup_radius.powi(2)
                })
                .map(|(entity, network_id, _, stacks, _)| (network_id.0, entity, stacks.0))
                .collect::<Vec<_>>();
            nearby.sort_by_key(|candidate| candidate.0);
            let recipients = match scope {
                BoostGrantScope::Individual => nearby
                    .iter()
                    .find(|candidate| candidate.2 < rules.stack_limit)
                    .map(|candidate| vec![candidate.1])
                    .unwrap_or_default(),
                BoostGrantScope::Team => candidates
                    .iter()
                    .filter(|(_, _, _, stacks, participant)| {
                        participant.match_id == state.match_id && stacks.0 < rules.stack_limit
                    })
                    .map(|(entity, network_id, ..)| (network_id.0, entity))
                    .collect::<BTreeMap<_, _>>()
                    .into_values()
                    .collect(),
            };
            (nearby, recipients)
        };
        if nearby.is_empty() || recipients_to_apply.is_empty() {
            continue;
        }
        let mut recipients = fighters.p1();
        for entity in recipients_to_apply {
            let Ok((mut health, mut stacks, fighter_stats, participant)) =
                recipients.get_mut(entity)
            else {
                continue;
            };
            if participant.match_id != state.match_id || stacks.0 >= rules.stack_limit {
                continue;
            }
            grant_boost_stack(
                &mut health,
                &mut stacks,
                fighter_stats.maximum_health,
                rules.stack_limit,
            );
        }
        commands.entity(pickup_entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_granted_stack_adds_the_gained_maximum_and_stops_at_the_limit() {
        let mut health = CurrentHealth(500);
        let mut stacks = BoostStacks(0);
        grant_boost_stack(&mut health, &mut stacks, 1_000, 2);
        assert_eq!((health.0, stacks.0), (550, 1));
        grant_boost_stack(&mut health, &mut stacks, 1_000, 2);
        grant_boost_stack(&mut health, &mut stacks, 1_000, 2);
        assert_eq!((health.0, stacks.0), (600, 2));
    }
}
