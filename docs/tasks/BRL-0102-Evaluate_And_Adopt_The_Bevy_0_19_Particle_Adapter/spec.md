# Decision gate

Use local dependency metadata/examples and current primary documentation as needed to verify exact Bevy 0.19 compatibility, feature graph, platform support, lifecycle semantics, deterministic limits, shader/runtime cost, and maintenance status. Prototype only the smallest adapter needed for sparks/embers/frost fragments/smoke.

# Adoption requirements

- Client-only dependency and adapter; zero server feature-graph impact.
- Recipes reference bounded particle profiles through an optional layer, while CriticalGeometry remains mesh-based.
- Explicit global/per-effect capacity, spawn/lifetime cleanup, reduced/primitive fallback, asset readiness, diagnostics, and evidence metadata.
- Deterministic CPU-side profile/seed selection is required; exact cross-GPU particle positions are not claimed unless measured and proven.

# Acceptance criteria

- A recorded adopt/defer decision includes compatibility evidence, native Metal measurements, visual value, failure behavior, and maintenance cost.
- If adopted, Cold and Fire receive a restrained decorative upgrade with no loss of readability or bounds.
- If deferred, downstream recipes retain a complete mesh-only path and the decision does not block BRL-0103.

# Verification

Role checks, lifecycle/capacity tests, reduced/primitive tests, native Metal performance and paired captures, plus leak/resource high-water evidence.


# Implementation decision — provisional adoption spike, 2026-08-31

## Compatibility evidence

- bevy_hanabi 0.19.0 declares compatibility with Bevy 0.19 and Rust 1.95, matching Brawler's Bevy 0.19.1 and Rust 1.95 toolchain.
- The crate supports disabling defaults and enabling only its 3d feature.
- Its GPU simulation uses Bevy's render pipeline and wgpu 29 with Metal support.
- Hanabi documents one-shot bursts, rates, randomized attributes, gradients, custom meshes, and 3D camera rendering, which cover the demonstrated decorative needs.
- A current upstream macOS pink-screen report exists for versions since 0.18. This makes the native Metal run a real adoption gate rather than a formality.

Primary references: the official djeedai/bevy_hanabi changelog and Cargo manifest, plus the 0.19.0 crate documentation.

## Provisional architecture

Adopt only if the native gate passes:

- Add bevy_hanabi 0.19.0 as an optional dependency enabled by Brawler's client feature, with default features disabled and only 3d enabled.
- Keep the dependency and every adapter type under src/client; server and bare gameplay builds must not resolve the crate.
- Extend the client-only VFX catalog with a bounded particle-layer collection. Recipes reference zero or more named particle layers in addition to their mandatory mesh layers.
- Particle layers are always Decorative; the schema cannot mark them as CriticalGeometry or Identity.
- The first typed profiles are restrained Cold fragments and Fire embers. The adapter owns concrete Hanabi assets and modifier construction; authored data stays bounded and cannot expose arbitrary Hanabi expression graphs.
- Use stable presentation identity and recipe/layer identity for CPU-side seed/profile selection. GPU particle trajectories are not promised bit-identical across devices.
- Normal Cold/Fire activation and impact recipes may add one particle layer. Reduced recipes omit all particle layers; primitive presentation also suppresses them.
- Existing mesh recipes remain complete and authoritative for silhouette, exact geometry, identity, timing, fallback, and cleanup if particles are unavailable.
- One transient root owns both mesh and particle children. Existing root concurrency and lifetime cleanup therefore bounds emitters and releases them with the effect.
- Add explicit per-layer particle capacity/count/lifetime limits and a global active-emitter ceiling no higher than the existing transient root ceiling.
- Evidence reports active particle emitters, authored capacity, profile identity, and high-water count.

## Initial bounds

- At most 2 particle layers per recipe.
- At most 256 particles of GPU capacity per layer.
- At most 96 simultaneously active transient particle emitters.
- Particle lifetime remains within the existing 10-second VFX lifetime ceiling; the first profiles use sub-second lifetimes.
- Reduced and primitive recipes use zero particle emitters.

## Native adoption gate

The adapter is adopted only if a macOS/Metal VfxLab and production-client run show:

1. no pink-screen, shader compilation, validation, or teardown failure;
2. visible improvement over the mesh-only Cold/Fire effects at gameplay camera scale;
3. stable cleanup and emitter/resource counts after repeated replay;
4. acceptable frame-time and bounded behavior under a representative 3v3 overlap preset;
5. no loss of exact boundary, fighter, projectile, objective, or relationship readability.

If any gate fails, remove the dependency/adapter, retain the mesh-only path, record the measured deferral, and unblock BRL-0103.

# Adoption result and implementation evidence — 2026-08-31

## Decision

Adopt bevy_hanabi 0.19.0 narrowly as the client-only decorative particle adapter. The adoption is deliberately limited to validated recipe profiles; CriticalGeometry, Identity, gameplay timing, authority, and fallback remain retained mesh/state concerns. The first Cold and Fire impact profiles validate the adapter and are not the final AAA art pass required by BRL-0094.

## Implemented contract

- The optional dependency is enabled only by the client feature with default features disabled and the 3d feature enabled. Server compilation and the server feature-graph guard both pass without Hanabi.
- VFX catalog schema 6 adds a separate bounded particle-layer table. Particle layers cannot declare a readability class, so they are structurally Decorative.
- Recipes reference at most two particle layers and at most eight combined layers. Catalog capacity is 64 particle layers; per-emitter capacity is at most 256; active transient emitters are capped at 96; lifetime is capped at 10 seconds.
- Normal Cold and Fire impact/ultimate-impact recipes add one seeded fragment/ember burst. Reduced and primitive variants materialize no particle emitters and retain complete mesh fallbacks.
- The adapter builds two retained EffectAsset handles at startup, uses retained shaped meshes, emits upward around a fighter-readable impact volume, and creates no per-frame particle, mesh, or material assets.
- Stable presentation identity plus recipe/layer/occurrence identity determines the per-instance seed. GPU trajectories remain statistically rather than pixel identical across hardware.
- A transient recipe root owns particle children and extends its cleanup lifetime through the longest particle layer. Oldest-first root eviction, per-recipe concurrency, and the global emitter ceiling remain bounded.
- One-shot bursts wait for two consecutive ready observations before reset, avoiding a cold-pipeline burst loss observed during the native spike.
- VfxLab pause/step now also controls Hanabi simulation. Technical diagnostics and paired JSON evidence expose particle profile, capacity, burst count, seed, active/high-water/suppressed emitters, and retained particle-asset count.

## Native Metal and visual evidence

Apple M3 / Metal runs showed no pink screen, shader validation error, GPU panic, or teardown failure. Early captures exposed and corrected two actual problems: automatic captures outlived the sub-second transient, and an emitter volume placed inside the fighter reference was depth-occluded. Paused one-tick captures now show shaped Fire embers and Cold fragments around the fighter silhouette:

- target/brl-0102-fire-particles-final/vfx-lab-combat-impact-000001-1788202885187-000.png and paired JSON
- target/brl-0102-cold-particles-final/vfx-lab-combat-impact-000001-1788202901750-000.png and paired JSON

Each paired snapshot records one active emitter, high-water one, zero suppression, capacity 64, the authored burst count, and the stable seed. A later expired capture records zero transient roots and zero active emitters, confirming ordinary root cleanup.

The particle shapes and motion add visible elemental breakup beyond the retained flash/rings, but this is a technical adoption slice. It does not close the project-level AAA-quality requirement; BRL-0094 and downstream effect tickets must replace/tune these profiles through native review rather than treating the first fragments/embers as final art.

## Performance and capacity evidence

A release native routed Practice Wipeout 3v3 run (one native client plus five authoritative bots, ordinary map and combat path) passed the locked renderer report at target/brl-0102-practice-3v3-render.txt:

- 1,261 samples over the 21-second measurement window;
- frame p50 16.663 ms, p95 16.996 ms, p99 17.169 ms, max 25.443 ms;
- 2 frames over 25 ms, zero over 50 ms, zero over 100 ms;
- effect high-water 13 and terminal zero; visual-root high-water 19 and terminal zero;
- two retained particle assets; no unbounded mesh/material/image growth.

The production 3v3 automation uses neutral default loadouts, so its scheduled screenshots report zero active particle emitters while measuring the installed adapter and retained assets. Actual Metal particle execution is covered by the paired Cold/Fire VfxLab captures; the materializer capacity test drives 108 particle-bearing pending effects and proves that active emitters settle at the global ceiling of 96. A shortened 10-second routed report was rejected only because it supplied 600 samples below the locked 1,200-sample minimum; its p95 was 17.139 ms. A subsequent human-client 21-second attempt ended with the short Wipeout worker lifecycle before reporting, so the supported Practice 3v3 route supplied the valid locked measurement.

## Failure behavior and maintenance cost

If a particle asset is unavailable, materialization skips that decorative child while the mesh recipe remains complete. Reduced/primitive selection also removes the adapter from the effect. Catalog validation rejects duplicate/missing IDs and all capacity/lifetime violations. Runtime diagnostics count suppression instead of exceeding the emitter ceiling.

The dependency adds meaningful client build/link cost: the cold release client build for the measured run took about 2 minutes, and several development relinks after adapter edits took roughly 1 minute; cached runs were sub-second builds. This cost, the current upstream macOS issue history, and the additional GPU lifecycle surface are accepted only for bounded client presentation value. Re-evaluate or remove the adapter if later native regressions, resource growth, or art workflow friction outweigh that value.

## Verification

- cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings — pass.
- cargo test --locked --no-default-features --features client --lib — 589 passed.
- cargo test --locked --no-default-features --features client --lib particle — 6 passed before the final readiness regression test; the final focused rerun is required at handoff.
- just _check-server — pass.
- just _server-features — pass; server feature graph excludes client presentation capabilities.
- Native VfxLab Cold/Fire paired captures — pass on Apple M3 / Metal.
- Native routed Practice Wipeout 3v3 locked render report — pass.

Human visual acceptance remains open. Keep BRL-0102 in doing until the user confirms the adapter-level Cold/Fire upgrade is an acceptable base for downstream AAA effect work or requests correction/defer.

## Evidence path and final focused-test correction

The exact final paired capture stems are:

- target/brl-0102-fire-particles-final/vfx-lab-combat-impact-000001-1788202885322-000.{png,json}
- target/brl-0102-cold-particles-final/vfx-lab-combat-impact-000001-1788202901788-000.{png,json}

These replace the slightly incorrect millisecond suffixes in the preceding evidence list. The final focused particle rerun passed 7 tests, including the cold-pipeline two-ready-observation regression test.

Final post-change full client library verification: `cargo test --locked --no-default-features --features client --lib` passed 590/590 tests.


# Human acceptance and learning review — 2026-08-31

The user reviewed the native Cold and Fire adapter captures and responded “looks good continue.” This accepts BRL-0102 as the bounded particle foundation for downstream effect work. It does not waive the AAA-quality bar recorded on BRL-0094; each downstream family still requires its own native visual review and tuning.

## Feedback disposition

- Adapter-level Cold/Fire visual value: accepted.
- Client-only ownership, deterministic profile selection, and mesh fallback: accepted as implemented.
- Further profile polish: belongs to the downstream family tickets and BRL-0094 closeout rather than extending this dependency-evaluation ticket.

## Learn-from-errors review

- A one-shot GPU burst cannot be assumed renderable when its ECS entity first exists. Waiting for two stable compiled-ready observations prevents cold-pipeline loss; retain the focused regression test for future adapter upgrades.
- Effects that are visible in isolation can disappear behind fighter geometry at the production camera. Author and verify emitter volume outside the fighter silhouette, using paired native captures rather than relying on catalog numbers.
- Automated evidence captured after a sub-second transient proves cleanup but not appearance. Pause/single-step capture is the reliable visual-evidence path for short effects.
- GPU-particle dependencies add non-trivial clean-build and link cost. Keep the adapter optional, client-only, typed, bounded, and removable; do not expose arbitrary shader graphs through gameplay recipes.
- A representative production performance run may install the adapter without exercising styled particles when its loadout is neutral. Combine production-route measurements with targeted native VfxLab execution and explicit capacity tests.
