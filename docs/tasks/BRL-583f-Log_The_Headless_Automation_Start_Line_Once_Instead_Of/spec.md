Reviewed 2026-11-15 against HEAD 3437c0d8: valid, spec corrected — the file moved to `automation/mod.rs`, and the line repeats every frame of the whole match, not only before the match.

# Outcome

Automation client logs stay readable: the "headless movement automation enabled" info line is written once per run.

# Context (found during BRL-1fam, re-verified)

- The log is at `packages/brawler-client/src/automation/mod.rs:185-194` (previously `automation.rs`), guarded by `automation.elapsed_ticks == 0`.
- Corrected root cause: `elapsed_ticks` only advances in `advance_headless_automation` (`automation/mod.rs:278-289`) when `simulation_ticks.is_some()`. Render evidence never passes `--simulation-ticks`, so `elapsed_ticks` stays 0 for the entire Active match. `apply_headless_input` runs once per rendered frame (`RunFixedMainLoop` / `BeforeFixedMainLoop`, `session/mod.rs:200-206`), so the line repeats about 120 times per second. Evidence: BRL-1fam `weapon-7.txt.client-1.log` has 1,773 lines from 17:08:49.84 to 17:09:04.78, the whole match.
- The same stuck `elapsed_ticks` also means `HEADLESS_FIRE_DURATION_TICKS` never expires and `ultimate_held` (`is_multiple_of(12)`) is true every frame in render evidence. Leave that behaviour unchanged here: it is the evidence's intended "fire continuously" mode. Note it in a comment.

# Plan

- Add a `mut logged: Local<bool>` to `apply_headless_input` and log once when the first input is written, with the same fields, independent of `elapsed_ticks`. If the rejoin or network-change smokes want a line per match session, reset the flag when the join phase leaves Active. Otherwise once per process is enough.
- This touches automation-feature code only; no gameplay or input change.

# Acceptance

- `BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_COMBAT=1 just v3-render-evidence target/583f/r.txt`, then `grep -c "headless movement automation enabled" target/583f/r.txt.client-1.log` prints 1.
- `just practice-e2e` logs still show the line once per client.
- `just lint` and `just test` pass.


# Result (2026-11-18)

- In `packages/brawler-client/src/automation/mod.rs`, the new `log_automation_start_once` helper writes the "headless movement automation enabled" line once per process. It is gated on a `Local<bool>` paired with `Res<HeadlessAutomation>` in a tuple parameter, which keeps the system within Clippy's 7-argument limit. Extracting the helper also keeps `apply_headless_input` under the 100-line limit.
- The helper's doc comment records that render evidence keeps `elapsed_ticks` at 0 on purpose: fire never expires and the ultimate is held every frame, which is the evidence's "fire continuously" mode. That behaviour is unchanged.
- Once per process is enough. The rejoin and network-change smokes don't depend on this line.

Verification:
- `BRAWLER_RENDER_BIND=127.0.0.1:5033 BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_COMBAT=1 just v3-render-evidence target/583f/r2.txt` passes. `grep -c "headless movement automation enabled" target/583f/r2.txt.client-1.log` prints 1; BRL-1fam's run had 1,773.
- `just practice-e2e` passes. Its Practice-smoke clients get no scripted move, aim, fire or ultimate input, so the line never prints there, before or after this change. The acceptance's "once per client" item therefore doesn't apply to that smoke.
- `just lint` and `just test` pass. Logs are in `target/583f/`.
