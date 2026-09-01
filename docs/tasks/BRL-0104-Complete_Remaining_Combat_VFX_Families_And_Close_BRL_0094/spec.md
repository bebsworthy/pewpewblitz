# Scope

- Finish pistol/hitscan tracer, muzzle, generic semantic impacts, remaining projectile/body/trail cases, deployable/sentry, dash, status, concealment and other currently supported ultimate/area families not completed by Cold/Fire or grenade slices.
- Reuse layers and recipes where semantics match; preserve distinctive source identity where they do not.
- Validate catalog budgets, high-water/terminal resource evidence, degradation, deterministic replay, late join/reconstruction, and production/VfxLab parity across the complete supported matrix.
- Reconcile `docs/11-art-and-presentation-direction.md`, player UX/evidence docs, controls/runbooks, and deferred backlog tickets.
- Perform required playtest, feedback disposition, and learn-from-errors review for BRL-0094.

# Acceptance criteria

- Every supported combat presentation family resolves to an intentional recipe/fallback and has an isolated VfxLab scenario.
- Required effects remain readable at gameplay scale with bloom disabled and in normal/reduced/primitive modes.
- Server authority/protocol boundaries, deterministic authored choices, resource caps, cleanup, native performance, and routed gameplay evidence pass.
- Deferred authoring/editing remains in BRL-0096; BRL-0094 closes only after all linked implementation acceptance criteria and user feedback are satisfied.

# Verification

Complete focused and canonical automated suite proportional to changes, VfxLab capture matrix, representative routed/native production evidence, performance/resource reports, and user playtest with recorded feedback disposition.

# Implementation findings and ordered phases — 2026-09-01

- Production inventory confirms no hitscan delivery exists today: Pulse Sidearm and Scatter Cannon are replicated straight projectiles. BRL-0104 will not invent a hitscan gameplay/presentation path; it will improve the existing straight-projectile family.
- The first actionable gap is semantic completeness. Cold and Fire have authored transient and retained styles, while Poison and Restoration currently collapse to generic single-material projectile/field geometry and are not selectable in VfxLab. Their signatures and materials already exist, so this is a client-only vertical slice without protocol or gameplay changes.
- Phase 1: add Poison and Restoration to VfxLab; author distinctive retained projectile/field/status silhouettes and transient muzzle/impact/area recipes with bounded reduced/primitive identity channels.
- Phase 2: complete the neutral supported carrier families: straight Pulse/Scatter, melee, cone Spray, Sticky Blomb/Big Blob, persistent Splash, dash, sentry, reveal/concealment, demolition, and remaining status/expiry cues. Add focused VfxLab fixtures only where an existing lifecycle owner needs isolated inspection.
- Phase 3: run the normal/reduced/primitive and representative routed overlap/recreation/resource matrix, reconcile durable docs and deferred tickets, collect user visual feedback, perform the BRL-0094 learning review, and close only after the AAA-quality gate is accepted.
- Phase 1 starts with the retained production/VfxLab path so gameplay-scale silhouettes can be reviewed before multiplying the same language across transient recipes.

## Phase 1 retained-style implementation evidence — 2026-09-01

Implemented the retained/VfxLab half of Phase 1 for Poison and Restoration while preserving the production lifecycle materializers:

- Poison straight and lobbed projectiles use a deterministic bubbling wake; Poison persistent areas use an animated cellular-contamination material with perimeter bubbles.
- Restoration straight and lobbed projectiles use a deterministic rotating vitality spark; Restoration persistent areas use rotating vitality arcs with perimeter petals.
- Normal area variants use the production custom-material path. Reduced and primitive variants preserve bounded mesh fallbacks and exact gameplay boundaries.
- VfxLab now accepts and cycles `poison` and `restoration` accents, creates the real field kinds, and reports their semantic identities.
- The first flat disc/ring capture was rejected during implementation as below the BRL-0094 quality bar and replaced with the procedural retained materials.

Native paired-capture evidence:

- `target/brl-0104-poison-field-procedural-v2/`
- `target/brl-0104-restoration-field-procedural-v2/`
- `target/brl-0104-poison-projectile-v2/`
- `target/brl-0104-restoration-projectile-v4/`

Verification:

- `cargo test --locked --no-default-features --features client --lib`: 598 passed.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings`: passed.
- `just _check-server`: passed.
- `just _server-features`: passed; client presentation remains excluded.
- `cargo fmt --all -- --check` and `git diff --check`: passed.

Phase 1 remains incomplete until the Poison and Restoration transient recipe mappings/layers are implemented and the retained styles receive native visual feedback. BRL-0104 remains `doing`.

## Phase 1 transient-style implementation evidence — 2026-09-01

Completed the authored transient half of Phase 1:

- Added typed Poison and Restoration material keys, a reusable ground vitality-star renderer, and bounded Poison-bubble and Restoration-petal Hanabi profiles.
- Added normal and reduced recipes plus typed selectors for muzzle, direct impact, ultimate impact, lobbed impact, ultimate lobbed impact, and elemental-field activation.
- Lobbed recipes preserve a thin authoritative-radius boundary. Reduced and primitive selection removes decorative particles while retaining mesh identity.
- Upgraded shared retained discs and rings from the default 32-segment geometry to 96/128-segment meshes so large authoritative areas retain a smooth silhouette.
- Stable request identities continue to seed particle randomness; the new profiles do not alter authoritative gameplay or the server feature graph.

Visual iteration disposition:

- Initial unfrozen captures correctly showed expired effects; the evidence path was corrected to use `--paused --sample-ticks N`.
- The first large-radius recipes were rejected during implementation because broad fills obscured the play space and additive overlap washed particles to white.
- Central fills/sparks were reduced, Poison and Restoration particles moved to alpha blending, Restoration burst counts were reduced, and shared ring resolution was raised before accepting the implementation slice.

Native paired-capture evidence:

- `target/brl-0104-poison-impact-frozen/`
- `target/brl-0104-restoration-impact-frozen/`
- `target/brl-0104-poison-lob-impact-v3/`
- `target/brl-0104-restoration-area-activation-v3/`

Verification:

- `cargo test --locked --no-default-features --features client --lib`: 599 passed.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings`: passed.
- `just _check-server` and `just _server-features`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.

Phase 1 implementation is complete. Native user visual feedback remains open while Phase 2 proceeds with the neutral supported carrier families. BRL-0104 remains `doing`.

## Phase 1 user feedback — 2026-09-01

The user accepted the Poison/Restoration retained and transient Phase 1 slice with “ok.” No Phase 1 correction remains open. BRL-0104 stays `doing` and proceeds to Phase 2.

## Phase 2 first slice — Cone Spray presentation — 2026-09-01

Native baseline `target/brl-0104-cone-baseline/` confirms the authoritative clipped cone silhouette and blocker reconstruction are correct, but the current single translucent StandardMaterial exposes the repeated segment geometry as a static striped wedge.

This slice preserves `cone_spray_primitives`, segment bounds, blocker clipping, lifecycle ownership, signature, and server authority. It replaces only the retained client material:
- Normal uses one shared deterministic animated gas material sampled in world space so clipped segments read as one coherent cloud rather than independent stripes.
- Reduced uses a lower-complexity gas variant.
- Primitive and ReducedPrimitive use a simple stable translucent identity.
- Animation derives from `AuthoritativeTick`; no wall-clock randomness, gameplay/protocol change, or server dependency is introduced.
- VfxLab cone fixture is the isolated visual acceptance path; focused ECS/material tests plus canonical client/server verification remain required.

## Phase 2 Cone Spray implementation evidence — 2026-09-01

The initial custom-material capture still exposed every translucent radial segment and was rejected during implementation. The corrected presentation replaces the 33 overlapping render meshes with one bounded triangle-fan surface rebuilt from the same 17 blocker-clipped ray endpoints returned by `cone_spray_primitives`. Gameplay geometry generation, propagation, occlusion, authority, protocol facts, and lifecycle ownership are unchanged.

- Normal and reduced variants use deterministic UV-space turbulent flow, broken curved wisps, and a restrained exact-frontier highlight animated from `AuthoritativeTick`.
- Primitive and ReducedPrimitive use the same clipped surface with a stable low-cost translucent identity.
- Each active cone owns one dynamic mesh asset; reconciliation explicitly removes that asset when the retained carrier disappears.
- The VfxLab fixture and production renderer install the same material plugin and lifecycle systems.

Native capture evidence:

- `target/brl-0104-cone-gas-v4/`
- `target/brl-0104-cone-reduced/`
- `target/brl-0104-cone-primitive/`

Verification:

- `cargo test --locked --no-default-features --features client --lib`: 600 passed.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings`: passed.
- `just _check-server` and `just _server-features`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.

The Cone Spray implementation slice is complete. Native user visual feedback remains open, and the remaining Phase 2 carrier families remain in scope; BRL-0104 stays `doing`.

## Cone Spray production parity feedback — 2026-09-01

The user confirmed that Cone Spray animates in VfxLab but remains static in gameplay. Inspection found that production installed `ConeSprayMaterialPlugin` and materialized the retained surface, but omitted `animate_cone_spray_materials` from `WorldPresentationSet::Animate`; VfxLab scheduled the same system correctly. Accept this as an implementation defect. Add the missing production schedule entry, verify the client/server gates, and retain one production-composition regression assertion so future custom retained materials cannot silently diverge from VfxLab.

## Cone Spray production parity correction evidence — 2026-09-01

Implemented the accepted gameplay-animation correction. Production and VfxLab now call one shared retained-material animation installer covering Fire, Poison/Restoration, and Cone Spray materials; the production call assigns it to `WorldPresentationSet::Animate`. A regression test builds the production foundation, advances `AuthoritativeTick` by one second, runs the production animation set, and verifies the Cone Spray material time uniform advances to one second.

Verification:

- Focused production schedule regression: passed.
- `cargo test --locked --no-default-features --features client --lib`: 601 passed.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings`: passed.
- `just _check-server` and `just _server-features`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.

The reported VfxLab/game parity defect is implemented and automated. Native gameplay confirmation remains requested before the feedback item is considered accepted.

## Phase 2 straight-projectile retained slice — 2026-09-01

Native baseline `target/brl-0104-straight-baseline/` shows the neutral straight carrier as only a small matte collision disc, with no readable motion or weapon language.

Implement one composable neutral straight-bolt presentation rather than branching on built-in preset IDs:
- Preserve the exact replicated projectile-body core and authority geometry.
- Normal adds a tapered, deterministic animated energy wake plus a smaller emissive cap.
- Reduced keeps a lower-complexity, shorter wake.
- Primitive and ReducedPrimitive retain only the exact core.
- Pulse Sidearm reads as one strong bolt; Scatter Cannon gains identity from its authoritative five-delivery spread using the same reusable bolt recipe.
- Exclude lobbed projectiles explicitly; this must not restore the removed Arc Launcher tail.
- Drive material time from `AuthoritativeTick` through the shared production/VfxLab retained-material scheduler.
- Keep current team-color ownership behavior for this slice; BRL-0107 owns the later integrated observer-relative ownership language.
- Review the retained gameplay-scale silhouette in VfxLab before extending the same neutral language across muzzle/impact recipes.

No gameplay, protocol, server, projectile movement, collision, or delivery changes are in scope.

## Phase 2 neutral straight-projectile implementation evidence — 2026-09-01

Implemented the reusable retained neutral bolt presentation shared by Pulse Sidearm and each authoritative Scatter Cannon delivery, without preset-ID branches or gameplay changes.

- Preserved the original replicated projectile mesh and team material as the exact collision-footprint core.
- Normal adds a smaller emissive cap plus a long tapered custom-material energy wake with two deterministic animated filaments.
- Reduced retains a shorter, lower-complexity wake; Primitive and ReducedPrimitive materialize only the exact core.
- The style selector requires the Neutral accent and StraightProjectile carrier, explicitly leaving the Arc Launcher/lobbed path unchanged.
- Production gameplay and VfxLab install the same material plugin and share the AuthoritativeTick-driven retained-material animation schedule. A production-composition regression verifies the trail time uniform advances in gameplay, preventing recurrence of the prior Lab/game animation divergence.
- VfxLab guide text now identifies the reusable Pulse/Scatter language and distinguishes the exact core from the presentation wake.

Visual iteration disposition:

- Baseline `target/brl-0104-straight-baseline/` was rejected because it showed only a small matte disc.
- The first custom wake capture `target/brl-0104-straight-bolt-v1/` exposed incorrect triangle winding and back-face culling; the mesh winding was corrected before acceptance.
- The readable but timid second pass was strengthened at fighter scale. Accepted implementation captures are `target/brl-0104-straight-bolt-v3/`, `target/brl-0104-straight-bolt-reduced/`, and `target/brl-0104-straight-bolt-primitive/`.

Verification:

- Focused straight-style selection/material/production-animation tests: 3 passed.
- `cargo test --locked --no-default-features --features client --lib`: 604 passed.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings`: passed.
- `just _check-server` and `just _server-features`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.

The retained neutral straight-projectile slice is implemented. Native user visual feedback remains open; neutral muzzle/impact unification and the remaining Phase 2 carrier families remain in scope. BRL-0104 stays `doing`.

## Neutral straight-projectile visual feedback — comet integration — 2026-09-01

The user accepted the animated tail motion but rejected the leading core as a separate, boring “hockey puck.” The supplied reference shows the desired relationship: the trailing energy broadens and wraps around a dimensional leading nucleus so the projectile reads as one comet body.

Accept this feedback for the active retained straight-projectile slice:

- Preserve the exact replicated collision-footprint core, but subordinate it inside the presentation rather than leaving it as the dominant exposed silhouette.
- Replace the flat emissive cap with a dimensional emissive nucleus centered on the authoritative core.
- Broaden and round the custom wake around the core, including a forward nose and side wrap, so nucleus and tail read as one continuous comet.
- Keep the local backward-facing trail direction, deterministic `AuthoritativeTick` animation, normal/reduced/primitive ladder, shared Pulse/Scatter recipe, and explicit lobbed exclusion.
- Review the corrected silhouette at gameplay scale in VfxLab before extending the language to transient muzzle/impact recipes.

This is an accepted presentation correction within BRL-0104; no gameplay, collision, protocol, or server change is authorized.

## Neutral straight-projectile comet correction evidence — 2026-09-01

Implemented the accepted “hockey puck” correction while preserving the retained recipe and degradation contracts.

- Normal and Reduced no longer expose the flat cylinder as their dominant head. Their exact planar gameplay footprint is represented by a fully dimensional lit sphere with the same authoritative radius.
- The shared wake mesh now has a rounded forward nose, broader side shoulders, and a closed comet silhouette extending around both sides and ahead of the nucleus.
- The custom trail shader adds a separately weighted animated wrap rim around the head while retaining the deterministic dual-filament tail motion.
- Nucleus materials use restrained emissive energy plus directional-light response, low roughness, and slight metallic response so the head reads as a solid orb inside the glow.
- Primitive and ReducedPrimitive deliberately retain the raw exact-core fallback. Lobbed projectiles remain excluded.
- VfxLab guide text now explains the exact-footprint dimensional nucleus and wrapping Pulse/Scatter comet language.

Visual iteration disposition:

- `target/brl-0104-straight-comet-v1/` was rejected because the original cylinder remained too exposed and the initial forward wrap collapsed to a small point.
- `target/brl-0104-straight-comet-v2/` established the accepted integrated topology.
- `target/brl-0104-straight-comet-v3/` refines the nucleus lighting and is the current normal candidate.
- Degradation evidence is in `target/brl-0104-straight-comet-reduced/` and `target/brl-0104-straight-comet-primitive/`.

Verification:

- `cargo test --locked --no-default-features --features client --lib`: 604 passed.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings`: passed.
- `just _check-server` and `just _server-features`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.

The reported core/tail integration correction is implemented. Native user visual confirmation remains requested before this feedback item is accepted; BRL-0104 stays `doing`.

## Neutral straight-projectile animation-direction feedback — 2026-09-01

The user reports that the live comet pulse travels from the tail toward the nucleus, reversing the intended visual causality. Inspection confirms both shader filament phases use `along * frequency - time * speed`; because longitudinal UV increases from tail to front, constant-phase bands move toward increasing UV and therefore toward the nucleus.

Accept this as a retained-material animation defect:

- Reverse both fast and slow filament time terms so their constant-phase bands propagate from the leading nucleus toward decreasing longitudinal UV into the tail.
- Preserve mesh orientation, projectile travel direction, authoritative-tick timing, deterministic replay, quality variants, and gameplay/server behavior.
- Verify direction using separated paused authoritative-tick captures and the live VfxLab path.

BRL-0104 remains `doing`.

## Neutral comet animation-direction correction evidence — 2026-09-01

Corrected both normal-quality filament phase equations from negative to positive time terms. With longitudinal UV increasing from tail (0) to nucleus (1), a constant phase now has negative longitudinal velocity (`d(along)/dt = -speed/frequency`), so bright energy bands propagate from the nucleus into the wake.

- Mesh orientation, projectile facing, deterministic `AuthoritativeTick` time, reduced/primitive behavior, and gameplay/server state are unchanged.
- The shader includes an explicit UV/direction comment to prevent a future sign reversal.
- Runtime paused captures at separated ticks loaded successfully in `target/brl-0104-comet-direction-tick-0/` and `target/brl-0104-comet-direction-tick-6/`; the live inspection path remains `just vfx-lab '--fixture straight --accent neutral'`.
- Focused straight-projectile tests: 3 passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.

The reported backwards-pulse defect is implemented. Native user confirmation remains requested; BRL-0104 stays `doing`.

## Phase 2 neutral straight muzzle/impact slice — 2026-09-01

The user instructed “commit and continue” after the neutral comet motion correction. Treat this as acceptance of the retained Pulse/Scatter comet direction and proceed to the previously ordered transient integration slice.

Current inspection shows neutral `combat.muzzle` and direct `combat.impact` still resolve to one generic fixed-size yellow/orange sphere each. Implement carrier-specific authored recipes through typed style mappings rather than changing the global neutral fallbacks or branching on Pulse/Scatter preset IDs:

- Match only Neutral + StraightProjectile + Activation for the muzzle recipe and Neutral + StraightProjectile + Impact for the direct-impact recipe. Lobbed explosions and other neutral carriers must remain unchanged.
- Reuse one cool-white/cyan energy language: compressed core/shell launch layers for muzzle, then core/shell/ground-wave dissipation for impact.
- Add bounded deterministic neutral energy-spark particle bursts to normal recipes; reduced recipes retain mesh identity without particles.
- Preserve current request/signature/gameplay/protocol contracts. BRL-0107 still owns observer-relative ownership color integration, so this slice must not add a competing ownership overlay or relationship branch.
- Use the existing read-only transient VfxLab fixture for gameplay-scale normal/reduced inspection before accepting the slice.

No gameplay, collision, damage, delivery, server, or protocol changes are in scope. BRL-0104 remains `doing`.

## Phase 2 neutral straight transient implementation evidence — 2026-09-01

Implemented typed Neutral + StraightProjectile recipes for `combat.muzzle` Activation and `combat.impact` Impact. Normal launch uses compressed bright/soft energy layers, a short ground echo, and a bounded deterministic cyan spark burst; normal impact stages a bright core, soft shell, expanding ground wave, and a larger bounded spark burst. Reduced recipes retain mesh identity and omit particles. Generic point, elemental, and lobbed recipes remain unchanged.

Added `NeutralBright`/`NeutralSoft` material keys and one reusable `NeutralSparks` GPU-particle profile. VfxLab now authors the same StraightProjectile phase signatures for neutral muzzle/impact previews, flanks transient effects with fighter-size references instead of occluding the effect, and describes the neutral energy language accurately. No gameplay, authority, protocol, collision, or ownership-color behavior changed.

Verification passed:
- `cargo test --features client client::vfx::catalog::tests --lib`: 11/11.
- `cargo test --features client client::vfx --lib`: 44/44.
- `cargo check --locked --no-default-features --features client --all-targets`.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings`.
- `cargo test --locked --no-default-features --features client --all-targets`: 606 library tests plus the client binary targets passed.
- Read-only VfxLab sampled the muzzle at tick 2 and impact at ticks 4 and 8, with paired PNG/JSON evidence under `target/brl-0104-neutral-muzzle-clear`, `target/brl-0104-neutral-impact-clear-t4`, and `target/brl-0104-neutral-impact-clear-t8`. The samples confirmed an unobstructed gameplay-scale launch flash, shell/ground-wave impact timing, and lingering deterministic spark dissipation.

BRL-0104 remains `doing`; native gameplay feedback for this slice and the remaining ordered closeout work are still pending.
