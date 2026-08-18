use std::collections::{HashMap, VecDeque};

use crate::{CodecError, PacketDirection, PacketRecord, RouteId, RoutingErrorCategory, WorkerId};

use super::{CoreConfig, RouteRegistration, WorkerKind};

#[derive(Clone, Debug, Default)]
struct RouteQueue {
    frames: VecDeque<PacketRecord>,
    bytes: usize,
}

#[derive(Clone, Debug)]
struct WorkerPacketQueue {
    kind: WorkerKind,
    routes: HashMap<RouteId, RouteQueue>,
    route_order: Vec<RouteId>,
    route_cursor: usize,
    frames: usize,
    bytes: usize,
    last_route: Option<RouteId>,
    consecutive: usize,
}

impl WorkerPacketQueue {
    fn new(kind: WorkerKind) -> Self {
        Self {
            kind,
            routes: HashMap::new(),
            route_order: Vec::new(),
            route_cursor: 0,
            frames: 0,
            bytes: 0,
            last_route: None,
            consecutive: 0,
        }
    }

    fn pop(&mut self, maximum_consecutive: usize) -> Option<PacketRecord> {
        if self.frames == 0 || self.route_order.is_empty() {
            return None;
        }
        let route_count = self.route_order.len();
        for offset in 0..route_count {
            let index = (self.route_cursor + offset) % route_count;
            let route_id = self.route_order[index];
            if self
                .routes
                .get(&route_id)
                .expect("route order is synchronized")
                .frames
                .is_empty()
            {
                continue;
            }
            let other_route_ready = self.routes.iter().any(|(candidate_id, candidate)| {
                *candidate_id != route_id && !candidate.frames.is_empty()
            });
            if self.last_route == Some(route_id)
                && self.consecutive >= maximum_consecutive
                && other_route_ready
            {
                continue;
            }
            let queue = self
                .routes
                .get_mut(&route_id)
                .expect("route order is synchronized");
            let packet = queue.frames.pop_front().expect("checked nonempty");
            let bytes = packet_size(&packet);
            queue.bytes -= bytes;
            self.frames -= 1;
            self.bytes -= bytes;
            self.route_cursor = (index + 1) % route_count;
            if self.last_route == Some(route_id) {
                self.consecutive += 1;
            } else {
                self.last_route = Some(route_id);
                self.consecutive = 1;
            }
            return Some(packet);
        }
        None
    }
}

#[derive(Clone, Debug)]
pub(super) struct PacketQueues {
    config: CoreConfig,
    workers: HashMap<WorkerId, WorkerPacketQueue>,
    match_order: Vec<WorkerId>,
    match_cursor: usize,
    lobby_worker: Option<WorkerId>,
    lobby_turn_due: bool,
    frames: usize,
    bytes: usize,
}

impl PacketQueues {
    pub fn new(config: CoreConfig) -> Self {
        Self {
            config,
            workers: HashMap::new(),
            match_order: Vec::new(),
            match_cursor: 0,
            lobby_worker: None,
            lobby_turn_due: true,
            frames: 0,
            bytes: 0,
        }
    }

    pub fn add_worker(&mut self, worker_id: WorkerId, kind: WorkerKind) {
        if kind == WorkerKind::Lobby {
            self.lobby_worker = Some(worker_id);
        } else {
            self.match_order.push(worker_id);
        }
        self.workers.insert(worker_id, WorkerPacketQueue::new(kind));
    }

    pub fn add_route(&mut self, route: RouteRegistration) {
        let worker = self
            .workers
            .get_mut(&route.worker_id)
            .expect("worker validated by core");
        worker.routes.insert(route.route_id, RouteQueue::default());
        worker.route_order.push(route.route_id);
    }

    pub fn enqueue(
        &mut self,
        route: RouteRegistration,
        payload: Vec<u8>,
    ) -> Result<(), RoutingErrorCategory> {
        let packet = PacketRecord::new(
            PacketDirection::SupervisorToWorker,
            route.worker_id,
            route.route_id,
            route.peer_id,
            payload,
        )
        .map_err(codec_to_routing)?;
        let bytes = packet_size(&packet);
        let worker = self
            .workers
            .get_mut(&route.worker_id)
            .ok_or(RoutingErrorCategory::ManifestIdentity)?;
        let queue = worker
            .routes
            .get_mut(&route.route_id)
            .ok_or(RoutingErrorCategory::ManifestIdentity)?;
        if queue.frames.len() >= self.config.route_packet_frames
            || queue.bytes.saturating_add(bytes) > self.config.route_packet_bytes
            || worker.frames >= self.config.worker_packet_frames
            || worker.bytes.saturating_add(bytes) > self.config.worker_packet_bytes
        {
            return Err(RoutingErrorCategory::PacketQueueFull);
        }
        queue.bytes += bytes;
        queue.frames.push_back(packet);
        worker.frames += 1;
        worker.bytes += bytes;
        self.frames += 1;
        self.bytes += bytes;
        Ok(())
    }

    pub fn drain(&mut self, maximum: usize) -> Vec<PacketRecord> {
        let mut result = Vec::with_capacity(maximum.min(self.frames));
        while result.len() < maximum {
            let worker_id = self.next_worker();
            let Some(worker_id) = worker_id else {
                break;
            };
            let Some(packet) = self
                .workers
                .get_mut(&worker_id)
                .and_then(|worker| worker.pop(self.config.max_consecutive_route_packets))
            else {
                continue;
            };
            self.frames -= 1;
            self.bytes -= packet_size(&packet);
            result.push(packet);
        }
        result
    }

    fn next_worker(&mut self) -> Option<WorkerId> {
        let lobby_ready = self.lobby_worker.filter(|id| {
            self.workers
                .get(id)
                .is_some_and(|worker| worker.frames != 0)
        });
        let match_ready = self.match_order.iter().any(|id| {
            self.workers
                .get(id)
                .is_some_and(|worker| worker.frames != 0)
        });
        if let Some(lobby) = lobby_ready {
            if !match_ready {
                return Some(lobby);
            }
            if self.lobby_turn_due {
                self.lobby_turn_due = false;
                return Some(lobby);
            }
            self.lobby_turn_due = true;
        }
        self.next_match_worker()
    }

    fn next_match_worker(&mut self) -> Option<WorkerId> {
        if self.match_order.is_empty() {
            return None;
        }
        for offset in 0..self.match_order.len() {
            let index = (self.match_cursor + offset) % self.match_order.len();
            let id = self.match_order[index];
            if self
                .workers
                .get(&id)
                .is_some_and(|worker| worker.frames != 0)
            {
                self.match_cursor = (index + 1) % self.match_order.len();
                return Some(id);
            }
        }
        None
    }

    pub fn remove_worker(&mut self, worker_id: WorkerId) -> usize {
        let Some(worker) = self.workers.remove(&worker_id) else {
            return 0;
        };
        self.frames -= worker.frames;
        self.bytes -= worker.bytes;
        if worker.kind == WorkerKind::Lobby {
            self.lobby_worker = None;
        }
        self.match_order.retain(|id| *id != worker_id);
        if self.match_order.is_empty() {
            self.match_cursor = 0;
        } else {
            self.match_cursor %= self.match_order.len();
        }
        worker.frames
    }

    pub fn remove_route(&mut self, route_id: RouteId) -> usize {
        for worker in self.workers.values_mut() {
            let Some(route) = worker.routes.remove(&route_id) else {
                continue;
            };
            worker.route_order.retain(|id| *id != route_id);
            if worker.route_order.is_empty() {
                worker.route_cursor = 0;
            } else {
                worker.route_cursor %= worker.route_order.len();
            }
            worker.frames -= route.frames.len();
            worker.bytes -= route.bytes;
            self.frames -= route.frames.len();
            self.bytes -= route.bytes;
            return route.frames.len();
        }
        0
    }

    pub const fn totals(&self) -> (usize, usize) {
        (self.frames, self.bytes)
    }
}

fn packet_size(packet: &PacketRecord) -> usize {
    crate::PACKET_HEADER_BYTES + packet.payload.len()
}

fn codec_to_routing(error: CodecError) -> RoutingErrorCategory {
    match error {
        CodecError::Oversize | CodecError::InvalidValue => RoutingErrorCategory::PublicMalformed,
        _ => RoutingErrorCategory::SupervisorInternal,
    }
}

#[derive(Clone, Debug, Default)]
struct ControlQueue {
    frames: VecDeque<Vec<u8>>,
    bytes: usize,
}

#[derive(Clone, Debug)]
pub(super) struct ControlQueues {
    config: CoreConfig,
    workers: HashMap<WorkerId, ControlQueue>,
    order: Vec<WorkerId>,
    cursor: usize,
    frames: usize,
    bytes: usize,
}

impl ControlQueues {
    pub fn new(config: CoreConfig) -> Self {
        Self {
            config,
            workers: HashMap::new(),
            order: Vec::new(),
            cursor: 0,
            frames: 0,
            bytes: 0,
        }
    }

    pub fn add_worker(&mut self, worker_id: WorkerId) {
        self.workers.insert(worker_id, ControlQueue::default());
        self.order.push(worker_id);
    }

    pub fn enqueue(
        &mut self,
        worker_id: WorkerId,
        record: Vec<u8>,
    ) -> Result<(), RoutingErrorCategory> {
        let queue = self
            .workers
            .get_mut(&worker_id)
            .ok_or(RoutingErrorCategory::ManifestIdentity)?;
        if queue.frames.len() >= self.config.worker_control_frames
            || queue.bytes.saturating_add(record.len()) > self.config.worker_control_bytes
        {
            return Err(RoutingErrorCategory::ControlQueueFull);
        }
        queue.bytes += record.len();
        queue.frames.push_back(record);
        self.frames += 1;
        self.bytes += queue.frames.back().map_or(0, Vec::len);
        Ok(())
    }

    pub fn drain(&mut self, maximum: usize) -> Vec<(WorkerId, Vec<u8>)> {
        let mut result = Vec::with_capacity(maximum.min(self.frames));
        while result.len() < maximum && self.frames != 0 {
            let mut found = false;
            for offset in 0..self.order.len() {
                let index = (self.cursor + offset) % self.order.len();
                let worker_id = self.order[index];
                let queue = self
                    .workers
                    .get_mut(&worker_id)
                    .expect("order is synchronized");
                if let Some(record) = queue.frames.pop_front() {
                    queue.bytes -= record.len();
                    self.frames -= 1;
                    self.bytes -= record.len();
                    self.cursor = (index + 1) % self.order.len();
                    result.push((worker_id, record));
                    found = true;
                    break;
                }
            }
            if !found {
                break;
            }
        }
        result
    }

    pub fn remove_worker(&mut self, worker_id: WorkerId) -> usize {
        let Some(queue) = self.workers.remove(&worker_id) else {
            return 0;
        };
        let frames = queue.frames.len();
        self.frames -= frames;
        self.bytes -= queue.bytes;
        self.order.retain(|id| *id != worker_id);
        if self.order.is_empty() {
            self.cursor = 0;
        } else {
            self.cursor %= self.order.len();
        }
        frames
    }

    pub const fn totals(&self) -> (usize, usize) {
        (self.frames, self.bytes)
    }
}
