//! Healing numbers for fighter health recovered outside healing combat cues.
//!
//! Restoration pickups already send an authoritative applied amount in `PickupCue::Collected`, so
//! their `+N` comes straight from that cue. Passive regeneration and Zombie boosts have no cue with
//! an amount; they are derived from increases of the replicated `CurrentHealth` of fighters
//! replicated to this observer. Every increase is first matched against healing already numbered
//! from a cue (healing `EffectApplied`, pickup collection) for the same fighter, and only the
//! unexplained remainder is held for a short attribution window so a cue that arrives after the
//! replicated health still claims it. Increases caused by lifecycle resets (respawn, match
//! restart) are dropped, and decreases never produce a number, so damage is never misattributed.

use super::{
    CombatCue, CombatEffectCue, DeduplicatedCombatCue, Fighter, FloatingNumberBook, HashMap,
    HitFeedbackCatalog, HitFeedbackRoots, HitStyle, NetworkEntityId,
};
use bevy::prelude::*;
use brawler::combat::{CurrentHealth, Defeated};
use brawler::matchplay::{MatchPhase, MatchState, RespawnState, SpawnProtection};

/// How long an increase no cue explains waits for a late healing cue before it becomes a
/// recovery number. Code-owned network-ordering tolerance (cues and replicated components travel
/// separately), not player-facing tuning.
const UNEXPLAINED_HOLD_SECONDS: f32 = 0.25;
/// How long cue healing waits for the matching replicated health increase. Longer than the hold
/// above: swallowing a little regeneration is preferable to showing one heal twice.
const EXPLAINED_WINDOW_SECONDS: f32 = 1.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct RecoveryTrack {
    last_health: Option<u16>,
    /// Cue healing already numbered but not yet seen in replicated health.
    explained: u32,
    explained_until: f32,
    /// Replicated increase no cue explains yet.
    pending: u32,
    pending_since: f32,
}

impl RecoveryTrack {
    fn expire_explained(&mut self, now: f32) {
        if now > self.explained_until {
            self.explained = 0;
        }
    }
}

/// Presentation-only attribution of replicated fighter health increases; never gameplay truth.
/// Bounded by the fighters replicated to this observer.
#[derive(Resource, Default, Debug)]
pub(super) struct HealthRecoveryLedger {
    tracks: HashMap<Entity, RecoveryTrack>,
}

impl HealthRecoveryLedger {
    /// Healing already numbered from an authoritative cue for `subject`.
    pub(super) fn explain(&mut self, subject: Entity, amount: u16, now: f32) {
        let track = self.tracks.entry(subject).or_default();
        let amount = u32::from(amount);
        // The replicated increase arrived first: the cue claims it.
        let cancelled = amount.min(track.pending);
        track.pending -= cancelled;
        let rest = amount - cancelled;
        if rest > 0 {
            track.expire_explained(now);
            track.explained = track.explained.saturating_add(rest);
            track.explained_until = now + EXPLAINED_WINDOW_SECONDS;
        }
    }

    /// One frame's replicated health of `subject`. `resetting` marks lifecycle state in which an
    /// increase is a reset rather than healing.
    pub(super) fn observe(&mut self, subject: Entity, health: u16, resetting: bool, now: f32) {
        let track = self.tracks.entry(subject).or_default();
        let previous = track.last_health.replace(health);
        track.expire_explained(now);
        if resetting || previous == Some(0) {
            track.pending = 0;
            track.explained = 0;
            return;
        }
        let Some(previous) = previous else { return };
        if health <= previous {
            return;
        }
        let gain = u32::from(health - previous);
        let claimed = gain.min(track.explained);
        track.explained -= claimed;
        let rest = gain - claimed;
        if rest > 0 {
            if track.pending == 0 {
                track.pending_since = now;
            }
            track.pending = track.pending.saturating_add(rest);
        }
    }

    /// Unexplained recovery whose attribution window has passed, in a stable subject order.
    pub(super) fn take_due(&mut self, now: f32) -> Vec<(Entity, u16)> {
        let mut due: Vec<_> = self
            .tracks
            .iter_mut()
            .filter(|(_, track)| {
                track.pending > 0 && now - track.pending_since >= UNEXPLAINED_HOLD_SECONDS
            })
            .map(|(subject, track)| {
                let amount = u16::try_from(track.pending).unwrap_or(u16::MAX);
                track.pending = 0;
                (*subject, amount)
            })
            .collect();
        due.sort_by_key(|(subject, _)| *subject);
        due
    }

    /// Forget fighters that are no longer replicated to this observer.
    pub(super) fn retain(&mut self, live: impl Fn(Entity) -> bool) {
        self.tracks.retain(|subject, _| live(*subject));
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.tracks.len()
    }
}

type RecoveryFighters<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static NetworkEntityId,
        &'static CurrentHealth,
        Has<Defeated>,
        Has<RespawnState>,
        Has<SpawnProtection>,
    ),
    With<Fighter>,
>;

/// Shows `+N` for pickup collection and for replicated fighter health recovery no healing cue
/// explains, through the shared floating-number pool.
#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "the system reconciles two authoritative cue streams with replicated health for one presentation channel"
)]
pub(super) fn collect_recovery_numbers(
    time: Res<Time>,
    catalog: Res<HitFeedbackCatalog>,
    mut book: ResMut<FloatingNumberBook>,
    mut ledger: ResMut<HealthRecoveryLedger>,
    mut cues: MessageReader<DeduplicatedCombatCue>,
    pickups: Option<Res<brawler::map::ReceivedPickupCues>>,
    match_states: Query<&MatchState>,
    fighters: RecoveryFighters,
    roots: HitFeedbackRoots,
) {
    let now = time.elapsed_secs();
    let by_network_id: HashMap<u64, Entity> = fighters
        .iter()
        .map(|(entity, id, ..)| (id.0, entity))
        .collect();
    // Healing cues are numbered by `collect_hit_feedback`; they only explain increases here.
    for DeduplicatedCombatCue(cue) in cues.read() {
        if let CombatCue::EffectApplied {
            target,
            effect: CombatEffectCue::Healing { amount, .. },
            ..
        } = cue
            && let Some(subject) = by_network_id.get(&target.0)
        {
            ledger.explain(*subject, *amount, now);
        }
    }
    let mut numbers = Vec::new();
    for cue in pickups.iter().flat_map(|pickups| pickups.0.iter()) {
        // A collector concealed from this observer arrives as `None` and stays anonymous.
        let brawler::map::PickupCue::Collected {
            collector: Some(collector),
            applied_restoration,
            position,
            ..
        } = *cue
        else {
            continue;
        };
        let Some(subject) = by_network_id.get(&collector.0).copied() else {
            continue;
        };
        if applied_restoration == 0 {
            continue;
        }
        ledger.explain(subject, applied_restoration, now);
        numbers.push((subject, applied_restoration, Some(position.as_vec2())));
    }
    let match_inactive = match_states
        .iter()
        .next()
        .is_some_and(|state| !matches!(state.phase, MatchPhase::Active { .. }));
    for (subject, _, health, defeated, respawning, protected) in &fighters {
        let resetting = match_inactive || defeated || respawning || protected;
        ledger.observe(subject, health.0, resetting, now);
    }
    ledger.retain(|subject| fighters.contains(subject));
    for (subject, amount) in ledger.take_due(now) {
        numbers.push((subject, amount, None));
    }
    for (subject, amount, cue_position) in numbers {
        let subject_roots = roots.roots(subject);
        // Recovery without a cue position needs the fighter's presentation root to anchor on.
        if cue_position.is_none() && subject_roots.is_empty() {
            continue;
        }
        let anchor = roots.anchor(&subject_roots, cue_position.unwrap_or_default());
        book.push(subject, HitStyle::Healing, amount, anchor, &catalog.numbers);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subject(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("valid test entity index")
    }

    #[test]
    fn unexplained_increases_wait_for_the_hold_then_become_one_recovery_number() {
        let mut ledger = HealthRecoveryLedger::default();
        let fighter = subject(1);
        ledger.observe(fighter, 50, false, 0.0);
        ledger.observe(fighter, 51, false, 0.1);
        ledger.observe(fighter, 53, false, 0.2);
        assert!(ledger.take_due(0.3).is_empty(), "still inside the hold");
        assert_eq!(ledger.take_due(0.35), vec![(fighter, 3)]);
        assert!(ledger.take_due(1.0).is_empty(), "taken exactly once");
    }

    #[test]
    fn cue_healing_claims_its_increase_in_either_arrival_order() {
        let mut ledger = HealthRecoveryLedger::default();
        let early_cue = subject(1);
        ledger.observe(early_cue, 40, false, 0.0);
        ledger.explain(early_cue, 20, 0.1);
        ledger.observe(early_cue, 62, false, 0.15);
        let late_cue = subject(2);
        ledger.observe(late_cue, 40, false, 0.0);
        ledger.observe(late_cue, 61, false, 0.1);
        ledger.explain(late_cue, 20, 0.2);
        // Only the unexplained regeneration on top of each heal remains.
        let mut due = ledger.take_due(1.0);
        due.sort_by_key(|(subject, _)| subject.index());
        assert_eq!(due, vec![(early_cue, 2), (late_cue, 1)]);
    }

    #[test]
    fn unmatched_cue_healing_expires_instead_of_swallowing_later_recovery() {
        let mut ledger = HealthRecoveryLedger::default();
        let fighter = subject(1);
        ledger.observe(fighter, 40, false, 0.0);
        // Healed 20 and damaged 20 in the same update: no visible increase.
        ledger.explain(fighter, 20, 0.1);
        ledger.observe(fighter, 40, false, 0.1);
        ledger.observe(fighter, 45, false, 2.0);
        assert_eq!(ledger.take_due(2.5), vec![(fighter, 5)]);
    }

    #[test]
    fn decreases_and_lifecycle_resets_never_become_healing() {
        let mut ledger = HealthRecoveryLedger::default();
        let fighter = subject(1);
        ledger.observe(fighter, 80, false, 0.0);
        ledger.observe(fighter, 30, false, 0.1);
        // Respawn from a defeat: previous health 0.
        ledger.observe(fighter, 0, false, 0.2);
        ledger.observe(fighter, 100, false, 0.3);
        // Restart reset arriving before the protection/phase signal: cleared when it lands.
        ledger.observe(fighter, 40, false, 0.4);
        ledger.observe(fighter, 100, false, 0.5);
        ledger.observe(fighter, 100, true, 0.6);
        // A reset seen together with the increase.
        ledger.observe(fighter, 20, false, 0.7);
        ledger.observe(fighter, 100, true, 0.8);
        assert!(ledger.take_due(5.0).is_empty());
        // The first observation of a newly replicated fighter is only a baseline.
        ledger.observe(subject(2), 90, false, 6.0);
        assert!(ledger.take_due(9.0).is_empty());
    }

    #[test]
    fn ledger_forgets_fighters_that_stop_replicating() {
        let mut ledger = HealthRecoveryLedger::default();
        ledger.observe(subject(1), 10, false, 0.0);
        ledger.observe(subject(2), 10, false, 0.0);
        ledger.retain(|entity| entity == subject(2));
        assert_eq!(ledger.len(), 1);
    }
}
