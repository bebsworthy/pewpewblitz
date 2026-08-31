# Technical specification

## Scope contract

Deliver a development-only, client-feature-only VfxLab that previews production combat VFX in isolation and grows through five independently reviewable phases. BRL-0094 owns only Phase 1, the read-only viewer, as an early implementation dependency. Later authoring phases remain owned by this umbrella and should receive separate child tickets before implementation.

This ticket may extract a focused production VFX plugin/API when reuse by the gameplay client and VfxLab demonstrates the boundary. It does not authorize gameplay protocol changes, server dependencies, a new renderer, a second recipe schema, or lab-only visual implementations.

## Architecture

### Executable and feature boundary

- Add a thin development binary, provisionally src/bin/vfx_lab.rs, behind the existing client/rendering feature boundary.
- Add a canonical just vfx-lab operator command.
- Compose a VfxLabPlugin from focused client-only plugins/resources.
- Do not install Lightyear transport, session admission, routed match lifecycle, server authority, combat mutation, HUD, or player input plugins unless a concrete production VFX dependency is extracted away from them.
- Preserve the headless server feature graph and role-specific build checks.

### Production VFX boundary

The gameplay client and lab must share:

- catalog types, loader, parser, validator, normalization, and stable IDs;
- semantic request and resolution logic;
- resolved recipe and layer representation;
- layer instantiation/materialization;
- timeline sampling and authoritative-time reconstruction;
- relationship and presentation-variant policy;
- bounded resource ownership, cleanup, and diagnostics.

The shared boundary should be a focused presentation plugin/module under client ownership. Its public surface should remain crate-private unless an actual external consumer is demonstrated. Gameplay cues remain presentation facts; VfxLab fixtures may construct equivalent local facts but do not register network messages or mutate gameplay.

### Lab-owned state

Use explicit resources/components for:

- selected recipe and selected semantic scenario;
- selected scene fixture, theme, relationship, and presentation variant;
- stable seed and randomization controls;
- playback state, sample time, speed, looping, and phase;
- inspection snapshot and resolution trace;
- validation issues and presentation warnings;
- capture/report request and result;
- diagnostics and stress-run state;
- later phases: disk catalog, mutable draft catalog, last valid compiled preview, dirty state, undo/redo history, and safe-save status.

Keep UI intent, editor/catalog state, resolved preview state, and spawned VFX entities distinct.

### System ordering

Make scheduling explicit at the VfxLab composition point. A representative order is:

1. collect UI/CLI intent;
2. update selection, playback, or draft state;
3. validate/compile draft when changed;
4. resolve semantic requests;
5. despawn or reset obsolete preview ownership;
6. materialize or advance production VFX;
7. collect render/VFX diagnostics;
8. process capture/report requests;
9. render UI inspection state.

Use deferred-command boundaries where entity replacement requires them. The lab must not introduce a second materialization schedule that behaves differently from gameplay.

## Catalog state and editing transaction

Phases 2–4 use three states:

- DiskCatalog: immutable snapshot loaded from the canonical repository asset.
- DraftCatalog: mutable editor model plus bounded undo/redo transactions.
- CompiledPreviewCatalog: most recent DraftCatalog accepted by production validation.

On edit:

1. apply one bounded editor transaction to DraftCatalog;
2. run production validation plus lab warnings;
3. if valid, rebuild the affected compiled recipe/catalog and refresh the preview;
4. if invalid, retain the prior CompiledPreviewCatalog while displaying the invalid draft and exact issues;
5. mark dirty based on normalized draft-versus-disk content, not UI history.

On save:

1. require a valid draft;
2. normalize deterministically;
3. show or make available the disk diff;
4. reject paths outside the canonical VFX catalog target;
5. write atomically;
6. reload and revalidate the written catalog;
7. update DiskCatalog only after successful reload;
8. preserve a useful error and draft state if any step fails.

Never autosave. Never infer gameplay data changes from a VFX edit.

## Semantic request and resolution trace

The production semantic presentation request should be rich enough for current demonstrated uses without becoming an open-ended tag language. Expected dimensions are:

- carrier/delivery family;
- lifecycle phase;
- accent/style identity;
- relationship;
- optional owned contextual override;
- supplied geometry and orientation;
- stable event or field identity;
- authoritative start, elapsed, and duration values where persistent;
- presentation variant.

Resolution must be deterministic for the same catalog and request. The inspector should expose:

- input dimensions;
- selected recipe ID and revision/fingerprint;
- applied fallback path;
- resolved ordered layers;
- derived seed per recipe/layer;
- fields derived from gameplay geometry or authoritative timing;
- warnings and unavailable optional capabilities.

## Determinism and randomness

- Mesh/material timelines sample from explicit elapsed time.
- Persistent effects reconstruct from authoritative timing and stable field identity.
- Random choices use a stable seed derived from event/field identity, recipe identity, layer identity, and an explicit salt where needed.
- The tool exposes the effective seed and lets the operator replace, copy, randomize, and replay it.
- Randomized values remain within authored bounds and do not affect collision, damage, status, area membership, targeting, or cleanup truth.
- GPU particle output need not be pixel-identical across hardware, but emitted counts, parameter bounds, and seed metadata should be reproducible where the adapter permits.
- Scrubbing backward through a particle effect resets and simulates forward rather than requiring reversible GPU state.

## Read-only Phase 1

Phase 1 is implemented in the BRL-0094 sequence and must include:

- isolated native launch without a server or network;
- canonical production VFX catalog loading and visible validation failure;
- recipe selection and a bounded set of semantic scenarios;
- production camera and representative scene fixtures;
- normal, reduced, and primitive variants;
- relationship, theme, geometry-overlay, and stable-seed controls where supported;
- play, pause, restart, single-step, speed, phase loop, and representative elapsed-time reconstruction;
- inspector for semantic input, selected recipe, layers, fallback, timing, seed, and warnings;
- production VFX materialization and cleanup;
- screenshot plus JSON capture using the existing evidence path or a small shared extraction;
- no editing, no dirty state, and no saving.

The read-only viewer must be useful before the full editor exists and must capture the pre-migration/current-effect baseline where practical.

## Mesh editor Phase 2

Add forms for demonstrated mesh/material layer fields such as shape, dimensions, color/gradient, opacity, emissive contribution, transform, orientation, lifetime, scale/fade timeline, pulse, attachment, sorting/depth bias, variant importance, and fallback. Support validation, draft/compiled state, undo/redo, diff, revert, reload, and atomic normalized save.

Do not add a general node graph. Layer order plus typed layer fields is the editing model.

## Context/lifecycle Phase 3

Add complete semantic context construction and trace, persistent-area and attached-status lifecycle fixtures, owner removal, interruption, late-join reconstruction, relationship/theme comparisons, degradation warnings, and representative gameplay-clutter presets.

## Particle Phase 4

Only after the production particle adapter is selected and proven:

- edit the supported bounded emitter/spawn, initial attribute, force/motion, lifetime, size, rotation, color-over-life, mesh/billboard, and capacity fields;
- display adapter capability limitations;
- expose active emitter/particle and capacity diagnostics;
- require a gameplay-readable mesh/material fallback for critical effects;
- preserve deterministic seed metadata and documented hardware limitations;
- reject unbounded rates, lifetimes, or capacities.

## Automation Phase 5

Add a stable non-interactive mode or bounded CLI surface with arguments for recipe or semantic scenario, variant, seed, sample time/duration, fixture/theme, capture directory, and report path. It must produce non-interactive exit status and machine-readable JSON without requiring a connected server.

## Validation policy

Errors block compilation/save. Warnings allow preview but must remain visible.

Required error classes:

- schema/type/range errors;
- missing recipe, layer, material, mesh, or asset references;
- cyclic fallback/reference graphs;
- invalid timeline ordering or non-finite values;
- unbounded runtime allocation/rate/lifetime;
- unsupported required adapter capability;
- invalid canonical save target.

Required warning classes where applicable:

- all critical layers absent in reduced/primitive;
- geometry visualization disagrees with supplied fixture;
- identity relies only on color;
- particle-backed critical presentation lacks fallback;
- persistent presentation cannot reconstruct;
- excessive expected overdraw/entity/particle cost;
- authored values outside recommended readability policy.

## Fixtures

Provide typed, bounded fixtures rather than free-form entity spawning:

- point activation;
- oriented impact;
- straight projectile;
- lobbed projectile and shadow;
- attached trail;
- circular and rectangular area;
- sticky countdown/explosion;
- fighter status;
- activation/active/pulse/expiry/interruption;
- owner removal;
- persistent reconstruction at 25/50/90 percent;
- representative 1v1, 2v2, and 3v3 stress presets.

Fixture geometry must be displayed and passed through the same conversion rules as gameplay presentation.

## Diagnostics and bounded ownership

All VfxLab-spawned preview entities belong to an explicit preview generation/root so replacement and cleanup are total. Track active and high-water counts for roots, layers, entities, materials/meshes created by VFX, emitters, and particles when exposed. Stress presets must have fixed duration/capacity and a cleanup observation interval.

Avoid per-frame asset creation. Reuse/cache immutable meshes and materials according to the production runtime policy. Report cache/pool terminal counts and residual preview ownership after reset.

## Evidence

Every phase records proportional evidence in its child ticket.

Automated evidence should include:

- client and server role-specific compilation;
- focused catalog validation/resolution/timeline tests;
- small App/World tests for preview ownership, replacement, reconstruction, and cleanup;
- normalized serialization/diff/save transaction tests once editing exists;
- non-interactive automation tests once Phase 5 exists.

Native evidence should include:

- launch command and controls;
- representative recipe/scenario/variant/seed captures;
- readability against scene fixtures;
- deterministic replay observations;
- normal/reduced/primitive comparison;
- invalid-data behavior for editing phases;
- stress-run terminal diagnostics.

Visual evidence complements rather than replaces automated checks.

## Acceptance criteria

The umbrella is complete only when:

- VfxLab launches through a documented canonical command without starting networking or a gameplay server.
- It uses the production VFX catalog, resolver, renderer/materializers, camera conventions, variants, cleanup, and evidence machinery; no second renderer/schema/runtime exists.
- Recipe and context modes cover the demonstrated transient, projectile, impact, persistent-area, status, sticky, and explosion lifecycle fixtures.
- Time controls and stable seeds allow a selected presentation to be reproduced and its context captured.
- Normal, reduced, and primitive presentation can be compared and critical readability warnings are visible.
- The editor maintains disk, draft, and last-valid-preview states; invalid edits never replace the valid preview or reach disk.
- Undo/redo, revert, reload protection, normalized diff, validation, and atomic canonical save are implemented for supported mesh and particle recipe fields.
- Particle editing is bounded by the proven production adapter and exposes capacity/fallback limitations.
- Stress presets finish with bounded terminal diagnostics and no residual preview ownership.
- Screenshot plus JSON evidence and the documented non-interactive capture/report path reproduce important scenarios.
- The dedicated-server feature graph remains free of rendering/editor dependencies.
- Focused/canonical verification passes, native workflow feedback is dispositioned, durable documentation is updated, and the ticket learning review is recorded.

## Non-goals and deferred choices

- No general shader or material graph.
- No gameplay-definition, weapon-stat, map, audio, animation, or balance editing.
- No network protocol or authoritative simulation.
- No automatic saving or arbitrary path export.
- No promise of cross-GPU pixel identity for particles.
- No browser control plane unless later evidence warrants a separate ticket.
- No expansion of BRL-0094 beyond the read-only phase without updating that ticket's scope first.
