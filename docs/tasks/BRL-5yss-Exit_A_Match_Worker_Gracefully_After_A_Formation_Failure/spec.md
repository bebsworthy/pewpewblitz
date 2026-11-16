# Found (BRL-jxfy, 2026-11-16)
When a match's formation fails (readiness not committed within 20 s, or a client cancels), `process_match_loading_messages` (`src/server/mod.rs` ~922-1078) sets `terminal_failure` and sends `CancelActivation`. Two seconds later it calls `exit.write(AppExit::Success)` directly, without Stop, unlink or `queue_exit`, so no `Exit` frame is sent. The supervisor's CancelActivation handling only forwards `ActivationDissolved` and never sends Stop. `reconcile_reaped` (`packages/brawler-routing/src/supervisor/process.rs` ~1228-1238) therefore sees success, code 0 and `worker.exit == None`, and logs `worker failed category=WorkerExitMismatch`. It was visible on the dev server after BRL-q73k started publishing runtime-cleanup failures.

Cleanup and the match-ended notice still happen, but a planned self-termination is logged and counted as a failure. A lobby worker would also be restarted on this path.

# Plan
Either have the worker exit gracefully through Exit after it unlinks (`terminal_peers == 0`), or have the supervisor send Stop on CancelActivation/StartFailed. Add a routing runtime test, and a routed-process test, for a formation failure that ends with `worker stopped forced=false` and no `worker failed`.

# Acceptance
A formation failure logs no `worker failed`. `just test`, `just lint` and `just e2e 2` pass.
