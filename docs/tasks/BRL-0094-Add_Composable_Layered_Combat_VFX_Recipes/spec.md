# Technical specification

## Status of this specification

This is the initial implementation specification for BRL-0094. The ticket remains `todo`; implementation does not begin until the phase-0 source audit has revalidated the facts and frozen any required protocol decision in this specification.

The work is deliberately phased. Each phase must end with a player-visible or architecture-verifying outcome, focused evidence, and an explicit gate before the next phase. A failed optional particle-backend gate does not invalidate the mesh-layer recipe foundation.

## Outcome

Deliver a client-only, bounded, validated, composable combat-presentation recipe system that:

- produces distinctive presentation for player-composed weapons from reusable carrier, phase, accent, and relationship policies;
- reuses the same elemental visual language across weapon payloads and matching ultimates/areas;
- supports transient bursts, owner-attached projectile/deployable views, and replicated persistent fields without confusing their lifecycles;
- preserves exact authoritative geometry, timing, source attribution, concealment privacy, late join, recovery, restart, and teardown;
- preserves deterministic request ordering and bounded CPU/GPU/entity/asset allocation;
- provides mandatory mesh-based counterplay layers and optional particle decoration;
- keeps all concrete assets, materials, particles, render heights, timelines, and degradation policy client-only;
- ends with native, routed, maximum-overlap readability and performance evidence.

## Scope

### Included

- Schema evolution of the client VFX catalog from single-profile request mappings to bounded layered recipes.
- Reusable typed mesh-layer definitions and bounded timeline animation.
- A client-only presentation signature and deterministic composition/resolution policy.
- Style-aware transient VFX requests using the existing sealed registry and producer ordering.
- Reuse of the recipe/layer materializer by selected attached and persistent presentation roots.
- Distinctive Cold and Fire weapon/field vertical slices.
- A gated Hanabi 0.19 macOS/Metal feasibility spike and particle adapter if the gate passes.
- A lobbed/explosive vertical slice with body, trail, planar shadow, flash, shockwave, fragments/sparks, and smoke/debris where supported.
- Reduced-effects and primitive-degradation behavior.
- Exact bounds, capacity, fallback, lifecycle, and catalog validation.
- Focused automated, routed, native, performance, documentation, feedback, and learning evidence.
- A small held-weapon presentation integration seam and a decision on whether one demonstrative held-weapon variation belongs in this ticket.

### Excluded unless this specification is explicitly amended before implementation

- New authoritative weapon delivery, payload, status, or ultimate mechanics.
- Hitscan gameplay or client-side projectile simulation.
- Gameplay balance changes.
- Player-authored arbitrary shader, particle, or effect graphs.
- Runtime executable plugins or dynamic renderer discovery.
- Renderer handles, asset paths, material IDs, particle IDs, or VFX recipe IDs in gameplay definitions or the network protocol.
- A second gameplay-world renderer.
- Bloom as a required readability mechanism.
- General camera-shake implementation; only an integration boundary may be identified.
- Full replacement of every existing world, UI, map, fighter, and objective visual in one ticket.
- General skin/entitlement systems.
- Hot reloading or an external visual editor.
- Broad held-weapon art production beyond the scoped integration seam or approved demonstration.

## Existing architecture to preserve

1. `src/client/vfx/request.rs` owns validated process-local semantic request identities, geometry/timing inputs, and stable ordering.
2. `src/client/vfx/registry.rs` owns build-time request registration, exact producer/catalog coverage, capability validation, deterministic sealing, and immutable runtime lookup.
3. Feature producer plugins own translation from their gameplay facts and do not import concrete renderer assets.
4. `assets/catalogs/vfx.ron` is client-only and does not contribute to gameplay-content compatibility.
5. `src/client/presentation_3d/combat/effects.rs` owns concrete resolution, materialization, caps, eviction, cleanup, and authoritative deadline observation.
6. Durable combat presentation is grouped by lifecycle in `src/client/presentation_3d/combat/`.
7. Projectile bodies follow replicated `Position`, `Rotation`, `StraightFlight`, `ProjectileBody`, and `LobbedFlight`; presentation does not integrate gameplay movement.
8. Persistent elemental, Splash, concealment, cone, sticky, status, and deployable visuals reconcile from replicated state.
9. The server feature graph excludes rendering, assets, input, audio, Hanabi, and client presentation.
10. `WorldPresentationPlugin` remains the sole gameplay-world renderer and keeps visible schedule ordering/deferred boundaries.

## Architectural decisions

### A. Recipes compose layers; layers do not execute gameplay

The catalog resolves a semantic request/signature to one `VfxRecipe`. A recipe contains a bounded ordered list of reusable layer references or inline bounded layer descriptors. A layer describes presentation only. It cannot query or mutate authoritative health, collision, status, score, targeting, map state, or match rules.

### B. Share vocabulary, not lifecycle ownership

Transient bursts, attached views, and persistent state views share:

- recipe IDs;
- layer definitions;
- renderer adapters;
- material and particle asset keys;
- timeline/easing definitions;
- readability classification;
- fallback and reduced policy;
- capacity accounting.

They do not share one universal spawn mechanism:

- transient effects originate from ordered `VfxRequest` messages;
- attached views originate from owner reconciliation and follow replicated pose/lifecycle;
- persistent views originate from replicated state and authoritative geometry/deadlines.

### C. Semantic style context does not become a key cross-product

Stable request keys continue to name semantic phases such as `combat.muzzle`, `combat.impact`, or `ability.elemental-field.activation`. Style context carries bounded typed information. Do not create separate registered keys for every carrier × element × team × phase combination.

### D. Exact counterplay geometry is a mandatory mesh layer

A field, explosion warning, sticky radius, cone, or other gameplay area keeps one exact readability-critical mesh layer based on authoritative geometry. Decorative layers may be approximate, stylized, omitted, or degraded. No particle system replaces the mandatory telegraph.

### E. Presentation composition has bounded deterministic precedence

The initial resolver uses this order:

1. explicit exact ultimate/content override;
2. explicit exact weapon-preset override;
3. carrier default plus selected accent layers;
4. carrier-only default;
5. safe fallback recipe.

Accent selection is deterministic. The initial maximum is two distinct accents per signature. Duplicate semantic accents collapse. Stable precedence is:

1. Restoration;
2. Cold;
3. Poison;
4. Fire;
5. Demolition;
6. neutral damage/knockback/slow accent.

This is presentation precedence only; it does not change payload order or gameplay meaning. Phase 0 must validate whether this ordering produces misleading combinations such as damage plus healing. If so, author an explicit recipient-aware split rather than silently recoloring one effect.

### F. Relationship and elemental identity are orthogonal

The observer relationship is resolved client-side as local, ally, enemy, or neutral. A relationship channel uses a secondary rim, outline, band, marker, or other dedicated layer. It must not replace the element channel. Existing concealment and observer filtering determine whether the source/effect is visible before style resolution.

### G. Particle simulation is decorative

GPU particle randomness need not be deterministic across clients because it does not enter gameplay or evidence convergence. Request acceptance, recipe choice, CPU-side ordering, instance caps, fallback, reduced selection, spawn count/rate bounds, and cleanup remain deterministic.

## Proposed data model

Names are provisional; Phase 0 may refine them while preserving responsibility.

### Presentation signature

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct CombatVisualSignature {
    carrier: VisualCarrier,
    phase: VisualPhase,
    accents: [Option<VisualAccent>; 2],
    relationship: VisualRelationship,
    override_key: Option<VisualOverrideKey>,
}
```

Required bounded enums:

- `VisualCarrier`: Straight, Lobbed, Sticky, Melee, Cone, SplashCircle, SplashRectangle, ElementalField, ConcealmentField, Deployable, Environment.
- `VisualPhase`: Muzzle, Body, Trail, Shadow, Impact, Activation, ActiveArea, Pulse, Expiration, Status.
- `VisualAccent`: Neutral, Cold, Fire, Poison, Restoration, Demolition, Concealment.
- `VisualRelationship`: Local, Ally, Enemy, Neutral.

This signature is client-local and may be computed from:

- `ResolvedMatchLoadout.primary_weapon.recipe`;
- weapon preset identity when present;
- payload effect kinds and recipient policies;
- `ResolvedUltimate` identity/kind;
- `ElementalFieldState.kind`;
- `PersistentSplashState.effects`;
- `StickyBlobState.kind`;
- team/controlled-team relationship;
- the cue's semantic source and geometry.

Do not serialize this type into the gameplay protocol in the first implementation.

### Request geometry

Replace or extend the current position/radius option set with one validated bounded geometry shape if required by the vertical slices:

```rust
enum VfxGeometry {
    Point {
        position: Vec2,
        normal: Option<Vec2>,
        direction: Option<Vec2>,
    },
    Circle {
        center: Vec2,
        radius: f32,
    },
    Rectangle {
        center: Vec2,
        facing: f32,
        half_extents: [f32; 2],
    },
    Segment {
        start: Vec2,
        end: Vec2,
        width: f32,
    },
}
```

All values must be finite and bounded by existing map/engine limits. Direction and normal vectors must be normalized or safely normalizable. Capability registration must validate geometry compatibility transitively through normal, reduced, and fallback recipes.

If the first vertical slices do not need a shape, retain the narrower current payload. Do not broaden the request contract speculatively.

### Catalog source

The next catalog schema should separate mappings, recipes, reusable layers, and particle assets only as demonstrated:

```text
VfxCatalogSource
  schema_version
  default_recipe
  mappings[]
  recipes[]
  mesh_layers[]
  particle_layers[]   # only after Hanabi gate
```

A mapping connects a registered semantic request key and optional typed selector to a recipe. The exact selector representation must avoid stringly typed cross-products and exhaustive central code edits for new content using supported primitives.

### Recipe

```rust
struct VfxRecipeSource {
    id: String,
    layers: Vec<String>,
    concurrency_cap: usize,
    fallback_recipe: String,
    reduced_recipe: Option<String>,
}
```

Initial bounds to validate in Phase 0 and then freeze:

- maximum request mappings: at least current 15 plus planned vertical slices; proposed 64;
- maximum recipes: proposed 128;
- maximum reusable layers: proposed 256;
- maximum layers per recipe: proposed 8;
- maximum total active recipe roots: preserve or explicitly replace current 96;
- maximum active decorative particle emitters: measured in the Hanabi gate;
- maximum catalog string/key lengths: preserve current bounded conventions;
- maximum transient lifetime: preserve 10 seconds unless a demonstrated recipe requires less/more;
- no recursive fallback or reduced-reference cycle;
- bounded total expanded layers after fallback/reduced resolution.

Bounds are engine safety limits, not player-authored balance.

### Layer

```rust
enum VfxLayerSource {
    Mesh(MeshLayerSource),
    Particle(ParticleLayerSource),
    Trail(TrailLayerSource),
}
```

A mesh layer includes:

- typed mesh key: Sphere, Disc, Ring, Capsule/Projectile, Cuboid/Segment, or another implemented primitive;
- typed material key;
- anchor;
- scale/geometry policy;
- timeline;
- readability class;
- shadow flags;
- optional orientation policy.

A particle layer includes:

- stable client-only particle asset key;
- burst or rate policy;
- capacity/spawn count;
- simulation-space policy;
- position/velocity/lifetime/size bounds or asset-owned equivalents;
- optional validated recipe properties;
- readability class, always Decorative in the first implementation;
- reduced and fallback behavior.

A trail layer includes only the minimum demonstrated representation. Prefer a bounded mesh segment or sampled ribbon owned by the attached lifecycle. Do not create a general trail engine before both straight-projectile and lobbed/explosive content demonstrate shared needs.

### Timeline

Use a bounded typed timeline such as:

```rust
struct VfxTimeline {
    delay_millis: u16,
    duration: VfxDuration,
    easing: VfxEasing,
    scale: VfxKeyframes<Vec3>,
    alpha: Option<VfxKeyframes<f32>>,
    translation: Option<VfxKeyframes<Vec3>>,
    rotation_radians: Option<VfxKeyframes<f32>>,
}

struct VfxKeyframes<T> {
    start: T,
    peak_at_basis_points: u16,
    peak: T,
    end: T,
}
```

Requirements:

- finite validated values;
- peak fraction in 0..=10,000;
- duration fixed or authoritative deadline;
- no wall-clock sleep;
- authoritative-deadline views derive remaining fraction from replicated ticks where appropriate;
- fixed transient animation uses `Time<Real>`;
- alpha implementation cannot clone unbounded `StandardMaterial` assets per frame;
- cleanup occurs even if an animation renderer fails closed.

The final type may omit unsupported properties in early phases. Do not ship authored fields that no renderer consumes.

## Module ownership

Proposed organization after implementation:

```text
src/client/vfx/
  mod.rs                 semantic public composition surface
  request.rs             validated request, order, geometry, timing
  signature.rs           client semantic style derivation and precedence
  catalog.rs             source schema, validation, immutable lookup
  registry.rs            producer registration and capability sealing
  recipes.rs             recipe expansion/resolution
  producers/
    combat.rs
    world_objects.rs
    pickups.rs
    heist.rs

src/client/presentation_3d/combat/
  mod.rs                 visible plugin/schedule composition
  effects/
    mod.rs               adapter composition
    transient.rs         ordered transient allocation transaction
    mesh.rs              mesh-layer materialization/timeline
    particles.rs         Hanabi adapter only if gate passes
    timeline.rs          focused animation/cleanup
  entities.rs or focused existing lifecycle modules
                         attached/persistent roots consuming recipes
```

Exact file splitting follows responsibility, not this sketch mechanically. Keep schedule order visible at the composition point. Do not hide `WorldPresentationSet`, `VfxRequestSet`, deferred boundaries, or cleanup ordering behind a general framework.

## Source identity and protocol decision gate

Phase 0 must answer with focused tests:

1. Can a transient cue always resolve the exact source `ResolvedMatchLoadout` when processed?
2. Can a projectile or impact outlive source-fighter removal?
3. Can Balance Lab or another supported flow replace a loadout while old projectiles/effects remain active?
4. Does late join provide enough information to reconstruct the spawn-time visual signature?
5. Is `WeaponDefinitionId` ambiguous for custom recipes in any accepted path?

Preferred outcomes, in order:

- client-only immutable signature snapshot at visual-root/request creation from existing replicated facts;
- bounded cache keyed by existing recipe fingerprint learned from replicated loadouts;
- only if neither is sufficient, add semantic `WeaponRecipeFingerprint` to the smallest relevant shared source/cue facts and perform the normal global compatibility migration.

A protocol change cannot be introduced incidentally after Phase 1 starts. Record the decision and its evidence in this spec first.

# Delivery phases

## Phase 0 — Characterization and implementation-ready design

### Outcome

Freeze current behavior and the smallest schema/type changes before altering presentation.

### Work

- Re-read BRL-0082 implementation and exact current VFX catalog/registry tests.
- Characterize every current request mapping, profile, geometry, transform, lifetime, fallback, reduced variant, capacity, and cleanup rule.
- Inventory transient, attached, and persistent combat presentation paths.
- Inventory exact semantic facts available for weapon, ultimate, field, sticky, deployable, and world-object style resolution.
- Resolve the source identity/protocol gate above.
- Freeze catalog and runtime bounds.
- Decide how per-instance alpha can be implemented without unbounded material asset creation.
- Identify the minimum mesh/trail primitives required by Cold/Fire and lobbed/explosive vertical slices.
- Define exact normal/reduced/primitive behavior for readability-critical versus decorative layers.
- Record baseline native screenshots/video/evidence for representative current effects.
- Amend this spec if findings require a material scope or protocol decision.

### Gate

- No unresolved architecture question blocks Phase 1.
- Existing behavior is covered by characterization tests or recorded baseline evidence.
- Any protocol migration is explicitly approved by the existing user direction or raised as a Ticket question.
- Ticket moves to `doing` only when Phase 1 implementation begins.

## Phase 1 — Layered mesh-recipe foundation with behavior parity

### Outcome

The existing 15 semantic requests resolve through bounded recipes/layers while producing acceptably equivalent output. No new gameplay or required particle dependency.

### Work

- Bump the client VFX catalog schema.
- Add validated recipe and mesh-layer definitions.
- Migrate current single profiles to one-layer recipes with byte-/value-equivalent normal and reduced policy.
- Preserve stable producer keys, ranks, event order, registry sealing, fallbacks, authoritative deadline semantics, total cap, per-profile/recipe cap, eviction, and cleanup.
- Add a generic bounded timeline component/system for the subset used by current recipes.
- Keep existing sphere/ring materialization available through mesh-layer adapters.
- Add synthetic multi-layer request materialization without changing renderer dispatch for the semantic key.
- Add cycle, expansion-capacity, invalid timeline, missing layer, fallback, reduced, and capability tests.
- Preserve client/server feature isolation.

### Gate

- Existing visual mappings and transforms are characterized as equivalent.
- A synthetic two- or three-layer recipe materializes in authored order and cleans up.
- Canonical client/server checks pass.
- No player-visible regression in native normal/reduced/primitive evidence.

## Phase 2 — Presentation signature and lifecycle adapter reuse

### Outcome

Transient producers and selected durable roots resolve the same semantic style vocabulary without merging lifecycle ownership.

### Work

- Implement `CombatVisualSignature` or the smaller validated equivalent.
- Derive carrier/accent/relationship/phase from replicated loadout, cues, and persistent state.
- Implement deterministic accent collapse and override/default precedence.
- Extend request geometry/style only as needed.
- Snapshot resolved signatures on attached roots.
- Route elemental-field and persistent-Splash material selection through recipe resolution while retaining state reconciliation.
- Route straight/lobbed projectile body selection through a recipe-capable attached materializer while preserving body/collider footprint tests and pose ownership.
- Preserve exact field boundary geometry and authoritative deadlines.
- Keep concealment privacy and observer relationship evaluation upstream of materialization.

### Gate

- A custom composed weapon using an existing delivery and an elemental part resolves a distinct client presentation signature without a central match edit.
- An elemental field and weapon accent reuse at least one shared layer/style definition.
- Late join/recovery reconstructs persistent views correctly.
- Loadout replacement/in-flight behavior matches the Phase-0 identity decision.
- No presentation component becomes authoritative or replicated solely for rendering convenience.

## Phase 3 — Cold and Fire player-visible vertical slice

### Outcome

Cold and Fire are immediately distinguishable across weapon fire, projectile/trail or delivery body, impact/status feedback, field activation, active field, pulse, and expiration.

### Work

- Author shared Cold and Fire accent families.
- Add style-aware muzzle and impact recipes.
- Add Cold/Fire projectile body or trail treatment where the delivery supports it.
- Branch elemental-field activation by `ElementalFieldKind` rather than one generic activation profile.
- Build exact field fill/boundary plus mesh-only decorative activation/pulse/expiry layers.
- Preserve team relationship through a secondary channel.
- Add reduced and primitive variants retaining mandatory geometry.
- Add focused tests showing a Cryogenic Module and Incendiary Module compose with at least two existing carrier families without per-weapon renderer code.
- Add native 1v1 and dense 3v3 comparison evidence.

### Gate

- Cold and Fire remain distinguishable without bloom and when seen together.
- Reduced effects retain exact field boundaries and sufficient weapon-impact identity.
- No hue-only dependency is identified in playtest.
- The same accent vocabulary is reused by weapons and ultimate fields.
- User feedback on the first presentation pass is recorded and corrections are reverified.

## Phase 4 — Hanabi 0.19 macOS/Metal spike and particle adapter

### Outcome

Decide from evidence whether Hanabi becomes the particle renderer for this ticket.

### Spike configuration

- `bevy_hanabi = 0.19`, optional/client-only.
- Default features disabled; enable only `3d`.
- No server, network-test headless-only, or Balance Lab web dependency leakage.
- Mark exactly the correct gameplay camera if the selected Hanabi API requires it.
- Use named bounded effect assets, not runtime-authored arbitrary expressions from content.
- Implement one Cold shard/snow effect and one Fire flame/ember effect.
- Exercise burst and persistent-rate lifecycles.
- Provide mesh-only readability fallback.

### Evidence

- Clean startup and teardown on macOS/Metal.
- Normal and reduced modes.
- Forced primitive-world mode.
- Routed match and return-to-lobby cleanup.
- Maximum expected 3v3 overlapping Cold/Fire fields and representative weapon fire.
- Frame percentiles and entity/effect/asset high-water/terminal signals.
- Build-size/compile impact noted.
- No panic, pink/fallback render, leaked emitter, unbounded asset growth, or server feature leakage.

### Gate

If accepted:

- integrate `ParticleLayer` as a typed renderer adapter;
- validate particle asset references and capacities;
- author reduced counts/rates and mesh fallbacks;
- retain deterministic CPU-side recipe/request order.

If rejected:

- remove or do not merge the dependency;
- record the failure and selected mesh/custom alternative;
- continue to Phase 5 with the recipe architecture intact;
- create a separate backlog ticket only if a materially different particle solution is warranted.

## Phase 5 — Lobbed/explosive vertical slice

### Outcome

One existing lobbed or sticky/explosive family demonstrates layered attached flight and sequenced impact without changing authoritative delivery.

### Recommended demonstrator

Use Arc Launcher for lobbed body/shadow and Sticky Blomb or Big Blob for armed/explosive treatment, selecting the smallest combination that proves:

- replicated visual arc;
- planar ground shadow;
- optional short trail;
- activation/impact flash;
- exact-radius shockwave or warning boundary;
- fragments/sparks;
- delayed smoke/debris when the accepted particle backend supports it.

### Work

- Add a ground-shadow layer anchored to authoritative planar position while the projectile root follows `LobbedFlight` height.
- Keep the projectile body aligned with existing replicated pose and body geometry.
- Add ordered explosion layers with authored delay/duration.
- Use hit normal/direction only where exact cue facts supply them.
- Preserve sticky fuse and blast-radius counterplay.
- Keep smoke/debris decorative and reduced/primitive-droppable.
- Keep camera feedback outside this recipe transaction.

### Gate

- Shadow, body, and impact timing remain visually coherent with authoritative landing.
- Shockwave/warning geometry matches authoritative radius.
- Late join and owner removal do not orphan attached roots.
- Maximum active explosive content remains within measured caps.
- Native playtest accepts trajectory and impact readability.

## Phase 6 — Breadth, held-weapon seam, documentation, and closeout

### Outcome

The framework is proven reusable beyond the two vertical slices and has a clear bounded path for held-weapon identity.

### Work

- Extend the recipe vocabulary only to demonstrated remaining weapon/ultimate/area families with the highest readability value.
- Add Poison and Restoration accents using distinct motion/silhouette.
- Evaluate sentry, cone, persistent Splash, demolition, reveal, concealment, pickups, world objects, and Heist feedback for migration or explicit deferral.
- Define `WeaponPresentationProfile` integration ownership for held model/primitive, attachment transform, muzzle socket, material family, animation compatibility, and fallback.
- Implement at most one demonstrative held-weapon variation if Phase 0/3 evidence and user feedback keep it in scope; otherwise create a linked backlog ticket with the seam documented.
- Reconcile `docs/11-art-and-presentation-direction.md`, relevant weapon/ability/UX documentation, asset provenance, reduced-effects behavior, and operator/evidence commands.
- Dispose every playtest/review item as implemented, deferred, rejected with rationale, or awaiting evidence.
- Perform the learn-from-errors review and create/improve a skill only if a recurring workflow warrants it.
- Run `ticket sync`; stop on conflict.
- Move the ticket to `done` only when every required gate and acceptance criterion is satisfied.

# Acceptance criteria

1. The client VFX catalog supports validated bounded recipes containing multiple ordered reusable layers.
2. Existing built-in transient VFX migrate without losing stable request registration, ordering, fallbacks, reduced policy, authoritative deadlines, caps, eviction, cleanup, or primitive fallback.
3. New content using supported carrier/accent/layer primitives can resolve without adding a central renderer or cue-family match arm.
4. Player-composed weapons derive presentation from their semantic resolved recipe, including delivery and payload accents, without embedding render data in gameplay definitions.
5. Source identity remains correct for late join, source removal, live loadout replacement, and in-flight projectiles; any required protocol migration is minimal, semantic, globally versioned, and tested.
6. Transient, attached, and persistent effects share recipe/layer vocabulary while retaining their correct lifecycle owners.
7. Exact gameplay geometry and authoritative deadlines drive every mandatory area, cone, warning, field, sticky, projectile-body, and shockwave/telegraph layer.
8. Cold and Fire share reusable weapon/ultimate accent definitions and are distinguishable by motion/silhouette as well as color.
9. Relationship/team readability remains visible without overwriting elemental identity.
10. Reduced-effects and primitive-degradation modes preserve every readability-critical layer and may remove only decorative content.
11. Mesh timeline animation is bounded, validated, cleanup-safe, and does not create unbounded material assets or entities.
12. Particle layers, if accepted, remain client-only, decorative, bounded, catalog-validated, reduced-capable, and backed by mandatory mesh fallbacks.
13. The Hanabi gate records macOS/Metal correctness, teardown, maximum-overlap performance, dependency impact, and an explicit accept/reject decision.
14. A lobbed/explosive vertical slice demonstrates authoritative body/arc alignment, planar shadow, sequenced impact, exact warning/shockwave geometry, and cleanup.
15. Concealment privacy, observer filtering, server authority, protocol identity, gameplay fingerprints, collision, damage, status, score, and match behavior remain unchanged except for an explicitly approved semantic identity migration.
16. Client-only dependencies and systems do not enter server/headless feature graphs.
17. Focused catalog/signature/recipe/timeline/materialization/lifecycle tests, representative network/recovery tests, canonical checks, lint, tests, and native evidence pass.
18. Required user presentation/readability feedback is recorded and every item is disposed before closeout.
19. Durable documentation and asset provenance reflect the final architecture, authoring rules, accessibility, fallbacks, and verified limitations.
20. Ticket learning records mistakes, causes, prevention, and reusable lessons before status becomes `done`.

# Verification plan

## Focused pure/catalog tests

- schema version and denied unknown fields;
- duplicate/missing/extra mappings, recipes, and layers;
- invalid IDs/keys/string lengths;
- missing references and reference cycles;
- maximum mapping/recipe/layer/expanded-layer capacities;
- fallback and reduced reachability;
- invalid renderer/geometry/anchor/timeline combinations;
- non-finite and out-of-range geometry/timeline values;
- deterministic signature accent collapse and precedence;
- synthetic new semantic mapping using existing layers without renderer dispatch edits;
- synthetic multi-layer order and cleanup;
- normal/reduced/primitive selection;
- mandatory/decorative classification rules;
- particle asset/cap validation if accepted.

## ECS/presentation tests

- producer-before-resolver and deterministic cross-producer ordering;
- total and per-recipe/layer cap eviction;
- timeline delay/start/peak/end and cleanup;
- authoritative deadline catch-up after late observation;
- attached signature snapshot and orphan cleanup;
- projectile body footprint/pose parity;
- lobbed shadow stays planar while body follows replicated arc;
- persistent field exact geometry and state-driven teardown;
- custom composed weapon style resolution;
- elemental field kind selects the matching activation/area recipe;
- reduced mode preserves mandatory boundary;
- material/asset high-water remains bounded;
- renderer failure closes safely without affecting gameplay.

## Role and canonical checks

At minimum, use repository-canonical commands appropriate to the touched graph:

- focused client library tests for `client::vfx` and presentation modules;
- `cargo check --locked --no-default-features --features server --lib`;
- `cargo check --locked --no-default-features --features client --lib`;
- `just check`;
- `just lint`;
- `just test`;
- targeted network/recovery/performance commands identified during Phase 0;
- script syntax and `git diff --check`.

If a new client-only optional feature is introduced, canonical checks must exercise both accepted feature configurations and prove server isolation.

## Native evidence matrix

Required scenarios:

1. normal effects, primitive fallback off;
2. reduced effects;
3. forced primitive world;
4. Cold weapon plus Cold field;
5. Fire weapon plus Fire field;
6. Cold and Fire overlapping in routed 3v3;
7. lobbed/explosive flight and impact;
8. routed return to lobby and second-match recreation;
9. late join or recovery scenario for persistent fields;
10. maximum expected active effect/emitter overlap.

Capture:

- exact command/config;
- map, mode, loadouts, controls/automation;
- screenshots or video frames at activation, active, overlap, and expiry;
- frame p50/p95/p99/max;
- effect/entity/mesh/material/image/particle high-water and terminal values;
- cleanup/recovery observations;
- normal/reduced/primitive comparison;
- readability observations and requested user feedback.

## Playtest questions

- Can players distinguish Cold, Fire, Poison, and Restoration without relying on hue alone?
- Can players distinguish local/ally/enemy ownership while retaining the element?
- Do exact AoE boundaries remain legible during maximum 3v3 overlap?
- Are projectile body, trail, shadow, and collider expectation coherent?
- Does the lobbed shadow improve landing prediction without implying false gameplay geometry?
- Are explosion flash and smoke durations readable without obscuring fighters or objectives?
- Does reduced mode retain all essential counterplay?
- Are held weapons still too visually similar after VFX improvements, justifying the linked presentation-profile follow-up?
- Are any particle effects distracting, noisy, nauseating, or harmful to target tracking?

# Performance and allocation policy

Phase 0 must record current baselines. The implementation must maintain explicit bounds for:

- active transient roots;
- active attached recipe roots;
- active persistent recipe roots;
- total expanded mesh layers;
- total particle emitters and particles if accepted;
- per-recipe concurrency;
- per-particle-asset instance count;
- retained mesh/material/particle assets;
- pending request batch size;
- catalog mappings/recipes/layers;
- timeline lifetime and delayed starts.

Prefer reuse of cached meshes, materials, and particle assets. Do not create a new mesh or material asset per effect instance unless a measured, bounded pool owns it and tests prove terminal recovery. Profile before adding instancing, pooling, or custom render extraction beyond the demonstrated need.

# Documentation and asset ownership

Update:

- `docs/11-art-and-presentation-direction.md`: signatures, recipes, layers, lifecycle adapters, mandatory/decorative policy, Hanabi decision, reduced/primitive rules;
- `docs/03-weapons-and-abilities.md`: player-facing visual identity expectations only, not renderer schema;
- `docs/13-player-ux.md`: readability/accessibility and reduced-effects behavior when materially affected;
- asset catalog/provenance documentation for every new texture, mesh, particle asset, or imported source;
- canonical evidence/run documentation only through existing repository paths.

Client VFX assets remain under client-owned asset/catalog locations. Gameplay content catalogs retain semantic gameplay definitions and do not acquire source paths or render configuration.

# Feedback disposition

Every architecture review, code review, native observation, and user playtest item must be recorded in the ticket as one of:

- implemented in this ticket;
- deferred to a new linked backlog ticket;
- rejected with evidence/rationale;
- awaiting required evidence, keeping BRL-0094 in `doing`.

Presentation acceptance is required after the Cold/Fire vertical slice and after the lobbed/explosive slice. Later feedback cannot silently broaden this ticket into unrelated art, camera, audio, gameplay, or skin systems.

# Learn-from-errors review

Before closeout record:

- mistakes or regressions encountered;
- whether a missing lifecycle distinction, source identity, capacity bound, fallback, test, or evidence check caused them;
- why review or characterization did not catch them earlier;
- the exact prevention added;
- reusable lessons for future client presentation work;
- whether a recurring Hanabi/visual-evidence workflow warrants a focused skill or script improvement.

# Dependencies and relationships

- BRL-0082 is the completed prerequisite request/registry separation.
- BRL-0059 is historical catalog-policy groundwork.
- BRL-0003 and BRL-0040 provide the elemental weapon/field and persistent Splash gameplay facts this ticket presents.
- BRL-0021 provides lifecycle-owned 3D combat module organization.
- BRL-0001 protects projectile body/collider/aim agreement.
- BRL-0046 is related team-colored projectile readability and must be reconciled rather than duplicated.
- BRL-0070 identified the post-refactor extension seam but does not own this new player-visible VFX scope.

## Phase 0 implementation findings and ordered execution

BRL-0097 froze the current VFX inventory and first recipe contract. Delivery source identity can use existing replicated `ReplicatedAttackSource` plus `ResolvedMatchLoadout` through a 512-entry FIFO client cache; no protocol migration is planned. The initial catalog bounds are 64 mappings, 128 recipes, 256 reusable layers, eight layers per recipe, 96 transient roots, per-recipe cap 1..=96, and ten-second maximum fixed duration. Initial timelines use static material alpha and bounded transform/scale keyframes; animated alpha waits for a proven bounded material-uniform or reclaimed-pool implementation. The durable details live in `docs/11-art-and-presentation-direction.md` and BRL-0097.

Implementation proceeds through blocker links in this order:

1. BRL-0097 — characterize and freeze the contract.
2. BRL-0098 — read-only VfxLab production viewer.
3. BRL-0099 — bounded layered mesh recipe migration.
4. BRL-0100 — semantic signatures and lifecycle adapters.
5. BRL-0101 — distinctive Cold and Fire weapon/area VFX.
6. BRL-0102 — evidence-gated particle adapter decision; a documented no-adoption result is valid.
7. BRL-0103 — lobbed projectile and explosion VFX using the accepted adapter decision.
8. BRL-0104 — remaining in-scope families, documentation, feedback, and BRL-0094 closeout.

The read-only VfxLab is early so it captures baseline/parity evidence. Full VfxLab editing and automation remain under BRL-0096 and do not block BRL-0094.


# Product quality gate — AAA-level effect finish

The target for normal presentation is AAA-level visual quality for a stylized top-down arena shooter. This is a player-visible acceptance requirement, not optional polish and not a claim that effects must be photorealistic or use an unrestricted production budget.

The implementations proposed and demonstrated so far have too often looked cheap: flat colored fills, bare geometric rings, sparse spheres or repeated primitives, uniformly synchronized motion, weak depth, and compositions that prove the renderer path without delivering a convincing finished effect. Technical correctness, composability, determinism, exact geometry, cleanup, and good performance are necessary but are not sufficient for visual acceptance.

A shippable normal-quality effect must demonstrate:

- a coherent authored silhouette and recognizable visual identity at the gameplay camera scale;
- layered color, value, depth, and motion rather than a collection of visibly independent primitives;
- intentional temporal composition, including anticipation or activation, sustained behavior, impact or pulse, and resolution where the lifecycle calls for them;
- bounded organic variation and secondary motion that avoid obvious repetition or synchronized mechanical pulsing;
- integration with the real map surface, production camera, fighters, objectives, lighting, relationship cues, and overlapping combat;
- an exact readable gameplay boundary and counterplay information without making the presentation look like a debug overlay;
- a strong element, weapon, or ultimate identity that remains readable without relying on bloom or hue alone;
- production-context native captures and explicit user visual acceptance against the intended quality reference.

Reuse must be invisible in the final composition: effects may share recipes, meshes, materials, shaders, particles, and lifecycle adapters internally, but they must not all look like recolored versions of the same ring, sphere, or pulse.

Reduced-effects and primitive modes may be intentionally simpler, provided they preserve critical geometry and identity. Their limitations must not define or cap the visual quality of the normal presentation.

When the bounded mesh vocabulary cannot meet this bar, the implementation should use an appropriate retained custom material, authored mesh/texture, or accepted particle adapter while preserving client-only ownership, deterministic semantic inputs, resource bounds, accessibility, and server isolation.

BRL-0102, BRL-0103, BRL-0104, and later effect-design follow-ups inherit this quality gate. BRL-0094 must not close based only on architectural or automated evidence if the remaining normal effects still read as cheap or provisional in native gameplay.

## Closeout audit after BRL-0104 acceptance — 2026-09-01

The ordered core implementation chain is complete: BRL-0097, BRL-0098, BRL-0099, BRL-0100, BRL-0101, BRL-0102, BRL-0103, and BRL-0104 are all `done`. The user explicitly accepted BRL-0104's completed presentation matrix and directed that ticket to close.

BRL-0094 remains `doing` for two deliberately deferred product-quality follow-ups, in order:

1. BRL-0107 integrates ally/enemy ownership into each effect's own visual language and removes the unrelated green effect overlay. Its open user correction must be incorporated before implementation: self treatment belongs to the player's fighter-selection ring and is not required on weapon/ultimate effects. This ticket owns relationship readability, normal/reduced/primitive parity, contrasting surfaces, and dense overlap.
2. BRL-0108 then previews the production resolver/materializers on real built-in maps and records paired VfxLab/game evidence. This ticket owns map-surface contrast, gameplay scale/camera/clutter parity, cleanup while switching contexts, and final production-context native feedback.

After BRL-0108, BRL-0094 needs only a closeout audit confirming that BRL-0107/0108 evidence satisfies acceptance criterion 9 and the production-context AAA quality gate, followed by final feedback disposition, Ticket sync, and status transition. No additional core recipe, signature, lifecycle-adapter, particle, lobbed/explosive, or effect-family implementation is currently identified.

BRL-0096, the editable VFX Workbench, remains a related `todo` but explicitly does not block BRL-0094; its read-only viewer foundation was delivered by BRL-0098.
