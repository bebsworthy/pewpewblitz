//! Server-owned concealment composition and observer-specific replication relevance.

mod field;
mod model;
mod rules;
pub use field::{
    AlliedConcealmentMemberships, ConcealmentFieldId, ConcealmentFieldState,
    MAX_ACTIVE_CONCEALMENT_FIELDS, ObjectiveCarrier, field_contains,
};
#[cfg(feature = "server")]
pub(crate) use field::{ConcealmentFieldOwner, NextConcealmentFieldId};
pub use model::{
    ConcealmentPresentationState, ConcealmentRevealDeadlines, ConcealmentSources,
    ForcedRevealSource, ForcedRevealSources, MAX_FORCED_REVEAL_SOURCES, ObserverRelation,
    ObserverVisibilityInput, TeamRevealDeadline, TerrainConcealmentMembership, observer_can_see,
    reveal_lock_active,
};
pub use rules::{
    CONCEALMENT_RULES_SCHEMA_VERSION, ConcealmentContentPlugin, ConcealmentRules,
    ConcealmentRulesResource, MAX_REVEAL_LOCK_TICKS,
};

#[cfg(feature = "server")]
mod network;
#[cfg(feature = "server")]
mod telemetry;

#[cfg(feature = "server")]
use bevy::prelude::*;

#[cfg(feature = "server")]
mod server {
    #![allow(
        clippy::wildcard_imports,
        reason = "the role-owned module composes the shared concealment contract"
    )]
    use super::{
        network::{
            ObserverVisibilityCache, QueuedVisibilityTransitions, apply_queued_observer_visibility,
        },
        telemetry::{ConcealmentTelemetry, VisibilityTransitionReason, VisibilityTransitionRecord},
        *,
    };
    use crate::{
        combat::{AcceptedAttackFacts, CombatOutcomeFacts, CombatOutcomeKind, Defeated, TeamId},
        map::{MapCatalogResource, MapConcealmentBehavior, ResolvedMap, placement_cells},
        matchplay::ActiveCombatant,
        protocol::{Fighter, NetworkEntityId},
        timing::SimulationTick,
    };
    use avian2d::prelude::Position;
    use lightyear::prelude::{ControlledBy, Replicate, ReplicationSystems};
    use std::collections::HashSet;

    #[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
    pub enum ConcealmentSet {
        ResolveSources,
        DecideObservers,
    }

    pub struct ServerConcealmentPlugin;

    impl Plugin for ServerConcealmentPlugin {
        fn build(&self, app: &mut App) {
            app.init_resource::<ObserverVisibilityCache>()
                .init_resource::<QueuedVisibilityTransitions>()
                .init_resource::<ConcealmentTelemetry>()
                .init_resource::<NextConcealmentFieldId>();
            configure_concealment_schedule(app);
            app.add_systems(
                FixedPostUpdate,
                observe_attack_and_damage_reveal_locks
                    .in_set(crate::abilities::AbilitySet::ObserveOutcomes),
            )
            .add_systems(
                FixedPostUpdate,
                (resolve_membership_and_reveal_locks, ApplyDeferred)
                    .chain()
                    .in_set(ConcealmentSet::ResolveSources),
            )
            .add_systems(
                FixedPostUpdate,
                decide_observer_visibility.in_set(ConcealmentSet::DecideObservers),
            )
            .add_systems(
                PostUpdate,
                (
                    sync_public_participant_projections,
                    apply_queued_observer_visibility,
                    ApplyDeferred,
                )
                    .chain()
                    .before(ReplicationSystems::Send),
            );
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    pub(super) fn observe_attack_and_damage_reveal_locks(
        mut commands: Commands,
        tick: Res<SimulationTick>,
        rules: Res<ConcealmentRulesResource>,
        accepted_attacks: Res<AcceptedAttackFacts>,
        outcomes: Res<CombatOutcomeFacts>,
        fighters: Query<
            (
                Entity,
                &NetworkEntityId,
                Option<&ConcealmentRevealDeadlines>,
            ),
            With<Fighter>,
        >,
    ) {
        let attack_reveals: HashSet<_> = accepted_attacks
            .0
            .iter()
            .map(|fact| fact.source_network_id.0)
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
        for (entity, network_id, prior) in &fighters {
            let mut deadlines = prior.copied().unwrap_or_default();
            if attack_reveals.contains(&network_id.0) {
                deadlines.attack_until_tick = deadlines
                    .attack_until_tick
                    .max(tick.0.saturating_add(rules.0.attack_reveal_ticks));
            }
            if damage_reveals.contains(&network_id.0) {
                deadlines.damage_until_tick = deadlines
                    .damage_until_tick
                    .max(tick.0.saturating_add(rules.0.damage_reveal_ticks));
            }
            if deadlines != prior.copied().unwrap_or_default() {
                commands.entity(entity).insert(deadlines);
            }
        }
    }

    pub(super) fn configure_concealment_schedule(app: &mut App) {
        app.configure_sets(
            FixedPostUpdate,
            (
                ConcealmentSet::ResolveSources
                    .in_set(crate::gameplay::AuthoritativePhase::Visibility)
                    .after(crate::abilities::AbilitySet::ObserveOutcomes)
                    .after(crate::combat::CombatSet::Lifecycle),
                ConcealmentSet::DecideObservers
                    .in_set(crate::gameplay::AuthoritativePhase::Visibility)
                    .after(ConcealmentSet::ResolveSources)
                    .before(crate::combat::CombatSet::TelemetryAndCues),
            ),
        );
    }

    #[allow(
        clippy::type_complexity,
        clippy::needless_pass_by_value,
        reason = "projection reconciliation owns the complete public participant shape"
    )]
    pub(super) fn sync_public_participant_projections(
        mut commands: Commands,
        fighters: Query<
            (
                &crate::protocol::PlayerId,
                &NetworkEntityId,
                &TeamId,
                &crate::matchplay::FighterDisplayName,
                &crate::matchplay::MatchParticipant,
                Option<&crate::builds::SelectedBuild>,
                Option<&crate::combat::ResolvedWeapon>,
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
            weapon,
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
                weapon_preset_id: weapon.and_then(|value| value.source_preset_id.map(|id| id.0)),
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

    #[allow(
        clippy::type_complexity,
        clippy::needless_pass_by_value,
        clippy::too_many_lines,
        reason = "one bounded source-resolution phase reconciles terrain, fields, and reveal state"
    )]
    fn resolve_membership_and_reveal_locks(
        mut commands: Commands,
        tick: Res<SimulationTick>,
        map: Option<Res<ResolvedMap>>,
        catalog: Res<MapCatalogResource>,
        fields: Query<&ConcealmentFieldState>,
        fighters: Query<
            (
                Entity,
                &NetworkEntityId,
                &TeamId,
                &Position,
                Option<&TerrainConcealmentMembership>,
                Option<&AlliedConcealmentMemberships>,
                Option<&ConcealmentRevealDeadlines>,
                &crate::builds::AbilityState,
                Option<&ForcedRevealSources>,
                Has<Defeated>,
                Has<ActiveCombatant>,
                Has<ObjectiveCarrier>,
            ),
            With<Fighter>,
        >,
    ) {
        let active_sources: HashSet<_> = fighters.iter().map(|(_, id, ..)| id.0).collect();
        let mut active_fields: Vec<_> = fields
            .iter()
            .filter_map(|state| {
                (tick.0 < state.expires_at_tick)
                    .then(|| state.radius().map(|radius| (*state, radius)))
                    .flatten()
            })
            .collect();
        active_fields.sort_by_key(|(state, ..)| state.id);
        for (
            entity,
            _network_id,
            team,
            position,
            prior_membership,
            prior_field_memberships,
            prior_deadlines,
            ability,
            forced,
            defeated,
            active,
            objective_carrier,
        ) in &fighters
        {
            let membership = (!defeated && active && !objective_carrier)
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

            let field_memberships = if !defeated && active && !objective_carrier {
                AlliedConcealmentMemberships::bounded(
                    active_fields
                        .iter()
                        .filter_map(|(state, radius)| {
                            (state.team == *team
                                && field_contains(state.center_vec2(), *radius, position.0))
                            .then_some(state.id)
                        })
                        .collect(),
                )
                .unwrap_or_default()
            } else {
                AlliedConcealmentMemberships::default()
            };
            if prior_field_memberships != Some(&field_memberships) {
                if field_memberships.0.is_empty() {
                    commands
                        .entity(entity)
                        .remove::<AlliedConcealmentMemberships>();
                } else {
                    commands.entity(entity).insert(field_memberships.clone());
                }
            }

            let deadlines = prior_deadlines.copied().unwrap_or_default();
            let mut forced = forced.cloned().unwrap_or_default();
            forced.0.retain(|source| {
                tick.0 < source.expires_at_tick
                    && active_sources.contains(&source.source_network_id.0)
            });
            let mut effective = std::collections::BTreeMap::new();
            for source in &forced.0 {
                effective
                    .entry(source.revealing_team)
                    .and_modify(|deadline: &mut u64| {
                        *deadline = (*deadline).max(source.expires_at_tick);
                    })
                    .or_insert(source.expires_at_tick);
            }
            let self_cloaked_until_tick = match ability.phase {
                crate::builds::AbilityPhase::Cloaked {
                    expires_at_tick, ..
                } if !defeated && active && !objective_carrier && tick.0 < expires_at_tick => {
                    expires_at_tick
                }
                _ => 0,
            };
            commands.entity(entity).insert((
                forced,
                ConcealmentPresentationState {
                    inside_concealing_terrain: membership.is_some(),
                    inside_allied_concealment_field: !field_memberships.0.is_empty(),
                    self_cloaked_until_tick,
                    revealed_until_tick: deadlines
                        .attack_until_tick
                        .max(deadlines.damage_until_tick),
                    forced_reveals: effective
                        .into_iter()
                        .map(|(team, expires_at_tick)| TeamRevealDeadline {
                            team,
                            expires_at_tick,
                        })
                        .collect(),
                },
            ));
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

    #[derive(Clone)]
    struct FighterView {
        entity: Entity,
        team: TeamId,
        network_id: NetworkEntityId,
        position: Vec2,
        reveal_radius: f32,
        connection: Option<Entity>,
        alive: bool,
        concealment: ConcealmentSources,
        forced_reveals: Vec<TeamRevealDeadline>,
        reveal_locked: bool,
    }

    #[allow(
        clippy::type_complexity,
        clippy::needless_pass_by_value,
        clippy::too_many_lines,
        reason = "the observer-decision coordinator materializes one bounded all-pairs snapshot"
    )]
    fn decide_observer_visibility(
        tick: Res<SimulationTick>,
        mut cache: ResMut<ObserverVisibilityCache>,
        mut telemetry: ResMut<ConcealmentTelemetry>,
        mut queued: ResMut<QueuedVisibilityTransitions>,
        fighters: Query<
            (
                Entity,
                &TeamId,
                &NetworkEntityId,
                &Position,
                &crate::builds::ResolvedFighterStats,
                Option<&ControlledBy>,
                Has<Defeated>,
                Has<ActiveCombatant>,
                Option<&TerrainConcealmentMembership>,
                Option<&AlliedConcealmentMemberships>,
                Option<&ConcealmentRevealDeadlines>,
                &crate::builds::AbilityState,
                Option<&ForcedRevealSources>,
                Has<ObjectiveCarrier>,
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
                    fighter_stats,
                    controlled,
                    defeated,
                    active,
                    membership,
                    field_memberships,
                    deadlines,
                    ability,
                    forced,
                    objective_carrier,
                )| FighterView {
                    entity,
                    team: *team,
                    network_id: *network_id,
                    position: position.0,
                    reveal_radius: fighter_stats.reveal_proximity_radius,
                    connection: controlled.map(|value| value.owner),
                    alive: !defeated && active,
                    concealment: if objective_carrier {
                        ConcealmentSources::NONE
                    } else {
                        ConcealmentSources {
                            terrain: membership.is_some() && !defeated,
                            self_cloak: matches!(ability.phase, crate::builds::AbilityPhase::Cloaked { expires_at_tick, .. } if !defeated && tick.0 < expires_at_tick),
                            allied_field: field_memberships
                                .is_some_and(|memberships| !memberships.0.is_empty())
                                && !defeated,
                        }
                    },
                    forced_reveals: forced.map_or_else(Vec::new, |sources| {
                        let mut deadlines = std::collections::BTreeMap::new();
                        for source in &sources.0 {
                            if tick.0 < source.expires_at_tick {
                                deadlines.entry(source.revealing_team).and_modify(|deadline: &mut u64| *deadline = (*deadline).max(source.expires_at_tick)).or_insert(source.expires_at_tick);
                            }
                        }
                        deadlines.into_iter().map(|(team, expires_at_tick)| TeamRevealDeadline { team, expires_at_tick }).collect()
                    }),
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
                let visible = observer_can_see(ObserverVisibilityInput {
                    relation: if allied_or_self {
                        ObserverRelation::SelfOrAlly
                    } else {
                        ObserverRelation::Enemy
                    },
                    observer_alive: observer.alive,
                    concealment: subject.concealment,
                    forced_revealed: subject.forced_reveals.iter().any(|reveal| {
                        reveal.team == observer.team && tick.0 < reveal.expires_at_tick
                    }),
                    subject_reveal_locked: subject.reveal_locked,
                    distance_squared,
                    reveal_radius: observer.reveal_radius,
                });
                if cache.0.get(&key).copied() == Some(visible) {
                    continue;
                }
                cache.0.insert(key, visible);
                let reason =
                    if allied_or_self {
                        VisibilityTransitionReason::SelfOrAlly
                    } else if subject.forced_reveals.iter().any(|reveal| {
                        reveal.team == observer.team && tick.0 < reveal.expires_at_tick
                    }) {
                        VisibilityTransitionReason::RevealLock
                    } else if !subject.concealment.any() {
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
                queued.0.push((subject.entity, connection, visible));
            }
        }
        cache.0.retain(|key, _| current.contains(key));
    }
}

#[cfg(feature = "server")]
pub(crate) use network::ObserverVisibilityCache;
#[cfg(feature = "server")]
pub use server::{ConcealmentSet, ServerConcealmentPlugin};

#[cfg(test)]
mod tests;
