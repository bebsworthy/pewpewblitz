//! Lightyear server IO for one routed worker IPC endpoint.
//!
//! This module deliberately stops at the Lightyear [`Link`] boundary.  BRPK records are decoded
//! here, but control frames remain owned by the worker lifecycle/control-plane code.  A control
//! reader reports EOF or a decoded peer close by triggering [`RoutedWorkerFailure`] or
//! [`RoutedPeerClose`].

use bevy::prelude::*;
use brawler_routing::{
    CodecError, IpcIoError, PacketDirection, PacketRecord, PeerId, ROUTED_LINK_MTU, RouteId,
    UnixWorkerChannels, WorkerId,
};
use lightyear::core::time::Instant;
use lightyear::link::{
    Link, LinkMtu, LinkReceiveSystems, LinkSystems, RecvPayload, SendPayload, Unlink, UnlinkReason,
    recv_payload_from_bytes,
};
use lightyear::prelude::{LinkOf, Linked};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const DEFAULT_RECEIVE_BURST: usize = 64;
const DEFAULT_SEND_BURST: usize = 64;
const MAX_CLOSED_PEERS: usize = 128;
/// Maximum number of already-Netcode-wrapped payloads retained by the adapter for one peer.
///
/// The routed adapter runs after Lightyear's Netcode send systems.  It must take ownership of
/// every transformed payload in the child link in the same frame; leaving one in `Link::send`
/// would cause Netcode to encrypt it a second time on the next frame.  This bound keeps the
/// adapter-owned handoff finite when the IPC writer is backpressured.
const MAX_PENDING_SEND_PAYLOADS_PER_PEER: usize = 128;

/// Identifies the routed supervisor/worker peer represented by a Lightyear child link.
///
/// These IDs are routing identities, not Netcode client IDs and not Bevy entities.  The Netcode
/// connection layer adds its authenticated `RemoteId` after consuming the first payload.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoutedPeer {
    /// The worker endpoint that owns this peer.
    pub worker: Entity,
    /// Stable supervisor route identity.
    pub route_id: RouteId,
    /// Stable routed peer identity.
    pub peer_id: PeerId,
}

/// A control-plane indication that the worker's packet/control stream has failed.
///
/// The control decoder is intentionally outside this plugin.  It can trigger this event for a
/// malformed/failed control stream; packet EOF and packet-frame identity failures trigger it from
/// the packet system itself.
#[derive(EntityEvent, Clone, Debug)]
pub struct RoutedWorkerFailure {
    /// Worker endpoint entity receiving the indication.
    #[event_target]
    pub worker: Entity,
    /// Bounded, non-secret lifecycle reason.
    pub reason: UnlinkReason,
}

/// A decoded control-plane peer close indication.
///
/// The event is the only control interaction this transport needs.  Decoding BRCT and deciding
/// whether a close is authorized remains in the worker lifecycle layer.
#[derive(EntityEvent, Clone, Debug)]
pub struct RoutedPeerClose {
    /// Worker endpoint entity receiving the close.
    #[event_target]
    pub worker: Entity,
    /// Route being closed.
    pub route_id: RouteId,
    /// Peer being closed.
    pub peer_id: PeerId,
    /// Close reason propagated to Lightyear.
    pub reason: UnlinkReason,
}

/// One Lightyear server endpoint backed by the worker's packet IPC stream.
///
/// Attach this component to the same entity as an existing Lightyear `Server`/`NetcodeServer`
/// endpoint and trigger the normal Lightyear `Start` event.  The routed endpoint marks its
/// transport linked and then creates one child `LinkOf` per `(RouteId, PeerId)` packet identity.
#[derive(Component)]
pub struct RoutedWorker {
    worker_id: WorkerId,
    channels: UnixWorkerChannels,
    peers: BTreeMap<(RouteId, PeerId), Entity>,
    pending_sends: BTreeMap<(RouteId, PeerId), VecDeque<SendPayload>>,
    suppress_peer_closes: BTreeSet<(RouteId, PeerId)>,
    closed_peers: VecDeque<(RouteId, PeerId)>,
    closed_set: BTreeSet<(RouteId, PeerId)>,
    send_cursor: usize,
    receive_burst: usize,
    send_burst: usize,
    failed: bool,
    unlink_requested: bool,
    packet_write_closed: bool,
}

impl RoutedWorker {
    /// Construct a routed endpoint with the contract's bounded packet bursts.
    #[must_use]
    pub fn new(worker_id: WorkerId, channels: UnixWorkerChannels) -> Self {
        Self::with_bursts(
            worker_id,
            channels,
            DEFAULT_RECEIVE_BURST,
            DEFAULT_SEND_BURST,
        )
    }

    /// Construct an endpoint with explicit deterministic receive/send bursts.
    #[must_use]
    pub fn with_bursts(
        worker_id: WorkerId,
        channels: UnixWorkerChannels,
        receive_burst: usize,
        send_burst: usize,
    ) -> Self {
        Self {
            worker_id,
            channels,
            peers: BTreeMap::new(),
            pending_sends: BTreeMap::new(),
            suppress_peer_closes: BTreeSet::new(),
            closed_peers: VecDeque::new(),
            closed_set: BTreeSet::new(),
            send_cursor: 0,
            receive_burst: receive_burst.max(1),
            send_burst: send_burst.max(1),
            failed: false,
            unlink_requested: false,
            packet_write_closed: false,
        }
    }

    /// The stable worker identity expected on every BRPK record.
    #[must_use]
    pub const fn worker_id(&self) -> WorkerId {
        self.worker_id
    }

    /// Access the bounded worker channels for test harnesses and lifecycle integration.
    #[must_use]
    pub fn channels(&self) -> &UnixWorkerChannels {
        &self.channels
    }

    /// Mutably access the bounded worker channels for test harnesses and lifecycle integration.
    pub fn channels_mut(&mut self) -> &mut UnixWorkerChannels {
        &mut self.channels
    }

    /// Number of currently mapped routed peers.
    #[must_use]
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Whether this endpoint has observed a terminal worker failure.
    #[must_use]
    pub const fn is_failed(&self) -> bool {
        self.failed
    }

    /// Whether the worker has emitted the packet-stream EOF quiescence barrier.  Once closed,
    /// no later Lightyear payload may be accepted into the worker-to-supervisor stream.
    #[must_use]
    pub const fn packet_write_closed(&self) -> bool {
        self.packet_write_closed
    }

    /// Half-close the worker-to-supervisor packet direction after all adapter-owned payloads
    /// have been flushed.  The control stream remains available for Stop/Exit.
    pub(crate) fn shutdown_packet_write(&mut self) -> Result<(), IpcIoError> {
        self.channels.shutdown_packet_write()?;
        self.packet_write_closed = true;
        Ok(())
    }

    fn remember_closed(&mut self, key: (RouteId, PeerId)) {
        if self.closed_set.insert(key) {
            self.closed_peers.push_back(key);
            if self.closed_peers.len() > MAX_CLOSED_PEERS
                && let Some(evicted) = self.closed_peers.pop_front()
            {
                self.closed_set.remove(&evicted);
            }
        }
    }

    fn remove_peer(&mut self, key: (RouteId, PeerId)) -> Option<Entity> {
        let entity = self.peers.remove(&key);
        // During endpoint teardown, the child link's transformed payloads are detached into the
        // adapter-owned FIFO and must survive the child `Unlinked` observer.  A normal peer close
        // still drops only that peer's queue, because there is no endpoint shutdown drain in that
        // case.
        if !self.unlink_requested {
            self.pending_sends.remove(&key);
        }
        if entity.is_some() {
            self.remember_closed(key);
        }
        entity
    }

    fn suppress_peer_close(&mut self, key: (RouteId, PeerId)) {
        self.suppress_peer_closes.insert(key);
    }

    /// Mark endpoint teardown before Lightyear's deferred child unlink fan-out begins.
    pub(crate) fn request_unlink(&mut self) {
        self.unlink_requested = true;
    }

    fn take_suppressed_peer_close(&mut self, key: (RouteId, PeerId)) -> bool {
        self.suppress_peer_closes.remove(&key)
    }

    fn clear_peers(&mut self) -> Vec<Entity> {
        let peers = self.peers.keys().copied().collect::<Vec<_>>();
        let entities = self.peers.values().copied().collect::<Vec<_>>();
        for key in peers {
            self.remember_closed(key);
        }
        self.peers.clear();
        self.send_cursor = 0;
        entities
    }

    /// Preserve already-Netcode-transformed payloads while the endpoint's child link is being
    /// unlinked.  The child link is about to be despawned, so its `Link::send` queue must be moved
    /// into the adapter-owned FIFO before the normal Lightyear teardown can discard it.
    fn retain_link_sends(&mut self, key: (RouteId, PeerId), link: &mut Link) -> bool {
        let pending = self.pending_sends.entry(key).or_default();
        let mut overflowed = false;
        while let Some(payload) = link.send.pop() {
            if pending.len() >= MAX_PENDING_SEND_PAYLOADS_PER_PEER {
                overflowed = true;
                break;
            }
            pending.push_back(payload);
        }
        overflowed || link.send.len() != 0
    }

    /// Number of transformed payloads retained after endpoint unlink.
    #[must_use]
    pub(crate) fn pending_send_count(&self) -> usize {
        self.pending_sends.values().map(VecDeque::len).sum()
    }

    /// Approximate payload bytes retained after endpoint unlink.  Terminal accounting is only
    /// accepted once this reaches zero; the exact framed byte count is reported by the IPC
    /// writer itself.
    #[must_use]
    pub(crate) fn pending_send_payload_bytes(&self) -> usize {
        self.pending_sends
            .values()
            .flat_map(|payloads| payloads.iter())
            .map(SendPayload::len)
            .sum()
    }

    /// Bytes still owned by the worker's packet/control queues and detached transformed payloads.
    /// The Exit frame itself is intentionally not included because it is the accounting record.
    #[must_use]
    pub(crate) fn terminal_queue_bytes(&self) -> usize {
        self.channels
            .packet_pending_bytes()
            .saturating_add(self.channels.control_pending_bytes())
            .saturating_add(self.pending_send_payload_bytes())
    }

    /// Move detached transformed payloads into the bounded packet writer in stable route order.
    /// `Oversize` means the bounded writer is full and is therefore a retryable backpressure
    /// result here; the payload is restored at the front of its FIFO.
    pub(crate) fn flush_pending_sends(
        &mut self,
        maximum_frames: usize,
    ) -> Result<usize, IpcIoError> {
        let keys = self.pending_sends.keys().copied().collect::<Vec<_>>();
        let mut sent = 0;
        for key in keys {
            while sent < maximum_frames {
                let Some(payload) = self
                    .pending_sends
                    .get_mut(&key)
                    .and_then(VecDeque::pop_front)
                else {
                    break;
                };
                let record = match PacketRecord::new(
                    PacketDirection::WorkerToSupervisor,
                    self.worker_id,
                    key.0,
                    key.1,
                    payload.to_vec(),
                ) {
                    Ok(record) => record,
                    Err(error) => {
                        self.pending_sends
                            .entry(key)
                            .or_default()
                            .push_front(payload);
                        return Err(IpcIoError::Malformed(error));
                    }
                };
                let encoded = match record.encode() {
                    Ok(encoded) => encoded,
                    Err(error) => {
                        self.pending_sends
                            .entry(key)
                            .or_default()
                            .push_front(payload);
                        return Err(IpcIoError::Malformed(error));
                    }
                };
                match self.channels.enqueue_packet(&encoded) {
                    Ok(()) => sent += 1,
                    Err(IpcIoError::Malformed(CodecError::Oversize)) => {
                        self.pending_sends
                            .entry(key)
                            .or_default()
                            .push_front(payload);
                        return Ok(sent);
                    }
                    Err(error) => {
                        self.pending_sends
                            .entry(key)
                            .or_default()
                            .push_front(payload);
                        return Err(error);
                    }
                }
            }
            if sent >= maximum_frames {
                break;
            }
        }
        self.pending_sends
            .retain(|_, payloads| !payloads.is_empty());
        Ok(sent)
    }

    fn mark_failed(&mut self) {
        self.failed = true;
    }
}

/// Installs the routed worker transport at Lightyear's normal IO seams.
pub struct RoutedWorkerPlugin;

impl RoutedWorkerPlugin {
    #[allow(
        clippy::needless_pass_by_value,
        reason = "Bevy observers receive On as an owned system parameter"
    )]
    fn link(
        trigger: On<lightyear::link::LinkStart>,
        query: Query<(), With<RoutedWorker>>,
        mut commands: Commands,
    ) {
        // Unlike UDP, a worker endpoint is already connected to its private supervisor stream.
        // Start still follows Lightyear's normal endpoint lifecycle, so Netcode sees `Linked`
        // only after the ordinary `Start -> LinkStart` path.
        if query.get(trigger.entity).is_ok() {
            commands.entity(trigger.entity).insert(Linked);
        }
    }

    fn receive(
        mut worker_query: Query<(Entity, &mut RoutedWorker), With<Linked>>,
        mut links: Query<&mut Link>,
        mut commands: Commands,
    ) {
        for (worker_entity, mut worker) in &mut worker_query {
            if worker.failed {
                continue;
            }

            let receive_burst = worker.receive_burst;
            let progress = match worker.channels.packet_read_ready(receive_burst) {
                Ok(progress) => progress,
                Err(error) => {
                    worker.mark_failed();
                    commands.trigger(RoutedWorkerFailure {
                        worker: worker_entity,
                        reason: transport_error_reason(&error),
                    });
                    continue;
                }
            };

            for raw_record in progress.records {
                let packet =
                    match PacketRecord::decode(&raw_record, PacketDirection::SupervisorToWorker) {
                        Ok(packet) => packet,
                        Err(error) => {
                            worker.mark_failed();
                            commands.trigger(RoutedWorkerFailure {
                                worker: worker_entity,
                                reason: UnlinkReason::TransportError(format!(
                                    "malformed worker packet: {error}"
                                )),
                            });
                            break;
                        }
                    };

                if packet.worker_id != worker.worker_id {
                    worker.mark_failed();
                    commands.trigger(RoutedWorkerFailure {
                        worker: worker_entity,
                        reason: UnlinkReason::TransportError(
                            "worker identity conflict in packet IPC".to_string(),
                        ),
                    });
                    break;
                }

                let key = (packet.route_id, packet.peer_id);
                if worker.closed_set.contains(&key)
                    || worker.closed_set.iter().any(|(closed_route, closed_peer)| {
                        *closed_route == packet.route_id || *closed_peer == packet.peer_id
                    })
                {
                    // A stale frame from a closed generation is a bounded drop.  Reconnects use
                    // fresh route/peer identities, so this cannot resurrect a child link.
                    continue;
                }

                if let Some(entity) = worker.peers.get(&key).copied() {
                    match links.get_mut(entity) {
                        Ok(mut link) => {
                            link.recv.push(recv_payload(packet.payload), Instant::now());
                        }
                        Err(_) => {
                            worker.remove_peer(key);
                        }
                    }
                    continue;
                }

                // Both dimensions are identity-bound.  A route or peer being reused with a
                // different counterpart is a protocol conflict, not a new connection.
                if worker.peers.keys().any(|(route_id, peer_id)| {
                    *route_id == packet.route_id || *peer_id == packet.peer_id
                }) {
                    worker.mark_failed();
                    commands.trigger(RoutedWorkerFailure {
                        worker: worker_entity,
                        reason: UnlinkReason::TransportError(
                            "route/peer identity conflict in packet IPC".to_string(),
                        ),
                    });
                    break;
                }

                let mut link = Link::default().with_mtu(LinkMtu::new(ROUTED_LINK_MTU));
                link.recv.push(recv_payload(packet.payload), Instant::now());
                let child = commands
                    .spawn((
                        LinkOf {
                            server: worker_entity,
                        },
                        link,
                        RoutedPeer {
                            worker: worker_entity,
                            route_id: packet.route_id,
                            peer_id: packet.peer_id,
                        },
                        Linked,
                    ))
                    .id();
                // Insert before the deferred spawn is applied.  The explicit ApplyDeferred in
                // the plugin makes this child and its triggering payload visible to Netcode in
                // this same frame's connection receive phase.
                worker.peers.insert(key, child);
            }

            if progress.eof {
                worker.mark_failed();
                commands.trigger(RoutedWorkerFailure {
                    worker: worker_entity,
                    reason: UnlinkReason::TransportError(
                        "worker packet IPC reached EOF".to_string(),
                    ),
                });
            }
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the send phase keeps bounded flush, fair peer selection, and failure handling in one explicit schedule boundary"
    )]
    fn send(
        mut worker_query: Query<
            (Entity, &mut RoutedWorker, &lightyear::link::server::Server),
            With<Linked>,
        >,
        mut links: Query<&mut Link>,
        mut commands: Commands,
    ) {
        for (worker_entity, mut worker, _server) in &mut worker_query {
            if worker.failed || worker.packet_write_closed {
                continue;
            }

            // Complete pending partial writes first.  WouldBlock leaves FramedWriter's current
            // frame and offset intact; no payload is lost or reordered.
            let send_burst = worker.send_burst;
            if let Err(error) = worker.channels.flush_packet(send_burst)
                && !matches!(error, IpcIoError::WouldBlock)
            {
                worker.mark_failed();
                commands.trigger(RoutedWorkerFailure {
                    worker: worker_entity,
                    reason: transport_error_reason(&error),
                });
                continue;
            }

            let mut keys = worker.peers.keys().copied().collect::<Vec<_>>();
            if keys.is_empty() {
                continue;
            }

            // NetcodeServerPlugin runs immediately before this transport and transforms every
            // payload in each child link into an encrypted Netcode packet in that same
            // `Link::send` queue.  Drain all of those transformed packets into adapter-owned
            // bounded FIFOs before sending any of them.  Leaving a transformed packet in the
            // child queue would make the next frame's Netcode send path encrypt it again.
            let mut overflowed = None;
            for key in keys.iter().copied() {
                let Some(entity) = worker.peers.get(&key).copied() else {
                    worker.remove_peer(key);
                    continue;
                };
                let Ok(mut link) = links.get_mut(entity) else {
                    worker.remove_peer(key);
                    continue;
                };
                let pending_len = worker.pending_sends.get(&key).map_or(0, VecDeque::len);
                let queued_len = link.send.len();
                if pending_len.saturating_add(queued_len) > MAX_PENDING_SEND_PAYLOADS_PER_PEER {
                    // The transformed payloads must not remain in Link::send, even on the
                    // failure path.  The worker is torn down below, so dropping this bounded
                    // batch is preferable to allowing a second Netcode transformation.
                    link.send.drain().for_each(drop);
                    worker.pending_sends.remove(&key);
                    overflowed = Some(key);
                    break;
                }
                if queued_len > 0 {
                    let pending = worker.pending_sends.entry(key).or_default();
                    pending.extend(link.send.drain());
                }
            }
            if let Some((route_id, peer_id)) = overflowed {
                worker.mark_failed();
                commands.trigger(RoutedWorkerFailure {
                    worker: worker_entity,
                    reason: UnlinkReason::TransportError(format!(
                        "pending routed send queue exceeded {MAX_PENDING_SEND_PAYLOADS_PER_PEER} payloads for route {route_id:?}, peer {peer_id:?}"
                    )),
                });
                continue;
            }

            // BTreeMap order is stable across frames; the cursor gives each route one turn before
            // any route receives a second turn.
            let start = worker.send_cursor % keys.len();
            keys.rotate_left(start);
            let mut sent = 0_usize;
            let mut blocked = false;
            for key in keys.iter().copied() {
                if sent >= worker.send_burst {
                    break;
                }
                let Some(payload) = worker
                    .pending_sends
                    .get_mut(&key)
                    .and_then(VecDeque::pop_front)
                else {
                    continue;
                };
                let record = match PacketRecord::new(
                    PacketDirection::WorkerToSupervisor,
                    worker.worker_id,
                    key.0,
                    key.1,
                    payload.to_vec(),
                ) {
                    Ok(record) => record,
                    Err(error) => {
                        worker
                            .pending_sends
                            .entry(key)
                            .or_default()
                            .push_front(payload);
                        worker.mark_failed();
                        commands.trigger(RoutedWorkerFailure {
                            worker: worker_entity,
                            reason: UnlinkReason::TransportError(format!(
                                "worker payload violates routed MTU: {error}"
                            )),
                        });
                        break;
                    }
                };
                let encoded = match record.encode() {
                    Ok(encoded) => encoded,
                    Err(error) => {
                        worker
                            .pending_sends
                            .entry(key)
                            .or_default()
                            .push_front(payload);
                        worker.mark_failed();
                        commands.trigger(RoutedWorkerFailure {
                            worker: worker_entity,
                            reason: UnlinkReason::TransportError(format!(
                                "worker packet encode failed: {error}"
                            )),
                        });
                        break;
                    }
                };
                match worker.channels.enqueue_packet(&encoded) {
                    Ok(()) => sent += 1,
                    Err(IpcIoError::Malformed(CodecError::Oversize)) => {
                        // The packet itself is bounded; Oversize here means the bounded writer
                        // queue is full.  Keep the already-transformed payload in the adapter
                        // FIFO and retry next frame.  It never returns to Link::send.
                        worker
                            .pending_sends
                            .entry(key)
                            .or_default()
                            .push_front(payload);
                        blocked = true;
                        break;
                    }
                    Err(error) => {
                        worker
                            .pending_sends
                            .entry(key)
                            .or_default()
                            .push_front(payload);
                        worker.mark_failed();
                        commands.trigger(RoutedWorkerFailure {
                            worker: worker_entity,
                            reason: transport_error_reason(&error),
                        });
                        break;
                    }
                }
            }
            if !blocked && sent > 0 && !keys.is_empty() {
                worker.send_cursor = (start + sent) % keys.len();
            } else if blocked && !keys.is_empty() {
                worker.send_cursor = start;
            }

            if worker.failed {
                continue;
            }
            let send_burst = worker.send_burst;
            if let Err(error) = worker.channels.flush_packet(send_burst)
                && !matches!(error, IpcIoError::WouldBlock)
            {
                worker.mark_failed();
                commands.trigger(RoutedWorkerFailure {
                    worker: worker_entity,
                    reason: transport_error_reason(&error),
                });
            }
        }
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "Bevy observers receive On as an owned system parameter"
    )]
    fn failure(
        trigger: On<RoutedWorkerFailure>,
        mut worker_query: Query<&mut RoutedWorker>,
        mut commands: Commands,
    ) {
        error!(worker = ?trigger.worker, reason = ?trigger.reason, "brawler routed worker transport failure");
        let Ok(mut worker) = worker_query.get_mut(trigger.worker) else {
            return;
        };
        worker.mark_failed();
        if !worker.unlink_requested {
            worker.unlink_requested = true;
            commands.trigger(Unlink {
                entity: trigger.worker,
                reason: trigger.reason.clone(),
            });
        }
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "Bevy observers receive On as an owned system parameter"
    )]
    fn peer_close(
        trigger: On<RoutedPeerClose>,
        mut worker_query: Query<&mut RoutedWorker>,
        mut commands: Commands,
    ) {
        let Ok(mut worker) = worker_query.get_mut(trigger.worker) else {
            return;
        };
        let key = (trigger.route_id, trigger.peer_id);
        worker.suppress_peer_close(key);
        let Some(entity) = worker.remove_peer(key) else {
            return;
        };
        commands.trigger(Unlink {
            entity,
            reason: trigger.reason.clone(),
        });
        // A peer close is transport-owned teardown of this child link.  The server endpoint is
        // still linked, so ServerLinkPlugin's server-level Unlinked observer cannot own this
        // per-peer despawn; queue it alongside the Unlink transition for the same deferred pass.
        commands.entity(entity).try_despawn();
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "Bevy observers receive On as an owned system parameter"
    )]
    fn peer_unlinked(
        trigger: On<Add, lightyear::link::Unlinked>,
        peers: Query<&RoutedPeer>,
        mut workers: Query<&mut RoutedWorker>,
    ) {
        let Ok(peer) = peers.get(trigger.entity) else {
            return;
        };
        if let Ok(mut worker) = workers.get_mut(peer.worker) {
            worker.remove_peer((peer.route_id, peer.peer_id));
        }
        // ServerLinkPlugin owns the deferred child despawn. Keeping only mapping removal here
        // avoids enqueueing a second despawn command against the same entity generation.
    }

    /// Report a locally removed peer exactly once. Supervisor-requested closes are marked before
    /// unlink and intentionally do not echo back; endpoint teardown suppresses all close records
    /// because the worker itself is already failing/stopping.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "Bevy observers receive On as an owned system parameter"
    )]
    fn peer_removed(
        trigger: On<Remove, RoutedPeer>,
        peers: Query<&RoutedPeer>,
        mut workers: Query<&mut RoutedWorker>,
        mut state: Option<ResMut<super::worker::WorkerControlState>>,
        mut commands: Commands,
    ) {
        let Ok(peer) = peers.get(trigger.entity) else {
            return;
        };
        let key = (peer.route_id, peer.peer_id);
        let Ok(mut worker) = workers.get_mut(peer.worker) else {
            return;
        };
        if worker.take_suppressed_peer_close(key) {
            return;
        }
        let Some(state) = state.as_mut() else {
            return;
        };
        if super::worker::queue_peer_close(&mut worker, state, peer.route_id, peer.peer_id, 0)
            .is_err()
        {
            worker.mark_failed();
            commands.trigger(RoutedWorkerFailure {
                worker: peer.worker,
                reason: UnlinkReason::TransportError(
                    "worker peer-close control queue failed".to_string(),
                ),
            });
        }
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "Bevy observers receive On as an owned system parameter"
    )]
    fn worker_unlinked(
        trigger: On<Add, lightyear::link::Unlinked>,
        mut workers: Query<&mut RoutedWorker>,
        mut links: Query<&mut Link>,
        mut commands: Commands,
    ) {
        let Ok(mut worker) = workers.get_mut(trigger.entity) else {
            return;
        };
        // ServerLinkPlugin owns the endpoint -> child Unlink fan-out.  Clearing our mapping here
        // makes that fan-out idempotent and leaves the child Add<Unlinked> observer with one
        // removal opportunity per peer.
        worker.unlink_requested = true;
        let peers = worker
            .peers
            .iter()
            .map(|(key, entity)| (*key, *entity))
            .collect::<Vec<_>>();
        let mut overflowed = false;
        if !worker.failed {
            for (key, entity) in peers {
                if let Ok(mut link) = links.get_mut(entity) {
                    overflowed |= worker.retain_link_sends(key, &mut link);
                }
            }
        }
        for key in worker.peers.keys().copied().collect::<Vec<_>>() {
            worker.suppress_peer_close(key);
        }
        worker.clear_peers();
        if overflowed {
            worker.mark_failed();
            commands.trigger(RoutedWorkerFailure {
                worker: trigger.entity,
                reason: UnlinkReason::TransportError(
                    "pending routed sends exceeded shutdown bound".to_string(),
                ),
            });
        }
    }
}

fn transport_error_reason(error: &IpcIoError) -> UnlinkReason {
    UnlinkReason::TransportError(format!("worker packet IPC failed: {error}"))
}

fn recv_payload(payload: Vec<u8>) -> RecvPayload {
    recv_payload_from_bytes(SendPayload::from(payload))
}

impl Plugin for RoutedWorkerPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<lightyear::link::LinkPlugin>() {
            app.add_plugins(lightyear::link::LinkPlugin);
        }
        if !app.is_plugin_added::<lightyear::link::server::ServerLinkPlugin>() {
            app.add_plugins(lightyear::link::server::ServerLinkPlugin);
        }
        app.add_observer(Self::link);
        app.add_observer(Self::failure);
        app.add_observer(Self::peer_close);
        app.add_observer(Self::peer_unlinked);
        app.add_observer(Self::peer_removed);
        app.add_observer(Self::worker_unlinked);
        // `ApplyDeferred` is deliberately part of the BufferToLink chain.  A first packet's
        // LinkOf child and triggering payload therefore exist before ApplyConditioner and the
        // Lightyear connection receive systems in the same PreUpdate.
        app.add_systems(
            PreUpdate,
            (Self::receive, ApplyDeferred)
                .chain()
                .in_set(LinkReceiveSystems::BufferToLink),
        );
        app.add_systems(PostUpdate, Self::send.in_set(LinkSystems::Send));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brawler_routing::WORKER_PACKET_QUEUE_FRAMES;
    use std::{
        io::{ErrorKind, Write},
        os::unix::net::UnixStream,
    };

    fn app_with_worker() -> (App, UnixWorkerChannels, Entity) {
        app_with_worker_bursts(DEFAULT_RECEIVE_BURST, DEFAULT_SEND_BURST)
    }

    fn app_with_worker_bursts(
        receive_burst: usize,
        send_burst: usize,
    ) -> (App, UnixWorkerChannels, Entity) {
        let (worker_channels, supervisor_channels) = channels();
        app_with_channels(
            worker_channels,
            supervisor_channels,
            receive_burst,
            send_burst,
        )
    }

    fn app_with_channels(
        worker_channels: UnixWorkerChannels,
        supervisor_channels: UnixWorkerChannels,
        receive_burst: usize,
        send_burst: usize,
    ) -> (App, UnixWorkerChannels, Entity) {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            lightyear::link::server::ServerLinkPlugin,
            RoutedWorkerPlugin,
        ));
        let endpoint = app
            .world_mut()
            .spawn((
                lightyear::link::server::Server::default(),
                RoutedWorker::with_bursts(
                    WorkerId::new(7).unwrap(),
                    worker_channels,
                    receive_burst,
                    send_burst,
                ),
                Linked,
            ))
            .id();
        (app, supervisor_channels, endpoint)
    }

    fn channels() -> (UnixWorkerChannels, UnixWorkerChannels) {
        let (packet_a, packet_b) = UnixStream::pair().unwrap();
        let (control_a, control_b) = UnixStream::pair().unwrap();
        packet_a.set_nonblocking(true).unwrap();
        packet_b.set_nonblocking(true).unwrap();
        control_a.set_nonblocking(true).unwrap();
        control_b.set_nonblocking(true).unwrap();
        (
            UnixWorkerChannels::from_std(packet_a, control_a),
            UnixWorkerChannels::from_std(packet_b, control_b),
        )
    }

    fn channels_with_blocked_worker_writer() -> (UnixWorkerChannels, UnixWorkerChannels) {
        let (packet_worker, packet_supervisor) = UnixStream::pair().unwrap();
        let (control_worker, control_supervisor) = UnixStream::pair().unwrap();
        packet_worker.set_nonblocking(true).unwrap();
        packet_supervisor.set_nonblocking(true).unwrap();
        control_worker.set_nonblocking(true).unwrap();
        control_supervisor.set_nonblocking(true).unwrap();

        // Fill the worker-to-supervisor kernel buffer before the worker adapter starts.  The
        // packet writer then reports WouldBlock without retiring its current framed records,
        // allowing the adapter-owned FIFO retention path to be tested deterministically.
        let mut filler = packet_worker.try_clone().unwrap();
        let bytes = [0xa5_u8; 4096];
        loop {
            match filler.write(&bytes) {
                Ok(_) => {}
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) => panic!("fill worker packet socket: {error}"),
            }
        }

        (
            UnixWorkerChannels::from_std(packet_worker, control_worker),
            UnixWorkerChannels::from_std(packet_supervisor, control_supervisor),
        )
    }

    fn packet(direction: PacketDirection, route: u128, peer: u128, payload: &[u8]) -> Vec<u8> {
        PacketRecord::new(
            direction,
            WorkerId::new(7).unwrap(),
            RouteId::new(route).unwrap(),
            PeerId::new(peer).unwrap(),
            payload.to_vec(),
        )
        .unwrap()
        .encode_framed()
        .unwrap()
    }

    fn send_packet(supervisor: &mut UnixWorkerChannels, route: u128, peer: u128, payload: &[u8]) {
        let record = PacketRecord::new(
            PacketDirection::SupervisorToWorker,
            WorkerId::new(7).unwrap(),
            RouteId::new(route).unwrap(),
            PeerId::new(peer).unwrap(),
            payload.to_vec(),
        )
        .unwrap();
        supervisor
            .enqueue_packet(&record.encode().unwrap())
            .unwrap();
        supervisor.flush_packet(64).unwrap();
    }

    fn routed_peers(app: &mut App) -> Vec<(Entity, RoutedPeer)> {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &RoutedPeer)>();
        query
            .iter(world)
            .map(|(entity, peer)| (entity, *peer))
            .collect()
    }

    #[test]
    fn endpoint_constructor_keeps_contract_mtu_and_bounds() {
        let (worker_channels, _) = channels();
        let worker = RoutedWorker::with_bursts(WorkerId::new(7).unwrap(), worker_channels, 0, 0);
        assert_eq!(worker.receive_burst, 1);
        assert_eq!(worker.send_burst, 1);
        assert_eq!(ROUTED_LINK_MTU, 1_133);
        assert_eq!(worker.peer_count(), 0);
        assert!(!worker.is_failed());
    }

    #[test]
    fn unix_channels_preserve_two_framed_packets_fifo() {
        let (mut worker, mut supervisor) = channels();
        let first = packet(PacketDirection::SupervisorToWorker, 10, 20, b"one");
        let second = packet(PacketDirection::SupervisorToWorker, 11, 21, b"two");
        // `UnixWorkerChannels` writes through its framed writer, so enqueueing on the supervisor
        // side and flushing exercises the exact BRPK bytes consumed by the worker.
        supervisor.enqueue_packet(&first[4..]).unwrap();
        supervisor.enqueue_packet(&second[4..]).unwrap();
        supervisor.flush_packet(8).unwrap();
        let progress = worker.packet_read_ready(8).unwrap();
        assert_eq!(progress.records.len(), 2);
        assert_eq!(progress.records[0], first[4..]);
        assert_eq!(progress.records[1], second[4..]);
    }

    #[test]
    fn first_packet_creates_link_and_is_visible_before_receive_finishes() {
        let (mut app, mut supervisor, _) = app_with_worker();
        send_packet(&mut supervisor, 10, 20, b"first");

        app.update();

        let peers = routed_peers(&mut app);
        assert_eq!(peers.len(), 1);
        let link = app.world().get::<Link>(peers[0].0).unwrap();
        assert_eq!(link.mtu(), ROUTED_LINK_MTU);
        assert_eq!(link.recv.len(), 1);
        assert_eq!(peers[0].1.route_id, RouteId::new(10).unwrap());
        assert_eq!(peers[0].1.peer_id, PeerId::new(20).unwrap());
    }

    #[test]
    fn two_peers_are_mapped_independently_and_fifo_send_is_fair() {
        let (mut app, mut supervisor, _) = app_with_worker();
        send_packet(&mut supervisor, 10, 20, b"connect-a");
        send_packet(&mut supervisor, 11, 21, b"connect-b");
        app.update();
        let peers = routed_peers(&mut app);
        assert_eq!(peers.len(), 2);

        let mut ordered = peers;
        ordered.sort_by_key(|(_, peer)| (peer.route_id, peer.peer_id));
        for (entity, _) in &ordered {
            let mut link = app.world_mut().get_mut::<Link>(*entity).unwrap();
            link.send
                .push(SendPayload::from(if *entity == ordered[0].0 {
                    b"a1".to_vec()
                } else {
                    b"b1".to_vec()
                }));
            link.send
                .push(SendPayload::from(if *entity == ordered[0].0 {
                    b"a2".to_vec()
                } else {
                    b"b2".to_vec()
                }));
        }
        app.update();
        let first = supervisor.packet_read_ready(8).unwrap();
        assert_eq!(first.records.len(), 2);
        let first = first
            .records
            .iter()
            .map(|record| {
                PacketRecord::decode(record, PacketDirection::WorkerToSupervisor).unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(first[0].route_id, RouteId::new(10).unwrap());
        assert_eq!(first[0].payload, b"a1");
        assert_eq!(first[1].route_id, RouteId::new(11).unwrap());
        assert_eq!(first[1].payload, b"b1");

        app.update();
        let second = supervisor.packet_read_ready(8).unwrap();
        assert_eq!(second.records.len(), 2);
        let second = second
            .records
            .iter()
            .map(|record| {
                PacketRecord::decode(record, PacketDirection::WorkerToSupervisor).unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(second[0].payload, b"a2");
        assert_eq!(second[1].payload, b"b2");
    }

    #[test]
    fn adapter_drains_all_transformed_payloads_before_bounded_fair_flush() {
        let (mut app, mut supervisor, endpoint) = app_with_worker_bursts(64, 1);
        send_packet(&mut supervisor, 10, 20, b"connect");
        app.update();
        let peer = routed_peers(&mut app)[0].0;
        {
            let mut link = app.world_mut().get_mut::<Link>(peer).unwrap();
            link.send.push(SendPayload::from(vec![0x85, 1]));
            link.send.push(SendPayload::from(vec![0x85, 2]));
        }

        // One packet is allowed onto IPC this frame, but both packets must leave Link::send.
        // This is the invariant that prevents the next frame's Netcode send phase from wrapping
        // the second packet a second time.
        app.update();
        let first = supervisor.packet_read_ready(8).unwrap();
        assert_eq!(first.records.len(), 1);
        let first =
            PacketRecord::decode(&first.records[0], PacketDirection::WorkerToSupervisor).unwrap();
        assert_eq!(first.payload, [0x85, 1]);
        assert_eq!(app.world().get::<Link>(peer).unwrap().send.len(), 0);
        assert_eq!(
            app.world()
                .get::<RoutedWorker>(endpoint)
                .unwrap()
                .pending_sends
                .get(&(RouteId::new(10).unwrap(), PeerId::new(20).unwrap()))
                .map_or(0, VecDeque::len),
            1
        );

        app.update();
        let second = supervisor.packet_read_ready(8).unwrap();
        assert_eq!(second.records.len(), 1);
        let second =
            PacketRecord::decode(&second.records[0], PacketDirection::WorkerToSupervisor).unwrap();
        assert_eq!(second.payload, [0x85, 2]);
    }

    #[test]
    fn endpoint_detach_retains_and_drains_transformed_payloads() {
        let (worker_channels, mut supervisor) = channels();
        let mut worker = RoutedWorker::with_bursts(
            WorkerId::new(7).unwrap(),
            worker_channels,
            DEFAULT_RECEIVE_BURST,
            DEFAULT_SEND_BURST,
        );
        let key = (RouteId::new(10).unwrap(), PeerId::new(20).unwrap());
        let mut link = Link::default();
        link.send.push(SendPayload::from(vec![0x85, 1]));
        link.send.push(SendPayload::from(vec![0x85, 2]));

        assert!(!worker.retain_link_sends(key, &mut link));
        assert_eq!(link.send.len(), 0);
        assert_eq!(worker.pending_send_count(), 2);
        assert!(worker.terminal_queue_bytes() >= 4);

        assert_eq!(worker.flush_pending_sends(1).unwrap(), 1);
        worker.channels_mut().flush_packet(8).unwrap();
        let first = supervisor.packet_read_ready(8).unwrap();
        let first =
            PacketRecord::decode(&first.records[0], PacketDirection::WorkerToSupervisor).unwrap();
        assert_eq!(first.payload, [0x85, 1]);
        assert_eq!(worker.pending_send_count(), 1);

        assert_eq!(worker.flush_pending_sends(8).unwrap(), 1);
        worker.channels_mut().flush_packet(8).unwrap();
        let second = supervisor.packet_read_ready(8).unwrap();
        let second =
            PacketRecord::decode(&second.records[0], PacketDirection::WorkerToSupervisor).unwrap();
        assert_eq!(second.payload, [0x85, 2]);
        assert_eq!(worker.pending_send_count(), 0);
        assert_eq!(worker.terminal_queue_bytes(), 0);
    }

    #[test]
    fn pending_send_overflow_fails_worker_without_requeueing_into_link() {
        let (mut app, mut supervisor, endpoint) = app_with_worker();
        send_packet(&mut supervisor, 10, 20, b"connect");
        app.update();
        let peer = routed_peers(&mut app)[0].0;
        let key = (RouteId::new(10).unwrap(), PeerId::new(20).unwrap());
        {
            let mut worker = app.world_mut().get_mut::<RoutedWorker>(endpoint).unwrap();
            worker.pending_sends.insert(
                key,
                (0..MAX_PENDING_SEND_PAYLOADS_PER_PEER)
                    .map(|value| SendPayload::from(vec![u8::try_from(value).unwrap()]))
                    .collect(),
            );
        }
        app.world_mut()
            .get_mut::<Link>(peer)
            .unwrap()
            .send
            .push(SendPayload::from(vec![0x85, 0xff]));

        app.update();
        let worker = app.world().get::<RoutedWorker>(endpoint).unwrap();
        assert!(worker.is_failed());
        assert_eq!(worker.peer_count(), 0);
        assert!(worker.pending_sends.is_empty());
        assert!(app.world().get_entity(peer).is_err());
    }

    #[test]
    fn ipc_would_block_retains_transformed_payload_in_adapter_fifo() {
        let (worker_channels, supervisor_channels) = channels_with_blocked_worker_writer();
        let (mut app, mut supervisor, endpoint) =
            app_with_channels(worker_channels, supervisor_channels, 64, 1);
        send_packet(&mut supervisor, 10, 20, b"connect");
        app.update();
        let peer = routed_peers(&mut app)[0].0;
        let key = (RouteId::new(10).unwrap(), PeerId::new(20).unwrap());
        let full_record = PacketRecord::new(
            PacketDirection::WorkerToSupervisor,
            WorkerId::new(7).unwrap(),
            key.0,
            key.1,
            vec![0xa5],
        )
        .unwrap()
        .encode()
        .unwrap();
        {
            let mut worker = app.world_mut().get_mut::<RoutedWorker>(endpoint).unwrap();
            for _ in 0..WORKER_PACKET_QUEUE_FRAMES {
                worker.channels_mut().enqueue_packet(&full_record).unwrap();
            }
        }
        app.world_mut()
            .get_mut::<Link>(peer)
            .unwrap()
            .send
            .push(SendPayload::from(vec![0x85, 0x01]));
        app.world_mut()
            .get_mut::<Link>(peer)
            .unwrap()
            .send
            .push(SendPayload::from(vec![0x85, 0x02]));

        app.update();
        let worker = app.world().get::<RoutedWorker>(endpoint).unwrap();
        assert!(!worker.is_failed());
        assert_eq!(app.world().get::<Link>(peer).unwrap().send.len(), 0);
        assert_eq!(worker.pending_sends.get(&key).map_or(0, VecDeque::len), 2);
    }

    #[test]
    fn stale_peer_close_does_not_resurrect_and_identity_conflict_fails_worker() {
        let (mut app, mut supervisor, endpoint) = app_with_worker();
        send_packet(&mut supervisor, 10, 20, b"first");
        app.update();
        let peer = routed_peers(&mut app)[0].0;
        app.world_mut()
            .get_mut::<RoutedWorker>(endpoint)
            .unwrap()
            .pending_sends
            .insert(
                (RouteId::new(10).unwrap(), PeerId::new(20).unwrap()),
                VecDeque::from([SendPayload::from(b"pending".to_vec())]),
            );
        app.world_mut().trigger(RoutedPeerClose {
            worker: endpoint,
            route_id: RouteId::new(10).unwrap(),
            peer_id: PeerId::new(20).unwrap(),
            reason: UnlinkReason::ByPeer("closed".to_string()),
        });
        app.update();
        assert!(app.world().get_entity(peer).is_err());
        assert_eq!(routed_peers(&mut app).len(), 0);
        assert!(
            app.world()
                .get::<RoutedWorker>(endpoint)
                .unwrap()
                .pending_sends
                .is_empty()
        );

        send_packet(&mut supervisor, 10, 20, b"stale");
        app.update();
        assert_eq!(routed_peers(&mut app).len(), 0);
        assert!(
            !app.world()
                .get::<RoutedWorker>(endpoint)
                .unwrap()
                .is_failed()
        );

        send_packet(&mut supervisor, 10, 21, b"conflict");
        app.update();
        // The original route is tombstoned, so this is a stale-generation drop rather than a
        // route conflict.  A live route conflict is checked separately below.
        assert!(
            !app.world()
                .get::<RoutedWorker>(endpoint)
                .unwrap()
                .is_failed()
        );

        send_packet(&mut supervisor, 30, 40, b"live");
        send_packet(&mut supervisor, 30, 41, b"identity-conflict");
        app.update();
        assert!(
            app.world()
                .get::<RoutedWorker>(endpoint)
                .unwrap()
                .is_failed()
        );
        assert_eq!(routed_peers(&mut app).len(), 0);
    }

    #[test]
    fn packet_eof_marks_worker_failed_and_unlinks_all_peers_once() {
        let (mut app, mut supervisor, endpoint) = app_with_worker();
        send_packet(&mut supervisor, 10, 20, b"first");
        send_packet(&mut supervisor, 11, 21, b"second");
        app.update();
        assert_eq!(routed_peers(&mut app).len(), 2);

        drop(supervisor);
        app.update();
        assert!(
            app.world()
                .get::<RoutedWorker>(endpoint)
                .unwrap()
                .is_failed()
        );
        assert_eq!(
            app.world()
                .get::<RoutedWorker>(endpoint)
                .unwrap()
                .peer_count(),
            0
        );
        // A second schedule pass must not re-trigger peer teardown or resurrect mappings.
        app.update();
        assert_eq!(routed_peers(&mut app).len(), 0);
        assert!(
            app.world()
                .get::<RoutedWorker>(endpoint)
                .unwrap()
                .is_failed()
        );
    }

    #[test]
    fn worker_packet_queue_reports_bounded_backpressure_without_reordering() {
        let (worker_channels, _) = channels();
        let mut worker = RoutedWorker::new(WorkerId::new(7).unwrap(), worker_channels);
        let record = PacketRecord::new(
            PacketDirection::WorkerToSupervisor,
            WorkerId::new(7).unwrap(),
            RouteId::new(10).unwrap(),
            PeerId::new(20).unwrap(),
            b"payload".to_vec(),
        )
        .unwrap()
        .encode()
        .unwrap();
        for _ in 0..512 {
            worker.channels_mut().enqueue_packet(&record).unwrap();
        }
        assert!(matches!(
            worker.channels_mut().enqueue_packet(&record),
            Err(IpcIoError::Malformed(CodecError::Oversize))
        ));
        assert!(worker.channels().packet_pending());
    }
}
