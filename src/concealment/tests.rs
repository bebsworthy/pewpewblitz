use super::*;

#[test]
fn distance_equality_reveals_and_defeated_observer_has_no_enemy_permission() {
    let input = |distance_squared, observer_alive| ObserverVisibilityInput {
        relation: ObserverRelation::Enemy,
        observer_alive,
        concealment: ConcealmentSources::Terrain,
        forced_revealed: false,
        subject_reveal_locked: false,
        distance_squared,
        reveal_radius: 160.0,
    };
    assert!(observer_can_see(input(160.0_f32.powi(2), true)));
    assert!(!observer_can_see(input(160.01_f32.powi(2), true)));
    assert!(!observer_can_see(input(0.0, false)));
    assert!(observer_can_see(ObserverVisibilityInput {
        relation: ObserverRelation::SelfOrAlly,
        distance_squared: f32::INFINITY,
        ..input(0.0, false)
    }));
}

#[test]
fn reveal_deadline_end_tick_is_exclusive() {
    let deadlines = ConcealmentRevealDeadlines {
        attack_until_tick: 12,
        damage_until_tick: 10,
    };
    assert!(reveal_lock_active(11, deadlines));
    assert!(!reveal_lock_active(12, deadlines));
}

#[test]
fn self_cloak_ignores_proximity_but_team_reveal_overrides_it_for_live_observers() {
    let cloaked = ObserverVisibilityInput {
        relation: ObserverRelation::Enemy,
        observer_alive: true,
        concealment: ConcealmentSources::SelfCloak,
        forced_revealed: false,
        subject_reveal_locked: false,
        distance_squared: 0.0,
        reveal_radius: 160.0,
    };
    assert!(!observer_can_see(cloaked));
    assert!(observer_can_see(ObserverVisibilityInput {
        forced_revealed: true,
        distance_squared: f32::INFINITY,
        ..cloaked
    }));
    assert!(!observer_can_see(ObserverVisibilityInput {
        observer_alive: false,
        forced_revealed: true,
        subject_reveal_locked: true,
        ..cloaked
    }));
}

#[test]
fn forced_reveal_sources_refresh_without_stacking_and_remain_team_scoped() {
    let mut sources = ForcedRevealSources::default();
    let source = ForcedRevealSource {
        revealing_team: crate::combat::TeamId(1),
        source_network_id: crate::protocol::NetworkEntityId(7),
        source_generation: 2,
        applied_at_tick: 10,
        expires_at_tick: 20,
    };
    assert!(sources.apply(source));
    assert!(sources.apply(ForcedRevealSource {
        expires_at_tick: 25,
        ..source
    }));
    assert_eq!(sources.0.len(), 1);
    assert!(sources.apply(ForcedRevealSource {
        source_network_id: crate::protocol::NetworkEntityId(8),
        source_generation: 1,
        expires_at_tick: 22,
        ..source
    }));
    assert_eq!(sources.0.len(), 2);
    assert!(sources.active_for_team(crate::combat::TeamId(1), 24));
    assert!(!sources.active_for_team(crate::combat::TeamId(1), 25));
    assert!(!sources.active_for_team(crate::combat::TeamId(2), 24));
}

#[cfg(feature = "server")]
#[test]
fn observer_decision_is_scheduled_after_outcomes_and_before_cue_filtering() {
    use crate::{abilities::AbilitySet, combat::CombatSet};
    use bevy::{prelude::*, time::TimeUpdateStrategy};

    #[derive(Resource, Default)]
    struct Trace(Vec<&'static str>);

    fn observe(mut trace: ResMut<Trace>) {
        trace.0.push("outcomes");
    }
    fn resolve(mut trace: ResMut<Trace>) {
        trace.0.push("sources");
    }
    fn decide(mut trace: ResMut<Trace>) {
        trace.0.push("observers");
    }
    fn cues(mut trace: ResMut<Trace>) {
        trace.0.push("cues");
    }

    let mut app = App::new();
    app.add_plugins((MinimalPlugins, crate::gameplay::GameplayPlugin))
        .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
        .init_resource::<Trace>();
    app.configure_sets(
        FixedPostUpdate,
        AbilitySet::ObserveOutcomes.after(CombatSet::Damage),
    );
    server::configure_concealment_schedule(&mut app);
    app.add_systems(
        FixedPostUpdate,
        (
            observe.in_set(AbilitySet::ObserveOutcomes),
            resolve.in_set(ConcealmentSet::ResolveSources),
            decide.in_set(ConcealmentSet::DecideObservers),
            cues.in_set(CombatSet::TelemetryAndCues),
        ),
    );

    app.update();
    app.update();
    assert_eq!(
        app.world().resource::<Trace>().0,
        vec!["outcomes", "sources", "observers", "cues"]
    );
}
