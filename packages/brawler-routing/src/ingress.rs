//! Bounded public-ingress abuse accounting.
//!
//! The supervisor never replies to an unauthenticated source.  This registry only decides whether
//! a datagram may continue to envelope parsing/route admission; it owns no gameplay or worker
//! state and uses monotonic timestamps supplied by the owner loop for deterministic tests.

use std::{collections::BTreeMap, net::SocketAddr};

use crate::{
    MonotonicMillis, PUBLIC_MALFORMED_PER_WINDOW, PUBLIC_PREAUTH_BYTES_PER_WINDOW,
    PUBLIC_PREAUTH_DATAGRAMS_PER_WINDOW, PUBLIC_SOURCE_REGISTRY_MAX, PUBLIC_SUPPRESSION_MILLIS,
    PUBLIC_WINDOW_MILLIS,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IngressDecision {
    Allowed,
    SourceLimited,
    Suppressed,
}

#[derive(Clone, Copy, Debug)]
struct SourceRecord {
    window_start: MonotonicMillis,
    last_seen: MonotonicMillis,
    preauth_datagrams: u16,
    preauth_bytes: u32,
    malformed: u16,
    suppressed_until: Option<MonotonicMillis>,
    /// A source that has presented a currently valid capability is no longer treated as an
    /// unauthenticated lobby sender.  Malformed-packet suppression still applies independently.
    authenticated: bool,
}

impl SourceRecord {
    const fn new(now: MonotonicMillis) -> Self {
        Self {
            window_start: now,
            last_seen: now,
            preauth_datagrams: 0,
            preauth_bytes: 0,
            malformed: 0,
            suppressed_until: None,
            authenticated: false,
        }
    }

    fn roll_window(&mut self, now: MonotonicMillis) {
        if now.0.saturating_sub(self.window_start.0) >= PUBLIC_WINDOW_MILLIS {
            self.window_start = now;
            self.preauth_datagrams = 0;
            self.preauth_bytes = 0;
            self.malformed = 0;
        }
        self.last_seen = now;
    }
}

#[derive(Clone, Debug)]
pub struct SourceIngressLimiter {
    sources: BTreeMap<SocketAddr, SourceRecord>,
    maximum_sources: usize,
}

impl Default for SourceIngressLimiter {
    fn default() -> Self {
        Self::new(PUBLIC_SOURCE_REGISTRY_MAX)
    }
}

impl SourceIngressLimiter {
    #[must_use]
    pub fn new(maximum_sources: usize) -> Self {
        Self {
            sources: BTreeMap::new(),
            maximum_sources: maximum_sources.max(1),
        }
    }

    /// Admit one default-lobby datagram under the pre-auth budget.
    pub fn admit_default(
        &mut self,
        source: SocketAddr,
        bytes: usize,
        now: MonotonicMillis,
    ) -> IngressDecision {
        let record = self.record(source, now);
        record.roll_window(now);
        if record.suppressed_until.is_some_and(|until| now.0 < until.0) {
            return IngressDecision::Suppressed;
        }
        if record.authenticated {
            return IngressDecision::Allowed;
        }
        if usize::from(record.preauth_datagrams) >= PUBLIC_PREAUTH_DATAGRAMS_PER_WINDOW
            || usize::try_from(record.preauth_bytes)
                .unwrap_or(usize::MAX)
                .saturating_add(bytes)
                > PUBLIC_PREAUTH_BYTES_PER_WINDOW
        {
            return IngressDecision::SourceLimited;
        }
        record.preauth_datagrams = record.preauth_datagrams.saturating_add(1);
        record.preauth_bytes = record
            .preauth_bytes
            .saturating_add(u32::try_from(bytes).unwrap_or(u32::MAX));
        IngressDecision::Allowed
    }

    /// Promote a source after the supervisor has successfully authorized a capability-bearing
    /// envelope.  This prevents a legitimate session from remaining trapped at the small
    /// unauthenticated lobby budget for its entire lifetime while preserving malformed traffic
    /// suppression and bounded source-record eviction.
    pub fn promote_authenticated(&mut self, source: SocketAddr, now: MonotonicMillis) {
        let record = self.record(source, now);
        record.roll_window(now);
        record.authenticated = true;
        record.preauth_datagrams = 0;
        record.preauth_bytes = 0;
    }

    /// Record a malformed public datagram. Returns `Suppressed` only once the 60-second source
    /// suppression is active; callers still count the individual malformed packet separately.
    pub fn record_malformed(
        &mut self,
        source: SocketAddr,
        now: MonotonicMillis,
    ) -> IngressDecision {
        let record = self.record(source, now);
        record.roll_window(now);
        if record.suppressed_until.is_some_and(|until| now.0 < until.0) {
            return IngressDecision::Suppressed;
        }
        record.malformed = record.malformed.saturating_add(1);
        if usize::from(record.malformed) >= PUBLIC_MALFORMED_PER_WINDOW {
            record.suppressed_until = Some(now.saturating_add(PUBLIC_SUPPRESSION_MILLIS));
            return IngressDecision::Suppressed;
        }
        IngressDecision::Allowed
    }

    #[must_use]
    pub fn is_suppressed(&self, source: SocketAddr, now: MonotonicMillis) -> bool {
        self.sources
            .get(&source)
            .and_then(|record| record.suppressed_until)
            .is_some_and(|until| now.0 < until.0)
    }

    /// Expire idle records. Eviction is deterministic: oldest `last_seen`, then socket ordering.
    pub fn expire(&mut self, now: MonotonicMillis) {
        self.sources.retain(|_, record| {
            let suppression_live = record.suppressed_until.is_some_and(|until| now.0 < until.0);
            suppression_live || now.0.saturating_sub(record.last_seen.0) < PUBLIC_SUPPRESSION_MILLIS
        });
    }

    #[must_use]
    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    fn record(&mut self, source: SocketAddr, now: MonotonicMillis) -> &mut SourceRecord {
        if !self.sources.contains_key(&source) {
            if self.sources.len() >= self.maximum_sources {
                let evicted = self
                    .sources
                    .iter()
                    .min_by_key(|(address, record)| (record.last_seen, **address))
                    .map(|(address, _)| *address);
                if let Some(address) = evicted {
                    self.sources.remove(&address);
                }
            }
            self.sources.insert(source, SourceRecord::new(now));
        }
        self.sources
            .get_mut(&source)
            .expect("record inserted above")
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

    use super::*;

    fn source(port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)
    }

    #[test]
    fn default_budget_is_eight_datagrams_and_nine_kibibytes_per_window() {
        let mut limiter = SourceIngressLimiter::default();
        let now = MonotonicMillis(0);
        for _ in 0..PUBLIC_PREAUTH_DATAGRAMS_PER_WINDOW {
            assert_eq!(
                limiter.admit_default(source(1), 1_120, now),
                IngressDecision::Allowed
            );
        }
        assert_eq!(
            limiter.admit_default(source(1), 1, now),
            IngressDecision::SourceLimited
        );
        assert_eq!(
            limiter.admit_default(source(2), 9_000, now),
            IngressDecision::Allowed
        );
        assert_eq!(
            limiter.admit_default(source(2), 216, now),
            IngressDecision::Allowed
        );
        assert_eq!(
            limiter.admit_default(source(2), 1, now),
            IngressDecision::SourceLimited
        );
        assert_eq!(
            limiter.admit_default(source(1), 1, MonotonicMillis(PUBLIC_WINDOW_MILLIS)),
            IngressDecision::Allowed
        );
    }

    #[test]
    fn malformed_threshold_suppresses_for_sixty_seconds_and_then_recovers() {
        let mut limiter = SourceIngressLimiter::default();
        let now = MonotonicMillis(4);
        for _ in 0..PUBLIC_MALFORMED_PER_WINDOW - 1 {
            assert_eq!(
                limiter.record_malformed(source(3), now),
                IngressDecision::Allowed
            );
        }
        assert_eq!(
            limiter.record_malformed(source(3), now),
            IngressDecision::Suppressed
        );
        assert!(limiter.is_suppressed(source(3), now));
        assert_eq!(
            limiter.record_malformed(
                source(3),
                MonotonicMillis(now.0 + PUBLIC_SUPPRESSION_MILLIS)
            ),
            IngressDecision::Allowed
        );
        assert!(!limiter.is_suppressed(
            source(3),
            MonotonicMillis(now.0 + PUBLIC_SUPPRESSION_MILLIS)
        ));
    }

    #[test]
    fn authenticated_source_is_promoted_out_of_preauth_budget() {
        let mut limiter = SourceIngressLimiter::default();
        let now = MonotonicMillis(7);
        for _ in 0..PUBLIC_PREAUTH_DATAGRAMS_PER_WINDOW {
            assert_eq!(
                limiter.admit_default(source(6), 1, now),
                IngressDecision::Allowed
            );
        }
        assert_eq!(
            limiter.admit_default(source(6), 1, now),
            IngressDecision::SourceLimited
        );
        limiter.promote_authenticated(source(6), now);
        for _ in 0..(PUBLIC_PREAUTH_DATAGRAMS_PER_WINDOW * 2) {
            assert_eq!(
                limiter.admit_default(source(6), 1, now),
                IngressDecision::Allowed
            );
        }
    }

    #[test]
    fn registry_eviction_is_bounded_and_deterministic_across_ipv4_ipv6() {
        let mut limiter = SourceIngressLimiter::new(2);
        limiter.record_malformed(source(4), MonotonicMillis(1));
        limiter.record_malformed(
            SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 4),
            MonotonicMillis(2),
        );
        limiter.record_malformed(source(5), MonotonicMillis(3));
        assert_eq!(limiter.source_count(), 2);
        assert!(!limiter.is_suppressed(source(4), MonotonicMillis(3)));
    }
}
