//! Mode-neutral authoritative combat facts consumed by game-mode rules and telemetry.

use super::{AttackId, CombatEventId, TeamId, WeaponPresetId, WeaponRecipeFingerprint, WorldPoint};
use crate::protocol::{NetworkEntityId, PlayerId};
use bevy::prelude::*;

/// One fixed tick cannot commit more accepted primary attacks than this authority buffer retains.
/// The ceiling is deliberately above the supported match and capacity-test fighter counts while
/// remaining a hard bound on transient server state.
pub const MAX_ACCEPTED_ATTACK_FACTS_PER_TICK: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AcceptedAttackFact {
    pub event_id: CombatEventId,
    pub tick: u64,
    pub attack_id: AttackId,
    pub source_network_id: NetworkEntityId,
}

#[derive(Resource, Default, Debug)]
pub struct AcceptedAttackFacts(pub Vec<AcceptedAttackFact>);

impl AcceptedAttackFacts {
    #[must_use]
    pub fn has_capacity(&self) -> bool {
        self.0.len() < MAX_ACCEPTED_ATTACK_FACTS_PER_TICK
    }

    pub fn record(&mut self, fact: AcceptedAttackFact) -> bool {
        if !self.has_capacity() {
            return false;
        }
        self.0.push(fact);
        true
    }
}

#[cfg(feature = "server")]
#[allow(clippy::needless_pass_by_value)]
fn clear_accepted_attack_facts(mut facts: ResMut<AcceptedAttackFacts>) {
    facts.0.clear();
}

#[cfg(feature = "server")]
pub(crate) fn register_accepted_attack_fact_lifecycle(app: &mut App) {
    app.init_resource::<AcceptedAttackFacts>().add_systems(
        FixedPostUpdate,
        clear_accepted_attack_facts
            .in_set(super::CombatSet::Finalize)
            .before(super::publish_authoritative_tick),
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombatOutcomeKind {
    ProtectedContact,
    Damage {
        amount: u16,
    },
    Healing {
        requested: u16,
        applied: u16,
        resulting_health: u16,
    },
    Defeat,
    DeployableDestroyed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombatTargetKind {
    Fighter,
    Deployable,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CombatOutcomeFact {
    pub event_id: CombatEventId,
    pub tick: u64,
    pub attack_id: AttackId,
    pub source_kind: super::CombatSourceKind,
    pub source_player: Option<PlayerId>,
    pub source_network_id: Option<NetworkEntityId>,
    pub source_team: Option<TeamId>,
    pub target_network_id: NetworkEntityId,
    pub target_kind: CombatTargetKind,
    pub target_team: TeamId,
    pub preset_id: Option<WeaponPresetId>,
    pub recipe_fingerprint: Option<WeaponRecipeFingerprint>,
    pub position: WorldPoint,
    pub engagement_distance: f32,
    pub kind: CombatOutcomeKind,
}

#[derive(Resource, Default, Debug)]
pub struct CombatOutcomeFacts(pub Vec<CombatOutcomeFact>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepted_attack_facts_enforce_the_per_tick_bound() {
        let mut facts = AcceptedAttackFacts::default();
        for value in 0..MAX_ACCEPTED_ATTACK_FACTS_PER_TICK {
            assert!(facts.record(AcceptedAttackFact {
                event_id: CombatEventId(value as u64),
                tick: 7,
                attack_id: AttackId(value as u64),
                source_network_id: NetworkEntityId(value as u64),
            }));
        }
        assert!(!facts.record(AcceptedAttackFact {
            event_id: CombatEventId(u64::MAX),
            tick: 7,
            attack_id: AttackId(u64::MAX),
            source_network_id: NetworkEntityId(u64::MAX),
        }));
        assert_eq!(facts.0.len(), MAX_ACCEPTED_ATTACK_FACTS_PER_TICK);
    }

    #[cfg(feature = "server")]
    #[test]
    fn accepted_attack_facts_clear_each_tick_without_matchplay() {
        use crate::gameplay::GameplayPlugin;
        use bevy::{prelude::MinimalPlugins, time::TimeUpdateStrategy};

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, GameplayPlugin))
            .insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
        register_accepted_attack_fact_lifecycle(&mut app);
        assert!(
            app.world_mut()
                .resource_mut::<AcceptedAttackFacts>()
                .record(AcceptedAttackFact {
                    event_id: CombatEventId(1),
                    tick: 0,
                    attack_id: AttackId(1),
                    source_network_id: NetworkEntityId(1),
                })
        );

        app.update();
        app.update();
        assert!(app.world().resource::<AcceptedAttackFacts>().0.is_empty());
        assert_eq!(app.world().resource::<crate::timing::SimulationTick>().0, 1);
    }
}
