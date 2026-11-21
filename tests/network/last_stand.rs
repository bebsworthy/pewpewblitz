//! BRL-5ss9: a routed solo Last Stand Practice round replicates its boundary and standings, and an
//! eliminated fighter stays out: no respawn, a final place, and death boosts on the floor.

use super::*;
use brawler::matchplay::{BoostPickupIdentity, BoostPickupSource, LastStandState, RespawnState};
use brawler::testing::TestRoutedParticipant;

const HUMAN: TestRoutedParticipant = TestRoutedParticipant {
    player_id: 7,
    netcode_client_id: 8,
    peer_id: 9,
    team: 0,
};

/// One human on team 0 and five manifest bots on teams 1..5 on Sunder Ring.
fn last_stand_practice_manifest(manifest: &mut brawler_routing::MatchManifestV1) {
    let maps = brawler::map::MapContentCatalog::embedded().expect("embedded maps");
    manifest.mode = brawler_routing::GameMode::LastStand;
    manifest.map_preset = brawler::map::SUNDER_RING_PRESET.0;
    manifest.map_revision = u32::from(brawler::map::SUNDER_RING_ADMISSION_REVISION);
    manifest.map_recipe_fingerprint = maps
        .resolve_preset(brawler::map::SUNDER_RING_PRESET, MapInstanceId(1))
        .expect("Sunder Ring resolves")
        .snapshot
        .identity
        .recipe_fingerprint
        .0;
    manifest.objective_target = 6;
    let template = manifest.participants[0];
    manifest.bots = (1..=5_u8)
        .map(|team| brawler_routing::MatchManifestBot {
            player_id: brawler_routing::PlayerId::new(100 + u64::from(team)).unwrap(),
            team,
            display_name: brawler_routing::MatchDisplayName::new(&format!("Bot {team}")).unwrap(),
            recipe_fingerprint: template.recipe_fingerprint,
            revision: template.revision,
            build_snapshot: template.build_snapshot,
        })
        .collect();
}

fn server_last_stand(harness: &mut Harness) -> LastStandState {
    let world = harness.server.world_mut();
    let mut query = world.query_filtered::<&LastStandState, With<MatchRootMarker>>();
    *query.single(world).expect("one server Last Stand state")
}

fn client_last_stand(harness: &mut Harness) -> Option<LastStandState> {
    let world = harness.clients[0].world_mut();
    let mut query = world.query_filtered::<&LastStandState, With<MatchRootMarker>>();
    query.iter(world).next().copied()
}

fn client_death_drops(harness: &mut Harness) -> usize {
    let world = harness.clients[0].world_mut();
    let mut query = world.query::<(&BoostPickupIdentity, &BoostPickupSource)>();
    query
        .iter(world)
        .filter(|(_, source)| **source == BoostPickupSource::PlayerDeath)
        .count()
}

/// Defeat the human through the combat outcome facts, as a lethal bot hit would.
fn defeat_human(harness: &mut Harness, fighter: Entity) {
    let world = harness.server.world_mut();
    let target_network_id = *world.get::<NetworkEntityId>(fighter).unwrap();
    let defeat_event = CombatEventId(80_000);
    world.entity_mut(fighter).insert((
        CurrentHealth(0),
        Defeated {
            event_id: defeat_event,
        },
    ));
    let tick = world.resource::<SimulationTick>().0;
    world
        .resource_mut::<CombatOutcomeFacts>()
        .0
        .push(CombatOutcomeFact {
            event_id: defeat_event,
            tick,
            attack_id: AttackId(80_000),
            source_kind: CombatSourceKind::PrimaryWeapon,
            source_player: None,
            source_network_id: None,
            source_team: None,
            target_network_id,
            target_kind: brawler::combat::CombatTargetKind::Fighter,
            target_team: TeamId(HUMAN.team),
            preset_id: None,
            recipe_fingerprint: None,
            position: WorldPoint { x: 0.0, y: 0.0 },
            engagement_distance: 0.0,
            kind: CombatOutcomeKind::Defeat,
        });
}

#[test]
fn routed_last_stand_eliminates_without_respawn_and_replicates_standings() {
    let mut harness = Harness::new_routed_match_with(&[HUMAN], last_stand_practice_manifest);
    harness.add_routed_match_client(HUMAN.netcode_client_id, HUMAN.peer_id);
    harness.step_until(|harness| harness.client_is_active(0));
    for _ in 0..240 {
        harness.check_in_routed_match(0);
        harness.step();
        if matches!(harness.server_match_phase(), MatchPhase::Active { .. }) {
            break;
        }
    }
    assert!(
        matches!(harness.server_match_phase(), MatchPhase::Active { .. }),
        "one human and five manifest bots activate the round"
    );

    let opening = server_last_stand(&mut harness);
    assert_eq!((opening.fighters, opening.alive), (6, 6));
    assert_eq!(opening.placements, [0; 6]);
    harness.step_until(|harness| {
        client_last_stand(harness).is_some_and(|state| state.boundary == opening.boundary)
    });

    let fighter = harness
        .server_fighter(HUMAN.player_id)
        .expect("human fighter");
    defeat_human(&mut harness, fighter);
    harness.step_until(|harness| {
        client_last_stand(harness).is_some_and(|state| state.placements[0] != 0)
            && client_death_drops(harness) > 0
    });
    let standing = server_last_stand(&mut harness);
    assert_eq!(
        standing.placements[0], 6,
        "the first fighter out takes sixth"
    );
    assert_eq!(standing.alive, 5);
    assert_eq!(client_last_stand(&mut harness), Some(standing));

    // Well past the manifest respawn delay, the eliminated fighter is still out.
    for _ in 0..360 {
        harness.step();
        assert!(
            harness
                .server
                .world()
                .get::<RespawnState>(fighter)
                .is_none(),
            "Last Stand never schedules a respawn"
        );
    }
    assert!(harness.server.world().get::<Defeated>(fighter).is_some());
    assert_eq!(server_last_stand(&mut harness).placements[0], 6);
}
