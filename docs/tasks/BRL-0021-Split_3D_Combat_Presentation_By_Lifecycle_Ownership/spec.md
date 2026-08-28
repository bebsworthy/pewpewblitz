# Outcome

Replace `src/client/presentation_3d/combat.rs` with a `combat/` composition module whose focused submodules own distinct client-presentation lifecycles. The refactor must preserve native visuals and gameplay behavior while making schedule dependencies, local entity relationships, and future customization easier to review.

# Evidence and motivation

The current file contains roughly 2,750 lines and combines independently changing concerns:

- fighter, projectile, sentry, and concealment-field root materialization and cleanup;
- fighter ground markers and concealment material variants;
- projected fighter names, health, and ammunition UI;
- durable slow, knockback, reveal, and dash feedback;
- controlled-player aim preview calculation and rendering;
- combat, world-object, pickup, and Heist transient cue effects;
- bounded effect allocation and cleanup;
- fighter, projectile, sentry, and status pose updates;
- tests for all of the above.

BRL-0014 already separated the former all-in-one combat state system into focused systems. This ticket completes the ownership boundary by separating the physical module and the remaining multi-family reconciliation and pose systems.

# Module design

Create `src/client/presentation_3d/combat/` with a composition-only `mod.rs` and focused implementation modules:

- `entities.rs`: durable fighter, projectile, sentry, and concealment-field roots plus their focused pose writers and fighter ground-marker ownership;
- `fighter_ui.rs`: projected overhead root, name, health, ammunition state, and projection;
- `fighter_feedback.rs`: concealment material variants, status visuals, and dash trails;
- `aim_preview.rs`: controlled aim collection, dynamic blockers, targeted-ultimate calculation, and preview visual application;
- `effects.rs`: cue-family conversion to transient world-effect descriptors, one bounded allocator, and cleanup.

The exact private helper placement may vary when ownership is clearer, but `combat/mod.rs` must remain an intentional composition and narrow re-export surface rather than another implementation dump.

Do not create one plugin per module. `WorldPresentationPlugin` remains the client 3D presentation composition owner.

# Reconciliation and scheduling

Split the current `reconcile_combat_visuals` orchestration into focused lifecycle systems. Each system owns materialization, duplicate repair where applicable, and orphan cleanup for its visual family.

Preserve these explicit constraints:

1. fighter-root reconciliation runs before `upgrade_fighters_to_imported_models` so a newly replicated fighter can be upgraded in the same presentation phase;
2. all reconciliation remains inside `WorldPresentationSet::ReconcileState`;
3. cue-family consumers retain their deterministic combat → world object → pickup → Heist order before shared effect materialization;
4. fighter pose writing runs after Lightyear interpolation and Avian writeback and before camera follow;
5. projectile, sentry, and status poses run after interpolation/writeback and before transform propagation;
6. fighter overhead projection remains after transform propagation and camera update and before UI preparation;
7. no new ordering edge is introduced where systems only read common state.

# Direct lookup simplification

- Give each overhead root a direct process-local link to its fighter visual root, or an equivalent focused index, so projection uses `Query::get` instead of scanning all fighter visuals per overhead.
- Read overhead fighter data with `fighters.get(owner)` rather than rebuilding a per-frame `HashMap` keyed by the same owner entity.
- Avoid unconditional text allocation when the displayed name or health amount has not changed, where this can be done without adding a general cache framework.
- Replace nested combat-cue owner/visual scans with one bounded per-pass lookup or direct local relationship if it remains simpler than a persistent cache.
- Keep direct `Entity` links client-local. Do not add process-local entities to replicated or wire-visible types.

# Customizability constraints

- Keep visual constants, material selection, mesh selection, transforms, labels, reduced-effects policy, and cue-family validation local to the owning presentation module.
- Preserve `Primitive3dAssets` and `Material3dAssets` as the current shared asset palettes.
- Preserve the common transient-effect descriptor and bounded allocation service; do not replace it with an arbitrary effect graph, renderer abstraction, or configurable framework.
- Preserve imported fighter models and primitive fallback behavior.
- Preserve concealment privacy and observer-relative team presentation.

# Compatibility boundaries

- Client-only refactor: no authority, protocol, content schema, stable ID, input, simulation, or server-feature change.
- Preserve public and crate-visible paths outside `presentation_3d`; new submodules and implementation types should remain private by default.
- The headless server feature graph must remain free of rendering, UI, input-device, audio, and client asset dependencies.
- Do not modify unrelated user files, including `asset_src/blocks/brawler-blocks.blend1` and `notes.md`.

# Acceptance criteria

- The monolithic `combat.rs` is replaced by a focused `combat/` module tree with composition-only `mod.rs`.
- Durable visual reconciliation and pose writing are split into systems with one recognizable lifecycle each.
- Fighter-root upgrade, camera-follow, transform-propagation, cue ordering, and UI-projection schedule contracts remain explicit and tested or compile-validated.
- Overhead projection performs direct visual-root lookup; overhead state no longer builds a redundant per-frame fighter map.
- Cue-triggered fighter animation avoids nested full-query scans.
- Existing fighter, projectile, sentry, concealment, overhead/ammunition, status, dash, aim-preview, transient-effect, and diagnostics behavior remains intact.
- Tests move beside their owning implementation and retain the existing behavioral coverage.
- `cargo fmt --all -- --check` passes.
- Client and server role-specific checks pass.
- Focused combat-presentation tests and the complete client suite pass.
- Strict client Clippy with warnings denied passes.
- `git diff --check` and `ticket sync` pass.

# Verification plan

1. Run formatting and both role-specific compilation checks.
2. Run focused combat-presentation tests after the module move and schedule split.
3. Run the complete client test suite.
4. Run strict client Clippy with `-D warnings`.
5. Inspect the final schedule composition and module dependency surface.
6. Record exact evidence here and move the ticket to done only after every acceptance criterion holds.


# As built

Implemented the combat presentation as a directory module with a 45-line composition root and six focused implementation modules:

- `common.rs` owns the observer-relative fighter relation, shared ground-effect height, and duplicate-root helper;
- `entities.rs` owns fighter, projectile, sentry, and concealment-field roots and separate fighter/projectile/sentry pose systems;
- `fighter_ui.rs` owns overhead root reconciliation, health/name/ammunition state, and post-propagation projection;
- `fighter_feedback.rs` owns concealment materials, durable status markers, dash trails, orphan cleanup, and status poses;
- `aim_preview.rs` owns preview slot materialization, dynamic blocker collection, targeted-ultimate calculation, and visual application;
- `effects.rs` owns combat/world-object/pickup/Heist cue conversion, deterministic shared allocation, and cleanup.

`reconcile_combat_visuals` was removed. Fighter root creation, imported-model upgrade, and overhead linking are one explicit chain; the other durable families reconcile independently in `WorldPresentationSet::ReconcileState`.

`write_combat_visual_poses` was removed. Fighter pose remains chained directly before camera follow, while projectile, sentry, and status poses share only the required interpolation/writeback and transform-propagation boundaries.

Each fighter overhead now stores a client-local direct link to its durable fighter visual root. Projection uses `Query::get` on that entity instead of scanning every fighter visual. Overhead state uses `fighters.get(owner)` rather than allocating a redundant per-frame fighter map, reuses the existing text buffers for names, and formats health only when the displayed value changes.

Combat cue animation builds bounded owner indexes once per presentation pass and performs direct mutable visual lookup for accepted attacks instead of nesting full owner and visual scans.

The shared transient-effect descriptor, cue-family policies, material palette, mesh palette, labels, reduced-effects behavior, imported/fallback model behavior, concealment privacy, and observer-relative team colors remain unchanged.

# Verification evidence — 2026-08-28

- `cargo fmt --all -- --check` passed.
- `cargo check --locked --no-default-features --features client --all-targets` passed.
- `cargo check --locked --no-default-features --features server --all-targets` passed, preserving role isolation.
- Focused combat-presentation tests passed: 20/20.
- `cargo test --locked --no-default-features --features client --all-targets` passed: 421 library tests and the client binary target, with no failures.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings` passed.
- `git diff --check` passed.
