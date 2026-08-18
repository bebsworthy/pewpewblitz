# M01 paired CPU and gameplay-bandwidth evidence

The canonical lane is:

```sh
just network-paired-evidence 3 90 wipeout
```

Use `just network-paired-evidence 1 90 wipeout` for a bounded local smoke, or select
`hot-zone` as the fourth argument. The harness runs the existing direct-UDP and routed
supervisor launchers sequentially, with `verification` rules and the same mode, source tree,
feature builds, and host. It samples every Brawler process in each launcher tree at 10 Hz and
records per-process CPU/RSS samples in the JSON artifact.

The hard comparisons are intentionally limited to the common gameplay boundary:

* direct `server.closeout` `transport_bytes_received/sent`;
* routed supervisor `traffic.match_inner_ingress/egress.bytes`, excluding lobby
  authentication/allocation traffic.

Ingress and egress are checked independently and the aggregate is checked as well. Routed
aggregate CPU time is checked against direct aggregate CPU time only when every observed process
has at least two CPU samples and both sides have a non-zero direct baseline; the limit is 20%.
Inner gameplay bytes have a 10% regression limit. An unavailable or incomparable series remains
`unsupported` and cannot be presented as a pass.

Routed public envelope bytes are reported using the exact `42 * datagrams` overhead formula.
Framed IPC bytes are reported separately, with their mixed packet/control scope. Neither is
compared with direct gameplay bytes.

The parser and gate math are covered without launching processes:

```sh
just test-paired-evidence
```

The output schema is `brawler-paired-evidence-v1`; summaries are written to
`target/paired-evidence-<UTC timestamp>.json`.

## Current measured result

The latest corrected one-pair Wipeout run at
`target/paired-evidence-20260818T220108Z.json` proved the same exact 3,600 authoritative ticks and
identical nonzero protocol/content fingerprints on both sides. Direct egress was 696,924 B and
routed match-worker egress was 782,718 B (+12.31%), so the selected 10% directional target failed.
Ingress was +0.35% and total transport was +7.92%; raw aggregate CPU was +17.29%, but CPU remains
unsupported because its samples are not yet correlated to the exact common window. An earlier
corrected run measured +13.31% egress, so the miss is reproducible but variable. Review found no
packet duplication or scenario-identity mismatch. The
most plausible contributors are different pre-Active replication warmup, required routed MTU
1,133 versus direct 1,200, and transport counters sampled at an app-frame boundary around fixed
lifecycle ticks. This result is retained as a failed measurement, not relaxed or relabeled.

On 2026-08-19 the user accepted the topology for development use and deferred performance tuning
and exhaustive production evidence to M09. A future hardening run should add an identical
benchmark-only pre-ready settle interval, A/B direct MTU 1,133, and capture transport counters at
one explicit boundary before attributing the full delta to routed transport.
