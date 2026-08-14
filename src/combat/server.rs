//! Server-only combat identity reservation and lifecycle limits.

use super::{AttackId, CombatEventId, NextCombatIds};

pub(super) const MAX_ACTIVE_ATTACK_TRACKERS: usize = 512;

pub(super) fn reserve_event_ids(
    ids: &mut NextCombatIds,
    count: usize,
) -> Option<Vec<CombatEventId>> {
    let count = u64::try_from(count).ok()?;
    let first = ids.next_event_id;
    let next = first.checked_add(count)?;
    ids.next_event_id = next;
    Some(
        (0..count)
            .map(|offset| CombatEventId(first + offset))
            .collect(),
    )
}

pub(super) fn reserve_attack_and_events(
    ids: &mut NextCombatIds,
    event_count: usize,
) -> Option<(AttackId, Vec<CombatEventId>)> {
    let previous_attack_id = ids.next_attack_id;
    let attack_id = ids.allocate_attack()?;
    let Some(events) = reserve_event_ids(ids, event_count) else {
        ids.next_attack_id = previous_attack_id;
        return None;
    };
    Some((attack_id, events))
}
