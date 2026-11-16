Reviewed 2026-11-15 against HEAD 3437c0d8: still valid (unexplained, not reproduced); added code refs for every supervisor kill path, a unified-log check, and a diagnostic step (log the exit signal).

# Found
Device check 2026-11-15 (Pixel 8a), HEAD b97a8888, server on 0.0.0.0:5000 with RUST_LOG=info. Seen once. (HEAD 3437c0d8 / BRL-4zx2 later changed src/server/seat_hold.rs to also remove `ControlledSend` on hold; it does not explain a signal exit.)

# What happened
- Wipeout 1v1 match worker pid 63370 (worker 88618531226783727793869621698450704611), started 17:24:29Z. The headless opponent left, and its seat was held at 17:24:43 (bot controls it).
- Phone force-stopped at about 17:25:01Z. At 17:25:11, about when the phone's link timeout would hold its seat too, the supervisor logged `worker reaped ... success=false code=None`. There was no result-received, exit-received, stop-requested or "failed" line, and the worker printed nothing (no panic). The lobby then released both seats, so the phone's rejoin found no match (Dashboard).
- No crash report in ~/Library/Logs/DiagnosticReports. The same steps worked at 17:27:25 and later: both seats held, rejoin OK.
- The adb daemon was crashing repeatedly on the Mac during the session (adb .ips reports), so an external kill cannot be ruled out.

# Review findings (2026-11-15)
- Every supervisor-initiated kill logs first: `fail_worker` (packages/brawler-routing/src/supervisor/process.rs:767, kill at ~:1170) emits `LifecycleEvent::Failed` -> "worker failed" line; `force_due_stop` (~:1195) emits `ForcedStop`; only `Drop for ProcessSupervisor` (~:1630) kills silently, and the supervisor kept running. Reporting: packages/brawler-routing/src/runtime/reporting.rs:42-100. So the supervisor most likely did not send the signal.
- A worker panic would print: the panic hook (src/diagnostics/failure.rs:164-174, installed at src/server/mod.rs:1478-1482) writes a failure record and chains to the default hook; worker stderr is inherited (process.rs:600-606, policy-dependent). A silent signal points at SIGKILL/SIGSEGV/external kill.
- The reaped line only logs `status.code` (`None` for a signal), losing the signal number (reporting.rs:61-64).
- macOS unified log, 18:24:30–18:25:20 local (UTC+1): no entry for processID 63370, no kernel/memorystatus/"killed" line at 18:25:09–18:25:12. Inconclusive (not a jetsam kill as far as the log shows).

# Plan
1. Diagnostics first: record the terminating signal in `ChildReaped` (`std::os::unix::process::ExitStatusExt::signal()`) and log it as `signal=` (routing crate, with a unit test). Small, worth doing even if no repro.
2. Reproduce on the 1v1 path: opponent leaves (seat held, bot controls), then the second link times out (second hold) while the first hold is active, with a supervised worker. Automate via the rejoin harness (`just rejoin-e2e`, scripts/e2e-rejoin.sh) extended to two sequential link losses, and in tests/network/match_rejoin.rs (two held seats at once). Loop it ~20 times.
3. If reproduced: identify the crash (signal number, `lldb` attach or core dump), fix with a test. If not reproduced after the loop with signal logging in place: record "external kill (adb crash storm) most likely" and close.

# Acceptance
- Reproduce, or rule out an external cause, by running the 1v1 held-seat + second link-loss path with a supervised worker. If it is a worker crash, fix it with a test.
- Supervisor reaped line reports the signal; `just test`, `just lint`, `just rejoin-e2e` pass.

Evidence (/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/vc): server.log lines 36-50 (17:24:29-17:25:11), f-launch.jpg, f-rejoin.jpg, f-logcat.txt; server-4zx2.log (post-4zx2 runs) has no `success=false`. Scratchpad is temporary; copy what matters into docs/tasks/BRL-q73k-*/ if needed.


# Resolution 2026-11-16: supervisor bug, reproduced and fixed
**Cause:** when a seat's link times out, the supervisor expires that peer's route and sends `PeerClose` to the match worker. The worker can already have written packets for that peer. When one arrives, `SupervisorCore::accept_worker_packet` finds no route and returns `ManifestIdentity`. `read_worker_packets` treats that as a worker fault, so `routing_io.rs:142` calls `cleanup_worker` → `fail_worker`, which SIGKILLs the match. Its `Failed` event was discarded (`let _ =`), so the log showed only `reaped success=false code=None`. The same race can hit lobby workers on idle route expiry.

**Repro:** a real supervisor with lobby and Wipeout 1v1 worker processes. A leaves (seat 1 held with a bot); after D = 0-19 s, B is `kill -9`ed (seat 2 held). Before the fix: 6 of 31 valid iterations ended with `signal=Some(9)`. Instrumented run z14 showed `accept_worker_packet refused category=ManifestIdentity` → `cleanup_worker kills live worker caller=routing_io.rs:142` → reaped `signal=Some(9)`, 10.08 s after B was killed. After the fix: 20 of 20 had both seats held and clean exits.

**Fix (packages/brawler-routing, no protocol or IPC change):**
- The core keeps routes torn down while their worker lives (`retired_routes`, capped at `max_routes`, cleared on worker cleanup). A packet for a retired route with the same route, worker and peer is `RouteExpired` and is dropped, not counted as a worker fault.
- `cleanup_worker` keeps `fail_worker`'s events and `poll_once` publishes them, so a runtime cleanup kill always logs `worker failed` before the reap.
- The reaped line now reports `signal=` (`ExitStatusExt::signal()`).
- `scripts/network-product-match.sh` passes `--data-directory` (default `target/e2e/server`) so e2e runs stop writing the tracked `data/server/`. Other scripts still use the default; not changed.

**Tests:** `packets_racing_a_route_teardown_are_expired_not_identity_faults`, `retired_routes_are_bounded_by_the_route_limit`, `worker_packets_racing_a_route_expiry_are_dropped_without_failing_the_worker`, `runtime_cleanup_kill_publishes_failed_before_the_reap` (fake-worker `malformed-packet` mode), plus signal reporting tests. `just lint`, `just test` (2464 tests) and `just rejoin-e2e` pass.

The 2026-11-16 06:49Z lobby reap on the dev server is unrelated: `worker failed category=HeartbeatTimeout` after the Mac woke from hibernation, which is expected.
