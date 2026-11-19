//! Stable, bounded combat-state evidence for process and impairment verification.

#[cfg(feature = "server")]
use super::CombatCue;
use super::{
    ActiveEffects, AttackDelivery, AttackId, AuthoritativeTick, ConeSprayState, Defeated,
    KnockbackFeedback, LobbedFlight, NetworkEntityId, ProjectileBody, ProjectileDeadline,
    ReplicatedAttackSource, ResolvedWeapon, StraightFlight, WeaponPresetId,
    WeaponRecipeFingerprint, WeaponState, WorldPoint,
};
#[cfg(feature = "server")]
use super::{
    AuthoritativePose, ConeSpray, CurrentHealth, DUMMY_NETWORK_ENTITY, Fighter,
    MAX_COMBAT_EVIDENCE_EVENTS, Position, Projectile, Rotation, unix_epoch_micros,
};
#[cfg(feature = "server")]
use super::{
    COMBAT_CHECKPOINT_LATCH_TICKS, ExternalMotion, LinkOf, SimulationTick, WeaponCatalogResource,
    env,
};
#[cfg(feature = "server")]
use super::{FiringPattern, WeaponCatalog};
#[cfg(feature = "server")]
use bevy::prelude::Resource;
#[cfg(feature = "server")]
use bevy::prelude::*;
use bevy::prelude::{Message, Vec2};
use serde::{Deserialize, Serialize};
#[cfg(feature = "server")]
use std::collections::BTreeMap;
use std::fmt::Write as _;

pub const MAX_STATE_SNAPSHOT_BYTES: usize = 32 * 1024;

/// Network-visible gameplay state keyed only by stable identities.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CombatStateSnapshot {
    pub authoritative_tick: u64,
    pub fighters: Vec<CombatFighterSnapshot>,
    pub projectiles: Vec<CombatProjectileSnapshot>,
    #[serde(default)]
    pub cone_sprays: Vec<CombatConeSpraySnapshot>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CombatFighterSnapshot {
    pub network_entity_id: NetworkEntityId,
    pub selected_build: Option<crate::builds::SelectedBuild>,
    pub resolved_weapon: Option<ResolvedWeapon>,
    pub weapon_state: Option<WeaponState>,
    pub active_effects: Option<ActiveEffects>,
    pub knockback_feedback: Option<KnockbackFeedback>,
    pub defeated: Option<Defeated>,
    pub health: Option<u16>,
    pub position: WorldPoint,
    pub facing: f32,
    pub authoritative_tick: AuthoritativeTick,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CombatProjectileSnapshot {
    pub attack_id: AttackId,
    pub delivery_index: u8,
    pub source_preset_id: Option<WeaponPresetId>,
    pub recipe_fingerprint: Option<WeaponRecipeFingerprint>,
    pub position: WorldPoint,
    pub body: Option<ProjectileBody>,
    pub lobbed_flight: Option<LobbedFlight>,
    pub deadline: Option<ProjectileDeadline>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CombatConeSpraySnapshot {
    pub attack_id: AttackId,
    pub recipe_fingerprint: Option<WeaponRecipeFingerprint>,
    pub state: ConeSprayState,
}

impl CombatConeSpraySnapshot {
    #[must_use]
    pub fn from_components(
        delivery: Option<&AttackDelivery>,
        source: Option<&ReplicatedAttackSource>,
        state: &ConeSprayState,
    ) -> Option<Self> {
        let delivery = delivery?;
        Some(Self {
            attack_id: delivery.attack_id,
            recipe_fingerprint: source.map(|source| source.attack.recipe_fingerprint),
            state: *state,
        })
    }
}

impl CombatProjectileSnapshot {
    #[must_use]
    #[allow(
        clippy::too_many_arguments,
        reason = "the snapshot constructor consumes the complete bounded projectile component view"
    )]
    #[expect(
        clippy::cast_precision_loss,
        reason = "elapsed and total flight ticks are bounded authored projectile lifetimes"
    )]
    pub fn from_components(
        position: WorldPoint,
        delivery: Option<&AttackDelivery>,
        source: Option<&ReplicatedAttackSource>,
        body: Option<&ProjectileBody>,
        lobbed_flight: Option<&LobbedFlight>,
        deadline: Option<&ProjectileDeadline>,
        straight_flight: Option<&StraightFlight>,
        authoritative_tick: u64,
    ) -> Option<Self> {
        let delivery = delivery?;
        let position = straight_flight
            .map(|flight| {
                let elapsed_ticks = authoritative_tick.saturating_sub(flight.launched_at_tick);
                let distance = (elapsed_ticks as f32 * flight.speed
                    / crate::timing::SIMULATION_TICK_HZ_F32)
                    .min(flight.maximum_range)
                    .max(0.0);
                WorldPoint::from(
                    flight.origin.as_vec2() + Vec2::from_angle(flight.facing) * distance,
                )
            })
            .or_else(|| {
                lobbed_flight.map(|flight| {
                    let progress = authoritative_tick.saturating_sub(flight.launched_at_tick)
                        as f32
                        / flight
                            .lands_at_tick
                            .saturating_sub(flight.launched_at_tick)
                            .max(1) as f32;
                    WorldPoint::from(
                        flight.launch.as_vec2()
                            + (flight.landing.as_vec2() - flight.launch.as_vec2())
                                * progress.clamp(0.0, 1.0),
                    )
                })
            })
            .unwrap_or(position);
        Some(Self {
            attack_id: delivery.attack_id,
            delivery_index: delivery.delivery_index,
            source_preset_id: source.and_then(|source| source.attack.source_preset_id),
            recipe_fingerprint: source.map(|source| source.attack.recipe_fingerprint),
            position,
            body: body.copied(),
            lobbed_flight: lobbed_flight.copied(),
            deadline: deadline.copied(),
        })
    }
}

#[must_use]
#[cfg(feature = "server")]
fn is_active_spread_flight(projectile: &CombatProjectileSnapshot, weapons: &WeaponCatalog) -> bool {
    projectile.lobbed_flight.is_none()
        && projectile
            .source_preset_id
            .and_then(|preset_id| weapons.preset(preset_id))
            .is_some_and(|preset| {
                matches!(
                    preset.configuration.recipe.firing,
                    FiringPattern::Spread { .. }
                )
            })
}

#[must_use]
pub fn encode_state_snapshot(snapshot: &CombatStateSnapshot) -> Option<String> {
    postcard::to_allocvec(snapshot)
        .ok()
        .filter(|bytes| bytes.len() <= MAX_STATE_SNAPSHOT_BYTES)
        .map(|bytes| {
            let mut encoded = String::with_capacity(bytes.len() * 2);
            for byte in bytes {
                let _ = write!(encoded, "{byte:02x}");
            }
            encoded
        })
}

#[cfg(feature = "server")]
#[derive(Resource, Default, Debug)]
pub struct CombatOutbox(pub Vec<CombatCue>);

#[cfg(feature = "server")]
#[derive(Resource, Default, Debug)]
pub struct CombatEvidenceSnapshots {
    pub checkpoints: BTreeMap<String, CombatStateSnapshot>,
    pub checkpoint_candidates: BTreeMap<String, Vec<(CombatStateSnapshot, u128)>>,
    pub(super) checkpoint_latched_snapshots: BTreeMap<String, CombatStateSnapshot>,
    pub(super) checkpoint_latch_ticks: BTreeMap<String, u16>,
    pub checkpoint_timestamps: BTreeMap<String, u128>,
    pub state_mutation_timestamps: Vec<(u64, u128)>,
    pub last_encoded_snapshot: Option<String>,
    pub saw_defeat: bool,
    pub pending_checkpoints: Vec<CombatEvidenceCheckpoint>,
}

#[cfg(any(feature = "server", feature = "client"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CombatCheckpoint {
    ActiveScatterFlight,
    ActiveLobFlight,
    ActiveConeSpray,
    ActiveSlow,
    ActiveKnockback,
    Defeat,
    Reset,
}

#[cfg(any(feature = "server", feature = "client"))]
impl CombatCheckpoint {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ActiveScatterFlight => "active_scatter_flight",
            Self::ActiveLobFlight => "active_lob_flight",
            Self::ActiveConeSpray => "active_cone_spray",
            Self::ActiveSlow => "active_slow",
            Self::ActiveKnockback => "active_knockback",
            Self::Defeat => "defeat",
            Self::Reset => "reset",
        }
    }
}

#[cfg(feature = "server")]
fn normalize_checkpoint_snapshot(
    checkpoint: CombatCheckpoint,
    mut snapshot: CombatStateSnapshot,
) -> CombatStateSnapshot {
    if matches!(
        checkpoint,
        CombatCheckpoint::ActiveSlow
            | CombatCheckpoint::ActiveKnockback
            | CombatCheckpoint::Defeat
            | CombatCheckpoint::Reset
    ) {
        snapshot.projectiles.clear();
    }
    for fighter in &mut snapshot.fighters {
        // Component presence can converge one replication frame after its empty value. Treat an
        // absent effect container and an explicitly replicated empty container as the same state;
        // a populated effect remains part of the strict checkpoint payload.
        if fighter.active_effects == Some(ActiveEffects::default()) {
            fighter.active_effects = None;
        }
        // The replicated state can be observed after the named server tick while remaining the
        // same gameplay state. Latency is measured separately from the equality payload.
        fighter.authoritative_tick = AuthoritativeTick::default();
        match checkpoint {
            CombatCheckpoint::ActiveScatterFlight
            | CombatCheckpoint::ActiveLobFlight
            | CombatCheckpoint::ActiveConeSpray => {
                // Transient delivery checkpoints prove stable fighter build/configuration and
                // full projectile identity, reconstructed position, flight data, and deadline.
                fighter.weapon_state = None;
                fighter.active_effects = None;
                fighter.knockback_feedback = None;
                fighter.defeated = None;
                fighter.health = None;
                fighter.position = WorldPoint::from(Vec2::ZERO);
                fighter.facing = 0.0;
            }
            CombatCheckpoint::ActiveSlow => {
                fighter.weapon_state = None;
                fighter.knockback_feedback = None;
                fighter.defeated = None;
                fighter.health = None;
                fighter.position = WorldPoint::from(Vec2::ZERO);
                fighter.facing = 0.0;
            }
            CombatCheckpoint::ActiveKnockback => {
                fighter.weapon_state = None;
                fighter.active_effects = None;
                fighter.defeated = None;
                fighter.health = None;
                fighter.position = WorldPoint::from(Vec2::ZERO);
                fighter.facing = 0.0;
            }
            CombatCheckpoint::Defeat => {
                // Defeat proves the exact terminal health and Defeated marker together with the
                // stable fighter build/configuration. Weapon cadence, transient effects, motion,
                // and pose can advance independently before that marker is observed.
                fighter.weapon_state = None;
                fighter.active_effects = None;
                fighter.knockback_feedback = None;
                if fighter.defeated.is_none() {
                    fighter.health = None;
                }
                fighter.position = WorldPoint::from(Vec2::ZERO);
                fighter.facing = 0.0;
            }
            CombatCheckpoint::Reset => {
                // The neutral dummy is the deterministic reset subject. Other fighters continue
                // attacking while its reset components replicate and therefore contribute only
                // stable identity/build/configuration to this checkpoint.
                if fighter.network_entity_id != DUMMY_NETWORK_ENTITY {
                    fighter.weapon_state = None;
                    fighter.active_effects = None;
                    fighter.knockback_feedback = None;
                    fighter.defeated = None;
                    fighter.health = None;
                }
                fighter.position = WorldPoint::from(Vec2::ZERO);
                fighter.facing = 0.0;
            }
        }
    }
    if checkpoint != CombatCheckpoint::ActiveConeSpray {
        snapshot.cone_sprays.clear();
    }
    snapshot
}

#[cfg(any(feature = "server", feature = "client"))]
#[derive(Message, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CombatEvidenceCheckpoint {
    pub checkpoint: CombatCheckpoint,
    pub snapshot: CombatStateSnapshot,
}

#[cfg(feature = "server")]
fn record_distinct_checkpoint_candidate(
    candidates: &mut Vec<(CombatStateSnapshot, u128)>,
    snapshot: &CombatStateSnapshot,
    timestamp: u128,
) {
    if candidates.len() < MAX_COMBAT_EVIDENCE_EVENTS
        && candidates
            .last()
            .is_none_or(|(candidate, _)| candidate != snapshot)
    {
        candidates.push((snapshot.clone(), timestamp));
    }
}

#[cfg(feature = "server")]
#[allow(clippy::too_many_lines)]
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime; the queries declare the full snapshot view this checkpoint records"
)]
pub(super) fn capture_server_combat_checkpoints(
    tick: Res<SimulationTick>,
    mut evidence: ResMut<CombatEvidenceSnapshots>,
    weapons: Res<WeaponCatalogResource>,
    fighters: Query<
        (
            &NetworkEntityId,
            Option<&crate::builds::SelectedBuild>,
            Option<&crate::builds::ResolvedMatchLoadout>,
            Option<&WeaponState>,
            Option<&ActiveEffects>,
            Option<&Defeated>,
            Option<&ExternalMotion>,
            Option<&KnockbackFeedback>,
            Option<&CurrentHealth>,
            Option<&AuthoritativePose>,
            &Position,
            &Rotation,
            Option<&AuthoritativeTick>,
        ),
        With<Fighter>,
    >,
    projectiles: Query<
        (
            &Position,
            Option<&AttackDelivery>,
            Option<&ReplicatedAttackSource>,
            Option<&ProjectileBody>,
            Option<&LobbedFlight>,
            Option<&ProjectileDeadline>,
            Option<&StraightFlight>,
        ),
        With<Projectile>,
    >,
    cone_sprays: Query<
        (
            Option<&AttackDelivery>,
            Option<&ReplicatedAttackSource>,
            &ConeSprayState,
        ),
        With<ConeSpray>,
    >,
) {
    if !env::var("BRAWLER_NETWORK_ASSERT_COMBAT").is_ok_and(|value| value == "1") {
        return;
    }
    let expired_latches = evidence
        .checkpoint_latch_ticks
        .iter_mut()
        .filter_map(|(checkpoint, ticks)| {
            if *ticks == 0 {
                Some(checkpoint.clone())
            } else {
                *ticks = ticks.saturating_sub(1);
                None
            }
        })
        .collect::<Vec<_>>();
    for checkpoint in expired_latches {
        evidence.checkpoint_latch_ticks.remove(&checkpoint);
        evidence.checkpoint_latched_snapshots.remove(&checkpoint);
    }
    let mut fighter_snapshots = fighters
        .iter()
        .map(
            |(
                network_id,
                selected_build,
                resolved_weapon,
                weapon_state,
                active_effects,
                defeated,
                _external_motion,
                knockback_feedback,
                health,
                authoritative_pose,
                position,
                rotation,
                authoritative_tick,
            )| {
                CombatFighterSnapshot {
                    network_entity_id: *network_id,
                    selected_build: selected_build.copied(),
                    resolved_weapon: resolved_weapon.map(|loadout| loadout.primary_weapon.clone()),
                    weapon_state: weapon_state.copied(),
                    active_effects: active_effects.copied(),
                    knockback_feedback: knockback_feedback.copied(),
                    defeated: defeated.copied(),
                    health: health.map(|health| health.0),
                    position: authoritative_pose
                        .map_or_else(|| WorldPoint::from(position.0), |pose| pose.position),
                    facing: authoritative_pose
                        .map_or_else(|| rotation.as_radians(), |pose| pose.facing),
                    authoritative_tick: authoritative_tick.copied().unwrap_or_default(),
                }
            },
        )
        .collect::<Vec<_>>();
    fighter_snapshots.sort_by_key(|fighter| fighter.network_entity_id.0);
    let mut projectile_snapshots = projectiles
        .iter()
        .filter_map(
            |(position, delivery, source, body, lobbed_flight, deadline, straight_flight)| {
                CombatProjectileSnapshot::from_components(
                    WorldPoint::from(position.0),
                    delivery,
                    source,
                    body,
                    lobbed_flight,
                    deadline,
                    straight_flight,
                    tick.0,
                )
            },
        )
        .collect::<Vec<_>>();
    projectile_snapshots
        .sort_by_key(|projectile| (projectile.attack_id.0, projectile.delivery_index));
    let mut cone_spray_snapshots = cone_sprays
        .iter()
        .filter_map(|(delivery, source, state)| {
            CombatConeSpraySnapshot::from_components(delivery, source, state)
        })
        .collect::<Vec<_>>();
    cone_spray_snapshots.sort_by_key(|spray| spray.attack_id.0);
    let snapshot = CombatStateSnapshot {
        authoritative_tick: tick.0,
        fighters: fighter_snapshots,
        projectiles: projectile_snapshots,
        cone_sprays: cone_spray_snapshots,
    };
    let Some(encoded) = encode_state_snapshot(&snapshot) else {
        return;
    };
    if evidence.last_encoded_snapshot.as_deref() != Some(encoded.as_str()) {
        if evidence.state_mutation_timestamps.len() < MAX_COMBAT_EVIDENCE_EVENTS {
            evidence
                .state_mutation_timestamps
                .push((tick.0, unix_epoch_micros()));
        }
        evidence.last_encoded_snapshot = Some(encoded);
    }
    let has_scatter_flight = snapshot
        .projectiles
        .iter()
        .any(|projectile| is_active_spread_flight(projectile, &weapons.0));
    let has_lob_flight = snapshot
        .projectiles
        .iter()
        .any(|projectile| projectile.lobbed_flight.is_some());
    let has_cone_spray = !snapshot.cone_sprays.is_empty();
    let has_slow = snapshot.fighters.iter().any(|fighter| {
        fighter
            .active_effects
            .is_some_and(|effects| effects.slow.is_some())
    });
    let has_defeat = snapshot
        .fighters
        .iter()
        .any(|fighter| fighter.defeated.is_some());
    evidence.saw_defeat |= has_defeat;
    let has_knockback = fighters
        .iter()
        .any(|(_, _, _, _, _, _, external_motion, _, _, _, _, _, _)| external_motion.is_some());
    let has_reset = evidence.saw_defeat
        && snapshot.fighters.iter().any(|fighter| {
            fighter.network_entity_id == DUMMY_NETWORK_ENTITY && fighter.defeated.is_none()
        });
    for (checkpoint, active) in [
        ("active_scatter_flight", has_scatter_flight),
        ("active_lob_flight", has_lob_flight),
        ("active_cone_spray", has_cone_spray),
        ("active_slow", has_slow),
        ("active_knockback", has_knockback),
        ("defeat", has_defeat),
        ("reset", has_reset),
    ] {
        let repeat_checkpoint =
            checkpoint.starts_with("active_") || checkpoint == "defeat" || checkpoint == "reset";
        let latched = evidence
            .checkpoint_latched_snapshots
            .contains_key(checkpoint);
        if (active || latched)
            && (!evidence.checkpoints.contains_key(checkpoint) || repeat_checkpoint)
        {
            let checkpoint_kind = match checkpoint {
                "active_scatter_flight" => CombatCheckpoint::ActiveScatterFlight,
                "active_lob_flight" => CombatCheckpoint::ActiveLobFlight,
                "active_cone_spray" => CombatCheckpoint::ActiveConeSpray,
                "active_slow" => CombatCheckpoint::ActiveSlow,
                "active_knockback" => CombatCheckpoint::ActiveKnockback,
                "defeat" => CombatCheckpoint::Defeat,
                "reset" => CombatCheckpoint::Reset,
                _ => unreachable!("combat evidence checkpoint is a known name"),
            };
            let current_snapshot = if matches!(checkpoint, "defeat" | "reset") {
                CombatStateSnapshot {
                    projectiles: Vec::new(),
                    ..snapshot.clone()
                }
            } else {
                snapshot.clone()
            };
            let current_snapshot = normalize_checkpoint_snapshot(checkpoint_kind, current_snapshot);
            if active && !latched {
                evidence
                    .checkpoint_latched_snapshots
                    .insert(checkpoint.to_string(), current_snapshot.clone());
                evidence
                    .checkpoint_latch_ticks
                    .insert(checkpoint.to_string(), COMBAT_CHECKPOINT_LATCH_TICKS);
            }
            let checkpoint_snapshot = evidence
                .checkpoint_latched_snapshots
                .get(checkpoint)
                .cloned()
                .unwrap_or(current_snapshot);
            let timestamp = unix_epoch_micros();
            let candidates = evidence
                .checkpoint_candidates
                .entry(checkpoint.to_string())
                .or_default();
            record_distinct_checkpoint_candidate(candidates, &checkpoint_snapshot, timestamp);
            if !evidence.checkpoints.contains_key(checkpoint) {
                evidence
                    .checkpoint_timestamps
                    .insert(checkpoint.to_string(), timestamp);
                evidence
                    .checkpoints
                    .insert(checkpoint.to_string(), checkpoint_snapshot.clone());
            }
            if evidence.pending_checkpoints.len() >= MAX_COMBAT_EVIDENCE_EVENTS {
                continue;
            }
            evidence.pending_checkpoints.push(CombatEvidenceCheckpoint {
                checkpoint: checkpoint_kind,
                snapshot: checkpoint_snapshot,
            });
        }
    }
}

#[cfg(feature = "server")]
pub(super) fn send_combat_evidence_checkpoints(
    mut evidence: ResMut<CombatEvidenceSnapshots>,
    mut senders: Query<
        &mut lightyear::prelude::MessageSender<CombatEvidenceCheckpoint>,
        With<LinkOf>,
    >,
) {
    let checkpoints = std::mem::take(&mut evidence.pending_checkpoints);
    for mut sender in &mut senders {
        for checkpoint in &checkpoints {
            sender.send::<crate::protocol::CombatChannel>(checkpoint.clone());
        }
    }
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;
    use crate::combat::{CombatEventId, SlowEffect, WeaponPhase};

    fn fighter(network_entity_id: NetworkEntityId) -> CombatFighterSnapshot {
        CombatFighterSnapshot {
            network_entity_id,
            selected_build: Some(crate::builds::SelectedBuild {
                recipe_fingerprint: crate::builds::BuildRecipeFingerprint(77),
                revision: crate::builds::BuildRevision(1),
            }),
            resolved_weapon: None,
            weapon_state: Some(WeaponState {
                ammo: 2,
                phase: WeaponPhase::Cooldown { ready_at_tick: 99 },
                ammo_recovery: None,
            }),
            active_effects: Some(ActiveEffects {
                slow: Some(SlowEffect {
                    source_attack_id: AttackId(11),
                    source_network_entity_id: NetworkEntityId(1),
                    movement_multiplier_milli: 700,
                    expires_at_tick: 88,
                }),
                ..default()
            }),
            knockback_feedback: Some(KnockbackFeedback {
                velocity: WorldPoint { x: 3.0, y: -4.0 },
                expires_at_tick: 66,
            }),
            defeated: None,
            health: Some(73),
            position: WorldPoint { x: 9.0, y: 10.0 },
            facing: 1.5,
            authoritative_tick: AuthoritativeTick(55),
        }
    }

    fn projectile() -> CombatProjectileSnapshot {
        CombatProjectileSnapshot {
            attack_id: AttackId(11),
            delivery_index: 2,
            source_preset_id: Some(WeaponPresetId(3)),
            recipe_fingerprint: Some(WeaponRecipeFingerprint(77)),
            position: WorldPoint { x: 12.0, y: 13.0 },
            body: Some(ProjectileBody::circle(6.0)),
            lobbed_flight: None,
            deadline: Some(ProjectileDeadline {
                expires_at_tick: 101,
            }),
        }
    }

    fn snapshot() -> CombatStateSnapshot {
        CombatStateSnapshot {
            authoritative_tick: 55,
            fighters: vec![fighter(DUMMY_NETWORK_ENTITY), fighter(NetworkEntityId(1))],
            projectiles: vec![projectile()],
            cone_sprays: Vec::new(),
        }
    }

    #[test]
    fn scatter_checkpoint_uses_authored_spread_pattern_from_stable_preset_identity() {
        let weapons = WeaponCatalog::embedded().unwrap();
        let mut candidate = projectile();

        candidate.source_preset_id = Some(WeaponPresetId(2));
        assert!(is_active_spread_flight(&candidate, &weapons));

        candidate.source_preset_id = Some(WeaponPresetId(1));
        assert!(!is_active_spread_flight(&candidate, &weapons));

        candidate.source_preset_id = None;
        assert!(!is_active_spread_flight(&candidate, &weapons));
    }

    #[test]
    fn transient_checkpoint_schemas_preserve_the_named_payload_only() {
        let slow = normalize_checkpoint_snapshot(CombatCheckpoint::ActiveSlow, snapshot());
        assert!(slow.projectiles.is_empty());
        assert!(slow.fighters[0].active_effects.unwrap().slow.is_some());
        assert_eq!(slow.fighters[0].health, None);
        assert_eq!(slow.fighters[0].knockback_feedback, None);
        assert_eq!(
            slow.fighters[0].selected_build,
            snapshot().fighters[0].selected_build
        );

        let knockback =
            normalize_checkpoint_snapshot(CombatCheckpoint::ActiveKnockback, snapshot());
        assert!(knockback.projectiles.is_empty());
        assert_eq!(
            knockback.fighters[0].knockback_feedback,
            snapshot().fighters[0].knockback_feedback
        );
        assert_eq!(knockback.fighters[0].active_effects, None);
        assert_eq!(knockback.fighters[0].health, None);
    }

    #[test]
    fn flight_checkpoint_preserves_exact_projectile_identity_and_deadline() {
        let normalized =
            normalize_checkpoint_snapshot(CombatCheckpoint::ActiveLobFlight, snapshot());
        assert_eq!(normalized.projectiles, snapshot().projectiles);
        assert_eq!(normalized.fighters[0].health, None);
        let mut changed = normalized.clone();
        changed.projectiles[0].deadline = Some(ProjectileDeadline {
            expires_at_tick: 102,
        });
        assert_ne!(normalized, changed);
        changed = normalized.clone();
        changed.projectiles[0].recipe_fingerprint = Some(WeaponRecipeFingerprint(78));
        assert_ne!(normalized, changed);
    }

    #[test]
    fn defeat_and_reset_exclude_unrelated_fighter_volatility() {
        let mut source = snapshot();
        source.fighters[0].defeated = Some(Defeated {
            event_id: CombatEventId(9),
        });
        source.fighters[0].health = Some(0);
        let defeat = normalize_checkpoint_snapshot(CombatCheckpoint::Defeat, source);
        assert_eq!(defeat.fighters[0].health, Some(0));
        assert!(defeat.fighters[0].defeated.is_some());
        assert_eq!(defeat.fighters[1].health, None);

        let reset = normalize_checkpoint_snapshot(CombatCheckpoint::Reset, snapshot());
        assert_eq!(reset.fighters[0].health, Some(73));
        assert!(reset.fighters[0].weapon_state.is_some());
        assert_eq!(reset.fighters[1].health, None);
        assert_eq!(reset.fighters[1].weapon_state, None);
    }

    #[cfg(feature = "server")]
    #[test]
    fn server_candidate_history_deduplicates_latches_without_starving_later_states() {
        let first = snapshot();
        let mut second = first.clone();
        second.authoritative_tick += 1;
        let mut candidates = Vec::new();
        for timestamp in 0..MAX_COMBAT_EVIDENCE_EVENTS as u128 {
            record_distinct_checkpoint_candidate(&mut candidates, &first, timestamp);
        }
        record_distinct_checkpoint_candidate(&mut candidates, &second, 999);

        assert_eq!(candidates, vec![(first, 0), (second, 999)]);
    }
}
