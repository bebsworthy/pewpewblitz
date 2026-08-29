use super::*;
#[cfg(feature = "balance-lab")]
#[test]
fn revised_catalog_loadout_keeps_build_identity_and_replicates_authoritative_values() {
    let canonical_builds = brawler::builds::BuildCatalog::embedded().unwrap();
    let canonical_weapons = brawler::combat::WeaponCatalog::embedded().unwrap();
    let fighter = FighterDefinitions::default().entries[0];
    let canonical = brawler::builds::resolve_saved_brawler_recipe(
        &canonical_builds,
        &canonical_weapons,
        &fighter,
        brawler::profiles::FighterProfileId(1),
        brawler::profiles::WeaponBaseId(1),
        brawler::builds::UltimateDefinitionId(1),
        [
            brawler::builds::PassiveDefinitionId(3),
            brawler::builds::PassiveDefinitionId(4),
        ],
    )
    .unwrap();

    let mut harness = Harness::new(1);
    harness.clients[0]
        .world_mut()
        .resource_mut::<ClientNetworkConfig>()
        .weapon_preset = Some(1);
    harness.step_until(|harness| harness.client_is_active(0) && harness.loadout_is_ready(0));
    let server_entity = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<Entity, (With<Fighter>, Without<TestDummy>)>();
        query.single(world).unwrap()
    };
    {
        let world = harness.server.world_mut();
        let mut builds = world.resource_mut::<brawler::builds::BuildCatalogResource>();
        builds.0.fighter_profiles.default.maximum_health = 222;
        builds.0.fighter_profiles.default.movement_speed = 444.0;
        builds.0.fighter_profiles.default.cold_capacity = 750;
        builds
            .0
            .fighter_profiles
            .default
            .cold_resistance_basis_points = 1_000;
        builds.0.fighter_profiles.lightweight.maximum_health = 222;
        builds.0.fighter_profiles.lightweight.movement_speed = 444.0;
        builds.0.fighter_profiles.reinforced.maximum_health = 222;
        builds.0.fighter_profiles.reinforced.movement_speed = 444.0;
        let mut weapons = world.resource_mut::<brawler::combat::WeaponCatalogResource>();
        weapons.0.presets[0]
            .configuration
            .recipe
            .fire_cooldown_ticks += 1;
    }
    let server_loadout = brawler::builds::resolve_saved_brawler_recipe(
        &harness
            .server
            .world()
            .resource::<brawler::builds::BuildCatalogResource>()
            .0,
        &harness
            .server
            .world()
            .resource::<brawler::combat::WeaponCatalogResource>()
            .0,
        &fighter,
        brawler::profiles::FighterProfileId(1),
        brawler::profiles::WeaponBaseId(1),
        brawler::builds::UltimateDefinitionId(1),
        [
            brawler::builds::PassiveDefinitionId(3),
            brawler::builds::PassiveDefinitionId(4),
        ],
    )
    .unwrap();
    harness
        .server
        .world_mut()
        .entity_mut(server_entity)
        .insert(server_loadout.clone());
    assert_eq!(server_loadout.identity, canonical.identity);
    assert_ne!(
        server_loadout.primary_weapon.recipe_fingerprint,
        canonical.primary_weapon.recipe_fingerprint
    );
    assert_eq!(server_loadout.fighter_stats.maximum_health, 222);
    assert_eq!(server_loadout.fighter_stats.movement_speed, 444.0);
    assert_eq!(server_loadout.fighter_stats.cold_capacity, 750);
    assert_eq!(
        server_loadout.fighter_stats.cold_resistance_basis_points,
        1_000
    );

    harness.step_until(|harness| {
        let entity = harness.controlled_entity(0);
        harness.clients[0]
            .world()
            .get::<brawler::builds::ResolvedMatchLoadout>(entity)
            .is_some_and(|loadout| loadout == &server_loadout)
    });
}

fn active_sentry_fixture() -> (Harness, Entity, brawler::abilities::SentryIdentity) {
    let mut harness = Harness::new_match(2);
    harness.clients[0]
        .world_mut()
        .resource_mut::<ClientNetworkConfig>()
        .weapon_preset = Some(3);
    harness.clients[1]
        .world_mut()
        .resource_mut::<ClientNetworkConfig>()
        .weapon_preset = Some(1);
    harness.step_until(|harness| {
        (0..2).all(|index| harness.client_is_active(index) && harness.loadout_is_ready(index))
    });
    harness.install_saved_brawler_loadout(0, 3, 2, [5, 6]);
    let match_id = {
        let world = harness.server.world_mut();
        let mut roots = world.query_filtered::<&MatchState, With<MatchRootMarker>>();
        roots.single(world).unwrap().match_id
    };
    for index in 0..2 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: 90,
                match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    harness.step_until(|harness| {
        matches!(
            {
                let world = harness.server.world_mut();
                let mut roots = world.query_filtered::<&MatchState, With<MatchRootMarker>>();
                roots.single(world).unwrap().phase
            },
            MatchPhase::Active { .. }
        )
    });
    let owner_player = harness.controlled_player_id(0);
    let owner_entity = {
        let world = harness.server.world_mut();
        let mut fighters = world.query_filtered::<(
            Entity,
            &PlayerId,
            &mut Position,
            &mut Rotation,
            &mut brawler::builds::AbilityState,
        ), With<Fighter>>();
        let mut owner = None;
        for (entity, player, mut position, mut rotation, mut ability) in fighters.iter_mut(world) {
            if *player == owner_player {
                owner = Some(entity);
                position.0 = Vec2::new(-300.0, 0.0);
                *rotation = Rotation::radians(0.0);
                *ability = brawler::builds::AbilityState {
                    charge: 1_000,
                    phase: brawler::builds::AbilityPhase::Ready,
                };
            } else {
                position.0 = Vec2::new(140.0, 0.0);
                *rotation = Rotation::radians(std::f32::consts::PI);
            }
        }
        owner.unwrap()
    };
    let entities: Vec<_> = {
        let world = harness.server.world_mut();
        let mut fighters = world.query_filtered::<Entity, With<Fighter>>();
        fighters.iter(world).collect()
    };
    for entity in entities {
        harness
            .server
            .world_mut()
            .entity_mut(entity)
            .remove::<brawler::matchplay::SpawnProtection>();
    }
    harness.arm_packet_impairment(0);
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, None, FighterInput::ULTIMATE),
    );
    harness.step_until(|harness| {
        let world = harness.server.world_mut();
        let mut sentries = world.query_filtered::<Entity, With<brawler::abilities::Sentry>>();
        sentries.iter(world).next().is_some()
    });
    harness.set_controlled_input(0, FighterInput::default());
    let identity = {
        let world = harness.server.world_mut();
        let mut sentries = world.query_filtered::<
            &brawler::abilities::SentryIdentity,
            With<brawler::abilities::Sentry>,
        >();
        *sentries.single(world).unwrap()
    };
    (harness, owner_entity, identity)
}

#[test]
fn sentry_projectile_travels_and_damages_a_hostile_fighter() {
    let (mut harness, _, identity) = active_sentry_fixture();
    let (sentry_position, target, initial_health) = {
        let world = harness.server.world_mut();
        let mut sentries = world.query_filtered::<
            (&Position, &brawler::abilities::SentryIdentity),
            With<brawler::abilities::Sentry>,
        >();
        let sentry_position = sentries.single(world).unwrap().0.0;
        let mut fighters = world.query_filtered::<
            (Entity, &TeamId, &CurrentHealth),
            (With<Fighter>, Without<TestDummy>),
        >();
        let (target, _, health) = fighters
            .iter(world)
            .find(|(_, team, _)| **team != identity.team_id)
            .unwrap();
        (sentry_position, target, health.0)
    };
    harness
        .server
        .world_mut()
        .entity_mut(target)
        // North of the sentry keeps a clear line of sight past the central destructible
        // block that now occupies the old origin-to-east corridor.
        .insert(Position(sentry_position + Vec2::new(0.0, 200.0)))
        .remove::<brawler::matchplay::SpawnProtection>();

    for _ in 0..120 {
        if harness
            .server
            .world()
            .get::<CurrentHealth>(target)
            .is_some_and(|health| health.0 < initial_health)
        {
            break;
        }
        harness.step();
    }

    let telemetry = harness
        .server
        .world()
        .resource::<brawler::abilities::AbilityTelemetry>();
    assert_eq!(
        harness
            .server
            .world()
            .get::<CurrentHealth>(target)
            .unwrap()
            .0,
        initial_health - 10,
        "sentry aggregate: {:?}",
        telemetry.sentries[&identity.deployable_id]
    );
    assert_eq!(telemetry.sentries[&identity.deployable_id].hits, 1);
    assert_eq!(telemetry.sentries[&identity.deployable_id].damage, 10);

    let combat = harness.server.world().resource::<CombatTelemetry>();
    assert!(combat.cues.iter().any(|cue| {
        matches!(
            cue,
            CombatCue::SentryFired {
                deployable_id,
                position,
                ..
            } if *deployable_id == identity.deployable_id
                && position.as_vec2() == sentry_position
        )
    }));
    assert!(combat.cues.iter().any(|cue| {
        matches!(
            cue,
            CombatCue::DamageApplied {
                source: brawler::combat::DamageSource::Deployable {
                    deployable_id,
                    ultimate_id,
                    ..
                },
                position,
                amount: 10,
                ..
            } if *deployable_id == identity.deployable_id
                && *ultimate_id == identity.ultimate_id
                && position.as_vec2() != Vec2::ZERO
        )
    }));
}
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the scenario proves one dash-and-sentry authority lifecycle across both replicated clients"
)]
fn dash_and_sentry_activation_are_server_owned_and_replicate_durable_state() {
    let mut harness = Harness::new_match(2);
    harness.clients[0]
        .world_mut()
        .resource_mut::<ClientNetworkConfig>()
        .weapon_preset = Some(1);
    harness.clients[1]
        .world_mut()
        .resource_mut::<ClientNetworkConfig>()
        .weapon_preset = Some(3);
    harness.step_until(|harness| {
        (0..2).all(|index| harness.client_is_active(index) && harness.loadout_is_ready(index))
    });
    harness.install_saved_brawler_loadout(1, 3, 2, [5, 6]);
    let match_id = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&MatchState, With<MatchRootMarker>>();
        query.single(world).unwrap().match_id
    };
    for index in 0..2 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: 80,
                match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    harness.step_until(|harness| {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&MatchState, With<MatchRootMarker>>();
        query
            .single(world)
            .is_ok_and(|state| matches!(state.phase, MatchPhase::Active { .. }))
    });
    let player_ids = [
        harness.controlled_player_id(0),
        harness.controlled_player_id(1),
    ];
    {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(
            &PlayerId,
            &mut brawler::builds::AbilityState,
            &mut Position,
            &mut Rotation,
        ), With<Fighter>>();
        for (player, mut ability, mut position, mut rotation) in query.iter_mut(world) {
            if *player == player_ids[0] {
                position.0 = Vec2::new(-300.0, 0.0);
                *rotation = Rotation::radians(0.0);
            }
            if *player == player_ids[1] {
                position.0 = Vec2::new(-140.0, 0.0);
                *rotation = Rotation::radians(std::f32::consts::PI);
            }
            *ability = brawler::builds::AbilityState {
                charge: 1_000,
                phase: brawler::builds::AbilityPhase::Ready,
            };
        }
        let entities: Vec<_> = world
            .query_filtered::<Entity, With<Fighter>>()
            .iter(world)
            .collect();
        for entity in entities {
            world
                .entity_mut(entity)
                .remove::<brawler::matchplay::SpawnProtection>();
        }
    }
    harness.arm_packet_impairment(0);
    for index in 0..2 {
        let buttons = if index == 0 {
            FighterInput::ULTIMATE | FighterInput::PRIMARY_FIRE
        } else {
            FighterInput::ULTIMATE
        };
        harness.set_controlled_input(index, FighterInput::from_axes(Vec2::ZERO, None, buttons));
    }
    for _ in 0..16 {
        harness.step();
    }
    let world = harness.server.world_mut();
    let mut sentries = world.query_filtered::<(&brawler::abilities::SentryIdentity, &CurrentHealth), With<brawler::abilities::Sentry>>();
    let sentry_health = sentries
        .iter(world)
        .map(|(_, health)| health.0)
        .collect::<Vec<_>>();
    assert_eq!(sentry_health, vec![45]);
    let mut fighters =
        world.query_filtered::<(&PlayerId, &brawler::builds::AbilityState), With<Fighter>>();
    let states: Vec<_> = fighters
        .iter(world)
        .map(|(player, state)| (*player, *state))
        .collect();
    assert!(states.iter().any(|(player, state)| *player == player_ids[0]
        && matches!(state.phase, brawler::builds::AbilityPhase::Dashing { .. })
        && state.charge == 0));
    assert!(states.iter().any(|(player, state)| *player == player_ids[1]
        && matches!(state.phase, brawler::builds::AbilityPhase::Deployed { .. })
        && state.charge == 0));
    let (dash_entity, dash_owner) = {
        let mut owners =
            world.query_filtered::<(Entity, &PlayerId, &NetworkEntityId), With<Fighter>>();
        owners
            .iter(world)
            .find(|(_, player, _)| **player == player_ids[0])
            .map(|(entity, _, network_id)| (entity, *network_id))
            .unwrap()
    };
    let mut deliveries = world.query::<&ReplicatedAttackSource>();
    assert!(!deliveries.iter(world).any(|source| {
        source.attack.owner_network_entity_id == dash_owner
            && matches!(source.attack.kind, CombatSourceKind::PrimaryWeapon)
    }));
    let mut health = world.query_filtered::<(&PlayerId, &CurrentHealth), With<Fighter>>();
    let fighter_health: Vec<_> = health
        .iter(world)
        .map(|(player, health)| (*player, health.0))
        .collect();
    assert!(
        fighter_health
            .iter()
            .any(|(player, health)| *player == player_ids[1] && *health == 965),
        "fighter_health={fighter_health:?}"
    );
    let sentry_owner = sentries
        .iter(world)
        .next()
        .map(|(identity, _)| identity.owner_player_id)
        .unwrap();
    let owner_entity = {
        let mut owners = world.query_filtered::<(Entity, &PlayerId), With<Fighter>>();
        owners
            .iter(world)
            .find(|(_, player)| **player == sentry_owner)
            .map(|(entity, _)| entity)
            .unwrap()
    };
    let scores_before = {
        let mut roots =
            world.query_filtered::<&brawler::matchplay::WipeoutState, With<MatchRootMarker>>();
        roots.single(world).unwrap().team_scores
    };
    world
        .entity_mut(owner_entity)
        .insert(brawler::matchplay::SpawnProtection {
            expires_at_tick: u64::MAX,
        });
    let _ = world;
    harness.set_controlled_input(0, FighterInput::default());
    harness.set_controlled_input(1, FighterInput::default());
    harness.step();
    let dash_position_before = *harness.server.world().get::<Position>(dash_entity).unwrap();
    harness.server.world_mut().entity_mut(dash_entity).insert((
        Defeated {
            event_id: CombatEventId(899_999),
        },
        brawler::builds::AbilityState {
            charge: 1_000,
            phase: brawler::builds::AbilityPhase::Ready,
        },
    ));
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(
            Vec2::ZERO,
            None,
            FighterInput::ULTIMATE | FighterInput::PRIMARY_FIRE,
        ),
    );
    for _ in 0..3 {
        harness.step();
    }
    assert_eq!(
        harness.server.world().get::<Position>(dash_entity),
        Some(&dash_position_before)
    );
    assert_eq!(
        harness
            .server
            .world()
            .resource::<brawler::abilities::AbilityTelemetry>()
            .dash_uses,
        1
    );
    harness
        .server
        .world_mut()
        .entity_mut(dash_entity)
        .remove::<Defeated>()
        .insert(brawler::builds::AbilityState::default());
    harness.set_controlled_input(0, FighterInput::default());
    harness.step();
    harness
        .server
        .world_mut()
        .entity_mut(owner_entity)
        .insert(brawler::builds::AbilityState {
            charge: 1_000,
            phase: brawler::builds::AbilityPhase::Ready,
        });
    harness.set_controlled_input(
        1,
        FighterInput::from_axes(Vec2::ZERO, None, FighterInput::ULTIMATE),
    );
    for _ in 0..2 {
        harness.step();
    }
    {
        let world = harness.server.world_mut();
        let mut sentries = world.query_filtered::<Entity, With<brawler::abilities::Sentry>>();
        assert_eq!(sentries.iter(world).count(), 1);
    }
    assert_eq!(
        harness
            .server
            .world()
            .resource::<brawler::abilities::AbilityTelemetry>()
            .sentry_uses,
        1
    );
    harness.set_controlled_input(1, FighterInput::default());
    harness.step();
    harness.step_until(|harness| {
        (0..2).all(|index| {
            let world = harness.clients[index].world_mut();
            let mut query = world.query_filtered::<
                (&CurrentHealth, &brawler::abilities::SentryDeadline),
                With<brawler::abilities::Sentry>,
            >();
            query.iter(world).any(|(health, _)| health.0 == 45)
        })
    });
    let impairment = harness.packet_impairment(0);
    assert!(impairment.injected);
    assert!(impairment.dropped_packets > 0);
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .resource::<brawler::abilities::AbilityTelemetry>()
            .sentry_shots
            >= 1
    });
    assert!(
        harness
            .server
            .world()
            .get::<brawler::matchplay::SpawnProtection>(owner_entity)
            .is_none(),
        "an attributed sentry shot must break owner spawn protection"
    );
    {
        let world = harness.server.world_mut();
        let mut deliveries = world.query::<&ReplicatedAttackSource>();
        assert!(
            deliveries
                .iter(world)
                .any(|source| matches!(source.attack.kind, CombatSourceKind::Deployable { .. }))
        );
    }
    harness
        .server
        .world_mut()
        .entity_mut(owner_entity)
        .insert(Defeated {
            event_id: CombatEventId(900_000),
        });
    harness.step_until(|harness| {
        let server_clear = {
            let world = harness.server.world_mut();
            let mut query = world.query_filtered::<Entity, With<brawler::abilities::Sentry>>();
            query.iter(world).next().is_none()
        };
        server_clear
            && (0..2).all(|index| {
                let world = harness.clients[index].world_mut();
                let mut query = world.query_filtered::<Entity, With<brawler::abilities::Sentry>>();
                query.iter(world).next().is_none()
            })
    });
    let world = harness.server.world_mut();
    let mut sentries = world.query_filtered::<Entity, With<brawler::abilities::Sentry>>();
    assert_eq!(sentries.iter(world).count(), 0);
    let mut deliveries = world.query::<&ReplicatedAttackSource>();
    assert!(
        !deliveries
            .iter(world)
            .any(|source| matches!(source.attack.kind, CombatSourceKind::Deployable { .. }))
    );
    let mut roots =
        world.query_filtered::<&brawler::matchplay::WipeoutState, With<MatchRootMarker>>();
    assert_eq!(roots.single(world).unwrap().team_scores, scores_before);
    let owner_ability = world
        .get::<brawler::builds::AbilityState>(owner_entity)
        .unwrap();
    assert!(matches!(
        (owner_ability.charge, owner_ability.phase),
        (1_000, brawler::builds::AbilityPhase::Ready)
            | (0..=999, brawler::builds::AbilityPhase::Charging)
    ));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the scenario compares expiry, destruction, disconnect, and impaired recovery for one sentry lifecycle"
)]
fn sentry_expiry_destruction_disconnect_and_impaired_recovery_converge() {
    let (mut expiry, owner, identity) = active_sentry_fixture();
    expiry.step_until(|harness| {
        let server_tick = harness.server.world().resource::<SimulationTick>().0;
        harness.packet_impairment(0).injected
            && (0..2).all(|index| {
                let world = harness.clients[index].world_mut();
                let mut sentries = world.query_filtered::<(
                    &brawler::abilities::SentryIdentity,
                    &CurrentHealth,
                    &brawler::abilities::SentryDeadline,
                ), With<brawler::abilities::Sentry>>();
                sentries.iter(world).any(|(replicated, health, deadline)| {
                    *replicated == identity
                        && health.0 == brawler::abilities::SENTRY_MAXIMUM_HEALTH
                        && deadline.expires_at_tick > server_tick
                })
            })
    });
    let authoritative_deadline = {
        let world = expiry.server.world_mut();
        let mut sentries = world.query_filtered::<(
            &brawler::abilities::SentryIdentity,
            &CurrentHealth,
            &brawler::abilities::SentryDeadline,
        ), With<brawler::abilities::Sentry>>();
        let (identity, health, deadline) = sentries.single(world).unwrap();
        (*identity, *health, *deadline)
    };
    {
        let world = expiry.clients[0].world_mut();
        let mut sentries = world.query_filtered::<Entity, With<brawler::abilities::Sentry>>();
        let entity = sentries.single(world).unwrap();
        world.entity_mut(entity).insert((
            brawler::abilities::SentryIdentity {
                owner_player_id: PlayerId(u64::MAX),
                owner_network_id: NetworkEntityId(u64::MAX),
                ..identity
            },
            CurrentHealth(0),
            brawler::abilities::SentryDeadline { expires_at_tick: 0 },
        ));
    }
    for _ in 0..12 {
        expiry.step();
    }
    {
        let world = expiry.server.world_mut();
        let mut sentries = world.query_filtered::<(
            &brawler::abilities::SentryIdentity,
            &CurrentHealth,
            &brawler::abilities::SentryDeadline,
        ), With<brawler::abilities::Sentry>>();
        let (identity, health, deadline) = sentries.single(world).unwrap();
        assert_eq!((*identity, *health, *deadline), authoritative_deadline);
    }
    let expiry_tick = expiry.server.world().resource::<SimulationTick>().0;
    {
        let world = expiry.server.world_mut();
        let mut sentries = world.query_filtered::<
            &mut brawler::abilities::SentryDeadline,
            With<brawler::abilities::Sentry>,
        >();
        sentries.single_mut(world).unwrap().expires_at_tick = expiry_tick;
    }
    expiry.step_until(|harness| {
        let server_clear = {
            let world = harness.server.world_mut();
            let mut sentries = world.query_filtered::<Entity, With<brawler::abilities::Sentry>>();
            sentries.iter(world).next().is_none()
        };
        server_clear
            && (0..2).all(|index| {
                let world = harness.clients[index].world_mut();
                let mut sentries =
                    world.query_filtered::<Entity, With<brawler::abilities::Sentry>>();
                sentries.iter(world).next().is_none()
            })
    });
    assert!(matches!(
        expiry
            .server
            .world()
            .get::<brawler::builds::AbilityState>(owner)
            .unwrap()
            .phase,
        brawler::builds::AbilityPhase::Charging
    ));

    let (mut destruction, _, _) = active_sentry_fixture();
    let scores_before = {
        let world = destruction.server.world_mut();
        let mut roots =
            world.query_filtered::<&brawler::matchplay::WipeoutState, With<MatchRootMarker>>();
        roots.single(world).unwrap().team_scores
    };
    {
        let world = destruction.server.world_mut();
        let mut sentries = world.query_filtered::<Entity, With<brawler::abilities::Sentry>>();
        let sentry = sentries.single(world).unwrap();
        world.entity_mut(sentry).insert(Defeated {
            event_id: CombatEventId(910_000),
        });
    }
    destruction.step_until(|harness| {
        let server_clear = {
            let world = harness.server.world_mut();
            let mut sentries = world.query_filtered::<Entity, With<brawler::abilities::Sentry>>();
            sentries.iter(world).next().is_none()
        };
        server_clear
            && (0..2).all(|index| {
                let world = harness.clients[index].world_mut();
                let mut sentries =
                    world.query_filtered::<Entity, With<brawler::abilities::Sentry>>();
                sentries.iter(world).next().is_none()
            })
    });
    let scores_after = {
        let world = destruction.server.world_mut();
        let mut roots =
            world.query_filtered::<&brawler::matchplay::WipeoutState, With<MatchRootMarker>>();
        roots.single(world).unwrap().team_scores
    };
    assert_eq!(scores_after, scores_before);

    let (mut completion, _, _) = active_sentry_fixture();
    let completion_tick = completion.server.world().resource::<SimulationTick>().0;
    {
        let world = completion.server.world_mut();
        let mut roots = world.query_filtered::<&mut MatchState, With<MatchRootMarker>>();
        roots.single_mut(world).unwrap().phase = MatchPhase::Active {
            ends_at_tick: completion_tick,
        };
    }
    completion.step_until(|harness| {
        let completed = {
            let world = harness.server.world_mut();
            let mut roots = world.query_filtered::<&MatchState, With<MatchRootMarker>>();
            matches!(
                roots.single(world).unwrap().phase,
                MatchPhase::Completed { .. }
            )
        };
        let server_clear = {
            let world = harness.server.world_mut();
            let mut sentries = world.query_filtered::<Entity, With<brawler::abilities::Sentry>>();
            sentries.iter(world).next().is_none()
        };
        completed && server_clear
    });
    completion.step_until(|harness| {
        (0..2).all(|index| {
            let world = harness.clients[index].world_mut();
            let mut sentries = world.query_filtered::<Entity, With<brawler::abilities::Sentry>>();
            sentries.iter(world).next().is_none()
        })
    });

    let (mut disconnect, _, _) = active_sentry_fixture();
    disconnect.clients[0].world_mut().trigger(Disconnect {
        entity: disconnect.client_entities[0],
    });
    disconnect.step_until(|harness| {
        let server_clear = {
            let world = harness.server.world_mut();
            let mut sentries = world.query_filtered::<Entity, With<brawler::abilities::Sentry>>();
            sentries.iter(world).next().is_none()
        };
        let peer_clear = {
            let world = harness.clients[1].world_mut();
            let mut sentries = world.query_filtered::<Entity, With<brawler::abilities::Sentry>>();
            sentries.iter(world).next().is_none()
        };
        server_clear && peer_clear
    });
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the scenario keeps dash collision truncation and the coupled primary-fire exclusion in one authority proof"
)]
fn dash_shape_cast_truncates_before_map_collision_and_blocks_primary_fire() {
    let mut harness = Harness::new_match(2);
    for client in &mut harness.clients {
        client
            .world_mut()
            .resource_mut::<ClientNetworkConfig>()
            .weapon_preset = Some(1);
    }
    harness.step_until(|harness| {
        (0..2).all(|index| harness.client_is_active(index) && harness.loadout_is_ready(index))
    });
    let match_id = {
        let world = harness.server.world_mut();
        let mut roots = world.query_filtered::<&MatchState, With<MatchRootMarker>>();
        roots.single(world).unwrap().match_id
    };
    for index in 0..2 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: 95,
                match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    harness.step_until(|harness| {
        matches!(
            {
                let world = harness.server.world_mut();
                let mut roots = world.query_filtered::<&MatchState, With<MatchRootMarker>>();
                roots.single(world).unwrap().phase
            },
            MatchPhase::Active { .. }
        )
    });
    let owner_player = harness.controlled_player_id(0);
    let (owner_entity, owner_network_id) = {
        let world = harness.server.world_mut();
        let mut fighters = world.query_filtered::<(
            Entity,
            &PlayerId,
            &NetworkEntityId,
            &mut Position,
            &mut Rotation,
            &mut brawler::builds::AbilityState,
        ), With<Fighter>>();
        let mut owner = None;
        let mut entities = Vec::new();
        for (entity, player, network_id, mut position, mut rotation, mut ability) in
            fighters.iter_mut(world)
        {
            entities.push(entity);
            if *player == owner_player {
                position.0 = Vec2::new(-200.0, 250.0);
                *rotation = Rotation::radians(0.0);
                *ability = brawler::builds::AbilityState {
                    charge: 1_000,
                    phase: brawler::builds::AbilityPhase::Ready,
                };
                owner = Some((entity, *network_id));
            } else {
                position.0 = Vec2::new(300.0, 250.0);
            }
        }
        for entity in entities {
            world
                .entity_mut(entity)
                .remove::<brawler::matchplay::SpawnProtection>();
        }
        owner.unwrap()
    };
    harness.server.world_mut().spawn((
        Collider::rectangle(20.0, 200.0),
        avian2d::prelude::RigidBody::Static,
        Position::from_xy(0.0, 250.0),
        CollisionLayers::new(
            brawler::movement::STATIC_MAP_LAYER,
            brawler::movement::FIGHTER_LAYER,
        ),
    ));
    harness.step();
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(
            Vec2::ZERO,
            None,
            FighterInput::ULTIMATE | FighterInput::PRIMARY_FIRE,
        ),
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .resource::<brawler::abilities::AbilityTelemetry>()
            .dash_uses
            == 1
            && matches!(
                harness
                    .server
                    .world()
                    .get::<brawler::builds::AbilityState>(owner_entity)
                    .unwrap()
                    .phase,
                brawler::builds::AbilityPhase::Charging
            )
    });
    let endpoint = harness
        .server
        .world()
        .get::<Position>(owner_entity)
        .unwrap()
        .0;
    let nearest_legal_center = -(10.0 + brawler::movement::STANDARD_FIGHTER_RADIUS);
    assert!(
        endpoint.x > -200.0 && endpoint.x <= nearest_legal_center + 0.5,
        "endpoint={endpoint:?}"
    );
    let world = harness.server.world_mut();
    let mut deliveries = world.query::<&ReplicatedAttackSource>();
    assert!(!deliveries.iter(world).any(|source| {
        source.attack.owner_network_entity_id == owner_network_id
            && matches!(source.attack.kind, CombatSourceKind::PrimaryWeapon)
    }));
}
