# Outcome

Accepted results, confirmations and rewards get one short celebratory burst, the shell background may carry sparse low-contrast toy motifs, and every shell screen's press/entrance motion is natively reviewed.

# Context

BRL-drny delivered shared surfaces, release activation, press squash/lip collapse, Dashboard Play breathing/shine, and destination-change entrances (`src/client/ui/surface.rs`, `press.rs`, `docs/design/player-shell/motion.md`). `motion.md` assigns results bursts, preview flourishes and motif drift to later screen tickets. Native evidence in BRL-drny covered the gallery, Dashboard, first-brawler creation, Connecting and Connection Lost only.

# Scope

- Burst (scale pop plus shine or particles) on the changed element for match results, delete/leave confirmations and accepted equipment/ability changes. Presentation only, disabled under reduced motion/effects, never delays navigation.
- Optional sparse motif layer on `game-selection-background` and the Dashboard shader, kept clear of hero and controls (BRL-11zt board reference).
- Native review of Settings, game selection, Practice map picker, brawler list/details, weapon and parts, abilities, confirmations, results, match menu and the map maker after BRL-drny; fix any entrance replay on rebuilds or press regressions found (map maker buttons now activate on release).
- Connecting shows the monospace fallback "PEWPEW BLITZ" before the logo asset loads; use the display font or the logo.

# Acceptance criteria

- Bursts and motifs disabled under reduced motion (test).
- Native captures of each listed screen with owner acceptance.

# Related

Follows BRL-drny. Icons are BRL-nzre.

# Implementation (2026-11-14)

Research: `SurfaceMotion` (`packages/brawler-client/src/ui/surface.rs`) already owns press, entrance and idle scale through `UiTransform`, and `shell_surface.wgsl` already had a shine sweep. The Dashboard planes drift in `dashboard_background.wgsl`; eleven screens draw the static `game-selection-background.png`. Profile decisions arrive through `resolve_profile_decision`, which already sets the focus index of the changed slot. Bevy UI renders after bloom/tonemapping, and every burst color is a mix toward white (≤ 1.0), so nothing can bloom (BRL-0176). The first Connecting frame runs before `ClientAssetHandles` exists, so `spawn_logo` drew fallback text in Bevy's default font while panel text was rescued by the surface font rebind.

Decisions:

- Burst = `SurfaceBurst` (presentation only, 420 ms): 7% pop reached at 25% then an ease-out-back settle, one shine sweep over the first 60%, and a face flash toward white starting at 0.3 (`motion.z` in `shell_surface.wgsl`). Plain nodes (results trophy/headline) get the pop only. Tuning constants sit beside the other surface motion constants in `surface.rs`; there is no authored client data file for shell motion yet.
- Which element bursts: results (trophy, outcome headline, local-team card) on the first presentation of a result only; accepted weapon-part change (the slot); accepted ability change (the ability slot); Delete on the brawler confirmation while the server decides. Flow reducers set `FlowCommit::celebrate`; `commit_flow` records `FlowBurst`, which stays open for 250 ms so the screen rebuilt by the commit bursts its matching action (overlay-scope checked). Later rebuilds never replay it.
- Leave match and Change server close their confirmation in the same frame, so they get no burst (their destination's entrance plays). Navigation never waits for a burst.
- Motifs = `ui::motifs::ShellBackdrop` adds one `shell_motifs.wgsl` layer (rings, plus signs, diamonds, sparkles; 8.5% white) to the Dashboard background (Dashboard layout, placed clear of logo, status bar, hero and action column) and to all eleven `game-selection-background` screens plus the gallery (edges layout: outer margins). Motifs drift with the planes' offset plus a gentle bob/rotation; positions are relative to the node and sizes scale with its height, so they follow the BRL-0196 phone fit. Two shared materials; frozen motifs are not rewritten.
- Reduced motion/effects: bursts inert, motifs frozen at the same pose as the planes. Low tier: `shell_motifs: false` in `render_quality.ron` (schema 3) hides the layer; High/Medium keep it. Bursts are cheap screen-space UI and stay on every tier.
- Connecting logo: `spawn_logo` loads `LOADING_LOGO_PATH` directly when the handle resource does not exist yet; text remains only for headless tests without an asset server.
- Gallery: shared motif backdrop, an interactive "REWARD BURST" card that replays on activation (Reduced motion demo suppresses it), a "Reward burst at peak" reference, and a middle capture (`<stem>-middle.png`).

Files: `ui/surface.rs`, new `ui/motifs.rs`, new `assets/brawler/shaders/shell_motifs.wgsl`, `shell_surface.wgsl`, `render_quality.ron`/`render_quality.rs`, `dashboard.rs`, `flow.rs`, `flow/actions.rs`, `flow/reducer.rs`, `flow/reducer/profile.rs`, `flow/screens/shared.rs`, `results/view.rs`, `results/objective.rs`, `connection_shell.rs` (+ callers), `assets.rs`, the eleven background call sites, `ui/gallery.rs`; docs `motion.md`, `ui-toolkit.md`, `ui/README.md`, `docs/11` tier list.

# Verification (2026-11-14)

- Focused tests: `surface.rs` (burst pops/shines/flashes then settles; reduced motion suppresses bursts on surfaces and plain nodes; plain-node burst; burst shape), `motifs.rs` (one decorative layer per backdrop that drifts; reduced motion freezes pose; Low tier hides and restores; layer fills the backdrop at 914x411 through `layout_harness` with no controls), `render_quality` (Low drops motifs, schema 3), flow tests (accepted equipment and confirmed Delete request the burst; `flow_burst_targets_the_active_action_and_expires`; results first presentation bursts trophy and headline once), `connection_shell` (logo loads before the handle resource exists).
- `just check`: pass. `just lint` (real ripgrep first on PATH): pass after fixing new Clippy findings. `just test`: pass after the last code edit, 2388 passed, 0 failed (log `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl51zd/test.log`).
- Native evidence (scratch target, scratch server/client data dirs, supervisor on 127.0.0.1:5047; `data/` untouched, UDP 5000 untouched):
  - Before (HEAD export): `SP/brl51zd/before/gallery-wide.png`, `-bottom.png`, `gallery-phone.png`, `-bottom.png`. Native Connecting/Dashboard baselines are the BRL-drny after-captures (`.../evidence/after/recovery/brawler-000060.png` shows the monospace "PEWPEW BLITZ").
  - After: `SP/brl51zd/after/gallery-wide.png`, `gallery-wide-middle.png` (held press and burst at peak), `gallery-wide-bottom.png` (edge motifs), `gallery-phone*.png` (914x411), `SP/brl51zd/after/native/brawler-000020.png` (Connecting with the real logo and edge motifs), `brawler-000620.png` (first-brawler creation with edge motifs).
  - Where SP = `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad`.
- Not captured natively: the Dashboard motif layout (needs a seeded brawler), live bursts on results/equipment/abilities/Delete, and the per-screen press/entrance review of Settings, game selection, Practice maps, brawler list/details, weapon and parts, abilities, confirmations, results, match menu and map maker. This agent has no OS input automation; BRL-pw9k tracks an automation path. These remain owner review items for this ticket's second acceptance criterion.

# Playtest request

Run `just ui-gallery`, then `just run 1` (or `just server` + `just client`).

- Gallery: activate REWARD BURST (pop, one shine, flash); toggle Reduced motion and activate again (nothing moves). Check the "Reward burst at peak" reference and the faint edge motifs.
- Client: Connecting shows the real logo from the first frame. Dashboard: motifs sit clear of logo, hero, status bar and buttons and drift slowly. Equip a weapon part: the slot pops once. Change an ultimate or passive: the ability slot pops once. Delete a brawler: Delete pops while "Deleting…" shows. Finish a match: trophy, headline and your team card pop once; resizing the window does not replay it. Turn on Reduced motion: no bursts, motifs frozen. Set Graphics quality to Low: motifs disappear.
- Per-screen review (carried from BRL-drny): Settings, game selection, Practice maps, brawler list/details, weapon and parts, abilities, confirmations, results, match menu and map maker scale in once and do not re-pop on rebuilds; map maker buttons act on release.
- Requested observations: burst strength (the flash briefly lowers white-label contrast at its peak), motif density/placement on each screen and at phone size, any entrance replay or press regression.

# Learn from errors

- Mistake: the first "before" build compiled from the working tree while I was already editing it (cargo snapshots a crate when it compiles, and the gallery loads shaders from the tree at runtime). Prevention: build before-evidence from a `git archive HEAD` export with the same `CARGO_TARGET_DIR`, which isolates both source and assets.
- Mistake: changing a helper's return type (`text` → `Entity`) broke a closure that implicitly returned it; and new code again tripped Clippy (`float_cmp`, `needless_pass_by_value` in the automation-only gallery, a test over 100 lines). Prevention: run `cargo clippy` for both the client and `automation` feature sets before `just lint`, and keep assertions added to long tests to one or two lines.
- Reusable: the gallery capture hook now saves top/middle/bottom, and a client-only run against a scratch supervisor on an ephemeral port captures Connecting and first-brawler creation without input.

# Follow-ups

- BRL-pw9k (backlog): automation path that opens each shell screen for native capture.


# Closeout (2026-11-14)

Moved to done under the owner's process: implementation and agent verification are complete. Owner playtest happens on done tickets; open decisions above stay recorded for that review, and changes come back as a reopen or a new ticket.
