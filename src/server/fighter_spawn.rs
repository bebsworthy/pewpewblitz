//! Shared authoritative fighter assembly for connected and server-hosted participants.

use crate::{
    builds::{AbilityState, MatchLoadoutProjection, PassiveRuntimeState, ResolvedMatchLoadout},
    combat::{
        ActiveEffects, AuthoritativeTick, HealthRecoveryState, SpawnState, resolved_fighter_runtime,
    },
    map::{MapInstanceId, SpawnAssignment},
    matchplay::{FighterDisplayName, MatchId, MatchMember, MatchParticipant, SpawnCandidate},
    movement::InputFreshness,
    protocol::{Fighter, NetworkEntityId, PlaceholderState, PlayerId},
};
use avian2d::prelude::{
    AngularVelocity, Collider, CollisionLayers, CustomPositionIntegration, LinearVelocity,
    Position, RigidBody, Rotation,
};
use bevy::prelude::*;
use lightyear::prelude::{InterpolationTarget, NetworkTarget, Replicate};

pub(super) struct AuthoritativeFighterSpawnSpec {
    pub(super) player_id: PlayerId,
    pub(super) network_entity_id: NetworkEntityId,
    pub(super) team: crate::combat::TeamId,
    pub(super) display_name: String,
    pub(super) loadout: ResolvedMatchLoadout,
    pub(super) spawn: SpawnCandidate,
    pub(super) match_id: MatchId,
    pub(super) map_instance_id: MapInstanceId,
    pub(super) ready: bool,
}

pub(super) fn spawn_authoritative_fighter(
    commands: &mut Commands,
    fighter_body: crate::builds::FighterBody,
    spec: AuthoritativeFighterSpawnSpec,
) -> Entity {
    let projection = MatchLoadoutProjection::new(&spec.loadout, fighter_body);
    let (fighter_definition, team, health, weapon) = resolved_fighter_runtime(
        spec.team,
        &spec.loadout.fighter_stats,
        &spec.loadout.primary_weapon,
    );
    let entity = commands
        .spawn((
            Fighter,
            spec.player_id,
            spec.network_entity_id,
            PlaceholderState {
                spawn_slot: u64::from(spec.spawn.id.0),
            },
            fighter_definition,
            team,
            health,
            weapon,
            spec.loadout.identity,
            spec.loadout,
            projection,
            AbilityState::default(),
            PassiveRuntimeState::default(),
            ActiveEffects::default(),
            AuthoritativeTick::default(),
        ))
        .id();
    commands.entity(entity).insert((
        HealthRecoveryState::default(),
        SpawnState {
            position: spec.spawn.position,
            facing: spec.spawn.facing,
        },
        Position::from_xy(spec.spawn.position.x, spec.spawn.position.y),
        Rotation::radians(spec.spawn.facing),
        LinearVelocity::default(),
        AngularVelocity::default(),
        FighterDisplayName(spec.display_name),
        MatchParticipant {
            match_id: spec.match_id,
            ready: spec.ready,
            restart_ready: false,
        },
        MatchMember(spec.match_id),
        SpawnAssignment {
            map_instance_id: spec.map_instance_id,
            spawn_point_id: spec.spawn.id,
        },
        Collider::circle(fighter_body.radius),
        RigidBody::Kinematic,
        CustomPositionIntegration,
        CollisionLayers::new(
            crate::movement::FIGHTER_LAYER,
            avian2d::prelude::LayerMask::NONE,
        ),
        InputFreshness::default(),
    ));
    commands.entity(entity).insert((
        Replicate::to_clients(NetworkTarget::All),
        InterpolationTarget::to_clients(NetworkTarget::All),
    ));
    entity
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::world::CommandQueue;
    use lightyear::prelude::input::native::ActionState;
    use lightyear::prelude::{ControlledBy, Lifetime};

    fn spawn_spec(
        loadout: ResolvedMatchLoadout,
        player: u64,
        ready: bool,
    ) -> AuthoritativeFighterSpawnSpec {
        AuthoritativeFighterSpawnSpec {
            player_id: PlayerId(player),
            network_entity_id: NetworkEntityId(player),
            team: crate::combat::TeamId(u8::try_from(player % 2).unwrap()),
            display_name: format!("Fighter {player}"),
            loadout,
            spawn: SpawnCandidate {
                id: crate::map::SpawnPointId(u16::try_from(player).unwrap()),
                position: Vec2::new(f32::from(u16::try_from(player).unwrap()), 2.0),
                facing: 0.5,
            },
            match_id: MatchId(9),
            map_instance_id: MapInstanceId(3),
            ready,
        }
    }

    #[test]
    fn common_assembly_stays_controller_neutral_and_callers_add_ownership() {
        let builds = crate::builds::BuildCatalog::embedded().unwrap();
        let weapons = crate::combat::WeaponCatalog::embedded().unwrap();
        let connected_loadout =
            crate::builds::resolve_direct_diagnostic_loadout(&builds, &weapons, 1).unwrap();
        let practice_loadout =
            crate::builds::resolve_direct_diagnostic_loadout(&builds, &weapons, 2).unwrap();
        let fighter_body = builds.fighter_body;
        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let (connected, practice);
        {
            let mut commands = Commands::new(&mut queue, &world);
            let owner = commands.spawn_empty().id();
            connected = spawn_authoritative_fighter(
                &mut commands,
                fighter_body,
                spawn_spec(connected_loadout, 1, false),
            );
            commands.entity(connected).insert(ControlledBy {
                owner,
                lifetime: Lifetime::SessionBased,
            });
            practice = spawn_authoritative_fighter(
                &mut commands,
                fighter_body,
                spawn_spec(practice_loadout, 2, true),
            );
            commands.entity(practice).insert((
                ActionState::<crate::protocol::FighterInput>::default(),
                crate::bots::PracticeBotController::new(2),
            ));
        }
        queue.apply(&mut world);

        for entity in [connected, practice] {
            let fighter = world.entity(entity);
            assert!(fighter.contains::<Fighter>());
            assert!(fighter.contains::<crate::combat::CurrentHealth>());
            assert!(fighter.contains::<crate::combat::WeaponState>());
            assert!(fighter.contains::<crate::builds::ResolvedFighterStats>());
            assert!(fighter.contains::<crate::builds::FighterBody>());
            assert!(fighter.contains::<crate::combat::ResolvedWeapon>());
            assert!(fighter.contains::<AbilityState>());
            assert!(fighter.contains::<PassiveRuntimeState>());
            assert!(fighter.contains::<ActiveEffects>());
            assert!(fighter.contains::<HealthRecoveryState>());
            assert!(fighter.contains::<SpawnState>());
            assert!(fighter.contains::<Collider>());
            assert!(fighter.contains::<MatchParticipant>());
            assert!(fighter.contains::<SpawnAssignment>());
            assert!(fighter.contains::<Replicate>());
            assert!(fighter.contains::<InterpolationTarget>());
            assert!(fighter.contains::<InputFreshness>());
        }

        let connected_ref = world.entity(connected);
        assert!(connected_ref.contains::<ControlledBy>());
        assert!(!connected_ref.contains::<ActionState<crate::protocol::FighterInput>>());
        assert!(!connected_ref.contains::<crate::bots::PracticeBotController>());
        assert!(!connected_ref.get::<MatchParticipant>().unwrap().ready);

        let practice_ref = world.entity(practice);
        assert!(!practice_ref.contains::<ControlledBy>());
        assert!(practice_ref.contains::<ActionState<crate::protocol::FighterInput>>());
        assert!(practice_ref.contains::<crate::bots::PracticeBotController>());
        assert!(practice_ref.get::<MatchParticipant>().unwrap().ready);
    }
}
