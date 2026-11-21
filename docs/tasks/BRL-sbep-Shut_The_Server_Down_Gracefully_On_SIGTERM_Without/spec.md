# Outcome
Stopping the server the way a container runtime does (SIGTERM) runs the existing graceful shutdown, and no worker process outlives the supervisor.

# Found (2026-11-21)
- **SIGTERM bypasses graceful shutdown.** The supervisor registers `ctrlc::set_handler` → `runtime.stop_handle().request()` (`packages/brawler-routing/src/bin/supervisor.rs:109`). `ctrlc` 3.5.2 handles only SIGINT unless its `termination` feature is on (`ctrlc-3.5.2/src/platform/unix/mod.rs:98`). `cargo tree -e features -i ctrlc` shows only `default` for both the supervisor and the Bevy-side graph.
- **In a container this is worse than an abrupt kill.** As PID 1 under Linux, a process ignores signals it has no handler for. `docker stop` sends SIGTERM to the supervisor, which ignores it; after the stop timeout (default 10 s) SIGKILL tears down the whole namespace. No ordered Stop reaches the workers and no `--metrics-file` is written.
- **The graceful path is bounded and fits a container stop timeout.** `ProcessSupervisorConfig` (`supervisor/process.rs:92`) has `graceful_stop` 2 s, `forced_reap` 1 s and `shutdown_deadline` 5 s. Shutdown sends Stop, waits, force-reaps, and never forgets an unreaped child (`process.rs:948`, `:1207`). That ends running matches; draining them first is BRL-pweq, not this ticket.
- **Workers already die with the supervisor.** A worker treats an unsolicited control-stream EOF as failure and exits with an error (`src/server/worker.rs:49`, `:1096`). Packet-stream EOF triggers `RoutedWorkerFailure` (`src/server/routed_worker.rs`). When the supervisor dies, even by SIGKILL, the kernel closes its socket ends, so workers should exit within one poll. **No test covers supervisor death:** `tests/routed_process.rs` kills workers (`:368`, `:449`) but never the supervisor.
- Workers don't install Bevy's `TerminalCtrlCHandlerPlugin` (asserted at `src/server/admission.rs:1449`, `:1936`), so a terminal Ctrl-C reaching the whole process group doesn't race the supervisor's ordered stop. Keep it that way.

# Decisions
- Enable `ctrlc`'s `termination` feature in `brawler-routing`, so SIGINT, SIGTERM and SIGHUP all request the existing graceful stop. No new signal code (`unsafe_code = "deny"` stays).
- Run the supervisor in the container under `tini` (`docker run --init` or compose `init: true`) rather than as bare PID 1, so stray grandchildren are reaped and signals are forwarded with standard semantics. The supervisor does not become an init.
- Container stop timeout ≥ 10 s (the 5 s shutdown deadline plus margin), recorded in `docs/19-server-operations.md`. A longer drain belongs to BRL-pweq, which then raises it.

# Scope
1. Cargo feature change plus a short comment saying why.
2. A real-process test in `tests/routed_process.rs`: start the runtime as a child process (the real `brawler-supervisor` binary), send SIGTERM, and assert ordered worker stop (`worker stopped forced=false`), exit status 0, and no remaining worker PIDs.
3. A real-process test: SIGKILL the supervisor and assert every worker PID is gone within a bounded time (≤ 2 s) by polling process existence. No wall-clock sleeps as the assertion, only a bounded poll.
4. A SIGTERM step in `docs/19-server-operations.md` (stop procedure, expected log lines).

# Non-goals
Match drain before stop (BRL-pweq); restart policy (BRL-n7c8); changing the shutdown bounds.

# Acceptance
Both new tests pass on macOS and in the Linux CI job from BRL-jjwa; `docker stop` on the image from BRL-jjwa logs ordered worker stops and exits within the deadline.

# Verification
`cargo test -p brawler-routing`; `cargo test --test routed_process` with the features it needs (see BRL-shwv for that target's database hygiene); `just lint`; a manual `docker stop` run recorded here once BRL-jjwa exists.
