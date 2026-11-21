# Outcome
The owner gets an alert within minutes when the public server is down or its workers keep crashing, and server logs are bounded and readable.

# Found (2026-11-21)
- **No live health surface.** The supervisor prints `brawler-supervisor listening on …` once (`packages/brawler-routing/src/bin/supervisor.rs:111`). `--metrics-file` is written only **after** shutdown (`supervisor.rs:114`, `write_metrics` at `:154`), so it can't serve as a health signal. Workers write a readiness file only under `BRAWLER_SERVER_READY_FILE` (`src/server/mod.rs:516`).
- **Useful log lines already exist:**
  - lifecycle: `worker spawned|ready|stop-sent|cleaned|exit-received`, `worker failed … category=` (`packages/brawler-routing/src/runtime/reporting.rs:149`, `:159`);
  - ingress: `ingress dropped … source=` and a per-window `ingress summary` (`reporting.rs:44-95`).
- **Volume is tiny.** About 22 KB of supervisor and worker stderr for four 30 s Practice matches (`target/capacity/run2/supervisor.log`, BRL-fdbz), roughly 5 KB per match, so 14 days of logs fit easily.
- **Logs carry ANSI colour codes even when redirected to a file** (Bevy `LogPlugin` default formatting), which makes container logs noisy to read and grep.
- Supervisor log lines include client IP addresses, so retention is personal-data retention: 14 days (BRL-tysh).
- Panics go to stderr. `BRAWLER_FAILURE_REPORT` appends structured failure records to a file (`src/diagnostics/failure.rs:140`).

# Decisions (owner-approved 2026-11-21)
1. **Status file.** The supervisor atomically rewrites a small JSON status file in the data directory every 5 s: `updated_at`, lobby ready, running match workers, free match slots, worker failures in the last hour, and the ingress summary counters. No new socket.
2. **Health endpoint without a new port.** `brawler-token-service` (BRL-z277) serves `GET /healthz` through Caddy on 443. It returns 200 when the status file is fresh (< 15 s) and the lobby is ready, and 503 otherwise, with a body that says why but no secrets or IPs. The compose healthcheck for the supervisor container checks the same file locally.
3. **Alerting.** A free external HTTPS uptime monitor (for example UptimeRobot or Healthchecks.io; choose at implementation and record it) polls `/healthz` every 1–5 min and emails the owner. `/healthz` also returns 503 when worker failures reach 3 or more in the last hour, so a crash loop alerts too.
4. **Logs:**
   - Docker `local` log driver with size rotation (for example 5 × 20 MB per container), and a 14-day retention prune documented in `docs/19-server-operations.md`.
   - Colour off when stderr isn't a terminal, or when `NO_COLOR` is set, in both the supervisor and the worker log setup.
   - `BRAWLER_FAILURE_REPORT` points into the data volume in the image, so crash records survive restarts; it's bounded by rotation in the same prune.
5. No metrics backend (Prometheus or Grafana) at launch. The status file and logs are enough for one host; revisit only with a demonstrated need.

# Scope
Decisions 1–5, plus tests: the status-file writer is atomic and bounded; `/healthz` returns 200, 503 for stale status and 503 for a crash loop; logs have no ANSI codes when stderr isn't a TTY.

# Acceptance
- Stopping the supervisor container makes `/healthz` return 503 within 15 s and the external monitor emails within its interval (recorded).
- Container logs rotate at the configured size.
- `docker logs` output has no escape codes.

# Verification
`just lint`, `just test`, the compose stack locally, and the external monitor set up against the real domain (recorded).
