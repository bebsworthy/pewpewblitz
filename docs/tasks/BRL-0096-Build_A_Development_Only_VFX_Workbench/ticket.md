---
id: BRL-0096
title: Build a development-only VFX Workbench
status: todo
theme:
release:
created: 2026-08-31T08:18:17Z
modified: 2026-08-31T08:21:28Z
closed:
revision: a2a73ae668b06fc7
blocks: []
related: [BRL-0094, BRL-0098]
---

# Description

## Outcome

Build **VfxLab**, a development-only native Bevy workbench for viewing, comparing, diagnosing, and eventually authoring Brawler's composable combat visual effects in relative isolation.

VfxLab must make it fast to answer these questions without starting a routed match:

- What does a recipe look like from the production camera?
- How does it read against fighters, terrain, cover, objectives, and team colors?
- Does the same semantic effect remain recognizable in normal, reduced, and primitive presentation?
- How do activation, active, pulse, impact, expiry, owner-removal, and late-join reconstruction look?
- Which authored layers and resolved overrides produced the visible result?
- Is variation repeatable from a known seed?
- Does an edit validate, what changed, and can it be safely saved?
- Does the effect clean up correctly under representative and stress scenarios?

The workbench exists to shorten the visual iteration loop while protecting the game's presentation architecture. It must exercise the same authored catalogs, resolution rules, layer materializers, materials, meshes, particle adapters, cleanup paths, and production camera assumptions used by the actual client. A preview that succeeds through a private lab renderer but fails in gameplay is not acceptable.

## Product role

VfxLab is distinct from Balance Lab.

Balance Lab remains a development-only, server-authoritative gameplay tuning workflow. VfxLab is a client-only presentation tool. It must not calculate hits, damage, status application, objective progress, authoritative area membership, projectile collision, or any other gameplay outcome. It supplies bounded fixtures representing already-decided gameplay facts and geometry, then presents them using production VFX code.

VfxLab is also not a player-facing feature. It may use development-only assets, diagnostics, controls, and launch paths. The dedicated-server feature graph must not acquire rendering, windowing, input, audio, or VFX authoring dependencies.

## Application shape

Prefer a thin brawler-vfx-lab binary and a cohesive client-only VfxLabPlugin, launched by a canonical command such as just vfx-lab. The application may compose the minimum production client presentation plugins it needs, but it must not boot networking, admission, matchmaking, or a gameplay worker.

The workbench should use one native window with:

- a top bar for recipe, semantic scenario, presentation variant, theme, and seed selection;
- the production 3D preview viewport as the primary surface;
- an inspector showing the semantic input, resolved recipe, ordered layers, authored values, derived values, fallbacks, and warnings;
- a bottom timeline for playback, phase selection, scrubbing, stepping, looping, and speed;
- a footer or side status area for validation, dirty state, draft-versus-disk diff, undo/redo, revert, reload, save, capture, and report actions.

Native Bevy UI is the preferred initial implementation so the tool shares one process, ECS world, assets, and renderer with production presentation. A browser-based editor is not a prerequisite. If authoring complexity later demonstrates a need for a separate control surface, that should be a follow-up ticket with an explicit synchronization boundary.

## Production reuse contract

VfxLab must consume production interfaces rather than duplicate them:

- the client-only VFX catalog and its schema;
- catalog parsing, validation, normalization, and stable recipe identities;
- semantic presentation signatures and the production resolver;
- mesh, material, trail, decal, particle, and lifecycle materializers;
- production relationship/team-color policy;
- production normal, reduced, and primitive presentation variants;
- production camera and world-coordinate conventions;
- production cleanup, bounds, pooling, and diagnostics;
- existing screenshot plus JSON evidence capture where applicable.

If current presentation code cannot be reused without booting the whole game client, extract only the focused VFX materialization boundary demonstrated by both consumers. Do not create a second gameplay-world renderer, a second recipe schema, or lab-only effect implementations.

## Preview modes

VfxLab should support two complementary modes.

### Recipe mode

Recipe mode selects a concrete authored recipe and instantiates it directly with explicit fixture geometry. It is the primary mode for inspecting layers, tuning authored values, comparing revisions, and testing validation.

### Context mode

Context mode constructs a semantic presentation request using the same dimensions gameplay supplies, including carrier or delivery family, lifecycle phase, elemental or stylistic accent, relationship, optional contextual override, geometry, stable event or field identity, and authoritative timing. The production resolver chooses the recipe and derived parameters. The UI displays both the semantic input and the resolution trace.

Context mode is essential because visually correct recipes can still be wired incorrectly. It must reveal whether a projectile, impact, persistent field, ultimate, sticky payload, or status presentation resolves to the intended identity.

## Scene fixtures

The default scene should be small, deterministic, and visually representative:

- a neutral unit grid and scale references;
- the production gameplay camera with camera preset controls;
- a fighter dummy and ally/enemy pair;
- ground, wall, tall grass, damageable object, and objective references;
- a production map-theme toggle or bounded representative theme fixtures;
- optional overlays for exact gameplay radius, width, length, origin, direction, hit normal, path, and ground projection.

Required lifecycle fixtures should cover:

- point activation and point impact;
- oriented surface impact;
- moving straight projectile;
- lobbed projectile with planar ground shadow;
- attached projectile or fighter trail;
- circle and rectangle persistent areas;
- sticky countdown and detonation;
- fighter-attached status;
- activation, active, pulse, impact, expiry, and interruption phases;
- owner removal and early cleanup;
- reconstructed persistent state at representative 25%, 50%, and 90% elapsed points.

Fixtures provide presentation inputs only. They must not reproduce combat authority.

## Time, variation, and replay

The tool must make presentation behavior reproducible:

- play, pause, restart, single-step, playback-speed, and loop controls;
- scrubbing for mesh/material timelines;
- deterministic reconstruction for persistent effects;
- stable default seed derived from semantic event or field identity plus recipe/layer identity;
- explicit seed entry, randomize, replay-seed, and copy-seed actions;
- visible recipe revision, scenario, variant, geometry, seed, and authoritative timing in captures and reports.

Authored randomness is allowed inside declared bounds, but the same inputs and seed must produce the same choices. GPU particle simulation may be statistically repeatable rather than bit-identical across hardware. Reverse particle scrubbing may reset and simulate forward to the requested time.

## Presentation comparisons

The workbench must make degradation and readability first-class:

- normal, reduced, and primitive presentation variants;
- bloom off/on comparison if bloom becomes part of the production contract;
- self, ally, enemy, and neutral relationship variants where meaningful;
- elemental or style identity comparisons such as neutral, Cold, and Fire;
- clean-scene and representative-clutter backgrounds;
- synchronized side-by-side viewports when justified, with reproducible sequential screenshots acceptable for the first slice.

Gameplay-critical information—geometry, team relationship, impact location, active-area boundary, and timing—must remain legible without bloom and after optional layers are removed.

## Authoring state and save safety

The full authoring workbench should maintain three explicit catalog states:

1. **Disk canonical catalog**: the last catalog loaded from the repository.
2. **In-memory draft catalog**: mutable editor state.
3. **Last valid compiled preview catalog**: the last draft accepted by production validation and used by the renderer.

Every edit runs production validation. A valid edit updates the preview. An invalid edit remains visible in the form with actionable errors while the preview retains the last valid compiled state. The UI must clearly distinguish disk, draft, and preview state.

VfxLab must never autosave. It must support:

- obvious dirty state;
- undo and redo;
- field, recipe, and full-catalog revert;
- reload from disk with dirty-state protection;
- normalized, human-reviewable RON output;
- draft-versus-disk diff;
- validation before save;
- atomic save only to the canonical allowed VFX catalog location;
- capture and report export under target/vfx-lab/.

Arbitrary filesystem paths, silent format rewrites, and saving an invalid catalog are out of scope.

## Validation and warnings

In addition to schema validation, surface presentation-specific warnings where production rules can detect them:

- reduced or primitive variants drop every gameplay-critical layer;
- boundary geometry disagrees with supplied gameplay geometry;
- semantic identity depends only on color;
- alpha, duration, scale, rate, particle count, or bounds exceed policy;
- a fallback is missing or cyclic;
- a particle-backed recipe has no required mesh/readability fallback;
- a persistent recipe cannot reconstruct from authoritative timing;
- cleanup, capacity, or lifecycle ownership is unbounded;
- an authored asset or material reference is unavailable.

Warnings should identify the recipe, layer, field, and policy involved.

## Diagnostics and stress scenarios

Support bounded load scenarios representative of actual matches:

- configurable simultaneous effect count and spawn frequency;
- persistent fields and overlapping 1v1, 2v2, and 3v3 combat;
- mixed elemental identities;
- repeated impacts and explosions;
- controlled run duration followed by cleanup observation.

Expose useful diagnostics such as:

- FPS and frame-time percentiles;
- active VFX roots and layers;
- active emitters and particles when available;
- VFX-owned entity, mesh, material, image, and particle counts;
- cache or pool hit, miss, eviction, high-water, and terminal counts;
- validation, materialization, fallback, and cleanup failures;
- post-scenario residual entities or assets indicating a leak.

## Evidence and automation

The interactive workbench is primary, but important previews must be reproducible from a command. A later automation surface should accept bounded arguments for recipe or semantic scenario, presentation variant, seed, duration or sample time, capture directory, and report output.

Reuse the existing PNG plus JSON evidence-capture path and render diagnostics rather than inventing a parallel artifact format. A capture should record enough context to reproduce the view: recipe identity and revision, semantic signature when used, scenario, theme, presentation variant, seed, geometry, authoritative elapsed/duration values, camera preset, and validation state.

## Delivery phases

1. **Read-only viewer** — launch the isolated app, load canonical production recipes, select recipes/scenarios/variants/seeds, play lifecycle fixtures, inspect resolution/layers, and capture evidence. This phase belongs in the ordered BRL-0094 implementation sequence because it establishes a baseline and supports parity checks while the richer recipe system is built.
2. **Mesh recipe editor** — editable mesh/material/timeline layers with validation, draft/compiled state, undo/redo, diff, revert, atomic save, and normalized RON.
3. **Semantic context and lifecycle expansion** — resolution tracing, all persistent/transient lifecycle fixtures, late-join reconstruction, degradation warnings, and relationship/theme comparison.
4. **Particle editor** — production particle-adapter properties, deterministic seed controls where supported, capacity diagnostics, and mandatory readability fallback checks.
5. **Automation and closeout** — stable non-interactive capture/report interface, representative stress presets, durable operator documentation, and accepted native workflow.

Each phase should be independently reviewable and deliver a usable vertical slice. The full editor must not block the first distinctive weapon VFX slice; the read-only viewer is the only VfxLab phase required inside BRL-0094 unless that ticket's scope is explicitly changed.

## Non-goals

VfxLab is not:

- a gameplay, balance, weapon-recipe, map, or shader-graph editor;
- a source of gameplay authority or replicated state;
- a replacement for gameplay/native verification;
- a general-purpose asset browser or arbitrary filesystem editor;
- a second renderer, recipe schema, or effect runtime;
- permission to make effects depend on frame rate or local wall-clock time;
- permission to let presentation determine collision, damage, status, scoring, or cleanup truth.
