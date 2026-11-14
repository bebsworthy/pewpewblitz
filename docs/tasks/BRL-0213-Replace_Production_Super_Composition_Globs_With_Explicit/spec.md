# Outcome

Production child modules import the exact parent items they use; no item-level `clippy::wildcard_imports`
allow remains in `src/`.

# Context

BRL-0166 removed every file-wide `#![allow(clippy::wildcard_imports)]` and attached a narrow
`#[allow(clippy::wildcard_imports, reason = …)]` to each production glob `use` instead (66 sites), alongside 12 older
item-level wildcard allows (e.g. `src/combat/{delivery,server}.rs`, `src/client/mod.rs`, `src/server/mod.rs`,
`src/matchplay/zombie.rs`). The largest group is the `src/client/presentation_3d/` tree (38 files, including `super::super::*` and
`super::super::super::*`), which had inherited one allow from `presentation_3d/mod.rs`. Other sites:
`src/combat/**` (authority, conditions, fields, splash, spray, sticky, effects/{application,planning,runtime},
delivery/projectiles, definitions/resolver, client/{mod,cues,preview}), `src/client/{assets,automation,hud,
input,presentation}.rs`, `src/map/{client,server}.rs`, `src/server/verification.rs`, the role modules in
`src/matchplay/{heist,hot_zone,wipeout}.rs`, and `src/concealment/mod.rs` (`mod server`). BRL-0025 set the
precedent by removing the session module's `super::*` imports.

`#[expect(clippy::wildcard_imports)]` on a `use` item is reported unfulfilled by clippy 0.1.95 even though
it suppresses the lint, so these are `#[allow]`.

# Scope

- Replace production `use super::*` / `use super::runtime::*` / `use super::super::*` with explicit imports
  (Clippy's suggestion lists the used names). Work one module tree per change if the diff is large.
- `#[cfg(test)] mod tests { use super::*; }` stays; Clippy already exempts it.
- Delete each item-level allow once its glob is gone.
- Organization-only: no behavior, public API, schedule ordering, or wire change.

# Acceptance criteria

- `rg -n 'clippy::wildcard_imports' src` finds nothing (except any explicitly justified, recorded remnant).
- `just lint`, `just test`, Android `cargo ndk --target arm64-v8a --platform 31 clippy --locked -p brawler-android -- -D warnings` pass.
