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
