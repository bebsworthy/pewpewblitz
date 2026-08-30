use super::*;
use crate::builds::{AbilityPhase, AbilityState};
use crate::protocol::NetworkEntityId;
use bevy::prelude::Vec2;
#[cfg(feature = "server")]
use bevy::prelude::{App, Entity, FixedUpdate};

const fn test_charge_policy() -> crate::builds::UltimateChargePolicy {
    crate::builds::UltimateChargePolicy {
        maximum: 1_000,
        dealt_damage_multiplier: 5,
        received_damage_multiplier: 3,
    }
}

#[cfg(feature = "server")]
fn test_loadout(
    ultimate: crate::builds::UltimateDefinitionId,
    passives: [crate::builds::PassiveDefinitionId; 2],
) -> crate::builds::ResolvedMatchLoadout {
    let builds = crate::builds::BuildCatalog::embedded().unwrap();
    let weapons = crate::combat::WeaponCatalog::embedded().unwrap();
    crate::builds::resolve_build_recipe(
        &builds,
        &weapons,
        crate::builds::BrawlerBuildRecipe {
            weapon: crate::builds::WeaponChoice::Preset(crate::combat::WeaponPresetId(1)),
            ultimate,
            passives,
        },
    )
    .unwrap()
}

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
    let non_default = crate::builds::UltimateChargePolicy {
        maximum: 250,
        dealt_damage_multiplier: 2,
        received_damage_multiplier: 1,
    };
    assert_eq!(
        apply_charge(AbilityState::default(), 100, 25, non_default),
        AbilityState {
            charge: 225,
            phase: AbilityPhase::Charging,
        }
    );
    assert_eq!(
        apply_charge(AbilityState::default(), 1, 1, test_charge_policy()),
        AbilityState {
            charge: 8,
            phase: AbilityPhase::Charging,
        }
    );
    let state = apply_charge(AbilityState::default(), 100, 100, test_charge_policy());
    assert_eq!(state.charge, 800);
    let state = apply_charge(state, u16::MAX, u16::MAX, test_charge_policy());
    assert_eq!(
        state,
        AbilityState {
            charge: 1_000,
            phase: AbilityPhase::Ready
        }
    );
    assert_eq!(apply_charge(state, 1, 1, test_charge_policy()), state);
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
            apply_charge(active, u16::MAX, u16::MAX, test_charge_policy()),
            AbilityState {
                charge: test_charge_policy().maximum,
                phase,
            },
            "active execution preserves its phase while charge continues to accrue",
        );
        assert_eq!(
            settled_ability_phase(test_charge_policy().maximum, test_charge_policy().maximum),
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
    let loadout = crate::builds::resolve_build_recipe(
        &builds,
        &weapons,
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
                charge: test_charge_policy().maximum,
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
                charge: test_charge_policy().maximum,
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
        test_charge_policy().maximum
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
    let close_quarters = crate::builds::PassiveParameters::CloseQuarters {
        near_distance_milliunits: 240_000,
        far_distance_milliunits: 480_000,
        near_damage_basis_points: 11_500,
        far_damage_basis_points: 8_500,
    };
    assert_eq!(apply_close_quarters_damage(100, 0.0, close_quarters), 115);
    assert_eq!(apply_close_quarters_damage(100, 240.0, close_quarters), 115);
    assert_eq!(apply_close_quarters_damage(100, 360.0, close_quarters), 100);
    assert_eq!(apply_close_quarters_damage(101, 360.0, close_quarters), 101);
    assert_eq!(apply_close_quarters_damage(100, 480.0, close_quarters), 85);
    assert_eq!(
        apply_close_quarters_damage(100, 10_000.0, close_quarters),
        85
    );
    assert_eq!(apply_quick_cycle_ticks(0, 6_000), 1);
    assert_eq!(apply_quick_cycle_ticks(1, 6_000), 1);
    assert_eq!(apply_quick_cycle_ticks(60, 6_000), 36);
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
        assert_eq!(apply_quick_cycle_ticks(economy.refill_ticks(), 6_000), 36);
    }
    assert_eq!(apply_tenacity_ticks(0, 6_500), 1);
    assert_eq!(apply_tenacity_ticks(1, 6_500), 1);
    assert_eq!(apply_tenacity_ticks(45, 6_500), 30);
    assert_eq!(apply_quick_cycle_ticks(60, 5_000), 30);
    assert_eq!(apply_tenacity_ticks(45, 8_000), 36);
}

#[test]
fn dash_interpolation_is_bounded_by_committed_segment_and_deadline() {
    let maximum_distance = 360.0;
    let duration_ticks = 18;
    let maximum_targets = 8;
    let origin = Vec2::new(10.0, 20.0);
    assert_eq!(
        bounded_dash_endpoint(origin, Vec2::X, 500.0, 125.0),
        Some(origin + Vec2::X * 125.0),
        "a non-default authored maximum changes committed travel",
    );
    assert_eq!(
        dash_position(origin, origin + Vec2::X * 100.0, 2, 5),
        origin + Vec2::X * 40.0,
        "a non-default authored duration changes interpolation",
    );
    assert_eq!(
        bounded_dash_endpoint(origin, Vec2::X, maximum_distance + 100.0, maximum_distance,),
        Some(origin + Vec2::X * maximum_distance)
    );
    assert_eq!(
        bounded_dash_endpoint(origin, Vec2::new(2.0, 0.0), 123.0, maximum_distance),
        Some(origin + Vec2::X * 123.0)
    );
    assert_eq!(
        bounded_dash_endpoint(origin, Vec2::X, 0.5, maximum_distance),
        None
    );
    assert_eq!(
        bounded_dash_endpoint(origin, Vec2::X, -1.0, maximum_distance),
        None
    );
    assert_eq!(
        bounded_dash_endpoint(origin, Vec2::ZERO, 100.0, maximum_distance),
        None
    );
    assert_eq!(
        bounded_dash_endpoint(origin, Vec2::X, f32::NAN, maximum_distance),
        None
    );
    let endpoint = origin + Vec2::X * maximum_distance;
    assert_eq!(dash_position(origin, endpoint, 0, duration_ticks), origin);
    assert_eq!(
        dash_position(origin, endpoint, 9, duration_ticks),
        origin.lerp(endpoint, 0.5)
    );
    assert_eq!(
        dash_position(origin, endpoint, 18, duration_ticks),
        endpoint
    );
    assert_eq!(
        dash_position(origin, endpoint, 100, duration_ticks),
        endpoint
    );

    let contacts = stable_dash_contacts(
        Vec2::ZERO,
        Vec2::new(360.0, 0.0),
        crate::builds::BuildCatalog::embedded()
            .unwrap()
            .fighter_body
            .radius,
        &[NetworkEntityId(2)],
        (1_u64..=12).rev().map(|id| {
            (
                NetworkEntityId(id),
                Vec2::new(
                    f32::from(u16::try_from(id).unwrap()) * 20.0,
                    if id == 12 {
                        crate::builds::MAX_FIGHTER_BODY_RADIUS * 2.0 + 1.0
                    } else {
                        0.0
                    },
                ),
                id != 11,
            )
        }),
        maximum_targets,
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
            crate::builds::BuildCatalog::embedded()
                .unwrap()
                .fighter_body
                .radius,
            &already_hit,
            [(NetworkEntityId(9), Vec2::new(180.0, 0.0), true)],
            maximum_targets,
        )
        .is_empty(),
        "the eight-target lifetime cap must prevent later contacts"
    );
}

#[test]
fn sentry_offsets_and_target_ties_are_stable() {
    let placement_offsets = [96_000, 88_000, 80_000, 72_000, 64_000, 56_000];
    let radius = 20.0;
    let acquisition_range = 480.0;
    assert_eq!(
        first_clear_sentry_placement(
            Vec2::ZERO,
            Vec2::X,
            &[42_000, 21_000],
            7.0,
            |candidate, radius| {
                (candidate.x - 42.0).abs() < f32::EPSILON && (radius - 7.0).abs() < f32::EPSILON
            },
        ),
        Some(Vec2::new(42.0, 0.0)),
        "non-default authored placement geometry is consumed directly",
    );
    let mut attempted = Vec::new();
    let placement = first_clear_sentry_placement(
        Vec2::ZERO,
        Vec2::X,
        &placement_offsets,
        radius,
        |candidate, radius| {
            attempted.push((candidate, radius));
            candidate.x <= 80.0
        },
    );
    assert_eq!(placement, Some(Vec2::new(80.0, 0.0)));
    assert_eq!(
        attempted,
        [96.0, 88.0, 80.0]
            .map(|x| (Vec2::new(x, 0.0), radius))
            .to_vec()
    );
    assert_eq!(
        first_clear_sentry_placement(Vec2::ZERO, Vec2::X, &placement_offsets, radius, |_, _| {
            false
        },),
        None
    );
    assert_eq!(
        first_clear_sentry_placement(
            Vec2::ZERO,
            Vec2::ZERO,
            &placement_offsets,
            radius,
            |_, _| true,
        ),
        None
    );
    let selected = stable_sentry_target(
        [
            (NetworkEntityId(9), 100.0, true),
            (NetworkEntityId(3), 100.0, true),
            (NetworkEntityId(1), 25.0, false),
            (NetworkEntityId(2), 481.0_f32.powi(2), true),
        ],
        acquisition_range,
    );
    assert_eq!(selected, Some(NetworkEntityId(3)));
    assert_eq!(
        stable_sentry_target(
            [
                (NetworkEntityId(2), acquisition_range.powi(2), true),
                (NetworkEntityId(1), (acquisition_range + 0.01).powi(2), true,),
                (NetworkEntityId(0), 1.0, false),
            ],
            acquisition_range
        ),
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
        sentry::stable_sentry_objective_target(
            [(farther, 100.0, true), (stable_tie_winner, 100.0, true),],
            480.0
        ),
        Some(stable_tie_winner)
    );
    assert_eq!(
        sentry::stable_sentry_objective_target(
            [
                (stable_tie_winner, 1.0, false),
                (farther, 481.0_f32.powi(2), true),
            ],
            480.0
        ),
        None
    );

    let fighter = stable_sentry_target([(NetworkEntityId(4), 400.0, true)], 480.0);
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
    app.world_mut().spawn((
        Fighter,
        owner,
        TeamId(0),
        test_loadout(
            crate::builds::UltimateDefinitionId(1),
            [
                crate::builds::PassiveDefinitionId(1),
                crate::builds::PassiveDefinitionId(3),
            ],
        ),
        AbilityState::default(),
    ));
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
    let acquisition_interval = 6;
    let fire_interval = 30;
    let mut runtime = sentry::SentryRuntime::new(100, acquisition_interval, fire_interval);
    for tick in 100..106 {
        assert!(!runtime.begin_acquisition_if_due(tick, acquisition_interval));
    }
    assert!(runtime.begin_acquisition_if_due(106, acquisition_interval));
    runtime.set_fighter_target(stable_sentry_target(
        [
            (NetworkEntityId(9), 100.0, true),
            (NetworkEntityId(3), 100.0, true),
        ],
        480.0,
    ));
    assert_eq!(
        runtime.target(),
        Some(sentry::SentryTarget::Fighter(NetworkEntityId(3)))
    );
    assert!(!runtime.begin_acquisition_if_due(111, acquisition_interval));
    assert!(runtime.begin_acquisition_if_due(112, acquisition_interval));
    runtime.set_fighter_target(stable_sentry_target(
        [
            (NetworkEntityId(3), 100.0, true),
            (NetworkEntityId(9), 100.0, true),
        ],
        480.0,
    ));
    assert_eq!(
        runtime.target(),
        Some(sentry::SentryTarget::Fighter(NetworkEntityId(3)))
    );

    assert!(!runtime.fire_is_due(129));
    assert!(runtime.fire_is_due(130));
    runtime.record_fire(130, fire_interval);
    assert!(!runtime.fire_is_due(159));
    assert!(runtime.fire_is_due(160));
}

#[cfg(feature = "server")]
fn sentry_characterization_app() -> App {
    use crate::{
        combat::{
            CombatOutbox, CombatTelemetry, MeleeAttack, NextCombatIds, PendingDelivery,
            PendingPayload,
        },
        gameplay::GameplayPlugin,
    };
    use bevy::prelude::*;

    let mut app = App::new();
    app.add_plugins((MinimalPlugins, GameplayPlugin))
        .add_plugins(avian2d::prelude::PhysicsPlugins::default())
        .init_resource::<NextCombatIds>()
        .init_resource::<AbilityTelemetry>()
        .init_resource::<AbilityCleanupFacts>()
        .init_resource::<CombatTelemetry>()
        .init_resource::<CombatOutbox>()
        .add_message::<SentryCleanupRequest>()
        .add_message::<PendingPayload>()
        .add_message::<PendingDelivery>()
        .add_message::<MeleeAttack>();
    configure_ability_schedule(&mut app);
    app
}

#[cfg(feature = "server")]
fn canonical_sentry_tuning() -> sentry::ResolvedSentryTuning {
    let loadout = test_loadout(
        crate::builds::UltimateDefinitionId(2),
        [
            crate::builds::PassiveDefinitionId(1),
            crate::builds::PassiveDefinitionId(3),
        ],
    );
    sentry::resolve_sentry_tuning_for_test(&loadout.ultimate).unwrap()
}

#[cfg(feature = "server")]
fn spawn_sentry_owner(app: &mut App, position: Vec2) -> Entity {
    use crate::{
        combat::TeamId,
        matchplay::ActiveCombatant,
        protocol::{Fighter, PlayerId},
    };
    use avian2d::prelude::Position;

    app.world_mut()
        .spawn((
            Fighter,
            PlayerId(1),
            NetworkEntityId(1),
            TeamId(0),
            Position(position),
            test_loadout(
                crate::builds::UltimateDefinitionId(2),
                [
                    crate::builds::PassiveDefinitionId(1),
                    crate::builds::PassiveDefinitionId(3),
                ],
            ),
            AbilityState::default(),
            ActiveCombatant,
        ))
        .id()
}

#[cfg(feature = "server")]
fn spawn_sentry_target(app: &mut App, network_id: NetworkEntityId, position: Vec2) -> Entity {
    use crate::{
        combat::TeamId,
        matchplay::ActiveCombatant,
        protocol::{Fighter, PlayerId},
    };
    use avian2d::prelude::Position;

    app.world_mut()
        .spawn((
            Fighter,
            PlayerId(network_id.0),
            network_id,
            TeamId(1),
            Position(position),
            test_loadout(
                crate::builds::UltimateDefinitionId(2),
                [
                    crate::builds::PassiveDefinitionId(1),
                    crate::builds::PassiveDefinitionId(3),
                ],
            ),
            AbilityState::default(),
            ActiveCombatant,
        ))
        .id()
}

#[cfg(feature = "server")]
fn spawn_characterized_sentry(
    app: &mut App,
    tuning: sentry::ResolvedSentryTuning,
    runtime: sentry::SentryRuntime,
    expires_at_tick: u64,
) -> Entity {
    use crate::{
        builds::{DeployableId, UltimateDefinitionId},
        combat::TeamId,
        matchplay::MatchId,
        protocol::PlayerId,
    };
    use avian2d::prelude::Position;

    app.world_mut()
        .spawn((
            Sentry,
            SentryIdentity {
                deployable_id: DeployableId(7),
                owner_player_id: PlayerId(1),
                owner_network_id: NetworkEntityId(1),
                team_id: TeamId(0),
                ultimate_id: UltimateDefinitionId(2),
                match_id: MatchId(11),
            },
            SentryDeadline { expires_at_tick },
            runtime,
            tuning,
            Position(Vec2::ZERO),
        ))
        .id()
}

#[cfg(feature = "server")]
fn run_sentry_tick(app: &mut App, tick: u64) {
    app.world_mut()
        .resource_mut::<crate::timing::SimulationTick>()
        .0 = tick;
    app.world_mut().run_schedule(FixedUpdate);
}

#[cfg(feature = "server")]
#[test]
#[allow(clippy::too_many_lines)]
fn sentry_production_tick_preserves_exact_cadence_and_authored_projectile_recipe() {
    use crate::combat::{
        CombatCue, CombatOutbox, CombatSourceKind, ComposedProjectileRuntime, DamageFalloff,
        DeliveryMethod, FiringPattern, PayloadEffectDefinition, Projectile, ProjectileBody,
        RecipientPolicy, ReplicatedAttackSource, TargetSelection, WeaponEconomy,
        WeaponPresentationProfileId,
    };
    use bevy::prelude::*;

    let mut app = sentry_characterization_app();
    app.add_systems(
        FixedUpdate,
        sentry::tick_sentries.in_set(AbilitySet::Movement),
    );
    let tuning = canonical_sentry_tuning();
    spawn_sentry_owner(&mut app, Vec2::ZERO);
    let sentry_entity = spawn_characterized_sentry(
        &mut app,
        tuning.clone(),
        sentry::SentryRuntime::new(0, 6, 30),
        720,
    );
    spawn_sentry_target(&mut app, NetworkEntityId(2), Vec2::new(100.0, 0.0));
    crate::test_app::reject_owned_schedule_ambiguities(&mut app, FixedUpdate);
    crate::test_app::reject_owned_schedule_ambiguities(&mut app, FixedPostUpdate);
    crate::test_app::finalize(&mut app);

    run_sentry_tick(&mut app, 5);
    assert_eq!(
        app.world()
            .get::<sentry::SentryRuntime>(sentry_entity)
            .unwrap()
            .target(),
        None
    );
    run_sentry_tick(&mut app, 6);
    assert_eq!(
        app.world()
            .get::<sentry::SentryRuntime>(sentry_entity)
            .unwrap()
            .target(),
        Some(sentry::SentryTarget::Fighter(NetworkEntityId(2)))
    );
    run_sentry_tick(&mut app, 29);
    assert!(app.world().resource::<CombatOutbox>().0.is_empty());
    run_sentry_tick(&mut app, 30);

    let cues = &app.world().resource::<CombatOutbox>().0;
    assert!(matches!(
        cues.as_slice(),
        [CombatCue::SentryFired {
            tick: 30,
            owner: NetworkEntityId(1),
            deployable_id: crate::builds::DeployableId(7),
            target: Some(NetworkEntityId(2)),
            presentation_profile_id: WeaponPresentationProfileId(1),
            ..
        }]
    ));
    {
        let world = app.world_mut();
        let mut projectiles = world.query_filtered::<(
            &ReplicatedAttackSource,
            &ProjectileBody,
            &ComposedProjectileRuntime,
        ), With<Projectile>>();
        let (replicated, body, runtime) = projectiles.single(world).unwrap();
        assert!(matches!(
            replicated.attack.kind,
            CombatSourceKind::Deployable {
                ultimate_id: crate::builds::UltimateDefinitionId(2),
                deployable_id: crate::builds::DeployableId(7),
            }
        ));
        let recipe_bytes = postcard::to_allocvec(&(
            crate::combat::definitions::FINGERPRINT_FORMAT_VERSION,
            &runtime.recipe,
        ))
        .unwrap();
        assert_eq!(
            replicated.attack.recipe_fingerprint,
            crate::combat::WeaponRecipeFingerprint(crate::content::fnv1a64(&recipe_bytes).max(1))
        );
        assert_eq!(
            replicated.attack.presentation_profile_id,
            WeaponPresentationProfileId(1)
        );
        assert!((body.shape.bounding_radius() - 6.0).abs() < f32::EPSILON);
        assert_eq!(runtime.expires_at_tick, 62);
        assert!((runtime.maximum_range - 480.0).abs() < f32::EPSILON);
        assert_eq!(runtime.velocity, Vec2::new(900.0, 0.0));
        assert_eq!(
            runtime.recipe.economy,
            WeaponEconomy::Magazine {
                capacity: 1,
                refill_ticks: 30
            }
        );
        assert_eq!(runtime.recipe.fire_cooldown_ticks, 30);
        assert_eq!(runtime.recipe.firing, FiringPattern::Single);
        assert_eq!(
            runtime.recipe.delivery,
            DeliveryMethod::Straight {
                speed: 900.0,
                radius: 6.0,
                range: 480.0,
                lifetime_ticks: 32,
                muzzle_offset: 0.0,
            }
        );
        assert_eq!(runtime.recipe.payload_bundles.len(), 1);
        assert_eq!(
            runtime.recipe.payload_bundles[0].target,
            TargetSelection::Direct
        );
        assert_eq!(
            runtime.recipe.payload_bundles[0].effects,
            vec![PayloadEffectDefinition::Damage {
                amount: 10,
                falloff: DamageFalloff::None,
                recipients: RecipientPolicy::Hostiles,
            }]
        );
    }

    run_sentry_tick(&mut app, 59);
    assert_eq!(app.world().resource::<CombatOutbox>().0.len(), 1);
    run_sentry_tick(&mut app, 60);
    assert_eq!(app.world().resource::<CombatOutbox>().0.len(), 2);
}

#[cfg(feature = "server")]
#[test]
fn sentry_prefers_fighters_then_fires_only_at_a_live_hostile_objective() {
    use crate::{
        combat::{CombatCue, CombatOutbox, CurrentHealth, TeamId},
        map::{
            DamageableLifeState, DamageableTargetIdentity, MapDynamicGeneration, MapInstanceId,
            ModeAnchorId,
        },
        matchplay::{HeistSafe, MatchId},
    };
    use avian2d::prelude::Position;
    use bevy::prelude::*;

    let mut app = sentry_characterization_app();
    app.add_systems(
        FixedUpdate,
        sentry::tick_sentries.in_set(AbilitySet::Movement),
    );
    let tuning = canonical_sentry_tuning();
    spawn_sentry_owner(&mut app, Vec2::ZERO);
    let sentry_entity =
        spawn_characterized_sentry(&mut app, tuning, sentry::SentryRuntime::new(0, 6, 30), 720);
    let fighter = spawn_sentry_target(&mut app, NetworkEntityId(2), Vec2::new(100.0, 0.0));
    let objective = DamageableTargetIdentity::HeistSafe {
        match_id: MatchId(11),
        anchor_id: ModeAnchorId(4),
        defending_team: TeamId(1),
    };
    let objective_entity = app
        .world_mut()
        .spawn((
            Position(Vec2::new(80.0, 0.0)),
            objective,
            HeistSafe {
                match_id: MatchId(11),
                anchor_id: ModeAnchorId(4),
                defending_team: TeamId(1),
                generation: MapDynamicGeneration {
                    map_instance_id: MapInstanceId(1),
                    generation: 1,
                },
            },
            CurrentHealth(2_000),
            DamageableLifeState::Live,
        ))
        .id();
    crate::test_app::reject_owned_schedule_ambiguities(&mut app, FixedUpdate);
    crate::test_app::reject_owned_schedule_ambiguities(&mut app, FixedPostUpdate);
    crate::test_app::finalize(&mut app);

    run_sentry_tick(&mut app, 6);
    assert_eq!(
        app.world()
            .get::<sentry::SentryRuntime>(sentry_entity)
            .unwrap()
            .target(),
        Some(sentry::SentryTarget::Fighter(NetworkEntityId(2))),
        "a visible hostile fighter wins even when the live hostile objective is closer"
    );
    run_sentry_tick(&mut app, 30);
    app.world_mut()
        .entity_mut(fighter)
        .remove::<crate::matchplay::ActiveCombatant>();
    run_sentry_tick(&mut app, 36);
    assert_eq!(
        app.world()
            .get::<sentry::SentryRuntime>(sentry_entity)
            .unwrap()
            .target(),
        Some(sentry::SentryTarget::ModeObjective(objective))
    );
    run_sentry_tick(&mut app, 60);
    let cues: Vec<_> = app
        .world()
        .resource::<CombatOutbox>()
        .0
        .iter()
        .filter_map(|cue| match cue {
            CombatCue::SentryFired { tick, target, .. } => Some((*tick, *target)),
            _ => None,
        })
        .collect();
    assert_eq!(cues, vec![(30, Some(NetworkEntityId(2))), (60, None)]);

    *app.world_mut()
        .get_mut::<DamageableLifeState>(objective_entity)
        .unwrap() = DamageableLifeState::TerminalCommitted;
    run_sentry_tick(&mut app, 66);
    assert_eq!(
        app.world()
            .get::<sentry::SentryRuntime>(sentry_entity)
            .unwrap()
            .target(),
        None
    );
    run_sentry_tick(&mut app, 90);
    assert_eq!(app.world().resource::<CombatOutbox>().0.len(), 2);
}

#[cfg(feature = "server")]
#[test]
fn sentry_revalidates_visibility_and_target_liveness_before_firing() {
    use crate::{
        combat::CombatOutbox,
        concealment::TerrainConcealmentMembership,
        map::{MapInstanceId, MapPlacementId},
    };
    use bevy::prelude::*;

    fn custom_tuning() -> sentry::ResolvedSentryTuning {
        let mut loadout = test_loadout(
            crate::builds::UltimateDefinitionId(2),
            [
                crate::builds::PassiveDefinitionId(1),
                crate::builds::PassiveDefinitionId(3),
            ],
        );
        let crate::builds::UltimateParameters::Sentry {
            ref mut acquisition_interval_ticks,
            ..
        } = loadout.ultimate.parameters
        else {
            unreachable!()
        };
        *acquisition_interval_ticks = 7;
        sentry::resolve_sentry_tuning_for_test(&loadout.ultimate).unwrap()
    }

    let mut hidden_app = sentry_characterization_app();
    hidden_app.add_systems(
        FixedUpdate,
        sentry::tick_sentries.in_set(AbilitySet::Movement),
    );
    let tuning = custom_tuning();
    spawn_sentry_owner(&mut hidden_app, Vec2::ZERO);
    let sentry_entity = spawn_characterized_sentry(
        &mut hidden_app,
        tuning.clone(),
        sentry::SentryRuntime::new(0, 7, 30),
        720,
    );
    let target = spawn_sentry_target(&mut hidden_app, NetworkEntityId(2), Vec2::new(300.0, 0.0));
    crate::test_app::reject_owned_schedule_ambiguities(&mut hidden_app, FixedUpdate);
    crate::test_app::reject_owned_schedule_ambiguities(&mut hidden_app, FixedPostUpdate);
    crate::test_app::finalize(&mut hidden_app);
    for tick in [7, 14, 21, 28] {
        run_sentry_tick(&mut hidden_app, tick);
    }
    hidden_app
        .world_mut()
        .entity_mut(target)
        .insert(TerrainConcealmentMembership {
            map_instance_id: MapInstanceId(1),
            placement_id: MapPlacementId(1),
        });
    run_sentry_tick(&mut hidden_app, 30);
    assert!(hidden_app.world().resource::<CombatOutbox>().0.is_empty());
    assert_eq!(
        hidden_app
            .world()
            .get::<sentry::SentryRuntime>(sentry_entity)
            .unwrap()
            .target(),
        None,
        "a target that becomes concealed outside reveal proximity is rejected at fire time"
    );

    let mut inactive_app = sentry_characterization_app();
    inactive_app.add_systems(
        FixedUpdate,
        sentry::tick_sentries.in_set(AbilitySet::Movement),
    );
    spawn_sentry_owner(&mut inactive_app, Vec2::ZERO);
    let inactive_sentry = spawn_characterized_sentry(
        &mut inactive_app,
        tuning,
        sentry::SentryRuntime::new(0, 7, 30),
        720,
    );
    let inactive_target =
        spawn_sentry_target(&mut inactive_app, NetworkEntityId(2), Vec2::new(100.0, 0.0));
    crate::test_app::reject_owned_schedule_ambiguities(&mut inactive_app, FixedUpdate);
    crate::test_app::reject_owned_schedule_ambiguities(&mut inactive_app, FixedPostUpdate);
    crate::test_app::finalize(&mut inactive_app);
    for tick in [7, 14, 21, 28] {
        run_sentry_tick(&mut inactive_app, tick);
    }
    inactive_app
        .world_mut()
        .entity_mut(inactive_target)
        .remove::<crate::matchplay::ActiveCombatant>();
    run_sentry_tick(&mut inactive_app, 30);
    assert!(inactive_app.world().resource::<CombatOutbox>().0.is_empty());
    assert_eq!(
        inactive_app
            .world()
            .get::<sentry::SentryRuntime>(inactive_sentry)
            .unwrap()
            .target(),
        None,
        "a target that leaves active combat is rejected at fire time"
    );
}

#[cfg(feature = "server")]
fn characterized_sentry_source(
    deployable_id: crate::builds::DeployableId,
    attack_id: u64,
) -> crate::combat::AttackSource {
    crate::combat::AttackSource {
        kind: crate::combat::CombatSourceKind::Deployable {
            ultimate_id: crate::builds::UltimateDefinitionId(2),
            deployable_id,
        },
        attack_id: crate::combat::AttackId(attack_id),
        player_id: crate::protocol::PlayerId(1),
        owner_network_entity_id: NetworkEntityId(1),
        team_id: crate::combat::TeamId(0),
        recipe_fingerprint: crate::combat::WeaponRecipeFingerprint(91),
        presentation_profile_id: crate::combat::WeaponPresentationProfileId(1),
        legacy_compatibility: false,
        source_preset_id: None,
        origin: crate::combat::WorldPoint { x: 0.0, y: 0.0 },
        facing: 0.0,
    }
}

#[cfg(feature = "server")]
#[test]
#[allow(clippy::too_many_lines)]
fn sentry_cleanup_uses_reason_priority_and_purges_only_owned_work() {
    use crate::{
        builds::{AbilityPhase, DeployableId},
        combat::{
            CombatCue, CombatOutbox, MeleeAttack, PendingDelivery, PendingDeliveryKind,
            PendingPayload, ReplicatedAttackSource, WorldPoint,
        },
    };
    use bevy::prelude::*;

    let mut app = sentry_characterization_app();
    app.add_systems(
        FixedUpdate,
        (cleanup_requested_sentries, ApplyDeferred)
            .chain()
            .in_set(AbilitySet::Movement),
    );
    let owner = spawn_sentry_owner(&mut app, Vec2::new(4.0, 5.0));
    *app.world_mut().get_mut::<AbilityState>(owner).unwrap() = AbilityState {
        charge: 0,
        phase: AbilityPhase::Deployed {
            deployable_id: DeployableId(7),
            expires_at_tick: 720,
        },
    };
    let sentry_entity = spawn_characterized_sentry(
        &mut app,
        canonical_sentry_tuning(),
        sentry::SentryRuntime::new(0, 6, 30),
        720,
    );
    let owned_source = characterized_sentry_source(DeployableId(7), 41);
    let foreign_source = characterized_sentry_source(DeployableId(8), 42);
    let owned_delivery = app
        .world_mut()
        .spawn(ReplicatedAttackSource {
            attack: owned_source,
        })
        .id();
    let foreign_delivery = app
        .world_mut()
        .spawn(ReplicatedAttackSource {
            attack: foreign_source,
        })
        .id();
    let recipe = test_loadout(
        crate::builds::UltimateDefinitionId(2),
        [
            crate::builds::PassiveDefinitionId(1),
            crate::builds::PassiveDefinitionId(3),
        ],
    )
    .primary_weapon
    .recipe;
    let bundle = recipe.payload_bundles[0].clone();
    for source in [owned_source, foreign_source] {
        app.world_mut().write_message(PendingPayload {
            source,
            delivery_index: 0,
            bundle_index: 0,
            target: owner,
            target_network_id: NetworkEntityId(1),
            position: Vec2::ZERO,
            engagement_distance: 0.0,
            delivery_travel: 0.0,
            contact_fraction: 0.0,
            bundle: bundle.clone(),
        });
        app.world_mut().write_message(PendingDelivery {
            entity: None,
            source,
            delivery_index: 0,
            tick: 12,
            engagement_distance: 0.0,
            delivery_travel: 0.0,
            kind: PendingDeliveryKind::StraightImpact {
                target: None,
                position: WorldPoint { x: 0.0, y: 0.0 },
                normal: WorldPoint { x: 1.0, y: 0.0 },
                distance_band: crate::combat::DistanceBand::Close,
            },
            world_effects: Vec::new(),
        });
        app.world_mut().write_message(MeleeAttack {
            source,
            origin: Vec2::ZERO,
            facing: 0.0,
            tick: 12,
            recipe: recipe.clone(),
        });
    }
    app.world_mut().write_message(SentryCleanupRequest {
        deployable_id: DeployableId(7),
        reason: SentryCleanupReason::MatchRestarted,
        requested_at_tick: 12,
    });
    app.world_mut().write_message(SentryCleanupRequest {
        deployable_id: DeployableId(7),
        reason: SentryCleanupReason::Destroyed,
        requested_at_tick: 12,
    });
    crate::test_app::reject_owned_schedule_ambiguities(&mut app, FixedUpdate);
    crate::test_app::reject_owned_schedule_ambiguities(&mut app, FixedPostUpdate);
    crate::test_app::finalize(&mut app);

    run_sentry_tick(&mut app, 12);

    assert!(app.world().get_entity(sentry_entity).is_err());
    assert!(app.world().get_entity(owned_delivery).is_err());
    assert!(app.world().get_entity(foreign_delivery).is_ok());
    assert!(matches!(
        app.world().get::<AbilityState>(owner).unwrap().phase,
        AbilityPhase::Charging
    ));
    assert!(app.world().resource::<CombatOutbox>().0.iter().any(|cue| {
        matches!(
            cue,
            CombatCue::DeployableRemoved {
                deployable_id: DeployableId(7),
                reason: SentryCleanupReason::Destroyed,
                ..
            }
        )
    }));
    assert!(
        app.world()
            .resource::<AbilityTelemetry>()
            .records
            .iter()
            .any(|record| {
                matches!(
                    record.kind,
                    AbilityTelemetryKind::SentryCleanup {
                        deployable_id: DeployableId(7),
                        reason: SentryCleanupReason::Destroyed,
                        ..
                    }
                )
            })
    );
    let payloads: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<PendingPayload>>()
        .drain()
        .collect();
    let deliveries: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<PendingDelivery>>()
        .drain()
        .collect();
    let melee: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<MeleeAttack>>()
        .drain()
        .collect();
    assert_eq!(payloads.len(), 1);
    assert_eq!(deliveries.len(), 1);
    assert_eq!(melee.len(), 1);
    assert_eq!(payloads[0].source, foreign_source);
    assert_eq!(deliveries[0].source, foreign_source);
    assert_eq!(melee[0].source, foreign_source);
}

#[cfg(feature = "server")]
#[test]
fn sentry_cleanup_is_applied_before_a_due_fire_in_the_same_fixed_phase() {
    use crate::{
        builds::{AbilityPhase, DeployableId},
        combat::{CombatCue, CombatOutbox, Projectile},
    };
    use bevy::prelude::*;

    let mut app = sentry_characterization_app();
    app.add_systems(
        FixedUpdate,
        (
            publish_ability_cleanup_facts,
            request_sentry_lifecycle_cleanup,
            cleanup_requested_sentries,
            ApplyDeferred,
            sentry::tick_sentries,
        )
            .chain()
            .in_set(AbilitySet::Movement),
    );
    let owner = spawn_sentry_owner(&mut app, Vec2::ZERO);
    *app.world_mut().get_mut::<AbilityState>(owner).unwrap() = AbilityState {
        charge: 0,
        phase: AbilityPhase::Deployed {
            deployable_id: DeployableId(7),
            expires_at_tick: 30,
        },
    };
    let mut runtime = sentry::SentryRuntime::new(0, 6, 30);
    runtime.set_fighter_target(Some(NetworkEntityId(2)));
    let sentry_entity =
        spawn_characterized_sentry(&mut app, canonical_sentry_tuning(), runtime, 30);
    spawn_sentry_target(&mut app, NetworkEntityId(2), Vec2::new(100.0, 0.0));
    crate::test_app::reject_owned_schedule_ambiguities(&mut app, FixedUpdate);
    crate::test_app::reject_owned_schedule_ambiguities(&mut app, FixedPostUpdate);
    crate::test_app::finalize(&mut app);

    run_sentry_tick(&mut app, 30);

    assert!(app.world().get_entity(sentry_entity).is_err());
    assert!(app.world().resource::<CombatOutbox>().0.iter().any(|cue| {
        matches!(
            cue,
            CombatCue::DeployableRemoved {
                reason: SentryCleanupReason::Expired,
                ..
            }
        )
    }));
    assert!(
        !app.world()
            .resource::<CombatOutbox>()
            .0
            .iter()
            .any(|cue| { matches!(cue, CombatCue::SentryFired { .. }) })
    );
    let world = app.world_mut();
    let mut projectiles = world.query_filtered::<Entity, With<Projectile>>();
    assert_eq!(projectiles.iter(world).count(), 0);
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
    let runner = crate::builds::resolve_build_recipe(
        &builds,
        &weapons,
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

    {
        let mut runner_loadout = app
            .world_mut()
            .get_mut::<crate::builds::ResolvedMatchLoadout>(runner_entity)
            .unwrap();
        let adrenal = runner_loadout
            .passives
            .iter_mut()
            .find(|passive| passive.kind == crate::builds::PassiveKind::AdrenalResponse)
            .unwrap();
        adrenal.parameters = crate::builds::PassiveParameters::AdrenalResponse {
            duration_ticks: 45,
            rearm_ticks: 80,
            movement_bonus_basis_points: 1_500,
        };
    }
    app.world_mut().resource_mut::<SimulationTick>().0 = 340;
    app.world_mut().resource_mut::<CombatOutcomeFacts>().0[0].event_id = CombatEventId(4);
    app.update();
    let runner_state = app
        .world()
        .get::<crate::builds::PassiveRuntimeState>(runner_entity)
        .unwrap();
    assert_eq!(runner_state.adrenaline_until_tick, Some(385));
    assert_eq!(runner_state.adrenaline_rearm_at_tick, Some(420));
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

#[cfg(feature = "server")]
#[test]
fn cleanup_behavior_registration_is_additive_and_duplicate_safe() {
    use crate::gameplay::GameplayPlugin;
    use bevy::prelude::*;

    #[derive(Resource, Default)]
    struct CleanupProbe(u8);

    fn observe_cleanup(mut probe: ResMut<CleanupProbe>) {
        probe.0 = probe.0.saturating_add(1);
    }

    struct TestCleanupBehaviorPlugin;

    impl Plugin for TestCleanupBehaviorPlugin {
        fn build(&self, app: &mut App) {
            app.add_systems(
                AbilityCleanupSchedule,
                observe_cleanup.in_set(AbilityCleanupSet::RequestBehaviors),
            );
        }
    }

    let mut without_behavior = App::new();
    without_behavior
        .add_plugins((MinimalPlugins, GameplayPlugin, AbilityCorePlugin))
        .init_resource::<CleanupProbe>();
    run_ability_cleanup(without_behavior.world_mut());
    assert_eq!(without_behavior.world().resource::<CleanupProbe>().0, 0);

    let mut with_behavior = App::new();
    with_behavior
        .add_plugins((MinimalPlugins, GameplayPlugin, AbilityCorePlugin))
        .add_plugins(TestCleanupBehaviorPlugin)
        .init_resource::<CleanupProbe>();
    run_ability_cleanup(with_behavior.world_mut());
    assert_eq!(with_behavior.world().resource::<CleanupProbe>().0, 1);

    let duplicate = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_behavior.add_plugins(TestCleanupBehaviorPlugin);
    }));
    assert!(
        duplicate.is_err(),
        "Bevy must reject a duplicate behavior plugin"
    );
}
