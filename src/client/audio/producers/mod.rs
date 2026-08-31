//! Feature-owned gameplay and session interpretation for semantic audio requests.

mod ability;
mod combat;
mod matchplay;
mod session;

use bevy::prelude::*;

use super::request::AudioRequestOrder;

/// All built-in audio request producers, in their established capacity-precedence order.
pub(crate) struct AudioProducersPlugin;

impl Plugin for AudioProducersPlugin {
    fn build(&self, app: &mut App) {
        combat::register(app);
        matchplay::register_heist(app);
        ability::register_ability(app);
        ability::register_reload(app);
        session::register(app);
        matchplay::register_common(app);
        matchplay::register_hot_zone(app);

        app.add_systems(
            Update,
            (
                combat::produce_combat_audio_requests,
                matchplay::produce_heist_audio_requests,
                ability::produce_ability_audio_requests,
                ability::produce_reload_audio_requests,
                session::produce_session_audio_requests,
                matchplay::produce_match_audio_requests,
                matchplay::produce_hot_zone_audio_requests,
            )
                .chain()
                .after(crate::map::MapPresentationSet::Readiness)
                .in_set(AudioProducerSet),
        );
    }
}

/// Feature-owned production phase nested inside the generic playback schedule boundary.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct AudioProducerSet;

/// System-local stable sequence used only to break ties within one registered producer rank.
#[derive(Default)]
pub(super) struct AudioProducerSequence(u64);

impl AudioProducerSequence {
    fn next(&mut self, producer_rank: u16) -> AudioRequestOrder {
        let order = AudioRequestOrder::new(producer_rank, self.0);
        self.0 = self.0.saturating_add(1);
        order
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        builds::{AbilityPhase, AbilityState},
        client::{
            ClientPlayableGate,
            audio::{
                registry::AudioRegistryPlugin,
                request::{AudioRequest, cue_keys},
            },
            hud::ClientHeistReadiness,
        },
        combat::{
            AttackId, CombatCue, CombatEventId, DeduplicatedCombatCue, WeaponDefinitionId,
            WeaponState, WorldPoint,
        },
        map::{
            ClientMapReadiness, DamageableTargetIdentity, ModeAnchorId, WIPEOUT_MODE_DEFINITION,
        },
        matchplay::{
            HeistObjectiveCue, HeistObjectiveCueKind, HotZoneState, HotZoneStatus, MatchId,
            MatchPhase, MatchRoot, MatchState, ReceivedHeistObjectiveCue, WipeoutState,
        },
        protocol::{Fighter, NetworkEntityId},
    };
    use lightyear::prelude::Controlled;

    #[derive(Resource, Default)]
    struct ObservedRequests(Vec<AudioRequest>);

    fn observe_requests(
        mut requests: MessageReader<AudioRequest>,
        mut observed: ResMut<ObservedRequests>,
    ) {
        observed.0.extend(requests.read().copied());
    }

    fn combat_cue() -> DeduplicatedCombatCue {
        DeduplicatedCombatCue(CombatCue::AttackAccepted {
            event_id: CombatEventId(10),
            tick: 100,
            attack_id: AttackId(11),
            source: NetworkEntityId(12),
            position: WorldPoint { x: 1.0, y: 2.0 },
            weapon_definition_id: WeaponDefinitionId(13),
        })
    }

    fn heist_cue() -> ReceivedHeistObjectiveCue {
        ReceivedHeistObjectiveCue(HeistObjectiveCue {
            event_id: CombatEventId(20),
            tick: 100,
            attack_id: AttackId(21),
            source_subject: None,
            target: DamageableTargetIdentity::HeistSafe {
                match_id: MatchId(1),
                anchor_id: ModeAnchorId(2),
                defending_team: crate::combat::TeamId(1),
            },
            position: WorldPoint { x: 3.0, y: 4.0 },
            amount: 1,
            health_after: 9,
            maximum_health: 10,
            kind: HeistObjectiveCueKind::Damaged,
        })
    }

    #[test]
    fn built_in_chain_emits_before_same_frame_consumer_in_preserved_order() {
        let mut app = App::new();
        app.add_message::<DeduplicatedCombatCue>()
            .add_message::<ReceivedHeistObjectiveCue>()
            .add_message::<AudioRequest>()
            .insert_resource(ClientHeistReadiness::Ready)
            .insert_resource(ClientPlayableGate(false))
            .insert_resource(ClientMapReadiness::Ready)
            .init_resource::<ObservedRequests>()
            .add_plugins(AudioRegistryPlugin)
            .add_plugins(AudioProducersPlugin)
            .add_systems(Update, observe_requests.after(AudioProducerSet));

        let fighter = app
            .world_mut()
            .spawn((
                Fighter,
                Controlled,
                AbilityState::default(),
                WeaponState::ready(1),
            ))
            .id();
        let match_root = app
            .world_mut()
            .spawn((
                MatchRoot,
                MatchState {
                    match_id: MatchId(1),
                    mode_definition_id: WIPEOUT_MODE_DEFINITION,
                    phase: MatchPhase::Active { ends_at_tick: 200 },
                    rules_revision: 1,
                },
                WipeoutState {
                    team_scores: [0, 0],
                    target_score: 10,
                },
                HotZoneState {
                    match_id: MatchId(1),
                    zone_anchor_id: ModeAnchorId(3),
                    occupants: [0, 0],
                    status: HotZoneStatus::Empty,
                    progress_ticks: [0, 0],
                    target_progress_ticks: 100,
                    next_evaluation_tick: 101,
                },
            ))
            .id();
        crate::test_app::finalize(&mut app);

        // Seed transition memories and reload's system-local ammunition observation.
        app.update();
        app.world_mut().resource_mut::<ObservedRequests>().0.clear();

        app.world_mut().write_message(combat_cue());
        app.world_mut().write_message(heist_cue());
        app.world_mut().resource_mut::<ClientPlayableGate>().0 = true;
        app.world_mut()
            .entity_mut(fighter)
            .get_mut::<AbilityState>()
            .unwrap()
            .phase = AbilityPhase::Dashing { ends_at_tick: 110 };
        app.world_mut()
            .entity_mut(fighter)
            .get_mut::<WeaponState>()
            .unwrap()
            .ammo = 2;
        app.world_mut()
            .entity_mut(match_root)
            .get_mut::<WipeoutState>()
            .unwrap()
            .team_scores = [1, 0];
        app.world_mut()
            .entity_mut(match_root)
            .get_mut::<HotZoneState>()
            .unwrap()
            .status = HotZoneStatus::Controlled {
            team: crate::combat::TeamId(0),
        };

        app.update();

        assert_eq!(
            app.world().resource::<ObservedRequests>().0,
            [
                AudioRequest::for_occurrence(cue_keys::FIRE, 11, AudioRequestOrder::new(10, 0),),
                AudioRequest::for_occurrence(
                    cue_keys::OBJECTIVE_HIT,
                    20,
                    AudioRequestOrder::new(20, 0),
                ),
                AudioRequest::once(cue_keys::DASH, AudioRequestOrder::new(30, 0)),
                AudioRequest::once(cue_keys::RELOAD, AudioRequestOrder::new(40, 0)),
                AudioRequest::once(cue_keys::READY, AudioRequestOrder::new(50, 0)),
                AudioRequest::once(cue_keys::IMPACT, AudioRequestOrder::new(60, 1)),
                AudioRequest::once(cue_keys::READY, AudioRequestOrder::new(70, 0)),
            ]
        );

        let observed = &app.world().resource::<ObservedRequests>().0;
        let mut scrambled = observed.clone();
        scrambled.reverse();
        scrambled.sort_unstable_by_key(|request| request.order);
        assert_eq!(&scrambled, observed);
    }
}
