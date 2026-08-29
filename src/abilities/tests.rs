use super::*;
use crate::builds::{AbilityPhase, AbilityState};
use crate::protocol::NetworkEntityId;
use bevy::prelude::Vec2;

#[cfg(feature = "server")]
fn damage_fact(
    event: u64,
    source_kind: crate::combat::CombatSourceKind,
    target_kind: crate::combat::CombatTargetKind,
    source_team: crate::combat::TeamId,
    target_team: crate::combat::TeamId,
) -> crate::combat::CombatOutcomeFact {
    crate::combat::CombatOutcomeFact {
        event_id: crate::combat::CombatEventId(event),
        tick: 10,
        attack_id: crate::combat::AttackId(event),
        source_kind,
        source_player: Some(crate::protocol::PlayerId(1)),
        source_network_id: Some(NetworkEntityId(1)),
        source_team: Some(source_team),
        target_network_id: NetworkEntityId(2),
        target_kind,
        target_team,
        preset_id: Some(crate::combat::WeaponPresetId(1)),
        recipe_fingerprint: Some(crate::combat::WeaponRecipeFingerprint(1)),
        position: crate::combat::WorldPoint { x: 0.0, y: 0.0 },
        engagement_distance: 100.0,
        kind: crate::combat::CombatOutcomeKind::Damage { amount: 10 },
    }
}

#[cfg(feature = "server")]
#[test]
fn self_cloak_ends_from_attack_fact_without_an_attack_cue_and_preserves_precedence() {
    use crate::combat::{
        AcceptedAttackFact, AcceptedAttackFacts, AttackId, CombatCue, CombatEventId, CombatOutbox,
        CombatOutcomeFacts, NextCombatIds, SelfCloakEndReason,
    };
    use crate::protocol::Fighter;
    use crate::timing::SimulationTick;
    use bevy::prelude::*;

    let mut app = App::new();
    app.insert_resource(SimulationTick(10))
        .init_resource::<AcceptedAttackFacts>()
        .init_resource::<CombatOutcomeFacts>()
        .init_resource::<NextCombatIds>()
        .init_resource::<CombatOutbox>()
        .init_resource::<AbilityTelemetry>()
        .add_systems(Update, self_cloak::resolve_self_cloak_lifecycle);
    let fighter = app
        .world_mut()
        .spawn((
            Fighter,
            NetworkEntityId(2),
            AbilityState {
                charge: 0,
                phase: AbilityPhase::Cloaked {
                    generation: 3,
                    activated_at_tick: 4,
                    expires_at_tick: 10,
                },
            },
        ))
        .id();
    assert!(
        app.world_mut()
            .resource_mut::<AcceptedAttackFacts>()
            .record(AcceptedAttackFact {
                event_id: CombatEventId(8),
                tick: 10,
                attack_id: AttackId(8),
                source_network_id: NetworkEntityId(2),
            })
    );
    app.world_mut().resource_mut::<CombatOutcomeFacts>().0 = vec![damage_fact(
        9,
        crate::combat::CombatSourceKind::PrimaryWeapon,
        crate::combat::CombatTargetKind::Fighter,
        crate::combat::TeamId(0),
        crate::combat::TeamId(1),
    )];

    app.update();

    assert_eq!(
        app.world().get::<AbilityState>(fighter).unwrap().phase,
        AbilityPhase::Charging
    );
    let outbox = &app.world().resource::<CombatOutbox>().0;
    assert!(
        !outbox
            .iter()
            .any(|cue| matches!(cue, CombatCue::AttackAccepted { .. }))
    );
    assert!(outbox.iter().any(|cue| matches!(
        cue,
        CombatCue::SelfCloakEnded {
            source: NetworkEntityId(2),
            reason: SelfCloakEndReason::Attack,
            ..
        }
    )));
}

#[test]
fn charge_uses_exact_damage_multipliers_caps_and_becomes_ready() {
    assert_eq!(
        apply_charge(AbilityState::default(), 1, 1),
        AbilityState {
            charge: 8,
            phase: AbilityPhase::Charging,
        }
    );
    let state = apply_charge(AbilityState::default(), 100, 100);
    assert_eq!(state.charge, 800);
    let state = apply_charge(state, u16::MAX, u16::MAX);
    assert_eq!(
        state,
        AbilityState {
            charge: 1_000,
            phase: AbilityPhase::Ready
        }
    );
    assert_eq!(apply_charge(state, 1, 1), state);
    for phase in [
        AbilityPhase::Dashing { ends_at_tick: 10 },
        AbilityPhase::Deployed {
            deployable_id: crate::builds::DeployableId(4),
            expires_at_tick: 20,
        },
        AbilityPhase::Cloaked {
            generation: 1,
            activated_at_tick: 4,
            expires_at_tick: 20,
        },
    ] {
        let active = AbilityState { charge: 0, phase };
        assert_eq!(
            apply_charge(active, u16::MAX, u16::MAX),
            AbilityState {
                charge: ULTIMATE_CHARGE_MAX,
                phase,
            },
            "active execution preserves its phase while charge continues to accrue",
        );
        assert_eq!(
            settled_ability_phase(ULTIMATE_CHARGE_MAX),
            AbilityPhase::Ready,
        );
    }
}

#[test]
fn reveal_scan_targeting_uses_current_aim_distance_and_clamps_center() {
    let bounds = crate::map::AxisAlignedMapRect {
        min: Vec2::new(-100.0, -80.0),
        max: Vec2::new(100.0, 80.0),
    };
    assert_eq!(
        targeted_ultimate_center(Vec2::ZERO, Vec2::X, Some(Vec2::Y), Some(40.0), 64.0, bounds),
        Some(Vec2::new(0.0, 40.0))
    );
    assert_eq!(
        targeted_ultimate_center(Vec2::new(90.0, 0.0), Vec2::X, None, None, 64.0, bounds),
        Some(Vec2::new(100.0, 0.0))
    );
    assert!(targeted_ultimate_center(Vec2::NAN, Vec2::X, None, None, 64.0, bounds).is_none());
}

#[cfg(feature = "server")]
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the focused ECS test verifies acceptance, held input, and stale rejection together"
)]
fn demolition_activation_spends_charge_and_emits_one_typed_world_effect() {
    use avian2d::prelude::{Position, Rotation};
    use bevy::prelude::*;
    use lightyear::prelude::input::native::ActionState;

    let builds = crate::builds::BuildCatalog::embedded().unwrap();
    let weapons = crate::combat::WeaponCatalog::embedded().unwrap();
    let fighter = crate::combat::FighterDefinitions::default().entries[0];
    let loadout = crate::builds::resolve_build_recipe(
        &builds,
        &weapons,
        &fighter,
        crate::builds::BrawlerBuildRecipe {
            weapon: crate::builds::WeaponChoice::Preset(crate::combat::WeaponPresetId(1)),
            ultimate: crate::builds::UltimateDefinitionId(6),
            passives: [
                crate::builds::PassiveDefinitionId(1),
                crate::builds::PassiveDefinitionId(3),
            ],
        },
    )
    .unwrap();
    let mut app = App::new();
    app.insert_resource(crate::timing::SimulationTick(7))
        .insert_resource(crate::map::PlayableBounds(crate::map::AxisAlignedMapRect {
            min: Vec2::splat(-1_000.0),
            max: Vec2::splat(1_000.0),
        }))
        .insert_resource(crate::movement::InputTuning::default())
        .init_resource::<crate::combat::NextCombatIds>()
        .init_resource::<crate::combat::CombatOutbox>()
        .init_resource::<crate::combat::CombatWorldEffectFacts>()
        .init_resource::<AbilityTelemetry>()
        .add_systems(Update, demolition::activate_demolition_strike);
    let fighter_entity = app
        .world_mut()
        .spawn((
            crate::protocol::Fighter,
            Position(Vec2::new(10.0, 20.0)),
            Rotation::IDENTITY,
            loadout,
            NetworkEntityId(9),
            crate::movement::InputFreshness {
                last_fresh_tick: Some(7),
            },
            AbilityState {
                charge: ULTIMATE_CHARGE_MAX,
                phase: AbilityPhase::Ready,
            },
            ActionState(crate::protocol::FighterInput::from_axes_with_aim_distance(
                Vec2::ZERO,
                Some(Vec2::X),
                Some(100.0),
                crate::protocol::FighterInput::ULTIMATE,
            )),
            crate::matchplay::ActiveCombatant,
            crate::matchplay::SpawnProtection {
                expires_at_tick: 99,
            },
        ))
        .id();

    app.update();

    assert_eq!(
        app.world().get::<AbilityState>(fighter_entity),
        Some(&AbilityState {
            charge: 0,
            phase: AbilityPhase::Charging,
        })
    );
    assert!(
        app.world()
            .get::<crate::matchplay::SpawnProtection>(fighter_entity)
            .is_none()
    );
    let facts = &app
        .world()
        .resource::<crate::combat::CombatWorldEffectFacts>()
        .0;
    assert_eq!(facts.len(), 1);
    assert!(matches!(
        facts[0].source,
        crate::combat::CombatWorldEffectSource::Ultimate {
            owner_network_entity_id: NetworkEntityId(9),
            ultimate_id: crate::builds::UltimateDefinitionId(6),
            ..
        }
    ));
    assert_eq!(
        facts[0].position,
        crate::combat::WorldPoint { x: 110.0, y: 20.0 }
    );
    assert_eq!(
        facts[0].effect,
        crate::combat::WorldEffectDefinition::DestroyMap { radius: 64.0 }
    );
    assert!(matches!(
        app.world()
            .resource::<crate::combat::CombatOutbox>()
            .0
            .as_slice(),
        [crate::combat::CombatCue::DemolitionStrikeActivated {
            source: NetworkEntityId(9),
            radius_milliunits: 64_000,
            ..
        }]
    ));

    // Holding the button cannot duplicate an accepted activation.
    app.update();
    assert_eq!(
        app.world()
            .resource::<crate::combat::CombatWorldEffectFacts>()
            .0
            .len(),
        1
    );
    assert_eq!(
        app.world()
            .resource::<crate::combat::CombatOutbox>()
            .0
            .len(),
        1
    );

    // A stale activation request is rejected before charge or world state changes.
    let demolition_loadout = app
        .world()
        .get::<crate::builds::ResolvedMatchLoadout>(fighter_entity)
        .unwrap()
        .clone();
    let stale = app
        .world_mut()
        .spawn((
            crate::protocol::Fighter,
            Position(Vec2::ZERO),
            Rotation::IDENTITY,
            demolition_loadout,
            NetworkEntityId(10),
            crate::movement::InputFreshness {
                last_fresh_tick: None,
            },
            AbilityState {
                charge: ULTIMATE_CHARGE_MAX,
                phase: AbilityPhase::Ready,
            },
            ActionState(crate::protocol::FighterInput::from_axes(
                Vec2::ZERO,
                Some(Vec2::X),
                crate::protocol::FighterInput::ULTIMATE,
            )),
            crate::matchplay::ActiveCombatant,
        ))
        .id();
    app.update();
    assert_eq!(
        app.world().get::<AbilityState>(stale).unwrap().charge,
        ULTIMATE_CHARGE_MAX
    );
    assert_eq!(
        app.world()
            .resource::<crate::combat::CombatWorldEffectFacts>()
            .0
            .len(),
        1
    );
}

#[test]
fn passive_arithmetic_is_deterministic_at_boundaries() {
    use crate::combat::WeaponEconomy;
    assert_eq!(apply_close_quarters_damage(100, 0.0), 115);
    assert_eq!(apply_close_quarters_damage(100, 240.0), 115);
    assert_eq!(apply_close_quarters_damage(100, 360.0), 100);
    assert_eq!(apply_close_quarters_damage(101, 360.0), 101);
    assert_eq!(apply_close_quarters_damage(100, 480.0), 85);
    assert_eq!(apply_close_quarters_damage(100, 10_000.0), 85);
    assert_eq!(apply_quick_cycle_ticks(0), 1);
    assert_eq!(apply_quick_cycle_ticks(1), 1);
    assert_eq!(apply_quick_cycle_ticks(60), 36);
    for economy in [
        WeaponEconomy::Magazine {
            capacity: 6,
            refill_ticks: 60,
        },
        WeaponEconomy::Charges {
            capacity: 2,
            recharge_ticks: 60,
        },
    ] {
        assert_eq!(apply_quick_cycle_ticks(economy.refill_ticks()), 36);
    }
    assert_eq!(apply_tenacity_ticks(0), 1);
    assert_eq!(apply_tenacity_ticks(1), 1);
    assert_eq!(apply_tenacity_ticks(45), 30);
}

#[test]
fn dash_interpolation_is_bounded_by_committed_segment_and_deadline() {
    let origin = Vec2::new(10.0, 20.0);
    assert_eq!(
        bounded_dash_endpoint(origin, Vec2::X, DASH_MAX_DISTANCE + 100.0),
        Some(origin + Vec2::X * DASH_MAX_DISTANCE)
    );
    assert_eq!(
        bounded_dash_endpoint(origin, Vec2::new(2.0, 0.0), 123.0),
        Some(origin + Vec2::X * 123.0)
    );
    assert_eq!(bounded_dash_endpoint(origin, Vec2::X, 0.5), None);
    assert_eq!(bounded_dash_endpoint(origin, Vec2::X, -1.0), None);
    assert_eq!(bounded_dash_endpoint(origin, Vec2::ZERO, 100.0), None);
    assert_eq!(bounded_dash_endpoint(origin, Vec2::X, f32::NAN), None);
    let endpoint = origin + Vec2::X * DASH_MAX_DISTANCE;
    assert_eq!(dash_position(origin, endpoint, 0), origin);
    assert_eq!(
        dash_position(origin, endpoint, 9),
        origin.lerp(endpoint, 0.5)
    );
    assert_eq!(dash_position(origin, endpoint, 18), endpoint);
    assert_eq!(dash_position(origin, endpoint, 100), endpoint);

    let contacts = stable_dash_contacts(
        Vec2::ZERO,
        Vec2::new(360.0, 0.0),
        &[NetworkEntityId(2)],
        (1_u64..=12).rev().map(|id| {
            (
                NetworkEntityId(id),
                Vec2::new(
                    f32::from(u16::try_from(id).unwrap()) * 20.0,
                    if id == 12 {
                        crate::movement::STANDARD_FIGHTER_RADIUS * 2.0 + 1.0
                    } else {
                        0.0
                    },
                ),
                id != 11,
            )
        }),
    );
    assert_eq!(
        contacts,
        [1, 3, 4, 5, 6, 7, 8].map(NetworkEntityId).to_vec()
    );
    let mut already_hit = vec![NetworkEntityId(2)];
    already_hit.extend(contacts);
    assert!(
        stable_dash_contacts(
            Vec2::ZERO,
            Vec2::new(360.0, 0.0),
            &already_hit,
            [(NetworkEntityId(9), Vec2::new(180.0, 0.0), true)],
        )
        .is_empty(),
        "the eight-target lifetime cap must prevent later contacts"
    );
}

#[test]
fn sentry_offsets_and_target_ties_are_stable() {
    assert_eq!(SENTRY_PLACEMENT_OFFSETS, [96, 88, 80, 72, 64, 56]);
    assert!((SENTRY_RADIUS - 20.0).abs() < f32::EPSILON);
    assert!((SENTRY_ACQUISITION_RANGE - 480.0).abs() < f32::EPSILON);
    assert_eq!(SENTRY_ACQUISITION_INTERVAL_TICKS, 6);
    assert_eq!(SENTRY_FIRE_INTERVAL_TICKS, 30);
    assert_eq!(SENTRY_LIFETIME_TICKS, 720);
    assert_eq!(SENTRY_MAXIMUM_HEALTH, 80);
    let mut attempted = Vec::new();
    let placement = first_clear_sentry_placement(Vec2::ZERO, Vec2::X, |candidate, radius| {
        attempted.push((candidate, radius));
        candidate.x <= 80.0
    });
    assert_eq!(placement, Some(Vec2::new(80.0, 0.0)));
    assert_eq!(
        attempted,
        [96.0, 88.0, 80.0]
            .map(|x| (Vec2::new(x, 0.0), SENTRY_RADIUS))
            .to_vec()
    );
    assert_eq!(
        first_clear_sentry_placement(Vec2::ZERO, Vec2::X, |_, _| false),
        None
    );
    assert_eq!(
        first_clear_sentry_placement(Vec2::ZERO, Vec2::ZERO, |_, _| true),
        None
    );
    let selected = stable_sentry_target([
        (NetworkEntityId(9), 100.0, true),
        (NetworkEntityId(3), 100.0, true),
        (NetworkEntityId(1), 25.0, false),
        (NetworkEntityId(2), 481.0_f32.powi(2), true),
    ]);
    assert_eq!(selected, Some(NetworkEntityId(3)));
    assert_eq!(
        stable_sentry_target([
            (NetworkEntityId(2), SENTRY_ACQUISITION_RANGE.powi(2), true),
            (
                NetworkEntityId(1),
                (SENTRY_ACQUISITION_RANGE + 0.01).powi(2),
                true,
            ),
            (NetworkEntityId(0), 1.0, false),
        ]),
        Some(NetworkEntityId(2))
    );
}

#[cfg(feature = "server")]
#[test]
fn sentry_objective_fallback_is_stable_and_does_not_replace_fighter_priority() {
    use crate::{
        combat::TeamId,
        map::{DamageableTargetIdentity, ModeAnchorId},
        matchplay::MatchId,
    };

    let farther = DamageableTargetIdentity::HeistSafe {
        match_id: MatchId(7),
        anchor_id: ModeAnchorId(9),
        defending_team: TeamId(1),
    };
    let stable_tie_winner = DamageableTargetIdentity::HeistSafe {
        match_id: MatchId(7),
        anchor_id: ModeAnchorId(3),
        defending_team: TeamId(1),
    };
    assert_eq!(
        sentry::stable_sentry_objective_target([
            (farther, 100.0, true),
            (stable_tie_winner, 100.0, true),
        ]),
        Some(stable_tie_winner)
    );
    assert_eq!(
        sentry::stable_sentry_objective_target([
            (stable_tie_winner, 1.0, false),
            (farther, (SENTRY_ACQUISITION_RANGE + 1.0).powi(2), true),
        ]),
        None
    );

    let fighter = stable_sentry_target([(NetworkEntityId(4), 400.0, true)]);
    assert_eq!(fighter, Some(NetworkEntityId(4)));
    assert!(
        fighter.is_some(),
        "the authority path must consult an objective only when no fighter target exists"
    );
}

#[cfg(feature = "server")]
#[test]
#[allow(clippy::too_many_lines)]
fn charge_observer_accepts_only_hostile_primary_damage_and_is_idempotent() {
    use crate::combat::{CombatOutcomeFacts, CombatOutcomeKind, CombatSourceKind, TeamId};
    use crate::protocol::Fighter;
    use bevy::prelude::*;
    let mut app = App::new();
    app.init_resource::<CombatOutcomeFacts>()
        .init_resource::<AbilityTelemetry>()
        .add_systems(Update, charge::observe_primary_damage_charge);
    let owner = NetworkEntityId(1);
    app.world_mut()
        .spawn((Fighter, owner, TeamId(0), AbilityState::default()));
    let mut received = damage_fact(
        6,
        CombatSourceKind::PrimaryWeapon,
        crate::combat::CombatTargetKind::Fighter,
        TeamId(1),
        TeamId(0),
    );
    received.source_network_id = Some(NetworkEntityId(2));
    received.target_network_id = owner;
    app.world_mut().resource_mut::<CombatOutcomeFacts>().0 = vec![
        damage_fact(
            1,
            CombatSourceKind::PrimaryWeapon,
            crate::combat::CombatTargetKind::Fighter,
            TeamId(0),
            TeamId(1),
        ),
        damage_fact(
            2,
            CombatSourceKind::Ultimate {
                ultimate_id: crate::builds::UltimateDefinitionId(1),
            },
            crate::combat::CombatTargetKind::Fighter,
            TeamId(0),
            TeamId(1),
        ),
        damage_fact(
            3,
            CombatSourceKind::PrimaryWeapon,
            crate::combat::CombatTargetKind::Fighter,
            TeamId(0),
            TeamId(0),
        ),
        damage_fact(
            4,
            CombatSourceKind::PrimaryWeapon,
            crate::combat::CombatTargetKind::Deployable,
            TeamId(0),
            TeamId(1),
        ),
        damage_fact(
            5,
            CombatSourceKind::Deployable {
                ultimate_id: crate::builds::UltimateDefinitionId(2),
                deployable_id: crate::builds::DeployableId(1),
            },
            crate::combat::CombatTargetKind::Fighter,
            TeamId(0),
            TeamId(1),
        ),
        received,
    ];
    app.update();
    let state = *app
        .world_mut()
        .query::<&AbilityState>()
        .single(app.world())
        .unwrap();
    assert_eq!(state.charge, 80);
    app.update();
    let state = *app
        .world_mut()
        .query::<&AbilityState>()
        .single(app.world())
        .unwrap();
    assert_eq!(state.charge, 80);
    assert_eq!(
        app.world()
            .resource::<AbilityTelemetry>()
            .charge_damage_dealt_by_owner[&owner],
        10
    );
    assert_eq!(
        app.world()
            .resource::<AbilityTelemetry>()
            .charge_damage_received_by_owner[&owner],
        10
    );
    app.world_mut().resource_mut::<CombatOutcomeFacts>().0 = vec![damage_fact(
        7,
        CombatSourceKind::PrimaryWeapon,
        crate::combat::CombatTargetKind::Fighter,
        TeamId(0),
        TeamId(1),
    )];
    let CombatOutcomeKind::Damage { amount } =
        &mut app.world_mut().resource_mut::<CombatOutcomeFacts>().0[0].kind
    else {
        unreachable!()
    };
    *amount = 190;
    app.update();
    assert_eq!(
        app.world()
            .resource::<AbilityTelemetry>()
            .first_full_charge_tick_by_owner[&owner],
        10
    );
}

#[cfg(feature = "server")]
#[test]
fn ability_telemetry_retains_aggregates_when_history_is_bounded() {
    let mut telemetry = AbilityTelemetry::default();
    for tick in 0..=telemetry::MAX_ABILITY_TELEMETRY_RECORDS as u64 {
        telemetry.record(AbilityTelemetryRecord {
            tick,
            owner_network_id: NetworkEntityId(1),
            kind: AbilityTelemetryKind::DashAccepted,
        });
    }
    assert_eq!(
        telemetry.records.len(),
        telemetry::MAX_ABILITY_TELEMETRY_RECORDS
    );
    assert_eq!(telemetry.dropped_records, 1);
    assert_eq!(telemetry.accepts, 1_025);
    assert_eq!(telemetry.dash_uses, 1_025);
}

#[cfg(feature = "server")]
#[test]
fn ability_telemetry_archives_typed_dash_sentry_passive_and_delay_evidence() {
    let owner = NetworkEntityId(7);
    let deployable = crate::builds::DeployableId(9);
    let passive = crate::builds::PassiveDefinitionId(4);
    let mut telemetry = AbilityTelemetry::default();
    let mut record = |tick, kind| {
        telemetry.record(AbilityTelemetryRecord {
            tick,
            owner_network_id: owner,
            kind,
        });
    };
    record(100, AbilityTelemetryKind::FullCharge);
    record(112, AbilityTelemetryKind::ActivationAttempt);
    record(112, AbilityTelemetryKind::SentryAccepted);
    record(112, AbilityTelemetryKind::SentrySpawned(deployable));
    record(130, AbilityTelemetryKind::SentryShot(deployable));
    record(
        135,
        AbilityTelemetryKind::SentryHit {
            deployable_id: deployable,
            damage: 25,
        },
    );
    record(140, AbilityTelemetryKind::AbilityDamage(25));
    record(140, AbilityTelemetryKind::AbilityTarget);
    record(
        150,
        AbilityTelemetryKind::PassiveModified {
            passive_id: passive,
            amount: 3,
        },
    );
    record(160, AbilityTelemetryKind::PassiveUnused(passive));
    record(
        172,
        AbilityTelemetryKind::SentryCleanup {
            deployable_id: deployable,
            reason: SentryCleanupReason::Expired,
            lifetime_ticks: 60,
        },
    );
    record(180, AbilityTelemetryKind::SelfCloakAccepted);
    record(
        210,
        AbilityTelemetryKind::SelfCloakEnded {
            reason: crate::combat::SelfCloakEndReason::Attack,
            active_ticks: 30,
        },
    );
    record(220, AbilityTelemetryKind::RevealScanAccepted { targets: 3 });

    assert_eq!(telemetry.ready_to_use_delay_ticks, 12);
    assert_eq!(telemetry.concurrent_sentry_high_water, 1);
    assert_eq!(
        telemetry.sentry_cleanup_reasons[&SentryCleanupReason::Expired],
        1
    );
    assert_eq!(telemetry.sentries[&deployable].hits, 1);
    assert_eq!(telemetry.sentries[&deployable].damage, 25);
    assert_eq!(telemetry.passive_modified_amounts[&passive], 3);
    assert_eq!(telemetry.passive_unused_triggers[&passive], 1);
    assert_eq!(telemetry.self_cloak_uses, 1);
    assert_eq!(telemetry.self_cloak_active_ticks, 30);
    assert_eq!(
        telemetry.self_cloak_end_reasons[&crate::combat::SelfCloakEndReason::Attack],
        1
    );
    assert_eq!(telemetry.reveal_scan_uses, 1);
    assert_eq!(telemetry.reveal_scan_targets, 3);

    let archived = telemetry.delta_since(&AbilityTelemetry::default(), 100);
    assert_eq!(
        archived.sentries[&deployable].cleanup_reason,
        Some(SentryCleanupReason::Expired)
    );
    assert_eq!(archived.concurrent_sentry_high_water, 1);
    assert_eq!(archived.ability_damage_by_owner[&owner], 25);
    assert_eq!(archived.ability_targets_by_owner[&owner], 1);
    assert_eq!(archived.self_cloak_active_ticks, 30);
    assert_eq!(archived.reveal_scan_targets, 3);
}

#[cfg(feature = "server")]
#[test]
fn deployable_allocator_never_wraps_or_reuses_an_id() {
    let mut ids = sentry::NextDeployableId(u64::MAX);
    assert_eq!(ids.allocate(), Some(crate::builds::DeployableId(u64::MAX)));
    assert_eq!(ids.allocate(), None);
}

#[cfg(feature = "server")]
#[test]
fn sentry_acquisition_and_fire_cadence_are_independent_and_stable() {
    let mut runtime = sentry::SentryRuntime::new(100);
    for tick in 100..106 {
        assert!(!runtime.begin_acquisition_if_due(tick));
    }
    assert!(runtime.begin_acquisition_if_due(106));
    runtime.set_fighter_target(stable_sentry_target([
        (NetworkEntityId(9), 100.0, true),
        (NetworkEntityId(3), 100.0, true),
    ]));
    assert_eq!(
        runtime.target(),
        Some(sentry::SentryTarget::Fighter(NetworkEntityId(3)))
    );
    assert!(!runtime.begin_acquisition_if_due(111));
    assert!(runtime.begin_acquisition_if_due(112));
    runtime.set_fighter_target(stable_sentry_target([
        (NetworkEntityId(3), 100.0, true),
        (NetworkEntityId(9), 100.0, true),
    ]));
    assert_eq!(
        runtime.target(),
        Some(sentry::SentryTarget::Fighter(NetworkEntityId(3)))
    );

    assert!(!runtime.fire_is_due(129));
    assert!(runtime.fire_is_due(130));
    runtime.record_fire(130);
    assert!(!runtime.fire_is_due(159));
    assert!(runtime.fire_is_due(160));
}

#[cfg(feature = "server")]
#[test]
#[allow(clippy::too_many_lines)]
fn passive_observer_rearms_adrenal_and_primes_quick_cycle_from_primary_facts() {
    use crate::combat::{
        AttackId, CombatEventId, CombatOutcomeFact, CombatOutcomeFacts, CombatOutcomeKind,
        CombatSourceKind, TeamId, WorldPoint,
    };
    use crate::protocol::{Fighter, PlayerId};
    use crate::timing::SimulationTick;
    use bevy::prelude::*;

    let builds = crate::builds::BuildCatalog::embedded().unwrap();
    let weapons = crate::combat::WeaponCatalog::embedded().unwrap();
    let fighter_definition = crate::combat::FighterDefinitions::default().entries[0];
    let runner = crate::builds::resolve_build_recipe(
        &builds,
        &weapons,
        &fighter_definition,
        crate::builds::BrawlerBuildRecipe {
            weapon: crate::builds::WeaponChoice::Preset(crate::combat::WeaponPresetId(1)),
            ultimate: crate::builds::UltimateDefinitionId(1),
            passives: [
                crate::builds::PassiveDefinitionId(1),
                crate::builds::PassiveDefinitionId(3),
            ],
        },
    )
    .unwrap();
    let controller = crate::builds::resolve_build_recipe(
        &builds,
        &weapons,
        &fighter_definition,
        crate::builds::BrawlerBuildRecipe {
            weapon: crate::builds::WeaponChoice::Preset(crate::combat::WeaponPresetId(3)),
            ultimate: crate::builds::UltimateDefinitionId(2),
            passives: [
                crate::builds::PassiveDefinitionId(5),
                crate::builds::PassiveDefinitionId(6),
            ],
        },
    )
    .unwrap();
    let mut app = App::new();
    app.insert_resource(SimulationTick(100))
        .init_resource::<CombatOutcomeFacts>()
        .init_resource::<AbilityTelemetry>()
        .add_systems(Update, passives::observe_passive_triggers);
    let runner_entity = app
        .world_mut()
        .spawn((
            Fighter,
            NetworkEntityId(1),
            TeamId(0),
            runner,
            crate::builds::PassiveRuntimeState::default(),
        ))
        .id();
    let controller_entity = app
        .world_mut()
        .spawn((
            Fighter,
            NetworkEntityId(2),
            TeamId(1),
            controller,
            crate::builds::PassiveRuntimeState::default(),
        ))
        .id();
    app.world_mut().resource_mut::<CombatOutcomeFacts>().0 = vec![
        CombatOutcomeFact {
            event_id: CombatEventId(1),
            tick: 100,
            attack_id: AttackId(1),
            source_kind: CombatSourceKind::PrimaryWeapon,
            source_player: Some(PlayerId(2)),
            source_network_id: Some(NetworkEntityId(2)),
            source_team: Some(TeamId(1)),
            target_network_id: NetworkEntityId(1),
            target_kind: crate::combat::CombatTargetKind::Fighter,
            target_team: TeamId(0),
            preset_id: None,
            recipe_fingerprint: None,
            position: WorldPoint { x: 0.0, y: 0.0 },
            engagement_distance: 200.0,
            kind: CombatOutcomeKind::Damage { amount: 10 },
        },
        CombatOutcomeFact {
            event_id: CombatEventId(2),
            tick: 100,
            attack_id: AttackId(2),
            source_kind: CombatSourceKind::PrimaryWeapon,
            source_player: Some(PlayerId(2)),
            source_network_id: Some(NetworkEntityId(2)),
            source_team: Some(TeamId(1)),
            target_network_id: NetworkEntityId(1),
            target_kind: crate::combat::CombatTargetKind::Fighter,
            target_team: TeamId(0),
            preset_id: None,
            recipe_fingerprint: None,
            position: WorldPoint { x: 0.0, y: 0.0 },
            engagement_distance: 200.0,
            kind: CombatOutcomeKind::Defeat,
        },
    ];
    app.update();
    let runner_state = *app
        .world()
        .get::<crate::builds::PassiveRuntimeState>(runner_entity)
        .unwrap();
    assert_eq!(runner_state.adrenaline_until_tick, Some(190));
    assert_eq!(runner_state.adrenaline_rearm_at_tick, Some(340));
    assert!(
        app.world()
            .get::<crate::builds::PassiveRuntimeState>(controller_entity)
            .unwrap()
            .quick_cycle_primed
    );
    assert_eq!(
        app.world()
            .resource::<AbilityTelemetry>()
            .passive_triggers
            .values()
            .sum::<u64>(),
        2
    );

    app.world_mut().resource_mut::<SimulationTick>().0 = 110;
    app.world_mut().resource_mut::<CombatOutcomeFacts>().0[0].event_id = CombatEventId(3);
    app.update();
    assert_eq!(
        app.world()
            .get::<crate::builds::PassiveRuntimeState>(runner_entity)
            .unwrap()
            .adrenaline_until_tick,
        Some(190)
    );
}

#[cfg(feature = "server")]
#[test]
fn production_ability_sets_have_an_explicit_authority_and_outcome_order() {
    use crate::{
        combat::CombatSet,
        gameplay::{GameplayPlugin, GameplaySet},
        matchplay::MatchSet,
    };
    use bevy::{prelude::*, time::TimeUpdateStrategy};

    #[derive(Resource, Default)]
    struct Trace(Vec<&'static str>);

    fn activation(mut trace: ResMut<Trace>) {
        trace.0.push("ability activation");
    }
    fn movement(mut trace: ResMut<Trace>) {
        trace.0.push("ability movement");
    }
    fn fire(mut trace: ResMut<Trace>) {
        trace.0.push("primary fire");
    }
    fn damage(mut trace: ResMut<Trace>) {
        trace.0.push("damage");
    }
    fn observe(mut trace: ResMut<Trace>) {
        trace.0.push("ability observers");
    }
    fn outcomes(mut trace: ResMut<Trace>) {
        trace.0.push("wipeout outcomes");
    }
    #[allow(clippy::needless_pass_by_value)]
    fn finalize(tick: Res<crate::timing::SimulationTick>, mut trace: ResMut<Trace>) {
        assert_eq!(tick.0, 0, "tick advancement must remain last");
        trace.0.push("finalize");
    }

    let mut app = App::new();
    app.add_plugins((MinimalPlugins, GameplayPlugin))
        .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
        .init_resource::<Trace>();
    configure_ability_schedule(&mut app);
    crate::matchplay::configure_match_schedule(&mut app);
    app.add_systems(
        FixedUpdate,
        (
            activation.in_set(AbilitySet::Activation),
            movement.in_set(AbilitySet::Movement),
            fire.in_set(GameplaySet::Fire),
        ),
    )
    .add_systems(
        FixedPostUpdate,
        (
            damage.in_set(CombatSet::Damage),
            observe.in_set(AbilitySet::ObserveOutcomes),
            outcomes.in_set(MatchSet::Outcomes),
            finalize
                .in_set(CombatSet::Finalize)
                .before(crate::gameplay::advance_simulation_tick),
        ),
    );
    crate::test_app::reject_owned_schedule_ambiguities(&mut app, FixedUpdate);
    crate::test_app::reject_owned_schedule_ambiguities(&mut app, FixedPostUpdate);

    app.update();
    app.update();

    assert_eq!(
        app.world().resource::<Trace>().0,
        vec![
            "ability activation",
            "ability movement",
            "primary fire",
            "damage",
            "ability observers",
            "wipeout outcomes",
            "finalize",
        ]
    );
    assert_eq!(app.world().resource::<crate::timing::SimulationTick>().0, 1);
}
