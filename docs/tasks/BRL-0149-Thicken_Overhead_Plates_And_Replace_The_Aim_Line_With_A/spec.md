# Outcome

Players can read health and ammo at a glance and see exactly where their weapon will reach.

# Context

- Overhead health bars are 11 px (`presentation_3d/combat/fighter_ui.rs:257`). The ammo row is 7 px. Names fall back to placeholders such as "Brawler-9A8C".
- The aim preview (`presentation_3d/combat/aim_preview.rs`) shows a thin 1 px line in captures.

# Scope

- About 2x thicker health plates with a dark outline, stroked numbers, and team-colored fill.
- Ammo segments beneath the plate for the local fighter.
- An aim lane/cone derived from the resolved loadout (range, projectile width, spread) for primary fire, plus a distinct ultimate preview.
- Distinct styling for local, ally, and enemy plates.

# Out of scope

The HUD layout redesign and font fixes (separate tickets).

# Acceptance criteria

- Aim lane geometry comes from resolved definitions. A test covers the width/range/spread mapping for each delivery family.
- Plates stay readable at the chosen camera framing in 3v3.
- Concealment rules are respected for plates.

# Verification

- Focused tests.
- `just test`.
- Native captures.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.

# Research and decision (2026-11-13)

## Findings

- Plates (`presentation_3d/combat/fighter_ui.rs`): logical-pixel Bevy UI projected above the
  fighter. Name 12.8 px, health number 15 px straddling an 11 px bar (2 px padding, so a 7 px
  fill), 76.8 px wide; ammo row 7 px. Local and ally fill are both green. Fighters are now ~80
  logical px tall at the BRL-0148 framing (720 logical px high), so the bar is a thin sliver.
  The plate bottom is the projected anchor and plates grow upward, so BRL-0144 numbers (spawned
  beside the body below the plate, z-index 124 above plates) are unaffected by a taller plate.
- Concealment: plates exist only for replicated fighters (`reconcile_fighter_overheads` despawns a
  plate when its fighter entity goes away); the server withholds concealed enemies. No change.
- Aim preview (`combat/client/preview.rs` + `presentation_3d/combat/aim_preview.rs`): already
  traces the authored delivery (projectile body sweep vs map/dynamic blockers, lob landing repair,
  melee sector, cone rays, splash shape), but draws everything as 49 pooled cuboid "corridor"
  slots. A straight lane is exactly the projectile diameter (Pulse/Scatter radius 2 -> 4 units =
  1/8 cell, the "1 px line"); lob guide 2 units, rings 3 units; melee is an outline only; all in a
  low-contrast yellow (invisible on the orange themes). Overlapping translucent slots (spread
  lanes, cone rays) stack alpha.
- Server hit semantics (body overlap): projectiles hit when their body touches a fighter body
  (radius 14), melee/cone use `sector_contains` with body padding, areas use shape intersection.

## Decision

Plates (presentation only, fixed logical px, sized for the ~80 px fighter):
- Root 128 px wide; name 15 px; health bar 100 x 22 px (2x the old 11 px) with a 2 px navy
  (`TYPE_STROKE`) outline and rounded corners; the health number (18 px Lilita One with the navy
  shadow stroke) sits centered inside the bar; fill colored by relation: local green, ally blue,
  enemy red (matching the ground rings). Local-only ammo row 100 x 9 px beneath the bar. Cold pie
  20 px beside the bar. Plate heights 42 (health) / 55 (with ammo); the upward stacking keeps
  using these heights.

Aim lane (client presentation of intent + authored geometry, never a gameplay decision):
- `preview.rs` produces pure, renderer-agnostic marks (translucent fills fanned from the fighter,
  crisp strokes) with a tone (clear / blocked / ultimate / demolition). The presentation draws all
  marks as one dynamic vertex-colored mesh (one entity instead of 49 slots), rebuilt only when the
  marks change, so overlapping fills never stack alpha.
- Straight/Sticky (Single and Spread): the fill is the fighter hit lane, the projectile path from
  the muzzle widened by projectile radius + fighter body radius on each side (Pulse 32 units = one
  cell): a fighter standing in the lane is hit. Spread draws one fan covering all deliveries plus
  one exact-width path stroke per delivery (min 3 units), so every emitted delivery and its
  blocked/clear stop still shows. Range and stop come from the existing trace.
- Lobbed/Splash: thin ground guide to the repaired landing point, filled landing area of the
  authored area radius / persistent Splash circle or oriented rectangle, crisp outline.
- MeleeArc: filled sector of authored reach and angle with outline. ConeSpray: filled fan through
  the traced rays (same rays as before) with outline.
- Areas, sectors, and cones keep the authored shape (body-overlap semantics, matching their
  effect VFX); only the straight lane adds the fighter radius because the exact 4-unit lane is
  unreadable. docs/11 records both rules.
- Targeted ultimates: distinct violet tone (demolition keeps orange-red): guide, filled area of
  the authored radius, thicker rim.
- Colors: clear = translucent white fill + near-white outline (reads on orange and grey floors),
  blocked = red. All unlit colors stay <= 1.0, far below the bloom knee (threshold 3.0). The lane is
  static, so reduced-effects/reduced-motion need no variant; it is never hidden by them.
- No new gameplay data, catalog, protocol, or dependency. Tests cover the width/range/spread
  mapping for each delivery family and the ultimate preview.

# Implementation (2026-11-13)

- `src/combat/client/preview.rs`: renderer-agnostic `AimMark` (Fill fanned from an apex with
  per-rim tones; Stroke polyline with width, open/closed) and `AimTone` (Clear / Blocked /
  Ultimate / Demolition). `aim_lane_marks` replaces `preview_primitives`: straight/sticky
  (`trace_straight_shots` keeps the existing muzzle + range body trace) fill the hit lane
  (half-width = projectile radius + fighter body radius, starting beside the fighter) plus an
  outline and one exact-width path stroke (min 3 units) per delivery; Spread fills one fan through
  every stop; Lobbed/Splash draw a guide, authored area/shape fill, and outline; MeleeArc and
  ConeSpray fill their sector (cone rays shared with the live spray via `cone_spray_rays`).
  `ultimate_area_marks` builds the targeted-ultimate preview (violet, Demolition orange-red,
  4.5-unit rim). Removed `MAX_PREVIEW_SEGMENTS` and the unused Disc primitive; the live cone-spray
  gas keeps its corridor primitives unchanged.
- `src/client/presentation_3d/combat/aim_preview.rs`: one `AimLaneVisual3d` entity (was 49 pooled
  cuboid/ring slots) with a vertex-colored dynamic mesh, rebuilt only when the marks change;
  fills first, strokes lifted 0.2 above (miter-capped joins); `NoFrustumCulling` because the mesh
  is rewritten in place. Tone colors live here (all components <= 1.0).
- `src/client/presentation_3d/mod.rs`: `aim_lane` material (white, unlit, blend, double-sided)
  replaces `preview` / `preview_blocked`.
- `src/client/presentation_3d/combat/fighter_ui.rs`: plate constants and layout per the decision
  (name 15 px; 96 x 22 px bar, 2 px navy outline, 18 px number inside; outlined 9 px ammo row;
  18 px cold pie; heights 41 / 53 px; overlap width 140 px); ally fill is now blue.
- `cone_spray.rs` (irrefutable corridor destructure), `combat/client/mod.rs` exports, tests.
- `docs/11-art-and-presentation-direction.md`: adoption table, overhead section, aim-lane
  contract (replaces the old exact-width corridor bullet).
- No server, protocol, catalog, or dependency change.

# Verification (2026-11-13)

- Focused: `cargo test --no-default-features --features client --lib combat::client` 20 passed
  (new: single straight hit lane for Pulse and Sticky, spread fan + every delivery, blocked
  stop/tone, Arc Launcher and Splash landing areas, Impact Blade and Spray sectors, ultimate tone
  and rim); `... --lib presentation_3d::combat` 87 passed (new: mesh builds fills before strokes
  with bounded miter, tones below the bloom knee and distinct, plate exists only while the
  fighter is replicated, all three relation fills distinct, bold plate geometry).
- `just lint` (real ripgrep first on PATH): pass after fixing `too_many_lines` (extracted
  `trace_straight_shots`) and `assertions_on_constants` (const block).
- `just test`: pass, 2174 tests, 0 failed (incl. performance gates).
- `just e2e 2`: pass (1v1 roster reached Active).
- Render evidence (`scripts/v3-render-evidence.sh`, `BRAWLER_RENDER_COMBAT=1`, 16:9 2560x1440,
  frames 300/480/660), p50/p95/p99 ms, all `result=pass`, no client panics. Before = BRL-0148
  `after-169-*`:
  - theme 1 Feature Yard 1v1: 16.655/17.443/18.454 -> 16.649/17.508/18.649 (vsync 60 Hz)
  - theme 2 Powderline Vault Heist Practice 3v3: 8.333/8.539/8.659 -> 8.332/8.526/8.649
  - theme 3 Verdant Crossfire Practice 3v3 (`BRAWLER_RENDER_GAME_TYPE=wipeout-3v3`):
    8.331/8.585/8.705 -> 8.335/8.582/8.717
  - Mesh-entity high water drops by the 48 retired preview slots: 381 -> 333, ~716 -> 668,
    ~628 -> 580.
- Evidence (PNG + JSON): before
  `/Users/boyd/wip/brawler/target/brl0148/after-169-theme{1-combat,2-practice,3-practice}.txt.captures`;
  after `/Users/boyd/wip/brawler/target/brl0149/after-169-theme{1-combat,2-practice,3-practice}.txt.captures`;
  first iteration (lane fill started at the muzzle, a visible wedge) `target/brl0149/a1-169-*`.
- Capture review: the Pulse lane now reads as a one-cell white band with crisp edges and a center
  path on both orange themes and the grey theme (the old yellow 4-unit line was nearly invisible on
  orange); blocked lanes turn red and stop at the wall or enemy contact; allies in the lane do not
  block. Plates are about twice as thick, the number sits inside the bar, local green / ally blue
  fills match the rings, the concealed ally in a bush keeps its plate, and the ammo row is legible.
  Upward stacking unchanged; floating numbers were not exercised in these frames.
- Not natively captured: Spread, Lobbed, MeleeArc, StickyStraight, ConeSpray, Splash lanes and the
  targeted-ultimate preview. The render harness cannot select a weapon for an existing saved
  brawler (`--weapon-preset` only seeds an empty profile and rejects preset 7); an uncommitted
  local script variant produced Pulse captures only, so they were discarded. Covered by the
  per-family unit tests and the playtest; harness gap deferred to BRL-0182.

# Playtest request

`just run 2` -> Practice on Feature Yard (theme 1), Powderline Vault Heist 3v3 (theme 2), and
Verdant Crossfire 3v3 (theme 3). In the Brawler editor try, one at a time: Pulse Sidearm,
Scatter Cannon, Arc Launcher, Impact Blade, Sticky Blomb, Spray, and Splash; also equip a targeted
ultimate (Reveal Scan or a field) and Demolition Strike. Aim with the mouse/right stick and:
- Straight/Sticky: does the one-cell lane match where shots connect? Does red read as "stops
  early" (wall or enemy), and is the center path useful or noise?
- Scatter: does the fan plus five paths read, and does it match the pellets?
- Arc Launcher / Splash: is the landing area obvious and matched by the explosion/pool size?
- Impact Blade / Spray: does the filled sector match the swing / gas?
- Targeted ultimate: is the violet (orange-red for Demolition) preview clearly distinct?
- Plates in a 3v3 brawl: health readable at a glance, ally blue vs enemy red, number inside the
  bar, plates stacking above clustered fighters, hit numbers still clear of plates; and whether
  names (still `Brawler-XXXX` placeholders, out of scope) or the plate width feel too large.
Known limitations: plates stay fixed logical pixels (do not scale with window height); a spread
pellet blocked within about a lane width of the fighter can fold the fan slightly; the lane has no
reduced-effects variant (static intent UI, never hidden).

# Learn from errors

- Mistake: the first theme-3 capture ran Feature Yard because the evidence script defaults
  `wipeout` to `wipeout-1v1` regardless of players per team. Prevention: read the previous
  ticket's report `map_recipe_id` and pass `BRAWLER_RENDER_GAME_TYPE` explicitly.
- Mistake: assumed `--weapon-preset` would select the weapon in a Practice render run; it only
  seeds an empty profile. Cost: six wasted runs. Prevention: confirm a harness flag's effect in the
  first capture's HUD before batching runs (BRL-0182 tracks the harness gap).
- Lint: `assertions_on_constants` for const-only layout checks; use a `const { }` block.
