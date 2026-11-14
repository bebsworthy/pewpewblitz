//! Feature-owned gameplay and session interpretation for semantic audio requests.

mod ability;
mod combat;
mod matchplay;
mod pickup;
mod session;
mod ui;

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
        pickup::register(app);
        ui::register(app);

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
                pickup::produce_pickup_audio_requests,
                ui::produce_ui_audio_requests,
            )
                .chain()
                .after(brawler::map::MapPresentationSet::Readiness)
                // Pickup cues are read from this frame's shared inbox.
                .after(brawler::map::ClientPickupCueSet)
                // Local attack echoes are paired before any cue is turned into sound.
                .after(crate::local_attack::LocalAttackSet::Reconcile)
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
        ClientPlayableGate,
        audio::{
            registry::AudioRegistryPlugin,
            request::{AudioRequest, cue_keys},
        },
        hud::ClientHeistReadiness,
    };
    use brawler::{
        builds::{AbilityPhase, AbilityState},
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
                defending_team: brawler::combat::TeamId(1),
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
                    ..default()
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
        brawler::test_app::finalize(&mut app);

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
            team: brawler::combat::TeamId(0),
        };

        app.update();

        assert_eq!(
            app.world().resource::<ObservedRequests>().0,
            [
                AudioRequest::for_occurrence(
                    cue_keys::FIRE_STRAIGHT,
                    11,
                    AudioRequestOrder::new(10, 0),
                ),
                AudioRequest::for_occurrence(
                    cue_keys::OBJECTIVE_HIT,
                    20,
                    AudioRequestOrder::new(20, 0),
                ),
                AudioRequest::once(cue_keys::DASH, AudioRequestOrder::new(30, 0)),
                AudioRequest::once(cue_keys::RELOAD, AudioRequestOrder::new(40, 0)),
                AudioRequest::once(cue_keys::PLAYABLE, AudioRequestOrder::new(50, 0)),
                AudioRequest::once(cue_keys::MATCH_SCORE, AudioRequestOrder::new(60, 1)),
                AudioRequest::once(cue_keys::ZONE_CAPTURED, AudioRequestOrder::new(70, 0)),
            ]
        );

        let observed = &app.world().resource::<ObservedRequests>().0;
        let mut scrambled = observed.clone();
        scrambled.reverse();
        scrambled.sort_unstable_by_key(|request| request.order);
        assert_eq!(&scrambled, observed);
    }

    /// Stand-in for the local attack reconciler: the cue becomes an echo of a local shot.
    fn pair_local_echo(mut ledger: ResMut<crate::local_attack::LocalAttackLedger>) {
        ledger.record(lightyear::prelude::Tick(1), 100);
        ledger.observe_own_attack(AttackId(11), 100);
    }

    #[test]
    fn local_fire_sounds_on_the_input_frame_and_its_paired_echo_is_silent() {
        use crate::local_attack::{
            LocalAttackKind, LocalAttackLedger, LocalAttackPresented, LocalAttackSet, LocalShotId,
        };
        let mut app = App::new();
        app.add_message::<DeduplicatedCombatCue>()
            .add_message::<ReceivedHeistObjectiveCue>()
            .add_message::<AudioRequest>()
            .insert_resource(ClientHeistReadiness::Ready)
            .insert_resource(ClientPlayableGate(false))
            .insert_resource(ClientMapReadiness::Ready)
            .init_resource::<LocalAttackLedger>()
            .init_resource::<ObservedRequests>()
            .add_plugins(AudioRegistryPlugin)
            .add_plugins(AudioProducersPlugin)
            .add_systems(Update, pair_local_echo.in_set(LocalAttackSet::Reconcile))
            .add_systems(Update, observe_requests.after(AudioProducerSet));
        brawler::test_app::finalize(&mut app);

        app.world_mut().write_message(LocalAttackPresented {
            fighter: Entity::PLACEHOLDER,
            owner: NetworkEntityId(12),
            team: brawler::combat::TeamId(1),
            kind: LocalAttackKind::PrimaryFire(LocalShotId(0)),
            origin: Vec2::ZERO,
            facing: 0.0,
            simulation_tick: 100,
        });
        // The echo arrives in the same frame; pairing runs before any cue becomes sound.
        app.world_mut().write_message(combat_cue());
        app.update();

        let observed = &app.world().resource::<ObservedRequests>().0;
        assert_eq!(observed.len(), 1, "one fire sound, no replay: {observed:?}");
        assert_eq!(observed[0].cue_key, cue_keys::FIRE_STRAIGHT);
        assert_ne!(
            observed[0].occurrence,
            Some(11),
            "the local occurrence, not the echo"
        );
    }

    /// Stand-in for the local attack reconciler: the own cloak cue and own sentry spawn echo
    /// local casts.
    fn pair_ultimate_echoes(
        mut ledger: ResMut<crate::local_attack::LocalAttackLedger>,
        sentries: Query<(Entity, &brawler::abilities::SentryIdentity)>,
    ) {
        use crate::local_attack::UltimateActivationEcho;
        use brawler::builds::UltimateKind;
        ledger.record_cast(UltimateKind::SelfCloak, lightyear::prelude::Tick(1), 100);
        ledger.observe_own_activation(
            UltimateKind::SelfCloak,
            100,
            Some(UltimateActivationEcho::Cue(CombatEventId(31))),
        );
        for (entity, identity) in &sentries {
            if identity.owner_network_id == NetworkEntityId(12) {
                ledger.record_cast(UltimateKind::Sentry, lightyear::prelude::Tick(1), 100);
                ledger.observe_own_activation(
                    UltimateKind::Sentry,
                    100,
                    Some(UltimateActivationEcho::Spawned(entity)),
                );
            }
        }
    }

    fn sentry(
        owner: u64,
    ) -> (
        brawler::abilities::Sentry,
        brawler::abilities::SentryIdentity,
    ) {
        (
            brawler::abilities::Sentry,
            brawler::abilities::SentryIdentity {
                deployable_id: brawler::builds::DeployableId(owner),
                owner_player_id: brawler::protocol::PlayerId(owner),
                owner_network_id: NetworkEntityId(owner),
                team_id: brawler::combat::TeamId(1),
                ultimate_id: brawler::builds::UltimateDefinitionId(2),
                match_id: MatchId(1),
            },
        )
    }

    #[test]
    fn local_ultimate_sounds_on_the_input_frame_and_its_activation_echo_is_silent() {
        use crate::local_attack::{
            LocalAttackKind, LocalAttackLedger, LocalAttackPresented, LocalAttackSet, LocalCastId,
        };
        use brawler::builds::UltimateKind;
        let mut app = App::new();
        app.add_message::<DeduplicatedCombatCue>()
            .add_message::<ReceivedHeistObjectiveCue>()
            .add_message::<AudioRequest>()
            .insert_resource(ClientHeistReadiness::Ready)
            .insert_resource(ClientPlayableGate(false))
            .insert_resource(ClientMapReadiness::Ready)
            .init_resource::<LocalAttackLedger>()
            .init_resource::<ObservedRequests>()
            .add_plugins(AudioRegistryPlugin)
            .add_plugins(AudioProducersPlugin)
            .add_systems(
                Update,
                pair_ultimate_echoes.in_set(LocalAttackSet::Reconcile),
            )
            .add_systems(Update, observe_requests.after(AudioProducerSet));
        brawler::test_app::finalize(&mut app);

        let local = |kind, cast| LocalAttackPresented {
            fighter: Entity::PLACEHOLDER,
            owner: NetworkEntityId(12),
            team: brawler::combat::TeamId(1),
            kind: LocalAttackKind::Ultimate { kind, cast },
            origin: Vec2::ZERO,
            facing: 0.0,
            simulation_tick: 100,
        };
        // Launch-type casts sound locally; Demolition Strike's sound stays its outcome cue.
        app.world_mut()
            .write_message(local(UltimateKind::SelfCloak, LocalCastId(1)));
        app.world_mut()
            .write_message(local(UltimateKind::Sentry, LocalCastId(2)));
        app.world_mut()
            .write_message(local(UltimateKind::DemolitionStrike, LocalCastId(3)));
        // Echoes arrive in the same frame, plus a teammate's cloak and sentry.
        for (event_id, source) in [(31, 12), (32, 13)] {
            app.world_mut()
                .write_message(DeduplicatedCombatCue(CombatCue::SelfCloakActivated {
                    event_id: CombatEventId(event_id),
                    tick: 100,
                    source: NetworkEntityId(source),
                    generation: 1,
                    expires_at_tick: 400,
                }));
        }
        app.world_mut().spawn(sentry(12));
        app.world_mut().spawn(sentry(13));
        app.update();

        let observed = &app.world().resource::<ObservedRequests>().0;
        let count = |key| {
            observed
                .iter()
                .filter(|request| request.cue_key == key)
                .count()
        };
        assert_eq!(
            count(cue_keys::ULTIMATE_CLOAK),
            2,
            "local cloak and the teammate's cloak, no replay: {observed:?}"
        );
        assert!(
            observed
                .iter()
                .all(|request| request.occurrence != Some(31)),
            "the own activation echo is silent"
        );
        assert_eq!(
            count(cue_keys::SENTRY_SPAWN),
            2,
            "local sentry and the teammate's sentry, no replay: {observed:?}"
        );
        assert_eq!(count(cue_keys::ULTIMATE_STRIKE), 0);
    }

    #[test]
    fn released_controls_request_their_surface_role_sound_and_disabled_controls_stay_silent() {
        use crate::ui::{press::ReleaseActivation, surface::Surface};
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
        brawler::test_app::finalize(&mut app);
        app.world_mut()
            .spawn((ReleaseActivation::activated_for_test(), Surface::Back));
        app.world_mut()
            .spawn((ReleaseActivation::activated_for_test(), Surface::Primary));
        app.world_mut().spawn((
            ReleaseActivation::activated_for_test(),
            bevy::ui::InteractionDisabled,
        ));
        app.world_mut().spawn(ReleaseActivation::default());

        app.update();

        let mut keys = app
            .world()
            .resource::<ObservedRequests>()
            .0
            .iter()
            .map(|request| request.cue_key)
            .collect::<Vec<_>>();
        keys.sort();
        let mut expected = vec![cue_keys::UI_BACK, cue_keys::UI_CONFIRM];
        expected.sort();
        assert_eq!(keys, expected);
    }
}
