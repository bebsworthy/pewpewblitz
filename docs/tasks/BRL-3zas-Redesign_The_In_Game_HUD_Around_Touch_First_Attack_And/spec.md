# Outcome

The in-match HUD is graphical, readable at a glance, and ready for mobile touch controls.

# Context

- `src/client/presentation.rs:16-52` spawns "HEALTH --/--" and "WEAPON --/--\nULT --" text boxes. `hud.rs` builds the timer, objective, and scoreboard as formatted strings.
- The HEALTH box duplicates the overhead plate. "ULT 69% charging" is text.

# Scope

- Bottom-right attack button and ultimate button with a radial charge fill and a ready pulse. Desktop shows key/controller hints on them.
- A bottom-left reserved zone for the touch joystick.
- Remove the HEALTH box. Ammo moves under the overhead plate.
- A top-center plate for timer, team scores, and mode objective state with team colors, for Wipeout, Hot Zone, Heist, and Zombie.
- A kill feed.
- Safe-area-aware layout that scales with resolution.
- Uses the shell UI toolkit and theme font, and follows the art direction decision.

# Out of scope

The touch input implementation itself (touch controls ticket); this ticket provides the layout and visual controls.

# Acceptance criteria

- All four modes show complete state without any text-only HUD boxes.
- Layout holds at 16:9, 2.21:1, and 19.5:9 with safe-area insets.
- The ultimate charge is readable at a glance.
- The kill feed respects concealment and relevance rules.
- Owner playtest acceptance.

# Verification

- Layout tests where feasible.
- `just test`.
- Native captures of every mode and aspect.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.


# Design (2026-11-13, implementation agent, before implementation)

## Research findings
- Today's in-match HUD: `presentation.rs` spawns the HEALTH and WEAPON/ULT text boxes, which
  `combat/client/hud.rs::update_combat_hud` fills; `hud.rs` fills a top-center timer box
  (`ReadinessHudText`) and a top-right objective box (`MatchHudText`, which also lists the team
  roster in Zombie). The VFX lab only hides the two combat boxes.
- The overhead plate (BRL-2dv1, `presentation_3d/combat/fighter_ui.rs`) already shows the local
  fighter's health bar and ammunition segments with reload progress, so the HEALTH box duplicates it.
- BRL-hcyk's `TouchControls` resource exposes the live layout and sticks. The ultimate hit circle is
  `(w - 0.62u, h - 0.58u)`, r = 0.16u (u = short side); everything else on the right half is a
  floating attack stick; the top 12% is ignored.
- Combat cues are already relevance-filtered per observer on the server
  (`authority.rs::combat_cue_subjects`): a defeat cue reaches a client only when the victim and any
  initiating fighter are visible to it. `FighterDefeated` is the canonical defeat cue (the legacy
  `Defeat` duplicates it); its target can also be a deployable. `PublicParticipantState` maps
  `fighter_network_id` to the public display name and team.
- Bevy 0.19 UI has `ConicGradient` backgrounds with hard stops, which draws a radial fill without
  custom shaders.

## Decisions
1. Shared control geometry. A pure `hud::layout::HudControlLayout::new(viewport)` (logical window
   px) owns the attack button, ultimate button and the movement-zone home position. Unit
   `c = min(short side, 480)`, so phone geometry follows the screen and desktop buttons stay
   indicator-sized:
   - attack: center `(w - 0.32c, h - 0.30c)`, r = 0.15c;
   - ultimate: center = attack center + `(-0.34c, +0.08c)`, r = 0.11c (left of attack, inside the
     right half);
   - movement home: `(0.30c, h - 0.30c)`.
   `TouchLayout` takes its ultimate hit circle from this geometry, so the visual and the hit area
   cannot drift. Touch input semantics stay as BRL-hcyk shipped them (floating attack stick on the
   rest of the right half, floating move stick on the left half); only the ultimate circle moves.
2. Module layout. `src/client/hud.rs` keeps the mode facts (score views, phase overlay,
   countdown). New focused submodules:
   - `hud/layout.rs`: pure geometry;
   - `hud/plate.rs`: top score/timer plate content (pure) and presentation;
   - `hud/kill_feed.rs`: bounded kill feed model and presentation;
   - `hud/controls.rs`: attack/ultimate buttons and the touch stick visuals.
   `touch/overlay.rs` (placeholder) is deleted; `combat/client/hud.rs` and the
   `CombatHudText`/`CombatAbilityHudText` text boxes are deleted, and the VFX lab hides the new HUD
   root instead.
3. Top plate (top center, ink plate in theme colors): local team chip on the left in the ally blue,
   opponent chip on the right in the enemy red (team 1 left when no local team), timer in the
   middle, and one objective subline.
   - Wipeout: kills per team; "FIRST TO n".
   - Hot Zone: capture % per team; EMPTY / CONTESTED / ALLY CONTROL / ENEMY CONTROL, tinted by
     the controller.
   - Heist: idol health % per team, captioned DEFEND / ATTACK.
   - Zombie: boosts (ally) and live zombies (enemy); LAST BRAWLER STANDING or CLEAR THE HORDE.
     A compact teammate strip (name, health bar, boosts, DOWN / respawn seconds) sits under the
     plate at top-left, keeping the documented Zombie roster contract.
   - Syncing shows dashes and SYNCING.
   The plate is shown in Countdown and Active, as the old boxes were.
4. Kill feed (top right, at most 4 rows, 4 s each, newest on top): killer, skull icon, victim, each
   name in local green / ally blue / enemy red. It is fed only by received `FighterDefeated` cues
   whose target is a public participant (deployables are skipped). The killer is the initiating
   fighter's public name, ZOMBIE for hostile NPCs, and omitted for environment or self defeats.
   Because the server already withholds cues about fighters the observer cannot see, the feed never
   names a concealed fighter. It clears when the match changes.
5. Controls (bottom right, both platforms):
   - Attack: crosshair icon. When ammunition is empty it dims and a conic fill shows the reload.
   - Ultimate: the family badge icon (`ui::icons::ultimate_icon`, moved from the brawler screens
     so both use one mapping) inside a ring whose conic fill is the replicated charge. When Ready
     the ring is gold, with a pulse that reduced motion disables. Active phases with an expiry show
     whole seconds left. Targeting shows a cyan ring.
   - Both dim while the local fighter is defeated.
   - On desktop each carries a key/controller hint chip from the current bindings (mouse/keyboard
     or gamepad by the active device). Hints are hidden on Android and once touch has been used.
   - Touch only (after a touch reaches gameplay): a faint movement-zone ring at the home position
     while no move finger is down, plus the floating stick base and knob for move, attack and
     ultimate drags.
6. Health and ammo stay on the overhead plate; the HEALTH and WEAPON boxes are removed. Slow and
   sentry text from the old box are dropped. The slow and cold state already have world visuals,
   and the sentry has its own health bar; the ultimate button shows the sentry's remaining seconds.
7. Positioning: plate, feed and strip are authored in UI px (scaled by `ui::viewport`). Controls
   convert window geometry into UI px with `viewport_to_ui`, the same way BRL-30za handles
   world-anchored UI.
8. Out of scope here: the Tab/menu scoreboard text panel and the waiting/completed phase panels
   (they are on-demand overlays, not the persistent HUD), plus touch pause (BRL-094c).

## Acceptance-criteria refinements
- No text-only persistent HUD box remains in any of the four modes; each mode's objective state is
  on the plate (and the Zombie teammate strip).
- Layout tests with the real HUD nodes at 1280x720 (16:9), 914x411 (2.22:1), 868x411 (phone after
  the cutout strip) and 1560x720 (19.5:9). Every HUD element must be fully on screen, and the plate,
  kill feed, Zombie strip, attack button, ultimate button and movement home must not overlap. The
  drawn buttons must match the touch hit circles.
- Ultimate charge reads as a radial fill plus a distinct Ready treatment. Unit tests cover the
  charge-to-angle and state mapping.
- Kill feed unit tests cover naming, relation colors, the deployable skip, the bound and expiry, and
  that it shows only what the relevance-filtered cue stream delivers.


# Implementation (2026-11-13)
- New `src/client/hud/`:
  - `layout.rs`: `HudControlLayout`, the pure button and movement-zone geometry.
  - `plate.rs`: `PlateState`, `plate_content`, the top plate and the Zombie teammate strip.
  - `kill_feed.rs`: the bounded `KillFeed` fed by `FighterDefeated` cues.
  - `controls.rs`: attack and ultimate buttons, radial rings, hints, touch sticks.
  - `screen_tests.rs`: headless layout tests.
- `src/client/hud.rs`:
  - `update_readiness_hud` now fills `PlateState` instead of the timer and objective text boxes.
  - A `HudRoot` node at GlobalZIndex 150 sits above the world-anchored overheads, numbers and bars (119-124) and below the phase panels (220+), scoreboard and menu.
- `src/client/touch/model.rs`: `TouchLayout` takes its ultimate hit circle from `HudControlLayout`. `touch/overlay.rs` and its plugin are removed.
- Old text HUD removed:
  - `combat/client/hud.rs` (`update_combat_hud`, `CombatHudText`, `CombatAbilityHudText`) and its test;
  - the HEALTH and WEAPON boxes in `presentation.rs` (only the scoreboard panel is still spawned there);
  - the VFX lab's `hide_gameplay_hud`.
- `ui::icons::ultimate_icon` is now shared by the brawler screens and the ultimate button.
- Bug fixed on the way: the Zombie roster read a `ResolvedFighterStats` component that fighters do not replicate, so teammate health was always unknown. It now reads the replicated `ResolvedMatchLoadout`, as the overhead plates do.
- Docs: `docs/13-player-ux.md` (touch play and the HUD contract), `docs/11-art-and-presentation-direction.md` (adoption table), `src/client/ui/README.md` (consumer row).
- Design adjustments made while verifying:
  - The HUD z-order was raised from 100 to 150. At 100, enemy overheads drew over the plate and the attack button.
  - Plate chips now use a minimum width, so "100%" no longer overflows.
  - The Heist subline is "BREAK THEIR IDOL".
  - Zombie boosts show as "+n".

# Verification (2026-11-13)
- Focused: `cargo test --features client,automation --lib -- hud:: touch:: art:: icons::` passes. New tests:
  - `hud::layout`: on screen, inside the right half, buttons apart, phone touch-target sizes.
  - `hud::plate`: observer-relative sides and tones for all four modes, plus Syncing.
  - `hud::kill_feed`: names and relation colors, NPC, environment and self defeats, deployable skip, bound and expiry, an ECS test that only delivered cues appear, cleared on a new match.
  - `hud::controls`: charge-to-fill and state mapping, reload only while empty, hints from bindings and device.
  - `hud::screen_tests`: the real HUD laid out headlessly at 1280x720, 868x411, 914x411 and 1560x720 for Wipeout and Zombie. It checks every element is on screen, no plate, feed, strip, button or movement-zone overlap, and drawn buttons equal the touch hit circles. A mutation check (moving the ultimate onto the attack button) fails the test.
  - The existing touch tests (11) pass with the moved ultimate circle.
- `just lint` (real ripgrep first on PATH): exit 0.
- `just test`: exit 0, 2254 passed, 0 failed.
- `just e2e 2`: exit 0, 1 exact 1v1 roster reached Active.
- Native desktop (windowed automation Practice with screenshots, local routed server):
  - Wipeout 3v3 at 1280x720, 914x411 and 1512x720 (1560 was clamped by the display);
  - Hot Zone 3v3 at 1280x720;
  - Heist 3v3 at 914x411 and 1280x720;
  - Zombie 3p at 1280x720 and 914x411.
  The captures show the plate, the kill feed, the reload ring, the gold Ready ring, key hints and the Zombie strip with health.
- Pixel 8a (serial 45171JEKB03025, `just android-install 192.168.31.47:5001`, server on 0.0.0.0:5001 with the scratch devdata), Practice Wipeout 2v2 on Feature Yard:
  - no key hints, movement-zone ring, plate;
  - attack drag shows the lane;
  - ultimate drag shows the violet knob and the gold rim;
  - the move stick moves the fighter;
  - violet radial charge and kill feed rows appear in play.
- Evidence (scratch, not committed): `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0155/`
  - desktop: `v2-*`, `final-*`, `phone-914-wipeout/`, `desk-1280-wipeout/`;
  - device: `device/d04-match.png`, `d06-ultimate-drag.jpg`, `d07-move.jpg`, `f4.jpg` (charge ring), `f7.jpg` (kill feed).
- Not verified: Hot Zone and Heist on the device (only desktop proxies), the Zombie strip on the device, and the targeted-ultimate Targeting look (unit-tested only).

# Playtest request (owner)
- Desktop: `just server`, then `just client` at the default size. Play Practice in each mode (Wipeout, Hot Zone, Heist, Zombie).
- Phone: run `BRAWLER_ROUTED_BIND=0.0.0.0:5001 BRAWLER_DEV_DATA_DIR=<scratch>/devdata just server`. The installed APK points at 192.168.31.47:5001. Play Practice by touch.
- What to check:
  1. Can you read the score, time and objective at a glance in every mode? Are left (your team, blue) and right (enemy, red) intuitive?
  2. Is the ultimate charge readable mid-fight? Does the gold Ready ring and pulse stand out without being distracting? Check with reduced motion on as well.
  3. On the phone: do the attack and ultimate buttons sit under your thumb? Does the ultimate touch circle match the drawn button? Do the stick visuals help?
  4. Kill feed: is it legible, and does it ever name someone you could not see?
  5. Zombie: is the teammate strip (health, +boosts, DOWN, respawn seconds) enough?
  6. Desktop: do the key hints match your bindings, and do they switch to controller labels when you use a pad?
- Known limitations:
  - At a map's top edge the local overhead can sit under the plate (BRL-fwtg).
  - A defeat whose killer or victim is not relevant to you never reaches the feed.
  - The Tab/menu scoreboard and the waiting/results panels are still text panels.

# Deferred
- BRL-fwtg (todo, parent BRL-q5c4, related BRL-3zas): keep the local fighter clear of the top plate at the map's top edge.
- The scoreboard and results panels stay with BRL-wfce and BRL-h0yc (existing tickets).
- The kill feed only shows defeats the server relays. Showing a victim-only "X DOWN" row when the killer is concealed would need a server cue change, which is out of scope.

# Learn-from-errors
- **Query conflicts are found only at schedule build time.** The first plate presenter had two overlapping `Text` queries. It compiled, and only the headless screen test exposed it (B0001). Action: every new HUD presenter gets a test that runs its real plugin.
- **The world UI z-band was not checked before choosing the HUD z.** Desktop captures showed enemy overheads over the plate. Action: grep `GlobalZIndex` and record bands when adding screen layers.
- **The pre-existing Zombie roster health was always unknown** because the HUD queried a component that is not replicated. Action: check that a component is replicated (or that another client reader uses it) before a presenter depends on it.
- **The adb daemon on this Mac dies between calls.** Use the retrying `adbr` wrapper. Screencaps can also come back truncated, so convert them to JPEG to inspect, and retake any partial frame.
- **Never `rm -rf` a computed path.** The safety check blocked it. Write captures to fresh directory names instead.


# Closeout (2026-11-14)

Moved to done under the owner's process: implementation and agent verification are complete. Owner playtest happens on done tickets; open decisions above stay recorded for that review, and changes come back as a reopen or a new ticket.
