//! Bounded routing-owner telemetry.
//!
//! These counters intentionally describe only bytes observed at a named boundary.  Public
//! counters include the UDP datagram/envelope bytes, inner counters include the opaque Netcode
//! payload, and IPC counters include the four-byte framed-stream prefix.  No counter infers
//! bytes from a paired direct-UDP run or from a packet capture that the owner did not perform.

use std::time::Duration;

/// A compact logarithmic latency histogram.  Buckets are inclusive upper bounds in microseconds:
/// bucket 0 is `[0, 1]`, bucket 1 is `(1, 2]`, bucket 2 is `(2, 4]`, and so on.  The final bucket
/// is an overflow bucket for values above `2^30` microseconds.  The exact sample count, sum,
/// minimum, and maximum remain available for diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LatencyHistogram {
    count: u64,
    sum_nanos: u128,
    min_nanos: u64,
    max_nanos: u64,
    buckets: [u64; 32],
}

impl Default for LatencyHistogram {
    fn default() -> Self {
        Self {
            count: 0,
            sum_nanos: 0,
            min_nanos: u64::MAX,
            max_nanos: 0,
            buckets: [0; 32],
        }
    }
}

impl LatencyHistogram {
    /// Add one monotonic elapsed duration to the bounded histogram.
    pub fn observe(&mut self, elapsed: Duration) {
        let nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        self.observe_nanos(nanos);
    }

    /// Add one nanosecond value. This is public for deterministic tests and alternate monotonic
    /// owners; production code should normally use [`Self::observe`].
    pub fn observe_nanos(&mut self, nanos: u64) {
        self.count = self.count.saturating_add(1);
        self.sum_nanos = self.sum_nanos.saturating_add(u128::from(nanos));
        self.min_nanos = self.min_nanos.min(nanos);
        self.max_nanos = self.max_nanos.max(nanos);
        self.buckets[bucket_index(nanos)] = self.buckets[bucket_index(nanos)].saturating_add(1);
    }

    #[must_use]
    pub const fn count(&self) -> u64 {
        self.count
    }

    #[must_use]
    pub const fn sum_nanos(&self) -> u128 {
        self.sum_nanos
    }

    #[must_use]
    pub const fn min_nanos(&self) -> Option<u64> {
        if self.count == 0 {
            None
        } else {
            Some(self.min_nanos)
        }
    }

    #[must_use]
    pub const fn max_nanos(&self) -> Option<u64> {
        if self.count == 0 {
            None
        } else {
            Some(self.max_nanos)
        }
    }

    /// Return the bucket upper bound in microseconds for a percentile in `0..=100`.
    #[must_use]
    pub fn percentile_us(&self, percentile: u8) -> Option<u64> {
        if self.count == 0 {
            return None;
        }
        let percentile = u64::from(percentile.min(100));
        let rank = self
            .count
            .saturating_mul(percentile)
            .saturating_add(99)
            .checked_div(100)
            .unwrap_or(self.count)
            .max(1);
        let mut observed = 0_u64;
        for (index, count) in self.buckets.iter().copied().enumerate() {
            observed = observed.saturating_add(count);
            if observed >= rank {
                return Some(bucket_upper_us(index));
            }
        }
        Some(bucket_upper_us(self.buckets.len() - 1))
    }

    #[must_use]
    pub fn p50_us(&self) -> Option<u64> {
        self.percentile_us(50)
    }

    #[must_use]
    pub fn p95_us(&self) -> Option<u64> {
        self.percentile_us(95)
    }

    #[must_use]
    pub fn p99_us(&self) -> Option<u64> {
        self.percentile_us(99)
    }

    /// Return compact bucket counts for machine-readable evidence. The final bucket is the
    /// overflow bucket and has an upper bound of `u64::MAX` microseconds.
    #[must_use]
    pub fn bucket_counts(&self) -> &[u64; 32] {
        &self.buckets
    }
}

const fn bucket_index(nanos: u64) -> usize {
    // Round up to microseconds so a sub-microsecond observation remains visible as 1 us.
    let micros = nanos.saturating_add(999) / 1_000;
    if micros <= 1 {
        return 0;
    }
    // Select the smallest inclusive power-of-two upper bound.  In particular, an exact power of
    // two stays in its own bucket (`2 -> 2`, `4 -> 4`) instead of being rounded into the next one.
    let floor = (u64::BITS - micros.leading_zeros() - 1) as usize;
    let index = if micros.is_power_of_two() {
        floor
    } else {
        floor + 1
    };
    if index > 30 { 31 } else { index }
}

const fn bucket_upper_us(index: usize) -> u64 {
    if index == 0 {
        1
    } else if index >= 31 {
        u64::MAX
    } else {
        1_u64 << index
    }
}

/// Exact counters for one directional boundary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TrafficCounters {
    pub datagrams: u64,
    pub frames: u64,
    pub bytes: u64,
}

impl TrafficCounters {
    /// Count one datagram. `bytes` is the exact boundary byte count, including its protocol
    /// overhead when the boundary is an encoded envelope or framed IPC stream.
    pub fn observe_datagram(&mut self, bytes: usize) {
        self.datagrams = self.datagrams.saturating_add(1);
        self.bytes = self
            .bytes
            .saturating_add(u64::try_from(bytes).unwrap_or(u64::MAX));
    }

    /// Count one logical frame without changing datagram count or byte count.
    pub fn observe_frame(&mut self) {
        self.frames = self.frames.saturating_add(1);
    }

    /// Count one framed-stream record with its exact four-byte prefix included.
    pub fn observe_ipc_frame(&mut self, framed_bytes: usize) {
        self.frames = self.frames.saturating_add(1);
        self.bytes = self
            .bytes
            .saturating_add(u64::try_from(framed_bytes).unwrap_or(u64::MAX));
    }

    /// Count bytes read from a stream and the complete records decoded from that read. Partial
    /// frames therefore remain represented in `bytes` even though they do not increment frames.
    pub fn observe_ipc_read(&mut self, bytes: usize, frames: usize) {
        self.frames = self
            .frames
            .saturating_add(u64::try_from(frames).unwrap_or(u64::MAX));
        self.bytes = self
            .bytes
            .saturating_add(u64::try_from(bytes).unwrap_or(u64::MAX));
    }
}

/// Supervisor-owned routing telemetry exposed by the metrics snapshot.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RoutingMetrics {
    pub public_ingress: TrafficCounters,
    pub public_egress: TrafficCounters,
    pub inner_ingress: TrafficCounters,
    pub inner_egress: TrafficCounters,
    /// Inner packets admitted to match workers only. Lobby authentication and allocation traffic
    /// is excluded so paired gameplay comparisons use the same boundary as direct UDP.
    pub match_inner_ingress: TrafficCounters,
    /// Inner packets emitted by match workers only; lobby grant and keepalive traffic is excluded.
    pub match_inner_egress: TrafficCounters,
    pub ipc_to_worker: TrafficCounters,
    pub ipc_from_worker: TrafficCounters,
    /// From the supervisor's public UDP receive return to successful packet-IPC queue enqueue.
    pub public_receive_to_packet_ipc_enqueue: LatencyHistogram,
    /// From a decoded worker BRPK record to successful public UDP send. Queue wait is included.
    pub worker_packet_to_public_send: LatencyHistogram,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ten_thousand_deterministic_samples_have_bounded_percentiles() {
        let mut histogram = LatencyHistogram::default();
        for sample in 0..10_000_u64 {
            histogram.observe_nanos((sample % 2_048) * 1_000);
        }
        assert_eq!(histogram.count(), 10_000);
        assert_eq!(histogram.min_nanos(), Some(0));
        assert_eq!(histogram.max_nanos(), Some(2_047_000));
        assert!(histogram.p50_us().is_some_and(|value| value <= 1_024));
        assert!(histogram.p95_us().is_some_and(|value| value <= 2_048));
        assert!(histogram.p99_us().is_some_and(|value| value <= 2_048));
        assert!(histogram.p50_us() <= histogram.p95_us());
        assert!(histogram.p95_us() <= histogram.p99_us());
    }

    #[test]
    fn exact_directional_accounting_keeps_overheads_at_their_boundary() {
        let mut public = TrafficCounters::default();
        public.observe_datagram(1_200);
        public.observe_frame();
        assert_eq!(
            public,
            TrafficCounters {
                datagrams: 1,
                frames: 1,
                bytes: 1_200
            }
        );

        let mut inner = TrafficCounters::default();
        inner.observe_datagram(1_158);
        inner.observe_frame();
        assert_eq!(
            inner,
            TrafficCounters {
                datagrams: 1,
                frames: 1,
                bytes: 1_158
            }
        );

        let mut ipc = TrafficCounters::default();
        ipc.observe_ipc_frame(1_220);
        assert_eq!(
            ipc,
            TrafficCounters {
                datagrams: 0,
                frames: 1,
                bytes: 1_220
            }
        );
        ipc.observe_ipc_read(4, 0);
        assert_eq!(ipc.bytes, 1_224);
        assert_eq!(ipc.frames, 1);
    }

    #[test]
    fn histogram_bucket_boundaries_are_inclusive_and_unambiguous() {
        let mut histogram = LatencyHistogram::default();
        for micros in [0, 1, 2, 3, 4, 5, 1_024, 1_025] {
            histogram.observe_nanos(micros * 1_000);
        }
        assert_eq!(histogram.bucket_counts()[0], 2); // [0, 1]
        assert_eq!(histogram.bucket_counts()[1], 1); // (1, 2]
        assert_eq!(histogram.bucket_counts()[2], 2); // (2, 4]
        assert_eq!(histogram.bucket_counts()[10], 1); // (512, 1,024]
        assert_eq!(histogram.bucket_counts()[11], 1); // (1,024, 2,048]
        assert_eq!(histogram.bucket_counts().iter().sum::<u64>(), 8);
    }
}
