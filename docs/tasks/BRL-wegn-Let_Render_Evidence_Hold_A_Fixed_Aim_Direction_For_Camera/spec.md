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
