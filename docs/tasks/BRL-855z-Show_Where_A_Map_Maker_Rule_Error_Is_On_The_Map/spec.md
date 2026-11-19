# Context (deferred from BRL-2xpc, 2026-11-19)

Map Maker's Check tab (`map_maker/check.rs`) lists the countable requirements: spawns per team, the capture zone and the Heist safes. Each unmet requirement has an **Add** action that opens Assets on the entry that fixes it.

When every count is met but `validate_authored_recipe` still refuses the map, Check shows the validator's message as text only. Examples: "effect tile violates spawn safety clearance", "placement is incompatible with the effective surface", and the navigation rules. The validator returns a `String` without the cells or placements involved, so the editor cannot point at the problem.

# Scope

- Let the authoritative validation report the offending placement IDs or cells next to its reason. Use a typed error detail, not message parsing, and keep the server admission path and wire contracts unchanged.
- In Check, add **Show** for those issues. It selects the offending placements (Select tool) and frames the camera on them.

# Acceptance

- A spawn-clearance or surface refusal shows **Show**, and Show selects and frames the offending items.
- The validator's accept/refuse decisions are unchanged (existing map validation tests pass).

# Implementation (2026-11-19)

## Validator detail design

- New `brawler::map::MapRuleRefusal { reason: String, placements: Vec<MapPlacementId>, cells: Vec<MapCell> }` (`src/map/resolution.rs`, re-exported from `map`). `From<String>`/`From<&str>` keep every unlocated rule a one-line change; `Display` prints the reason.
- `AuthoringRecipeError::Recipe(String)` became `Recipe(MapRuleRefusal)`, with `AuthoringRecipeError::rule_refusal()`. `Display` still prints only the reason, so `error.to_string()` callers (server admission, worker manifest check, publish) see identical text.
- `resolve_located_grid_recipe` runs the unchanged rule pipeline returning `MapRuleRefusal`; `resolve_grid_recipe` (presets, client snapshots, `resolve_recipe`) maps it back to the reason `String`, so its signature and callers are unchanged. `validate_authored_recipe` calls the located variant.
- Located rules today:
  - effect tile violates spawn safety clearance: `[tile id, spawn id]`, `[tile cell, spawn cell]`;
  - spawn overlaps blocking feature / spawn lacks perimeter clearance: the spawn;
  - placement is incompatible with the effective surface: the placement and its refused cells.
- Same rules, same order, same first refusal: accept/refuse decisions and reason text are unchanged. Nothing new is serialized; no protocol or wire contract changed. Navigation, Heist access and Zombie ingress refusals remain unlocated (they need path reconstruction; follow-up if wanted).
- Expanded filled-rectangle placements report their expanded IDs; the editor falls back to selecting by the reported cells when an ID is not a source placement.

## Editor

- `check::RuleIssue { text, placements, cells }` replaces the rule string. A located refusal is listed even while counts are unmet (no count explains it); an unlocated one still shows only when every count is met (the old rule).
- The issue row has **Show** (index 1045) with advice "Show selects it on the map. Fix it, then check again." `FlowUiAction::MapMakerShowRuleIssue` recomputes the check, switches to Select (Check stays open), selects the refused placements, frames the camera on the refused cells plus the selected placements (6-cell margin, zoom 0.5..1), and puts the reason in the status toast.
- Durable docs: `docs/16-grid-map-asset-system.md` (refusal detail), `docs/13-player-ux.md` (Check Show).

## Verification

- `map::tests::authored_rule_refusals_locate_the_refused_placements_and_cells`: Feature Yard Wipeout with its damage tile moved beside a spawn → reason unchanged, `[tile, spawn]` and both cells; Water under a 1×1 wall → reason unchanged, `[wall]` and its cell; a dimension limit → no rule refusal. The preset itself still validates.
- Existing map validation tests: `cargo test -p brawler --lib map::` 45/45 before the client work; the full `just test` passed every suite including server admission and network.
- `map_maker::tests::check_show_selects_and_frames_a_spawn_clearance_refusal`, `check_show_selects_a_placement_on_an_incompatible_surface`, `a_located_refusal_shows_even_while_counts_are_unmet`.
- `just lint`: pass. `just test`: all suites pass except the wall-clock performance gates under the parallel run; rerun single-threaded 12/12 pass.
- Captures (off-screen, port 5130, 1280x720 and 914x411), new routes `map-editor-rule` (wall line with Water painted under two cells, Check open) and `map-editor-rule-show` (after Show). Before: `target/q5sm/before/map-editor-rule*/` — Check lists only the two spawn counts; the surface problem is invisible. After: `target/q5sm/after/map-editor-rule*/` — a third row "Placement is incompatible with the effective surface." with Show; after Show the Select tool is active, the refused wall is selected with its toolbar, the camera is zoomed onto it and the toast repeats the reason. Paths are in worktree `agent-ac1506e403a2eb479`.

## Pixel / controller playtest

- Same build as BRL-q5sm. In a draft, paint a wall, then paint Water under it (Ground category); or place a Damage Tile next to a spawn once both teams have 3 spawns. Open Check: the rule row shows **Show**. Tap Show (or reach it with Select → D-pad → South): Select tool, the offending items selected, camera framed. Delete or move them; the row disappears.

## Known limits

- The validator stops at the first refusal, so Show points at one problem at a time.

## Commit

cdf9f562 on branch `worktree-agent-ac1506e403a2eb479` (shared with BRL-q5sm).
