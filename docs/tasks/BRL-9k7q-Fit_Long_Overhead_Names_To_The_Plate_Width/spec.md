Reviewed 2026-11-15 against HEAD 3437c0d8: valid, spec stale (plate is 140 px, not 120; name byte cap limits CJK to 21 chars) — updated.

# Context
`packages/brawler-client/src/presentation_3d/combat/fighter_ui.rs` lays out every overhead plate at a fixed `OVERHEAD_WIDTH = 140.0` logical px (`:19`; was 120 when this was filed). The name is a 15 px Lilita One `Text` (`PLAYER_NAME_FONT_SIZE`, `:20`) with `LineBreak::NoWrap`, centered in a full-width container with no overflow clip (`spawn_overhead_text`, `:348-385`). Overlap resolution assumes that width: `overhead_plates_overlap` (`:504-508`) treats two plates as disjoint once `|dx| >= OVERHEAD_WIDTH`, and `resolve_overhead_overlaps` (`:480-502`) stacks only plates that overlap by that rule. So any name wider than 140 px spills into a neighbouring plate that sits side by side.

Name limits (`src/lobby.rs:31-32`, `normalize_proposed_display_name` `:417-427`): 3..=24 graphemes and <= 64 UTF-8 bytes. Worst cases: 24 wide Latin graphemes (~240 px at 15 px), 21 CJK characters (3 bytes each, ~315 px), 16 emoji (once BRL-z2hf renders them on Android). In the BRL-5e2s capture "Łukasz Жора Αλέξης" was already wider than the 96 px health bar.

# Plan
Smallest legible change, in order of preference:
1. Clip/ellipsize: after layout, read the name's `TextLayoutInfo`/`ComputedNode` width; if it exceeds `OVERHEAD_WIDTH - 2*pad`, truncate by grapheme (unicode-segmentation, already a dependency) and append "…" (Lilita One has U+2026? verify with `ui::theme::display_font_has_glyph`; otherwise use "..."). Cache the shown string per overhead so it is recomputed only when the name changes (`update_fighter_overhead_state`, `:618`, writes the name text at `:693`).
2. Alternative: shrink the name font down to a floor (e.g. 11 px) before truncating.
3. Keep the plate width fixed so `resolve_overhead_overlaps` stays correct; also add `Overflow::clip_x()` on the name container as a backstop.
Pure helper (e.g. `fit_overhead_name(name, measure) -> String`) so it is unit-testable without rendering.

# Acceptance
- Unit test in `fighter_ui.rs` tests: the fitting helper keeps short names unchanged and fits a 24-grapheme `W…` name, a 21-char CJK name and a mixed-script name within the plate width (with a fake per-grapheme measure).
- ECS test: two plates at exactly `OVERHEAD_WIDTH` horizontal separation with max-length names produce name widths <= `OVERHEAD_WIDTH` (no overlap).
- `cargo test -p brawler-client --lib presentation_3d::combat::fighter_ui`, `just lint`.
- Native capture (macOS) of two adjacent fighters with max-length CJK and Latin names: no text overlap.
- Pixel 8a device check: same capture on device (system Noto fallback widths differ from macOS).

# Related
BRL-z2hf (emoji rendering on Android) will widen emoji names; the fit must measure the real laid-out width, not a grapheme estimate.

# Implementation (2026-11-18, against HEAD cca1413a)

## Decisions
- Plan option 1 (truncate with ellipsis), no font shrink: the name keeps the 15 px Lilita One size so plates stay uniform; option 2 was not needed.
- Ellipsis: Lilita One maps U+2026 (checked by the test `display_font_draws_the_truncation_ellipsis` through `ui::theme::display_font_has_glyph`), so truncation appends the single glyph `…`.
- Real measured width, not a grapheme estimate: `fit_overhead_player_names` lays each candidate out through `bevy_text`'s own `FontCx` (display font plus system-font fallbacks from `system_font_discovery`) and `LayoutCx` with parley at scale 1, `NoWrap`, the label's font alias and size, and reads `Layout::full_width()` in logical px. Emoji and fallback-font glyphs therefore count at their real advance. `parley =0.9.0` moved from dev-dependency to dependency of `brawler-client` (same pinned version `bevy_text` uses, already in `Cargo.lock`; no lockfile change).
- Pure helper `fit_overhead_name(name, max_width, measure) -> String`: returns the name unchanged when it fits, else binary-searches the longest grapheme prefix (unicode-segmentation, extended clusters) whose `prefix.trim_end() + "…"` fits. About 5-6 layouts per long name.
- Budget: `PLAYER_NAME_MAX_WIDTH = OVERHEAD_WIDTH - 2 * 4 px` (covers the drop-shadow offset and scale-factor rounding). `OVERHEAD_WIDTH` stays 140, so `overhead_plates_overlap`/`resolve_overhead_overlaps` are unchanged and remain correct.
- Recompute only on change: a `FighterOverheadNameUi { full, fitted }` component on the name text entity. `update_fighter_overhead_state` only stores a changed replicated name and clears `fitted`; `fit_overhead_player_names` (chained right after it in `WorldPresentationSet::Animate`) fits unfitted labels and writes `Text`. If the display font is not yet registered in the parley collection (asset alias empty or no family), the label stays unfitted and is retried next frame rather than measured against a fallback face.
- Backstop: the overhead text containers (`spawn_overhead_text`) now have `Overflow::clip_x()`, so nothing can paint past the fixed plate width.

## Files
- `packages/brawler-client/src/presentation_3d/combat/fighter_ui.rs`: constants, `FighterOverheadNameUi`, `fit_overhead_player_names`, `registered_font_family`, `measure_text_width`, `fit_overhead_name`, clip backstop, tests.
- `packages/brawler-client/src/presentation_3d/combat/mod.rs`, `presentation_3d/mod.rs`: export and schedule the fit system after `update_fighter_overhead_state`.
- `packages/brawler-client/Cargo.toml`: `parley` is a normal dependency.

## Verification
- Unit tests (fake per-grapheme measure): `short_names_are_shown_unchanged`, `wide_names_truncate_by_grapheme_to_the_longest_fitting_prefix` (24 x `W`, 21-char CJK at the 64-byte cap, mixed Latin/Cyrillic/Greek/CJK; result fits, ends in `…`, no trailing space, and one more grapheme would not fit), `truncation_never_splits_a_grapheme_cluster` (ZWJ family emoji and flag), `maximum_names_are_valid_lobby_names` (fixtures pass `normalize_proposed_display_name`; a 22nd CJK char is rejected), `display_font_draws_the_truncation_ellipsis`.
- ECS test `adjacent_plates_with_maximum_names_do_not_overlap`: registers the shipped Lilita One as a `Font` asset, runs Bevy's `load_font_assets_into_font_collection`, runs `fit_overhead_player_names` on Latin/CJK/mixed max names, asserts each full name is wider than a plate, each shown name is truncated and measures <= `PLAYER_NAME_MAX_WIDTH`, and centered names on plates exactly `OVERHEAD_WIDTH` apart do not intersect (and those plates are disjoint per `overhead_plates_overlap`). It also checks that a fitted label is not re-measured until its name changes.
- Fixture names in tests use `\u{..}` escapes with a readable comment, because `hud::tests::hud_and_overhead_string_templates_use_only_display_font_glyphs` scans every string literal in `fighter_ui.rs` (including tests) for Lilita-only glyphs.
- `cargo test -p brawler-client --lib presentation_3d::combat::fighter_ui`: 15 passed (now 25 with the `hud::tests` filter included).
- `just lint`: pass.
- `just test`: pass (2510 passed, 0 failed), after the glyph-guard fix below.

## Native macOS capture
- Release `brawler-client --features automation`, 1280x720 at 2x Retina, routed supervisor on `127.0.0.1:5093` with `--data-directory target/9k7q/<run>/server-data` (not the port-5000 server). Wipeout 3v3 (Verdant Crossfire), measured windowed client 1 plus five headless clients; team 0 spawns side by side on one row.
- The automation `--auto-connect` path always proposes the generated `Brawler-XXXX` name (see BRL-5e2s). For the capture only, a temporary hook in `session/admission.rs` read the proposed name from an env var; it was reverted before handoff (`git diff` shows no change to that file).
- Capture: `target/9k7q/run3v3-cjk/captures/brawler-001200.png` (crop `target/9k7q/run3v3-cjk/crop-001200.png`): three adjacent allies named `李小雷さくら김민준李小雷さくら김민준李小雷` (shown `李小雷さくら김민…`), `WWWWWWWWWWWWWWWWWWWWWWWW` (local, shown `WWWWWWWWW…`) and `Łukasz Жора Αλέξης 李` (shown `Łukasz Жора Αλέ…`). Every name sits inside its own plate; no text overlap. Untruncated, the CJK name (~315 logical px) would have crossed into the neighbour plate (~182 logical px between centers here).
- Also `target/9k7q/run3v3/captures/brawler-001200.png` (Latin/Latin/mixed row) and `target/9k7q/run1/captures/` (1v1 local plate showing the truncated `W` name).

## Remaining
- Pixel 8a device capture (acceptance item) not done: Android system Noto fallback widths differ, but the fit measures through the same parley collection on device, so it adapts. Needs a device run with a non-Latin name; the phone still cannot type one (BRL-06pz), so it needs the automation display-name path or a second client proposing the name.
- Automation has no display-name override; this is the second evidence run that needed one (BRL-5e2s was the first). Worth a small automation-only `--display-name` flag if a third need appears.

## Mistakes / lessons
- First full `just test` failed: the HUD glyph-guard test scans all string literals in `fighter_ui.rs`, including test fixtures. Fixed by escaping non-Lilita fixture text. Lesson: test fixtures in HUD/overhead source files must avoid literal non-Latin-1 text.
- First capture attempt (Wipeout 1v1, walking toward each other) never brought the fighters together: Feature Yard spawns are 47 cells apart with obstacles. Team spawns that share a row (Verdant Crossfire 3v3) give adjacent plates deterministically.
