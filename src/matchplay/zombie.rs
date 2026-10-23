//! Cooperative Zombie pressure, hostile-NPC combat, and match-scoped boost authority.

use crate::protocol::NetworkEntityId;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub const ZOMBIE_RULES_REVISION: u16 = 1;
pub const MAX_LIVE_ZOMBIES: u8 = 24;
pub const MAX_BOOST_STACKS: u8 = 20;
pub const MAX_LIVE_BOOST_PICKUPS: usize = 96;
#[cfg(feature = "server")]
pub const HOSTILE_NPC_NETWORK_NAMESPACE: u64 = 0x4000_0000_0000_0000;
#[cfg(feature = "server")]
pub const BOOST_PICKUP_NETWORK_NAMESPACE: u64 = 0x2000_0000_0000_0000;
#[allow(
    dead_code,
    reason = "client-only role builds retain the shared HostileNpc source conversion contract"
)]
pub(crate) const ZOMBIE_DEFINITION: HostileNpcDefinitionId = HostileNpcDefinitionId(1);

#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct ZombieRules {
    pub pressure_stage_count: u8,
    pub initial_target_live: u8,
    pub target_live_step: u8,
    pub spawn_interval_ticks: u64,
    pub spawn_interval_step_ticks: u64,
    pub batch_size: u8,
    pub maximum_live: u8,
    pub total_spawn_limit: u16,
    pub player_exclusion_radius: f32,
    pub zombie_health: u16,
    pub zombie_radius: f32,
    pub movement_speed_per_second: f32,
    pub melee_range: f32,
    pub melee_damage: u16,
    pub melee_windup_ticks: u64,
    pub melee_recovery_ticks: u64,
    pub target_commitment_ticks: u64,
    pub clear_grace_ticks: u64,
    pub boost_stack_limit: u8,
    pub ambient_drop_budget: u16,
    pub ambient_drop_every_defeats: u16,
    pub team_drop_every: u16,
    pub pickup_radius: f32,
}

impl Default for ZombieRules {
    fn default() -> Self {
        Self {
            pressure_stage_count: 4,
            initial_target_live: 8,
            target_live_step: 5,
            spawn_interval_ticks: 120,
            spawn_interval_step_ticks: 20,
            batch_size: 4,
            maximum_live: MAX_LIVE_ZOMBIES,
            total_spawn_limit: 240,
            player_exclusion_radius: 180.0,
            zombie_health: 240,
            zombie_radius: 18.0,
            movement_speed_per_second: 76.0,
            melee_range: 44.0,
            melee_damage: 28,
            melee_windup_ticks: 30,
            melee_recovery_ticks: 42,
            target_commitment_ticks: 90,
            clear_grace_ticks: 3_600,
            boost_stack_limit: 10,
            ambient_drop_budget: 18,
            ambient_drop_every_defeats: 3,
            team_drop_every: 4,
            pickup_radius: 30.0,
        }
    }
}

impl ZombieRules {
    pub fn validate(self) -> Result<Self, &'static str> {
        if self.pressure_stage_count == 0
            || self.initial_target_live == 0
            || self.batch_size == 0
            || self.maximum_live == 0
            || self.maximum_live > MAX_LIVE_ZOMBIES
            || self.total_spawn_limit < u16::from(self.maximum_live)
            || self.spawn_interval_ticks == 0
            || self.melee_damage == 0
            || self.melee_windup_ticks == 0
            || self.melee_recovery_ticks == 0
            || self.target_commitment_ticks == 0
            || self.clear_grace_ticks == 0
            || self.boost_stack_limit == 0
            || self.boost_stack_limit > MAX_BOOST_STACKS
            || self.ambient_drop_every_defeats == 0
            || self.team_drop_every == 0
            || usize::from(self.ambient_drop_budget)
                .saturating_add(2)
                .saturating_add(usize::from(self.boost_stack_limit).saturating_mul(3))
                > MAX_LIVE_BOOST_PICKUPS
        {
            return Err("invalid bounded Zombie rules");
        }
        for value in [
            self.player_exclusion_radius,
            self.zombie_radius,
            self.movement_speed_per_second,
            self.melee_range,
            self.pickup_radius,
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err("invalid Zombie geometry");
            }
        }
        if self.melee_range <= self.zombie_radius {
            return Err("Zombie melee range must exceed its body radius");
        }
        Ok(self)
    }

    #[must_use]
    pub fn target_live_for_stage(self, stage: u8) -> u8 {
        self.initial_target_live
            .saturating_add(self.target_live_step.saturating_mul(stage))
            .min(self.maximum_live)
    }

    #[must_use]
    pub fn spawn_interval_for_stage(self, stage: u8) -> u64 {
        self.spawn_interval_ticks
            .saturating_sub(
                self.spawn_interval_step_ticks
                    .saturating_mul(u64::from(stage)),
            )
            .max(15)
    }
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostileNpc;

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostileNpcIdentity {
    pub match_id: super::MatchId,
    pub spawn_id: u32,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostileNpcDefinitionId(pub u16);

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZombiePhase {
    Pursuing {
        target: Option<NetworkEntityId>,
        reconsider_at_tick: u64,
    },
    Windup {
        target: NetworkEntityId,
        contact_at_tick: u64,
    },
    Recovery {
        ready_at_tick: u64,
    },
}

impl Default for ZombiePhase {
    fn default() -> Self {
        Self::Pursuing {
            target: None,
            reconsider_at_tick: 0,
        }
    }
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BoostStacks(pub u8);

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BoostPickup;

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoostPickupIdentity {
    pub match_id: super::MatchId,
    pub pickup_id: u32,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoostPickupSource {
    Chest,
    Zombie,
    PlayerDeath,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoostGrantScope {
    Individual,
    Team,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoostPickupAvailableAtTick(pub u64);

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ZombieModeState {
    pub match_id: super::MatchId,
    pub active_started_at_tick: u64,
    pub spawning_ends_at_tick: u64,
    pub hard_deadline_tick: u64,
    pub pressure_stage: u8,
    pub next_spawn_at_tick: u64,
    pub live_zombies: u8,
    pub total_spawned: u16,
    pub total_defeated: u16,
    pub living_players: u8,
    pub ambient_drops_remaining: u16,
    pub spawning_complete: bool,
    pub last_brawler_standing: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ZombieSummary {
    pub total_spawned: u16,
    pub total_defeated: u16,
    pub peak_live: u8,
    pub pressure_stage_reached: u8,
}

/// Round-half-up basis-point scaling used by both health and player-owned damage.
#[must_use]
pub fn scale_boosted_value(base: u16, stacks: u8) -> u16 {
    let percent = 100_u32.saturating_add(u32::from(stacks).saturating_mul(5));
    u16::try_from(u32::from(base).saturating_mul(percent).saturating_add(50) / 100)
        .unwrap_or(u16::MAX)
}

#[must_use]
pub fn effective_maximum_health(base: u16, stacks: BoostStacks) -> u16 {
    scale_boosted_value(base, stacks.0)
}

#[cfg(feature = "server")]
mod authority {
    #[allow(clippy::wildcard_imports)]
    use super::*;
    use crate::{
        combat::{
            ActiveEffects, CombatCue, CombatOutbox, CombatOutcomeFact, CombatOutcomeFacts,
            CombatOutcomeKind, CombatSourceKind, CombatTargetKind, CurrentHealth, DamageSource,
            Defeated, DistanceBand, NextCombatIds, TeamId, WorldPoint,
        },
        map::{MapCatalogResource, MapDynamicState, MapRoot, ResolvedMap},
        matchplay::{
            ActiveCombatant, MatchMember, MatchOutcomeDiagnostics, MatchParticipant, MatchPhase,
            MatchRestartSet, MatchRoot, MatchSet, MatchState, ModeOutcomeCause, ModeRuleOutcome,
            PendingMatchRestart, PendingModeRuleOutcome, SpawnProtection, offer_mode_rule_outcome,
            prepare_mode_rule_facts,
        },
        protocol::{Fighter, NetworkEntityId},
        timing::SimulationTick,
    };
    use avian2d::prelude::{Collider, CollisionLayers, Position, RigidBody, Rotation};
    use lightyear::prelude::{NetworkTarget, Replicate};
    use std::collections::{BTreeMap, BTreeSet};

    #[derive(Resource, Default)]
    struct NextZombieIds {
        spawn: u32,
        pickup: u32,
    }

    impl NextZombieIds {
        fn next_spawn(&mut self) -> Option<u32> {
            let value = self.spawn.checked_add(1)?;
            self.spawn = value;
            Some(value)
        }

        fn next_pickup(&mut self) -> Option<u32> {
            let value = self.pickup.checked_add(1)?;
            self.pickup = value;
            Some(value)
        }

        fn clear(&mut self) {
            *self = Self::default();
        }
    }

    #[derive(Resource, Default)]
    struct PendingZombieMelee(Vec<PendingMelee>);

    #[derive(Clone, Copy)]
    struct PendingMelee {
        source: NetworkEntityId,
        target: NetworkEntityId,
    }

    #[derive(Resource, Default)]
    struct ProcessedZombieDefeats {
        match_id: Option<super::super::MatchId>,
        events: BTreeSet<u64>,
    }

    #[derive(Resource, Default)]
    struct ProcessedZombieChests {
        match_id: Option<super::super::MatchId>,
        placements: BTreeSet<crate::map::MapPlacementId>,
    }

    #[derive(Resource, Default)]
    struct NextAmbientDropAt(u16);

    #[derive(bevy::ecs::system::SystemParam)]
    struct ZombieMapFacts<'w, 's> {
        resolved: Res<'w, ResolvedMap>,
        states: Query<'w, 's, &'static MapDynamicState, With<MapRoot>>,
        capacity: Res<'w, crate::matchplay::ResolvedMatchCapacity>,
        catalog: Res<'w, MapCatalogResource>,
    }

    fn bounded_drop_roll(match_id: super::super::MatchId, defeat: u16, salt: u64) -> u64 {
        let bytes = match_id.0.to_le_bytes();
        let folded = u64::from_le_bytes(bytes[..8].try_into().expect("u128 has a low u64"))
            ^ u64::from_le_bytes(bytes[8..].try_into().expect("u128 has a high u64"));
        let mut value = folded
            .wrapping_add(u64::from(defeat).wrapping_mul(0x9e37_79b9_7f4a_7c15))
            .wrapping_add(salt);
        value ^= value >> 30;
        value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value ^= value >> 27;
        value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    #[derive(Resource, Default)]
    pub(crate) struct ZombieTelemetry {
        pub(crate) peak_live: u8,
    }

    pub struct ZombieModePlugin;

    impl Plugin for ZombieModePlugin {
        fn build(&self, app: &mut App) {
            let setup = app
                .world()
                .get_resource::<crate::matchplay::MatchModeSetup>()
                .copied()
                .unwrap_or_default();
            assert_eq!(
                setup.mode_definition_id,
                crate::map::ZOMBIE_MODE_DEFINITION,
                "ZombieModePlugin requires a Zombie match mode setup"
            );
            let rules = app
                .world()
                .get_resource::<ZombieRules>()
                .copied()
                .unwrap_or_default()
                .validate()
                .expect("Zombie rules must be valid");
            app.insert_resource(rules)
                .init_resource::<NextZombieIds>()
                .init_resource::<PendingZombieMelee>()
                .init_resource::<ProcessedZombieDefeats>()
                .init_resource::<ProcessedZombieChests>()
                .init_resource::<NextAmbientDropAt>()
                .init_resource::<ZombieTelemetry>()
                .add_systems(
                    Startup,
                    initialize_zombie_state
                        .after(crate::matchplay::server::initialize_match_root)
                        .after(crate::map::MapStartupSet::Instantiate),
                )
                .add_systems(
                    FixedUpdate,
                    (
                        advance_zombie_director,
                        advance_zombie_crowd,
                        collect_boost_pickups,
                    )
                        .chain()
                        .after(MatchSet::FighterLifecycle),
                )
                .add_systems(
                    FixedUpdate,
                    cleanup_completed_runtime.after(MatchSet::PreGameOutcomes),
                )
                .add_systems(
                    FixedUpdate,
                    reset_zombie_mode.in_set(MatchRestartSet::ModeReset),
                )
                .add_systems(
                    FixedPostUpdate,
                    (
                        apply_zombie_melee_damage,
                        ApplyDeferred,
                        consume_zombie_defeats_and_resolve_terminal,
                    )
                        .chain()
                        .in_set(MatchSet::ModeRules)
                        .after(prepare_mode_rule_facts),
                );
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    fn initialize_zombie_state(
        mut commands: Commands,
        rules: Res<ZombieRules>,
        resolved: Res<ResolvedMap>,
        roots: Query<(Entity, &MatchState), With<MatchRoot>>,
    ) {
        let Ok((root, state)) = roots.single() else {
            return;
        };
        assert_eq!(state.mode_definition_id, crate::map::ZOMBIE_MODE_DEFINITION);
        assert!(!resolved.zombie_ingress.is_empty());
        commands.entity(root).insert((
            ZombieModeState {
                match_id: state.match_id,
                ambient_drops_remaining: rules.ambient_drop_budget,
                ..default()
            },
            crate::matchplay::BotObjectiveView::Elimination,
        ));
    }

    fn active_stage(
        state: &MatchState,
        mode: &ZombieModeState,
        rules: ZombieRules,
        tick: u64,
    ) -> u8 {
        let MatchPhase::Active { ends_at_tick } = state.phase else {
            return 0;
        };
        let duration = ends_at_tick
            .saturating_sub(mode.active_started_at_tick)
            .max(1);
        let elapsed = tick
            .saturating_sub(mode.active_started_at_tick)
            .min(duration);
        let raw = elapsed
            .saturating_mul(u64::from(rules.pressure_stage_count))
            .checked_div(duration)
            .unwrap_or(0);
        u8::try_from(raw)
            .unwrap_or(u8::MAX)
            .min(rules.pressure_stage_count.saturating_sub(1))
    }

    #[allow(
        clippy::too_many_arguments,
        clippy::needless_pass_by_value,
        clippy::type_complexity,
        clippy::too_many_lines,
        reason = "the director owns one bounded authoritative spawn decision and commit phase"
    )]
    fn advance_zombie_director(
        mut commands: Commands,
        tick: Res<SimulationTick>,
        rules: Res<ZombieRules>,
        resolved: Res<ResolvedMap>,
        mut ids: ResMut<NextZombieIds>,
        mut telemetry: ResMut<ZombieTelemetry>,
        spatial_query: avian2d::prelude::SpatialQuery,
        mut roots: Query<(&MatchState, &mut ZombieModeState), With<MatchRoot>>,
        fighters: Query<
            (&Position, &MatchParticipant),
            (With<Fighter>, With<ActiveCombatant>, Without<Defeated>),
        >,
        zombies: Query<&Position, (With<HostileNpc>, Without<Defeated>)>,
    ) {
        let Ok((state, mut mode)) = roots.single_mut() else {
            return;
        };
        let MatchPhase::Active { ends_at_tick } = state.phase else {
            return;
        };
        if mode.active_started_at_tick == 0 || mode.match_id != state.match_id {
            mode.match_id = state.match_id;
            mode.active_started_at_tick = tick.0;
            mode.spawning_ends_at_tick = ends_at_tick;
            mode.hard_deadline_tick = ends_at_tick.saturating_add(rules.clear_grace_ticks);
            mode.next_spawn_at_tick = tick.0;
            mode.ambient_drops_remaining = rules.ambient_drop_budget;
            mode.spawning_complete = false;
        }
        if tick.0 >= ends_at_tick {
            mode.spawning_complete = true;
            return;
        }
        let live = u8::try_from(zombies.iter().count()).unwrap_or(u8::MAX);
        mode.live_zombies = live;
        telemetry.peak_live = telemetry.peak_live.max(live);
        let pressure_stage = active_stage(state, &mode, *rules, tick.0);
        mode.pressure_stage = pressure_stage;
        let target_live = rules.target_live_for_stage(pressure_stage);
        if tick.0 < mode.next_spawn_at_tick
            || live >= target_live
            || mode.total_spawned >= rules.total_spawn_limit
        {
            return;
        }
        let player_positions = fighters
            .iter()
            .filter(|(_, participant)| participant.match_id == state.match_id)
            .map(|(position, _)| position.0)
            .collect::<Vec<_>>();
        let zombie_positions = zombies
            .iter()
            .map(|position| position.0)
            .collect::<Vec<_>>();
        let safe = resolved
            .zombie_ingress
            .iter()
            .filter(|anchor| {
                player_positions.iter().all(|player| {
                    player.distance_squared(anchor.position)
                        >= rules.player_exclusion_radius.powi(2)
                }) && zombie_positions.iter().all(|zombie| {
                    zombie.distance_squared(anchor.position) >= (rules.zombie_radius * 2.0).powi(2)
                })
            })
            .copied()
            .collect::<Vec<_>>();
        if safe.is_empty() {
            mode.next_spawn_at_tick = tick.0.saturating_add(15);
            return;
        }
        let hidden = safe
            .iter()
            .filter(|anchor| {
                player_positions.iter().all(|player| {
                    let delta = anchor.position - *player;
                    let Some(direction) = Dir2::new(delta.normalize_or_zero()).ok() else {
                        return false;
                    };
                    let filter = avian2d::prelude::SpatialQueryFilter::from_mask(
                        crate::movement::STATIC_MAP_LAYER | crate::movement::DESTRUCTIBLE_MAP_LAYER,
                    );
                    spatial_query
                        .cast_ray(*player, direction, delta.length(), true, &filter)
                        .is_some()
                })
            })
            .copied()
            .collect::<Vec<_>>();
        let candidates = if hidden.is_empty() { &safe } else { &hidden };
        let available = target_live.saturating_sub(live);
        let total_remaining = rules.total_spawn_limit.saturating_sub(mode.total_spawned);
        let count = rules
            .batch_size
            .min(available)
            .min(u8::try_from(total_remaining).unwrap_or(u8::MAX));
        for offset in 0..count {
            let Some(spawn_id) = ids.next_spawn() else {
                break;
            };
            let anchor_index = usize::try_from(
                u64::from(spawn_id.saturating_sub(1)).saturating_add(u64::from(offset)),
            )
            .unwrap_or(0)
                % candidates.len();
            let position = candidates[anchor_index].position;
            let network_id = NetworkEntityId(HOSTILE_NPC_NETWORK_NAMESPACE | u64::from(spawn_id));
            commands
                .spawn((
                    HostileNpc,
                    HostileNpcIdentity {
                        match_id: state.match_id,
                        spawn_id,
                    },
                    ZOMBIE_DEFINITION,
                    network_id,
                    TeamId(crate::combat::NEUTRAL_TEAM.0),
                    CurrentHealth(rules.zombie_health),
                    ActiveEffects::default(),
                    ZombiePhase::default(),
                    MatchMember(state.match_id),
                    Position::from_xy(position.x, position.y),
                    Rotation::radians(0.0),
                    Transform::from_translation(position.extend(0.0)),
                ))
                .insert((
                    RigidBody::Kinematic,
                    Collider::circle(rules.zombie_radius),
                    CollisionLayers::new(
                        crate::movement::HOSTILE_NPC_LAYER,
                        crate::movement::HOSTILE_NPC_LAYER
                            | crate::movement::STATIC_MAP_LAYER
                            | crate::movement::DESTRUCTIBLE_MAP_LAYER,
                    ),
                    Replicate::to_clients(NetworkTarget::All),
                ));
            mode.total_spawned = mode.total_spawned.saturating_add(1);
        }
        mode.live_zombies = live.saturating_add(count);
        telemetry.peak_live = telemetry.peak_live.max(mode.live_zombies);
        mode.next_spawn_at_tick = tick
            .0
            .saturating_add(rules.spawn_interval_for_stage(pressure_stage));
    }

    #[derive(Clone, Copy)]
    struct FighterTarget {
        network_id: NetworkEntityId,
        position: Vec2,
    }

    #[derive(Clone, Copy)]
    struct ZombieBody {
        entity: Entity,
        identity: HostileNpcIdentity,
        network_id: NetworkEntityId,
        position: Vec2,
        phase: ZombiePhase,
        active_effects: ActiveEffects,
        external_motion: Option<crate::combat::ExternalMotion>,
    }

    fn distance_to_segment(point: Vec2, start: Vec2, end: Vec2) -> f32 {
        let segment = end - start;
        let length_squared = segment.length_squared();
        if length_squared <= f32::EPSILON {
            return point.distance(start);
        }
        let t = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
        point.distance(start + segment * t)
    }

    fn melee_path_is_clear(
        zombie: &ZombieBody,
        target: FighterTarget,
        bodies: &[ZombieBody],
        radius: f32,
    ) -> bool {
        bodies.iter().all(|other| {
            other.entity == zombie.entity
                || other.position.distance_squared(target.position)
                    >= zombie.position.distance_squared(target.position)
                || distance_to_segment(other.position, zombie.position, target.position)
                    >= radius * 1.8
        })
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the exclusive crowd phase snapshots, plans, resolves, and synchronously commits one bounded 24-body movement transaction"
    )]
    fn advance_zombie_crowd(world: &mut World) {
        let tick = world.resource::<SimulationTick>().0;
        let rules = *world.resource::<ZombieRules>();
        let Some((match_id, active)) = world
            .query_filtered::<&MatchState, With<MatchRoot>>()
            .iter(world)
            .next()
            .map(|state| {
                (
                    state.match_id,
                    matches!(state.phase, MatchPhase::Active { .. }),
                )
            })
        else {
            return;
        };
        if !active {
            return;
        }
        let mut fighters = world
            .query_filtered::<(
                Entity,
                &NetworkEntityId,
                &Position,
                &MatchParticipant,
            ), (With<Fighter>, With<ActiveCombatant>, Without<Defeated>)>()
            .iter(world)
            .filter(|(_, _, _, participant)| participant.match_id == match_id)
            .map(|(_, network_id, position, _)| FighterTarget {
                network_id: *network_id,
                position: position.0,
            })
            .collect::<Vec<_>>();
        fighters.sort_by_key(|target| target.network_id.0);
        if fighters.is_empty() {
            return;
        }
        let mut bodies = world
            .query_filtered::<(
                Entity,
                &HostileNpcIdentity,
                &NetworkEntityId,
                &Position,
                &ZombiePhase,
                &ActiveEffects,
                Option<&crate::combat::ExternalMotion>,
            ), (With<HostileNpc>, Without<Defeated>)>()
            .iter(world)
            .filter(|(_, identity, ..)| identity.match_id == match_id)
            .map(
                |(
                    entity,
                    identity,
                    network_id,
                    position,
                    phase,
                    active_effects,
                    external_motion,
                )| ZombieBody {
                    entity,
                    identity: *identity,
                    network_id: *network_id,
                    position: position.0,
                    phase: *phase,
                    active_effects: *active_effects,
                    external_motion: external_motion.copied(),
                },
            )
            .collect::<Vec<_>>();
        bodies.sort_by_key(|body| body.identity.spawn_id);
        let snapshot = world.resource::<ResolvedMap>().snapshot.clone();
        let catalog = world.resource::<MapCatalogResource>().0.clone();
        let Some(map_state) = world
            .query_filtered::<&MapDynamicState, With<MapRoot>>()
            .iter(world)
            .next()
            .cloned()
        else {
            return;
        };
        let mut pending = Vec::new();
        let mut planned = Vec::with_capacity(bodies.len());
        let tick_seconds = crate::timing::SIMULATION_TICK.as_secs_f32();
        let step = rules.movement_speed_per_second * tick_seconds;
        for body in &bodies {
            let mut phase = body.phase;
            if let Some(motion) = body
                .external_motion
                .filter(|motion| tick < motion.expires_at_tick)
            {
                let proposed = crate::map::resolve_circle_against_blocking_map(
                    body.position + motion.velocity * tick_seconds,
                    rules.zombie_radius,
                    &snapshot,
                    &map_state,
                    &catalog,
                );
                planned.push((body.entity, proposed, phase));
                continue;
            }
            if body.active_effects.is_frozen(tick) {
                planned.push((body.entity, body.position, phase));
                continue;
            }
            match phase {
                ZombiePhase::Windup {
                    target: _,
                    contact_at_tick,
                } if tick < contact_at_tick => {
                    planned.push((body.entity, body.position, phase));
                    continue;
                }
                ZombiePhase::Windup { target, .. } => {
                    if fighters
                        .iter()
                        .any(|candidate| candidate.network_id == target)
                    {
                        pending.push(PendingMelee {
                            source: body.network_id,
                            target,
                        });
                    }
                    phase = ZombiePhase::Recovery {
                        ready_at_tick: tick.saturating_add(rules.melee_recovery_ticks),
                    };
                    planned.push((body.entity, body.position, phase));
                    continue;
                }
                ZombiePhase::Recovery { ready_at_tick } if tick < ready_at_tick => {
                    planned.push((body.entity, body.position, phase));
                    continue;
                }
                ZombiePhase::Recovery { .. } => {
                    phase = ZombiePhase::default();
                }
                ZombiePhase::Pursuing { .. } => {}
            }
            let committed = match phase {
                ZombiePhase::Pursuing {
                    target: Some(target),
                    reconsider_at_tick,
                } if tick < reconsider_at_tick => fighters
                    .iter()
                    .find(|candidate| candidate.network_id == target)
                    .copied(),
                _ => None,
            };
            let target = committed.unwrap_or_else(|| {
                *fighters
                    .iter()
                    .min_by(|left, right| {
                        body.position
                            .distance_squared(left.position)
                            .total_cmp(&body.position.distance_squared(right.position))
                            .then_with(|| left.network_id.0.cmp(&right.network_id.0))
                    })
                    .expect("living target list is nonempty")
            });
            let target_distance = body.position.distance(target.position);
            if target_distance <= rules.melee_range
                && melee_path_is_clear(body, target, &bodies, rules.zombie_radius)
            {
                planned.push((
                    body.entity,
                    body.position,
                    ZombiePhase::Windup {
                        target: target.network_id,
                        contact_at_tick: tick.saturating_add(rules.melee_windup_ticks),
                    },
                ));
                continue;
            }
            let slot_angle =
                (f32::from((body.identity.spawn_id % 12) as u8) / 12.0) * core::f32::consts::TAU;
            let approach =
                target.position + Vec2::from_angle(slot_angle) * (rules.melee_range * 0.78);
            let mut desired = (approach - body.position).normalize_or_zero();
            let mut separation = Vec2::ZERO;
            for other in &bodies {
                if other.entity == body.entity {
                    continue;
                }
                let away = body.position - other.position;
                let distance = away.length();
                let separation_range = rules.zombie_radius * 3.0;
                if distance > 0.001 && distance < separation_range {
                    separation += away / distance * (1.0 - distance / separation_range);
                }
            }
            desired = (desired + separation * 0.9).normalize_or_zero();
            let movement_multiplier = body
                .active_effects
                .slow
                .filter(|slow| tick < slow.expires_at_tick)
                .map_or(1.0, |slow| {
                    f32::from(slow.movement_multiplier_milli) / 1000.0
                });
            let proposed = crate::map::resolve_circle_against_blocking_map(
                body.position + desired * step * movement_multiplier,
                rules.zombie_radius,
                &snapshot,
                &map_state,
                &catalog,
            );
            planned.push((
                body.entity,
                proposed,
                ZombiePhase::Pursuing {
                    target: Some(target.network_id),
                    reconsider_at_tick: tick.saturating_add(rules.target_commitment_ticks),
                },
            ));
        }
        for _ in 0..2 {
            for high in 0..planned.len() {
                for low in 0..high {
                    let delta = planned[high].1 - planned[low].1;
                    let minimum = rules.zombie_radius * 2.0;
                    let distance = delta.length();
                    if distance < minimum {
                        let direction = if distance > 0.001 {
                            delta / distance
                        } else {
                            Vec2::from_angle(f32::from(u16::try_from(high).unwrap_or_default()))
                        };
                        planned[high].1 = crate::map::resolve_circle_against_blocking_map(
                            planned[high].1 + direction * (minimum - distance),
                            rules.zombie_radius,
                            &snapshot,
                            &map_state,
                            &catalog,
                        );
                    }
                }
            }
        }
        for high in 0..planned.len() {
            let overlaps_prior = |position: Vec2, planned: &[(Entity, Vec2, ZombiePhase)]| {
                planned[..high].iter().any(|(_, other, _)| {
                    other.distance_squared(position) < (rules.zombie_radius * 2.0).powi(2)
                })
            };
            if !overlaps_prior(planned[high].1, &planned) {
                continue;
            }
            let origin = bodies[high].position;
            let mut repaired = None;
            for ordinal in 0..8_u8 {
                let candidate = origin
                    + Vec2::from_angle(f32::from(ordinal) * core::f32::consts::FRAC_PI_4)
                        * rules.zombie_radius;
                let candidate = crate::map::resolve_circle_against_blocking_map(
                    candidate,
                    rules.zombie_radius,
                    &snapshot,
                    &map_state,
                    &catalog,
                );
                if !overlaps_prior(candidate, &planned) {
                    repaired = Some(candidate);
                    break;
                }
            }
            planned[high].1 = repaired.unwrap_or(origin);
        }
        for (entity, position, phase) in planned {
            if let Ok(mut entity) = world.get_entity_mut(entity) {
                entity.insert((
                    Position::from_xy(position.x, position.y),
                    phase,
                    Transform::from_translation(position.extend(0.0)),
                ));
                if bodies.iter().any(|body| {
                    body.entity == entity.id()
                        && body
                            .external_motion
                            .is_some_and(|motion| tick >= motion.expires_at_tick)
                }) {
                    entity.remove::<crate::combat::ExternalMotion>();
                }
            }
        }
        world.resource_mut::<PendingZombieMelee>().0.extend(pending);
    }

    #[allow(
        clippy::too_many_arguments,
        clippy::needless_pass_by_value,
        clippy::type_complexity,
        clippy::too_many_lines,
        reason = "Zombie melee validates and publishes one complete authoritative contact transaction"
    )]
    fn apply_zombie_melee_damage(
        mut commands: Commands,
        tick: Res<SimulationTick>,
        rules: Res<ZombieRules>,
        mut pending: ResMut<PendingZombieMelee>,
        mut ids: ResMut<NextCombatIds>,
        mut facts: ResMut<CombatOutcomeFacts>,
        mut outbox: ResMut<CombatOutbox>,
        spatial_query: avian2d::prelude::SpatialQuery,
        mut fighters: Query<
            (
                Entity,
                &NetworkEntityId,
                &Position,
                &mut CurrentHealth,
                Option<&SpawnProtection>,
            ),
            (With<Fighter>, With<ActiveCombatant>, Without<Defeated>),
        >,
        zombies: Query<(&NetworkEntityId, &Position), (With<HostileNpc>, Without<Defeated>)>,
    ) {
        pending
            .0
            .sort_by_key(|attack| (attack.target.0, attack.source.0));
        for attack in pending.0.drain(..) {
            let Some((entity, target, target_position, mut health, protected)) = fighters
                .iter_mut()
                .find(|(_, network_id, ..)| **network_id == attack.target)
            else {
                continue;
            };
            if protected.is_some() {
                continue;
            }
            let Some(source_position) = zombies.iter().find_map(|(network_id, position)| {
                (*network_id == attack.source).then_some(position)
            }) else {
                continue;
            };
            let contact_delta = target_position.0 - source_position.0;
            if health.0 == 0 || contact_delta.length() > rules.melee_range * 1.25 {
                continue;
            }
            let Some(contact_direction) = Dir2::new(contact_delta.normalize_or_zero()).ok() else {
                continue;
            };
            let blocker_filter = avian2d::prelude::SpatialQueryFilter::from_mask(
                crate::movement::STATIC_MAP_LAYER | crate::movement::DESTRUCTIBLE_MAP_LAYER,
            );
            if spatial_query
                .cast_ray(
                    source_position.0,
                    contact_direction,
                    contact_delta.length(),
                    true,
                    &blocker_filter,
                )
                .is_some()
            {
                continue;
            }
            let Some(attack_id) = ids.allocate_attack() else {
                continue;
            };
            let defeated = rules.melee_damage >= health.0;
            let event_count = if defeated { 2 } else { 1 };
            let Some(events) = crate::combat::server::reserve_event_ids(&mut ids, event_count)
            else {
                continue;
            };
            let applied = rules.melee_damage.min(health.0);
            health.0 = health.0.saturating_sub(applied);
            let source = DamageSource::HostileNpc {
                npc_id: attack.source,
                definition_id: ZOMBIE_DEFINITION.0,
            };
            facts.0.push(CombatOutcomeFact {
                event_id: events[0],
                tick: tick.0,
                attack_id,
                source_kind: CombatSourceKind::HostileNpc,
                source_player: None,
                source_network_id: Some(attack.source),
                source_team: Some(crate::combat::NEUTRAL_TEAM),
                target_network_id: *target,
                target_kind: CombatTargetKind::Fighter,
                target_team: TeamId(0),
                preset_id: None,
                recipe_fingerprint: None,
                position: WorldPoint::from(target_position.0),
                engagement_distance: source_position.0.distance(target_position.0),
                kind: CombatOutcomeKind::Damage { amount: applied },
            });
            outbox.0.push(CombatCue::DamageApplied {
                event_id: events[0],
                tick: tick.0,
                attack_id,
                source,
                target: *target,
                position: WorldPoint::from(target_position.0),
                amount: applied,
                health_after: health.0,
                distance_band: DistanceBand::Close,
            });
            if defeated {
                commands.entity(entity).insert((
                    Defeated {
                        event_id: events[1],
                    },
                    CollisionLayers::new(
                        crate::movement::FIGHTER_LAYER,
                        avian2d::prelude::LayerMask::NONE,
                    ),
                    ActiveEffects::default(),
                ));
                commands
                    .entity(entity)
                    .remove::<crate::combat::ExternalMotion>()
                    .remove::<crate::combat::KnockbackFeedback>();
                facts.0.push(CombatOutcomeFact {
                    event_id: events[1],
                    tick: tick.0,
                    attack_id,
                    source_kind: CombatSourceKind::HostileNpc,
                    source_player: None,
                    source_network_id: Some(attack.source),
                    source_team: Some(crate::combat::NEUTRAL_TEAM),
                    target_network_id: *target,
                    target_kind: CombatTargetKind::Fighter,
                    target_team: TeamId(0),
                    preset_id: None,
                    recipe_fingerprint: None,
                    position: WorldPoint::from(target_position.0),
                    engagement_distance: source_position.0.distance(target_position.0),
                    kind: CombatOutcomeKind::Defeat,
                });
                outbox.0.push(CombatCue::FighterDefeated {
                    event_id: events[1],
                    tick: tick.0,
                    attack_id,
                    source: Some(source),
                    target: *target,
                    position: WorldPoint::from(target_position.0),
                });
            }
        }
    }

    fn spawn_boost_pickup(
        commands: &mut Commands,
        ids: &mut NextZombieIds,
        match_id: super::super::MatchId,
        source: BoostPickupSource,
        scope: BoostGrantScope,
        position: Vec2,
        tick: u64,
    ) -> bool {
        let Some(pickup_id) = ids.next_pickup() else {
            return false;
        };
        let network_id = NetworkEntityId(BOOST_PICKUP_NETWORK_NAMESPACE | u64::from(pickup_id));
        commands.spawn((
            BoostPickup,
            BoostPickupIdentity {
                match_id,
                pickup_id,
            },
            source,
            scope,
            BoostPickupAvailableAtTick(tick.saturating_add(1)),
            network_id,
            Position::from_xy(position.x, position.y),
            Transform::from_translation(position.extend(0.0)),
            Replicate::to_clients(NetworkTarget::All),
        ));
        true
    }

    #[allow(
        clippy::too_many_arguments,
        clippy::needless_pass_by_value,
        clippy::type_complexity,
        clippy::too_many_lines,
        reason = "the mode-outcome phase consumes cross-domain defeat facts atomically before common completion"
    )]
    fn consume_zombie_defeats_and_resolve_terminal(
        mut commands: Commands,
        tick: Res<SimulationTick>,
        rules: Res<ZombieRules>,
        mut ids: ResMut<NextZombieIds>,
        facts: Res<CombatOutcomeFacts>,
        mut processed: ResMut<ProcessedZombieDefeats>,
        mut processed_chests: ResMut<ProcessedZombieChests>,
        mut next_drop: ResMut<NextAmbientDropAt>,
        mut outcomes: ResMut<ModeRuleOutcome>,
        mut diagnostics: ResMut<MatchOutcomeDiagnostics>,
        mut roots: Query<(&MatchState, &mut ZombieModeState), With<MatchRoot>>,
        zombies: Query<(Entity, &NetworkEntityId, &Position), With<HostileNpc>>,
        mut fighters: Query<
            (
                &NetworkEntityId,
                &Position,
                &mut BoostStacks,
                Has<Defeated>,
                Has<ActiveCombatant>,
            ),
            With<Fighter>,
        >,
        pickups: Query<(), With<BoostPickup>>,
        map_facts: ZombieMapFacts,
    ) {
        let Ok((state, mut mode)) = roots.single_mut() else {
            return;
        };
        if !matches!(state.phase, MatchPhase::Active { .. }) {
            return;
        }
        if processed.match_id != Some(state.match_id) {
            processed.match_id = Some(state.match_id);
            processed.events.clear();
            next_drop.0 = 1_u16.saturating_add(
                u16::try_from(
                    bounded_drop_roll(state.match_id, 0, 0)
                        % u64::from(rules.ambient_drop_every_defeats),
                )
                .unwrap_or_default(),
            );
        }
        if processed_chests.match_id != Some(state.match_id) {
            processed_chests.match_id = Some(state.match_id);
            processed_chests.placements.clear();
        }
        if let Ok(map_state) = map_facts.states.single() {
            let terminal = map_state
                .terminal_states
                .iter()
                .map(|transition| transition.placement_id)
                .collect::<BTreeSet<_>>();
            for placement in map_facts
                .resolved
                .snapshot
                .placements
                .iter()
                .filter(|placement| {
                    placement.asset_id == crate::map::TREASURE_CHEST_ASSET
                        && terminal.contains(&placement.placement_id)
                        && processed_chests.placements.insert(placement.placement_id)
                })
            {
                let scope = if placement.placement_id.0 % 2 == 0 {
                    BoostGrantScope::Team
                } else {
                    BoostGrantScope::Individual
                };
                let _ = spawn_boost_pickup(
                    &mut commands,
                    &mut ids,
                    state.match_id,
                    BoostPickupSource::Chest,
                    scope,
                    map_facts
                        .resolved
                        .snapshot
                        .dimensions
                        .cell_center(placement.cell),
                    tick.0,
                );
            }
        }
        let zombie_by_id = zombies
            .iter()
            .map(|(entity, network_id, position)| (network_id.0, (entity, position.0)))
            .collect::<BTreeMap<_, _>>();
        let mut removed_zombies = 0_u8;
        for fact in &facts.0 {
            if !matches!(fact.kind, CombatOutcomeKind::Defeat)
                || !processed.events.insert(fact.event_id.0)
            {
                continue;
            }
            match fact.target_kind {
                CombatTargetKind::HostileNpc => {
                    let Some((entity, position)) =
                        zombie_by_id.get(&fact.target_network_id.0).copied()
                    else {
                        continue;
                    };
                    commands.entity(entity).despawn();
                    removed_zombies = removed_zombies.saturating_add(1);
                    mode.total_defeated = mode.total_defeated.saturating_add(1);
                    if mode.ambient_drops_remaining > 0
                        && mode.total_defeated >= next_drop.0
                        && pickups.iter().count()
                            < MAX_LIVE_BOOST_PICKUPS.saturating_sub(
                                usize::from(rules.boost_stack_limit).saturating_mul(3),
                            )
                    {
                        let scope =
                            if bounded_drop_roll(state.match_id, mode.total_defeated, 0xa11c_e5ed)
                                .is_multiple_of(u64::from(rules.team_drop_every))
                            {
                                BoostGrantScope::Team
                            } else {
                                BoostGrantScope::Individual
                            };
                        let cost = if scope == BoostGrantScope::Team {
                            u16::from(map_facts.capacity.maximum_active_fighters)
                        } else {
                            1
                        };
                        if mode.ambient_drops_remaining >= cost
                            && spawn_boost_pickup(
                                &mut commands,
                                &mut ids,
                                state.match_id,
                                BoostPickupSource::Zombie,
                                scope,
                                position,
                                tick.0,
                            )
                        {
                            mode.ambient_drops_remaining =
                                mode.ambient_drops_remaining.saturating_sub(cost);
                            let maximum_gap =
                                rules.ambient_drop_every_defeats.saturating_mul(2).max(1);
                            let gap = 1_u16.saturating_add(
                                u16::try_from(
                                    bounded_drop_roll(
                                        state.match_id,
                                        mode.total_defeated,
                                        0xd20f_5eed,
                                    ) % u64::from(maximum_gap),
                                )
                                .unwrap_or_default(),
                            );
                            next_drop.0 = mode.total_defeated.saturating_add(gap);
                        }
                    }
                }
                CombatTargetKind::Fighter => {
                    let Some((_, position, mut stacks, _, _)) = fighters
                        .iter_mut()
                        .find(|(network_id, ..)| **network_id == fact.target_network_id)
                    else {
                        continue;
                    };
                    let count = stacks.0;
                    stacks.0 = 0;
                    for stack in 0..count {
                        let angle = f32::from(stack) * 2.399_963_1;
                        let radius = 18.0 + f32::from(stack) * 3.0;
                        let scattered = position.0 + Vec2::from_angle(angle) * radius;
                        let scattered =
                            map_facts
                                .states
                                .single()
                                .ok()
                                .map_or(scattered, |map_state| {
                                    crate::map::resolve_circle_against_blocking_map(
                                        scattered,
                                        10.0,
                                        &map_facts.resolved.snapshot,
                                        map_state,
                                        &map_facts.catalog.0,
                                    )
                                });
                        let _ = spawn_boost_pickup(
                            &mut commands,
                            &mut ids,
                            state.match_id,
                            BoostPickupSource::PlayerDeath,
                            BoostGrantScope::Individual,
                            scattered,
                            tick.0,
                        );
                    }
                }
                CombatTargetKind::Deployable => {}
            }
        }
        mode.live_zombies = mode.live_zombies.saturating_sub(removed_zombies);
        let living = fighters
            .iter()
            .filter(|(_, _, _, defeated, active)| !*defeated && *active)
            .count();
        mode.living_players = u8::try_from(living).unwrap_or(u8::MAX);
        mode.last_brawler_standing = living == 1;
        let result = if living == 0 {
            Some((
                ModeOutcomeCause::Threshold,
                super::super::MatchResult::Overrun,
            ))
        } else if tick.0 >= mode.hard_deadline_tick {
            Some((
                ModeOutcomeCause::Timeout,
                super::super::MatchResult::Overrun,
            ))
        } else if mode.spawning_complete && mode.live_zombies == 0 {
            Some((
                ModeOutcomeCause::Threshold,
                super::super::MatchResult::Cleared,
            ))
        } else {
            None
        };
        if let Some((cause, result)) = result {
            offer_mode_rule_outcome(
                &mut outcomes,
                &mut diagnostics,
                PendingModeRuleOutcome {
                    match_id: state.match_id,
                    evaluated_tick: tick.0,
                    cause,
                    result,
                },
            );
        }
    }

    #[allow(
        clippy::type_complexity,
        clippy::needless_pass_by_value,
        reason = "the Bevy system parameters declare the complete deterministic pickup transaction view"
    )]
    fn collect_boost_pickups(
        mut commands: Commands,
        tick: Res<SimulationTick>,
        rules: Res<ZombieRules>,
        roots: Query<&MatchState, With<MatchRoot>>,
        pickups: Query<
            (
                Entity,
                &BoostPickupIdentity,
                &BoostGrantScope,
                &BoostPickupAvailableAtTick,
                &Position,
            ),
            With<BoostPickup>,
        >,
        mut fighters: ParamSet<(
            Query<
                (
                    Entity,
                    &NetworkEntityId,
                    &Position,
                    &BoostStacks,
                    &MatchParticipant,
                ),
                (With<Fighter>, With<ActiveCombatant>, Without<Defeated>),
            >,
            Query<
                (
                    &mut CurrentHealth,
                    &mut BoostStacks,
                    &crate::builds::ResolvedFighterStats,
                    &MatchParticipant,
                ),
                With<Fighter>,
            >,
        )>,
    ) {
        let Ok(state) = roots.single() else { return };
        if !matches!(state.phase, MatchPhase::Active { .. }) {
            return;
        }
        let mut ordered = pickups.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|(_, identity, ..)| identity.pickup_id);
        for (pickup_entity, identity, scope, available, position) in ordered {
            if identity.match_id != state.match_id || tick.0 < available.0 {
                continue;
            }
            let (nearby, recipients_to_apply) = {
                let candidates = fighters.p0();
                let mut nearby = candidates
                    .iter()
                    .filter(|(_, _, candidate_position, _, participant)| {
                        participant.match_id == state.match_id
                            && candidate_position.0.distance_squared(position.0)
                                <= rules.pickup_radius.powi(2)
                    })
                    .map(|(entity, network_id, _, stacks, _)| (network_id.0, entity, stacks.0))
                    .collect::<Vec<_>>();
                nearby.sort_by_key(|candidate| candidate.0);
                let recipients = match scope {
                    BoostGrantScope::Individual => nearby
                        .iter()
                        .find(|candidate| candidate.2 < rules.boost_stack_limit)
                        .map(|candidate| vec![candidate.1])
                        .unwrap_or_default(),
                    BoostGrantScope::Team => candidates
                        .iter()
                        .filter(|(_, _, _, stacks, participant)| {
                            participant.match_id == state.match_id
                                && stacks.0 < rules.boost_stack_limit
                        })
                        .map(|(entity, network_id, ..)| (network_id.0, entity))
                        .collect::<BTreeMap<_, _>>()
                        .into_values()
                        .collect(),
                };
                (nearby, recipients)
            };
            if nearby.is_empty() || recipients_to_apply.is_empty() {
                continue;
            }
            let mut recipients = fighters.p1();
            for entity in recipients_to_apply {
                let Ok((mut health, mut stacks, fighter_stats, participant)) =
                    recipients.get_mut(entity)
                else {
                    continue;
                };
                if participant.match_id != state.match_id || stacks.0 >= rules.boost_stack_limit {
                    continue;
                }
                let previous_maximum =
                    effective_maximum_health(fighter_stats.maximum_health, *stacks);
                stacks.0 = stacks.0.saturating_add(1).min(rules.boost_stack_limit);
                let new_maximum = effective_maximum_health(fighter_stats.maximum_health, *stacks);
                health.0 = health
                    .0
                    .saturating_add(new_maximum.saturating_sub(previous_maximum))
                    .min(new_maximum);
            }
            commands.entity(pickup_entity).despawn();
        }
    }

    #[allow(
        clippy::needless_pass_by_value,
        clippy::type_complexity,
        clippy::too_many_arguments,
        reason = "restart clears every bounded Zombie-owned runtime resource and entity family together"
    )]
    fn reset_zombie_mode(
        mut commands: Commands,
        pending: Res<PendingMatchRestart>,
        rules: Res<ZombieRules>,
        mut ids: ResMut<NextZombieIds>,
        mut next_drop: ResMut<NextAmbientDropAt>,
        mut roots: Query<&mut ZombieModeState, With<MatchRoot>>,
        runtime: Query<Entity, Or<(With<HostileNpc>, With<BoostPickup>)>>,
        fighters: Query<Entity, With<Fighter>>,
    ) {
        let Some(slot) = pending.slot() else { return };
        for entity in &runtime {
            commands.entity(entity).despawn();
        }
        for fighter in &fighters {
            commands.entity(fighter).insert(BoostStacks(0));
        }
        ids.clear();
        next_drop.0 = 0;
        if let Ok(mut state) = roots.single_mut() {
            *state = ZombieModeState {
                match_id: slot.next_id,
                ambient_drops_remaining: rules.ambient_drop_budget,
                ..default()
            };
        }
    }

    #[allow(
        clippy::type_complexity,
        reason = "the cleanup filter intentionally spans both Zombie-owned runtime families"
    )]
    fn cleanup_completed_runtime(
        mut commands: Commands,
        roots: Query<&MatchState, With<MatchRoot>>,
        runtime: Query<Entity, Or<(With<HostileNpc>, With<BoostPickup>)>>,
    ) {
        let Ok(state) = roots.single() else { return };
        if !matches!(state.phase, MatchPhase::Completed { .. }) {
            return;
        }
        for entity in &runtime {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(feature = "server")]
pub use authority::ZombieModePlugin;
#[cfg(feature = "server")]
pub(crate) use authority::ZombieTelemetry;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boost_scaling_is_round_half_up_and_stacks_linearly() {
        assert_eq!(scale_boosted_value(100, 0), 100);
        assert_eq!(scale_boosted_value(100, 3), 115);
        assert_eq!(scale_boosted_value(101, 1), 106);
    }

    #[test]
    fn pressure_targets_and_intervals_are_bounded() {
        let rules = ZombieRules::default().validate().unwrap();
        assert_eq!(rules.target_live_for_stage(0), 8);
        assert_eq!(rules.target_live_for_stage(1), 13);
        assert_eq!(rules.target_live_for_stage(2), 18);
        assert_eq!(rules.target_live_for_stage(3), 23);
        assert_eq!(rules.batch_size, 4);
        assert_eq!(rules.zombie_health, 240);
        assert!(rules.target_live_for_stage(u8::MAX) <= MAX_LIVE_ZOMBIES);
        assert!(rules.spawn_interval_for_stage(u8::MAX) >= 15);
    }

    #[test]
    fn pickup_capacity_reserves_every_players_death_stack() {
        let mut rules = ZombieRules {
            ambient_drop_budget: 65,
            ..ZombieRules::default()
        };
        assert!(rules.validate().is_err());

        rules.ambient_drop_budget = 64;
        assert!(rules.validate().is_ok());
    }
}
