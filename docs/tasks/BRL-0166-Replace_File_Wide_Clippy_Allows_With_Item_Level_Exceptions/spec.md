# Outcome

Lint exceptions are narrow and reviewable, as AGENTS.md requires.

# Context

File-wide allows exist in `src/combat/mod.rs:2`, `src/combat/fields.rs:3`, `src/combat/spray.rs:3`, `src/combat/splash.rs:3`, `src/combat/conditions.rs:3`, and `src/concealment/mod.rs:33`. Many more files carry file-wide `wildcard_imports` allows.

# Scope

- Replace each file-wide complexity allow with item-level allows or decomposition.
- Decide whether file-wide `wildcard_imports` is acceptable and record the rule.
- Add a lint check that rejects new file-wide complexity allows.

# Out of scope

Behavior changes.

# Acceptance criteria

- No file-wide complexity `#![allow]` remains in `src/`.
- `just lint` passes.
- The check script guards against regressions.

# Verification

`just lint`, `just test`.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.


# Inventory (2026-11-14, before code changes)

`rg -n '#!\[(allow|expect)\(' src packages tests` finds 29 inner (module/file-wide) lint attributes:

- Complexity/cast (1): `src/combat/mod.rs:2` allows `cast_possible_truncation`, `cast_sign_loss`,
  `cast_precision_loss` for the entire `combat` module tree (every combat submodule inherits it).
- `clippy::wildcard_imports` file-wide (24): `src/combat/{fields,spray,splash,conditions,authority,sticky}.rs`,
  `src/combat/effects/{application,planning,runtime}.rs`, `src/combat/delivery/projectiles.rs`,
  `src/combat/definitions/resolver.rs`, `src/combat/client/{mod,cues,preview}.rs`,
  `src/client/{automation,input,presentation,hud,assets}.rs`, `src/client/presentation_3d/mod.rs`,
  `src/map/{server,client}.rs`, `src/server/verification.rs`.
- `clippy::wildcard_imports` module-wide inside a role module (4): `src/concealment/mod.rs:33` (`mod server`),
  `src/matchplay/heist.rs:215` (`mod authority`), `src/matchplay/wipeout.rs:75` and
  `src/matchplay/hot_zone.rs:241` (`mod rules`).
- `unused_imports` (1, rustc, not Clippy): `tests/map_public_api.rs:1`, a public-API compile test whose
  imports exist only to prove the paths resolve.
- `packages/` has none.

The context list in this spec named `fields/spray/splash/conditions/concealment` as complexity allows; they
are `wildcard_imports` allows. The only file-wide complexity allow is the combat cast allow.

# Approach

- Repo convention: no `#[expect]` anywhere yet; ~229 item-level `#[allow(clippy::…)]`. MSRV 1.95 supports
  `#[expect(…, reason = …)]`, and it reports stale exceptions, so new item-level exceptions use
  `#[expect]` with a reason, except where a lint fires only in some feature configurations (then `#[allow]`).
- Cast allow: delete the inner attribute, run Clippy for every role, then for each finding either make a
  small clearly-better fix (e.g. `u16::try_from`, `f32::from`, saturating helpers already in the crate) or
  attach an item-level `#[expect]` with a reason at the quantization site. No large refactors.
- Wildcard rule (decision): a glob import of the parent composition surface (`use super::*` / sibling
  `runtime::*`) is acceptable as an item-level exception attached to that `use` statement only, with a
  reason; inner `#![allow(clippy::wildcard_imports)]` is not. `use super::*` in `#[cfg(test)] mod tests` is
  already exempt by Clippy and needs no attribute. Replacing the globs with explicit lists is churn with no
  behavior value and is not done here.
- `tests/map_public_api.rs`: move the `unused_imports` allow onto the `use` item(s) it covers.
- Regression guard: new `scripts/check-lint-exceptions.sh`, wired into `just lint`, rejects any inner
  `#![allow(clippy::…)]`/`#![expect(clippy::…)]` (and inner complexity/cast allows of any form) in `src/`,
  `packages/`, `tests/`.
- Organization-only: no behavior, public API, schedule ordering, or wire change.


# Implementation (2026-11-14)

All 29 inner lint attributes are gone; `rg -n '#!\[' src packages tests --glob '*.rs'` finds nothing.

- Combat cast allow (`src/combat/mod.rs`): removed. Small fix: new `crate::timing::SIMULATION_TICK_HZ_F32`
  (beside the existing `SIMULATION_TICK_HZ_F64`, same item-level allow) replaces 7 `SIMULATION_TICK_HZ as f32`
  casts in `combat/{attack,model,evidence}.rs`, `combat/definitions/mod.rs` (3), `combat/delivery/projectiles.rs`.
  The remaining 12 cast sites carry item- or statement-level `#[expect(…, reason)]`:
  - `combat/attack.rs`: `cone_spray_timing` fill-ticks `let` (truncation/sign); `resolved_lob_flight_ticks` fn.
  - `combat/definitions/mod.rs`: cone-spray fill-ticks `let` inside `validate` (statement, not the large fn).
  - `combat/delivery/projectiles.rs`: `plan_lob_trajectory` (precision).
  - `combat/model.rs`: `ConeSprayState::reached_distance` (precision).
  - `combat/evidence.rs`: `CombatProjectileSnapshot::from_components` (precision).
  - `combat/effects/application.rs`: healing `requested` `let` (clamped to 1..=u16::MAX).
  - `combat/effects/runtime.rs`: the per-mille slow multiplier was an inline call argument; hoisted into a
    `let movement_multiplier_per_mille` (pure arithmetic, same evaluation result) so the expect is
    statement-level instead of covering `plan_runtime_effects`.
  - `combat/client/preview.rs`: two `to_cell` closure `let`s (clamped to a u16 dimension) and
    `sector_marks`, `circle_points`, `cone_spray_rays`, `cone_spray_preview_primitives` (small usize rim/ray
    constants to f32).
- Wildcard imports: the inner allows covered whole module trees. Removing them exposed 66 production glob
  imports: 29 in the 27 inventoried files, 36 in `src/client/presentation_3d/**` submodules that had inherited
  the `presentation_3d/mod.rs` allow, and 1 in `presentation_3d/diagnostics.rs` (automation/network configs).
  Each glob `use` now carries `#[allow(clippy::wildcard_imports, reason = "…")]`; the file's previous reason
  is kept where it had one.
- `tests/map_public_api.rs`: `#[expect(unused_imports, reason)]` on each of its three `use` items.
- Guard: `scripts/check-lint-exceptions.sh` (`just lint` → `_lint-exceptions`) fails on any inner
  `#![allow(`/`#![expect(` or `#![cfg_attr(…allow|expect(…)]` in `*.rs` under `src`, `packages`, `tests`,
  and fails closed (exit 2) when ripgrep is missing or errors.

# Rule recorded (wildcard imports)

A glob import of the parent composition surface is allowed only as an item-level
`#[allow(clippy::wildcard_imports, reason = "…")]` on that `use`; never as an inner attribute. `use super::*`
inside `#[cfg(test)] mod tests` needs nothing (Clippy exempts it). New code should prefer explicit imports
(BRL-0025 precedent); BRL-0213 removes the remaining globs. `#[expect]` is the default for other new
item-level exceptions; wildcard ones use `#[allow]` (see learning).

# Verification (2026-11-14)

Logs: `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0166/`.

- Per-role Clippy probe without `-D` (client, client+automation, server, network-test, balance-lab):
  0 warnings, 0 unfulfilled expectations (`probe-*.log`).
- `just lint` with VS Code's ripgrep first on PATH: **pass** (`lint.log`), including `_server-features`,
  `_client-features`, and the new `_lint-exceptions`.
- `cargo ndk --target arm64-v8a --platform 31 clippy --locked -p brawler-android -- -D warnings`: **pass**
  (`android-clippy.log`). There is no justfile/script recipe for it; command as recorded by BRL-0161.
- `just test`: **pass**, 23 suites, 2305 passed, 0 failed (`test.log`).
- `just e2e 2`: **pass**, one exact 1v1 roster reached Active, workers reaped cleanly (`e2e2.log`).
- Guard self-test: the pattern matches a probe file containing `#![allow(clippy::foo)]`.

# Remaining exceptions

- 12 cast `#[expect]`s listed above plus the `SIMULATION_TICK_HZ_F32` allow; each is at the quantization or
  conversion site with a bounded-value reason. None points at a decomposition need.
- 66 item-level `clippy::wildcard_imports` allows (plus 12 older item-level ones) → BRL-0213.
- 3 `unused_imports` expects in the public-API compile test (intentional).

# Deferred

- BRL-0213 (todo, parent BRL-0171, related BRL-0166): replace production `super::*` globs with explicit
  imports and delete the allows.
- Not ticketed (noted only): `combat/definitions/mod.rs` validation repeats `cone_spray_timing`'s fill-ticks
  formula from `combat/attack.rs` (server-only); dedupe if either changes.
- Durable doc: the wildcard rule lives in this spec and the guard script; AGENTS.md was not edited.

# Learn from errors

- The spec's Context called five `wildcard_imports` allows "complexity" allows; the inventory step caught it.
  Lesson: classify each lint from the source before planning.
- Inner allows silently cover whole module trees: `presentation_3d/mod.rs`'s allow hid 37 submodules (36 + diagnostics) and
  the combat cast allow covered every combat file. A text inventory undercounts; run Clippy per role after
  removal to find the real scope.
- `#[expect(clippy::wildcard_imports)]` on a `use` is reported "unfulfilled" by clippy 0.1.95 in every config
  although it suppresses the lint. Use `#[allow]` for that lint; keep `#[expect]` elsewhere and run every
  feature config, because expectations are evaluated per configuration.
- First guard draft passed when `rg` was missing (`if rg …` treats exit 127 as no match). Guards built on
  search tools must distinguish "no match" (1) from errors.
- In zsh, `set -- $var` does not word-split, so a first probe loop silently ran nothing; run multi-config
  loops under bash.
