//! Server-owned terrain concealment and observer-specific replication relevance.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub const ATTACK_REVEAL_TICKS: u64 = 90;
pub const DAMAGE_REVEAL_TICKS: u64 = 120;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainConcealmentMembership {
    pub map_instance_id: crate::map::MapInstanceId,
    pub placement_id: crate::map::MapPlacementId,
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConcealmentRevealDeadlines {
    /// A deadline is active while `current_tick < deadline`.
    pub attack_until_tick: u64,
    pub damage_until_tick: u64,
}

/// Replicated only with an already-permitted fighter for owner/ally/revealed presentation.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConcealmentPresentationState {
    pub inside_concealing_terrain: bool,
    pub revealed_until_tick: u64,
}

#[must_use]
pub fn reveal_lock_active(tick: u64, deadlines: ConcealmentRevealDeadlines) -> bool {
    tick < deadlines.attack_until_tick || tick < deadlines.damage_until_tick
}

#[must_use]
#[allow(
    clippy::fn_params_excessive_bools,
    reason = "the observer rule is an explicit six-input truth table covered at its boundaries"
)]
pub fn observer_can_see(
    allied_or_self: bool,
    observer_alive: bool,
    subject_concealed: bool,
    subject_reveal_locked: bool,
    distance_squared: f32,
    reveal_radius: f32,
) -> bool {
    allied_or_self
        || !subject_concealed
        || subject_reveal_locked
        || (observer_alive
            && distance_squared.is_finite()
            && reveal_radius.is_finite()
            && distance_squared <= reveal_radius * reveal_radius)
}

#[cfg(feature = "server")]
mod server {
    #![allow(
        clippy::wildcard_imports,
        reason = "the role-owned module composes the shared concealment contract"
    )]
    use super::*;
    use crate::{
        combat::{
            CombatCue, CombatOutbox, CombatOutcomeFacts, CombatOutcomeKind, Defeated, TeamId,
        },
        map::{MapCatalogResource, MapConcealmentBehavior, ResolvedMap, placement_cells},
        matchplay::ActiveCombatant,
        protocol::{Fighter, NetworkEntityId},
        timing::SimulationTick,
    };
    use avian2d::prelude::Position;
    use lightyear::prelude::{ControlledBy, Replicate, ReplicationSystems, VisibilityExt};
    use std::collections::{HashMap, HashSet};

    #[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
    pub enum ConcealmentSet {
        Resolve,
    }

    #[derive(Resource, Default)]
    pub(crate) struct ObserverVisibilityCache(HashMap<(Entity, Entity), bool>);

    const MAX_CONCEALMENT_TRANSITIONS: usize = 2_048;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum VisibilityTransitionReason {
        SelfOrAlly,
        PublicOrOutsideTerrain,
        RevealLock,
        Proximity,
        TerrainConcealment,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct VisibilityTransitionRecord {
        pub tick: u64,
        pub observer_team: TeamId,
        pub subject: NetworkEntityId,
        pub visible: bool,
        pub reason: VisibilityTransitionReason,
    }

    #[derive(Resource, Default, Debug)]
    pub struct ConcealmentTelemetry {
        pub transitions: Vec<VisibilityTransitionRecord>,
        pub dropped_transitions: u64,
    }

    impl ConcealmentTelemetry {
        fn record(&mut self, transition: VisibilityTransitionRecord) {
            if self.transitions.len() < MAX_CONCEALMENT_TRANSITIONS {
                self.transitions.push(transition);
            } else {
                self.dropped_transitions = self.dropped_transitions.saturating_add(1);
            }
        }
    }

    impl ObserverVisibilityCache {
        #[must_use]
        pub(crate) fn permits(&self, connection: Entity, subject: Entity) -> bool {
            self.0.get(&(connection, subject)).copied().unwrap_or(false)
        }
    }

    pub struct ServerConcealmentPlugin;

    impl Plugin for ServerConcealmentPlugin {
        fn build(&self, app: &mut App) {
            app.init_resource::<ObserverVisibilityCache>()
                .init_resource::<ConcealmentTelemetry>()
                .configure_sets(
                    FixedPostUpdate,
                    ConcealmentSet::Resolve
                        .after(crate::combat::CombatSet::Damage)
                        .before(crate::combat::CombatSet::TelemetryAndCues),
                )
                .add_systems(
                    FixedPostUpdate,
                    (resolve_membership_and_reveal_locks, ApplyDeferred)
                        .chain()
                        .in_set(ConcealmentSet::Resolve),
                )
                .add_systems(
                    PostUpdate,
                    (
                        sync_public_participant_projections,
                        apply_observer_visibility,
                        ApplyDeferred,
                    )
                        .chain()
                        .before(ReplicationSystems::Send),
                );
        }
    }

    #[allow(clippy::type_complexity, clippy::needless_pass_by_value)]
    fn sync_public_participant_projections(
        mut commands: Commands,
        fighters: Query<
            (
                &crate::protocol::PlayerId,
                &NetworkEntityId,
                &TeamId,
                &crate::matchplay::FighterDisplayName,
                &crate::matchplay::MatchParticipant,
                Option<&crate::builds::SelectedBuild>,
                Option<&crate::builds::ResolvedMatchLoadout>,
                Option<&crate::matchplay::RespawnState>,
                Option<&crate::matchplay::SpawnProtection>,
                Has<Defeated>,
            ),
            With<Fighter>,
        >,
        mut projections: Query<(Entity, &mut crate::matchplay::PublicParticipantState)>,
    ) {
        use crate::matchplay::{PublicParticipantState, PublicParticipantStatus};
        use lightyear::prelude::{NetworkTarget, Replicate};
        let mut active_ids = HashSet::new();
        for (
            player,
            network,
            team,
            name,
            participant,
            selected,
            loadout,
            respawn,
            protection,
            defeated,
        ) in &fighters
        {
            active_ids.insert(network.0);
            let status = if let Some(respawn) = respawn {
                PublicParticipantStatus::Respawning {
                    respawn_at_tick: respawn.respawn_at_tick,
                }
            } else if let Some(protection) = protection {
                PublicParticipantStatus::Protected {
                    expires_at_tick: protection.expires_at_tick,
                }
            } else if defeated {
                PublicParticipantStatus::Defeated
            } else if participant.restart_ready {
                PublicParticipantStatus::RestartReady
            } else if participant.ready {
                PublicParticipantStatus::Ready
            } else {
                PublicParticipantStatus::Alive
            };
            let state = PublicParticipantState {
                player_id: *player,
                fighter_network_id: *network,
                team_id: *team,
                display_name: name.0.clone(),
                participant: *participant,
                selected: selected.is_some(),
                weapon_preset_id: loadout
                    .and_then(|value| value.primary_weapon.source_preset_id.map(|id| id.0)),
                status,
            };
            if let Some((_, mut existing)) = projections
                .iter_mut()
                .find(|(_, value)| value.fighter_network_id == *network)
            {
                if *existing != state {
                    *existing = state;
                }
            } else {
                commands.spawn((state, Replicate::to_clients(NetworkTarget::All)));
            }
        }
        for (entity, projection) in &mut projections {
            if !active_ids.contains(&projection.fighter_network_id.0) {
                commands.entity(entity).try_despawn();
            }
        }
    }

    #[allow(clippy::type_complexity, clippy::needless_pass_by_value)]
    fn resolve_membership_and_reveal_locks(
        mut commands: Commands,
        tick: Res<SimulationTick>,
        map: Option<Res<ResolvedMap>>,
        catalog: Res<MapCatalogResource>,
        outbox: Res<CombatOutbox>,
        outcomes: Res<CombatOutcomeFacts>,
        fighters: Query<
            (
                Entity,
                &NetworkEntityId,
                &Position,
                Option<&TerrainConcealmentMembership>,
                Option<&ConcealmentRevealDeadlines>,
                Has<Defeated>,
            ),
            With<Fighter>,
        >,
    ) {
        let attack_reveals: HashSet<_> = outbox
            .0
            .iter()
            .filter_map(|cue| match cue {
                CombatCue::AttackAccepted { source, .. } => Some(source.0),
                _ => None,
            })
            .collect();
        let damage_reveals: HashSet<_> = outcomes
            .0
            .iter()
            .filter_map(|fact| match fact.kind {
                CombatOutcomeKind::Damage { amount } if amount > 0 => {
                    Some(fact.target_network_id.0)
                }
                _ => None,
            })
            .collect();

        for (entity, network_id, position, prior_membership, prior_deadlines, defeated) in &fighters
        {
            let membership = (!defeated)
                .then_some(map.as_ref())
                .flatten()
                .and_then(|map| concealment_membership(position.0, map, &catalog.0));
            if membership != prior_membership.copied() {
                if let Some(membership) = membership {
                    commands.entity(entity).insert(membership);
                } else {
                    commands
                        .entity(entity)
                        .remove::<TerrainConcealmentMembership>();
                }
            }

            let mut deadlines = prior_deadlines.copied().unwrap_or_default();
            if attack_reveals.contains(&network_id.0) {
                deadlines.attack_until_tick = deadlines
                    .attack_until_tick
                    .max(tick.0.saturating_add(ATTACK_REVEAL_TICKS));
            }
            if damage_reveals.contains(&network_id.0) {
                deadlines.damage_until_tick = deadlines
                    .damage_until_tick
                    .max(tick.0.saturating_add(DAMAGE_REVEAL_TICKS));
            }
            if deadlines != prior_deadlines.copied().unwrap_or_default() {
                commands.entity(entity).insert(deadlines);
            }
            commands
                .entity(entity)
                .insert(ConcealmentPresentationState {
                    inside_concealing_terrain: membership.is_some(),
                    revealed_until_tick: deadlines
                        .attack_until_tick
                        .max(deadlines.damage_until_tick),
                });
        }
    }

    fn concealment_membership(
        position: Vec2,
        map: &ResolvedMap,
        catalog: &crate::map::MapContentCatalog,
    ) -> Option<TerrainConcealmentMembership> {
        if !position.is_finite() {
            return None;
        }
        map.snapshot.placements.iter().find_map(|placement| {
            let asset = catalog.asset(placement.asset_id)?;
            let profile = catalog.profile(asset.gameplay_profile_id)?;
            if profile.concealment != MapConcealmentBehavior::HideOccupants {
                return None;
            }
            let contains = placement_cells(map.snapshot.dimensions, asset, placement)?
                .into_iter()
                .any(|cell| {
                    let min = map.snapshot.dimensions.cell_min(cell);
                    let max = min + Vec2::splat(crate::map::MAP_CELL_SIZE_WORLD);
                    position.x >= min.x
                        && position.x <= max.x
                        && position.y >= min.y
                        && position.y <= max.y
                });
            contains.then_some(TerrainConcealmentMembership {
                map_instance_id: map.snapshot.identity.instance_id,
                placement_id: placement.placement_id,
            })
        })
    }

    #[derive(Clone, Copy)]
    struct FighterView {
        entity: Entity,
        team: TeamId,
        network_id: NetworkEntityId,
        position: Vec2,
        reveal_radius: f32,
        connection: Option<Entity>,
        alive: bool,
        concealed: bool,
        reveal_locked: bool,
    }

    #[allow(clippy::type_complexity, clippy::needless_pass_by_value)]
    fn apply_observer_visibility(
        mut commands: Commands,
        tick: Res<SimulationTick>,
        mut cache: ResMut<ObserverVisibilityCache>,
        mut telemetry: ResMut<ConcealmentTelemetry>,
        fighters: Query<
            (
                Entity,
                &TeamId,
                &NetworkEntityId,
                &Position,
                Option<&crate::builds::ResolvedMatchLoadout>,
                Option<&ControlledBy>,
                Has<Defeated>,
                Has<ActiveCombatant>,
                Option<&TerrainConcealmentMembership>,
                Option<&ConcealmentRevealDeadlines>,
            ),
            (With<Fighter>, With<Replicate>),
        >,
    ) {
        let views: Vec<_> = fighters
            .iter()
            .map(
                |(
                    entity,
                    team,
                    network_id,
                    position,
                    loadout,
                    controlled,
                    defeated,
                    active,
                    membership,
                    deadlines,
                )| FighterView {
                    entity,
                    team: *team,
                    network_id: *network_id,
                    position: position.0,
                    reveal_radius: loadout
                        .map_or(160.0, |value| value.fighter_stats.reveal_proximity_radius),
                    connection: controlled.map(|value| value.owner),
                    alive: !defeated && active,
                    concealed: membership.is_some() && !defeated,
                    reveal_locked: deadlines
                        .is_some_and(|value| reveal_lock_active(tick.0, *value)),
                },
            )
            .collect();
        let mut current = HashSet::new();
        for observer in views.iter().filter(|view| view.connection.is_some()) {
            let connection = observer
                .connection
                .expect("filtered observer has connection");
            for subject in &views {
                let key = (connection, subject.entity);
                current.insert(key);
                let allied_or_self =
                    observer.entity == subject.entity || observer.team == subject.team;
                let distance_squared = observer.position.distance_squared(subject.position);
                let visible = observer_can_see(
                    allied_or_self,
                    observer.alive,
                    subject.concealed,
                    subject.reveal_locked,
                    distance_squared,
                    observer.reveal_radius,
                );
                if cache.0.get(&key).copied() == Some(visible) {
                    continue;
                }
                cache.0.insert(key, visible);
                let reason = if allied_or_self {
                    VisibilityTransitionReason::SelfOrAlly
                } else if !subject.concealed {
                    VisibilityTransitionReason::PublicOrOutsideTerrain
                } else if subject.reveal_locked {
                    VisibilityTransitionReason::RevealLock
                } else if visible {
                    VisibilityTransitionReason::Proximity
                } else {
                    VisibilityTransitionReason::TerrainConcealment
                };
                telemetry.record(VisibilityTransitionRecord {
                    tick: tick.0,
                    observer_team: observer.team,
                    subject: subject.network_id,
                    visible,
                    reason,
                });
                if visible {
                    commands.gain_visibility(subject.entity, connection);
                } else {
                    commands.lose_visibility(subject.entity, connection);
                }
            }
        }
        cache.0.retain(|key, _| current.contains(key));
    }
}

#[cfg(feature = "server")]
pub(crate) use server::ObserverVisibilityCache;
#[cfg(feature = "server")]
pub use server::{ConcealmentSet, ServerConcealmentPlugin};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_equality_reveals_and_defeated_observer_has_no_enemy_permission() {
        assert!(observer_can_see(
            false,
            true,
            true,
            false,
            160.0_f32.powi(2),
            160.0
        ));
        assert!(!observer_can_see(
            false,
            true,
            true,
            false,
            160.01_f32.powi(2),
            160.0
        ));
        assert!(!observer_can_see(false, false, true, false, 0.0, 160.0));
        assert!(observer_can_see(
            true,
            false,
            true,
            false,
            f32::INFINITY,
            160.0
        ));
    }

    #[test]
    fn reveal_deadline_end_tick_is_exclusive() {
        let deadlines = ConcealmentRevealDeadlines {
            attack_until_tick: 12,
            damage_until_tick: 10,
        };
        assert!(reveal_lock_active(11, deadlines));
        assert!(!reveal_lock_active(12, deadlines));
    }
}
