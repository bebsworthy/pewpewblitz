# Scope

- Present lobbed body, trajectory readability, ground-projected shadow, and bounded trail without changing authoritative motion or collision.
- Compose impact as explicitly timed layers: flash, ground flash, shockwave, fragments/sparks, smoke/debris, with critical radius/timing retained in every variant.
- Use the BRL-0102 particle adapter only if adopted; otherwise deliver the complete mesh recipe and decorative fallbacks.
- Support ordinary and ultimate/context variants through semantic recipes, not bespoke gameplay systems.

# Acceptance criteria

- Flight path and landing point are readable from the production camera while visuals remain decoupled from the collider.
- Explosion sequencing is distinctive, deterministic in authored choices, bounded, and cleans up completely.
- Shockwave/ground geometry matches the authoritative effect and survives reduced/primitive mode.
- Production, VfxLab, performance evidence, and focused playtest pass.

# Verification

Trajectory/shadow/timeline/cleanup tests, recipe validation, lifecycle and late-join cases, VfxLab normal/reduced/primitive matrix, role/performance checks, native paired capture and playtest feedback.


# Technical design and phases — 2026-08-31

## Ownership and data flow

- The authoritative server continues to own launch, repaired landing point, flight ticks, collision, payload radius, and the `LobLanded` cue. Presentation never predicts a hit or changes the projectile entity.
- The retained lobbed-projectile visual owns four bounded children for its replicated lifetime: exact bright body/relationship core, a ground-projected shadow under the current planar position, a fixed landing-point marker, and a short tangent-aligned arc trail. These are entity-lifecycle presentation rather than transient recipes because they follow continuously changing replicated facts.
- The client VFX signature cache retains the largest authored `TargetSelection::Area` radius with the attack style. `LobLanded` projects that value into `VfxGeometry::Circle` and the optional request radius. Missing/late identity falls back to the existing point impact recipe rather than inventing geometry.
- Catalog style selection gains an optional carrier selector. `combat.impact + LobbedProjectile + Impact` chooses the explosion family; accent and ultimate context remain orthogonal selectors.
- The explosion is an authored transient recipe: exact full-radius boundary, ground flash, flash body, expanding shockwave, delayed core/echo, plus optional bounded sparks and smoke. Reduced/primitive variants preserve the exact boundary and mesh staging while omitting particles.
- `world-object.explosion` reuses the same explosion layer vocabulary so the oil-barrel path benefits without another renderer system.

## Initial bounds and readability

- Keep the existing maximum of eight combined recipe children. Normal explosions use at most five mesh layers and two particle layers; reduced explosions use at most three mesh layers and no particles.
- The exact boundary is full authoritative radius for its complete visible lifetime. Decorative shockwave scaling never substitutes for it.
- Ground aids are unlit, non-shadowing, and remain below projectile/fighter geometry. The landing marker communicates destination, not damage radius; the authoritative explosion boundary appears only on the landing cue.
- Particle profiles are typed `ExplosionSparks` and `ExplosionSmoke`; arbitrary Hanabi graphs remain unavailable to content.
- All authored choices and CPU seeds are deterministic. Exact cross-GPU particle positions remain explicitly non-deterministic.

## Implementation phases

1. Extend carrier-aware catalog selection and cache the authored lobbed payload radius, with selector/fallback/late-identity tests.
2. Add bounded lobbed-flight shadow, landing marker, and tangent trail with pure pose tests and production/VfxLab lifecycle cleanup.
3. Author schema-8 neutral, Cold, Fire, and ultimate explosion mappings/layers plus reduced variants; reuse them for world-object explosions.
4. Add typed sparks/smoke particle compilation and validate combined-layer/capacity cleanup behavior.
5. Run focused and canonical role checks, capture normal/reduced/primitive VfxLab flight and explosion evidence, then run native production performance and request visual feedback.

# Implementation and evidence — 2026-08-31

## Delivered

- Added carrier-aware schema-8 style selection and authored neutral, Cold, Fire, ordinary, ultimate, reduced, and primitive-compatible lobbed-explosion recipes.
- Retained the largest authored area target radius in the bounded attack-signature cache; `LobLanded` now emits exact circle geometry. A missing late-cue radius explicitly falls back to the generic point impact rather than dropping the effect or inventing geometry.
- Added production-owned lobbed-flight aids parented to the replicated projectile: projected ground shadow, fixed landing marker, and tangent-aligned short trail. All poses derive from authoritative launch/landing ticks and the retained visual arc.
- Added reusable `GroundThinRing` and `GroundDisc` mesh vocabulary plus typed bounded `ExplosionSparks` and `ExplosionSmoke` GPU particle profiles. World-object explosions reuse the same layered vocabulary.
- Added the VfxLab `--lobbed-impact` production request path, fixture-relative reference placement, operator explanation, and deterministic capture metadata.
- Reduced mode selects three mesh layers and zero particle layers. Primitive mode retains the normal five-mesh recipe/exact boundary while suppressing GPU materialization. Normal recipes remain bounded to five mesh plus two particle layers; ultimate remains within the frozen eight-child ceiling.

## Automated verification

- `cargo test --locked --no-default-features --features client --lib`: 593 passed.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings`: passed.
- `just _check-server`: passed.
- `just _server-features`: passed; client presentation remains excluded from the server graph.
- `cargo test --locked --no-default-features --features network-test --test performance -- --nocapture`: 12 passed.
- `cargo fmt --all -- --check`/`git diff --check`: clean after formatting.

## Native evidence

- Normal explosion: `target/brl-0103-explosion-final/vfx-lab-combat-impact-000001-1788208107385-000.{png,json}`.
- Lobbed flight at apex: `target/brl-0103-flight-final/vfx-lab-lobbed-projectile---arc-000001-1788208125117-000.{png,json}`.
- Reduced Fire explosion: `target/brl-0103-explosion-reduced/vfx-lab-combat-impact-000001-1788208898921-000.{png,json}`; resolved `fire_lob_explosion_reduced`, three mesh layers, zero authored/active particle emitters.
- Primitive Fire explosion: `target/brl-0103-explosion-primitive/vfx-lab-combat-impact-000001-1788208912817-000.{png,json}`; resolved `fire_lob_explosion`, five mesh layers, two authored particle layers, zero active GPU emitters.
- Native captures completed without panic, shader compilation failure, or missing-material fallback. The window teardown warning is the existing harmless Bevy/winit capture-exit diagnostic.

## Remaining acceptance

- Human visual review in VfxLab/production camera remains required. Keep BRL-0103 in `doing` until that feedback is accepted or dispositioned.

# Playtest feedback correction — 2026-08-31

- User accepted the layered explosion presentation: “explosion seem good.” Explosion feedback is complete.
- User reported that the Fire lobbed projectile tail did not follow the flight normal and therefore did not read as a trail.
- Cause: the retained Fire trailing lobes were fixed in the unrotated projectile-root frame. The separate short segment used a sampled chord, so neither established one shared analytical tangent contract for the complete projectile body.
- Correction: added a dedicated lobbed-projectile body child rotated from local +X onto the analytical derivative of the authoritative parabolic visual arc. Elemental body geometry and Fire trailing lobes inherit that rotation; ground shadow and landing marker remain unrotated sibling aids. The short segment now uses the same tangent rotation.
- Regression evidence asserts positive vertical tangent during ascent, horizontal tangent at apex, negative tangent during descent, and alignment of the trail mesh's local +X axis with the tangent.
- Ascending native capture: `target/brl-0103-flight-tangent-fix/vfx-lab-lobbed-projectile---arc-000001-1788211022559-000.{png,json}`.
- Descending native capture: `target/brl-0103-flight-tangent-fix-descent/vfx-lab-lobbed-projectile---arc-000001-1788211062676-000.{png,json}`.
- Affected verification: focused tangent test passed; full client library suite 593/593 passed; strict client all-target Clippy passed; `git diff --check` passed.
- Remaining acceptance: human confirmation that the moving Fire tail now follows the arc in VfxLab. BRL-0103 stays `doing` until confirmed.

# Playtest feedback correction — longer Fire wake — 2026-08-31

- User requested a longer tail with trailing smoke after the tangent correction.
- Extended the hot core from three to four staggered Fire lobes and lengthened the tangent ribbon from 8.5% to 14% of the authoritative flight duration.
- Added a bounded retained smoke wake sampled from earlier positions on the same authoritative parabolic arc. Normal/Reduced/Primitive/ReducedPrimitive variants use 8/3/5/2 smoke puffs respectively; older samples expand, drift, and vary spacing so the result reads as a smoke cloud rather than another gameplay geometry marker.
- Smoke remains presentation-only, deterministic, parent-lifetime bounded, non-shadowing, and does not affect collision, landing, or server code.
- Native mid-flight capture: `target/brl-0103-flight-smoke-final/vfx-lab-lobbed-projectile---arc-000001-1788212071003-000.{png,json}`; evidence reports one projectile root and eleven bounded aids (shadow, landing marker, tangent ribbon, eight smoke puffs).
- Affected verification: smoke sampling assertions passed; full client library suite 593/593 passed; strict client all-target Clippy passed; formatting and diff checks passed.
- Remaining acceptance: human confirmation of the longer hot/smoke wake. BRL-0103 stays `doing` until confirmed.

# Playtest feedback correction — true lingering smoke — 2026-09-01

- User rejected the retained mesh-puff wake because it moved as a static formation with the projectile and requested real emitted particles that linger and fade.
- Removed the mesh-puff approximation. The Fire lobbed projectile now owns one bounded Hanabi rate emitter only in the normal non-primitive variant.
- Advanced the VFX catalog to schema 8 and replaced the burst-only particle count with the typed `Burst(count)` / `RatePerSecond(count)` spawn contract. The proving `ProjectileSmoke` profile is authored at capacity 32, rate 28/s, 900 ms lifetime, randomized local origin/velocity, expanding size, upward acceleration, and a transparent color-over-lifetime gradient.
- The emitter follows the projectile only as the source transform. Hanabi global simulation space leaves previously emitted particles at their world positions so the projectile moves away from them. When the projectile disappears, the spawner becomes inactive while particles continue aging; the emitter is removed after the authored 900 ms lifetime.
- Retained and transient emitters share the frozen global cap of 96. Reconciliation reserves capacity before spawning and the transient recipe materializer subtracts retained emitters from its available budget. Reduced and primitive variants omit the emitter.
- Lifecycle regression evidence verifies source following, emission shutdown on owner removal, lingering for a partial lifetime, and cleanup after expiry.
- Native moving capture: `target/brl-0103-flight-real-smoke/vfx-lab-lobbed-projectile---arc-000001-1788214355785-000.{png,json}`; evidence reports one projectile, one retained rate emitter, and three non-smoke mesh aids.
- Verification: full client library suite 594/594 passed; strict client all-target Clippy passed; server build and feature-isolation checks passed; formatting and diff checks passed.
- Remaining acceptance: human confirmation of the real emitted smoke in motion. BRL-0103 stays `doing` until confirmed.

# Final acceptance and learning — 2026-09-01

- The user accepted the real emitted smoke trail after native playtesting ("ok"). The previously accepted explosion and the projectile-tail direction, length, and smoke feedback are fully dispositioned.
- BRL-0103 acceptance criteria are satisfied: the lobbed projectile and impact use layered authored recipes, the projectile visual follows its trajectory tangent, Fire has a longer emitted smoke trail whose particles remain in world space and fade, reduced/primitive degradation remains bounded, diagnostics expose the active particle spawn mode, and automated plus native verification passed.

## Learn from errors

- The first projectile tail used an unsuitable child-frame orientation, so it did not follow the trajectory normal/tangent. Future moving-effect reviews must exercise ascent, apex, and descent rather than judging one pose.
- The first smoke correction used fixed mesh samples that moved as a formation. That met a still-image approximation but not the requested temporal behavior. When feedback says an effect must emit, linger, and fade, verification must inspect world-space lifetime ownership over motion, not only a screenshot.
- The reusable contract is now explicit: a moving source drives a globally simulated retained emitter; source removal stops spawning; the emitter remains for the authored particle lifetime; and the shared emitter cap bounds the result.

# Reopened production integration feedback — 2026-09-01

- Native user feedback after the previous closeout: the emitted smoke trail works in VfxLab but does not work in the real game.
- BRL-0103 is reopened. Completion now requires evidence from the production gameplay path, not VfxLab alone: the real lobbed Fire projectile must select and materialize the same retained world-space smoke emitter, particles must visibly linger and fade during live gameplay, and the affected production lifecycle must have an automated regression check where practical.

# Production integration correction — 2026-09-01

- Root cause: VfxLab creates a complete fixture entity atomically, while Lightyear may expose a production projectile before its replicate-once `ReplicatedAttackSource` identity is available. `reconcile_projectile_visuals` materialized a neutral root on that partial frame and treated the root as final for the projectile lifetime. When the authored Fire signature arrived later, neither the Fire body style nor its retained smoke emitter could be added.
- Correction: projectile reconciliation now retains the signature attached to each visual root. An unchanged signature keeps the root; a late or changed authored signature replaces the root through the same production `spawn_projectile` path. Any emitter owned by the replaced root is removed before the replacement is allocated, preserving the shared 96-emitter budget and preventing duplicates.
- Regression coverage includes the pure spawn/keep/replace decision and a production ECS sequence that first materializes a neutral lobbed projectile, then inserts its replicated attack identity, resolves an Arc Launcher Fire loadout, and verifies that exactly one rebuilt lobbed Fire visual remains.
- Full client library verification: 596 passed, 0 failed. Strict client all-target Clippy passed. Dedicated-server build and client-feature exclusion passed. Formatting and diff checks passed.
- Production routed evidence used an isolated persisted profile with Arc Launcher plus Incendiary Module in a real Practice match. `target/brl-0103-production-smoke-4y8HXn/captures/brawler-000400.{png,json}` visibly shows the emitted world-space smoke wake; its evidence records Fire `LobbedProjectile` signatures, three retained projectile-smoke emitters, seven total bounded particle emitters, and zero suppressed emitters. Adjacent captures repeatedly record one to three retained emitters with zero suppression.
- The dense ten-update screenshot cadence deliberately prioritized temporal VFX evidence and caused the auxiliary render report to miss only its sample-count threshold; it is not performance acceptance evidence. The prior bounded performance gate remains unchanged.
- Remaining acceptance: user confirmation in their normal game profile. BRL-0103 remains `doing` until that retest is accepted.

# Playtest feedback — remove debug-like yellow trail — 2026-09-01

- User confirmed the production smoke correction but rejected the normal yellow `ArcTrail` bar on the Arc Launcher projectile because it reads as a debug trajectory indicator rather than finished weapon VFX.
- Remove that mesh aid from the production lobbed-projectile composition and VfxLab parity fixture. Preserve the authored parabolic body pose, ground-projected shadow, fixed landing marker, and optional Fire smoke emitter. Update evidence/count expectations so the yellow bar cannot regress.

# Playtest correction implemented — remove debug-like yellow trail — 2026-09-01

- Removed the retained `ArcTrail` cuboid, its sampled-chord pose helper, and the yellow muzzle material from the lobbed-projectile composition. Production and VfxLab now retain only the ground-projected shadow and fixed landing marker as flight aids.
- Preserved the analytical tangent rotation for the authored projectile body and Fire lobes. The optional Fire particle emitter remains attached to the projectile source while emitted smoke particles stay in world space, linger, expand, and fade.
- Updated VfxLab explanatory copy and the durable presentation contract. The thin fighter-to-target line visible while firing in the product client is the separate targeting indicator, not a projectile child, and is intentionally unchanged.
- Regression coverage now asserts exactly two lobbed aids before and after late replicated Fire identity replaces the neutral visual, preventing the debug-like yellow bar from returning.
- Verification: focused arc/body and late-replication production ECS tests passed; the full client library suite passed 596/596; strict client all-target Clippy passed; server all-target check and server feature-exclusion check passed; formatting and `git diff --check` passed.
- Routed native Practice evidence with the isolated Arc Launcher plus Incendiary Module profile: `target/brl-0103-production-no-debug-tail/captures/brawler-000400.{png,json}`. The frame visibly shows the orange projectile and lingering smoke without the solid yellow projectile tail; JSON records one lobbed projectile root, exactly two lobbed aids, two retained projectile-smoke emitters, four total bounded emitters, and zero suppressed emitters.
- BRL-0103 remains `doing` pending user visual confirmation of this final correction.
