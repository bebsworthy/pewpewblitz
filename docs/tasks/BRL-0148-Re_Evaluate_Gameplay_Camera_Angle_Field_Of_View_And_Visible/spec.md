# Outcome

Fighters are clearly readable characters rather than specks, and the framing is chosen by playtest evidence.

# Context

- `presentation_3d/camera.rs:5-9`: 15-degree vertical FOV, 62-degree elevation, 1,495-unit distance. This is nearly orthographic and shows about 14 cells vertically. `docs/11-art-and-presentation-direction.md:52-58` locks these values.
- Fighters occupy roughly 5% of screen height in captures, and their faces are not visible.

# Scope

- Prototype at least three framings, for example FOV 25-35 degrees at 50-55 degrees elevation with 11-14 visible cells.
- Evaluate at the native 2.21:1 aspect and a phone aspect.
- Criteria: fighter readability, threat visibility at the longest authored weapon range, objective visibility, and motion comfort.
- Choose one. Update the framing constants, the framing tests, and `docs/11`.

# Out of scope

Camera smoothing and shake (camera rig ticket) and map size changes.

# Acceptance criteria

- The chosen framing is documented with comparison captures and rationale.
- The longest-range authored weapon's full range from the fighter is on screen in the primary aim directions, or an explicit decision explains the exception.
- Framing tests are updated and passing.
- Native playtest acceptance.

# Verification

- `just test`.
- Native captures and a playtest.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.


# Research and decision (2026-11-13)

## Constraints found

- Framing is client presentation only (`src/client/presentation_3d/camera.rs`); server ranges,
  concealment, and replication are unaffected. Aim and auto-aim already project through the live
  (unshaken) camera, so they follow any framing automatically. Viewport auto-aim only targets
  on-screen enemies, so the visible area also bounds auto-aim reach.
- Fairness: the old contract fixed ~14 cells of depth and let width grow with aspect. At 2.21:1 the
  25-cell 3v3 maps fit completely, which hid the problem. Any zoom-in makes width follow the
  player, so a wider (or user-resized ultra-wide) window would see more of the arena than a phone.
- Longest authored reaches: Pulse Sidearm/Scatter 320 (10 cells), Splash 480, Arc lob 520, Sticky
  Blomb 640 (20 cells, about 755 with Long Assembly), ultimates up to 640; custom Pulse Long 1,050
  exists only as a catalog seam with no player UI. No framing in the 11-14 cell range (including
  the old one, about 6.5 cells behind and 7.5 ahead) can show 20 cells in every direction.
- Projected plates (BRL-0145) and floating numbers (BRL-0144) are pixel-sized and need no change.

## Candidates (analytic footprint plus native captures)

| | FOV / elevation / distance | depth x near-edge width @2.21 | row width | fighter height on screen |
|---|---|---|---|---|
| Baseline | 15° / 62° / 1,495 | 14 x 25.4 cells | 27.2 | ~6% |
| A | 30° / 55° / 566 | 12 x 17.6 (4.9 behind, 7.1 ahead) | 21.0 | ~11% |
| B | 25° / 55° / 750 | 13 x 19.9 | 23.0 | ~9.5% |
| C | 30° / 50° / 478 | 11 x 14.4 | 17.7 | ~14% |
| Width-preserving (analytic only) | 20° / 45° / 1,227 | 19.7 x 25.4 | 29.9 | ~8.7% |

Captures (frames 300/480/660, `BRAWLER_RENDER_COMBAT=1`): baseline 2.21:1
`target/brl0148/base221-theme{1-combat,2-practice,3-practice}.txt.captures`; baseline 16:9 is
BRL-0152's `target/brl0152/after-*`; candidates `target/brl0148/cand{A,B,C}-221-theme{1,2,3}-*`
and `cand{A,B,C}-169-theme3-practice.txt.captures`.

## Decision: candidate A (30° FOV, 55° elevation, 566 units, ~12 visible cells)

- Fighters read about 1.7x taller, front and face visible; the Heist idol face reads; perspective
  stays restrained (walls lean only slightly). B is a marginal gain; C is cramped, leans walls
  noticeably, and cannot show the 10-cell Pulse range to the side.
- Weapon-range exception (explicit): A keeps the standard 320-unit Pulse Sidearm range on screen
  to both sides at the reference aspect (row half-width 10.5 cells). Vertical aim shows about
  4.9-7.1 cells plus 1.5 cells of aim look-ahead. Lobs, Splash, Sticky Blomb, and ranged
  ultimates extend beyond the view; the aim indicator and look-ahead carry them, as before (the
  old framing could not show them either). This was accepted as the cost of readable fighters.
- Width-preserving framings were rejected: they keep fighters small (~8.7%) and reveal far more
  depth.
- Aspect fairness (new, small): `camera_vertical_fov(aspect)` keeps the reference vertical FOV
  for aspects up to 2.21:1 and the reference horizontal FOV above it (vertical FOV shrinks). No
  window shape sees more than the reference aspect on either axis; narrower windows see less width,
  as before. The arena camera's projection FOV is written each frame from the window aspect.
- Production 25x37 maps now follow the player on both axes at the default aspect (previously the
  width was locked). The per-axis clamp is unchanged.
- BRL-0143 rig ceilings re-checked (comment c-000009): the 3-cell lead ceiling stays below the
  nearest view extent (4.9 cells behind at the reference aspect), 1.5-cell look-ahead unchanged,
  and the rig visibility tests pass at every edge. Shake is a ground-plane offset, so its screen
  size grows only 14/12. Known limitation: on extreme ultra-wide windows (32:9) the depth behind
  the target falls to ~3.2 cells, so a full 3-cell lead toward the bottom would sit near the edge;
  normal lead (1.5 cells plus small follow lag) is well clear.


# Implementation (2026-11-13)

- `src/client/presentation_3d/camera.rs`: framing constants 30° / 55° / 566 units;
  `camera_vertical_fov(aspect)` (aspect-fair FOV); `conservative_ground_footprint` uses it;
  `follow_3d_camera` writes the arena projection FOV each frame (only on change). Framing tests
  replaced: twelve-cell reference target, Pulse range beside the fighter, no viewport shape sees
  more than the reference aspect (4:3 through 8:1), narrow map centers width, production 25x37 map
  follows both axes and clamps at the perimeter, resize fit/follow.
- `src/client/presentation_3d/camera_rig.rs` (ceiling comment) and `camera_rig/tests.rs`
  (projection on the test camera; ECS test that framing writes the aspect-fair FOV).
- `docs/11-art-and-presentation-direction.md`: framing contract, weapon-range exception,
  aspect-fairness rule, adoption table.
- Server, protocol, catalogs, and gameplay ranges unchanged. BRL-0180 has the new camera numbers
  (comment c-000010).

# Verification

- Focused: `cargo test --no-default-features --features client --lib presentation_3d::camera`
  22 passed (camera + camera_rig).
- `just lint` (real ripgrep first on PATH): pass after fixing two `float_cmp` findings.
- `just test`: pass, 2165 tests, 0 failed (incl. performance gates).
- `just e2e 2`: pass (1v1 roster reached Active).
- Render evidence (`scripts/v3-render-evidence.sh`, `BRAWLER_RENDER_COMBAT=1`, frames 300/480/660),
  p50/p95/p99 ms, all `result=pass`, no client panics:
  - Note: on this display a requested 1591x720 window is clamped to 1512x720 logical (3024x1440,
    2.1:1), and 2560x720 is clamped the same way, so native ultra-wide evidence was not possible;
    the wide-aspect FOV cap is covered by unit tests.
  - ~2.1:1 before -> after: theme 1 Feature Yard 1v1 16.663/17.193/18.141 -> 16.676/17.916/19.150
    (vsync 60 Hz; the first after run failed p95 at 19.78 while `just lint` compiled in parallel,
    and passed on an idle rerun); theme 2 Powderline Vault Heist 3v3 8.330/8.538/8.667 ->
    8.333/8.550/8.665; theme 3 Verdant Crossfire 3v3 8.334/8.542/8.661 -> 8.333/8.538/8.640.
  - 16:9 after (before = BRL-0152 `after-*`): theme 1 16.655/17.443/18.454, theme 2
    8.333/8.539/8.659, theme 3 8.331/8.585/8.705. 4:3 theme 3: 8.339/8.572/8.684.
  - Mesh-entity high water unchanged (381 / ~716 / ~628).
- Evidence (PNG + JSON), all under `/Users/boyd/wip/brawler/target/brl0148/`:
  before `base221-theme{1-combat,2-practice,3-practice}.txt.captures` and
  `/Users/boyd/wip/brawler/target/brl0152/after-theme{1-combat,2-practice,3-practice}.txt.captures`
  (16:9); after `after2-221-theme1-combat.txt.captures`,
  `after-221-theme{2,3}-practice.txt.captures`, `after-169-theme{1-combat,2-practice,3-practice}.txt.captures`,
  `after-43-theme3-practice.txt.captures`; candidates `cand{A,B,C}-*`.
- Capture review: fighters read about 1.7x taller with front, hair, weapon, and outline legible;
  relationship rings, contact shadows, plates (BRL-0145), and aim lanes stay readable and correctly
  placed; the Pulse aim lane ends on screen to the side at ~2.1:1; the Heist idol reads with its
  face. Plates and floating numbers are pixel-sized, so they are proportionally smaller next to the
  larger fighters (BRL-0149 will size plates for this framing). Walls lean slightly more at the
  screen edges; occlusion behind 1-cell cover grows from ~0.53 to ~0.70 cells.

# Playtest request

`just run 2` -> Practice: Feature Yard (theme 1), Powderline Vault Heist 3v3 (theme 2), Verdant
Crossfire 3v3 (theme 3), plus a 1v1 match if possible. Default window (2.21:1); then resize to
16:9 and narrower, and try a very wide short window. Look at: fighter size and face readability;
whether you can see threats in time when aiming left/right (Pulse range should fit) and up/down
(about 5-7 cells; lobs/Sticky Blomb exceed the view); motion comfort while moving and strafing with
the 1.5-cell look-ahead; the 3v3 maps now scroll horizontally (full width no longer visible); wide
windows should not show more width than the default, only less depth; overhead plates and damage
numbers relative to the larger fighters; Map Maker preview still fits.

# Learn from errors

- A render-evidence p95 failure came from running `just lint` in parallel with a vsync-bound
  capture; run frame-time evidence on an idle machine before suspecting the change.
- `--window-size` larger than the display is silently clamped by macOS; read `window_width` in the
  report before claiming an aspect was captured.
- Clippy denies `float_cmp` even in tests; compare FOVs with a tolerance from the start.
- Deferred: BRL-0181 (aim look-ahead for long-range and vertical aim under this framing).
