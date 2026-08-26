# PewPew Blitz — Full UX Audit

**Date:** 2026-08-23
**Auditor:** GLM (ZCode) code-and-document audit
**Tree state audited:** working tree at `9e54efc` + uncommitted V9 M02 / V10-prep changes
  (concealment abilities, hot-zone-1v1, V10 roadmap).
**Reference framework:** `references/best-practices/Out-of-Game-UX-UI-Unified-Guidelines.md`
  (the "Unified Guidelines") cross-checked against the project's own contracts in
  `docs/13-player-ux.md`, `docs/05-gameplay-loops.md`, and `docs/11-art-and-presentation-direction.md`.
**Method:** full read of the flow-state and overlay implementation (`src/client/flow.rs`, `shell.rs`,
  `dashboard.rs`, `presentation.rs`, `hud.rs`, `build_editor.rs`, `settings/*`), the in-match HUD and
  3D presentation layer (`src/combat/client/hud.rs`, `src/client/presentation_3d/*`), and all
  recorded playtest feedback in `docs/implementation/v1`–`v9` and `docs/backlog.md`. This is a
  code-level audit; no new live playtest was performed. Where a claim rests on a recorded playtest
  rather than code, that is noted.

---

## 1. Executive summary

**Overall verdict: B−.** The shell's *information architecture and behavioral honesty are exemplary*
— arguably better than most shipped games — but its *visual craft, action hierarchy, settings
surface, and in-match feedback depth* are at a functional-wireframe level. The single biggest
structural risk is not missing features; it is that **every interactive element in the shell is the
same button**, so nothing communicates primary/secondary/destructive intent, and the single biggest
usability debt is the **Settings overlay**, which presents a console-style text dump plus a grid of
19 interleaved buttons instead of settings rows.

### Top strengths (protect these)

1. **State honesty is a real, enforced discipline.** No invented wait times, no speculative maps, no
   fake success. `SYNCING` / `Population updating` / `Updating queue` placeholders
   (`flow.rs:5626-5647,5693-5697`, `hud.rs:222-246,383`), a queue cancel button that becomes
   `CANCELLING…` while awaiting server acknowledgement (`flow.rs:5931-5937`), a rate-limited retry
   that counts down on the disabled button itself (`flow.rs:3180-3208,6520-6553`), and a replay
   button that disables with the factual reason "The previous game is not available on this server."
   (`flow.rs:5598-5617`). This is exactly what the Unified Guidelines call "the interface presents
   accepted or authoritative facts honestly," and it is consistently implemented.
2. **Draft/confirm/back semantics are uniform.** Every child surface (Game Type Select, Brawler
   Editor, Weapon Equipment, Settings) edits a local draft; Confirm commits, Back discards; pending
   mutations disable admission and further mutations until the authoritative outcome arrives
   (`flow.rs:3759`, `shell.rs:276-287`). One mental model everywhere.
3. **Focus handling is a first-class system.** Deterministic stale-focus repair, disabled targets
   skipped, scroll-into-view in Compact dashboard and long overlays, return-focus restoration to the
   invoking dashboard control (`DashboardReturnFocus`, `flow.rs:470,3757`), pointer/keyboard/gamepad
   feeding one action owner (`flow.rs:1064-1323`, `docs/13` §Dashboard). Most shipped games do not
   have this.
4. **Destructive flows are safe by construction.** Safe action gets default focus in every
   confirmation; delete names its target ("Delete {name}? This cannot be undone.",
   `flow.rs:5283-5291`); Change Server explains its consequence; the non-pausing match menu states
   "MATCH CONTINUES" in its own title (`presentation.rs:229`).
5. **A genuinely responsive dashboard.** Wide/Compact reflow at effective 1000×640 with preserved
   selection, spatial focus neighbors in Wide, visible-order focus in Compact, wheel scroll and
   focus-driven autoscroll (`flow.rs:5954-6198`) — the direct product of the V5 M01/M03 playtest
   lessons.
6. **Connecting is the model screen.** Staged progress (`STEP 1 OF 3`), animated dots, candidate
   counts, a bounded remaining time, and a focused Cancel (`flow.rs:6747-6766`) — implemented after
   the V2 M03 "Connecting looked dead" feedback and still the best waiting surface in the game.

### Top issues (fix these first)

| # | Issue | Where | Severity |
|---|---|---|---|
| 1 | No action hierarchy anywhere: primary/secondary/destructive actions are visually identical buttons; overlay buttons even reuse the *error-dialog* style (muted plum) for everything | `flow.rs:6428-6518,6555-6645` | High |
| 2 | Settings overlay is a text dump + 19-button grid with interleaved order, no rows/sliders/toggles, no visible value feedback, keyboard shortcuts dead in the product draft | `shell.rs:296-471`, `settings/ui.rs:196-355` | High |
| 3 | Disabled actions almost never show a *visible* reason (reasons exist only in `AccessibleLabel`; gray styling with unchanged white text carries the whole signal) | `flow.rs:6555-6645`, Dashboard `update_dashboard_live_facts` | High |
| 4 | Team identity in-match is color-only (blue/red rings, generic `T1`/`T2` labels); no colorblind mode — violates the project's own "important information is not color-only" principle | `presentation_3d/mod.rs:400-417`, `combat.rs:715-728`, `hud.rs:365-383` | High |
| 5 | Simulation units and internal ids leak into player text: cooldowns in ticks (`COOLDOWN 12t`), weapon stats in ticks (`Fire 24t · Slow 40%/90t`), dashboard summary as raw ids (`Profile 2 · Weapon base 3`), scoreboard weapon as `W3`, map as `Map 3` | `combat/client/hud.rs:124-127`, `flow.rs:3769-3778,5232-5263`, `hud.rs:590-592`, `flow.rs:4404` | High |
| 6 | Double error panel: a failed Settings Apply renders *two* stacked panels with duplicated buttons (flow's error overlay + shell's local error panel) | `shell.rs:849-878` + `flow.rs:3097-3105` | Bug |
| 7 | Results screen is honest but thin: outcome + score + replay only; no personal contribution, roster, or "what changed" — the guidelines' "understand what changed / my contribution" step of the loop is missing | `flow.rs:5520-5620` | Medium |
| 8 | In-match menu is keyboard/gamepad only (no mouse), uses an ASCII `>` selection marker, and resets selection on every open; Escape on keyboard simultaneously opens the menu *and* cancels ultimate targeting | `presentation.rs:155-231`, `input.rs:250-253,383-386` | Medium |
| 9 | No combat feedback layer beyond world spheres + audio: no damage direction, hit markers, kill feed, offscreen-objective indicator (deferred with evidence since V1 M09), local respawn countdown, or low-health state | `presentation_3d/combat.rs`, backlog `M09-ZONE-OFFSCREEN` note | Medium |
| 10 | Dead/legacy Build Editor overlay ships in the product binary but is unreachable outside tests; F3 diagnostics overlay installed unconditionally in the product client | `flow.rs:38-43,3216-3412`, `diagnostics/overlay.rs:98`, `client/mod.rs:620` | Low |

---

## 2. Evaluation against the Unified Guidelines

The guidelines' five statements, assessed:

> **"I know where I am."** — Pass. One connected home (Dashboard), no title screen, every overlay
> has a heading, `DespawnOnExit` prevents screen stacking. The shell never leaves you ambiguous
> about connection state (identity chip shows `SERVER: {name}  ONLINE`).
>
> **"I know what changed."** — Partial. Dashboard notices explain catalog changes ("The previous
> game is unavailable. {X} is now selected.", `flow.rs:3751-3756`), queue admission freezes and
> echoes the accepted build, and error kinds name their category. But **Results answers nothing
> about the player's own match**, and there is no post-match "what changed" because progression does
> not exist yet (acceptable per non-goals — the gap is the missing *personal* summary).
>
> **"I know what I can do."** — Pass on enabled actions, **fail on disabled ones**. Disabled buttons
> carry reasons only in `AccessibleLabel`, never as visible text. The guidelines are explicit:
> "Do not rely on gray styling alone."
>
> **"I understand the consequences of my choice."** — Partial. Game Type Select shows rules, map
> pool, topology, and live population before commit; weapon parts show signed deltas and
> equipped-by-another-brawler conflicts. But permanent creation choices show only a name
> (`LIGHTWEIGHT`, `PULSE SIDEARM`) with **no description of what the profile/base actually does**,
> and the equipment preview speaks in ticks and unexplained trait names (`Slow`).
>
> **"I can return to play when I am ready."** — Pass, strongly. Play Again / Practice Again with
> exact-replay gating, Practice always one press away, queue cancel honest, Match Loading cancel
> honest. The PLAY → RESULT → CHOICE → PLAY cycle the guidelines describe physically exists.

Player-needs scorecard (Competence / Autonomy / Relatedness / Immediacy / Trust):

| Need | Score | Notes |
|---|---|---|
| Competence | ★★☆☆☆ | No post-match learning surface; build consequences partly opaque (ticks, no profile descriptions) |
| Autonomy | ★★★★☆ | Meaningful permanent + editable choices, non-coercive navigation, no forced confirmations on trivial acts |
| Relatedness | ★★☆☆☆ | By design out of scope (no parties/social); roster visibility in scoreboard is minimal |
| Immediacy | ★★★★☆ | Short paths everywhere; Practice one press; first-run auto-opens brawler creation |
| Trust | ★★★★★ | The shell's defining strength; every waiting/pending/rejected state is honest |

Design-law spot checks:

- *Critical law 2 (Back/Cancel/Close predictable)*: Pass — per-overlay Escape mapping is safe-side
  and consistent (`flow.rs:1278-1322`).
- *Critical law 3 (essential controls usable with every input mode)*: Partial — mouse cannot operate
  the in-match menu or select settings rows; brawler-name editing is keyboard-only (no paste).
- *Critical law 6 (accessibility before it is needed)*: Partial — reduced motion/effects, UI scale,
  rebinding, Y-invert exist; colorblind support, captions, and text-size options do not.
- *High law 6 (every interaction receives immediate acknowledgement)*: Partial — button hover/press
  states exist, but there are **no screen transitions at all** (instant UI swaps), and pending
  brawler mutations render as *silent no-ops* rather than disabled controls (`flow.rs:2014-2062`).

Scope note: the Unified Guidelines govern the out-of-game shell; in-match HUD is governed by
`docs/11` readability contracts. Section 7 audits in-match UX against the project's own contracts.

---

## 3. Screen inventory and flow map

### 3.1 Primary flow states (`ClientFlow`, `flow.rs:56-66`)

| # | Screen | Product role | Implementation | Grade |
|---|---|---|---|---|
| 1 | Connecting | Initial/manual lobby connection | `spawn_connecting` flow.rs:3643; staged copy 6747 | A− |
| 2 | Server Select | Recovery + manual connection, favorites/recents | `spawn_server_select_root` flow.rs:2987 | C+ |
| 3 | Dashboard | Sole authenticated home | `spawn_dashboard` flow.rs:3717; layout 5954 | B+ |
| 4 | Game Type Select | Dashboard child draft | `spawn_game_type_select` flow.rs:4201 | B− |
| 5 | Queue | Accepted multiplayer admission | `spawn_queue` flow.rs:4317 | B |
| 6 | Match Loading | Reservation handoff + countdown gate | `spawn_match_loading` flow.rs:4351 | B− |
| 7 | Match | Authoritative gameplay + HUD + menu | `presentation_3d/*`, `hud.rs`, `presentation.rs` | C |
| 8 | Results | Completed-match decision | `spawn_results` flow.rs:5520 | C+ |

### 3.2 Overlay surfaces (`ClientOverlay`, `flow.rs:82-97`)

| # | Overlay | Purpose | Implementation | Grade |
|---|---|---|---|---|
| 9 | Settings | Local settings (input/display/audio) | `shell.rs:296-471` + `settings/*` | D+ |
| 10 | Credits | Attribution | `shell.rs:473-493` | B |
| 11 | Dashboard Menu | Connected utilities hub | `present_dashboard_menu` flow.rs:5346 | C |
| 12 | Brawler Creation | Permanent-facts creation | flow.rs:4660 | B− |
| 13 | Brawler Editor | Mutable edit draft | flow.rs:4775 | B− |
| 14 | Weapon Equipment | 4-slot part equipment draft | flow.rs:4916 | B− |
| 15 | Delete Brawler Confirmation | Destructive gate | flow.rs:5266 | B |
| 16 | Cancel Match Start Confirmation | Destructive-ish gate | flow.rs:4423 | B− |
| 17 | Leave Match Confirmation | Destructive gate | flow.rs:4484 | B− |
| 18 | Change Server Confirmation | Membership-change gate | flow.rs:4540 | B |
| 19 | Error overlay | Contextual recovery | flow.rs:3090 | B− |
| 20 | Match Complete | Non-interactive result bridge | flow.rs:5472 | B |
| 21 | Build Editor (legacy) | Unreachable outside tests | flow.rs:3216 | F (dead code) |

In-match sublayers (not navigation destinations): countdown/phase overlay, objective score, health
panel, ability panel, overhead fighter UI, scoreboard, match menu, diagnostics overlay.

### 3.3 Flow map (as implemented)

```mermaid
flowchart TD
    Launch([Launch]) --> Connecting
    Connecting -- accepted --> Dashboard
    Connecting -- "cancel / bounded failure" --> ServerSelect[Server Select]
    ServerSelect -- connect --> Connecting

    Dashboard -- "brawler card (has brawler)" --> DashboardMenu
    Dashboard -- "brawler card (empty)" --> Creation[Brawler Creation]
    DashboardMenu -- create/edit/delete/equip --> Creation & Editor[Brawler Editor] --> Equipment[Weapon Equipment]
    DashboardMenu -- credits --> Credits
    DashboardMenu -- change server --> ChangeServerConfirm --> ServerSelect
    Dashboard -- "Change Game" --> GameTypeSelect[Game Type Select]
    GameTypeSelect -- "Confirm / Back" --> Dashboard
    Dashboard -- "Play accepted" --> Queue
    Dashboard -- "Practice accepted" --> MatchLoading
    Queue -- reservation --> MatchLoading
    Queue -- "cancel acked" --> Dashboard
    MatchLoading -- cancel-confirm --> Dashboard
    MatchLoading -- countdown --> Match
    Match -- "in-match menu (non-pausing)" --> MatchMenu[Match Menu sublayer]
    Match -- "authoritative completion" --> MatchComplete[Match Complete bridge]
    MatchComplete --> Results
    Match -- "confirmed leave / failure" --> Dashboard
    Results -- "Play/Practice Again" --> Queue / MatchLoading
    Results -- Dashboard --> Dashboard
    Dashboard & GameTypeSelect & Queue & Results -- "lobby lost" --> ServerSelect
```

The implemented graph matches the canonical `docs/13` mermaid exactly; no rogue screens or competing
hubs were found. Depth is shallow (max 4 levels: Dashboard → Menu → Editor → Equipment), matching
the guidelines' shallow-shell guidance.

### 3.4 Z-order / stacking (implemented)

`WeaponEquipment (z520) > creation/editor/delete (510) > menu/error/confirmations/shell (500) >
build editor (480) > match completion (450) > flow screens (410) > dashboard bg (408) > gameplay
world`. Coherent; nothing incorrectly covers the equipment overlay. (From `flow.rs` root spawns.)

---

## 4. Global findings

Each finding cites evidence and a concrete recommendation. Findings marked **[BP]** indicate a
Unified Guidelines clause the implementation currently violates.

### G-01 — One button to rule them all: no action hierarchy or intent styling [BP] (High)

`spawn_flow_button` and `spawn_flow_error_button` are the only two button generators in the shell
(`flow.rs:6428-6518`). The "error" variant (muted plum background, 92% width) is used for **every**
overlay button — confirmations, dashboard menu, creation, editor, equipment — so `DELETE BRAWLER`
and `CREDITS` are pixel-identical. The only hierarchy signal in the entire shell is the Dashboard's
orange primary Play button (`flow.rs:6330-6334`), which proves the team can do this.

**Recommendation:** introduce three button intents in the shared generator — `Primary` (filled,
warm accent), `Secondary` (current dark panel), `Destructive` (red-tinted border or fill) — plus a
`Safe` variant for confirmations' index-0 action. Apply: Dashboard Menu (DELETE/QUIT destructive),
all confirmations, editor SAVE (primary) vs CANCEL (secondary), creation CONFIRM (primary), error
RETRY (primary). This is a styling-only change to `flow_button_background`/`border` plus intent
parameters at call sites, and it does not touch flow logic.

### G-02 — Disabled actions don't explain themselves visibly [BP] (High)

Disabled styling is a flat background with **no border and unchanged white text**
(`flow.rs:6555-6645`) — the guidelines explicitly reject gray-alone. Real reasons exist but are
hidden in `AccessibleLabel` only ("Match in progress; Play unavailable",
"Joining match; Play unavailable", `flow.rs:5733-5744`). The Game Type Select CONFIRM disables
silently when no draft choice was made; Weapon Equipment SAVE disables with only the string
`INVALID PART COMBINATION` in the preview line; the brawler cap ("Brawler limit reached (16).")
surfaces as a dashboard notice *after* leaving the menu instead of on the disabled control.

**Recommendation:** add a small caption slot under (or beside) every disabled action with the reason
and remedy, exactly like the rate-limit button already does ("TRY AGAIN IN {x.x}s"). At minimum,
give disabled buttons reduced-opacity text + dashed border so they read as inert, and put the
profile-empty and capacity reasons visibly on the Dashboard Play/Practice buttons, not only in
accessibility labels.

### G-03 — Simulation units and internal ids leak into player copy (High)

Examples: `COOLDOWN 12t`, `RELOADING 24t` in the ability HUD (`combat/client/hud.rs:124-127`);
weapon preview `Capacity 12 · Damage 9 · Fire 24t · Refill 90t · Reach 420 · Slow`
(`flow.rs:5254-5262`); part effects `fire interval -3t +10%`, `Slow 40%/90t` (`flow.rs:5192-5230`);
dashboard build card `Profile 2 · Weapon base 3 · 3 of 16 saved` (`flow.rs:3769-3778`) even though
human names exist and are used two overlays away (`fighter_profile_name`, `flow.rs:4620-4657`);
scoreboard `W3`/`W?` weapon codes (`hud.rs:590-592`); Match Loading `Map {preset_id.0}`
(`flow.rs:4404`) while Game Type Select shows real map display names. The legacy build editor even
humanized ticks ("approximately 1.5s", `build_editor.rs:668-686`) — the new surfaces regressed.

**Recommendation:** add one shared formatting module (ticks→seconds at 60 t/s, ids→display names,
milliunits→"units") and route every player-visible string through it. This also fixes the
"/12 points" budget question on Match Loading (`flow.rs:4400-4406`): docs/13 says the V7 flow does
*not* carry the 12-point budget, so either the denominator is stale copy or the budget survives —
verify and align.

### G-04 — Team identity is color-only in-match, with no colorblind path [BP] (High)

Fighter bodies/rings/names use blue vs red hue only (`presentation_3d/mod.rs:400-417`,
`combat.rs:715-728`); score lines read `T1 3 — 2 T2` (`hud.rs:365-383`). The project's own spec
(`docs/13`: "non-color-only team identification and a colorblind-friendly palette direction") makes
this a contract gap, not just a polish item; it is tracked in `CAND-RELEASE-READINESS`.

**Recommendation:** cheapest robust fix: differentiate team *shape* language — e.g., Team 1 fighters
get a solid ring, Team 2 a dashed/notched ring; add a small team glyph next to score numbers; offer
a colorblind palette setting that shifts to blue/orange + pattern. Keep the frozen
green=self / blue=ally / red=enemy relation hues as the base layer.

### G-05 — No shell motion or celebration budget; screens swap instantly (Medium)

Only two motion behaviors exist in the entire shell: the Settings panel's 0.16 s entrance
(`shell.rs:1225-1245`) and the animated Connecting dots. Navigation between flow states is a hard
cut. Match Complete — the one moment the guidelines would rate "celebratory" — is a static text
panel. There is no motion *problem* (nothing is excessive), but there is also no motion
*explanation*: overlays pop in without fade, which reads as flicker on slower displays and gives no
causal link between "I pressed PLAY" and "the queue appeared."

**Recommendation:** a single reusable 120–160 ms fade/slide for overlay enter (faster, ~80–100 ms,
on exit), honoring the existing `reduced_motion` flag; a restrained victory treatment on Results
(wordmark scale-in or a color wash, skippable by input). Do not add ambient motion beyond this.

### G-06 — Text register is inconsistent and some markers are ASCII-level (Low)

ALL-CAPS button labels, sentence-case body, `Label: value` fragments, `" · "` vs `" - "` vs `" | "`
separators all coexist (worst in the equipment overlay and game-type buttons
`flow.rs:4276-4287,5101-5109`). The in-match menu marks selection with a literal `>` character and
alignment spaces (`presentation.rs:216-227`); text fields render a `|` caret
(`render_editor_value`, `flow.rs:6768-6775`). The V5 playtest already established the display/body
typography roles (Lilita One + Fira Mono) — the flow overlays mostly ignore them (default font,
`spawn_heading`).

**Recommendation:** one copy style guide pass: heading font for titles, body font for facts, single
separator glyph, and replace ASCII selection markers with the existing focus-border visuals the
flow buttons already have.

### G-07 — Silent no-ops for guarded actions (Medium)

Creation/edit/equipment/delete actions return early without effect while a queue/practice/profile
operation is pending (`flow.rs:2014-2021,2054-2062,2107-2114,2263-2270`). To the player this is a
dead button with no feedback. The Dashboard correctly disables its cards during the same states
(`flow.rs:3954-3996`) — the overlays just don't.

**Recommendation:** when guards are active, either disable the entry control that leads to the
overlay, or render a small "waiting for server…" line in the overlay header. The state is already
computable from the same resources the guards read.

### G-08 — Mouse parity gaps [BP] (Medium)

The in-match menu cannot be clicked (no `Button`/`Interaction` entities, `presentation.rs:155-231`);
settings values are plain text, so pointer users must walk a 19-button grid; brawler-name editing
has no paste (server-select address editing does, `flow.rs:1157-1171`) and no click-to-position
caret; weapon-equipment slot cycling is button-per-press with no drag/drop or click-to-choose-list
pattern. The guidelines require equivalent intentions across input modes.

**Recommendation, ordered by value:** (1) make the four match-menu rows real flow buttons (the
infrastructure exists); (2) settings rows as buttons (see §6); (3) allow paste in the brawler name
editor by reusing the existing arboard path; (4) leave caret/paste asymmetry documented as a
keyboard-first surface otherwise.

### G-09 — Audio is five reused clips with no mix control players can reach meaningfully (Medium)

Five `.ogg` one-shots; dash is the defeat clip at 1.45×, sentry is ready at 0.75×
(`audio.rs:100-121`); no music/ambience; volume changes don't preview until APPLY (unlike display
settings which preview immediately, `shell.rs:1134-1154` vs `1190-1219`); no focus-loss mute for
*sounds* (backlog says focus-loss audio policy is resolved — only the frame-rate policy is,
`client/mod.rs:699-707`). Most of this is known (`GAP-AUDIO-SETTINGS` under
`CAND-RELEASE-POLISH`).

**Recommendation:** when audio polish is promoted: separate SFX/music categories with live-preview
volume, focus-loss mute implementing the already-documented policy, and at least distinct
fire/impact/dash/defeat identities rather than pitch-shifted reuse. Near-term, make the volume
buttons preview live — it is a two-line change to apply from the draft instead of active settings.

### G-10 — Diagnostics and legacy surfaces ship in the product binary (Low)

The F3 diagnostics overlay is installed unconditionally (`client/mod.rs:620`,
`diagnostics/overlay.rs:98`); the legacy Build Editor overlay (~500 lines + constants) is
unreachable outside tests (`flow.rs:3216-3412`, only test writers set
`ClientOverlay::BuildEditor`); the dev build-selection overlay is spawned in the product HUD tree
and merely hidden at runtime (`presentation.rs:78-94` vs `session.rs:1165-1170`).

**Recommendation:** feature-gate diagnostics behind a debug feature or the existing
`BRAWLER_DIAGNOSTICS_OVERLAY` env; delete the legacy Build Editor overlay and its index constants
in a cleanup milestone; spawn the build-selection overlay only in automation configurations.

### G-11 — The guidelines' "collection findability" layer is absent, and that's currently correct

With ≤16 brawlers and a handful of parts, tabs/filters/search would be over-engineering (the
project's own no-over-engineering rules and the guidelines' "scale discovery with collection size"
both say so). The current `SELECT NEXT BRAWLER` single-cycle button in the Dashboard Menu is the
weakest form, though: with 16 brawlers, cycling to #14 is painful.

**Recommendation:** when brawler count actually grows past ~6 in playtests, replace cycling with a
simple vertical list overlay (name + profile + weapon base per row, selected row marked) — reuse
the Weapon Equipment list pattern. Not before.

### G-12 — Onboarding exists only as "auto-open creation once" [BP] (Medium)

The empty profile auto-opens Brawler Creation (good, `flow.rs:4604-4618`), and creation explains
permanence. But nothing teaches controls: no control help anywhere in the product UI (README only),
no first-match hints, and the ready prompt is the only in-match instruction (`PRESS SPACE / ENTER /
A TO READY`, `hud.rs:513`). The guidelines require teaching an intention at the moment of value.

**Recommendation (small, honest):** a one-time "controls" card on the first Match Loading or first
Waiting phase (move/aim/fire/ultimate/menu, with current bindings from settings, auto-dismissed by
any input or 8 s), plus a `CONTROLS` row in the in-match menu and Dashboard Menu that shows the
same card. Reuse binding data; do not build a tutorial system.

### G-13 — Double error panel on Settings Apply failure (Bug)

Shell spawns its own "LOCAL SETTINGS ERROR" panel (`shell.rs:849-878`) *and* sets
`ClientOverlay::Error` with `return_flow: Dashboard`; because the flow state is Dashboard,
`present_flow_error_overlay` independently renders a second full-screen z500 panel for the same
error (`flow.rs:3097-3105`). Two stacked modal panels with duplicated RETRY SAVE / CONTINUE WITHOUT
SAVING buttons.

**Recommendation:** pick one owner — simplest is to have shell stop rendering its own panel when it
raises the flow error (or give the flow overlay an origin marker so it suppresses the shell panel).
Add a regression test asserting exactly one error root per error.

### G-14 — Escape double-duty during ultimate targeting (Bug-ish)

On keyboard, the pause binding also sets `cancel_pressed` (`input.rs:250-253`), so opening the menu
while a targeted ultimate is armed simultaneously opens the menu *and* cancels targeting
(`input.rs:383-386`). One keypress, two state changes, neither explained.

**Recommendation:** when targeting is active, let Escape consume the press for targeting-cancel
only (mirroring the pad, where East cancels targeting and Start opens the menu). Verify against the
V9 M02 two-phase targeting acceptance notes.

### G-15 — Responsive behavior is dashboard-only

Only the Dashboard has Wide/Compact reflow. Every other screen is a centered column with
percent/max-width caps, which degrades acceptably at small sizes but was, per V5 evidence, never
visually verified below the dashboard contract. Overlays fix their max-widths (620–900 px) and
scroll only where explicitly implemented (equipment, build editor, settings, server select via root
scroll). Queue/Game Type/Results roots do scroll (`flow_root_node` sets `Overflow::scroll_y`,
`flow.rs:5939-5952`), so small windows are functional — the gap is *verified comfort*, not
capability.

**Recommendation:** add the compact window size to the existing visual-check matrix for the four
highest-traffic non-dashboard screens (Game Type Select, Equipment, Queue, Results) rather than
building new layout classes.

---

## 5. Screen-by-screen audit

Grades: A exemplary · B good · C functional with notable issues · D weak · F remove.

### 5.1 Connecting — Grade A−

**Purpose.** Initial or manual connection progress; Cancel/Settings/Quit available.
**Implementation.** `spawn_connecting` (`flow.rs:3643-3709`); stage copy `connection_presentation`
(`flow.rs:6747-6766`); wordmark/logo asset with text fallback; Cancel focused (index 0); two-phase
DNS/timeout deadlines (`flow.rs:36-37,2751-2763`).
**Presentation.** Logo (62% width, max 560 px) over a bordered status panel; 22 px stage text; three
buttons; a 14 px input hint (`ESC / PAD EAST - CANCEL`).
**Quality.** The best waiting surface: honest stages, animated dots, bounded time, candidate
progress ("Candidate 2 of 3"), immediate cancel. Born from the V2 M03 "looked dead" playtest.
**Issues.** None material. Minor: the hint mentions only CANCEL while two other buttons exist; no
auto-retry of the *next* candidate is communicated (it happens, via candidate cycling) — one line
like "trying next address…" would explain the jumps.
**Improvements.** (1) Make the hint generic: "ENTER / SOUTH — SELECT · ESC / EAST — CANCEL". (2)
When a candidate fails and another follows, show "Trying next address…" so the address/candidate
count change is causal.

### 5.2 Server Select — Grade C+

**Purpose.** Recovery + manual connection; address/name entry, favorites, recents.
**Implementation.** `spawn_server_select_root` (`flow.rs:2987-3052`); custom caret editor
(`flow.rs:2638-2696`); paste via arboard (address/name only, `flow.rs:1157-1171`); validation before
connect (`validate_target`, `flow.rs:2698-2706`); inline error text at the bottom.
**Presentation.** Heading, two field buttons (values rendered with `|` caret), CONNECT (default
focus), then a flat alternating list of `JOIN {name} - {address}` / `REMOVE {name}` pairs per
favorite, then `RECENT …` rows, then SETTINGS/QUIT. One plum-ish button style for everything.
**Quality.** Functionally complete and honest (invalid input → inline red message; no fake
progress). The list model is the weak point: every favorite costs *two* rows (join + remove), so
focus order ping-pongs between "do the thing" and "destroy the thing" — with remove firing
**immediately, no confirmation** (`flow.rs:1899-1912`), only focus-repaired afterwards. A mis-press
on D-pad down + South deletes a saved server.
**Issues.**
- Destructive REMOVE has no confirm/undo and sits directly beneath its JOIN sibling.
- No visual grouping (favorites vs recents are distinguished only by row-label prefix).
- The caret editor is keyboard-shaped: pointer users click the field then must type; no
  click-to-place caret; the field buttons render values but look identical to action buttons.
- No empty-state guidance ("no favorites yet — connect and add one from the Dashboard menu").
**Improvements.**
1. Restructure rows: one selectable row per server (name, address, favorite star), with JOIN as the
   row action and REMOVE moved behind a confirm or an undo toast (the guidelines' "destructive but
   recoverable → confirmation *or* undo").
2. Group headers ("FAVORITES", "RECENT") as non-focusable labels.
3. Field rows styled as input fields (inset background, monospace value), not as buttons.
4. After a successful join, the recent list updates — surface "joined" feedback on the row rather
   than silently reordering recents (stability of spatial memory).

### 5.3 Player Dashboard — Grade B+

**Purpose.** Sole authenticated home: selected brawler + game type, Play, Practice, utilities.
**Implementation.** `spawn_dashboard` (`flow.rs:3717-4154`) + live-facts updater (`flow.rs:5656-5808`)
+ Wide/Compact layout applier (`flow.rs:5954-6117`) + 3D fighter preview and animated background
(`dashboard.rs`, offscreen camera on layer 29, frozen under reduced motion `dashboard.rs:166-172`).
**Presentation.** Header: wordmark, identity chip (name + `SERVER: {name} ONLINE`, green), spacer,
compact SETTINGS/MENU icon buttons (labels hidden in Compact). Center: large fighter preview
(clickable → manage/create) above a light build card (name, summary, "MANAGE BRAWLERS" affordance).
Action row: mode card (game display name, rules summary, map pool, population), PRACTICE (secondary,
24 px), PLAY (orange filled, 38 px, shadow) — the one place hierarchy exists. Notice line (amber)
under header for catalog changes. Disabled states: `InteractionDisabled` inserted during pending
admissions; label text switches to `JOINING...`/`MATCH IN PROGRESS`.
**Quality.** This screen received the V5 M01 playtest overhaul and it shows: real hierarchy, real
preview, restrained atmosphere, live facts updated in place. Wide/Compact is genuinely responsive
with focus repair and autoscroll. The empty-profile path is well handled (card becomes "CREATE YOUR
FIRST BRAWLER"; creation auto-opens once).
**Issues.**
- Build summary uses raw ids ("Profile 2 · Weapon base 3") while names exist (G-03).
- Disabled Play/Practice reasons are invisible (G-02); worse, the visible label still says "PLAY"
  while the a11y label says "Match in progress; Play unavailable".
- The build card and preview both trigger the same Dashboard Menu overlay — fine after the V7
  pointer fix, but the card's "MANAGE BRAWLERS" copy promises management while the preview has no
  label until focus/hover; two identical-target controls with different affordances.
- "Population updating" can sit for a while with no age indicator (honest, but a subtle
  "as of {n}s ago" or spinner-equivalent would be kinder).
- No route to brawler *editing* other than Menu → EDIT SELECTED BRAWLER — the natural "edit this
  build" expectation on the card opens the generic menu first.
**Improvements.**
1. Human ids in the summary; include ultimate name (one line) so the card previews the loadout.
2. Visible disabled captions (G-02) and keep the visible label in sync with the reason.
3. Consider a two-action card: primary = select/manage, secondary icon = edit (opens editor
   directly). Preserves the V7 "complete management flow" lesson without a nested hub.
4. Add the game-type population fact to Compact too (it's present — verify it survives the
   narrower card without truncation at 1000×640 exactly).

### 5.4 Game Type Select — Grade B−

**Purpose.** Dashboard child; edits one local draft from the advertised catalog.
**Implementation.** `spawn_game_type_select` (`flow.rs:4201-4311`); draft selection via
`SelectGameTypeDraft(index)`, Confirm/Back (`flow.rs:2519-2534`); per-game population labels.
**Presentation.** Heading; identity line; optional amber "previous game no longer available" notice;
one full-width button per game with a dense pipe-separated line
(`First Blood | Wipeout | 2v2 | Crossroads Facility, Tidal Garden | first to 5; 180s limit`);
a 15 px population line under each; CONFIRM (disabled until a draft choice) and BACK.
**Quality.** Honest, information-complete (mode, topology, maps, rules, population), cheap to scan
at the current catalog size (≤4 entries). Draft semantics correct.
**Issues.**
- **Focus vs draft ambiguity:** navigation focus starts at row 0 (`flow.rs:4212`) while the draft is
  unselected; a focused-but-not-chosen row looks nearly identical to a chosen one, and CONFIRM is
  disabled with no visible reason. A player who presses Enter on the focused row *does* select it
  (activate = SelectGameTypeDraft), but before that first press the state is unreadable.
- The pipe-separated single line is doing four jobs; wrapping varies by window width.
- No mode *explanation* (what is Hot Zone?) beyond the rules string — acceptable for now, one
  sentence would help first-timers.
**Improvements.**
1. Pre-select the current committed game in the draft (it's known) so CONFIRM starts enabled and the
   current row shows the selected style — removes the empty-draft state entirely.
2. Split each row into title line + detail line (mode · topology · maps · rules) instead of pipes.
3. Add the one-line mode blurb from the catalog when it exists.

### 5.5 Queue — Grade B

**Purpose.** Accepted admission; frozen ticket + honest pool facts; cancellable.
**Implementation.** `spawn_queue` (`flow.rs:4317-4348`); membership text (`flow.rs:5810-5874`);
cancel button with pending presentation (`flow.rs:5876-5937`).
**Presentation.** Heading "QUEUE"; 20 px centered block: game name, `{n} waiting · {m} players per
match`, `Build: {name} · {points} points`, `{ultimate} · {passive1} / {passive2}`; one CANCEL QUEUE
button that becomes disabled `CANCELLING…` while the server acknowledges.
**Quality.** Exactly matches the spec's honesty contract (no wait estimates, no roster). The cancel
pending-state is the correct acknowledgement pattern. The frozen build echo is good trust-building.
**Issues.**
- With one action on screen, the guidelines suggest the primary action during admission may become
  Cancel — it's visually identical to every other button (G-01).
- Build echo shows preset name or "Custom" via `source_build_preset_id` — V7 brawlers may not have
  one; "Custom" plus points-only omits the weapon parts that actually distinguish the loadout.
- Population freshness shows "Updating queue" but there is no age/refresh cadence communicated.
**Improvements.**
1. Give CANCEL the primary style (it is the only action; per guidelines the escape action should not
   wander).
2. Echo the brawler name + ultimate + part names (up to 4) instead of the legacy preset name.
3. Optional calm "looking for players…" pulse animation on the population line (respecting reduced
   motion) so the screen doesn't read as frozen.

### 5.6 Match Loading — Grade B−

**Purpose.** Reserved-match handoff: worker connect, map sync, readiness, countdown gate.
**Implementation.** `spawn_match_loading`/`update_match_loading` (`flow.rs:4351-4420`); phase
lifecycle from the lobby model; cancel via confirmation overlay.
**Presentation.** Heading; 20 px status block: phase name ("Reserving roster" … "Waiting for
players"), `{n}v{n} · Map {id}`, `Your accepted build: {points}/12 points`; CANCEL MATCH START
button (opens confirmation).
**Quality.** Phases update live; cancellation races resolve server-side before the UI changes
membership — the honesty contract holds. The confirm-gated cancel is appropriately deliberate.
**Issues.**
- Raw map id "Map 3" (names exist) and the "/12" budget question (G-03).
- No progress *shape*: seven phase strings of unknown relative length; "Waiting for players" can be
  the long pole with no roster/quorum shown, while the in-match Waiting phase *does* show a ready
  quorum (`hud.rs:506-518`). Inconsistent depth for the same waiting problem.
- No participant list — fine per privacy contract, but a quorum count (`{ready}/{total} checked in`
  if the server exposes it) would mirror the in-match pattern honestly.
**Improvements.**
1. Map display name; verify/repair the `/12` copy.
2. Show the checked-in quorum during WaitingForPlayers if available; otherwise a steady,
  non-speculative "waiting for other players" sub-line.

### 5.7 Match Complete (bridge) — Grade B

**Purpose.** Non-interactive result preservation while the fresh lobby session is established.
**Presentation/Implementation.** Full-screen 96%-opaque panel, 38 px heading, 28 px result line
(`TEAM {n} WINS` / `DRAW` / forfeit variant), grey "RETURNING TO LOBBY…" (`flow.rs:5472-5517`).
**Quality.** Does exactly what the spec says and nothing else; correct z-order below overlays.
**Issues.** It flashes between two static screens (Match end overlay → this → Results) with no
continuity; the result wording here (`TEAM 1 WINS`) differs from Results (`VICTORY`/`DEFEAT` from
the local perspective) — a small dissonance.
**Improvements.** Use the local-perspective outcome word here too (or suppress this bridge's result
line when the transition is fast); a 150 ms fade would make the three-beat ending read as one
sequence (G-05).

### 5.8 Results — Grade C+

**Purpose.** Completed-match decision: authoritative outcome + exact replay or Dashboard.
**Implementation.** `spawn_results` (`flow.rs:5520-5620`); replay gated on exact game-type id present
in the fresh lobby catalog at the current generation (`flow.rs:5531-5540`); Practice/Multiplayer
purpose switches the label; disabled replay shows the factual reason.
**Presentation.** Heading "RESULTS"; 30 px outcome; grey game name; `YOU — T{n}`; 22 px final
score; amber replay-unavailable reason; PLAY/PRACTICE AGAIN (index 0) and DASHBOARD.
**Quality.** The replay gate is the best-in-class honesty pattern (fresh revisions, revalidated
build, factual disable). Outcome localization to the local team is correct.
**Issues.**
- **Thin.** No personal contribution (defeats, damage, objective time), no team rosters, no match
  duration — the guidelines' Results contract ("the player's relevant contribution or learning") is
  unmet even though the scoreboard already computes per-player status.
- `YOU — T1` uses the generic team label (G-04 adjacency).
- No celebration whatsoever for a win (G-05) — VICTORY is 30 px of white text.
**Improvements.**
1. Add the final scoreboard (already-rendered rows: name/team/status) as a static block — data
   exists in the result context or the cached roster.
2. Add 2–3 personal facts (defeats inflicted, damage dealt, time alive) once the telemetry
   aggregates exist; keep it one line, not a stats screen.
3. Give VICTORY/DEFEAT distinct treatments (color + wordmark scale, skippable) — the one place
   celebratory motion is warranted.
4. Keep PLAY AGAIN as index 0 focus — correct now; just give it the primary style.

### 5.9 Match (gameplay), HUD, menu, scoreboard — Grade C (see §7 for the deep dive)

**Summary of grade drivers.** Strong honesty layer (SYNCING states, generation-gated clock), good
self-identification (green ring + name + ammo pips only for self), but: color-only teams (G-04),
tick units in the ability HUD (G-03), no damage-direction/hit-marker/kill-feed/offscreen-objective
feedback (G-09), pause menu not mouse-operable with ASCII selection markers (G-08), scoreboard
shows raw weapon codes and no stats, Escape double-duty (G-14), and no on-screen control help
(G-12).

### 5.10 Dashboard Menu — Grade C

**Purpose.** Connected utility overlay: brawler management, credits, favorite, change server, quit.
**Implementation.** `present_dashboard_menu` (`flow.rs:5346-5469`); conditional rows (brawlers exist?
runtime lobby target?).
**Presentation.** Narrow panel (max 430 px), up to nine identical buttons: CREATE BRAWLER (PERMANENT
PROFILE + BASE), SELECT NEXT BRAWLER, EDIT SELECTED BRAWLER, DELETE SELECTED BRAWLER, CREDITS,
FAVORITE/REMOVE FAVORITE SERVER, CHANGE SERVER, QUIT, BACK.
**Quality.** Correct gating (favorite only with a real runtime target — the V5 M02 fix) and correct
z-order. But this is where G-01 bites hardest: DELETE and QUIT are visually indistinguishable from
CREDITS; the permanent-choice warning lives inside a parenthetical button label; and brawler
selection is a single-cycle button (fine at 1–3 brawlers, poor at 16).
**Improvements.**
1. Destructive intent styling for DELETE/QUIT (G-01); move QUIT to the bottom, separated.
2. Group the list: BRAWLERS (create/select/edit/delete) · SERVER (favorite/change) · GAME (credits,
   quit). Small group headers, non-focusable.
3. Replace `SELECT NEXT BRAWLER` with a brawler list overlay when count grows (G-11).
4. Drop "(PERMANENT PROFILE + BASE)" from the label — the creation overlay already explains
   permanence properly; the menu label is spec-speak.

### 5.11 Brawler Creation — Grade B−

**Purpose.** Permanent-facts creation (profile, weapon base) + starting ultimate.
**Implementation.** `present_brawler_creation` (`flow.rs:4660-4768`); auto-opens on empty profile;
draft cycles profile (3) / weapon (4) / ultimate (4); name auto-generated "Brawler {n}"; passives
fixed; server confirms.
**Presentation.** Amber permanence warning, `FIELD: {value} [PERMANENT]` rows, a "can be changed
later" note, CONFIRM/CANCEL. Panel styled like the editor family.
**Quality.** Controller-complete without text entry (spec-compliant); permanence is stated twice
(warning + row markers) — appropriately deliberate for an irreversible choice.
**Issues.**
- **No descriptions of what profile/weapon base/ultimate choices actually do.** `LIGHTWEAT` vs
  `REINFORCED`, `PULSE SIDEARM` vs `ARC LAUNCHER` are names only; the player's first consequential
  decision is uninformed (violates the guidelines' "consequential choices are deliberate and
  understandable" critical law).
- Cycling is one-step-per-press (3–4 presses worst case; acceptable) with no wraparound display of
  where you are in the cycle (e.g., "1/3").
- The 16-brawler cap is not discoverable here (surfaces as a dashboard notice after the fact).
- Silent no-ops while ops pending (G-07).
**Improvements.**
1. One-line description under each cycled row from the catalogs (they carry stats; reuse the weapon
   preview formatter — humanized per G-03).
2. `n/total` position hint on each cycle row.
3. Show "X of 16 brawlers" in the overlay header; disable CONFIRM with a visible reason at cap
   before the server round-trip.

### 5.12 Brawler Editor — Grade B−

**Purpose.** Edit name, ultimate, two passives; entry to equipment.
**Implementation.** `present_brawler_editor` (`flow.rs:4775-4889`); inline caret name editing with
keyboard events (`flow.rs:1090-1146`); passive conflict auto-resolution (`flow.rs:2176-2198`);
Save = full revision-bound profile mutation.
**Presentation.** Amber `PERMANENT: {profile} · {weapon base}` line (good — permanent facts
read-only and labeled), NAME/ULTIMATE/PASSIVE rows, inline red validation, WEAPON EQUIPMENT, SAVE,
CANCEL.
**Quality.** Permanent-vs-editable separation is textually clear; draft/confirm correct; name
validation messages are precise ("Invalid name: {error}").
**Issues.**
- Name editing: no paste (G-08), caret is a literal `|` (G-06), no mouse caret placement, 64-byte
  cap error is jargon ("Name exceeds this field's bounds").
- Passive cycling **silently changes the other slot** to avoid duplicates (`flow.rs:2176-2198`) — a
  surprise mutation with no message.
- No explanation of what passives/ultimates do (same as creation).
- Silent no-op guards (G-07).
**Improvements.**
1. Message the auto-swap ("Passive 2 changed to {x} — each passive can be equipped once").
2. One-line descriptions under ultimate/passive rows.
3. Reuse the arboard paste path; humanize the length error ("Names are limited to 64 characters").

### 5.13 Weapon Equipment — Grade B−

**Purpose.** Four generic part slots; owned-instance inventory; live resolved preview.
**Implementation.** `present_weapon_equipment` (`flow.rs:4916-5135`); scroll area with owned
`ScrollPosition` surviving rebuilds (the V7 M02 fix); focus scroll-into-view; conflicts as
`EQUIPPED BY {name}` suffix + inline error on attempt; Save = full slot replacement; Cancel returns
to the editor overlay.
**Presentation.** Two-line stat readout; slot rows (`SLOT N: {part|EMPTY}`, `> ` prefix on the
selected slot); `UNEQUIP SELECTED SLOT`; amber section header; part buttons
`{name} [{type}] — {signed effects}`; fixed SAVE/CANCEL footer.
**Quality.** The most improved surface in the codebase (two V7 playtest rounds landed here):
scrollable, focus-visible, conflict-labeled, draft-safe. Signed per-effect deltas are the right
comparison primitive.
**Issues.**
- Preview/effects in ticks + basis-point percentages ("fire interval -3t +10%", "Slow 40%/90t") with
  no plain-language summary (G-03).
- `INVALID PART COMBINATION` as the entire invalid-state explanation — which slot, which part,
  which rule?
- The header "OWNED PARTS — type labels are presentation only" is developer caveat text in the
  player's face.
- Moving a part between this brawler's slots clears the old slot silently.
- No comparison against the *currently equipped* part for the selected slot (the deltas are vs the
  base weapon, which is defensible; a "vs current slot" toggle would be better).
**Improvements.**
1. Humanize units (G-03) and add one summary phrase per part ("faster, weaker shots").
2. Structured invalid reason: "Slot 2 + Slot 4: {rule}" style.
3. Delete the "presentation only" clause from the player-facing header (it's a spec footnote — move
   it to docs).
4. When equipping a part already in another of *this* brawler's slots, message the move.
5. Longer term: swap the selected-slot prefix with proper selected styling (G-06).

### 5.14 Confirmations (Delete / Cancel Match Start / Leave Match / Change Server) — Grades B / B− / B− / B

**Implementation.** `flow.rs:5266-5338,4423-4602`; shared skeleton: scrim, plain panel (no border),
safe action index 0 with default focus, destructive index 1.
**Quality.** Safe-default focus everywhere is correct and consistent; Escape maps to the safe
action; Delete names its target; Change Server explains its consequence.
**Issues.** Cancel-match-start and Leave-match carry no consequence text at all ("CANCEL MATCH
START?" with two buttons and zero body). All four panels are borderless while every other dialog
family has a border — reads unfinished rather than intentional. Destructive buttons unstyled (G-01).
Leave's consequence is the most interesting one (forfeit + vulnerable-while-menuing) and is exactly
what's missing.
**Improvements.** One body line each, factually: Leave → "You will forfeit and return to the lobby;
the match continues for everyone else." Cancel start → "Your reservation will be released; other
players continue forming." Add the shared border + destructive styling (G-01). This is small copy +
style work on an already-correct skeleton.

### 5.15 Error overlay — Grade B−

**Implementation.** `present_flow_error_overlay` (`flow.rs:3090-3174`); typed kinds with titles;
≤2 actions from a closed set; rate-limited queue errors get the countdown button.
**Presentation.** Red-bordered panel (the only red-bordered surface — good signal), salmon message,
RETRY-focused by default.
**Quality.** Category titles ("CONNECTION ERROR" vs "SAVE ERROR") genuinely help; the action set is
honest and per-context; the rate-limit countdown is the best disabled-state pattern in the shell.
**Issues.** Double-panel bug with shell errors (G-13); message strings sometimes embed server
jargon; retry-first focus is right for transient errors but wrong-ish for "SAVE ERROR" where
"CONTINUE WITHOUT SAVING" (data-affecting) benefits from being the deliberate path — keep retry
first, but style the destructive-ish secondary distinctly (G-01).
**Improvements.** Fix G-13; audit message strings for jargon; apply intent styling.

### 5.16 Settings — Grade D+ (see §6)

### 5.17 Credits — Grade B

Clean, complete, correctly licensed attribution with a single BACK (focus + Escape both work).
Only note: it's reachable only via Dashboard Menu (fine), and body copy is a single 16 px block —
grouping by category would be kinder, but this is genuinely fine for a credits screen.

### 5.18 Legacy Build Editor — Grade F

Unreachable production code (~500 lines of presenter + constants + draft state) from the retired
local-build era. Delete it in a cleanup pass; the V7 brawler surfaces own this job now. Keeping it
costs confusion (its constants sit beside live ones at `flow.rs:38-43`) and test surface.

---

## 6. Settings deep dive (the weakest screen)

**What exists** (`shell.rs:296-471`, `settings/ui.rs`, `settings/persistence.rs`): rebindable
keyboard/mouse/gamepad bindings with conflict detection and logical-key matching; move/aim
deadzones, aim commit threshold, trigger press/release hysteresis; Y-inversion ×2; UI scale 0.8–1.4;
reduced motion; reduced combat effects; master volume ±10; mute-unfocused; fullscreen; vsync;
reset; draft/Apply/Cancel with live preview of display settings; robust atomic persistence with
kind-specific failure copy. **As a capabilities list this is strong** — several of these are
missing from shipped AAA games.

**Why it still grades D+:** the *surface* is a console dump. Three blocks of monospace-ish text
lines (one showing `Cal move=0.00 aim=0.25 commit=0.35 trigger=0.55/0.45`), selection indicated by
`[brackets]` in the text, and a flat grid of 19 uniform buttons whose order interleaves unrelated
controls (`UI -` is 5 buttons away from `UI +`). There are no rows, no sliders, no toggles, no
per-setting labels in the layout sense. Raw-keyboard shortcuts exist but are dead in the product
draft path. Volume changes don't preview until Apply (display settings do). Values render in
ALL-CAPS fragments inside one text block (`UI SCALE 1.0    REDUCED MOTION OFF …`).

**Missing capabilities** (vs the project's own `docs/13` settings contract and typical
accessibility floors): colorblind palette (named in the contract), any captions/non-audio cue
equivalents, separate text size, language, mouse sensitivity/aim options, hold-vs-toggle options,
per-binding reset/unbind, controller glyph display (bindings render as debug names like
`RightTrigger2`), controller-disconnect handling surfaced in UI.

**Recommendation — rebuild as rows, reuse everything underneath.** The drafts, validation,
persistence, conflict detection, and rebind capture are all sound; only presentation needs to
change:
1. Grouped sections: CONTROLS (movement, combat, bindings, deadzones), DISPLAY (window, vsync, UI
   scale), AUDIO (volumes, mute-unfocused), ACCESSIBILITY (reduced motion/effects, future
   colorblind/captions).
2. One row per setting: label (left), current value (right), focused row adjusts with left/right or
   D-pad left/right, click selects, on-screen `−/+` only when a pointer is active or as small
   steppers. Toggles render as ON/OFF; ranges as `value ←/→` steppers (sliders are not required).
3. Rebind rows: action name, current binding (with glyph), press to capture; conflicts shown on the
   row, not a summary line.
4. Live-preview audio from the draft (align with display behavior).
5. Keep Apply/Cancel/Reset as a fixed footer; Reset asks for confirmation only because it discards
   the draft (currently instant, which is fine — pick one and be consistent with the leave/change
   dialogs).
6. Add the contract-named missing items as they're promoted (colorblind belongs to
   CAND-RELEASE-READINESS; captions can wait for audio depth).

This is the single highest-leverage UX rebuild in the shell: it touches every player, it's the
accessibility front door, and the model layer is already correct.

---

## 7. In-match UX deep dive

Audited against `docs/11` readability contracts and `docs/05` feedback layering; the Unified
Guidelines apply to input/focus/waiting behaviors here.

**What works well**
- **Honesty under replication lag:** generation-gated clock shows `SYNCING` instead of guessing
  (`hud.rs:222-246`); objective shows `SYNCING OBJECTIVE` on arrival mismatch; the same discipline
  as the shell.
- **Self-identification:** green ring + facing arrow + green name + own ammo pips; camera clamps to
  map bounds; no shake exists (so no reduced-shake debt).
- **Waiting-phase readiness:** quorum text `{r}/{n} fighters ready` + prompt naming all three input
  modes (`hud.rs:513`) — one of the few places control help exists.
- **Reduced effects** shrink/shorten cue spheres without hiding objectives/previews/identity
  (`combat.rs:1259-1353`) — spec-compliant.
- **Concealment readability (V9):** 52 % alpha for concealed allies/self, `CLOAKED {n}s` HUD phase,
  two-phase targeting with `TARGETING - FIRE TO CONFIRM / CANCEL TO EXIT` prompt, high-contrast
  reveal ring outside the team ring — the newest feature is also the most carefully presented.
- **Audio cue budget** with per-kind caps and dedup (`audio.rs:299-357`) prevents burst spam.

**Gaps (each maps to a finding above)**
1. Color-only teams + `T1/T2` (G-04).
2. Ticks in HUD (`COOLDOWN 12t` → "0.2 s" or a progress bar) (G-03).
3. No damage-direction or hit-marker feedback; taking damage offscreen is indistinguishable from
   nothing. The health panel updates, but the guidelines' "budget attention" principle wants the
   *source* hinted. A subtle directional vignette (reduced-effects-aware) is the standard solution.
4. No kill feed / event feed: defeats, forfeits, and reconnects are invisible except as scoreboard
   status changes. A two-line transient feed fits the existing bounded-cue architecture.
5. Offscreen objective (Hot Zone) — **deferred with recorded evidence since V1 M09** ("from team
  spawns the zone is entirely off-screen at match start and nothing on the HUD points toward it").
  This should be the first combat-readiness item promoted: a simple edge arrow at the HUD's top
  band toward the zone during Countdown/Active, hidden when on-screen.
6. No local respawn countdown overlay (respawns show only in scoreboard rows); a centered
  "RESPAWN IN {n}" during your own death is table stakes for the mode design (Wipeout re-entry
  timing).
7. No low-health state: the panel is text; no audio heartbeat, no vignette. Cheap and
  reduced-effects-friendly options exist.
8. In-match menu: not mouse-clickable, ASCII `>` selection, selection resets each open
  (G-08); scoreboard rows unclickable/unsorted.
9. Scoreboard: weapon as `W3` raw code, no K/D/damage/ping columns, sorted by team then player id
  rather than contribution.
10. No ultimate-ready prominence: charge is a per-mille percentage line in the ability panel; a
    ready state deserves the amber treatment the countdown numeral gets.
11. Escape double-duty during targeting (G-14).
12. Diagnostics overlay on F3 in product builds (G-10); dev build-selection overlay spawned-then-
    hidden.

**What I do *not* recommend adding now:** minimap (maps are small, camera fixed, and the zone arrow
covers the actual observed failure), floating damage numbers (contradicts the accepted restrained
presentation direction; the art doc's occlusion/triage bar applies), aim assist (explicitly
server-rule-gated in docs/05).

---

## 8. Prioritized improvement roadmap

Reconciled with the project's backlog so nothing here contradicts an owned decision. P0 = do first;
P1 = high value next; P2 = when the owning milestone/promotion happens.

### P0 — Bugs and contract violations (small, immediate)

1. **Fix the double settings-error panel** (G-13). Bug; two lines of ownership + a regression test.
2. **Fix Escape double-duty during ultimate targeting** (G-14).
3. **Humanize leaked units/ids** (G-03): seconds not ticks in ability HUD; map display name in Match
   Loading; profile/weapon names on the dashboard card; verify the `/12` budget copy.
4. **Visible disabled reasons** on Dashboard Play/Practice and Game Type CONFIRM (G-02), reusing the
   rate-limit caption pattern.

### P1 — High-leverage UX work (fits CAND-RELEASE-POLISH promotion)

5. **Button intent system** (G-01): primary/secondary/destructive styling across all overlays and
   confirmations; consequence copy on all four confirmation dialogs.
6. **Settings row rebuild** (§6): grouped rows, steppers/toggles, live audio preview, glyph names.
7. **Team identity beyond color** (G-04): ring shape language + scoreline glyphs; fold the
   colorblind palette in when promoted.
8. **Combat feedback baseline** (§7 items 3–7): damage-direction hint, kill/event feed, offscreen
   zone arrow (the V1 M09 recorded debt), respawn countdown, low-health state — all
   reduced-effects-aware.
9. **Results enrichment** (§5.8): final scoreboard block + 2–3 personal facts + victory/defeat
   treatment.
10. **In-match menu parity** (G-08): real buttons, mouse support, persistent selection; scoreboard
    weapon names.

### P2 — Owned by existing/planned milestones

11. Audio depth (G-09) under CAND-RELEASE-POLISH: categories, live preview, focus mute, distinct
    cue identities.
12. Onboarding controls card (G-12) — cheap now, but right before first external playtests is the
    natural moment.
13. Screen-transition fade + Results celebration (G-05) with the next presentation pass.
14. Server Select row restructure (§5.2) with favorite undo/confirm.
15. Creation/editor descriptions of profiles/bases/ultimates (§5.11–5.12) when the catalogs gain
    player-facing blurbs.
16. Housekeeping (G-10): delete legacy Build Editor, feature-gate F3 diagnostics, stop spawning the
    dev build overlay in product runs.
17. Heist/V10 UX (from the roadmap): the safe-objective HUD should ship *with* the offscreen-
    objective indicator (§7.5) since Heist adds two more always-relevant objectives; the roadmap's
    "safe/chest readability" gates will benefit from the team-shape language (G-04) landing first.

### Explicitly not recommended (respecting product non-goals)

No title screen, no parties/social surfaces, no progression/economy UI, no store, no minimap, no
hover-dependent information, no per-message protocol/UI versioning, no touch layout (mobile is a
documented non-goal until promoted). Several Unified Guidelines sections (commerce, social,
progression, randomized rewards) are correctly *not applicable* to this product today.

---

## 9. Acceptance checklist (Unified Guidelines, final table)

| Area | Verdict | Evidence / gap |
|---|---|---|
| Play access | **Pass** | Auto-connect, one-press Play/Practice, exact replay |
| Hierarchy | **Partial** | Dashboard yes; overlays have none (G-01) |
| Navigation | **Pass** | Back/Cancel/Close/Home semantics consistent and safe-default |
| Context preservation | **Pass** | Return focus, drafts, scroll positions preserved |
| Controller | **Pass** | Focus obvious/stable/repaired; no wrap is a minor feel choice |
| Touch | **N/A** | Mobile is a documented non-goal |
| Keyboard/mouse | **Partial** | Pause menu + settings values not mouse-operable (G-08) |
| Responsive layout | **Pass (dashboard) / unverified elsewhere** | G-15 |
| Build choice | **Partial** | Signed deltas yes; ticks, missing descriptions (G-03, §5.11) |
| Authority states | **Pass** | Best-in-class preview/pending/accepted/rejected/stale distinctions |
| Disabled states | **Fail** | Gray-alone, reasons hidden (G-02) |
| Notifications | **Pass** | Notice line + a11y labels have distinct semantics; no dot-spam |
| Accessibility | **Partial** | Motion/rebind/scale yes; colorblind/captions missing (G-04, §6) |
| Motion | **Partial** | None problematic; almost none explanatory (G-05) |
| Waiting | **Pass** | Immediate ack, honest pending, valid cancel everywhere |
| Progression | **N/A** | Correctly absent per non-goals |
| Commerce | **N/A** | Absent |
| Social | **N/A** | Absent |
| Growth | **Pass** | V10 Heist plans attach to existing surfaces (Dashboard/mode HUD) |

---

## Appendix — Primary sources

- Guidelines: `references/best-practices/Out-of-Game-UX-UI-Unified-Guidelines.md`
- Product/spec: `docs/00-product-direction.md`, `docs/13-player-ux.md`, `docs/05-gameplay-loops.md`,
  `docs/11-art-and-presentation-direction.md`, `docs/17-concealment.md`,
  `docs/18-damageable-world-objects-and-heist.md`, `docs/backlog.md`
- Implementation: `src/client/flow.rs`, `shell.rs`, `dashboard.rs`, `presentation.rs`, `hud.rs`,
  `build_editor.rs`, `server_select.rs`, `settings/*`, `input.rs`, `audio.rs`,
  `src/combat/client/hud.rs`, `src/client/presentation_3d/{mod,combat,camera}.rs`,
  `src/diagnostics/overlay.rs`
- Playtest record: `docs/implementation/v1/roadmap.md` (POST-V1-RELEASE-POLISH), `v1/milestone-09`
  (offscreen zone), `v2/milestone-03` (connecting), `v4/milestone-03` (overhead UI), `v5/milestone-01/02/03`
  (dashboard language, server-select recovery, compact layout), `v6/milestone-01` (Balance Lab),
  `v7/milestone-02` (pointer regression, equipment scroll), `v9/milestone-01/02` (concealment
  readability, two-phase targeting)
- Status context at audit time: V8 Complete; V9 M02 in `User playtest` (open closeout); V10 roadmap
  prepared, gated on V9 close.
