# Outcome
An operator sets how many concurrent matches and lobby sessions a host runs, within code ceilings sized from measurement, and a player who queues or starts Practice while the host is full sees that the server is busy.

# Found (2026-11-21)
## Capacity limits today (all code literals)
| Limit | Value | Where |
|---|---|---|
| Worker processes (lobby + matches) | 5 | `MAX_WORKERS`, `packages/brawler-routing/src/limits.rs:40` |
| Active matches advertised to the lobby | 4 | `active_matches: 4`, `packages/brawler-routing/src/bin/supervisor.rs:84` |
| Free match slots | `min(manifest, MAX_WORKERS-1) - running` | `packages/brawler-routing/src/runtime/capacity.rs:74` |
| Authenticated lobby sessions | 32 | `max_authenticated_sessions: 32` (`supervisor.rs:82`), capped by `MAX_AUTHENTICATED_LOBBY_SESSIONS = MAX_QUEUE_TICKETS = 32` (`src/server/lobby/mod.rs:63`, `src/lobby/queue.rs:10`) |
| Outstanding allocations | 2 | `supervisor.rs:83` |
| Routes and capabilities | 64 and 128 | `MAX_ACTIVE_ROUTES`, `MAX_CAPABILITIES` in `limits.rs` |
| Match seats tracked for rejoin | `MAX_WORKERS * MAX_PARTICIPANTS` | `src/server/lobby/rejoin.rs` |

So one host serves at most 32 online players and 4 concurrent matches (at most 24 players in matches).

## What players see when the host is full
- Lobby full: "The server lobby is full" with Retry (`packages/brawler-client/src/flow/reducer.rs:749`).
- Practice with no free slot: "The server has no free match capacity right now." (`flow/reducer/match_flow.rs:659`).
- Queue with no free slot: formation silently waits (`src/server/lobby/mod.rs:2024`). Whether the queue screen says why is unverified.
- Separately, the lobby forms only one match at a time. Filed as BRL-d8pv.

## Measurement (2026-11-21, Apple M6, release server and supervisor, debug headless clients)
Script and samples are attached to this ticket. Four Practice matches ran concurrently (heist-3v3, hot-zone-3v3 and wipeout-3v3 with five bots each, and zombie-3p), started 4 s apart, about 30 s each, sampled at 2 Hz:

| Role | RSS max | CPU mean | CPU p95 (one core) |
|---|---|---|---|
| Supervisor | 7.4 MiB | 0.8% | 1.9% |
| Lobby (idle / load) | 30.3 / 31.8 MiB | 5.5% / 5.7% | 9.2% / 13.4% |
| Match, each | ≤ 46.5 MiB | 13.0% | 24.7% |
| Whole host, 4 matches | 215 MiB | 35% | peak 70% |

V2 baselines were match about 42 MiB and lobby about 30 MiB (`docs/implementation/v2/evidence/m01-routed/README.md`), so memory has barely grown.

**Reading:**
- Memory is not the constraint: 4 GB fits about 60 matches.
- CPU is the constraint. A shared VPS vCPU is plausibly 2–3× slower than an M6 core, which would put one 3v3 match with bots at about 0.3–0.75 vCPU at p95. So a 2 vCPU host would run roughly 3–6 matches.
- Caveats: Practice smokes are short and early-match, and the numbers are from macOS. Linux on the real host, with full-length matches, is the measurement that counts.

# Scope
1. Operator configuration: supervisor flags `--max-matches` and `--max-lobby-sessions` (and the entrypoint environment from BRL-jjwa), validated `1..=ceiling`, feeding the lobby manifest instead of the literals.
2. Raise the code ceilings to bounded values that keep every dependent table bounded, for example `MAX_WORKERS` 17 (16 matches) and lobby sessions 128, with `MAX_QUEUE_TICKETS`, routes and capabilities scaled to match and checked by tests. Pick exact ceilings at implementation and record them. Defaults stay at today's values.
3. The queue screen tells the player when formation is waiting on server capacity. Verify first; implement only if it's silent.
4. A repeatable load procedure: promote the attached script to `scripts/capacity.sh` (or document it in `docs/19-server-operations.md`), so the operator can rerun it on the real host, including inside the BRL-jjwa image.

# Non-goals
Concurrent formations (BRL-d8pv); multiple hosts; autoscaling.

# Acceptance
- With `--max-matches 2` a third Practice start gets `CapacityUnavailable`.
- Values above the ceiling are refused at startup.
- Table-bound tests cover the raised ceilings.
- The capacity procedure runs on the BRL-kc93 host, and its numbers are recorded here and in BRL-kc93.

# Verification
`just lint`, `just test`, `cargo test --test routed_process`, and the capacity procedure on macOS and on the Linux host.
