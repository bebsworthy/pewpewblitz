Reviewed 2026-11-15 against HEAD 3437c0d8: valid, spec updated — code paths confirmed unchanged since BRL-fwtg (167bd75a); measurement can be done in pure rig tests, so BRL-wegn is not a blocker.

# Context
`CameraRig::step` (`packages/brawler-client/src/presentation_3d/camera_rig.rs:461-531`) frames the camera at `anchor + lead`, where `lead` follows the look-ahead spring (`look_ahead_target`, `:546-581`; tuning `assets/catalogs/camera_rig.ron:16-26`: `distance_cells 1.5`, `max_distance_cells 5.0`, `fighter_safe_margin_cells 1.0`). A south aim leans the camera south, so the fighter sits `-lead.y` north of the view centre, bounded only by `max_distance_cells` and `view.ray_extent(north, safe_margin)`.

BRL-fwtg's band (`top_hud_fighter_limit`, `presentation_3d/camera.rs:338-360`: `HUD_PLATE_BOTTOM + TOP_HUD_OVERHEAD_GAP(6) + OVERHEAD_AMMO_HEIGHT`, then projected) is applied only through `top_hud_band_center` (`camera_rig.rs:539-541`), which only relaxes the north map clamp and "never goes further north than the unclamped framing". Test `top_hud_band_never_moves_open_field_or_south_edge_framing` (`camera_rig/tests.rs:362`) locks that in. So open-field south lean with Sticky Blomb reach (`STICKY_REACH = 674.0`, tests `:195`) can put the local overhead under the top-centre score plate (BRL-fwtg limit ≈ 2 cells above the look target at 1280x720, ≈ 1.9 at 914x411).

# Plan
1. Measure in a pure rig test (no render capture needed): settle `CameraRig` via the existing `settled_center` helper (`camera_rig/tests.rs` `:215`) with aim `(0,-1)`, reach `STICKY_REACH`, open arena, viewports 1280x720 and 914x411; report `fighter.y - center.y` vs `top_hud_fighter_limit(viewport, 1.0)`.
2. Owner decision (ticket question) with the numbers: (a) cap — in `step`, clamp `lead.y >= -top_limit` (after the safe-frame clamp) so the fighter never sits more than `top_limit` above centre; this shortens the BRL-vptw south lean by the excess; or (b) accept the overlap.
3. If (a): update the `top_hud_band_never_moves_open_field...` test, which will no longer hold for south aim, and keep `look_ahead_keeps_the_fighter_visible...` (`:241`) green.

# Acceptance
- Decision recorded in the ticket with measured overlap at both viewports.
- If implemented: rig tests cover a sustained south aim with Sticky reach at 1280x720 and 914x411 and assert `fighter.y - center.y <= top_limit + EPS`; other look-ahead tests pass. `cargo test -p brawler-client --lib presentation_3d::camera_rig`, `just lint`.
- Optional native capture (needs BRL-wegn fixed aim) and Pixel 8a device check of a south Sticky Blomb aim: overhead clear of the score plate.

# Decision (2026-11-15)
Default **(a) cap**: after measuring, clamp the south lean so the overhead stays clear of the top HUD plate, and record the measured overlap in the ticket. The owner can reopen the ticket to accept the overlap instead.


# Measurement (2026-11-18, HEAD fd07cd62, pure rig test)
Settled `CameraRig` (180 frames), fighter at origin, open 60x60 arena, aim `(0,-1)`, reach `STICKY_REACH` (674), before the cap. Cell = 32 world units.

| viewport | ui_scale | fighter.y - center.y | top_hud_fighter_limit | overlap |
|---|---|---|---|---|
| 1280x720 | 1.0 (= fit) | 5.00 cells | 2.05 cells | 2.95 cells |
| 914x411 | 0.642 (fit) | 5.00 cells | 1.72 cells | 3.28 cells |
| 914x411 | 1.0 (unfitted) | 5.00 cells | 0.27 cells | 4.73 cells |

The south lean hits `max_distance_cells` (5) for Pulse reach too, not only Sticky Blomb, so every long-reach south aim put the overhead under the plate.

# Decision applied: (a) cap
`CameraRig::step` (`camera_rig.rs`) computes `top_limit` once (same safe-frame clamp as the BRL-fwtg band) and:
1. caps the look-ahead spring **target** `look_target.y >= -top_limit`, so the spring settles at rest at the cap;
2. caps `lead.y >= -top_limit` after the safe-frame clamp (also bounds a northward follow lag).
The BRL-fwtg north-edge band (`top_hud_band_center`) is unchanged. BRL-crdt shake gating untouched.

Smoothness: an aim swing south→north→south→east at both viewports measured a max per-frame camera step of about 7.2 world units and a max per-frame change in that step of about 1.7. A variant that clamped only the spring output (target uncapped) measured 10.9 and 6.8: the camera stalls while the excess unwinds, then pops. That is why the target is capped too.

Effect: the south lean shortens from 5 cells to about 2.05 cells at 1280x720 and 1.72 cells at 914x411, so the bottom of the screen shows less of a long south reach (BRL-vptw tradeoff). Owner can reopen to accept the overlap instead.

# Tests
- Replaced `top_hud_band_never_moves_open_field_or_south_edge_framing` with `top_hud_band_caps_only_the_south_lean_away_from_the_north_edge`: framing still a fixed point of the clamp; open-field framing equals the look-ahead target with only its south component capped; pure band rule asserts kept.
- `sustained_south_sticky_aim_keeps_the_local_overhead_below_the_top_hud_band` (1280x720, 914x411, fitted UI scale): uncapped lean exceeds the limit by more than a cell; every frame over 600 frames `fighter.y - center.y <= top_limit + EPS`, fighter on screen; settles exactly at the limit.
- `south_lean_cap_eases_in_and_out_of_aim_swings_without_a_pop`: per-frame step < 0.3 cell, jerk < 0.08 cell (the output-only clamp fails this), bound held every frame, settles back at the cap.
- Docs: `docs/11-art-and-presentation-direction.md` camera paragraph updated (cap rule, shortened south lean).

# Verification (2026-11-18)
- `cargo test -p brawler-client --lib presentation_3d::camera_rig`: 28 passed.
- `just lint`: pass.
- `just test`: pass (all suites, including network and performance gates; 0 failures).
- Not done: native capture / Pixel 8a check of a south Sticky Blomb aim (optional; needs BRL-wegn fixed aim). Playtest: `just server` + `just client` (or the Android build), equip Sticky Blomb, hold the aim stick straight down in open ground; the name/health/ammo overhead should stay below the top-centre score plate, and swinging the aim up and back down should ease without a jump.
