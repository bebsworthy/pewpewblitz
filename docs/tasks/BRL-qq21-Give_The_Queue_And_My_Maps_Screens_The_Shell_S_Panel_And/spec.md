# Origin

Found in the BRL-pw9k shell-capture sheets on 2026-11-18 (`just shell-capture`; see `target/pw9k/sheet-914x411.png` and `target/shell-capture/<size>/queue.png` and `map-library.png`).

- **Queue:** the text ("QUEUE", game line, "1 waiting · 4 players per match", saved brawler and abilities) is a centered monospace-looking block. CANCEL QUEUE is a thin full-width bar. There is no `Surface::Panel`, no shell font and no mode pennant.
- **My Maps** (`map-library`): plain centered text on a dark navy background, with tiny CREATE MAP and BACK buttons. Its touch-target size on phones is BRL-8q4c's scope; this ticket covers the visual style only.

# Scope

- Rebuild both screens with the shared shell pieces used by the overlays in BRL-zvqd: `Surface::Panel`, `theme::font()`, `spawn_action` buttons, and the shell background.
- Queue: show the mode pennant and the game type, the waiting count and players per match, and the selected brawler summary. CANCEL QUEUE becomes a normal secondary action button.
- My Maps: a titled panel with an empty state and properly sized CREATE MAP and BACK actions.

# Acceptance

- `just shell-capture queue` and `just shell-capture map-library` at both sizes show the styled screens.
- `just lint` and `just test` pass, and the existing flow tests still pass.

# Scope change (2026-11-19)
The My Maps (`map-library`) half moves into BRL-2xpc, the Map Maker overhaul, which is being designed separately. This ticket now covers the Queue screen only; `flow/screens/map_maker.rs` and the `map-library` capture are untouched here, and the My Maps acceptance line belongs to BRL-2xpc.

# Implementation: Queue (2026-11-19)
- `flow/screens/queue.rs` now follows the Connecting screen pattern: `connection_shell::root_node` (centered) + `root_background` + `spawn_background` (shell backdrop and motifs), and a `Surface::Panel` (88%, max 900 px, padding 44x30).
- Panel content, all in `theme::font()`: title FINDING A MATCH (46 px), the live population line ("N waiting · M players per match", ACCENT, 24 px), then two `Surface::Dark` tiles side by side: the mode pennant (`ClientAssetHandles::dashboard_badges`, 64x80) with the mode title (WIPEOUT) and the game display name, and the ultimate badge (`icons::ultimate_icon` of the accepted recipe's ultimate kind) with the queued saved brawler's name and "Ultimate · Passive / Passive".
- CANCEL QUEUE is a `spawn_action` button with `Surface::Secondary`, `action_node(70%, 60)` capped at 520 px. `QueueCancelButton` marks it; `update_queue_cancel_button` rewrites the label through the button's `Children` (the `QueueCancelLabel` marker is gone) and toggles `InteractionDisabled`. `FlowButton` requires `TouchTarget`, so the 48 dp floor applies on phones.
- Copy: `queue_membership_text` (one joined string) became `queue_copy` -> `QueueCopy { mode, game, population, brawler, build }`; each text carries a `QueueLabel` field and `update_queue_status` rewrites only the lines that changed. The brawler name is looked up by `membership.brawler_id` in the lobby profile, falling back to "Saved brawler". The mode title and pennant index reuse the Dashboard's `dashboard_mode_title` / `dashboard_badge_index` (now `pub(in crate::flow)`).
- Decision: the cancelling label is "CANCELLING..." (ASCII) because Lilita One has no U+2026 glyph; `cancel_pending_copy_is_explicit_and_disables_only_cancel` updated.
- Decision: the heading changed from QUEUE to FINDING A MATCH, which says what the player is waiting for.

# Verification (2026-11-19)
- Captures (off-screen, scratch supervisor on 127.0.0.1:5066): `BRAWLER_SHELL_CAPTURE_DIR=target/qq21/before|after BRAWLER_SHELL_CAPTURE_BIND=127.0.0.1:5066 just shell-capture queue`. Before: `target/qq21/before/{1280x720,914x411}/queue.png` (monospace text block, full-width bar). After: `target/qq21/after/1280x720/queue.png` and `target/qq21/after/914x411/queue.png`: panel, pennant, ultimate badge, shell font, Secondary cancel button; everything fits without scrolling at 914x411.
- `queue_copy_uses_advertised_game_and_saved_brawler_recipe` now checks each field and the saved-brawler name lookup.
- `shell_controls_are_48_dp_touch_targets_on_a_landscape_phone` passes (Queue at 100% and minimum UI scale).
- `just lint` passed. `just test` passed (exit 0, 0 failures across all suites, including network and performance gates).
- Not done: a Pixel device check of the queue screen (owner playtest; run `just android-build` against `just server`, queue Wipeout, check the panel and that CANCEL QUEUE returns to the Dashboard).
