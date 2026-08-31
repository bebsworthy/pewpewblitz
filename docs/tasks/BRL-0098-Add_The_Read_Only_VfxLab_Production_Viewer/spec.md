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

## Implementation progress — deterministic lifecycle and evidence slice, 2026-08-31

Completed the remaining pre-recipe production-viewer foundation:

- Added deterministic playback ownership. Space restarts with the same explicit presentation identity, N selects a new identity, Enter pauses, and Period advances exactly one canonical `SIMULATION_TICK`. Persistent fixture clocks and transient timers use the same state rather than wall-clock sleeps while paused.
- Added bounded startup inputs `--identity N` and `--radius WORLD_UNITS` (1 through 256), plus `--paused`. Identity and radius are visible in the inspector and serialized in evidence.
- Added Tab-selectable lifecycle fixtures for straight projectile, lobbed projectile, elemental field, cone propagation, and attached fighter status. The fixtures create only representative production facts. A focused client-only composition plugin runs the existing production fighter/projectile/field/cone/status materializers and pose writers; it does not duplicate visual construction or gameplay authority.
- The attached-status fixture now uses the production fighter materializer as its scale/attachment reference. Straight and lobbed fixture positions advance from their replicated flight facts and the Lab tick; no damage, collision, delivery, or authority rules run.
- Added F12 paired PNG/JSON capture by reusing the production asynchronous screenshot/evidence path. Automation is available through `--capture-dir PATH --capture-after UPDATES --exit-after-capture`. Captures identify build, catalog fingerprint, fixture/request, resolved current production profile, identity, tick, radius/deadline inputs, variant flags, active lifecycle roots, persistent facts, and retained asset counts.
- Hid production HUD fragments from the isolated view and replaced unsupported inspector glyphs after the first native capture exposed both issues.
- Updated README and the durable art/presentation contract with the exact run path, controls, lifecycle ownership, deterministic behavior, and read-only boundary.

The current catalog remains the pre-BRL-0099 single-profile schema, so the Lab truthfully reports the resolved renderer/material/profile/fallback rather than inventing ordered recipe layers or readability classifications that production does not yet own. BRL-0099 will evolve this same inspector/evidence seam when production mappings resolve to layered recipes. Likewise, observer relationship and contextual override inputs become meaningful when BRL-0100 adds the production semantic signature; this ticket does not create a Lab-only signature model.

Native evidence matrix passed at 1280x720 using auto-capture and clean process exit:

- point: `combat.damage`, identity 77, paused — one transient root and exact `damage` sphere profile;
- explosion: `world-object.explosion`, paused — one transient root and exact authoritative-radius ground-ring profile;
- straight projectile: running — one production projectile root following straight-flight facts;
- lobbed projectile: running — one production projectile root following launch/landing ticks and visual arc;
- elemental field: paused — one production field root with exact authored radius;
- cone: running — one production cone root at the sampled propagation tick;
- attached status: identity 77, paused — one status root attached to one production fighter visual;
- degradation: Fire field, identity 77, radius 48, reduced plus primitive — exact critical area geometry remained visible and JSON recorded both variants.

Evidence directories are under `target/brl-0098-vfx-lab-*`; each contains one PNG and matching JSON. Visual inspection confirmed the inspector is legible, the gameplay HUD is absent, the attached marker has a fighter reference, and exact point/area/cone silhouettes remain readable.

Focused and role verification after the implementation:

- `cargo fmt --all -- --check` — passed.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings` — passed.
- `cargo test --locked --no-default-features --features client client::vfx_lab` — 3 passed.
- `cargo test --locked --no-default-features --features client client::presentation_3d::combat` — 30 passed.
- Existing renderer allocation tests cover oldest-first total-cap and per-profile-cap enforcement; the Lab always despawns prior fixture/effect owners before replay or selection replacement.
- `just _check-server` — passed.
- `just _server-features` — passed; no rendering/windowing/client assets entered the server graph.
- `just _v3-world-presentation` — passed.

## Feedback disposition and learning review

- Implemented: isolated production viewer, all current transient mappings, representative production lifecycle fixtures, deterministic restart/identity/pause/step, bounded radius input, normal/reduced/primitive selection, paired evidence, native matrix, documentation, and role-isolation verification.
- Sequenced to BRL-0099 by design: ordered layered-recipe/readability-class inspection, because the current production catalog has only one `VfxProfile` per mapping. The Lab already consumes the shared registry and evidence collector that BRL-0099 will evolve.
- Sequenced to BRL-0100 by design: self/ally/enemy/neutral semantic signature and context override controls, because those typed production inputs do not exist yet. No Lab-only substitute was introduced.
- No user feedback remains awaiting disposition for this slice.

The first native capture exposed two issues automated checks could not: the default font lacked several decorative Unicode glyphs, and production combat HUD entities appeared in the isolated window. Both came from composing production presentation pieces outside the full player shell. The correction uses plain ASCII inspector text and explicitly hides only the two production gameplay HUD marker types. Future development viewers should include a native evidence capture early, and should audit UI entities inherited through shared production setup rather than assuming a minimal plugin composition is visually isolated.

A second issue was that the initial attached-status fixture materialized only the status ring, leaving no scale or attachment reference. The cause was installing the status materializer without the production fighter materializer. The corrected focused lifecycle plugin installs both fighter reconciliation/pose writing and status reconciliation, and the fixture carries the exact fighter facts those systems require. Future isolated fixtures should include the complete presentation ownership chain for the behavior being inspected, not merely the final decorative root.
