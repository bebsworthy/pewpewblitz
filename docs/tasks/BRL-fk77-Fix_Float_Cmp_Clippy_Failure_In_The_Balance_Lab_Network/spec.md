# Outcome
`cargo clippy --locked -p brawler-client --features network-test,balance-lab --tests -- -D warnings` passes, and `just lint` runs that combination so it cannot regress.

# Current state (checked 2026-11-15 at HEAD 3437c0d8)
- Still failing. Reproduced: the only warning across all targets (`--keep-going`, no `-D`) is `clippy::float_cmp` at `tests/network/builds.rs:92`: `assert_eq!(server_loadout.fighter_stats.movement_speed, 444.0);` (f32). The test is `#[cfg(feature = "balance-lab")]` (`tests/network/builds.rs:2`), so it compiles only with both features.
- Lint config: `packages/brawler-client/Cargo.toml:90-92` enables `clippy::all` + `pedantic` (pedantic includes `float_cmp`).
- `justfile:47` `lint` covers `_clippy-network` (`justfile:143-144`, network-test only) and `_clippy-balance-lab` (`justfile:146-147`, root crate, no network-test). No recipe clippies `brawler-client` with `network-test,balance-lab`, although `_test-balance-lab` (`justfile:173`) already runs this test.
- Repo convention for float asserts is tolerance comparison, e.g. `tests/network/combat_splash.rs:89` (`(a - b).abs() < f32::EPSILON`). File-wide `#![allow]` is rejected by `scripts/check-lint-exceptions.sh`.

# Plan
1. `tests/network/builds.rs:92`: replace with `assert!((server_loadout.fighter_stats.movement_speed - 444.0).abs() < f32::EPSILON);` (matches existing convention; no lint exception needed).
2. `justfile`: add `_clippy-network-balance-lab:` running `cargo clippy --locked -p brawler-client --features network-test,balance-lab --tests -- -D warnings`, placed after `_clippy-balance-lab`, and add it to the `lint` dependency list at line 47.

# Acceptance
- `cargo clippy --locked -p brawler-client --features network-test,balance-lab --tests -- -D warnings` exits 0.
- `just lint` passes (needs `rg` on PATH for `_lint-exceptions`) and its output shows the new recipe running.
- `cargo test --locked -p brawler-client --features network-test,balance-lab --test network builds::revised_catalog_loadout_keeps_build_identity_and_replicates_authoritative_values -- --test-threads=1` passes.

# Out of scope
- Restructuring lint recipes into a feature matrix, and workspace crate split (BRL-qrb8).
