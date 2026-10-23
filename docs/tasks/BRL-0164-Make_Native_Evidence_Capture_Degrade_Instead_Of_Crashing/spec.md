# Outcome

Render evidence runs produce captures reliably, and a capture failure never takes down the client or passes silently.

# Context

On 2026-10-23, this command:

```
BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_COMBAT=1 BRAWLER_RENDER_GAME_TYPE=wipeout-3v3 \
BRAWLER_RENDER_PLAYERS_PER_TEAM=3 BRAWLER_RENDER_WINDOW_SIZE=1600x900 \
./scripts/v3-render-evidence.sh target/review-captures/practice.txt
```

panicked in `capture_scheduled_screenshot` at `src/client/evidence_capture.rs:383` with `called Result::unwrap() on an Err value: Error("number out of range")` (serde_json). The capture directory stayed empty, yet the script exited 0.

# Scope

- Find the value serde_json cannot represent (for example a u128 or a non-finite float in the match snapshot) and serialize it correctly.
- Capture/serialization failures log and continue.
- `scripts/v3-render-evidence.sh` exits non-zero when the required report or captures are missing, or when a client exits abnormally.

# Out of scope

Black-capture investigation (BRL-0115) and duration validation (BRL-0106), unless they share a root cause.

# Acceptance criteria

- The repro command produces the expected PNG and JSON captures.
- An injected serialization failure does not crash the client (test).
- The script exits non-zero on missing captures or a client panic.

# Verification

- The repro command.
- A focused test.
- `just test`.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.

# Implementation (2026-10-23)

- Root cause: `collect_match_snapshot` interpolated the whole `WipeoutState` into `json!`. Its `match_id: Option<MatchId>` is a routed `u128` above `u64::MAX`, which `serde_json::to_value` rejects ("number out of range"), and `json!` unwraps that conversion. The earlier wide-ID fix covered state/clock/Hot Zone/Heist but not Wipeout.
- `src/client/evidence_capture.rs`: Wipeout is now serialized field by field with `match_id` as a decimal string, like the other match IDs. Typed (non-primitive) snapshot fields go through a new `evidence_field` helper. On failure it logs a `warn!` and records `{"serialization_error": …}` in place, so the PNG and the rest of the state are still saved.
- `scripts/v3-render-evidence.sh`: the script now records the measured client's exit status and fails on a non-zero status. It fails when any client log contains `panicked at`, because a panic in the macOS winit loop can exit without a non-zero status. It also fails unless the capture directory holds exactly `capture_count` (3) PNG and JSON files. The `--screenshot-count` flags share `capture_count`.

# Verification

- Focused: `cargo test --locked --no-default-features --features client --lib evidence_capture`: 3 passed. `wide_match_ids_are_captured_as_json_strings` now includes `WipeoutState { match_id: Some(MatchId(u128::MAX)) }` and would panic without the fix. The new `unserializable_evidence_fields_degrade_instead_of_panicking` covers an injected `Serialize` error and a u128 above `u64::MAX`.
- Repro command (into `target/brl-0164/practice.txt`): no panic. 3 PNG and 3 JSON captures were written (`brawler-000300/480/660`), with no `serialization_error` entries. The JSON `match.wipeout.match_id` is the full 37-digit routed ID as a string. The script exited 1 because the report failed its locked `frame_p95_ms` threshold (29.9 ms at 3200x1800 physical). The default 1v1 run also exited 1 on `frame_p95_ms` (56.9 ms). The host load average was about 16 from concurrent builds. This is a performance or environment result, not a capture failure, and is outside this ticket. A quiet-machine rerun should confirm the pass path.
- Script failure paths: the new check blocks were extracted from the script and run against real artifacts.
  - The original 2026-10-23 panicking log fails with "client panicked" and exit 1.
  - An empty capture directory and a missing capture directory each fail with "expected 3 PNG and JSON captures, found 0…" and exit 1.
  - A simulated status 101 fails with "measured client exited with status 101" and exit 1.
  - The fixed run's artifacts pass.
- `bash -n scripts/v3-render-evidence.sh`: pass. `just _fmt-check`: pass. `just check`: pass. `just test`: pass (exit 0, every suite ok).
- `just _clippy-client`: fails with 8 findings, all in files this ticket did not touch: `matchplay/zombie.rs`, `client/flow/input.rs`, `client/flow/screens/{equipment,map_maker}.rs`, `client/flow/tests.rs`, `client/shell/settings_screen.rs`, `client/ui/{controls,gallery}.rs`. There are no findings in `evidence_capture.rs`. The findings come from other work that landed on main.

# Learning

- A partial fix for one serialization hazard (wide match IDs) left a sibling path (`WipeoutState`) unguarded. Interpolating whole typed values into `json!` hides an `unwrap`. Evidence and diagnostic code should convert typed values fallibly and degrade per field, so one unrepresentable value cannot take down the client.
- An evidence script should assert the artifacts it promises (the capture count) and check client logs for panics, not trust exit status alone. Winit on macOS can swallow a panicking event handler.
