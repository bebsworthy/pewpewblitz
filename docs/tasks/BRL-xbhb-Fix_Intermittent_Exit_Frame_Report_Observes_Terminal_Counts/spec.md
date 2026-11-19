# Found (2026-11-19)

`just test` on main (after 302f37ec BRL-stws and the BRL-q5sm/855z cherry-pick), load average ~16: `_test-server` failed `diagnostics::tests::exit_frame_report_observes_terminal_counts_after_the_shutdown_chain` (`src/diagnostics/tests.rs`) with only "Encountered a panic in system `bevy_app::main_schedule::FixedMain::run_fixed_main`" lines; the panic message was not in the captured log. Five isolated runs and three full `--features server --lib` runs right after all passed (550/550).

# Notes
- The test uses `MinimalPlugins` (real time), so under load `run_fixed_main_schedule` may run several or zero fixed steps per `app.update()`; something in `ProcessDiagnosticsPlugin` fixed systems likely assumes otherwise. BRL-stws edited this file (other tests) but not this test.
- Reproduce with `RUST_BACKTRACE=1` and `--nocapture` in a loop under CPU load (e.g. `stress` or a parallel build), capture the panic, then drive time explicitly (`TimeUpdateStrategy::ManualDuration` or fixed-time advance per AGENTS.md "never wait on wall-clock").

# Acceptance
- Root cause recorded; the test advances time deterministically and passes 50 consecutive loaded runs.
