//! Server terrain wire handling: recovery-request validation, snapshot construction, and
//! the ordered publication drain over the terrain channel.

#![allow(
    clippy::wildcard_imports,
    reason = "the shared model mirror keeps the wire shapes nameable beside their rules"
)]

use crate::protocol::TerrainChannel;
use crate::server::{ServerSession, ServerSessionPhase};
use crate::terrain::TerrainSet;
use crate::terrain::authority::{TerrainOutbox, TerrainRecoveryCache, TerrainRoot};
use crate::terrain::grid as terrain_grid;
use crate::terrain::model::*;
use crate::terrain::telemetry::{
    TerrainRecoveryRejection, TerrainTelemetry, TerrainTelemetryOutcome, TerrainTelemetryRecord,
};
use bevy::prelude::*;
use lightyear::prelude::{Disconnected, LinkOf, MessageReceiver, MessageSender};

/// Serialized ceiling for one recovery request: three fixed integers.
pub const MAX_TERRAIN_REQUEST_BYTES: usize = 32;

/// Per-link recovery cooldown carried on the server link entity. Despawning the link
/// discards it; nothing survives a reconnect.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct TerrainRecoveryCooldown {
    pub last_served_tick: u64,
}

fn rejection_record(
    tick: u64,
    root: TerrainRoot,
    reason: TerrainRecoveryRejection,
) -> TerrainTelemetryRecord {
    TerrainTelemetryRecord {
        tick,
        map_instance_id: root.map_instance_id,
        revision: root.revision,
        source_attack_id: None,
        delivery_index: None,
        brush: None,
        affected_chunks: Vec::new(),
        erased_cells: 0,
        rebuilt_colliders: 0,
        serialized_event_bytes: None,
        outcome: TerrainTelemetryOutcome::RecoveryRejected { reason },
    }
}

/// Validate and serve recovery requests from accepted links only. Rejections never
/// mutate terrain and never produce a response. The cache and outbox exist only
/// between terrain install and teardown.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
#[allow(clippy::too_many_arguments)]
pub(super) fn receive_terrain_recovery_requests(
    mut commands: Commands,
    tick: Res<crate::timing::SimulationTick>,
    roots: Query<&TerrainRoot>,
    cache: Option<Res<TerrainRecoveryCache>>,
    mut outbox: Option<ResMut<TerrainOutbox>>,
    mut telemetry: ResMut<TerrainTelemetry>,
    links: Query<(&ServerSession, Has<Disconnected>), With<LinkOf>>,
    mut requests: Query<
        (
            Entity,
            Option<&mut TerrainRecoveryCooldown>,
            &mut MessageReceiver<TerrainRecoveryRequest>,
        ),
        With<LinkOf>,
    >,
) {
    let (Some(cache), Some(outbox)) = (cache.as_deref(), outbox.as_deref_mut()) else {
        return;
    };
    let Ok(root) = roots.single() else {
        return;
    };
    let root = *root;
    for (link, mut cooldown, mut receiver) in &mut requests {
        for request in receiver.receive() {
            telemetry.record_recovery_request();
            let last_served = cooldown.as_deref().map(|state| state.last_served_tick);
            let rejection = validate_recovery_request(
                link,
                &request,
                &root,
                &links,
                last_served,
                outbox,
                tick.0,
            );
            let Some(reason) = rejection else {
                serve_recovery_snapshot(link, &root, cache, outbox, &mut telemetry, tick.0);
                match cooldown.as_deref_mut() {
                    Some(state) => state.last_served_tick = tick.0,
                    None => {
                        commands.entity(link).insert(TerrainRecoveryCooldown {
                            last_served_tick: tick.0,
                        });
                    }
                }
                continue;
            };
            telemetry.record(rejection_record(tick.0, root, reason));
        }
    }
}

/// Build, measure, and stage one full snapshot response for an admitted request.
fn serve_recovery_snapshot(
    link: Entity,
    root: &TerrainRoot,
    cache: &TerrainRecoveryCache,
    outbox: &mut TerrainOutbox,
    telemetry: &mut TerrainTelemetry,
    tick: u64,
) {
    let snapshot =
        terrain_grid::recovery_snapshot(&cache.chunks, root.generation(), cache.revision);
    let bytes = terrain_grid::recovery_snapshot_bytes(&snapshot)
        .expect("validated occupancy snapshot serializes");
    assert!(
        bytes <= MAX_TERRAIN_RECOVERY_BYTES,
        "recovery snapshot of {bytes} bytes exceeds the {MAX_TERRAIN_RECOVERY_BYTES} byte ceiling"
    );
    outbox.push_recovery_response(link, snapshot);
    telemetry.record(TerrainTelemetryRecord {
        tick,
        map_instance_id: root.map_instance_id,
        revision: cache.revision,
        source_attack_id: None,
        delivery_index: None,
        brush: None,
        affected_chunks: Vec::new(),
        erased_cells: 0,
        rebuilt_colliders: 0,
        serialized_event_bytes: None,
        outcome: TerrainTelemetryOutcome::RecoverySent {
            bytes,
            chunks: cache.chunks.len(),
        },
    });
}

/// The exact rejection reason for one request, or `None` when it may be served.
fn validate_recovery_request(
    link: Entity,
    request: &TerrainRecoveryRequest,
    root: &TerrainRoot,
    links: &Query<(&ServerSession, Has<Disconnected>), With<LinkOf>>,
    last_served_tick: Option<u64>,
    outbox: &TerrainOutbox,
    tick: u64,
) -> Option<TerrainRecoveryRejection> {
    let accepted = links.get(link).is_ok_and(|(session, disconnected)| {
        matches!(session.phase, ServerSessionPhase::Active { .. }) && !disconnected
    });
    if !accepted {
        return Some(TerrainRecoveryRejection::UnknownLink);
    }
    // The requester identity is the link entity; the request cannot name any target.
    let request_bytes = postcard::to_allocvec(request).map_or(usize::MAX, |bytes| bytes.len());
    if root.match_id.is_none() || request.generation != root.generation() {
        return Some(TerrainRecoveryRejection::WrongGeneration);
    }
    if request_bytes > MAX_TERRAIN_REQUEST_BYTES {
        return Some(TerrainRecoveryRejection::OversizedRequest);
    }
    if last_served_tick
        .is_some_and(|last| tick.saturating_sub(last) < TERRAIN_RECOVERY_COOLDOWN_TICKS)
    {
        return Some(TerrainRecoveryRejection::CooldownActive);
    }
    if outbox
        .recovery_responses
        .iter()
        .any(|(staged_link, _)| *staged_link == link)
    {
        return Some(TerrainRecoveryRejection::ResponseAlreadyStaged);
    }
    None
}

/// Drain the terrain outbox over the ordered reliable terrain channel: reset marker,
/// recovery responses, then live events, only to accepted connected links.
pub(super) fn publish_terrain_traffic(
    outbox: Option<ResMut<TerrainOutbox>>,
    links: Query<(Entity, &ServerSession, Has<Disconnected>), With<LinkOf>>,
    mut reset_senders: Query<&mut MessageSender<TerrainResetEvent>, With<LinkOf>>,
    mut snapshot_senders: Query<&mut MessageSender<TerrainRecoverySnapshot>, With<LinkOf>>,
    mut event_senders: Query<&mut MessageSender<TerrainDestructionEvent>, With<LinkOf>>,
) {
    let Some(mut outbox) = outbox else {
        return;
    };
    let accepted: Vec<Entity> = links
        .iter()
        .filter(|(_, session, disconnected)| {
            matches!(session.phase, ServerSessionPhase::Active { .. }) && !disconnected
        })
        .map(|(link, _, _)| link)
        .collect();
    if let Some(reset) = outbox.reset.take() {
        for link in &accepted {
            if let Ok(mut sender) = reset_senders.get_mut(*link) {
                sender.send::<TerrainChannel>(reset);
            }
        }
    }
    let responses: Vec<_> = outbox.recovery_responses.drain(..).collect();
    for (link, snapshot) in responses {
        if let Ok(mut sender) = snapshot_senders.get_mut(link) {
            sender.send::<TerrainChannel>(snapshot);
        } else {
            outbox.dropped_events = outbox.dropped_events.saturating_add(1);
        }
    }
    let events: Vec<_> = outbox.events.drain(..).collect();
    for event in events {
        for link in &accepted {
            if let Ok(mut sender) = event_senders.get_mut(*link) {
                sender.send::<TerrainChannel>(event.clone());
            }
        }
    }
}
/// Register the terrain wire systems against the shared composition. Message receive
/// completes in `PreUpdate`, so the per-frame request receiver needs no explicit
/// ordering; publication drains every fixed tick inside the terrain chain.
pub fn register_terrain_network(app: &mut App) {
    app.add_systems(Update, receive_terrain_recovery_requests);
    app.add_systems(
        FixedPostUpdate,
        publish_terrain_traffic.in_set(TerrainSet::Publish),
    );
}
