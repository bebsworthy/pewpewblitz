# Found (BRL-jxfy, 2026-11-16)
When a match's formation fails (readiness not committed within 20 s, or a client cancels), `process_match_loading_messages` (`src/server/mod.rs` ~922-1078) sets `terminal_failure` and sends `CancelActivation`. Two seconds later it calls `exit.write(AppExit::Success)` directly, without Stop, unlink or `queue_exit`, so no `Exit` frame is sent. The supervisor's CancelActivation handling only forwards `ActivationDissolved` and never sends Stop. `reconcile_reaped` (`packages/brawler-routing/src/supervisor/process.rs` ~1228-1238) therefore sees success, code 0 and `worker.exit == None`, and logs `worker failed category=WorkerExitMismatch`. It was visible on the dev server after BRL-q73k started publishing runtime-cleanup failures.

Cleanup and the match-ended notice still happen, but a planned self-termination is logged and counted as a failure. A lobby worker would also be restarted on this path.

# Plan
Either have the worker exit gracefully through Exit after it unlinks (`terminal_peers == 0`), or have the supervisor send Stop on CancelActivation/StartFailed. Add a routing runtime test, and a routed-process test, for a formation failure that ends with `worker stopped forced=false` and no `worker failed`.

# Acceptance
A formation failure logs no `worker failed`. `just test`, `just lint` and `just e2e 2` pass.

# Implemented with BRL-xndc (2026-11-19)
Same defect as BRL-xndc; the research, decision, file list and full evidence are in the BRL-xndc spec.

- Chosen plan option: the worker exits gracefully through Exit after it unlinks (`terminal_peers == 0`), not a supervisor Stop on CancelActivation/StartFailed. A supervisor Stop would unlink every peer about one IPC round trip after the cancel and race the 2 s client-notice grace (`TerminalFailure`/`CancellationAccepted`); the supervisor's own 2 s graceful-stop deadline rules out deferring Stop admission for that grace.
- After the unchanged 2 s grace, `process_match_loading_messages` calls `MatchControlOutbox::retire()`. Once `CancelActivation`/`StartFailed` has entered the control stream, the worker runs the same ordered path as an admitted Stop: Unlink every peer, queue `Exit` (category 0) once peers and queues are empty, and write `AppExit::Success` only after Exit has flushed. The supervisor then logs `worker stopped forced=false` and no `worker failed`. Lobby workers never take this path, and a reaped match never schedules a restart.
- Tests: routing runtime `runtime_retires_a_formation_failed_match_through_exit_without_failure` (plus `runtime_still_classifies_a_crashed_match_as_a_failure`), and server lifecycle regression `loading_deadline_retires_through_cancel_then_exit`. Routed-process evidence for the cancel and missing-client cases: see BRL-xndc.
