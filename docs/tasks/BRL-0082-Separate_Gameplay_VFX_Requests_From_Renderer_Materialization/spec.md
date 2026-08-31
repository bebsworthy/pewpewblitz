# Technical specification

## Scope and outcome

Separate client gameplay-fact interpretation from Bevy 3D VFX resolution. Combat, map-object/pickup, and Heist presentation producers convert their owned cues into bounded stable-key `VfxRequest` messages. The renderer consumes requests, resolves the client-only authored VFX catalog, selects renderer/mesh/material/scale/anchor/lifetime policy, and performs the existing bounded allocation transaction.

This phase is organization and extensibility work only. It does not change authoritative gameplay, wire types, gameplay fingerprints, cue payloads, normal/reduced visual values, effect timing, renderer availability, or the accepted 3D presentation.

## Current problem

`src/client/presentation_3d/combat/effects.rs` currently combines four responsibilities: domain cue validation, cue-to-family exhaustive dispatch, authored catalog resolution, and concrete mesh/material/entity allocation. `VfxCueFamily` plus `ALL_VFX_CUE_FAMILIES` form a closed Rust inventory duplicated by `assets/catalogs/vfx.ron`. Adding a feature-specific VFX family requires editing the shared renderer enum, catalog completeness list, cue coordinator, and authored mapping even when the existing renderer/profile primitives are sufficient.

## Architecture

### Stable request contract

- Define a client-only `VfxRequest` Bevy message with a validated bounded stable request key, planar position, optional authoritative radius, optional authoritative activation/expiry deadline, and a diagnostic label or equivalent bounded static identity.
- Stable keys are semantic presentation identities such as `combat.muzzle`, `combat.impact`, `ability.reveal-scan`, `world-object.explosion`, `pickup.collected`, and `heist.critical`; they are not wire IDs and do not enter the gameplay-content fingerprint.
- The request carries no mesh handle, material handle, renderer family, reduced-effects choice, resolved duration, or catalog profile. Producers cannot access `Primitive3dAssets`, `Material3dAssets`, or `VfxCatalog`.
- Request construction rejects empty/oversized keys, non-finite positions/radii, nonpositive radii, and invalid deadline order. Existing trusted cue paths continue to produce exactly one request or no request according to current behavior.

### Feature-owned producers

- Split cue translation into focused producer plugins/systems for combat/ability cues, map-object and pickup cues, and Heist objective cues.
- Producers retain their current domain validity gates: deduplicated combat input, world-object/pickup generation matching, Heist readiness/match/target membership, supported milliunit conversion, and exact cue omissions.
- Attack animation state (`V3FighterVisual::shoot_seconds`) is renderer-owned feedback and must be separated from semantic VFX request production; it may remain a focused renderer system reading the same deduplicated cue stream.
- Producer plugins write requests in deterministic cue order. Adding another feature VFX producer requires a new plugin/system plus authored request mapping, not a change to request materialization.

### Authored catalog

- Replace the closed `VfxCueFamily` mapping with bounded stable request-key mappings in `assets/catalogs/vfx.ron`; bump only the client VFX catalog schema.
- Preserve every existing profile, normal/reduced mapping, renderer family, material key, scale/anchor policy, lifetime, fallback, and concurrency cap exactly.
- Validate bounded unique request keys, referenced profiles, profile/fallback/reduced references, default fallback safety, supported renderer/anchor combinations, finite bounds, and total catalog capacity.
- Resolve by request key. Unknown request keys and request/profile capability mismatches fail closed without panicking the presentation schedule; record bounded diagnostics if an existing facility is available, otherwise skip deterministically and cover the behavior in tests.
- Synthetic catalog/request tests prove that another stable request key using existing rendering primitives resolves and materializes without editing renderer dispatch.

### Renderer transaction

- The renderer reads `VfxRequest`, applies the current reduced-effects setting, resolves the request mapping/profile and deadline fallback, computes transform/lifetime, translates renderer/material keys to concrete handles, and emits/materializes the existing bounded pending-effect transaction.
- Preserve global `MAX_EFFECTS`, per-profile concurrency eviction, FIFO/order sequencing, authoritative deadline cleanup, reduced-profile selection, and deterministic oldest eviction.
- Keep the exhaustive renderer/material matches local to the renderer because adding a genuinely new rendering primitive or material family is an intentional typed renderer change. New semantic VFX using existing primitives must remain data plus producer-plugin work.

### Organization and scheduling

- Convert `combat/effects.rs` into responsibility-based modules or equivalent focused files: request contract/producers, catalog/profile resolution, renderer/materialization, and tests. Do not split solely by line count.
- Keep the six `WorldPresentationSet` phases and the `ConsumeCues` ordering visible in `WorldPresentationPlugin` composition.
- Within `ConsumeCues`, order feature producers before renderer request resolution/materialization. Preserve the deferred-command boundary and effect order.
- Do not install VFX production or rendering in the headless client/server graphs.

## Preserved invariants

- Server authority, protocol registration, routed enums, stable gameplay IDs, cue schemas, and gameplay-content fingerprint are untouched.
- Existing 15 semantic effects resolve to byte-equivalent authored profile values and the same concrete renderer/material choices.
- Reveal Scan authoritative lifetime, world explosion authoritative radius, elemental/demolition radius scaling, pickup generation filtering, Heist readiness/identity filtering, and reduced-effects behavior remain exact.
- Combat attack animation remains present and is not made authoritative.
- Bounded effect allocation, profile caps, cleanup, and cross-producer ordering remain deterministic.

## Acceptance criteria

1. Domain cue producers emit validated stable-key `VfxRequest` messages without importing meshes, materials, renderer families, or `VfxCatalog`.
2. `VfxCueFamily` and its closed completeness array are removed; the authored catalog maps bounded stable request keys to profiles.
3. Existing combat/ability, world-object, pickup, and Heist cue cases produce the same request profile semantics and retain exact filtering/precedence.
4. Renderer request resolution preserves every checked-in normal/reduced profile value, material, renderer, geometry, lifetime, fallback, and cap.
5. Invalid/unknown requests and incompatible radius/deadline shapes fail closed deterministically without crashing an update.
6. A synthetic stable request key and profile mapping materialize through existing renderer primitives without renderer dispatch changes.
7. Producer-before-materializer schedule order and cross-producer FIFO behavior are explicit and tested.
8. Headless client and server feature graphs remain free of rendering assets/systems.
9. No gameplay, protocol, routing, content-fingerprint, or authored balance value changes.
10. Durable presentation documentation describes semantic request keys versus concrete renderer extension.
11. Focused VFX/catalog/presentation tests, client/server checks, `just check`, `just lint`, and `just test` pass. Native evidence is required only if output differs; unchanged output is established through exact profile/transform/lifetime characterization.

## Implementation plan

1. Characterize current cue-to-profile mappings, filtering, reduced values, geometry, lifetimes, order, and allocation.
2. Add the bounded request type and stable built-in keys.
3. Replace enum mappings in the VFX RON/catalog with stable request-key mappings and preserve exact profiles.
4. Extract focused combat, map/pickup, and Heist request producer plugins/systems.
5. Make renderer materialization consume requests and own all concrete asset/profile resolution.
6. Separate attack animation feedback from VFX request production.
7. Add synthetic extension, invalid request, mapping parity, producer gating, scheduling, and allocation tests.
8. Update durable presentation architecture documentation and run proportional plus canonical verification.

## Scope exclusions

- Audio request architecture; track as a separate BRL-0070 child after this phase.
- New visual effect, renderer primitive, material, animation, or balance/presentation tuning.
- Server or shared gameplay changes.
- Wire-schema, routed-mode, or global compatibility changes.
- Runtime asset hot reload, dynamic executable plugins, trait-object renderer framework, or general event bus.

## Registration lifecycle refinement

Implementation must include a bounded client-only `VfxRegistry`, not rely on runtime string lookup alone. `VfxRegistryPlugin` creates a private builder during `Plugin::build`; feature VFX registration plugins synchronously contribute validated static request keys plus capability metadata (authoritative radius supplied and authoritative deadline supplied); finalization removes the builder, validates exact built-in/catalog coverage and request/profile compatibility, sorts deterministically, and inserts an immutable registry. Reject registration before the registry plugin, after sealing, duplicates, capacity overflow, missing built-in handlers/mappings, extra built-in mappings without a registration, and incompatible radius/deadline profiles. Do not lazily recreate a builder.

Producer order is part of output under the global effect cap. Preserve current combat → world object → pickup → Heist ordering through stable registered producer ranks and deterministic request ordering, or through an equivalently explicit chained schedule that defines additive-plugin ordering. A synthetic producer registration must participate without editing renderer dispatch.

Because this phase rewires player-visible presentation even though intended output is identical, native evidence is required. Exercise normal and reduced effects for combat/ability VFX plus world-object explosion, pickup, and Heist objective feedback; record observations and any corrections before closeout.

## Implemented result

- Added client-only `VfxRequestKey`, validated `VfxRequest`, stable `VfxRequestOrder`, authoritative `VfxDeadline`, and bounded diagnostic labels.
- Added `VfxRegistryPlugin` with a build-only builder, capacity 32, duplicate/key/rank validation, exact producer/catalog coverage, transitive radius/deadline capability validation including runtime-reachable deadline fallbacks, deterministic sealing, and immutable runtime lookup.
- Migrated `assets/catalogs/vfx.ron` from schema 2 enum families to schema 3 stable request-key mappings. All 15 mappings and every existing profile value remain unchanged.
- Added focused combat, world-object, pickup, and Heist producer plugins. Producers retain deduplication, generation, readiness, match, safe-membership, and milliunit gates while importing no meshes, materials, renderer families, reduced-effects settings, or catalog types.
- Nested `VfxRequestSet` inside `WorldPresentationSet::ConsumeCues`; renderer resolution explicitly runs after producers. Requests are sorted by producer rank, event ID, and key before resolution, preserving combat → world-object → pickup → Heist precedence and per-producer FIFO.
- The generic 3D adapter alone resolves profiles, reduced effects, deadlines, transforms, mesh/material handles, allocation caps, eviction, and cleanup.
- Split fighter shot animation feedback into its own cue-reading renderer system.
- Removed `VfxCueFamily`, its closed completeness inventory, and the old presentation-local VFX catalog module.
- Added a default-off `BRAWLER_RENDER_COMBAT=1` option to the canonical render-evidence script so native evidence can drive aim, fire, and ultimate input instead of only loading/moving through a match.
- Updated `docs/11-art-and-presentation-direction.md` with the stable request/registry/renderer extension contract.

## Acceptance evidence

Focused characterization and extension evidence:

- `cargo test --locked --no-default-features --features client --lib client::vfx::`: 18 passed before the final producer-gate additions; final canonical client suite includes all 24 VFX tests.
- Renderer effects tests: 11 passed, including exact transform/lifetime behavior, synthetic stable-key resolution through existing sphere/impact renderer materialization into `CombatEffect3d`, invalid fail-closed behavior, rank/event ordering, allocation, profile caps, and cleanup.
- World-object and pickup tests reject stale map generations and map current cues to the expected request keys.
- Heist tests reject non-ready, wrong-match, and absent-exact-safe cases and accept the exact current safe.
- Catalog/registry tests cover schema and numeric bounds, all 15 mappings, duplicate/missing/extra/capacity/lifecycle rejection, incompatible direct and fallback capabilities, synthetic registration, seal ordering, and runtime fail-closed resolution.
- `bash -n scripts/v3-render-evidence.sh` and `git diff --check`: passed.
- `just check`: passed for routing, client, server, network-test, Balance Lab web, and Balance Lab Rust graphs.
- `just lint`: passed on the final tree, including all Clippy targets, server feature isolation, sole-world-renderer boundary, and map cleanup.
- Final `just test`: passed. Routing 83 plus binary/process/isolation suites; client 523; server 482; Balance Lab 504; combined Balance Lab network 1; network 97; performance 12.
- Independent architecture review found no runtime behavior drift, schedule flaw, or Bevy message-lifecycle defect after corrections. Its requested AC6 synthetic materialization, AC7 request-boundary ordering, producer gate tests, and transitive fallback validation were implemented and reverified.

Native final-binary evidence:

- `target/brl-0082-final-normal-combat.txt`: routed Wipeout 1v1 Practice, normal effects, combat driving enabled; result pass, 1,801 samples, frame p95 17.157 ms, `effect_high_water=3`, `effect_terminal=0`, no client error/panic.
- `target/brl-0082-final-reduced-heist-combat.txt`: routed Heist 1v1 Practice, reduced effects, combat driving enabled; result pass, 1,801 samples, frame p95 17.082 ms, `effect_high_water=3`, `effect_terminal=0`, no client error/panic.
- Exact catalog, producer, geometry, lifetime, generation, and Heist gate tests cover the semantic source of combat/ability, world-object explosion, pickup, and objective feedback; the native runs confirm both normal/reduced materialization and teardown in their routed maps.

No gameplay, protocol, routing, content-fingerprint, server-authority, or authored balance value changed.

## Feedback and corrections

- Initial native Heist evidence loaded and passed but recorded no transient effects because the canonical render harness did not press fire. This was not accepted as VFX evidence. The default-off combat-driving option was added, and both final native runs recorded active transient effects.
- An exploratory Heist 3v3 render attempt ended with a worker-exit mismatch, and a shortened run failed only the harness's locked sample-count threshold. Neither reproduced in the final Heist 1v1 evidence, the 97-test network matrix, or the canonical suites; they did not require product-code changes.
- Independent review identified missing evidence rather than production drift. All four requested gaps were corrected before closeout.

## Learn-from-errors review

- Mistake: producer systems were initially ordered only before the resolver, not explicitly inside the established ConsumeCues phase. Cause: treating the local dependency edge as sufficient without rechecking the enclosing schedule contract. Prevention: nested the producer set in `WorldPresentationSet::ConsumeCues` and retained the explicit resolver-after-producer edge.
- Mistake: early tests proved registry resolution and pending-effect allocation separately but did not cross the new semantic boundary with a synthetic key. Cause: reusing legacy allocation tests after changing the extension seam. Prevention: every new extension boundary now needs one synthetic end-to-end test through the production resolver/materializer.
- Mistake: the first native evidence command exercised map loading and movement but not player attacks. Cause: assuming Practice bots guaranteed visible effects during the measurement window. Prevention: evidence harnesses for presentation work need an explicit default-off action driver and must assert a nonzero relevant high-water signal.
- Reusable lesson: capability validation must follow the same fallback reachability as runtime resolution. Direct-profile validation alone can defer an authored mismatch to runtime fail-closed behavior.
