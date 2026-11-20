# Found (2026-11-19)

`just test` on main (after 302f37ec BRL-stws and the BRL-q5sm/855z cherry-pick), load average ~16: `_test-server` failed `diagnostics::tests::exit_frame_report_observes_terminal_counts_after_the_shutdown_chain` (`src/diagnostics/tests.rs`) with only "Encountered a panic in system `bevy_app::main_schedule::FixedMain::run_fixed_main`" lines; the panic message was not in the captured log. Five isolated runs and three full `--features server --lib` runs right after all passed (550/550).

# Notes
- The test uses `MinimalPlugins` (real time), so under load `run_fixed_main_schedule` may run several or zero fixed steps per `app.update()`; something in `ProcessDiagnosticsPlugin` fixed systems likely assumes otherwise. BRL-stws edited this file (other tests) but not this test.
- Reproduce with `RUST_BACKTRACE=1` and `--nocapture` in a loop under CPU load (e.g. `stress` or a parallel build), capture the panic, then drive time explicitly (`TimeUpdateStrategy::ManualDuration` or fixed-time advance per AGENTS.md "never wait on wall-clock").

# Acceptance
- Root cause recorded; the test advances time deterministically and passes 50 consecutive loaded runs.


# Root cause (2026-11-20)
- Captured with `RUST_BACKTRACE=1 --nocapture` (`--no-default-features --features server`): `Encountered an error in system brawler::diagnostics::process::common_window::observe_common_window_fixed: Parameter Res<'_, SimulationTick> failed validation: Resource does not exist`, which then shows up as `FixedMain::run_fixed_main` / `run_fixed_main_schedule` / `Main::run_main`. These are the same lines the original failure printed.
- How it happens: `MinimalPlugins` uses real time. The first `app.update()` has zero delta. If more than one fixed timestep (about 15.6 ms) of wall time passes before the second update, a fixed step runs. With the `server` feature, `ProcessDiagnosticsPlugin` installs `observe_common_window_fixed` in `FixedPostUpdate`, and that system needs `Res<SimulationTick>`. The test never inserted that resource, so the system panicked. On an unloaded host the gap between the two updates is well under one timestep, which is why the failure was rare. 100 loaded runs (load ~16-20) without an injected stall did not reproduce it. A temporary 20 ms sleep between the two updates reproduced it every time.
- Not a production bug. `GameplayPlugin` always inits `SimulationTick` on the server (`src/gameplay.rs`). The begin/finish fixed-tick observation pair already handles zero or many fixed steps per frame.

# Change
- `src/diagnostics/tests.rs` `exit_frame_report_observes_terminal_counts_after_the_shutdown_chain`: inserts `TimeUpdateStrategy::ManualDuration(Time<Fixed>::timestep())`, so frame one runs zero fixed steps and frame two runs exactly one. It also inits `SimulationTick`, as production does. A new assertion checks that the report has `fixed_ticks=1`. All existing assertions are unchanged.
- Sibling tests checked: `disabled_process_diagnostics_install_no_observation_systems` and `common_window_uses_fixed_authoritative_tick_boundaries` run their schedules explicitly. The manifest-participant and Hot Zone aggregate tests install only `Update` systems and no fixed systems. None of them has the same hazard.

# Verification
- With a temporary 40 ms stall between the updates (since removed), the fixed test passes in both server and default (client) builds. Before the fix the same stall failed every time.
- 24-36 `yes` processes on 12 cores, load average 33-36: the target test passed 50/50 consecutive runs. A second set of 50 consecutive runs passed 50/50. The full `diagnostics::` module (28 tests) also passed 50/50.
- `just lint` passed (after `cargo fmt`). `just test` passed, including the network and performance gates.
- Commit 71b22b65 on worktree branch `worktree-agent-a6d54d19b75e6303b`.

# Learning
- `--exact` with a partial test name matches no tests and still reports "ok". The first 100-run loop ran nothing. Use the full path, and check for "1 passed".

## Merge verification (main, 2026-11-20)

Cherry-picked onto main (test-only change). `just lint` pass; `cargo test --locked --features server --lib diagnostics::` 5/5 runs, 28/28 each. The agent's full `just test` and 2×50 loaded runs are recorded above.
