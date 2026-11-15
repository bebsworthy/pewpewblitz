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
