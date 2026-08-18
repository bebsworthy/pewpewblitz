use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
    net::SocketAddr,
};

use crate::{
    CAPABILITY_HARD_LIFETIME_MILLIS, CAPABILITY_IDLE_MILLIS, CAPABILITY_PENDING_MILLIS,
    CAPABILITY_REBIND_WINDOW_MILLIS, CAPABILITY_REBINDS_PER_WINDOW, Capability, RouteId,
    RoutingErrorCategory, WorkerId,
};

use super::{Authorization, CapabilityBinding, CapabilityStatus, MonotonicMillis};

#[derive(Clone)]
enum State {
    Pending,
    Active {
        newest_source: SocketAddr,
        seen_sources: HashSet<SocketAddr>,
        rebinds: VecDeque<MonotonicMillis>,
        last_seen: MonotonicMillis,
    },
    Revoked,
}

impl fmt::Debug for State {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => formatter.write_str("Pending"),
            Self::Active {
                seen_sources,
                rebinds,
                last_seen,
                ..
            } => formatter
                .debug_struct("Active")
                .field("source_count", &seen_sources.len())
                .field("rebind_count", &rebinds.len())
                .field("last_seen", last_seen)
                .finish(),
            Self::Revoked => formatter.write_str("Revoked"),
        }
    }
}

#[derive(Clone)]
struct Record {
    binding: CapabilityBinding,
    created_at: MonotonicMillis,
    pending_expiry: MonotonicMillis,
    hard_expiry: MonotonicMillis,
    state: State,
}

impl fmt::Debug for Record {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Record")
            .field("binding", &self.binding)
            .field("created_at", &self.created_at)
            .field("pending_expiry", &self.pending_expiry)
            .field("hard_expiry", &self.hard_expiry)
            .field("state", &self.state)
            .finish()
    }
}

#[derive(Clone, Default)]
pub(super) struct CapabilityRegistry {
    records: HashMap<Capability, Record>,
}

impl fmt::Debug for CapabilityRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CapabilityRegistry")
            .field("record_count", &self.records.len())
            .finish()
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct ExpiryCounts {
    pub revoked: usize,
    pub errors: Vec<(RoutingErrorCategory, usize)>,
    pub bindings: Vec<(CapabilityBinding, RoutingErrorCategory)>,
}

impl CapabilityRegistry {
    pub fn binding(&self, capability: &Capability) -> Option<CapabilityBinding> {
        self.records.get(capability).map(|record| record.binding)
    }

    pub fn live_for_lobby_session(&self, session: crate::LobbySessionId) -> usize {
        self.records
            .values()
            .filter(|record| {
                record.binding.lobby_session_id == session
                    && !matches!(record.state, State::Revoked)
            })
            .count()
    }

    pub fn capability_for_route(&self, route_id: RouteId) -> Option<Capability> {
        self.records.iter().find_map(|(capability, record)| {
            (record.binding.route_id == route_id && !matches!(record.state, State::Revoked))
                .then(|| capability.clone())
        })
    }

    pub fn bind(
        &mut self,
        capability: Capability,
        binding: CapabilityBinding,
        now: MonotonicMillis,
        maximum_records: usize,
    ) -> Result<(), RoutingErrorCategory> {
        if self.records.contains_key(&capability) {
            return Err(RoutingErrorCategory::Binding);
        }
        if self.records.len() >= maximum_records {
            return Err(RoutingErrorCategory::AllocationCapacity);
        }
        self.records.insert(
            capability,
            Record {
                binding,
                created_at: now,
                pending_expiry: now.saturating_add(CAPABILITY_PENDING_MILLIS),
                hard_expiry: now.saturating_add(CAPABILITY_HARD_LIFETIME_MILLIS),
                state: State::Pending,
            },
        );
        Ok(())
    }

    pub fn authorize(
        &mut self,
        capability: &Capability,
        source: SocketAddr,
        now: MonotonicMillis,
    ) -> Result<Authorization, RoutingErrorCategory> {
        let record = self
            .records
            .get_mut(capability)
            .ok_or(RoutingErrorCategory::CapabilityUnknown)?;
        if now < record.created_at {
            return Err(RoutingErrorCategory::SupervisorInternal);
        }
        if now >= record.hard_expiry {
            record.state = State::Revoked;
            return Err(RoutingErrorCategory::RouteExpired);
        }
        match &mut record.state {
            State::Pending => {
                if now >= record.pending_expiry {
                    record.state = State::Revoked;
                    return Err(RoutingErrorCategory::PendingExpired);
                }
                let mut seen_sources = HashSet::with_capacity(CAPABILITY_REBINDS_PER_WINDOW + 1);
                seen_sources.insert(source);
                record.state = State::Active {
                    newest_source: source,
                    seen_sources,
                    rebinds: VecDeque::with_capacity(CAPABILITY_REBINDS_PER_WINDOW),
                    last_seen: now,
                };
                Ok(Authorization {
                    binding: record.binding,
                    activated: true,
                    rebound: false,
                })
            }
            State::Active {
                newest_source,
                seen_sources,
                rebinds,
                last_seen,
            } => {
                if now < *last_seen {
                    return Err(RoutingErrorCategory::SupervisorInternal);
                }
                if now.0.saturating_sub(last_seen.0) >= CAPABILITY_IDLE_MILLIS {
                    record.state = State::Revoked;
                    return Err(RoutingErrorCategory::RouteExpired);
                }
                if *newest_source == source {
                    *last_seen = now;
                    return Ok(Authorization {
                        binding: record.binding,
                        activated: false,
                        rebound: false,
                    });
                }
                if seen_sources.contains(&source) {
                    return Err(RoutingErrorCategory::Binding);
                }
                while rebinds
                    .front()
                    .is_some_and(|at| now.0.saturating_sub(at.0) >= CAPABILITY_REBIND_WINDOW_MILLIS)
                {
                    rebinds.pop_front();
                }
                if rebinds.len() >= CAPABILITY_REBINDS_PER_WINDOW {
                    return Err(RoutingErrorCategory::RebindLimited);
                }
                rebinds.push_back(now);
                seen_sources.insert(source);
                *newest_source = source;
                *last_seen = now;
                Ok(Authorization {
                    binding: record.binding,
                    activated: false,
                    rebound: true,
                })
            }
            State::Revoked => Err(RoutingErrorCategory::Revoked),
        }
    }

    pub fn revoke(&mut self, capability: &Capability) -> bool {
        let Some(record) = self.records.get_mut(capability) else {
            return false;
        };
        if matches!(record.state, State::Revoked) {
            return false;
        }
        record.state = State::Revoked;
        true
    }

    pub fn revoke_worker(&mut self, worker_id: WorkerId) -> usize {
        let mut revoked = 0;
        for record in self
            .records
            .values_mut()
            .filter(|record| record.binding.worker_id == worker_id)
        {
            if !matches!(record.state, State::Revoked) {
                record.state = State::Revoked;
                revoked += 1;
            }
        }
        revoked
    }

    pub fn revoke_route(&mut self, route_id: RouteId) -> usize {
        let mut revoked = 0;
        for record in self
            .records
            .values_mut()
            .filter(|record| record.binding.route_id == route_id)
        {
            if !matches!(record.state, State::Revoked) {
                record.state = State::Revoked;
                revoked += 1;
            }
        }
        revoked
    }

    pub fn status(&self, capability: &Capability) -> Option<CapabilityStatus> {
        self.records
            .get(capability)
            .map(|record| match record.state {
                State::Pending => CapabilityStatus::Pending,
                State::Active { .. } => CapabilityStatus::Active,
                State::Revoked => CapabilityStatus::Revoked,
            })
    }

    pub fn expire(&mut self, now: MonotonicMillis) -> ExpiryCounts {
        let mut pending = 0;
        let mut routes = 0;
        let mut bindings = Vec::new();
        for record in self.records.values_mut() {
            let category = match &record.state {
                State::Pending if now >= record.pending_expiry => {
                    Some(RoutingErrorCategory::PendingExpired)
                }
                State::Active { last_seen, .. }
                    if now >= record.hard_expiry
                        || now.0.saturating_sub(last_seen.0) >= CAPABILITY_IDLE_MILLIS =>
                {
                    Some(RoutingErrorCategory::RouteExpired)
                }
                _ => None,
            };
            if let Some(category) = category {
                record.state = State::Revoked;
                bindings.push((record.binding, category));
                match category {
                    RoutingErrorCategory::PendingExpired => pending += 1,
                    RoutingErrorCategory::RouteExpired => routes += 1,
                    _ => unreachable!(),
                }
            }
        }
        let mut errors = Vec::new();
        if pending != 0 {
            errors.push((RoutingErrorCategory::PendingExpired, pending));
        }
        if routes != 0 {
            errors.push((RoutingErrorCategory::RouteExpired, routes));
        }
        ExpiryCounts {
            revoked: pending + routes,
            errors,
            bindings,
        }
    }

    pub fn purge_expired_negative_records(&mut self, now: MonotonicMillis) {
        self.records.retain(|_, record| now < record.hard_expiry);
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn live_len(&self) -> usize {
        self.records
            .values()
            .filter(|record| !matches!(record.state, State::Revoked))
            .count()
    }
}
