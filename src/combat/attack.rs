//! Shared attack economy and deterministic delivery identity helpers.

use super::{AttackId, WeaponEconomy};

#[must_use]
pub fn delivery_count(firing: super::FiringPattern) -> u8 {
    match firing {
        super::FiringPattern::Single => 1,
        super::FiringPattern::Spread { delivery_count, .. } => delivery_count,
    }
}

#[must_use]
pub fn economy_ready(resource: u8, phase_ready: bool) -> bool {
    phase_ready && resource > 0
}

#[must_use]
pub fn refill_deadline(current_tick: u64, economy: WeaponEconomy) -> u64 {
    current_tick.saturating_add(economy.refill_ticks())
}

#[must_use]
pub fn delivery_key(attack_id: AttackId, delivery_index: u8) -> (u64, u8) {
    (attack_id.0, delivery_index)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delivery_indices_are_stable_within_one_attack() {
        assert_eq!(delivery_key(AttackId(7), 3), (7, 3));
        assert_eq!(delivery_count(super::super::FiringPattern::Single), 1);
    }
}
