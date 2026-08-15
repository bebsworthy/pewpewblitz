//! Reusable server-authoritative fighter activation, reset, respawn, and protection lifecycle.

use super::{
    ActiveCombatant, MatchParticipant, MatchPhase, MatchRoot, MatchSet, MatchState, RespawnState,
    SpawnProtection,
};
use crate::{
    combat::{
        ActiveEffects, CurrentHealth, Defeated, FighterDefinitions, SelectedBuild, SpawnState,
        WeaponDefinitions, WeaponPhase, WeaponState,
    },
    gameplay::GameplaySet,
    protocol::NetworkEntityId,
    timing::SimulationTick,
};
use avian2d::prelude::{CollisionLayers, LayerMask, LinearVelocity, Position, Rotation};
use bevy::prelude::*;
#[derive(Resource, Clone, Copy, Debug, Default)]
pub(crate) struct FighterLifecycleConfig {
    pub(crate) spawn_protection_ticks: u64,
}

pub struct AuthoritativeFighterLifecyclePlugin;

impl Plugin for AuthoritativeFighterLifecyclePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FighterLifecycleConfig>().add_systems(
            FixedUpdate,
            (expire_protection, respawn_due_fighters)
                .chain()
                .in_set(GameplaySet::Lifecycle)
                .in_set(MatchSet::FighterLifecycle),
        );
    }
}

#[derive(Clone, Copy)]
pub(crate) struct FighterReset {
    pub maximum_health: u16,
    pub ammunition: u8,
    pub position: Vec2,
    pub facing: f32,
    pub collision_mask: LayerMask,
    pub protection_until: Option<u64>,
    pub active: bool,
}

pub(crate) fn fighter_runtime_values(
    fighter_id: crate::combat::FighterDefinitionId,
    build: &SelectedBuild,
    resolved: Option<&crate::combat::ResolvedWeapon>,
    fighters: &FighterDefinitions,
    weapons: &WeaponDefinitions,
) -> Option<(u16, u8)> {
    let maximum_health = fighters.get(fighter_id)?.maximum_health;
    let ammunition = resolved.map_or_else(
        || {
            weapons
                .get(build.primary_weapon)
                .map_or(0, |weapon| weapon.magazine_capacity)
        },
        |weapon| weapon.recipe.economy.capacity(),
    );
    Some((maximum_health, ammunition))
}

pub(crate) fn reset_fighter_runtime(commands: &mut Commands, entity: Entity, reset: FighterReset) {
    let mut fighter = commands.entity(entity);
    fighter
        .insert((
            CurrentHealth(reset.maximum_health),
            WeaponState {
                ammo: reset.ammunition,
                phase: WeaponPhase::Ready,
            },
            Position::from_xy(reset.position.x, reset.position.y),
            Rotation::radians(reset.facing),
            LinearVelocity::ZERO,
            ActiveEffects::default(),
            crate::movement::InputFreshness::default(),
            CollisionLayers::new(crate::movement::FIGHTER_LAYER, reset.collision_mask),
        ))
        .remove::<Defeated>()
        .remove::<RespawnState>()
        .remove::<SpawnProtection>()
        .remove::<ActiveCombatant>()
        .remove::<crate::combat::ExternalMotion>()
        .remove::<crate::combat::KnockbackFeedback>();
    if reset.active {
        fighter.insert(ActiveCombatant);
    }
    if let Some(expires_at_tick) = reset.protection_until {
        fighter.insert(SpawnProtection { expires_at_tick });
    }
}

pub(crate) fn complete_fighter_lifecycle(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .insert((LinearVelocity::ZERO, ActiveEffects::default()))
        .remove::<ActiveCombatant>()
        .remove::<RespawnState>()
        .remove::<SpawnProtection>()
        .remove::<crate::combat::ExternalMotion>()
        .remove::<crate::combat::KnockbackFeedback>();
}

#[allow(clippy::needless_pass_by_value)]
fn expire_protection(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    roots: Query<&MatchState, With<MatchRoot>>,
    protected: Query<(Entity, &MatchParticipant, &SpawnProtection)>,
) {
    let Ok(state) = roots.single() else { return };
    for (entity, participant, protection) in &protected {
        if participant.match_id == state.match_id
            && matches!(state.phase, MatchPhase::Active { .. })
            && tick.0 >= protection.expires_at_tick
        {
            commands.entity(entity).remove::<SpawnProtection>();
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity
)]
fn respawn_due_fighters(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    config: Res<FighterLifecycleConfig>,
    fighters: Res<FighterDefinitions>,
    weapons: Res<WeaponDefinitions>,
    roots: Query<&MatchState, With<MatchRoot>>,
    mut telemetry: ResMut<super::MatchTelemetry>,
    query: Query<(
        Entity,
        &NetworkEntityId,
        &MatchParticipant,
        &crate::combat::FighterDefinitionId,
        &SelectedBuild,
        Option<&crate::combat::ResolvedWeapon>,
        &RespawnState,
        &SpawnState,
    )>,
) {
    let Ok(state) = roots.single() else { return };
    if !matches!(state.phase, MatchPhase::Active { .. }) {
        return;
    }
    for (entity, network_id, participant, fighter_id, build, resolved, respawn, spawn) in &query {
        if participant.match_id != state.match_id || tick.0 < respawn.respawn_at_tick {
            continue;
        }
        let Some((maximum_health, ammunition)) =
            fighter_runtime_values(*fighter_id, build, resolved, &fighters, &weapons)
        else {
            continue;
        };
        telemetry.record_respawn(network_id.0, tick.0);
        reset_fighter_runtime(
            &mut commands,
            entity,
            FighterReset {
                maximum_health,
                ammunition,
                position: spawn.position,
                facing: spawn.facing,
                collision_mask: crate::movement::INDESTRUCTIBLE_TERRAIN_LAYER
                    | crate::movement::DESTRUCTIBLE_TERRAIN_LAYER,
                protection_until: Some(tick.0.saturating_add(config.spawn_protection_ticks)),
                active: true,
            },
        );
    }
}
