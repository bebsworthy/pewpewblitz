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
