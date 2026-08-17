# v2 M01 direct-UDP baseline (recorded at v1 M11 closeout)

This directory is the reproducible single-process baseline that v2 M01 compares its routed
multi-worker transport against. Every report here was produced by one dedicated server process
using Lightyear UDP directly on loopback — no route envelope, IPC, supervisor, or second worker.
M11 recorded it without implementing any v2 transport; the numbers below are the un-routed control.

## Provenance

| Field | Value |
|---|---|
| Source revision | `9131095` (clean tree; `source_dirty=false` in every report) |
| Protocol / registry / content | protocol 12; exact fingerprints recorded per report |
| Hardware / OS | Apple M3, 8 cores, 16 GB RAM, macOS 26.5.1 (aarch64) |
| Build profile | `dev` (optimized + debuginfo) via `cargo build --locked` |
| Sample count | one declared window per case (single-match runs end at the movement-verification exit after ~10 s; the idle cases exit at tick 601) |
| Tick rate | 60 Hz fixed simulation |

## Cases

| Case | Files | Instrumentation | Command shape |
|---|---|---|---|
| Idle endpoint | `idle-endpoint-off/` | closeout only | `BRAWLER_SERVER_EXIT_AFTER_TICKS=601 brawler-server --bind 127.0.0.1:5099 --mode wipeout` |
| Idle endpoint | `idle-endpoint-metrics/` | closeout + `process-metrics` | same, metrics-enabled binary |
| Wipeout 2v2 | `wipeout-2v2-metrics/` | closeout + `process-metrics` | `BRAWLER_NETWORK_CLIENT_COUNT=4 BRAWLER_NETWORK_PROFILE=local BRAWLER_NETWORK_GAME_MODE=wipeout ./scripts/network.sh` (headless) |
| Hot Zone 2v2 | `hot-zone-2v2-metrics/` | closeout + `process-metrics` | same with `BRAWLER_NETWORK_GAME_MODE=hot-zone` |
| Overhead control pair | `wipeout-local-off/`, `wipeout-local-metrics/` | closeout vs closeout+metrics | 2-client local Wipeout, both lanes |

The full impairment matrix (2-client local/typical/adverse × both modes, metrics-on) and all run
logs stay under `target/diagnostics/m11-final/` (untracked); their numbers are recorded in
`milestone-11.md` slice 6 evidence. Reproduce any case with the commands above plus
`BRAWLER_DIAGNOSTICS_DIR`/`BRAWLER_DIAGNOSTICS_SCENARIO_ID`.

## Instrumentation-profile decision for v2 M01

The overhead pair shows the `process-metrics` recorder is below run-to-run host variance for
fixed-tick timing (metrics-on is not slower in either the match or idle pairing), while only the
metrics-on lane records transport bytes/packets. v2 M01 must therefore reproduce the **metrics-on**
profile — server built with `--features server,process-metrics`, closeout diagnostics on, clients
on the standard `client` feature set — for like-for-like transport and timing comparison, and
re-verify the diagnostics-off control on its own hardware to quantify measurement overhead there.

## Measurements

Server fixed-tick percentiles, RTT/jitter from LinkStats, and transport totals per case:

| Case | ticks | tick p50/p95/max (µs) | RTT p50/p95 (µs) | jitter p50/p95 (µs) | bytes sent/recv | packets sent/recv |
|---|---|---|---|---|---|---|
| idle (off) | 602 | 570 / 994 / 6 280 | — | — | 0 / 0 | 0 / 0 |
| idle (metrics) | 602 | 556 / 933 / 7 546 | — | — | 0 / 0 | 0 / 0 |
| wipeout local 2c (off) | 576 | 514 / 977 / 12 624 | 22 564 / 32 807 | 3 207 / 5 036 | 0 / 0 | 0 / 0 |
| wipeout local 2c | 596 | 503 / 915 / 8 816 | 20 433 / 24 307 | 2 253 / 4 163 | 107 845 / 52 851 | 738 / 707 |
| wipeout typical 2c | 596 | 533 / 904 / 8 558 | 103 324 / 108 735 | 4 939 / 17 981 | 107 227 / 49 921 | 746 / 700 |
| wipeout adverse 2c | 588 | 537 / 979 / 8 202 | 146 331 / 153 705 | 5 711 / 31 092 | 106 569 / 48 404 | 751 / 681 |
| hot-zone local 2c | 585 | 541 / 1 007 / 9 100 | 25 864 / 34 512 | 4 660 / 9 776 | 113 266 / 53 506 | 746 / 712 |
| hot-zone typical 2c | 577 | 561 / 1 114 / 7 769 | 100 704 / 104 218 | 3 959 / 17 665 | 112 983 / 50 629 | 754 / 710 |
| hot-zone adverse 2c | 600 | 617 / 1 109 / 9 186 | 146 539 / 150 192 | 5 797 / 23 595 | 111 905 / 48 659 | 749 / 687 |
| wipeout 2v2 | 587 | 592 / 1 119 / 9 115 | 23 796 / 28 544 | 4 446 / 5 694 | 295 568 / 102 214 | 1 480 / 1 411 |
| hot-zone 2v2 | 582 | 604 / 1 233 / 16 161 | 27 028 / 31 011 | 4 710 / 5 909 | 310 023 / 104 526 | 1 491 / 1 422 |

Every case: `exit_category=clean-exit`, `entity_high_water=512`, `link_high_water` = 0 (idle),
2 (two clients), or 4 (2v2), `dropped_messages=0`, `error_count=0`, `first_divergence=none`.

Process, memory, and lifecycle envelope:

- Server resident memory (sampled at 2 Hz via `ps`): idle ≈ 38.7 MB steady; representative 2v2
  match high-water ≈ 42–43 MB (`server-rss.log` per case; the first 32 KB sample in each 2v2 file
  is the process before initial page-in).
- Cold start: the UDP endpoint binds ~0.3 ms after the first server log line; whole-process
  overhead beyond the 10.03 s idle tick window was 62 ms (off) and 1 356 ms (metrics build,
  first launch page-in) in single samples.
- Stop-to-exit: AppExit-to-closeout-report-written is 21–23 ms in both idle pairings; shutdown
  forwards `AppExit` to Lightyear `Stop` and closes the socket before the report is written.
- Closeout report size: 1 070–1 117 bytes per process across all cases and roles.
- Headless client fixed-tick (input sampling + terrain convergence, no render load): p50 52–65 µs,
  p95 115–168 µs, worst max 1 245 µs. Windowed frame pacing is a supervised-playtest observation
  (the client authority/network overlay exposes it) and is not claimed here.

## Terminal cleanup

Every report records `terminal_links=0` and bounded terminal entities, with no drops or errors;
`scripts/network.sh` validates report digests after each run and prints a terminal digest over all
validated reports at exit.

## What v2 M01 must not infer from this baseline

These are un-routed, loopback, single-worker numbers from one machine and one declared window per
case. They set the control envelope — tick percentiles, entity/link high-water, RTT/jitter under
each impairment profile, transport volume, report size, idle/match memory — not v2 budgets. M01
derives routed thresholds during its own specification review, per the accepted architecture.
