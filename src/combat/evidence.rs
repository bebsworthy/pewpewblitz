//! Stable, bounded combat-state evidence for process and impairment verification.

#[cfg(feature = "server")]
use super::CombatCue;
#[cfg(feature = "client")]
use super::client::ClientCombatObservation;
#[allow(clippy::wildcard_imports)]
#[cfg(any(feature = "server", feature = "client"))]
use super::*;
use super::{
    ActiveEffects, AttackDelivery, AttackId, AuthoritativeTick, Defeated, KnockbackFeedback,
    NetworkEntityId, ProjectileBody, ProjectileDeadline, ReplicatedAttackSource, ResolvedWeapon,
    StraightFlight, WeaponRecipeFingerprint, WeaponState, WorldPoint,
};
#[cfg(feature = "client")]
use atomic_write_file::AtomicWriteFile;
#[cfg(feature = "server")]
use bevy::prelude::Resource;
use bevy::prelude::{Message, Vec2};
use serde::{Deserialize, Serialize};
#[cfg(feature = "server")]
use std::collections::BTreeMap;
use std::fmt::Write as _;
#[cfg(feature = "client")]
use std::io::Write as _;

pub const MAX_STATE_SNAPSHOT_BYTES: usize = 32 * 1024;
#[cfg(feature = "client")]
const MAX_PENDING_CHECKPOINTS_PER_KIND: usize = 32;

/// Network-visible gameplay state keyed only by stable identities.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CombatStateSnapshot {
    pub authoritative_tick: u64,
    pub fighters: Vec<CombatFighterSnapshot>,
    pub projectiles: Vec<CombatProjectileSnapshot>,
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
    pub presentation_profile_id: Option<WeaponPresentationProfileId>,
    pub recipe_fingerprint: Option<WeaponRecipeFingerprint>,
    pub position: WorldPoint,
    pub body: Option<ProjectileBody>,
    pub lobbed_flight: Option<LobbedFlight>,
    pub deadline: Option<ProjectileDeadline>,
}

impl CombatProjectileSnapshot {
    #[must_use]
    #[allow(
        clippy::too_many_arguments,
        reason = "the snapshot constructor consumes the complete bounded projectile component view"
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
                let distance = (elapsed_ticks as f32 * flight.speed / 60.0)
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
            presentation_profile_id: source.map(|source| source.attack.presentation_profile_id),
            recipe_fingerprint: source.map(|source| source.attack.recipe_fingerprint),
            position,
            body: body.copied(),
            lobbed_flight: lobbed_flight.copied(),
            deadline: deadline.copied(),
        })
    }
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
            Self::ActiveSlow => "active_slow",
            Self::ActiveKnockback => "active_knockback",
            Self::Defeat => "defeat",
            Self::Reset => "reset",
        }
    }
}

#[cfg(any(feature = "server", feature = "client"))]
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
            CombatCheckpoint::ActiveScatterFlight | CombatCheckpoint::ActiveLobFlight => {
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

#[cfg(feature = "client")]
fn enqueue_pending_checkpoint(
    pending: &mut Vec<CombatEvidenceCheckpoint>,
    checkpoint: CombatEvidenceCheckpoint,
) {
    let checkpoint_kind = checkpoint.checkpoint;
    let same_kind = pending
        .iter()
        .filter(|pending| pending.checkpoint == checkpoint_kind)
        .count();
    if same_kind >= MAX_PENDING_CHECKPOINTS_PER_KIND
        && let Some(oldest) = pending
            .iter()
            .position(|pending| pending.checkpoint == checkpoint_kind)
    {
        pending.remove(oldest);
    }
    if pending.len() < MAX_COMBAT_EVIDENCE_EVENTS {
        pending.push(checkpoint);
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
    let snapshot = CombatStateSnapshot {
        authoritative_tick: tick.0,
        fighters: fighter_snapshots,
        projectiles: projectile_snapshots,
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
    let has_scatter_flight = snapshot.projectiles.iter().any(|projectile| {
        projectile.presentation_profile_id == Some(WeaponPresentationProfileId(2))
            && projectile.lobbed_flight.is_none()
    });
    let has_lob_flight = snapshot
        .projectiles
        .iter()
        .any(|projectile| projectile.lobbed_flight.is_some());
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

#[cfg(feature = "client")]
pub fn receive_combat_evidence_checkpoints(
    mut observation: ResMut<ClientCombatObservation>,
    mut receivers: Query<
        Option<&mut lightyear::prelude::MessageReceiver<CombatEvidenceCheckpoint>>,
        With<lightyear::prelude::client::Client>,
    >,
) {
    for receiver in &mut receivers {
        let Some(mut receiver) = receiver else {
            continue;
        };
        for checkpoint in receiver.receive() {
            enqueue_pending_checkpoint(&mut observation.expected_checkpoints, checkpoint);
        }
    }
}

#[cfg(feature = "client")]
#[allow(clippy::too_many_lines)]
#[allow(
    clippy::type_complexity,
    reason = "the queries declare the full snapshot view this checkpoint records"
)]
pub fn capture_client_combat_checkpoints(
    mut observation: ResMut<ClientCombatObservation>,
    fighters: Query<
        (
            &NetworkEntityId,
            Option<&crate::builds::SelectedBuild>,
            Option<&crate::builds::ResolvedMatchLoadout>,
            Option<&WeaponState>,
            Option<&ActiveEffects>,
            Option<&KnockbackFeedback>,
            Option<&Defeated>,
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
) {
    if observation.ready_file.is_none() {
        return;
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
                knockback_feedback,
                defeated,
                health,
                authoritative_pose,
                position,
                rotation,
                authoritative_tick,
            )| CombatFighterSnapshot {
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
            },
        )
        .collect::<Vec<_>>();
    fighter_snapshots.sort_by_key(|fighter| fighter.network_entity_id.0);
    let authoritative_tick = fighter_snapshots
        .iter()
        .map(|fighter| fighter.authoritative_tick.0)
        .max()
        .unwrap_or(0);
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
                    authoritative_tick,
                )
            },
        )
        .collect::<Vec<_>>();
    projectile_snapshots
        .sort_by_key(|projectile| (projectile.attack_id.0, projectile.delivery_index));
    let snapshot = CombatStateSnapshot {
        authoritative_tick,
        fighters: fighter_snapshots,
        projectiles: projectile_snapshots,
    };
    observation
        .snapshot_history
        .insert(authoritative_tick, snapshot.clone());
    while observation.snapshot_history.len() > MAX_COMBAT_SNAPSHOT_HISTORY {
        observation.snapshot_history.pop_first();
    }
    let Some(encoded) = encode_state_snapshot(&snapshot) else {
        return;
    };
    if observation.last_encoded_snapshot.as_deref() != Some(encoded.as_str()) {
        if observation.state_mutation_timestamps.len() < MAX_COMBAT_EVIDENCE_EVENTS {
            observation
                .state_mutation_timestamps
                .push((authoritative_tick, unix_epoch_micros()));
        }
        observation.last_encoded_snapshot = Some(encoded);
    }
    let expected_checkpoints = std::mem::take(&mut observation.expected_checkpoints);
    let mut unmatched_checkpoints = Vec::new();
    for expected in expected_checkpoints {
        let checkpoint = expected.checkpoint.as_str();
        if observation
            .checkpoint_matches
            .get(checkpoint)
            .is_some_and(|matches| matches.len() >= 16)
        {
            continue;
        }
        // Reconstruct the checkpoint tick from replicated immutable flight data. The rendered
        // Position is intentionally interpolated and the fighter tick can advance before an
        // ordered evidence message is consumed, so comparing only the latest render snapshot
        // makes short-lived projectile checkpoints impossible to observe exactly.
        let mut checkpoint_fighters = snapshot.fighters.clone();
        for fighter in &mut checkpoint_fighters {
            fighter.authoritative_tick = AuthoritativeTick(expected.snapshot.authoritative_tick);
        }
        let mut checkpoint_projectiles = projectiles
            .iter()
            .filter(|(_, _, _, _, lobbed, deadline, straight)| {
                let launched_at_tick = straight
                    .map(|flight| flight.launched_at_tick)
                    .or_else(|| lobbed.map(|flight| flight.launched_at_tick))
                    .unwrap_or(expected.snapshot.authoritative_tick);
                let expires_at_tick = deadline
                    .map(|deadline| deadline.expires_at_tick)
                    .or_else(|| lobbed.map(|flight| flight.lands_at_tick))
                    .unwrap_or(u64::MAX);
                launched_at_tick <= expected.snapshot.authoritative_tick
                    && expected.snapshot.authoritative_tick <= expires_at_tick
            })
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
                        expected.snapshot.authoritative_tick,
                    )
                },
            )
            .collect::<Vec<_>>();
        checkpoint_projectiles
            .sort_by_key(|projectile| (projectile.attack_id.0, projectile.delivery_index));
        let checkpoint_snapshot = normalize_checkpoint_snapshot(
            expected.checkpoint,
            CombatStateSnapshot {
                authoritative_tick: expected.snapshot.authoritative_tick,
                fighters: checkpoint_fighters,
                projectiles: checkpoint_projectiles,
            },
        );
        let Some(snapshot) = observation
            .snapshot_history
            .values()
            .chain(std::iter::once(&snapshot))
            .chain(std::iter::once(&checkpoint_snapshot))
            .find_map(|candidate| {
                let candidate = if matches!(checkpoint, "defeat" | "reset") {
                    CombatStateSnapshot {
                        projectiles: Vec::new(),
                        ..candidate.clone()
                    }
                } else {
                    candidate.clone()
                };
                let mut candidate = normalize_checkpoint_snapshot(expected.checkpoint, candidate);
                // Preserve the server checkpoint tick long enough to reconstruct transient
                // deliveries, then compare the gameplay payload independently of when the
                // replicated state was observed on this client.
                candidate.authoritative_tick = expected.snapshot.authoritative_tick;
                (candidate == expected.snapshot).then_some(candidate)
            })
        else {
            if observation.checkpoints.contains_key(checkpoint) {
                continue;
            }
            unmatched_checkpoints.push(expected);
            continue;
        };
        let matches = observation
            .checkpoint_matches
            .entry(checkpoint.to_string())
            .or_default();
        if !matches.iter().any(|candidate| candidate == &snapshot) {
            matches.push(snapshot.clone());
        }
        observation
            .checkpoint_timestamps
            .entry(checkpoint.to_string())
            .or_insert_with(unix_epoch_micros);
        observation
            .checkpoints
            .entry(checkpoint.to_string())
            .or_insert(snapshot);
    }
    observation.expected_checkpoints = unmatched_checkpoints;
}

#[cfg(feature = "client")]
#[allow(clippy::too_many_lines)]
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime; the query declares this system's complete world view inline at its schedule boundary"
)]
pub fn record_headless_combat_observation(
    mut observation: ResMut<ClientCombatObservation>,
    mut status: ResMut<ClientCombatEvidenceStatus>,
    config: Res<crate::config::ClientNetworkConfig>,
    automation: Res<crate::client::HeadlessAutomation>,
    fighters: Query<
        (
            &NetworkEntityId,
            &CurrentHealth,
            &WeaponState,
            &FighterDefinitionId,
            Option<&Defeated>,
        ),
        With<Fighter>,
    >,
) {
    let Some(path) = observation.ready_file.clone() else {
        return;
    };
    if automation
        .simulation_ticks
        .is_some_and(|limit| automation.elapsed_ticks < limit)
    {
        return;
    }
    if !observation.wrote_ready && observation.waiting_reported_at_tick.is_none() {
        let required = config
            .weapon_preset
            .map(required_client_checkpoints)
            .unwrap_or_default();
        let missing = required
            .iter()
            .filter(|checkpoint| !observation.checkpoints.contains_key(**checkpoint))
            .copied()
            .collect::<Vec<_>>();
        if !observation.saw_defeat || !observation.saw_reset || !missing.is_empty() {
            let history_saw_slow = observation.snapshot_history.values().any(|snapshot| {
                snapshot.fighters.iter().any(|fighter| {
                    fighter
                        .active_effects
                        .is_some_and(|effects| effects.slow.is_some())
                })
            });
            let history_saw_knockback = observation.snapshot_history.values().any(|snapshot| {
                snapshot
                    .fighters
                    .iter()
                    .any(|fighter| fighter.knockback_feedback.is_some())
            });
            observation.waiting_reported_at_tick = Some(automation.elapsed_ticks);
            warn!(
                saw_defeat = observation.saw_defeat,
                saw_reset = observation.saw_reset,
                history_saw_slow,
                history_saw_knockback,
                ?missing,
                pending_checkpoints = observation.expected_checkpoints.len(),
                "headless client is extending its run until combat evidence completes"
            );
        }
    }
    if observation.wrote_ready
        || !observation.saw_defeat
        || !observation.saw_reset
        || config.weapon_preset.is_some_and(|preset| {
            required_client_checkpoints(preset)
                .iter()
                .any(|checkpoint| !observation.checkpoints.contains_key(*checkpoint))
        })
    {
        return;
    }
    let Some(_) = fighters
        .iter()
        .find(|(network_id, _, _, _, _)| network_id.0 != DUMMY_NETWORK_ENTITY.0)
    else {
        return;
    };
    let mut report = format!(
        "client_elapsed_ms={}\nclient_observation_epoch_us={}\ncue_count={}\nstate_mutation_count={}\npending_checkpoint_count={}\ndropped_cue_stream={}\ndropped_cue_timestamps={}\n",
        observation.started_at.elapsed().as_millis(),
        unix_epoch_micros(),
        observation.cue_stream.len(),
        observation.state_mutation_timestamps.len(),
        observation.expected_checkpoints.len(),
        observation.dropped_cue_stream,
        observation.dropped_cue_timestamps,
    );
    for (checkpoint, snapshot) in &observation.checkpoints {
        if let Some(encoded) = encode_state_snapshot(snapshot) {
            let _ = writeln!(report, "checkpoint_{checkpoint}={encoded}");
            let _ = writeln!(
                report,
                "checkpoint_{checkpoint}_tick={}",
                snapshot.authoritative_tick
            );
        }
        if let Some(timestamp) = observation.checkpoint_timestamps.get(checkpoint) {
            let _ = writeln!(
                report,
                "checkpoint_{checkpoint}_observed_epoch_us={timestamp}"
            );
        }
    }
    for (checkpoint, snapshots) in &observation.checkpoint_matches {
        for snapshot in snapshots {
            if let Some(encoded) = encode_state_snapshot(snapshot) {
                let _ = writeln!(report, "checkpoint_{checkpoint}_candidate={encoded}");
            }
        }
    }
    for (tick, timestamp) in &observation.state_mutation_timestamps {
        let _ = writeln!(report, "state_mutation_tick={tick}_epoch_us={timestamp}");
    }
    for (shot_id, timestamp) in &observation.cue_timestamps {
        let _ = writeln!(report, "cue_shot_id={}_epoch_us={}", shot_id.0, timestamp);
    }
    for cue in &observation.cue_stream {
        let _ = writeln!(report, "cue_stream={}", encode_combat_cue(cue));
    }
    match write_client_combat_evidence(&path, &report) {
        Ok(()) => {
            observation.wrote_ready = true;
            status.ready = true;
            info!(path = %path.display(), "headless client observed combat defeat and reset");
        }
        Err(error) => warn!(
            path = %path.display(),
            ?error,
            "headless combat observation write failed"
        ),
    }
}

#[cfg(feature = "client")]
fn write_client_combat_evidence(path: &std::path::Path, report: &str) -> Result<(), String> {
    let mut file = AtomicWriteFile::open(path)
        .map_err(|error| format!("could not open atomic combat evidence file: {error}"))?;
    file.write_all(report.as_bytes())
        .map_err(|error| format!("could not write combat evidence: {error}"))?;
    file.commit()
        .map_err(|error| format!("could not publish combat evidence: {error}"))
}

#[cfg(feature = "client")]
fn required_client_checkpoints(preset_id: u16) -> &'static [&'static str] {
    match preset_id {
        1 => &["defeat", "reset"],
        2 => &["active_scatter_flight", "defeat", "reset"],
        3 => &[
            "active_lob_flight",
            "active_slow",
            "active_knockback",
            "defeat",
            "reset",
        ],
        4 => &["active_knockback", "defeat", "reset"],
        _ => &[],
    }
}

#[cfg(all(test, any(feature = "server", feature = "client")))]
mod tests {
    use super::*;
    #[cfg(feature = "client")]
    use std::env;

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
            presentation_profile_id: Some(WeaponPresentationProfileId(3)),
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
        }
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

    #[cfg(feature = "client")]
    #[test]
    fn client_combat_evidence_is_atomically_published() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let directory = env::temp_dir().join(format!(
            "brawler-combat-evidence-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).expect("create evidence test directory");
        let path = directory.join("client-1.ready");

        write_client_combat_evidence(&path, "complete-evidence\n")
            .expect("publish combat evidence");

        assert_eq!(
            std::fs::read_to_string(&path).expect("read published combat evidence"),
            "complete-evidence\n"
        );
        std::fs::remove_dir_all(directory).expect("remove evidence test directory");
    }

    #[cfg(feature = "client")]
    #[test]
    fn repeated_transient_checkpoints_cannot_starve_durable_checkpoints() {
        let mut pending = Vec::new();
        for tick in 0..(MAX_PENDING_CHECKPOINTS_PER_KIND * 4) {
            let mut snapshot = snapshot();
            snapshot.authoritative_tick = tick as u64;
            enqueue_pending_checkpoint(
                &mut pending,
                CombatEvidenceCheckpoint {
                    checkpoint: CombatCheckpoint::ActiveScatterFlight,
                    snapshot,
                },
            );
        }
        enqueue_pending_checkpoint(
            &mut pending,
            CombatEvidenceCheckpoint {
                checkpoint: CombatCheckpoint::Defeat,
                snapshot: snapshot(),
            },
        );
        enqueue_pending_checkpoint(
            &mut pending,
            CombatEvidenceCheckpoint {
                checkpoint: CombatCheckpoint::Reset,
                snapshot: snapshot(),
            },
        );

        assert_eq!(
            pending
                .iter()
                .filter(|pending| pending.checkpoint == CombatCheckpoint::ActiveScatterFlight)
                .count(),
            MAX_PENDING_CHECKPOINTS_PER_KIND
        );
        assert!(
            pending
                .iter()
                .any(|pending| pending.checkpoint == CombatCheckpoint::Defeat)
        );
        assert!(
            pending
                .iter()
                .any(|pending| pending.checkpoint == CombatCheckpoint::Reset)
        );
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
