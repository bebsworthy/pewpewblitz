# Context

`src/client/presentation_3d/environment_assets.rs` is 835 physical lines (about 784 NLOC). It owns two distinct concerns: the authored client presentation catalog boundary (RON source shapes, visual/theme profiles, coverage validation, lookup, and themed material construction) and runtime Bevy asset preparation (handles, imported-scene readiness, bounds/fitting, material cloning/tinting, validation, and system orchestration). Repowise reports health 2.00; `validate_map_visuals` is CCN 23 with a ten-operator condition, and `prepare_environment_scenes` is a ten-parameter, CCN-14 system. The existing `fitting.rs` already demonstrates a useful pure geometry boundary.

# Target ownership

Retain `environment_assets` as the narrow client-presentation facade used by `presentation_3d/mod.rs`. Separate focused private modules for:

- catalog: embedded RON source types, `MapVisualKind`, fitting/adjacency metadata, visual/theme profiles, exact shared-catalog coverage validation, lookups, theme material definitions, and catalog-focused tests;
- loading/runtime: asset handles, imported scene collection, readiness/degradation state, load requests, material tint components/observer, fitted scene access, scene preparation, and runtime validation;
- fitting: retain the existing pure scene-bounds and footprint-fitting owner unless a small interface adjustment is required;
- focused tests for catalog coverage/content invariants, material tint behavior, fitting, and runtime readiness.

Keep one module if a type genuinely crosses both phases, but expose it through the facade rather than broad sibling imports.

# Function-level improvements

- Decompose `validate_map_visuals` into named predicates/validators for ID coverage, source kind/path, transform/fitting values, fallback/adjacency compatibility, and shared gameplay-profile expectations.
- Decompose `validate_map_themes` into named color/light/material constraints while preserving strict rejection and errors.
- Turn `prepare_environment_scenes` into a clear readiness coordinator over catalog availability, handle load state, scene validation, bounds/fitting, material preparation, and final ready/degraded commit.
- Separate pure preparation/planning from Bevy resource mutation where it improves deterministic tests; do not introduce a custom asset framework.
- Keep material tint cloning and scene-owned asset validation local to runtime preparation.
- Remove inline catalog tests from the production body and use explicit imports/visibility.

# Presentation and compatibility constraints

- Preserve every embedded catalog path and parsed RON schema, exact visual/theme coverage, profile IDs, asset paths, scales, yaw, offsets, tints, fallbacks, fitting modes, adjacency groups, theme colors, light values, and material behavior.
- Preserve `presentation_3d/mod.rs` call sites and effective crate-visible API unless a strictly internal visibility reduction is verified.
- Preserve load timing, readiness/degraded transitions, imported scene ownership, fitting geometry, alpha/color multiplication, and fallback behavior.
- Keep the entire boundary client-only. No server/headless feature graph may acquire Bevy rendering, AssetServer, meshes, materials, scenes, or client catalog assets.
- Preserve the separation between shared gameplay map catalogs and client-only visual catalogs; presentation validation must not become gameplay authority.
- Do not modify asset files, visual design, generated fallbacks, or map content.

# Acceptance criteria

- Embedded visual/theme catalog ownership is separate from runtime asset loading/preparation, with `environment_assets` remaining a narrow facade.
- Catalog source types, validation, lookups, and coverage tests live together; runtime handles, readiness, material tinting, fitted scenes, and preparation live together.
- `validate_map_visuals` and `validate_map_themes` express rules through named validators rather than large compound conditions, with equivalent error coverage.
- `prepare_environment_scenes` is a readable readiness coordinator and no longer contains all load-state, validation, fitting, material, and commit policy inline.
- Existing fitting behavior remains in the focused pure owner and is covered by representative exact/tiled/contained tests.
- All visual IDs, paths, profile values, theme values, load/degradation behavior, material tints, scene bounds, and rendered fallback choices remain unchanged.
- Existing `presentation_3d` consumers compile without widened visibility or a new public API.
- Server/headless feature checks prove no client rendering/assets dependency leaks.
- Focused environment/presentation tests plus `just fmt`, `just check`, `just lint`, and `just test` pass.
- Native visual smoke covers imported and generated environment assets across representative themes/maps, including degraded fallback behavior.
- Repowise health is rerun and remaining catalog/test duplication or co-change is dispositioned without a numeric target.
- Verification evidence, visual feedback disposition, learn-from-errors review, and conflict-free `ticket sync` are recorded before completion.

# Non-goals

- No art, theme, lighting, map, fitting, asset-path, or fallback redesign.
- No shared/server asset loading or general asset-management framework.
- No hard line-count or health-score target.

# Implementation evidence

Completed the organization-only refactor with `environment_assets` retained as a 15-line facade over three focused private owners:

- `catalog.rs` owns embedded RON sources, visual/theme definitions, exact gameplay-catalog coverage, validation predicates, lookups, and catalog tests.
- `runtime.rs` owns Bevy material handles, asset requests, tint cloning, imported-scene admission, readiness/degradation calculation, and resource publication.
- `fitting.rs` remains the pure bounds/footprint fitting owner and was not behaviorally changed.

`validate_map_visuals` now coordinates named kind/fallback, fitting/adjacency, color, transform, imported-scale, and path predicates. `validate_map_themes` coordinates named color and lighting validators. `prepare_environment_scenes` now coordinates three explicit phases: loaded-scene admission, readiness calculation, and changed-resource publication. Existing facade call sites, RON schemas and paths, profile/theme values, error strings, fallback policy, scene fitting, and client-only visibility were preserved.

# Verification

- `cargo check --no-default-features --features client`: passed.
- `cargo check --no-default-features --features server`: passed; no client rendering/asset dependency leaked into the headless role.
- `cargo clippy --locked --all-targets --no-default-features --features client -- -D warnings`: passed.
- Focused `client::presentation_3d::environment_assets` tests: 12 passed, including exact/tiled/contained fitting, catalog coverage/path invariants, and tint behavior.
- `just fmt`: passed.
- `just check`: passed across routing, client, server, network-test, Balance Lab web, and Balance Lab Rust targets.
- `just lint`: passed, including server feature isolation and renderer/map boundary checks.
- `just test`: passed: 428 client, 337 server, 354 Balance Lab, 88 network, 12 performance tests, and all routing/support suites.
- Repowise health for the new module: 8.29/10 healthy average, 10/10 hotspot, 0 alerts, and no static performance-risk findings. `catalog.rs` is 9.1 and `runtime.rs` is 8.8. The remaining warning is `fitting.rs` (6.35), whose geometry algorithm is cohesive, pre-existing, and covered by representative tests; no further split is justified by this ticket.

# Native visual evidence and feedback disposition

- Verdant Crossfire Wipeout 3v3, recipe 10/theme 3, `fallback=imported-auto`: canonical 10-second warmup plus 30-second measurement passed for both native clients (`target/brl-0030-verdant-imported-canonical.txt` and peer report).
- Powderline Vault Heist 3v3, recipe 12/theme 2, `BRAWLER_FORCE_PRIMITIVE_WORLD=1`, `fallback=primitive`: canonical measurement passed for both native clients on the repeat (`target/brl-0030-powderline-primitive-retry.txt` and peer report).
- A deliberately shortened 10-second imported measurement was rejected only because the locked report requires the canonical sample count. The first canonical primitive run rendered the correct map/theme/fallback but caught one isolated >100 ms frame; the immediate identical repeat passed both reports, so this was disposed as environmental pacing jitter rather than a reproducible presentation regression.
- Existing scene-admission warnings and Bevy duplicate-despawn warnings observed during smoke are pre-existing runtime diagnostics outside this organization-only ticket; no visual, asset, map, or fallback change was made.

# Learn-from-errors review

The initial mechanical extraction exposed two predictable module-boundary errors: embedded asset paths needed one additional parent segment, and former `pub(super)` visibility became too narrow after adding a facade layer. The compile-first pass caught both before behavioral work. Future Rust module splits should explicitly inventory relative `include_str!` depth and translate visibility against the intended facade boundary before moving bodies. A second lesson is to retain the canonical native evidence duration: shortening the measurement saved little time and produced a non-actionable sample-count failure. No new reusable skill is warranted; these checks belong in the normal decomposition workflow.
