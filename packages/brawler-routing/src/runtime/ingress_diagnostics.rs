//! Rate-limited public-ingress diagnostics (BRL-2nsg).
//!
//! The owner loop drops public datagrams without replying (unknown, limited, or not-yet-routable
//! sources). Counters alone left a device that could not reconnect indistinguishable from a
//! network fault, so each drop and each suspicious lobby-route reuse is reported as a bounded
//! diagnostic: at most one line per source and kind per window, plus one summary per window that
//! counts everything, including the lines that were folded into it.
//!
//! Capability-selector (match and rejoin) routes are observed too, because a match handoff that
//! never gets a Netcode challenge otherwise leaves no supervisor trace at all: the first datagram
//! from each new source on a route is reported once with the worker it was forwarded to, and any
//! route that carried a new source or a Netcode connection request in a window gets one
//! per-route line with that window's forwarded and reply counts.

use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
};

use crate::{
    MonotonicMillis, PUBLIC_WINDOW_MILLIS, RouteId, RoutingErrorCategory, WorkerId, WorkerKind,
};

/// Per-source diagnostic records retained at once. A flood from more sources than this is still
/// counted in the summary; it just stops naming each source.
const MAX_LOGGED_SOURCES: usize = 256;
/// A lobby route that has carried no datagram for this long and then resumes is reported. Live
/// Lightyear sessions send at least a keep-alive every 100 ms, so a gap this long means the peer
/// went silent (an app killed or a network change) and something reused its route.
pub(super) const LOBBY_ROUTE_REUSE_IDLE_MILLIS: u64 = 1_000;
/// A Netcode connection request arriving on a lobby route older than this is reported: a fresh
/// handshake on an established route means a new client is talking through an old session.
pub(super) const LOBBY_ROUTE_REUSE_AGE_MILLIS: u64 = 1_000;
/// `(route, source)` pairs remembered for first-datagram reporting. Pairs idle for
/// [`CAPABILITY_SOURCE_FORGET_MILLIS`] are evicted first when the table is full; past the bound a
/// new pair is still counted and forwarded, just not named.
const MAX_CAPABILITY_ROUTE_SOURCES: usize = 1_024;
/// A `(route, source)` pair silent for this long may be evicted from the first-seen table.
const CAPABILITY_SOURCE_FORGET_MILLIS: u64 = 60_000;
/// Size of a Netcode 1.02 connection request (prefix, version, protocol, expiry, nonce, token).
/// The supervisor never parses Netcode; this shape check only classifies diagnostic lines and
/// picks which pre-auth allowance a default-lobby datagram draws from.
pub(super) const NETCODE_CONNECTION_REQUEST_BYTES: usize = 1 + 13 + 8 + 8 + 24 + 1_024;

/// Whether an opaque lobby payload has the shape of a Netcode connection request. Every other
/// Netcode packet carries a nonzero sequence-length nibble in its prefix byte, so prefix 0 with
/// the exact request size identifies one.
#[must_use]
pub fn looks_like_netcode_connection_request(payload: &[u8]) -> bool {
    payload.len() == NETCODE_CONNECTION_REQUEST_BYTES && payload.first() == Some(&0)
}

/// Why the owner loop dropped one public datagram before it reached a worker.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IngressDropReason {
    /// The source is inside its malformed-traffic suppression window.
    Suppressed,
    /// The envelope failed to decode; the category says how.
    Malformed(RoutingErrorCategory),
    /// A default-lobby envelope arrived before the lobby worker's route was published.
    LobbyNotReady,
    /// The source exhausted its pre-authentication datagram or byte budget for this window.
    SourceLimited,
    /// The envelope decoded but route admission refused it (unknown or expired capability,
    /// capacity, queue full, ...).
    RouteRejected(RoutingErrorCategory),
}

impl IngressDropReason {
    const fn kind(self) -> DiagnosticKind {
        match self {
            Self::Suppressed => DiagnosticKind::Suppressed,
            Self::Malformed(_) => DiagnosticKind::Malformed,
            Self::LobbyNotReady => DiagnosticKind::LobbyNotReady,
            Self::SourceLimited => DiagnosticKind::SourceLimited,
            Self::RouteRejected(_) => DiagnosticKind::RouteRejected,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum DiagnosticKind {
    Suppressed,
    Malformed,
    LobbyNotReady,
    SourceLimited,
    RouteRejected,
    LobbyRouteReuse,
}

/// One bounded ingress observation, rendered by the production owner as a log line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IngressDiagnostic {
    /// The first drop of this kind from `source` in the window. `folded` counts earlier drops of
    /// the same kind from the source that were not logged individually.
    Drop {
        reason: IngressDropReason,
        source: SocketAddr,
        folded: u32,
    },
    /// A default-lobby datagram reused the route already mapped to its source after the route
    /// went idle, or a Netcode connection request arrived on an established route.
    LobbyRouteReuse {
        source: SocketAddr,
        route_id: RouteId,
        route_age_millis: u64,
        idle_millis: u64,
        connection_request: bool,
        folded: u32,
    },
    /// The first datagram from `source` on a capability-selector route, forwarded to the route's
    /// worker. Reported once per `(route, source)`.
    CapabilityRouteFirstSeen {
        route_id: RouteId,
        worker_id: WorkerId,
        worker_kind: Option<WorkerKind>,
        source: SocketAddr,
        payload_len: usize,
        connection_request: bool,
    },
    /// One window of a capability-selector route that carried a new source or a Netcode
    /// connection request. Counting starts at that datagram, so steady match traffic never
    /// produces this line. `replies` counts worker datagrams queued back to the route's source.
    CapabilityRouteWindow {
        route_id: RouteId,
        worker_id: WorkerId,
        window_millis: u64,
        new_sources: u32,
        forwarded: u64,
        connection_requests: u64,
        forwarded_bytes: u64,
        replies: u64,
    },
    /// Every drop and reuse of the last window, including the ones folded out of the lines.
    Summary {
        window_millis: u64,
        suppressed: u64,
        malformed: u64,
        lobby_not_ready: u64,
        source_limited: u64,
        route_rejected: u64,
        lobby_route_reuses: u64,
        /// Capability-route datagrams forwarded from a `(route, source)` seen for the first time.
        capability_first_seen: u64,
        /// Capability-route datagrams forwarded that have the shape of a Netcode request.
        capability_connection_requests: u64,
        sources: usize,
    },
}

#[derive(Clone, Copy, Debug)]
struct SourceLine {
    logged_at: MonotonicMillis,
    folded: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Counts {
    suppressed: u64,
    malformed: u64,
    lobby_not_ready: u64,
    source_limited: u64,
    route_rejected: u64,
    lobby_route_reuses: u64,
    capability_first_seen: u64,
    capability_connection_requests: u64,
}

impl Counts {
    fn add(&mut self, kind: DiagnosticKind) {
        let slot = match kind {
            DiagnosticKind::Suppressed => &mut self.suppressed,
            DiagnosticKind::Malformed => &mut self.malformed,
            DiagnosticKind::LobbyNotReady => &mut self.lobby_not_ready,
            DiagnosticKind::SourceLimited => &mut self.source_limited,
            DiagnosticKind::RouteRejected => &mut self.route_rejected,
            DiagnosticKind::LobbyRouteReuse => &mut self.lobby_route_reuses,
        };
        *slot = slot.saturating_add(1);
    }
}

/// One capability route's counters for the current window.
#[derive(Clone, Copy, Debug)]
struct RouteWindow {
    worker_id: WorkerId,
    new_sources: u32,
    forwarded: u64,
    connection_requests: u64,
    forwarded_bytes: u64,
    replies: u64,
}

/// Bounded rate limiter for [`IngressDiagnostic`]s.
#[derive(Debug, Default)]
pub(super) struct IngressDiagnostics {
    lines: HashMap<(SocketAddr, DiagnosticKind), SourceLine>,
    counts: Counts,
    window_sources: HashSet<SocketAddr>,
    window_started: Option<MonotonicMillis>,
    /// Last datagram time of each `(route, source)` already reported as first seen.
    capability_sources: HashMap<(RouteId, SocketAddr), MonotonicMillis>,
    /// Capability routes being counted in the current window, bounded by `MAX_LOGGED_SOURCES`.
    route_windows: HashMap<RouteId, RouteWindow>,
}

impl IngressDiagnostics {
    pub(super) fn drop(
        &mut self,
        reason: IngressDropReason,
        source: SocketAddr,
        now: MonotonicMillis,
        out: &mut Vec<IngressDiagnostic>,
    ) {
        if let Some(folded) = self.observe(reason.kind(), source, now) {
            out.push(IngressDiagnostic::Drop {
                reason,
                source,
                folded,
            });
        }
    }

    /// Record a lobby-route reuse; `folded` in `reuse` is filled in here.
    pub(super) fn lobby_route_reuse(
        &mut self,
        mut reuse: IngressDiagnostic,
        now: MonotonicMillis,
        out: &mut Vec<IngressDiagnostic>,
    ) {
        let IngressDiagnostic::LobbyRouteReuse { source, .. } = reuse else {
            return;
        };
        if let Some(folded) = self.observe(DiagnosticKind::LobbyRouteReuse, source, now) {
            if let IngressDiagnostic::LobbyRouteReuse { folded: slot, .. } = &mut reuse {
                *slot = folded;
            }
            out.push(reuse);
        }
    }

    /// Record one public datagram that a capability selector routed to `worker_id`. This runs for
    /// every forwarded match datagram, so it costs one map update plus, while the route is being
    /// counted, one more.
    #[allow(
        clippy::too_many_arguments,
        reason = "one flat call from the owner loop's hot path; a struct would only be rebuilt per datagram"
    )]
    pub(super) fn capability_forwarded(
        &mut self,
        route_id: RouteId,
        worker_id: WorkerId,
        worker_kind: Option<WorkerKind>,
        source: SocketAddr,
        payload: &[u8],
        now: MonotonicMillis,
        out: &mut Vec<IngressDiagnostic>,
    ) {
        let connection_request = looks_like_netcode_connection_request(payload);
        let first_seen = self.note_capability_source(route_id, source, now);
        if first_seen {
            self.counts.capability_first_seen = self.counts.capability_first_seen.saturating_add(1);
            out.push(IngressDiagnostic::CapabilityRouteFirstSeen {
                route_id,
                worker_id,
                worker_kind,
                source,
                payload_len: payload.len(),
                connection_request,
            });
        }
        if connection_request {
            self.counts.capability_connection_requests =
                self.counts.capability_connection_requests.saturating_add(1);
        }
        let window = if first_seen || connection_request {
            self.window_started.get_or_insert(now);
            if self.route_windows.len() >= MAX_LOGGED_SOURCES
                && !self.route_windows.contains_key(&route_id)
            {
                return;
            }
            self.route_windows.entry(route_id).or_insert(RouteWindow {
                worker_id,
                new_sources: 0,
                forwarded: 0,
                connection_requests: 0,
                forwarded_bytes: 0,
                replies: 0,
            })
        } else if self.route_windows.is_empty() {
            return;
        } else if let Some(window) = self.route_windows.get_mut(&route_id) {
            window
        } else {
            return;
        };
        window.new_sources = window.new_sources.saturating_add(u32::from(first_seen));
        window.forwarded = window.forwarded.saturating_add(1);
        window.connection_requests = window
            .connection_requests
            .saturating_add(u64::from(connection_request));
        window.forwarded_bytes = window
            .forwarded_bytes
            .saturating_add(u64::try_from(payload.len()).unwrap_or(u64::MAX));
    }

    /// Record one worker datagram queued back to the public source of `route_id`. Only routes
    /// being counted in the current window are touched.
    pub(super) fn capability_reply(&mut self, route_id: RouteId) {
        if self.route_windows.is_empty() {
            return;
        }
        if let Some(window) = self.route_windows.get_mut(&route_id) {
            window.replies = window.replies.saturating_add(1);
        }
    }

    /// Remember `(route, source)` and say whether it is new. When the table is full, pairs idle
    /// for a minute are evicted; if it is still full the pair is neither stored nor reported.
    fn note_capability_source(
        &mut self,
        route_id: RouteId,
        source: SocketAddr,
        now: MonotonicMillis,
    ) -> bool {
        if let Some(last_seen) = self.capability_sources.get_mut(&(route_id, source)) {
            *last_seen = now;
            return false;
        }
        if self.capability_sources.len() >= MAX_CAPABILITY_ROUTE_SOURCES {
            self.capability_sources.retain(|_, last_seen| {
                now.0.saturating_sub(last_seen.0) < CAPABILITY_SOURCE_FORGET_MILLIS
            });
            if self.capability_sources.len() >= MAX_CAPABILITY_ROUTE_SOURCES {
                return false;
            }
        }
        self.capability_sources.insert((route_id, source), now);
        true
    }

    /// Close the window once it is a full window old and report it if anything happened.
    pub(super) fn flush(&mut self, now: MonotonicMillis, out: &mut Vec<IngressDiagnostic>) {
        let Some(started) = self.window_started else {
            return;
        };
        let window_millis = now.0.saturating_sub(started.0);
        if window_millis < PUBLIC_WINDOW_MILLIS {
            return;
        }
        let mut routes = self.route_windows.drain().collect::<Vec<_>>();
        routes.sort_unstable_by_key(|(route_id, _)| route_id.get());
        out.extend(routes.into_iter().map(|(route_id, window)| {
            IngressDiagnostic::CapabilityRouteWindow {
                route_id,
                worker_id: window.worker_id,
                window_millis,
                new_sources: window.new_sources,
                forwarded: window.forwarded,
                connection_requests: window.connection_requests,
                forwarded_bytes: window.forwarded_bytes,
                replies: window.replies,
            }
        }));
        let counts = std::mem::take(&mut self.counts);
        out.push(IngressDiagnostic::Summary {
            window_millis,
            suppressed: counts.suppressed,
            malformed: counts.malformed,
            lobby_not_ready: counts.lobby_not_ready,
            source_limited: counts.source_limited,
            route_rejected: counts.route_rejected,
            lobby_route_reuses: counts.lobby_route_reuses,
            capability_first_seen: counts.capability_first_seen,
            capability_connection_requests: counts.capability_connection_requests,
            sources: self.window_sources.len(),
        });
        self.window_sources.clear();
        self.window_started = None;
        self.lines
            .retain(|_, line| now.0.saturating_sub(line.logged_at.0) < PUBLIC_WINDOW_MILLIS);
    }

    /// Count one observation and return `Some(folded)` when it earns its own line.
    fn observe(
        &mut self,
        kind: DiagnosticKind,
        source: SocketAddr,
        now: MonotonicMillis,
    ) -> Option<u32> {
        self.counts.add(kind);
        self.window_started.get_or_insert(now);
        if self.window_sources.len() < MAX_LOGGED_SOURCES {
            self.window_sources.insert(source);
        }
        let key = (source, kind);
        if let Some(line) = self.lines.get_mut(&key) {
            if now.0.saturating_sub(line.logged_at.0) < PUBLIC_WINDOW_MILLIS {
                line.folded = line.folded.saturating_add(1);
                return None;
            }
            let folded = std::mem::take(&mut line.folded);
            line.logged_at = now;
            return Some(folded);
        }
        if self.lines.len() >= MAX_LOGGED_SOURCES {
            self.lines
                .retain(|_, line| now.0.saturating_sub(line.logged_at.0) < PUBLIC_WINDOW_MILLIS);
            if self.lines.len() >= MAX_LOGGED_SOURCES {
                return None;
            }
        }
        self.lines.insert(
            key,
            SourceLine {
                logged_at: now,
                folded: 0,
            },
        );
        Some(0)
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use super::*;

    fn source(port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)
    }

    #[test]
    fn one_line_per_source_and_reason_per_window_and_a_summary_counts_everything() {
        let mut diagnostics = IngressDiagnostics::default();
        let mut out = Vec::new();
        for millis in 0..20 {
            diagnostics.drop(
                IngressDropReason::SourceLimited,
                source(1),
                MonotonicMillis(millis),
                &mut out,
            );
        }
        diagnostics.drop(
            IngressDropReason::LobbyNotReady,
            source(1),
            MonotonicMillis(20),
            &mut out,
        );
        diagnostics.flush(MonotonicMillis(9_999), &mut out);
        assert_eq!(
            out,
            vec![
                IngressDiagnostic::Drop {
                    reason: IngressDropReason::SourceLimited,
                    source: source(1),
                    folded: 0,
                },
                IngressDiagnostic::Drop {
                    reason: IngressDropReason::LobbyNotReady,
                    source: source(1),
                    folded: 0,
                },
            ]
        );
        out.clear();
        diagnostics.drop(
            IngressDropReason::SourceLimited,
            source(1),
            MonotonicMillis(10_000),
            &mut out,
        );
        diagnostics.flush(MonotonicMillis(10_000), &mut out);
        assert_eq!(
            out,
            vec![
                IngressDiagnostic::Drop {
                    reason: IngressDropReason::SourceLimited,
                    source: source(1),
                    folded: 19,
                },
                IngressDiagnostic::Summary {
                    window_millis: 10_000,
                    suppressed: 0,
                    malformed: 0,
                    lobby_not_ready: 1,
                    source_limited: 21,
                    route_rejected: 0,
                    lobby_route_reuses: 0,
                    capability_first_seen: 0,
                    capability_connection_requests: 0,
                    sources: 1,
                },
            ]
        );
        out.clear();
        diagnostics.flush(MonotonicMillis(30_000), &mut out);
        assert!(out.is_empty(), "a quiet window reports nothing");
    }

    #[test]
    fn per_source_lines_are_bounded_while_the_summary_still_counts() {
        let mut diagnostics = IngressDiagnostics::default();
        let mut out = Vec::new();
        let sources = u16::try_from(MAX_LOGGED_SOURCES).unwrap() + 10;
        for port in 1..=sources {
            diagnostics.drop(
                IngressDropReason::SourceLimited,
                source(port),
                MonotonicMillis(0),
                &mut out,
            );
        }
        assert_eq!(out.len(), MAX_LOGGED_SOURCES);
        out.clear();
        diagnostics.flush(MonotonicMillis(PUBLIC_WINDOW_MILLIS), &mut out);
        let expected_drops = u64::from(sources);
        assert!(matches!(
            out.as_slice(),
            [IngressDiagnostic::Summary { source_limited, sources, .. }]
                if *source_limited == expected_drops && *sources == MAX_LOGGED_SOURCES
        ));
    }

    fn route_id(value: u128) -> RouteId {
        RouteId::new(value).unwrap()
    }

    fn worker(value: u128) -> WorkerId {
        WorkerId::new(value).unwrap()
    }

    fn forward(
        diagnostics: &mut IngressDiagnostics,
        worker_kind: Option<WorkerKind>,
        port: u16,
        payload: &[u8],
        millis: u64,
        out: &mut Vec<IngressDiagnostic>,
    ) {
        diagnostics.capability_forwarded(
            route_id(7),
            worker(2),
            worker_kind,
            source(port),
            payload,
            MonotonicMillis(millis),
            out,
        );
    }

    fn connection_request() -> Vec<u8> {
        vec![0_u8; NETCODE_CONNECTION_REQUEST_BYTES]
    }

    #[test]
    fn a_capability_route_names_each_new_source_once_and_counts_its_handshake_window() {
        let mut diagnostics = IngressDiagnostics::default();
        let mut out = Vec::new();
        let request = connection_request();
        for millis in 0..3 {
            forward(
                &mut diagnostics,
                Some(WorkerKind::Match),
                1,
                &request,
                millis,
                &mut out,
            );
        }
        diagnostics.capability_reply(route_id(7));
        diagnostics.capability_reply(route_id(8));
        forward(
            &mut diagnostics,
            Some(WorkerKind::Match),
            1,
            &[0x12; 40],
            3,
            &mut out,
        );
        assert_eq!(
            out,
            vec![IngressDiagnostic::CapabilityRouteFirstSeen {
                route_id: route_id(7),
                worker_id: worker(2),
                worker_kind: Some(WorkerKind::Match),
                source: source(1),
                payload_len: NETCODE_CONNECTION_REQUEST_BYTES,
                connection_request: true,
            }],
            "three requests and a payload from one source on one route make one line"
        );
        out.clear();
        diagnostics.flush(MonotonicMillis(PUBLIC_WINDOW_MILLIS), &mut out);
        let request_bytes = u64::try_from(NETCODE_CONNECTION_REQUEST_BYTES).unwrap();
        assert_eq!(
            out,
            vec![
                IngressDiagnostic::CapabilityRouteWindow {
                    route_id: route_id(7),
                    worker_id: worker(2),
                    window_millis: PUBLIC_WINDOW_MILLIS,
                    new_sources: 1,
                    forwarded: 4,
                    connection_requests: 3,
                    forwarded_bytes: 3 * request_bytes + 40,
                    replies: 1,
                },
                IngressDiagnostic::Summary {
                    window_millis: PUBLIC_WINDOW_MILLIS,
                    suppressed: 0,
                    malformed: 0,
                    lobby_not_ready: 0,
                    source_limited: 0,
                    route_rejected: 0,
                    lobby_route_reuses: 0,
                    capability_first_seen: 1,
                    capability_connection_requests: 3,
                    sources: 0,
                },
            ]
        );

        out.clear();
        forward(
            &mut diagnostics,
            Some(WorkerKind::Match),
            1,
            &[0x12; 40],
            PUBLIC_WINDOW_MILLIS + 1,
            &mut out,
        );
        diagnostics.capability_reply(route_id(7));
        diagnostics.flush(MonotonicMillis(3 * PUBLIC_WINDOW_MILLIS), &mut out);
        assert!(
            out.is_empty(),
            "established traffic from a known source is not reported: {out:?}"
        );

        forward(
            &mut diagnostics,
            Some(WorkerKind::Match),
            2,
            &[0x12; 40],
            3 * PUBLIC_WINDOW_MILLIS,
            &mut out,
        );
        assert_eq!(
            out,
            vec![IngressDiagnostic::CapabilityRouteFirstSeen {
                route_id: route_id(7),
                worker_id: worker(2),
                worker_kind: Some(WorkerKind::Match),
                source: source(2),
                payload_len: 40,
                connection_request: false,
            }],
            "a rebound source on the same route is a new first datagram"
        );
    }

    #[test]
    fn capability_source_memory_is_bounded_and_forgets_idle_pairs_first() {
        let mut diagnostics = IngressDiagnostics::default();
        let mut out = Vec::new();
        let pairs = u16::try_from(MAX_CAPABILITY_ROUTE_SOURCES).unwrap();
        for port in 1..=pairs {
            forward(&mut diagnostics, None, port, &[1], 0, &mut out);
        }
        assert_eq!(out.len(), MAX_CAPABILITY_ROUTE_SOURCES);
        out.clear();
        forward(&mut diagnostics, None, pairs + 1, &[1], 1, &mut out);
        assert!(
            out.is_empty(),
            "a full table of live pairs names no new pair"
        );
        forward(
            &mut diagnostics,
            None,
            pairs + 1,
            &[1],
            CAPABILITY_SOURCE_FORGET_MILLIS,
            &mut out,
        );
        assert_eq!(out.len(), 1, "idle pairs are evicted to make room");
        assert_eq!(diagnostics.capability_sources.len(), 1);
    }

    #[test]
    fn only_a_netcode_request_shaped_payload_is_classified_as_a_connection_request() {
        let mut request = vec![0_u8; NETCODE_CONNECTION_REQUEST_BYTES];
        assert!(looks_like_netcode_connection_request(&request));
        request[0] = 0x12;
        assert!(!looks_like_netcode_connection_request(&request));
        assert!(!looks_like_netcode_connection_request(&[0_u8; 300]));
    }
}
