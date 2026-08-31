# Outcome

Add the first usable VfxLab as a thin client-only viewer over production VFX code. An operator can select a production recipe/scenario, replay it deterministically, inspect its resolved signature and active layers, switch presentation variants, and capture paired PNG/JSON evidence without starting a routed match.

# Scope

- Add a dedicated development binary or explicit development application mode and a canonical `just vfx-lab` entry point.
- Reuse production VFX catalog loading, validation, semantic resolution, retained mesh/material assets, materialization, animation, cleanup, and evidence serialization. The lab must not fork recipe schemas or renderer logic.
- Provide a neutral floor, production camera defaults, scale/reference fighter or target fixtures, and bounded fixtures for point, line/tracer, circular area, cone, lobbed path, status/attached, and explosion scenarios.
- Support selecting recipe/scenario, replay/pause/step/restart, normal/reduced/primitive presentation, self/ally/enemy/neutral relationship, relevant geometry inputs, theme/context override when production owns one, and an explicit deterministic seed/presentation identity.
- Show a compact inspector containing requested semantic signature, resolved recipe/fallback, ordered layers and readability classes, timing, active/root counts, retained asset counts, catalog fingerprint, validation warnings, and current variant.
- Capture the same paired PNG/JSON evidence contract used by production client evidence.

# Read-only boundary

- No recipe editor, arbitrary property controls, save/write/hot-reload pipeline, or network dependency.
- Selection and scenario inputs are bounded UI/runtime state; recipes remain authored in the production catalog.
- No gameplay authority simulation or duplicate damage/collision logic. Scenario fixtures supply presentation facts only.

# Acceptance criteria

- `just vfx-lab` starts without supervisor/server/network processes and presents at least the current production transient recipes plus representative lifecycle fixtures.
- Identical recipe, signature, presentation identity, and time produce identical authored layer choices across restart and replay.
- Variant switching visibly and diagnostically applies production normal/reduced/primitive rules, with CriticalGeometry retained.
- The inspector and JSON capture identify exact catalog fingerprint, semantic input, resolved recipe/fallback, layers, timing, and resource/root counts.
- Runtime entities and retained resources remain bounded through repeated replay and selection changes.
- The server feature graph remains free of rendering/windowing/client assets.

# Verification

Add focused pure/ECS tests for selection, deterministic replay, scenario mapping, variant switching, cleanup, and evidence serialization. Run role-specific client/server checks, formatting and linting, the world-presentation/server-feature guards, and a native operator pass covering at least point, line, area, attached/status, lobbed, and explosion fixtures.

# Sequencing

This is Phase 1 of BRL-0094 and blocks BRL-0099. Keep the viewer deliberately read-only. BRL-0096 owns the later authoring workbench and may begin only after production recipes have migrated and the read-only Lab has proven the shared seam.

## Implementation progress — initial production viewer slice, 2026-08-31

Implemented the first runnable read-only vertical slice:

- Added the client-only `brawler-vfx-lab` binary and canonical `just vfx-lab` command. It starts a Bevy window directly and installs no Lightyear client, routed transport, supervisor, lobby, match worker, or gameplay authority plugin.
- Extracted `ProductionVfxPlugin` as the one composition point for the existing sealed embedded catalog and all 15 feature-owned request registrations. Production `WorldPresentationPlugin` and VfxLab now consume that same plugin.
- VfxLab reuses the production `setup_3d_foundation`, retained primitive/material resources, request validation/resolution, reduced-profile selection, materializer, caps/eviction, lifetime cleanup, and catalog fingerprint.
- Added a neutral floor and read-only inspector showing selected request/profile, renderer/material, scale/anchor, lifetime, cap, fallback, variant, catalog fingerprint, active roots, camera count, and window dimensions.
- Added bounded controls: Left/Right selection, Space replay, R reduced toggle, P primitive toggle, Escape exit. CLI accepts `--effect`, `--reduced`, `--primitive`, and bounded `--window-size`.
- Documented the operator path and controls in README.

Verification passed:

- `cargo check --locked --no-default-features --features client --bin brawler-vfx-lab`
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings`
- `cargo test --locked --no-default-features --features client --lib client::vfx_lab` — 2 passed.
- Focused shared producer-order test — passed.
- Server all-target check, server-feature isolation guard, V3 world-presentation guard, and formatting — passed.
- Native smoke: `target/debug/brawler-vfx-lab --effect world-object.explosion --window-size 1280x720` remained stable until intentional termination, with no server/network process required.

BRL-0098 remains doing. Remaining acceptance scope includes production lifecycle fixtures beyond transient request mappings, pause/step/time control, paired PNG/JSON capture, richer diagnostics/validation errors, repeated-replay resource evidence, and the complete native scenario matrix.
