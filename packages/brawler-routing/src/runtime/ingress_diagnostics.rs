//! Rate-limited public-ingress diagnostics (BRL-2nsg).
//!
//! The owner loop drops public datagrams without replying (unknown, limited, or not-yet-routable
//! sources). Counters alone left a device that could not reconnect indistinguishable from a
//! network fault, so each drop and each suspicious lobby-route reuse is reported as a bounded
//! diagnostic: at most one line per source and kind per window, plus one summary per window that
//! counts everything, including the lines that were folded into it.

use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
};

use crate::{MonotonicMillis, PUBLIC_WINDOW_MILLIS, RouteId, RoutingErrorCategory};

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
/// Size of a Netcode 1.02 connection request (prefix, version, protocol, expiry, nonce, token).
/// The supervisor never parses Netcode; this shape check only classifies a diagnostic line.
const NETCODE_CONNECTION_REQUEST_BYTES: usize = 1 + 13 + 8 + 8 + 24 + 1_024;

/// Whether an opaque lobby payload has the shape of a Netcode connection request. Every other
/// Netcode packet carries a nonzero sequence-length nibble in its prefix byte, so prefix 0 with
/// the exact request size identifies one.
#[must_use]
pub(super) fn looks_like_connection_request(payload: &[u8]) -> bool {
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
    /// Every drop and reuse of the last window, including the ones folded out of the lines.
    Summary {
        window_millis: u64,
        suppressed: u64,
        malformed: u64,
        lobby_not_ready: u64,
        source_limited: u64,
        route_rejected: u64,
        lobby_route_reuses: u64,
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

/// Bounded rate limiter for [`IngressDiagnostic`]s.
#[derive(Debug, Default)]
pub(super) struct IngressDiagnostics {
    lines: HashMap<(SocketAddr, DiagnosticKind), SourceLine>,
    counts: Counts,
    window_sources: HashSet<SocketAddr>,
    window_started: Option<MonotonicMillis>,
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

    /// Close the window once it is a full window old and report it if anything happened.
    pub(super) fn flush(&mut self, now: MonotonicMillis, out: &mut Vec<IngressDiagnostic>) {
        let Some(started) = self.window_started else {
            return;
        };
        let window_millis = now.0.saturating_sub(started.0);
        if window_millis < PUBLIC_WINDOW_MILLIS {
            return;
        }
        let counts = std::mem::take(&mut self.counts);
        out.push(IngressDiagnostic::Summary {
            window_millis,
            suppressed: counts.suppressed,
            malformed: counts.malformed,
            lobby_not_ready: counts.lobby_not_ready,
            source_limited: counts.source_limited,
            route_rejected: counts.route_rejected,
            lobby_route_reuses: counts.lobby_route_reuses,
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

    #[test]
    fn only_a_netcode_request_shaped_payload_is_classified_as_a_connection_request() {
        let mut request = vec![0_u8; NETCODE_CONNECTION_REQUEST_BYTES];
        assert!(looks_like_connection_request(&request));
        request[0] = 0x12;
        assert!(!looks_like_connection_request(&request));
        assert!(!looks_like_connection_request(&[0_u8; 300]));
    }
}
