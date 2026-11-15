Reviewed 2026-11-15 against HEAD 3437c0d8: valid but stale; BRL-0170 (dc8302ea) moved most sites to packages/brawler-client, and the count is now 80 allows (33 in src/, 47 in packages/brawler-client/src/), not 66+12.

# Outcome

Production child modules import the exact parent items they use; no item-level `clippy::wildcard_imports`
allow remains in the workspace (`src/` and `packages/`).

# Context

BRL-gjz8 (BRL-0166, c02a2946) removed every file-wide `#![allow(clippy::wildcard_imports)]` and attached a narrow
`#[allow(clippy::wildcard_imports, …)]` to each production glob `use` instead. BRL-0170 then split the player client
into `packages/brawler-client`, so the sites moved. Count at HEAD (`rg -c 'clippy::wildcard_imports' src packages`):

- packages/brawler-client/src (47): presentation_3d tree 39 files (mod.rs, camera, camera_rig, border, contact_shadow,
  diagnostics, fighter_character, floor_decals, floor_tiles, map, map_editor, map_palette_preview, object_health,
  render_quality, toon, world_look, zombie, hit_feedback{,/flash,/numbers,/recovery}, environment_assets/{mod,catalog,
  runtime,sway}, combat/{aim_preview,cone_spray,dash_style,effects,effects/evidence,elemental_style,entities,
  fighter_feedback,fighter_outline,fighter_rim,fighter_ui,local_projectile,splash_style,sticky_blob,
  straight_projectile_style}); plus assets.rs, automation/mod.rs, hud.rs, input.rs, presentation.rs, and lib.rs ×2
  (`use input::*` :104-105, `use session::*` :119-120, child-module globs, not `super`).
- src (33): combat/{mod (`use authority::*` :36-38), attack, attack/emission, authority, conditions, delivery,
  delivery/projectiles, definitions/resolver, evidence, fields, server, splash, spray, sticky, effects/mod,
  effects/application ×2, effects/application/transaction, effects/planning ×2, effects/runtime, client/{mod,cues,
  preview}}, concealment/mod.rs, map/{client,server}.rs, matchplay/{heist,hot_zone,wipeout,zombie}.rs,
  server/mod.rs (`use verification::*` :86-87), server/verification.rs.

`#[expect(clippy::wildcard_imports)]` on a `use` item is reported unfulfilled by clippy 0.1.95 (still the toolchain,
rustc 1.95.0) even though it suppresses the lint, so these are `#[allow]`. scripts/check-lint-exceptions.sh only
forbids inner (`#![...]`) allows, so it needs no change.

# Scope

- Replace production `use super::*` / `use super::super::*` / `use <child>::*` globs with explicit imports
  (Clippy's suggestion lists the used names: temporarily delete the allow and run clippy). Work one module tree
  per change: (1) presentation_3d, (2) rest of brawler-client, (3) src/combat, (4) remaining src.
- `#[cfg(test)] mod tests { use super::*; }` stays; Clippy already exempts it.
- Delete each item-level allow once its glob is gone.
- Organization-only: no behavior, public API, schedule ordering, or wire change. Where a child glob re-exported
  items to siblings (combat/mod.rs `authority::*`, server/mod.rs `verification::*`), keep visibility identical.

# Acceptance criteria

- `rg -n 'clippy::wildcard_imports' src packages` finds nothing (except any explicitly justified, recorded remnant).
- `just lint` (all `_clippy-*` feature sets: routing, client, automation, server, network, balance-lab), `just test`,
  Android `cargo ndk --target arm64-v8a --platform 31 clippy --locked -p brawler-android -- -D warnings` pass.
