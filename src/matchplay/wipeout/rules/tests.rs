use super::*;
use crate::{
    combat::{AttackId, CombatEventId, CombatOutcomeFact, CombatSourceKind},
    matchplay::{FighterDisplayName, MatchId, MatchParticipant},
    protocol::PlayerId,
};

fn fact(event: u64, source: Option<(u64, u8)>, kind: CombatOutcomeKind) -> CombatOutcomeFact {
    CombatOutcomeFact {
        event_id: CombatEventId(event),
        tick: 1,
        attack_id: AttackId(event),
        source_kind: CombatSourceKind::PrimaryWeapon,
        source_player: source.map(|_| PlayerId(1)),
        target_kind: crate::combat::CombatTargetKind::Fighter,
        preset_id: None,
        recipe_fingerprint: None,
        position: crate::combat::WorldPoint { x: 0., y: 0. },
        engagement_distance: 0.,
        source_network_id: source.map(|s| NetworkEntityId(s.0)),
        source_team: source.map(|s| TeamId(s.1)),
        target_network_id: NetworkEntityId(20),
        target_team: TeamId(1),
        kind,
    }
}
fn app() -> (App, Entity, Entity) {
    let mut app = App::new();
    app.insert_resource(SimulationTick(1))
        .init_resource::<WipeoutRules>()
        .init_resource::<CombatOutcomeFacts>()
        .init_resource::<ScoredCombatEvents>()
        .init_resource::<MatchOutcomeDiagnostics>()
        .init_resource::<ModeRuleOutcome>()
        .add_systems(
            Update,
            (
                statistics::remember_roster,
                resolve_wipeout_scoring,
                statistics::publish_completion,
            )
                .chain(),
        );
    let root = app
        .world_mut()
        .spawn((
            MatchRoot,
            MatchState {
                match_id: MatchId(1),
                mode_definition_id: WIPEOUT_MODE_DEFINITION,
                phase: MatchPhase::Active { ends_at_tick: 100 },
                rules_revision: WIPEOUT_RULES_REVISION,
            },
            WipeoutState {
                target_score: 10,
                ..default()
            },
        ))
        .id();
    let mut source = Entity::PLACEHOLDER;
    for (player, network, team) in [(1, 10, 0), (2, 20, 1)] {
        let e = app
            .world_mut()
            .spawn((
                Fighter,
                PlayerId(player),
                NetworkEntityId(network),
                TeamId(team),
                MatchParticipant {
                    match_id: MatchId(1),
                    ready: true,
                    restart_ready: false,
                },
                FighterDisplayName(format!("Player {player}")),
            ))
            .id();
        if player == 1 {
            source = e;
        }
    }
    (app, root, source)
}
#[test]
fn kills_deaths_and_team_score_share_duplicate_and_stale_validation() {
    let (mut app, root, _) = app();
    let defeat = fact(7, Some((10, 0)), CombatOutcomeKind::Defeat);
    let mut stale = fact(8, Some((10, 0)), CombatOutcomeKind::Defeat);
    stale.tick = 0;
    app.world_mut().resource_mut::<CombatOutcomeFacts>().0 = vec![defeat, defeat, stale];
    app.update();
    let scores = app.world().get::<WipeoutState>(root).unwrap();
    assert_eq!(scores.team_scores, [1, 0]);
    assert_eq!(
        scores
            .players
            .iter()
            .flatten()
            .map(|p| (p.kills, p.deaths))
            .collect::<Vec<_>>(),
        [(1, 0), (0, 1)]
    );
}
#[test]
fn neutral_death_retains_recent_attacker_credit_after_departure() {
    let (mut app, root, source) = app();
    app.world_mut().resource_mut::<CombatOutcomeFacts>().0 = vec![fact(
        1,
        Some((10, 0)),
        CombatOutcomeKind::Damage { amount: 5 },
    )];
    app.update();
    app.world_mut().despawn(source);
    app.world_mut().resource_mut::<CombatOutcomeFacts>().0 =
        vec![fact(2, None, CombatOutcomeKind::Defeat)];
    app.update();
    let scores = app.world().get::<WipeoutState>(root).unwrap();
    assert_eq!(scores.team_scores, [1, 0]);
    assert_eq!(scores.players[0].as_ref().unwrap().kills, 1);
    assert_eq!(scores.players[1].as_ref().unwrap().deaths, 1);
    app.world_mut().get_mut::<MatchState>(root).unwrap().phase = MatchPhase::Completed {
        completed_at_tick: 1,
        restart_unlocked_at_tick: 2,
        result: MatchResult::TeamVictory { team: TeamId(0) },
    };
    app.update();
    let scores = app.world().get::<WipeoutState>(root).unwrap();
    assert_eq!(
        scores.completed_result,
        Some(MatchResult::TeamVictory { team: TeamId(0) })
    );
    assert_eq!(scores.players[0].as_ref().unwrap().kills, 1);
}
#[test]
fn uncredited_environment_death_counts_only_a_death() {
    let (mut app, root, _) = app();
    app.world_mut().resource_mut::<CombatOutcomeFacts>().0 =
        vec![fact(1, None, CombatOutcomeKind::Defeat)];
    app.update();
    let scores = app.world().get::<WipeoutState>(root).unwrap();
    assert_eq!(scores.team_scores, [0, 0]);
    assert_eq!(scores.players[1].as_ref().unwrap().deaths, 1);
}
