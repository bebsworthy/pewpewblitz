# Outcome

The camera feels alive and responsive without affecting gameplay framing guarantees.

# Context

`presentation_3d/camera.rs:79-85` sets the camera transform directly from the interpolated fighter pose every frame. There is no smoothing, look-ahead, or shake channel, and no shake/trauma code exists anywhere in `src/client`.

# Scope

- A critically damped follow on the (predicted, once that lands) local fighter pose.
- A bounded aim look-ahead that respects map-edge clamping and framing invariants.
- A trauma-based shake channel fed by existing `CombatCue`s: local fire (small), local damage taken (medium), nearby explosion/defeat (scaled by distance).
- Tunables in an authored client presentation catalog, not literals.
- Reduced-effects / reduced-motion settings disable shake and shorten smoothing.

# Out of scope

Changing camera angle, FOV, or visible-cell count (separate camera framing ticket).

# Acceptance criteria

- Framing tests in `camera.rs` still pass. The look-ahead never reveals space outside what the framing contract allows.
- Shake is zero when reduced motion is enabled.
- The rig never moves gameplay state and is client-only.
- A native playtest confirms the camera feels smoother and hits feel weightier.

# Verification

- Focused ECS tests for follow convergence and shake decay.
- `just test`.
- A native playtest.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.


# Design (2026-11-12, recorded before implementation)

Research: `follow_3d_camera` (PostUpdate, chained after `write_fighter_visual_poses`, before transform propagation) writes the `ArenaCamera` transform rigidly from the controlled fighter's `Position`, which BRL-0141 already replaces with the predicted, smoothed owner pose in Update. `clamp_3d_camera_center` owns the per-axis fit/follow framing contract. Mouse aim (`input::mouse_aim`) and viewport auto-aim (`targeting::ViewportAutoAim`) project through the camera `GlobalTransform`, so a naively shaken camera would jitter aim. Reduced motion and reduced combat effects already exist in `ClientShellSettings`. Local fire is announced by BRL-0142's `LocalAttackPresented` (plus non-echo `AttackAccepted` for the controlled fighter); damage/defeat/lob landings arrive as `DeduplicatedCombatCue`; barrel explosions arrive only as `WorldObjectCue::Exploded` in `ReceivedWorldObjectCues` (environment damage emits only legacy cues that `DeduplicatedCombatCue` filters out).

Decisions:
- New `src/client/presentation_3d/camera_rig.rs` owns a client-only `CameraRig` resource (follow spring, look-ahead spring, trauma, noise phase) and the validated `CameraRigCatalog` embedded from new `assets/catalogs/camera_rig.ron` (schema 1, `deny_unknown_fields`, code-owned ceilings). Presentation catalog only; not in the gameplay fingerprint or server graph.
- Follow: a critically damped spring (SmoothDamp form, frame-rate independent, dt clamped) on the controlled fighter's presented pose. Snaps on first frame, map-bounds change, leaving the map editor, or a jump larger than the authored snap distance (respawn).
- Look-ahead: a separately smoothed offset of `distance * strength` along `PendingLocalActions.aim_axis`; strength ramps with `aim_distance` (mouse/lob) toward a full-strength distance and is 1 for direction-only gamepad aim; zero when no aim. The combined lead (follow lag + look-ahead) is limited to a code-owned ceiling (3 cells) so the fighter always stays inside the conservative view, and the result then goes through the unchanged `clamp_3d_camera_center`, so look-ahead never reveals space the framing contract forbids (fit axes stay locked to map center; follow axes clamp at the edge). Existing framing tests stay unchanged.
- Shake: trauma model (trauma in [0,1], linear decay, offset = max_offset * trauma^exponent * smooth deterministic sine noise) applied as a pure ground-plane camera translation after framing. Triggers and amounts are authored: local fire (small), local damage taken (medium), local defeat, nearby lob landing / barrel explosion / defeat scaled by linear distance falloff. The applied offset is stored on the camera as `ArenaCameraShake`; mouse aim and viewport auto-aim project through the unshaken transform, so shake never affects aim, targeting, input, or gameplay. Shake amplitude has a code-owned ceiling of 0.25 cell; it is a transient presentation offset outside the follow/look-ahead framing clamp (documented exception, not applied in the map editor).
- Reduced motion: shake zero, look-ahead off, follow uses the shorter authored reduced-motion smoothing. Reduced combat effects: shake zero.
- Out of scope (BRL-0148): FOV, elevation, distance, visible-cell count.

Verification plan: pure tests for spring convergence/no-overshoot, lead ceiling and framing-clamp invariants with look-ahead at every map edge, trauma decay and reduced-motion zero shake; ECS tests with manual `Time` advancing explicit `Update`/`PostUpdate` runs for follow convergence and shake decay and for unshaken aim projection; catalog validation tests; then `just lint`, `just test`, `just e2e 2`.


# Implementation (2026-11-12)

- `src/client/presentation_3d/camera_rig.rs` (new): `CameraRigCatalog` (validated, embedded `assets/catalogs/camera_rig.ron`, schema 1), `CameraRig` resource (follow spring, look-ahead spring, trauma, noise phase), `ArenaCameraShake` component, `ArenaAimCameras` + `arena_aim_view` (unshaken aim view), and `accumulate_camera_trauma` (Update, `WorldPresentationSet::ConsumeCues`, before `VfxRequestSet` so it reads the world-object inbox before the VFX producer drains it).
- `src/client/presentation_3d/camera.rs`: `follow_3d_camera` keeps its PostUpdate slot (after `write_fighter_visual_poses`, before propagation) and steps the rig; the editor path bypasses and resets the rig. Framing functions and their tests are unchanged.
- `src/client/input.rs`, `src/client/targeting.rs`: mouse aim and viewport auto-aim use `arena_aim_view`.
- `docs/11-art-and-presentation-direction.md`: durable camera rig contract paragraph.
- Shake triggers: `LocalAttackPresented` and non-echo `AttackAccepted` for the controlled fighter (local fire), `DamageApplied`/`FighterDefeated` targeting the controlled fighter, and distance-scaled `FighterDefeated`, `LobLanded`, and `WorldObjectCue::Exploded`.
- Authored tuning: follow 0.11 s (reduced motion 0.03 s), snap at 6 cells; look-ahead 1.5 cells, 0.32 s, full strength at 5 cells of aim distance; shake 6 world units maximum, 16 Hz, decay 1.8/s, exponent 2; triggers fire 0.14, damage 0.42, local defeat 0.7, nearby explosion 0.6, nearby defeat 0.3, falloff 9 cells. Code ceilings: lead 3 cells, shake 0.25 cell, smoothing 0.01–1 s, step dt ≤ 0.1 s.

# Verification (2026-11-12)

- `cargo test --locked --no-default-features --features client --lib camera`: 20 passed (5 unchanged `camera.rs` framing tests, `client::tests::camera_clamp_*`, `movement::tests::camera_and_spawn_bounds_are_stable`, and 13 new `camera_rig` tests: catalog validation and ceilings, spring convergence/no overshoot/frame-rate independence (30 vs 150 Hz agree within 4 units after 0.1 s), follow smoothing and respawn snap, look-ahead framing fixed-point plus fighter-visibility at every edge of 50×50 and 25×37 maps in five aim directions, look-ahead strength ramp, trauma saturation/linear decay/shake ceiling, noise continuity across the phase wrap, unshaken aim-view restoration, and ECS tests with `TimeUpdateStrategy::ManualDuration` for follow convergence, local fire + nearby defeat trauma amounts, local-defeat shake decaying to exact rest without moving the framed center, and reduced motion keeping shake at zero).
- `just lint`: pass. Note: `scripts/check-v3-world-presentation.sh` and `scripts/check-v8-map-cleanup.sh` print `rg: command not found` in this agent shell (rg is only a shell function there) and then pass vacuously; both were re-run with a real `rg` on PATH and passed.
- `just test`: pass (all suites, including network and performance gates; 0 failures).
- `just e2e 2`: pass ("1 exact 1v1 roster(s) reached Active").
- Native visual evidence: not captured by the agent (no display session); covered by the playtest request below.

# Playtest request

Run `just run 2`, open Practice (any 1v1/2v2/3v3 map with bots; Feature Yard Heist includes oil barrels).
- Move around with WASD: the camera should trail slightly and settle with no overshoot; respawn should cut, not sweep.
- Aim with the mouse in a circle around the fighter and with a gamepad right stick: the view should lean about 1.5 cells toward the aim and ease back when the stick is released. At map edges and on the 25-wide axis the view must not show beyond the map.
- Fire continuously, take damage, get defeated, and stand near a lob landing, barrel explosion, or bot defeat: small, medium, and stronger shakes. Shots should still land where the cursor points during a shake.
- Toggle Reduced motion (no shake, no lean, near-rigid follow) and Reduced combat effects (no shake).
Observations wanted: does the camera feel smoother, do hits feel weightier, is any shake too strong, too frequent with fast weapons, or nauseating, and does the lean help or hurt aiming.
Known limitations: one fire-shake value for every weapon; damage-taken shake ignores the amount; shake has no separate intensity setting (BRL-0173); shake may reveal up to 6 world units past the framing clamp for a moment (documented exception); framing values themselves belong to BRL-0148 (comment left there about the lead ceiling).

# Learn from errors

- An initial test asserted that the framed center stays within the lead ceiling of the raw fighter position, which is false at clamped map edges. The correct invariant compares against the rigidly framed center, since the clamp is 1-Lipschitz per axis. Lesson: state framing invariants relative to the clamp output, not raw positions.
- Clippy's pedantic float/cast lints in tests failed the first lint pass. Lesson: write test step counts as integers and compare floats with tolerances from the start.
- Repository lint scripts depend on `rg` and silently pass when it is missing. That is a tooling risk worth noting at handoff, not a code change in this ticket.
