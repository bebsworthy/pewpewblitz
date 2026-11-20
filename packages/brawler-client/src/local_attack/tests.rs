//! Local fire gate, echo pairing, rejection, and schedule tests.

use super::*;
use brawler::combat::{AmmoRecovery, CombatEventId, WeaponDefinitionId, WorldPoint};

fn ready_gate() -> LocalFireGate {
    LocalFireGate {
        simulation_tick: 100,
        weapon: WeaponState::ready(3),
        capacity: 3,
        fire_cooldown_ticks: 10,
        unconfirmed_shots: 0,
        last_shot_simulation_tick: None,
    }
}

#[test]
fn fire_gate_follows_replicated_ammo_cooldown_and_unconfirmed_local_shots() {
    assert!(local_fire_ready(ready_gate()));
    assert!(
        !local_fire_ready(LocalFireGate {
            unconfirmed_shots: 3,
            ..ready_gate()
        }),
        "every replicated round is already spent by unconfirmed local shots"
    );
    assert!(
        !local_fire_ready(LocalFireGate {
            weapon: WeaponState {
                phase: WeaponPhase::Cooldown { ready_at_tick: 101 },
                ..WeaponState::ready(3)
            },
            ..ready_gate()
        }),
        "replicated cooldown not elapsed"
    );
    assert!(
        !local_fire_ready(LocalFireGate {
            last_shot_simulation_tick: Some(95),
            ..ready_gate()
        }),
        "the local cooldown since the previous shot is not elapsed"
    );
    assert!(local_fire_ready(LocalFireGate {
        last_shot_simulation_tick: Some(90),
        ..ready_gate()
    }));
    let empty_recovering = WeaponState {
        ammo: 0,
        phase: WeaponPhase::Ready,
        ammo_recovery: Some(AmmoRecovery {
            started_at_tick: 40,
            ready_at_tick: 100,
        }),
    };
    assert!(
        local_fire_ready(LocalFireGate {
            weapon: empty_recovering,
            ..ready_gate()
        }),
        "a recovery due at the estimated tick refills one round"
    );
    assert!(!local_fire_ready(LocalFireGate {
        simulation_tick: 99,
        weapon: empty_recovering,
        ..ready_gate()
    }));
}

#[test]
fn ultimate_gate_requires_full_charge_and_ready_phase() {
    let ready = AbilityState {
        charge: 100,
        phase: AbilityPhase::Ready,
    };
    assert!(local_ultimate_ready(ready, 100));
    assert!(!local_ultimate_ready(
        AbilityState {
            charge: 99,
            ..ready
        },
        100
    ));
    assert!(!local_ultimate_ready(
        AbilityState {
            phase: AbilityPhase::Charging,
            ..ready
        },
        100
    ));
}

#[test]
fn authoritative_attack_within_tolerance_becomes_an_echo_of_the_local_shot() {
    let mut ledger = LocalAttackLedger::default();
    let shot = ledger.record(Tick(10), 500);
    assert_eq!(ledger.unconfirmed(), 1);
    assert_eq!(
        ledger.observe_own_attack(AttackId(7), 500 + MATCH_LATE_TOLERANCE_TICKS),
        OwnAttackObservation::Matched(shot),
        "an input applied late by the server still pairs"
    );
    assert!(ledger.is_echo(AttackId(7)));
    assert_eq!(ledger.echo_shot(AttackId(7)), Some(shot));
    assert_eq!(
        ledger.status(shot),
        Some(LocalShotStatus::Confirmed(AttackId(7)))
    );
    assert_eq!(ledger.unconfirmed(), 0);
    assert_eq!(
        ledger.observe_own_attack(AttackId(7), 506),
        OwnAttackObservation::Repeated,
        "a duplicate cue never re-matches or double-plays"
    );
    let early = ledger.record(Tick(20), 600);
    assert_eq!(
        ledger.observe_own_attack(AttackId(8), 600 - MATCH_EARLY_TOLERANCE_TICKS - 1),
        OwnAttackObservation::Unmatched,
        "the server cannot fire well before the input tick"
    );
    assert_eq!(ledger.status(early), Some(LocalShotStatus::Pending));
    assert_eq!(
        ledger.observe_own_attack(AttackId(9), 700),
        OwnAttackObservation::Unmatched,
        "a server attack without a local presentation is presented from its cue"
    );
    assert!(!ledger.is_echo(AttackId(9)));
}

#[test]
fn a_later_match_rejects_older_skipped_shots_in_order() {
    let mut ledger = LocalAttackLedger::default();
    let skipped = ledger.record(Tick(10), 500);
    let fired = ledger.record(Tick(20), 510);
    assert_eq!(
        ledger.observe_own_attack(AttackId(9), 511),
        OwnAttackObservation::Matched(fired)
    );
    assert_eq!(ledger.status(skipped), Some(LocalShotStatus::Rejected));
}

#[test]
fn unconfirmed_shots_are_rejected_once_authority_passes_them_and_stay_matchable() {
    let mut ledger = LocalAttackLedger::default();
    let shot = ledger.record(Tick(100), 500);
    let margin = CONFIRMATION_MARGIN_TICKS;
    assert_eq!(
        ledger.expire(Some(Tick(100) + (margin - 1)), Tick(120)),
        0,
        "within margin"
    );
    assert_eq!(ledger.status(shot), Some(LocalShotStatus::Pending));
    assert_eq!(ledger.expire(Some(Tick(100) + margin), Tick(121)), 1);
    assert_eq!(ledger.status(shot), Some(LocalShotStatus::Rejected));
    assert_eq!(
        ledger.unconfirmed(),
        0,
        "a rejected shot no longer holds ammo"
    );
    // A late cue for the same input still pairs, so it cannot replay the presentation.
    assert_eq!(
        ledger.observe_own_attack(AttackId(3), 500),
        OwnAttackObservation::Matched(shot)
    );
    assert!(ledger.is_echo(AttackId(3)));

    let mut silent = LocalAttackLedger::default();
    let stranded = silent.record(Tick(10), 50);
    assert_eq!(silent.expire(None, Tick(99)), 0);
    assert_eq!(silent.expire(None, Tick(100)), 1, "absolute timeout");
    assert_eq!(silent.status(stranded), Some(LocalShotStatus::Rejected));
}

#[test]
fn ledger_is_bounded() {
    let mut ledger = LocalAttackLedger::default();
    for tick in 0..(u32::try_from(MAX_LOCAL_SHOTS).expect("small") * 3) {
        ledger.record(Tick(tick), u64::from(tick));
    }
    assert_eq!(ledger.shots.len(), MAX_LOCAL_SHOTS);
}

#[derive(Resource, Default)]
struct Presented(Vec<LocalAttackPresented>);

fn collect_presented(
    mut messages: MessageReader<LocalAttackPresented>,
    mut presented: ResMut<Presented>,
) {
    presented.0.extend(messages.read().copied());
}

fn plugin_app() -> App {
    let mut app = App::new();
    app.add_plugins(brawler::gameplay::GameplayPlugin)
        .init_resource::<LocalTimeline>()
        .init_resource::<lightyear::prelude::ReplicationCheckpointMap>()
        .insert_resource(brawler::builds::BuildCatalogResource(
            brawler::builds::BuildCatalog::embedded().expect("embedded build catalog"),
        ))
        .insert_resource(brawler::map::MapCatalogResource(
            brawler::map::MapContentCatalog::embedded().expect("embedded map catalog"),
        ))
        .add_message::<DeduplicatedCombatCue>()
        .init_resource::<Presented>()
        .add_plugins((crate::prediction::OwnerPredictionPlugin, LocalAttackPlugin))
        .add_systems(
            FixedUpdate,
            collect_presented.after(LocalAttackSet::Present),
        );
    app
}

fn loadout() -> ResolvedMatchLoadout {
    loadout_with_ultimate(1)
}

fn loadout_with_ultimate(ultimate: u16) -> ResolvedMatchLoadout {
    let builds = brawler::builds::BuildCatalog::embedded().expect("embedded build catalog");
    let weapons = brawler::combat::WeaponCatalog::embedded().expect("embedded weapon catalog");
    brawler::builds::resolve_loadout(
        &builds,
        &weapons,
        brawler::builds::LoadoutRecipe::test_default(
            brawler::combat::WeaponPresetId(1),
            brawler::builds::UltimateDefinitionId(ultimate),
            brawler::builds::PassiveDefinitionId(6),
        ),
    )
    .expect("preset loadout resolves")
}

fn fixed_tick(app: &mut App) {
    app.world_mut()
        .resource_mut::<Time<Fixed>>()
        .advance_by(brawler::timing::SIMULATION_TICK);
    app.world_mut()
        .resource_mut::<LocalTimeline>()
        .apply_delta(1);
    app.world_mut().run_schedule(FixedUpdate);
}

const OWNER: NetworkEntityId = NetworkEntityId(41);

fn spawn_owner(app: &mut App, buttons: u8) -> Entity {
    let owner = app
        .world_mut()
        .spawn((
            Fighter,
            Controlled,
            OWNER,
            TeamId(1),
            brawler::combat::AuthoritativePose {
                position: WorldPoint::from(Vec2::new(10.0, 20.0)),
                facing: 0.0,
                tick: 50,
            },
            ActionState(FighterInput::from_axes(Vec2::ZERO, Some(Vec2::Y), buttons)),
            loadout(),
            WeaponState::ready(3),
            avian2d::prelude::Position(Vec2::new(10.0, 20.0)),
            avian2d::prelude::Rotation::default(),
        ))
        .id();
    // Attach the prediction, then give it a server clock (normally from the first sample).
    fixed_tick(app);
    app.world_mut()
        .get_mut::<OwnerPrediction>(owner)
        .expect("attached")
        .observe_server_clock(Tick(1), 50);
    owner
}

fn accepted(attack_id: u64, tick: u64) -> DeduplicatedCombatCue {
    DeduplicatedCombatCue(CombatCue::AttackAccepted {
        event_id: CombatEventId(attack_id),
        tick,
        attack_id: AttackId(attack_id),
        source: OWNER,
        position: WorldPoint::from(Vec2::new(10.0, 20.0)),
        weapon_definition_id: WeaponDefinitionId(1),
    })
}

#[test]
fn held_fire_is_presented_on_the_input_tick_and_its_cue_becomes_an_echo() {
    let mut app = plugin_app();
    let owner = spawn_owner(&mut app, FighterInput::PRIMARY_FIRE);
    app.world_mut().resource_mut::<Presented>().0.clear();
    fixed_tick(&mut app);
    let presented = app.world().resource::<Presented>().0.clone();
    assert_eq!(
        presented.len(),
        1,
        "presented on the same fixed tick as the input"
    );
    let message = presented[0];
    let LocalAttackKind::PrimaryFire(shot) = message.kind else {
        panic!("primary fire expected");
    };
    assert_eq!(message.fighter, owner);
    assert_eq!(message.owner, OWNER);
    assert!(
        (message.facing - core::f32::consts::FRAC_PI_2).abs() < 1e-4,
        "the input tick's committed aim"
    );

    // The authoritative cue for that tick arrives later and is paired before presentation.
    let simulation_tick = message.simulation_tick;
    app.world_mut().write_message(accepted(77, simulation_tick));
    app.world_mut().run_schedule(Update);
    let ledger = app.world().resource::<LocalAttackLedger>();
    assert_eq!(ledger.echo_shot(AttackId(77)), Some(shot));
    let stats = app.world().resource::<LocalAttackStats>();
    assert_eq!(
        (
            stats.presented,
            stats.confirmed,
            stats.unmatched_own_attacks
        ),
        (1, 1, 0)
    );
}

#[test]
fn local_cooldown_paces_presentation_while_fire_is_held() {
    let mut app = plugin_app();
    spawn_owner(&mut app, FighterInput::PRIMARY_FIRE);
    let cooldown = loadout().primary_weapon.recipe.fire_cooldown_ticks;
    app.world_mut().resource_mut::<Presented>().0.clear();
    for _ in 0..cooldown * 2 {
        fixed_tick(&mut app);
    }
    let presented = app.world().resource::<Presented>().0.clone();
    assert_eq!(presented.len(), 2, "one shot per authored cooldown");
    assert_eq!(
        presented[1].simulation_tick - presented[0].simulation_tick,
        cooldown
    );
}

#[test]
fn suspended_frozen_and_targeted_ultimate_owners_do_not_present_fire() {
    let mut app = plugin_app();
    let owner = spawn_owner(&mut app, FighterInput::PRIMARY_FIRE);
    app.world_mut().entity_mut(owner).insert(ActiveEffects {
        cold: brawler::combat::ColdState {
            frozen_until_tick: Some(10_000),
            ..Default::default()
        },
        ..Default::default()
    });
    app.world_mut().resource_mut::<Presented>().0.clear();
    for _ in 0..5 {
        fixed_tick(&mut app);
    }
    assert!(app.world().resource::<Presented>().0.is_empty(), "frozen");

    let mut app = plugin_app();
    let owner = spawn_owner(&mut app, FighterInput::PRIMARY_FIRE);
    app.world_mut().entity_mut(owner).insert(AbilityState {
        phase: AbilityPhase::Dashing {
            ends_at_tick: 10_000,
        },
        ..Default::default()
    });
    app.world_mut().resource_mut::<Presented>().0.clear();
    for _ in 0..5 {
        fixed_tick(&mut app);
    }
    assert!(app.world().resource::<Presented>().0.is_empty(), "dashing");

    let mut app = plugin_app();
    let owner = spawn_owner(&mut app, FighterInput::PRIMARY_FIRE);
    app.world_mut().entity_mut(owner).insert(WeaponState {
        ammo: 0,
        phase: WeaponPhase::Ready,
        ammo_recovery: None,
    });
    app.world_mut().resource_mut::<Presented>().0.clear();
    for _ in 0..5 {
        fixed_tick(&mut app);
    }
    assert!(app.world().resource::<Presented>().0.is_empty(), "empty");

    // Holding a targeted ultimate (Reveal Scan) aims it; the server does not fire the weapon.
    let mut app = plugin_app();
    let owner = spawn_owner(&mut app, 0);
    app.world_mut().entity_mut(owner).insert((
        loadout_with_ultimate(4),
        ActionState(FighterInput::from_axes(
            Vec2::ZERO,
            None,
            FighterInput::PRIMARY_FIRE | FighterInput::ULTIMATE,
        )),
    ));
    app.world_mut().resource_mut::<Presented>().0.clear();
    for _ in 0..5 {
        fixed_tick(&mut app);
    }
    assert!(
        !app.world()
            .resource::<Presented>()
            .0
            .iter()
            .any(|message| matches!(message.kind, LocalAttackKind::PrimaryFire(_))),
        "targeted ultimate held"
    );
}

#[test]
fn ultimate_rising_edge_with_full_charge_presents_one_cast() {
    for (ultimate, expected) in [(3, 1), (1, 1)] {
        let mut app = plugin_app();
        let owner = spawn_owner(&mut app, 0);
        let loadout = loadout_with_ultimate(ultimate);
        let maximum = loadout.ultimate.charge_policy.maximum;
        app.world_mut().entity_mut(owner).insert((
            loadout,
            AbilityState {
                charge: maximum,
                phase: AbilityPhase::Ready,
            },
            ActionState(FighterInput::from_axes(
                Vec2::ZERO,
                None,
                FighterInput::ULTIMATE,
            )),
        ));
        app.world_mut().resource_mut::<Presented>().0.clear();
        for _ in 0..5 {
            fixed_tick(&mut app);
        }
        let casts = app
            .world()
            .resource::<Presented>()
            .0
            .iter()
            .filter(|message| matches!(message.kind, LocalAttackKind::Ultimate { .. }))
            .count();
        assert_eq!(
            casts, expected,
            "ultimate {ultimate}: only the rising edge casts"
        );
    }
}

#[test]
fn unconfirmed_fire_is_rejected_after_authority_passes_it() {
    let mut app = plugin_app();
    let owner = spawn_owner(&mut app, FighterInput::PRIMARY_FIRE);
    fixed_tick(&mut app);
    app.world_mut()
        .entity_mut(owner)
        .insert(ActionState(FighterInput::default()));
    let presented_at = app.world().resource::<LocalTimeline>().tick();
    // Authority confirms state well past the shot without an AttackAccepted cue.
    {
        let mut entity = app.world_mut().entity_mut(owner);
        let mut prediction = entity.get_mut::<OwnerPrediction>().expect("attached");
        prediction.reconcile(
            crate::prediction::AuthoritativeSample {
                tick: presented_at + (CONFIRMATION_MARGIN_TICKS + 1),
                position: Vec2::new(10.0, 20.0),
                facing: 0.0,
            },
            crate::prediction::OwnerPredictionTuning::default(),
        );
    }
    app.world_mut().run_schedule(Update);
    let stats = app.world().resource::<LocalAttackStats>();
    assert_eq!((stats.presented, stats.rejected), (1, 1));
}

fn self_cloak_cue(event_id: u64, tick: u64, source: NetworkEntityId) -> DeduplicatedCombatCue {
    DeduplicatedCombatCue(CombatCue::SelfCloakActivated {
        event_id: CombatEventId(event_id),
        tick,
        source,
        generation: 1,
        expires_at_tick: tick + 360,
    })
}

#[test]
fn ultimate_activation_within_tolerance_echoes_the_local_cast_once() {
    let mut ledger = LocalAttackLedger::default();
    let cast = ledger.record_cast(UltimateKind::SelfCloak, Tick(10), 100);
    assert!(
        ledger.ultimate_outstanding(),
        "a pending cast holds the charge"
    );
    // Another kind or a tick outside the late window never pairs.
    let cue = UltimateActivationEcho::Cue(CombatEventId(5));
    assert_eq!(
        ledger.observe_own_activation(UltimateKind::RevealScan, 100, Some(cue)),
        OwnActivationObservation::Unmatched
    );
    assert_eq!(
        ledger.observe_own_activation(
            UltimateKind::SelfCloak,
            100 + MATCH_LATE_TOLERANCE_TICKS + 1,
            Some(cue)
        ),
        OwnActivationObservation::Unmatched
    );
    assert_eq!(
        ledger.observe_own_activation(UltimateKind::SelfCloak, 104, Some(cue)),
        OwnActivationObservation::Matched(cast)
    );
    assert!(ledger.is_activation_echo_cue(CombatEventId(5)));
    assert_eq!(ledger.cast_status(cast), Some(LocalCastStatus::Confirmed));
    assert_eq!(
        ledger.observe_own_activation(UltimateKind::SelfCloak, 104, Some(cue)),
        OwnActivationObservation::Repeated
    );
    // A second activation signal for the same cast is not an echo (one sound per cast).
    let other = UltimateActivationEcho::Cue(CombatEventId(6));
    assert_eq!(
        ledger.observe_own_activation(UltimateKind::SelfCloak, 104, Some(other)),
        OwnActivationObservation::Unmatched
    );
    assert!(!ledger.is_activation_echo_cue(CombatEventId(6)));
    assert!(
        ledger.ultimate_outstanding(),
        "confirmed while the replicated charge still reads full"
    );
    ledger.observe_charge_spent();
    assert!(!ledger.ultimate_outstanding());
}

#[test]
fn outcome_activations_confirm_the_cast_without_suppressing_their_sound() {
    let mut ledger = LocalAttackLedger::default();
    let cast = ledger.record_cast(UltimateKind::DemolitionStrike, Tick(10), 100);
    let cue = UltimateActivationEcho::Cue(CombatEventId(9));
    assert_eq!(
        ledger.observe_own_activation(UltimateKind::DemolitionStrike, 100, Some(cue)),
        OwnActivationObservation::Matched(cast)
    );
    assert!(
        !ledger.is_activation_echo_cue(CombatEventId(9)),
        "the strike's explosion sound is an outcome and stays authoritative"
    );
}

#[test]
fn unanswered_cast_is_rejected_and_a_late_activation_still_pairs() {
    let mut ledger = LocalAttackLedger::default();
    let cast = ledger.record_cast(UltimateKind::Sentry, Tick(10), 100);
    assert_eq!(
        ledger.expire_casts(Some(Tick(10) + (CONFIRMATION_MARGIN_TICKS - 1)), Tick(20)),
        0
    );
    assert_eq!(
        ledger.expire_casts(Some(Tick(10) + CONFIRMATION_MARGIN_TICKS), Tick(20)),
        1
    );
    assert_eq!(ledger.cast_status(cast), Some(LocalCastStatus::Rejected));
    assert!(
        !ledger.ultimate_outstanding(),
        "a rejected cast frees the charge"
    );
    let spawned = UltimateActivationEcho::Spawned(Entity::from_raw_u32(7).unwrap());
    assert_eq!(
        ledger.observe_own_activation(UltimateKind::Sentry, 103, Some(spawned)),
        OwnActivationObservation::Matched(cast),
        "a late activation of a rejected cast never plays twice"
    );
    assert!(ledger.is_activation_echo_entity(Entity::from_raw_u32(7).unwrap()));
}

#[test]
fn spent_charge_confirms_a_cast_without_an_activation_signal_and_records_are_bounded() {
    let mut ledger = LocalAttackLedger::default();
    let cast = ledger.record_cast(UltimateKind::BigBlob, Tick(10), 100);
    let signalled = ledger.record_cast(UltimateKind::SelfCloak, Tick(11), 101);
    assert_eq!(ledger.observe_charge_spent(), 1);
    assert_eq!(ledger.cast_status(cast), Some(LocalCastStatus::Confirmed));
    assert_eq!(
        ledger.cast_status(signalled),
        Some(LocalCastStatus::Pending),
        "a kind with an activation signal is confirmed only by that signal"
    );
    assert!(
        ledger.ultimate_outstanding(),
        "the pending cast still holds the charge"
    );
    for index in 0..10 {
        ledger.record_cast(
            UltimateKind::SelfCloak,
            Tick(20 + index),
            200 + u64::from(index),
        );
    }
    assert_eq!(ledger.casts.len(), MAX_LOCAL_ULTIMATE_CASTS);
    for index in 0..20 {
        ledger
            .ultimate_echoes
            .push_back(UltimateActivationEcho::Cue(CombatEventId(index)));
    }
    let last = ledger.record_cast(UltimateKind::SelfCloak, Tick(90), 300);
    assert_eq!(
        ledger.observe_own_activation(
            UltimateKind::SelfCloak,
            300,
            Some(UltimateActivationEcho::Cue(CombatEventId(99)))
        ),
        OwnActivationObservation::Matched(last)
    );
    assert_eq!(ledger.ultimate_echoes.len(), MAX_ULTIMATE_ACTIVATION_ECHOES);
}

#[test]
fn only_launch_type_ultimates_present_their_activation_sound_locally() {
    for kind in [
        UltimateKind::SelfCloak,
        UltimateKind::RevealScan,
        UltimateKind::ConcealmentField,
        UltimateKind::Sentry,
        UltimateKind::CryogenicField,
        UltimateKind::FireField,
        UltimateKind::PoisonField,
        UltimateKind::RestorationField,
        UltimateKind::Dash,
    ] {
        assert!(presents_activation_sound_locally(kind), "{kind:?}");
        assert!(!plays_cast_sound_locally(kind), "{kind:?}");
    }
    for kind in [UltimateKind::DemolitionStrike, UltimateKind::BigBlob] {
        assert!(!presents_activation_sound_locally(kind), "{kind:?}");
        assert!(presents_cast_motion_locally(kind), "{kind:?}");
        assert!(plays_cast_sound_locally(kind), "{kind:?}");
    }
    assert!(
        !presents_cast_motion_locally(UltimateKind::Dash),
        "a local Dash is sound only; its motion and trail follow authority"
    );
}

/// Dash ultimate duration in the embedded build catalog (ultimate 1).
fn dash_duration_ticks() -> u64 {
    let brawler::builds::UltimateParameters::Dash { duration_ticks, .. } =
        loadout_with_ultimate(1).ultimate.parameters
    else {
        panic!("dash ultimate");
    };
    duration_ticks
}

#[test]
fn replicated_dash_phase_pairs_with_the_local_cast_on_time_late_and_after_rejection() {
    let duration = dash_duration_ticks();
    // On time: the activation tick (ends_at - duration) equals the estimated cast tick.
    let mut ledger = LocalAttackLedger::default();
    let cast = ledger.record_cast(UltimateKind::Dash, Tick(10), 100);
    assert_eq!(
        ledger.observe_own_dash(100 + duration, duration),
        OwnActivationObservation::Matched(cast)
    );
    assert!(ledger.is_activation_echo_dash(100 + duration));
    assert_eq!(ledger.cast_status(cast), Some(LocalCastStatus::Confirmed));
    assert_eq!(
        ledger.observe_own_dash(100 + duration, duration),
        OwnActivationObservation::Repeated,
        "the phase stays replicated for the whole dash and is folded in once"
    );

    // Late: the input reached the server six ticks late; one tick more is another dash.
    let mut ledger = LocalAttackLedger::default();
    let cast = ledger.record_cast(UltimateKind::Dash, Tick(10), 100);
    let beyond = 100 + MATCH_LATE_TOLERANCE_TICKS + 1 + duration;
    assert_eq!(
        ledger.observe_own_dash(beyond, duration),
        OwnActivationObservation::Unmatched
    );
    assert!(!ledger.is_activation_echo_dash(beyond));
    let late = 100 + MATCH_LATE_TOLERANCE_TICKS + duration;
    assert_eq!(
        ledger.observe_own_dash(late, duration),
        OwnActivationObservation::Matched(cast)
    );
    assert!(ledger.is_activation_echo_dash(late));

    // Rejected: the server never dashes, so the cast is rejected and frees the charge; nothing
    // else is presented. A dash that still arrives late pairs and never sounds twice.
    let mut ledger = LocalAttackLedger::default();
    let cast = ledger.record_cast(UltimateKind::Dash, Tick(10), 100);
    assert_eq!(ledger.observe_charge_spent(), 0, "Dash has its own signal");
    assert_eq!(
        ledger.expire_casts(Some(Tick(10) + CONFIRMATION_MARGIN_TICKS), Tick(20)),
        1
    );
    assert_eq!(ledger.cast_status(cast), Some(LocalCastStatus::Rejected));
    assert!(!ledger.ultimate_outstanding());
    assert_eq!(
        ledger.observe_own_dash(103 + duration, duration),
        OwnActivationObservation::Matched(cast)
    );
    assert!(ledger.is_activation_echo_dash(103 + duration));
}

#[test]
fn dash_press_presents_a_cast_and_the_replicated_dash_becomes_its_echo() {
    let mut app = plugin_app();
    let owner = spawn_owner(&mut app, 0);
    let loadout = loadout_with_ultimate(1);
    let maximum = loadout.ultimate.charge_policy.maximum;
    app.world_mut().entity_mut(owner).insert((
        loadout,
        AbilityState {
            charge: maximum,
            phase: AbilityPhase::Ready,
        },
    ));
    app.world_mut().resource_mut::<Presented>().0.clear();
    set_buttons(&mut app, owner, FighterInput::ULTIMATE);
    fixed_tick(&mut app);
    let [(UltimateKind::Dash, cast, simulation_tick)] = presented_casts(&app)[..] else {
        panic!("one dash cast");
    };
    // The server applies the dash four ticks late; its phase replicates for the whole dash.
    let ends_at_tick = simulation_tick + 4 + dash_duration_ticks();
    for _ in 0..3 {
        app.world_mut().entity_mut(owner).insert(AbilityState {
            charge: 0,
            phase: AbilityPhase::Dashing { ends_at_tick },
        });
        app.world_mut().run_schedule(Update);
    }
    let ledger = app.world().resource::<LocalAttackLedger>();
    assert!(ledger.is_activation_echo_dash(ends_at_tick));
    assert_eq!(ledger.cast_status(cast), Some(LocalCastStatus::Confirmed));
    let stats = app.world().resource::<LocalAttackStats>();
    assert_eq!(
        (
            stats.ultimates_presented,
            stats.ultimates_confirmed,
            stats.ultimate_activation_echoes,
            stats.unmatched_own_activations
        ),
        (1, 1, 1, 0)
    );
}

/// A controlled Self Cloak fighter with full charge, pressing ULTIMATE.
fn spawn_cloak_caster(app: &mut App) -> Entity {
    let owner = spawn_owner(app, 0);
    let loadout = loadout_with_ultimate(3);
    let maximum = loadout.ultimate.charge_policy.maximum;
    app.world_mut().entity_mut(owner).insert((
        loadout,
        AbilityState {
            charge: maximum,
            phase: AbilityPhase::Ready,
        },
    ));
    owner
}

fn set_buttons(app: &mut App, owner: Entity, buttons: u8) {
    app.world_mut()
        .entity_mut(owner)
        .insert(ActionState(FighterInput::from_axes(
            Vec2::ZERO,
            None,
            buttons,
        )));
}

fn presented_casts(app: &App) -> Vec<(UltimateKind, LocalCastId, u64)> {
    app.world()
        .resource::<Presented>()
        .0
        .iter()
        .filter_map(|message| match message.kind {
            LocalAttackKind::Ultimate { kind, cast } => Some((kind, cast, message.simulation_tick)),
            LocalAttackKind::PrimaryFire(_) => None,
        })
        .collect()
}

#[test]
fn repressing_while_the_cast_is_unanswered_does_not_cast_again() {
    let mut app = plugin_app();
    let owner = spawn_cloak_caster(&mut app);
    app.world_mut().resource_mut::<Presented>().0.clear();
    for buttons in [FighterInput::ULTIMATE, 0, FighterInput::ULTIMATE, 0] {
        set_buttons(&mut app, owner, buttons);
        fixed_tick(&mut app);
    }
    assert_eq!(
        presented_casts(&app).len(),
        1,
        "the stale full charge cannot cast twice before the server answers"
    );
    // Authority passes the cast without activating: it is rejected and a new press casts.
    let presented_at = app.world().resource::<LocalTimeline>().tick();
    app.world_mut()
        .get_mut::<OwnerPrediction>(owner)
        .expect("attached")
        .reconcile(
            crate::prediction::AuthoritativeSample {
                tick: presented_at + CONFIRMATION_MARGIN_TICKS,
                position: Vec2::new(10.0, 20.0),
                facing: 0.0,
            },
            crate::prediction::OwnerPredictionTuning::default(),
        );
    app.world_mut().run_schedule(Update);
    assert_eq!(
        app.world()
            .resource::<LocalAttackStats>()
            .ultimates_rejected,
        1
    );
    set_buttons(&mut app, owner, FighterInput::ULTIMATE);
    fixed_tick(&mut app);
    assert_eq!(presented_casts(&app).len(), 2);
}

#[test]
fn own_activation_cue_becomes_an_activation_echo_while_others_do_not() {
    let mut app = plugin_app();
    let owner = spawn_cloak_caster(&mut app);
    app.world_mut().resource_mut::<Presented>().0.clear();
    set_buttons(&mut app, owner, FighterInput::ULTIMATE);
    fixed_tick(&mut app);
    let [(kind, cast, simulation_tick)] = presented_casts(&app)[..] else {
        panic!("one cast");
    };
    assert_eq!(kind, UltimateKind::SelfCloak);
    // A teammate's cloak and the own cloak arrive together.
    app.world_mut()
        .write_message(self_cloak_cue(30, simulation_tick, NetworkEntityId(77)));
    app.world_mut()
        .write_message(self_cloak_cue(31, simulation_tick + 1, OWNER));
    app.world_mut().run_schedule(Update);
    let ledger = app.world().resource::<LocalAttackLedger>();
    assert!(ledger.is_activation_echo_cue(CombatEventId(31)));
    assert!(!ledger.is_activation_echo_cue(CombatEventId(30)));
    assert_eq!(ledger.cast_status(cast), Some(LocalCastStatus::Confirmed));
    let stats = app.world().resource::<LocalAttackStats>();
    assert_eq!(
        (
            stats.ultimates_presented,
            stats.ultimates_confirmed,
            stats.ultimate_activation_echoes,
            stats.unmatched_own_activations
        ),
        (1, 1, 1, 0)
    );
}

#[test]
fn own_sentry_spawn_becomes_an_activation_echo() {
    let mut app = plugin_app();
    let owner = spawn_owner(&mut app, 0);
    let loadout = loadout_with_ultimate(2);
    let maximum = loadout.ultimate.charge_policy.maximum;
    let brawler::builds::UltimateParameters::Sentry { lifetime_ticks, .. } =
        loadout.ultimate.parameters
    else {
        panic!("sentry ultimate");
    };
    app.world_mut().entity_mut(owner).insert((
        loadout,
        AbilityState {
            charge: maximum,
            phase: AbilityPhase::Ready,
        },
    ));
    app.world_mut().resource_mut::<Presented>().0.clear();
    set_buttons(&mut app, owner, FighterInput::ULTIMATE);
    fixed_tick(&mut app);
    let [(UltimateKind::Sentry, cast, simulation_tick)] = presented_casts(&app)[..] else {
        panic!("one sentry cast");
    };
    let identity = |owner_network_id| brawler::abilities::SentryIdentity {
        deployable_id: brawler::builds::DeployableId(1),
        owner_player_id: brawler::protocol::PlayerId(1),
        owner_network_id,
        team_id: TeamId(1),
        ultimate_id: brawler::builds::UltimateDefinitionId(2),
        match_id: brawler::matchplay::MatchId(1),
    };
    let deadline = brawler::abilities::SentryDeadline {
        expires_at_tick: simulation_tick + 2 + lifetime_ticks,
    };
    let own = app
        .world_mut()
        .spawn((brawler::abilities::Sentry, identity(OWNER), deadline))
        .id();
    let other = app
        .world_mut()
        .spawn((
            brawler::abilities::Sentry,
            identity(NetworkEntityId(77)),
            deadline,
        ))
        .id();
    app.world_mut().run_schedule(Update);
    let ledger = app.world().resource::<LocalAttackLedger>();
    assert!(ledger.is_activation_echo_entity(own));
    assert!(!ledger.is_activation_echo_entity(other));
    assert_eq!(ledger.cast_status(cast), Some(LocalCastStatus::Confirmed));
}
