mod capabilities;
mod process;
mod queues;

use std::{
    collections::{BTreeMap, HashMap},
    net::SocketAddr,
};

use crate::{
    AllocationId, Capability, Generation, LobbyAuthenticatedBody, LobbyNetcodeAuthenticatedBody,
    LobbySessionId, LogicalServerId, MAX_ACTIVE_ROUTES, MAX_CAPABILITIES,
    MAX_CONSECUTIVE_ROUTE_PACKETS, MAX_WORKERS, MatchId, PUBLIC_LOBBY_ROUTE_IDLE_MILLIS,
    PacketDirection, PacketRecord, PeerId, ProcessId, PublicEnvelope, RouteId, RouteSelector,
    RoutingErrorCategory, WORKER_CONTROL_QUEUE_BYTES, WORKER_CONTROL_QUEUE_FRAMES,
    WORKER_PACKET_QUEUE_BYTES, WORKER_PACKET_QUEUE_FRAMES, WorkerId,
};

use self::{
    capabilities::CapabilityRegistry,
    queues::{ControlQueues, PacketQueues},
};

pub use process::{
    DescriptorPolicy, LifecycleError, LifecycleEvent, LifecyclePhase, ProcessStatus,
    ProcessSupervisor, ProcessSupervisorConfig, ShutdownReport, StderrPolicy, WorkerLaunchSpec,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct MonotonicMillis(pub u64);

impl MonotonicMillis {
    #[must_use]
    pub const fn saturating_add(self, millis: u64) -> Self {
        Self(self.0.saturating_add(millis))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerKind {
    Lobby,
    Match,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkerRegistration {
    pub worker_id: WorkerId,
    pub process_id: ProcessId,
    pub generation: Generation,
    pub kind: WorkerKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteRegistration {
    pub route_id: RouteId,
    pub worker_id: WorkerId,
    pub peer_id: PeerId,
    pub is_default_lobby: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapabilityBinding {
    pub logical_server_id: LogicalServerId,
    pub supervisor_generation: Generation,
    pub worker_id: WorkerId,
    pub worker_generation: Generation,
    pub route_id: RouteId,
    pub peer_id: PeerId,
    pub lobby_session_id: LobbySessionId,
    pub allocation_id: AllocationId,
    pub match_id: MatchId,
    pub network_protocol: u64,
    pub content_fingerprint: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityStatus {
    Pending,
    Active,
    Revoked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Authorization {
    pub binding: CapabilityBinding,
    pub activated: bool,
    pub rebound: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QueueHighWater {
    pub frames: usize,
    pub bytes: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CoreMetrics {
    pub packet_current: QueueHighWater,
    pub packet_high_water: QueueHighWater,
    pub control_current: QueueHighWater,
    pub control_high_water: QueueHighWater,
    pub packet_dropped_newest: u64,
    pub control_rejected: u64,
    pub capabilities_activated: u64,
    pub capability_rebinds: u64,
    pub capabilities_revoked: u64,
    pub workers_cleaned: u64,
    pub routes_cleaned: u64,
    pub source_limited: u64,
    pub error_counts: BTreeMap<RoutingErrorCategory, u64>,
}

impl CoreMetrics {
    fn count_error(&mut self, category: RoutingErrorCategory) {
        *self.error_counts.entry(category).or_default() += 1;
        if category == RoutingErrorCategory::SourceLimited {
            self.source_limited += 1;
        }
    }

    fn observe_packet_queue(&mut self, frames: usize, bytes: usize) {
        self.packet_current = QueueHighWater { frames, bytes };
        self.packet_high_water.frames = self.packet_high_water.frames.max(frames);
        self.packet_high_water.bytes = self.packet_high_water.bytes.max(bytes);
    }

    fn observe_control_queue(&mut self, frames: usize, bytes: usize) {
        self.control_current = QueueHighWater { frames, bytes };
        self.control_high_water.frames = self.control_high_water.frames.max(frames);
        self.control_high_water.bytes = self.control_high_water.bytes.max(bytes);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoreConfig {
    pub logical_server_id: Option<LogicalServerId>,
    pub supervisor_generation: Option<Generation>,
    pub network_protocol: Option<u64>,
    pub content_fingerprint: Option<u64>,
    pub max_capabilities_per_lobby_session: usize,
    pub max_workers: usize,
    pub max_routes: usize,
    pub max_capabilities: usize,
    pub route_packet_frames: usize,
    pub route_packet_bytes: usize,
    pub worker_packet_frames: usize,
    pub worker_packet_bytes: usize,
    pub worker_control_frames: usize,
    pub worker_control_bytes: usize,
    pub max_consecutive_route_packets: usize,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            logical_server_id: None,
            supervisor_generation: None,
            network_protocol: None,
            content_fingerprint: None,
            max_capabilities_per_lobby_session: 2,
            max_workers: MAX_WORKERS,
            max_routes: MAX_ACTIVE_ROUTES,
            max_capabilities: MAX_CAPABILITIES,
            route_packet_frames: crate::ROUTE_PACKET_QUEUE_FRAMES,
            route_packet_bytes: crate::ROUTE_PACKET_QUEUE_BYTES,
            worker_packet_frames: WORKER_PACKET_QUEUE_FRAMES,
            worker_packet_bytes: WORKER_PACKET_QUEUE_BYTES,
            worker_control_frames: WORKER_CONTROL_QUEUE_FRAMES,
            worker_control_bytes: WORKER_CONTROL_QUEUE_BYTES,
            max_consecutive_route_packets: MAX_CONSECUTIVE_ROUTE_PACKETS,
        }
    }
}

impl CoreConfig {
    #[must_use]
    pub fn with_identity(
        logical_server_id: LogicalServerId,
        supervisor_generation: Generation,
        network_protocol: u64,
        content_fingerprint: u64,
    ) -> Self {
        Self {
            logical_server_id: Some(logical_server_id),
            supervisor_generation: Some(supervisor_generation),
            network_protocol: Some(network_protocol),
            content_fingerprint: Some(content_fingerprint),
            ..Self::default()
        }
    }

    #[must_use]
    pub const fn has_identity(&self) -> bool {
        self.logical_server_id.is_some()
            && self.supervisor_generation.is_some()
            && self.network_protocol.is_some()
            && self.content_fingerprint.is_some()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CleanupReport {
    pub routes_removed: usize,
    pub capabilities_revoked: usize,
    pub packet_frames_removed: usize,
    pub control_frames_removed: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteTeardown {
    pub route_id: RouteId,
    pub worker_id: WorkerId,
    pub peer_id: PeerId,
    pub reason: RoutingErrorCategory,
}

#[derive(Clone, Debug)]
struct WorkerState {
    registration: WorkerRegistration,
}

#[derive(Clone)]
pub struct SupervisorCore {
    config: CoreConfig,
    workers: HashMap<WorkerId, WorkerState>,
    routes: HashMap<RouteId, RouteRegistration>,
    default_lobby_route: Option<RouteId>,
    capabilities: CapabilityRegistry,
    packets: PacketQueues,
    controls: ControlQueues,
    route_sources: HashMap<RouteId, SocketAddr>,
    lobby_sources: HashMap<SocketAddr, RouteRegistration>,
    lobby_route_last_seen: HashMap<RouteId, MonotonicMillis>,
    metrics: CoreMetrics,
}

impl SupervisorCore {
    #[must_use]
    pub fn new(config: CoreConfig) -> Self {
        Self {
            packets: PacketQueues::new(config),
            controls: ControlQueues::new(config),
            config,
            workers: HashMap::new(),
            routes: HashMap::new(),
            default_lobby_route: None,
            capabilities: CapabilityRegistry::default(),
            route_sources: HashMap::new(),
            lobby_sources: HashMap::new(),
            lobby_route_last_seen: HashMap::new(),
            metrics: CoreMetrics::default(),
        }
    }

    pub fn register_worker(
        &mut self,
        registration: WorkerRegistration,
    ) -> Result<(), RoutingErrorCategory> {
        if self.workers.contains_key(&registration.worker_id) {
            return Err(RoutingErrorCategory::WorkerProtocolConflict);
        }
        if registration.kind == WorkerKind::Lobby
            && self
                .workers
                .values()
                .any(|worker| worker.registration.kind == WorkerKind::Lobby)
        {
            return Err(RoutingErrorCategory::WorkerProtocolConflict);
        }
        if self.workers.len() >= self.config.max_workers {
            return Err(RoutingErrorCategory::AllocationCapacity);
        }
        self.packets
            .add_worker(registration.worker_id, registration.kind);
        self.controls.add_worker(registration.worker_id);
        self.workers
            .insert(registration.worker_id, WorkerState { registration });
        Ok(())
    }

    pub fn register_route(&mut self, route: RouteRegistration) -> Result<(), RoutingErrorCategory> {
        let Some(worker) = self.workers.get(&route.worker_id) else {
            return Err(RoutingErrorCategory::ManifestIdentity);
        };
        if self.routes.contains_key(&route.route_id) {
            return Err(RoutingErrorCategory::WorkerProtocolConflict);
        }
        if self.routes.len() >= self.config.max_routes {
            return Err(RoutingErrorCategory::AllocationCapacity);
        }
        if route.is_default_lobby {
            if worker.registration.kind != WorkerKind::Lobby || self.default_lobby_route.is_some() {
                return Err(RoutingErrorCategory::WorkerProtocolConflict);
            }
            self.default_lobby_route = Some(route.route_id);
        }
        self.packets.add_route(route);
        self.routes.insert(route.route_id, route);
        Ok(())
    }

    /// Return whether the worker-owned default lobby route has been published.
    ///
    /// Public lobby datagrams can arrive while the worker process is still starting.  They are
    /// intentionally dropped until the worker has sent `Ready`; importantly, they must not
    /// consume a source's pre-auth budget because no route or worker work exists yet.  Keeping
    /// this check at the routing owner boundary makes the readiness race explicit without
    /// changing the 8-datagram/9-KiB limit for admitted public traffic.
    #[must_use]
    pub fn default_lobby_ready(&self) -> bool {
        self.default_lobby_route.is_some()
    }

    pub fn bind_capability(
        &mut self,
        capability: Capability,
        binding: CapabilityBinding,
        now: MonotonicMillis,
    ) -> Result<(), RoutingErrorCategory> {
        if self
            .config
            .logical_server_id
            .is_some_and(|expected| expected != binding.logical_server_id)
            || self
                .config
                .supervisor_generation
                .is_some_and(|expected| expected != binding.supervisor_generation)
            || self
                .config
                .network_protocol
                .is_some_and(|expected| expected != binding.network_protocol)
            || self
                .config
                .content_fingerprint
                .is_some_and(|expected| expected != binding.content_fingerprint)
        {
            self.metrics
                .count_error(RoutingErrorCategory::ManifestIncompatible);
            return Err(RoutingErrorCategory::ManifestIncompatible);
        }
        if self
            .capabilities
            .live_for_lobby_session(binding.lobby_session_id)
            >= self.config.max_capabilities_per_lobby_session
        {
            self.metrics
                .count_error(RoutingErrorCategory::AllocationCapacity);
            return Err(RoutingErrorCategory::AllocationCapacity);
        }
        let Some(route) = self.routes.get(&binding.route_id) else {
            return Err(RoutingErrorCategory::ManifestIdentity);
        };
        let Some(worker) = self.workers.get(&binding.worker_id) else {
            return Err(RoutingErrorCategory::ManifestIdentity);
        };
        if route.worker_id != binding.worker_id
            || route.peer_id != binding.peer_id
            || worker.registration.generation != binding.worker_generation
        {
            return Err(RoutingErrorCategory::Binding);
        }
        self.capabilities
            .bind(capability, binding, now, self.config.max_capabilities)
    }

    pub fn authorize(
        &mut self,
        capability: &Capability,
        source: SocketAddr,
        now: MonotonicMillis,
    ) -> Result<Authorization, RoutingErrorCategory> {
        self.authorize_with_teardown(capability, source, now).0
    }

    /// Authorize a public source and eagerly remove an expired capability's route.
    ///
    /// The teardown is returned alongside the authorization result so an owner loop can notify
    /// the worker with the exact route/peer identity.  [`Self::authorize`] retains the historical
    /// result-only API for callers that only need the decision, while still applying the cleanup.
    pub fn authorize_with_teardown(
        &mut self,
        capability: &Capability,
        source: SocketAddr,
        now: MonotonicMillis,
    ) -> (
        Result<Authorization, RoutingErrorCategory>,
        Option<RouteTeardown>,
    ) {
        let status_before = self.capabilities.status(capability);
        let binding_before = self.capabilities.binding(capability);
        let result = self.capabilities.authorize(capability, source, now);
        let revoked = status_before.is_some_and(|status| status != CapabilityStatus::Revoked)
            && self.capabilities.status(capability) == Some(CapabilityStatus::Revoked);
        if revoked {
            self.metrics.capabilities_revoked += 1;
        }
        match result {
            Ok(authorization) => {
                if authorization.activated {
                    self.metrics.capabilities_activated += 1;
                }
                if authorization.rebound {
                    self.metrics.capability_rebinds += 1;
                }
                (Ok(authorization), None)
            }
            Err(category) => {
                self.metrics.count_error(category);
                let teardown = revoked
                    .then_some(binding_before)
                    .flatten()
                    .and_then(|binding| {
                        self.teardown_route_with_reason(binding.route_id, category)
                    });
                (Err(category), teardown)
            }
        }
    }

    /// Route one already-decoded public envelope while keeping its payload opaque.
    pub fn route_public(
        &mut self,
        envelope: &PublicEnvelope,
        source: SocketAddr,
        now: MonotonicMillis,
    ) -> Result<RouteRegistration, RoutingErrorCategory> {
        let route = match envelope.selector() {
            RouteSelector::DefaultLobby => {
                if let Some(route) = self.lobby_sources.get(&source).copied() {
                    route
                } else {
                    let template_id = self
                        .default_lobby_route
                        .ok_or(RoutingErrorCategory::CapabilityUnknown)?;
                    let template = self
                        .routes
                        .get(&template_id)
                        .copied()
                        .ok_or(RoutingErrorCategory::ManifestIdentity)?;
                    let route = self.allocate_lobby_route(template, source)?;
                    self.lobby_sources.insert(source, route);
                    route
                }
            }
            RouteSelector::Capability(capability) => {
                let route_id = self.authorize(capability, source, now)?.binding.route_id;
                self.routes
                    .get(&route_id)
                    .copied()
                    .ok_or(RoutingErrorCategory::ManifestIdentity)?
            }
        };
        let route_id = route.route_id;
        if matches!(envelope.selector(), RouteSelector::DefaultLobby) && !route.is_default_lobby {
            self.metrics.count_error(RoutingErrorCategory::Binding);
            return Err(RoutingErrorCategory::Binding);
        }
        if matches!(envelope.selector(), RouteSelector::DefaultLobby)
            && !self.is_default_template(route_id)
        {
            // The envelope has passed the exact codec/parser and route validation.  Refresh the
            // dynamic route before enqueueing so valid ongoing lobby traffic keeps its route
            // alive even when a bounded packet queue is temporarily full.
            self.lobby_route_last_seen.insert(route_id, now);
        }
        self.route_sources.insert(route_id, source);
        self.enqueue_packet(route_id, envelope.payload().to_vec())?;
        Ok(route)
    }

    /// Validate the lobby worker's explicit post-Netcode authentication fact and return the
    /// public source bound to that exact routed peer. Promotion is deliberately separate from
    /// envelope/route admission: a syntactically valid default-lobby packet can never promote a
    /// source, and a capability packet can never promote a lobby source.
    pub fn authenticated_lobby_source(
        &mut self,
        worker_id: WorkerId,
        fact: LobbyAuthenticatedBody,
    ) -> Result<SocketAddr, RoutingErrorCategory> {
        let Some(worker) = self.workers.get(&worker_id) else {
            self.metrics
                .count_error(RoutingErrorCategory::ManifestIdentity);
            return Err(RoutingErrorCategory::ManifestIdentity);
        };
        if worker.registration.kind != WorkerKind::Lobby {
            self.metrics.count_error(RoutingErrorCategory::Binding);
            return Err(RoutingErrorCategory::Binding);
        }
        let Some(route) = self.routes.get(&fact.route_id).copied() else {
            self.metrics.count_error(RoutingErrorCategory::Binding);
            return Err(RoutingErrorCategory::Binding);
        };
        if route.worker_id != worker_id
            || route.peer_id != fact.peer_id
            || !route.is_default_lobby
            || self.is_default_template(route.route_id)
        {
            self.metrics.count_error(RoutingErrorCategory::Binding);
            return Err(RoutingErrorCategory::Binding);
        }
        let Some(source) = self.route_sources.get(&route.route_id).copied() else {
            self.metrics.count_error(RoutingErrorCategory::Binding);
            return Err(RoutingErrorCategory::Binding);
        };
        if self
            .lobby_sources
            .get(&source)
            .is_none_or(|registered| registered.route_id != route.route_id)
        {
            self.metrics.count_error(RoutingErrorCategory::Binding);
            return Err(RoutingErrorCategory::Binding);
        }
        Ok(source)
    }

    /// Validate the lobby worker's post-Netcode authentication fact and return the public source
    /// bound to that exact routed peer. This fact intentionally has no Brawler lobby session: it
    /// is emitted at Lightyear `Connected`, before hello/session admission can accept or reject
    /// the client, and only promotes the cryptographically authenticated source out of pre-auth
    /// ingress accounting.
    pub fn authenticated_lobby_netcode_source(
        &mut self,
        worker_id: WorkerId,
        fact: LobbyNetcodeAuthenticatedBody,
    ) -> Result<SocketAddr, RoutingErrorCategory> {
        let Some(worker) = self.workers.get(&worker_id) else {
            self.metrics
                .count_error(RoutingErrorCategory::ManifestIdentity);
            return Err(RoutingErrorCategory::ManifestIdentity);
        };
        if worker.registration.kind != WorkerKind::Lobby {
            self.metrics.count_error(RoutingErrorCategory::Binding);
            return Err(RoutingErrorCategory::Binding);
        }
        let Some(route) = self.routes.get(&fact.route_id).copied() else {
            self.metrics.count_error(RoutingErrorCategory::Binding);
            return Err(RoutingErrorCategory::Binding);
        };
        if route.worker_id != worker_id
            || route.peer_id != fact.peer_id
            || !route.is_default_lobby
            || self.is_default_template(route.route_id)
        {
            self.metrics.count_error(RoutingErrorCategory::Binding);
            return Err(RoutingErrorCategory::Binding);
        }
        let Some(source) = self.route_sources.get(&route.route_id).copied() else {
            self.metrics.count_error(RoutingErrorCategory::Binding);
            return Err(RoutingErrorCategory::Binding);
        };
        if self
            .lobby_sources
            .get(&source)
            .is_none_or(|registered| registered.route_id != route.route_id)
        {
            self.metrics.count_error(RoutingErrorCategory::Binding);
            return Err(RoutingErrorCategory::Binding);
        }
        Ok(source)
    }

    fn allocate_lobby_route(
        &mut self,
        template: RouteRegistration,
        source: SocketAddr,
    ) -> Result<RouteRegistration, RoutingErrorCategory> {
        if self.routes.len() >= self.config.max_routes {
            self.metrics
                .count_error(RoutingErrorCategory::AllocationCapacity);
            return Err(RoutingErrorCategory::AllocationCapacity);
        }
        let _ = source;
        for _ in 0..16 {
            let route_id = random_route_id()?;
            if route_id == template.route_id || self.routes.contains_key(&route_id) {
                continue;
            }
            let peer_id = random_peer_id()?;
            if peer_id == template.peer_id {
                continue;
            }
            let route = RouteRegistration {
                route_id,
                worker_id: template.worker_id,
                peer_id,
                is_default_lobby: true,
            };
            self.packets.add_route(route);
            self.routes.insert(route_id, route);
            return Ok(route);
        }
        Err(RoutingErrorCategory::SupervisorInternal)
    }

    /// Validate one opaque worker packet and return its current public destination.
    pub fn accept_worker_packet(
        &mut self,
        packet: &PacketRecord,
    ) -> Result<SocketAddr, RoutingErrorCategory> {
        if packet.direction != PacketDirection::WorkerToSupervisor {
            self.metrics.count_error(RoutingErrorCategory::IpcMalformed);
            return Err(RoutingErrorCategory::IpcMalformed);
        }
        let Some(worker) = self.workers.get(&packet.worker_id) else {
            self.metrics
                .count_error(RoutingErrorCategory::ManifestIdentity);
            return Err(RoutingErrorCategory::ManifestIdentity);
        };
        let Some(route) = self.routes.get(&packet.route_id) else {
            self.metrics
                .count_error(RoutingErrorCategory::ManifestIdentity);
            return Err(RoutingErrorCategory::ManifestIdentity);
        };
        if route.worker_id != packet.worker_id
            || route.peer_id != packet.peer_id
            || worker.registration.worker_id != packet.worker_id
        {
            self.metrics.count_error(RoutingErrorCategory::Binding);
            return Err(RoutingErrorCategory::Binding);
        }
        self.route_sources
            .get(&packet.route_id)
            .copied()
            .ok_or_else(|| {
                self.metrics.count_error(RoutingErrorCategory::Binding);
                RoutingErrorCategory::Binding
            })
    }

    #[must_use]
    pub fn public_selector_for_route(&self, route_id: RouteId) -> Option<RouteSelector> {
        let route = self.routes.get(&route_id)?;
        if route.is_default_lobby {
            Some(RouteSelector::DefaultLobby)
        } else {
            self.capabilities
                .capability_for_route(route_id)
                .map(RouteSelector::Capability)
        }
    }

    #[must_use]
    pub fn source_for_route(&self, route_id: RouteId) -> Option<SocketAddr> {
        self.route_sources.get(&route_id).copied()
    }

    #[must_use]
    pub fn is_default_template(&self, route_id: RouteId) -> bool {
        self.default_lobby_route == Some(route_id)
    }

    pub fn note_error(&mut self, category: RoutingErrorCategory) {
        self.metrics.count_error(category);
    }

    pub fn revoke_capability(&mut self, capability: &Capability) -> bool {
        self.revoke_capability_with_teardown(capability).is_some()
    }

    pub fn revoke_capability_with_teardown(
        &mut self,
        capability: &Capability,
    ) -> Option<RouteTeardown> {
        let binding = self.capabilities.binding(capability)?;
        if !self.capabilities.revoke(capability) {
            return None;
        }
        self.metrics.capabilities_revoked += 1;
        self.teardown_route_with_reason(binding.route_id, RoutingErrorCategory::Revoked)
    }

    #[must_use]
    pub fn capability_status(&self, capability: &Capability) -> Option<CapabilityStatus> {
        self.capabilities.status(capability)
    }

    #[must_use]
    pub fn capability_count(&self) -> usize {
        self.capabilities.len()
    }

    /// Number of capabilities that can still authorize public ingress. Revoked records remain
    /// bounded negative-cache entries until their hard expiry, so lifecycle evidence should use
    /// this view when asserting active route ownership.
    #[must_use]
    pub fn live_capability_count(&self) -> usize {
        self.capabilities.live_len()
    }

    pub fn expire(&mut self, now: MonotonicMillis) -> Vec<RouteTeardown> {
        let counts = self.capabilities.expire(now);
        self.metrics.capabilities_revoked += counts.revoked as u64;
        for (category, count) in counts.errors {
            *self.metrics.error_counts.entry(category).or_default() += count as u64;
        }
        self.capabilities.purge_expired_negative_records(now);
        let mut teardowns = Vec::new();
        let mut seen_routes = std::collections::HashSet::new();
        for (binding, reason) in counts.bindings {
            if !seen_routes.insert(binding.route_id) {
                continue;
            }
            if let Some(teardown) = self.teardown_route_with_reason(binding.route_id, reason) {
                teardowns.push(teardown);
            }
        }
        let idle_lobby_routes: Vec<_> = self
            .lobby_route_last_seen
            .iter()
            .filter_map(|(&route_id, &last_seen)| {
                let is_template = self.default_lobby_route == Some(route_id);
                let idle = now.0.saturating_sub(last_seen.0) >= PUBLIC_LOBBY_ROUTE_IDLE_MILLIS;
                (!is_template && idle).then_some(route_id)
            })
            .collect();
        for route_id in idle_lobby_routes {
            if seen_routes.insert(route_id)
                && let Some(teardown) =
                    self.teardown_route_with_reason(route_id, RoutingErrorCategory::RouteExpired)
            {
                teardowns.push(teardown);
            }
        }
        teardowns
    }

    /// Remove a revoked route and its bounded queued packets exactly once.  Expiry reports this
    /// operation to the runtime first so it can issue a `PeerClose` over the worker control stream.
    pub fn teardown_route(&mut self, route_id: RouteId) -> Option<RouteTeardown> {
        self.teardown_route_with_reason(route_id, RoutingErrorCategory::RouteExpired)
    }

    /// Remove a route at the request of its owning worker after the worker has observed the
    /// Lightyear peer unlink. The identity pair is checked before teardown so a stale worker
    /// cannot close another generation's route.
    pub fn close_route_from_worker(
        &mut self,
        worker_id: WorkerId,
        route_id: RouteId,
        peer_id: PeerId,
    ) -> Result<Option<RouteTeardown>, RoutingErrorCategory> {
        let Some(route) = self.routes.get(&route_id).copied() else {
            return Ok(None);
        };
        if route.worker_id != worker_id || route.peer_id != peer_id {
            return Err(RoutingErrorCategory::Binding);
        }
        self.metrics.capabilities_revoked += self.capabilities.revoke_route(route_id) as u64;
        Ok(self.teardown_route_with_reason(route_id, RoutingErrorCategory::Revoked))
    }

    fn teardown_route_with_reason(
        &mut self,
        route_id: RouteId,
        reason: RoutingErrorCategory,
    ) -> Option<RouteTeardown> {
        let route = self.routes.remove(&route_id)?;
        self.route_sources.remove(&route_id);
        self.lobby_route_last_seen.remove(&route_id);
        self.lobby_sources
            .retain(|_, mapped| mapped.route_id != route_id);
        self.packets.remove_route(route_id);
        self.metrics.routes_cleaned += 1;
        let (frames, bytes) = self.packets.totals();
        self.metrics.observe_packet_queue(frames, bytes);
        Some(RouteTeardown {
            route_id,
            worker_id: route.worker_id,
            peer_id: route.peer_id,
            reason,
        })
    }

    pub fn enqueue_packet(
        &mut self,
        route_id: RouteId,
        payload: Vec<u8>,
    ) -> Result<(), RoutingErrorCategory> {
        let Some(route) = self.routes.get(&route_id).copied() else {
            self.metrics
                .count_error(RoutingErrorCategory::CapabilityUnknown);
            return Err(RoutingErrorCategory::CapabilityUnknown);
        };
        let result = self.packets.enqueue(route, payload);
        if let Err(category) = result {
            if category == RoutingErrorCategory::PacketQueueFull {
                self.metrics.packet_dropped_newest += 1;
            }
            self.metrics.count_error(category);
        }
        let (frames, bytes) = self.packets.totals();
        self.metrics.observe_packet_queue(frames, bytes);
        result
    }

    #[must_use]
    pub fn drain_packets(&mut self, maximum: usize) -> Vec<PacketRecord> {
        let records = self.packets.drain(maximum);
        let (frames, bytes) = self.packets.totals();
        self.metrics.observe_packet_queue(frames, bytes);
        records
    }

    pub fn enqueue_control(
        &mut self,
        worker_id: WorkerId,
        record: Vec<u8>,
    ) -> Result<(), RoutingErrorCategory> {
        if record.is_empty() || record.len() > crate::CONTROL_MAX_RECORD_BYTES {
            self.metrics.count_error(RoutingErrorCategory::IpcMalformed);
            return Err(RoutingErrorCategory::IpcMalformed);
        }
        let result = self.controls.enqueue(worker_id, record);
        if let Err(category) = result {
            self.metrics.control_rejected += 1;
            self.metrics.count_error(category);
        }
        let (frames, bytes) = self.controls.totals();
        self.metrics.observe_control_queue(frames, bytes);
        result
    }

    #[must_use]
    pub fn drain_controls(&mut self, maximum: usize) -> Vec<(WorkerId, Vec<u8>)> {
        let records = self.controls.drain(maximum);
        let (frames, bytes) = self.controls.totals();
        self.metrics.observe_control_queue(frames, bytes);
        records
    }

    pub fn cleanup_worker(&mut self, worker_id: WorkerId) -> Option<CleanupReport> {
        self.workers.remove(&worker_id)?;
        let route_ids: Vec<_> = self
            .routes
            .values()
            .filter(|route| route.worker_id == worker_id)
            .map(|route| route.route_id)
            .collect();
        if self
            .default_lobby_route
            .is_some_and(|route| route_ids.contains(&route))
        {
            self.default_lobby_route = None;
        }
        for route_id in &route_ids {
            self.routes.remove(route_id);
            self.route_sources.remove(route_id);
            self.lobby_route_last_seen.remove(route_id);
        }
        self.lobby_sources
            .retain(|_, route| route.worker_id != worker_id);
        let packet_frames_removed = self.packets.remove_worker(worker_id);
        let control_frames_removed = self.controls.remove_worker(worker_id);
        let capabilities_revoked = self.capabilities.revoke_worker(worker_id);
        self.metrics.workers_cleaned += 1;
        self.metrics.routes_cleaned += route_ids.len() as u64;
        self.metrics.capabilities_revoked += capabilities_revoked as u64;
        let (packet_frames, packet_bytes) = self.packets.totals();
        let (control_frames, control_bytes) = self.controls.totals();
        self.metrics
            .observe_packet_queue(packet_frames, packet_bytes);
        self.metrics
            .observe_control_queue(control_frames, control_bytes);
        Some(CleanupReport {
            routes_removed: route_ids.len(),
            capabilities_revoked,
            packet_frames_removed,
            control_frames_removed,
        })
    }

    #[must_use]
    pub const fn metrics(&self) -> &CoreMetrics {
        &self.metrics
    }

    #[must_use]
    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }

    #[must_use]
    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    /// Return the stable route/peer registrations owned by one worker.
    ///
    /// This narrow observability seam is used by process-isolation evidence to prove that
    /// allocation routes remain attached to the surviving worker after a sibling exits.  It
    /// exposes only stable routing identities; payloads, capabilities, and process handles stay
    /// private to the supervisor owner.
    #[must_use]
    pub fn routes_for_worker(&self, worker_id: WorkerId) -> Vec<RouteRegistration> {
        self.routes
            .values()
            .filter(|route| route.worker_id == worker_id)
            .copied()
            .collect()
    }
}

impl Default for SupervisorCore {
    fn default() -> Self {
        Self::new(CoreConfig::default())
    }
}

fn random_u128() -> Result<u128, RoutingErrorCategory> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| RoutingErrorCategory::SupervisorInternal)?;
    Ok(u128::from_be_bytes(bytes).max(1))
}

fn random_route_id() -> Result<RouteId, RoutingErrorCategory> {
    RouteId::new(random_u128()?).ok_or(RoutingErrorCategory::SupervisorInternal)
}

fn random_peer_id() -> Result<PeerId, RoutingErrorCategory> {
    PeerId::new(random_u128()?).ok_or(RoutingErrorCategory::SupervisorInternal)
}

#[cfg(test)]
mod tests;
