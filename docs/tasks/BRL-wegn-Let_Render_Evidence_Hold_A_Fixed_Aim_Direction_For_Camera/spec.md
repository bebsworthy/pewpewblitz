Reviewed 2026-11-15 against HEAD 3437c0d8: valid but smaller than written — the evidence client already accepts `--aim-axis`, the camera rig reads it, and the capture JSON records it, so this is now a script-only change; spec updated.

# Outcome

`scripts/v3-render-evidence.sh` can hold a fixed aim direction for the measured client, so native captures show the BRL-vptw camera look-ahead toward the top, bottom, and sides of the screen.

# Corrections to the original context

- The original claim that the release evidence client has no fixed-aim input is stale. The script builds the release client with `--features automation` (`scripts/v3-render-evidence.sh:129-130`). `--aim-axis X,Y` (`packages/brawler-client/src/bin/client/automation.rs:131`, `parse_axis` line 23, i8 components) is accepted with `--render-report` (`automation/config.rs` `validate_scripted_input`). `BRAWLER_RENDER_COMBAT=1` already passes `--aim-axis 1,0` (script lines 59-62).
- The path is canonical. `apply_headless_input` (`automation/mod.rs:79`, runs after `resolve_targeted_ultimate_input`) writes `PendingLocalActions.aim_axis` whenever an aim is set. The camera rig reads that same sample (`presentation_3d/camera.rs:164-171` into `CameraRigFrameInput.aim`, then `look_ahead_target` at `camera_rig.rs:546`). With `aim_distance=None` the lean is at full strength.
- The capture JSON already records the aim: `client.pending_input.aim_axis` / `aim_distance` (`evidence_capture.rs:319-330`), plus `camera.translation`.
- BRL-1fam is done: `BRAWLER_RENDER_WEAPON=<id>` selects the weapon. Long-range IDs from `content/catalogs/weapons.ron`: Arc Launcher 3, Sticky Blomb 5, Splash 7.

# Plan

1. Add `BRAWLER_RENDER_AIM=up|down|left|right` (optionally `X,Y`) to the script's env parsing, validated like the other options. Map it to `--aim-axis 0,1 | 0,-1 | -1,0 | 1,0` for the measured client only. When set, it replaces the combat-smoke aim (the peer keeps its own).
2. While an aim is held, make the measured client stand still (`--move-axis 0,0`, or a `BRAWLER_RENDER_MOVE` override), so the lean is not mixed with follow motion. Default runs keep `--move-axis 1,0` and stay unchanged.
3. Print the aim in the success line. No client code change is expected; if the verification run shows the sample not reaching the rig, fix that in `apply_headless_input`, not with a second path.

# Acceptance

- For each direction (and with `BRAWLER_RENDER_WEAPON=3`, `5`, `7`), run `BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_AIM=<dir> just v3-render-evidence target/wegn/<dir>.txt`. The paired PNG/JSON captures show the lean, the JSON `client.pending_input.aim_axis` matches the direction, and `camera.translation` is offset from the fighter in that direction.
- A default run's command line and captures are unchanged; `bash -n` and `just lint` pass.
- Captures after the first are clean only once BRL-hkbm lands (the capture toast otherwise covers the bottom-right HUD).


# Result (2026-11-18)

Script-only change in `scripts/v3-render-evidence.sh`; no client code change was needed (`apply_headless_input` already writes the aim sample the camera rig reads).

- `BRAWLER_RENDER_AIM=up|down|left|right|X,Y` maps to the measured client's `--aim-axis` (`0,1`, `0,-1`, `-1,0`, `1,0`, or the signed i8 pair). It replaces the combat-smoke aim (`--aim-axis 1,0` or `--aim-dummy`) while keeping `--fire --ultimate`; the peer keeps its own flags. Rejects unknown words, `0,0`, components outside ±127, and malformed pairs (exit 2).
- `BRAWLER_RENDER_MOVE=X,Y` overrides the measured move axis. Default is `1,0`, or `0,0` while an aim is held.
- The success line appends `; aim=<value> (<axis>); move=<axis>` only when an aim is set. The default run's flags are unchanged (client log: `move_axis=Vec2(1.0, 0.0) aim_axis=None`).
- Screen mapping checked in code: `coordinates.rs` maps sim +Y to render -Z, and `camera.rs` places the camera at +Z looking toward -Z with +Y up, so sim +Y is the top of the screen. The captures confirm this: with `up`, the fighter sits below center.

Verification (all `BRAWLER_RENDER_BIND=127.0.0.1:5034 BRAWLER_RENDER_PRACTICE=1`, each passed, 3/3 PNG+JSON). The look target is derived from `camera.translation` minus `camera_offset()` (z 324.6); the offset is the look target minus the controlled fighter's position, and it was identical across the three captures.

| Run | weapon | aim_axis | fighter | camera.translation | look offset |
|---|---|---|---|---|---|
| up | default 1 | [0,1] | (-752,-304) | (-752, 463.6, 504.9) | (0, +123.9) |
| down | default 1 | [0,-1] | (-752,304) | (-752, 463.6, 86.2) | (0, -65.6) |
| left | default 1 | [-1,0] | (-752,304) | (-797, 463.6, 20.6) | (-45.0, 0) |
| right | default 1 | [1,0] | (-752,-304) | (-659.6, 463.6, 628.6) | (+92.4, 0) |
| w3-up | 3 Arc Launcher | [0,1] | (-752,304) | (-752, 463.6, -87.5) | (0, +108.1) |
| w5-left | 5 Sticky Blomb | [-1,0] | (-752,-304) | (-797, 463.6, 628.6) | (-45.0, 0) |
| w7-down | 7 Splash | [0,-1] | (-752,-304) | (-752, 463.6, 694.2) | (0, -65.6) |

Known limit: the left lean holds at the 1.5-cell minimum (45). The Practice spawn sits at x=-752 by the west wall, and in every left run the camera stops at x=-797, so the map's camera-bounds clamp is the likely cause; the lean is still visible. `target/wegn/directions.png` (2x2: up, down / left, right; frame 480) shows the lean in each direction. The weapon frames were also inspected.

Gates: `bash -n` ok; default `just v3-render-evidence target/wegn/default.txt` passed (both reports); `just lint` ok; `just test` ok (0 failed). Logs and evidence are in `target/wegn/`. Uncited PNG frames (300/660) and the server-data directories were deleted.


Reviewer note (2026-11-18): the shorter down lean (66 vs 124 up) is expected; BRL-htdb caps the south lean. The left lean sits at its minimum because the Practice spawn is at the west wall, so the camera bounds clamp it.
