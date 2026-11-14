# Outcome

Aiming a long-range weapon, or aiming up or down the screen, shows enough of the threat area under
the BRL-xyj7 framing (30° FOV, 55° elevation, ~12 visible cells: about 4.9 cells behind the look
target, 7.1 ahead, ~10.5 to each side at the 2.21:1 reference aspect).

# Context

- BRL-xyj7 accepted an explicit exception: the standard 320-unit Pulse range fits to either side
  at the reference aspect, but vertical aim shows only ~5-7 cells, and lobs (520), Splash (480),
  Sticky Blomb (640, ~755 with Long Assembly), and ranged ultimates exceed the view.
- The BRL-e4xj rig leans 1.5 cells toward aim (`assets/catalogs/camera_rig.ron`
  `look_ahead.distance_cells`), capped with follow lag at `MAX_CAMERA_LEAD_WORLD` = 3 cells, below
  the 4.9-cell near extent.

# Scope

- Evaluate per-direction or range-scaled look-ahead (e.g. lean further when aiming a long-reach
  weapon or along the screen's short axis) within an aspect-fair ceiling; keep it presentation
  only and disabled under reduced motion.
- Keep aim/auto-aim on the unshaken view and the per-axis framing clamp unchanged.

# Out of scope

Changing the BRL-xyj7 framing constants, weapon ranges, or map sizes.

# Acceptance criteria

- Rig tests still prove the fighter stays visible at every edge and aspect.
- Native captures and a playtest confirm long-range aim reads without motion discomfort.



# Design and decisions (2026-11-14)

Research: the BRL-e4xj rig leaned a fixed 1.5 cells toward `PendingLocalActions.aim_axis`, scaled by
`aim_distance` / 5 cells, then capped follow lag plus lean at a 3-cell radius
(`MAX_CAMERA_LEAD_WORLD`) before the per-axis framing clamp. Aim sources: mouse (`input::mouse_aim`:
direction plus cursor ground distance, measured through the unshaken but *leaning* view), gamepad
right stick (direction only for straight weapons; deflection × placement range for lob/Splash and
targeted ultimates; auto-aim direction plus target distance while firing), and touch attack/ultimate
sticks (same placement scaling; held-release carries the aim a few ticks). The view is a trapezoid on
the ground: at the 2.21:1 reference 4.87 cells to the bottom edge, 7.12 to the top, 8.8 to each side
at the near edge and about 10.5 on the look-target row. Standard Pulse reach is 320 range + 34 muzzle
= 354 units (11.06 cells).

Decisions:
- Reach-aware, per-direction lean. `lean = clamp(reach + reach_margin - view_extent(aim), minimum,
  maximum)`, then limited by the fighter safe frame (`view_extent(-aim)` shrunk by the safe margin),
  then scaled by the existing aim-distance strength. The extent is measured on the real visible
  trapezoid for the current window aspect (new `GroundView` in `camera.rs`; the conservative
  near-edge box is derived from it and the framing clamp is unchanged), so vertical aim leans more
  than horizontal aim and sideways aim keeps the old 1.5-cell lean when the reach is already on
  screen. Aspect fairness: the lean only ever reveals up to the commanded reach plus margin.
- Commanded reach: an armed targeted ultimate's placement range, otherwise the primary weapon's
  reach (straight/sticky: range + muzzle offset; lob/Splash: placement distance; melee/cone: reach).
- The 3-cell radius cap is replaced by a safe frame: follow lag plus lean is clamped so the fighter's
  ground point stays at least `fighter_safe_margin_cells` inside every screen edge of the current
  viewport (degrading to the look target itself on extreme aspects). Then the unchanged framing clamp.
- Mouse feedback: the cursor distance grows as the view leans, so `full_strength_aim_distance_cells`
  must be at least 1.5 × `max_distance_cells` (code-owned), keeping the loop gain at or below 2/3; the
  view settles instead of chasing the cursor. A pure test simulates the loop.
- Catalog schema 3 (`assets/catalogs/camera_rig.ron`): `distance_cells` 1.5 (minimum),
  `max_distance_cells` 5.0, `reach_margin_cells` 0.25, `fighter_safe_margin_cells` 1.0,
  `smooth_seconds` 0.32, `full_strength_aim_distance_cells` 8.0 (was 5.0). Code bounds: max lean
  ≤ 6 cells, minimum ≤ maximum, reach margin ≤ 2, safe margin 1–3 cells, full strength ≥ 1.5 × max.
  Reduced motion still disables the lean. Aim/auto-aim still use the unshaken view.
- Result at the reference aspect, standard Pulse: sideways full reach with the 1.5-cell lean; up
  shows 10.99 of 11.06 cells (safe-frame limited, lean 3.87); down shows 9.87 cells (lean at the
  5-cell maximum; was 6.4 before). Longer reaches (Arc 16, Splash 15, Sticky 21 cells) lean to the
  limit and still rely on the aim indicator.
- Known trade-off: with a short weapon, mouse aim at 5 cells now leans ~1.15 cells instead of 1.5
  (full strength moved from 5 to 8 cells to keep the mouse loop stable).

# Implementation (2026-11-14)

- `packages/brawler-client/src/presentation_3d/camera.rs`: `GroundView` (trapezoid rows, widths,
  `ray_extent`, `clamp`, test-only `contains`) and `ground_view(aspect)`;
  `conservative_ground_footprint` now derives from it (values unchanged). `follow_3d_camera` reads
  the controlled fighter's `ResolvedMatchLoadout` and passes the commanded reach (`aim_reach`).
- `packages/brawler-client/src/presentation_3d/camera_rig.rs`: schema 3 tuning and code bounds,
  `validate_look_ahead`, `look_ahead_target(tuning, aim, reach, view)`, `weapon_reach`, safe-frame
  lead clamp; `MAX_CAMERA_LEAD_WORLD` removed.
- `packages/brawler-client/src/presentation_3d/camera_rig/tests.rs`: new pure tests (below).
- `assets/catalogs/camera_rig.ron`: schema 3. `docs/11-art-and-presentation-direction.md`: rig
  contract paragraph updated.
- Server, protocol, gameplay catalogs, ranges, framing constants, and maps unchanged.

# Verification (2026-11-14)

All runs used a scratch `CARGO_TARGET_DIR`; the running user server was not touched.
- Focused: `cargo test --locked --lib presentation_3d::camera` (in `packages/brawler-client`):
  33 passed. New or rewritten pure tests: fighter stays inside the true view at every map edge and
  corner, for 5 aspects (4:3 to 32:9), 3 map shapes, 8 aim directions, and 4 reaches, stays a fixed
  point of the framing clamp, stays within the lean ceiling of the rigid view, and keeps the safe
  margin in open arena; the reach (plus margin) is on screen ahead unless the lean is at its limit
  (maximum or safe frame), and an on-screen reach leans only the minimum; standard Pulse per
  direction at the reference aspect (sides full, up within 0.1 cell, down at least 88%); vertical
  aim leans more than horizontal; the simulated mouse loop settles and keeps the safe frame;
  `weapon_reach` covers all 7 presets (Pulse = 354); catalog bounds fail closed (oversized maximum,
  thin safe margin, runaway full strength, schema 2). Existing follow/shake/ECS/framing tests pass.
- `just check`: pass.
- `just lint` (VS Code's bundled ripgrep first on PATH; no `rg: command not found`): pass after
  fixing unnecessary qualifications, an unused test helper, a test-only method, and a
  `too_many_lines` finding on `validate` (look-ahead bounds moved into `validate_look_ahead`).
- `just test`: pass, 2365 passed, 0 failed (including network and performance gates).
- Native captures: not taken. The release render-evidence client cannot hold a fixed aim direction;
  deferred to BRL-wegn. The playtest below covers the subjective check.

# Playtest request

The client changed and the server did not, so only clients need restarting. Run `just run 2` (or
`just client` against the running server) and open Practice on any map.
- Pulse Sidearm with the mouse: hold the cursor near the top, then the bottom of the screen. The view
  should lean further than before vertically (about 3.9 cells up and 5 down) and settle, not creep
  after the cursor. Sideways it should feel as before.
- A gamepad right stick and the touch attack stick: aim up and down and sweep the stick in circles.
  Watch for discomfort from the larger vertical travel.
- Switch to a long-range weapon (Sticky Blomb, Arc lob, Splash) and a targeted ultimate: the lean
  should reach its maximum, and the fighter should stay well clear of every screen edge.
- Resize to a narrow (16:9) and an ultra-wide window: the fighter stays on screen, and the lean
  differs by axis.
- Reduced motion: no lean.
Observations wanted: does long-range and vertical aim now read, is the 5-cell maximum or the
0.32 s smoothing uncomfortable, and does mouse lean feel stable.
Known limitations: Pulse down shows about 9.9 of 11 cells at the 5-cell maximum; reaches above about
12 cells still extend past the view; one safe margin covers every edge, although the fighter's
body rises toward the top edge.

# Learn from errors

- The first rig draft measured the view on the conservative near-edge box, which would have leaned
  sideways about 2.5 cells for Pulse when the real row already shows the reach. Lesson: use the true
  trapezoid for look-ahead and the conservative box only for fit and clamp decisions.
- An aspect assertion assumed that vertical lean always exceeds horizontal lean by a cell. On a
  narrower window the sideways view is also narrower. Lesson: derive per-aspect expectations from
  both axes.
- Lint: `unused_qualifications` fires for paths the presentation glob already imports, and a long
  `validate` crossed `too_many_lines`. Lesson: lint new client code early and split validation by
  section.
- Making the lean depend on mouse distance creates a feedback loop, because that distance is
  measured through the leaning view. The bound has to be code-owned and covered by a test.
