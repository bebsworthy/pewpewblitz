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

## Phase 2 next slice — Sticky Blomb / Big Blob retained presentation — 2026-09-01

Inventory after commit 5995974 identifies the armed sticky payload as the next largest player-visible neutral gap: production currently materializes one static purple sphere, an exact future-explosion ring, and a flat fuse-progress disc; VfxLab has no sticky lifecycle fixture.

Implement one retained sticky presentation shared by primary Sticky Blomb and the Big Blob ultimate secondary through the existing StickyPayload carrier and context, without built-in preset branches:
- Preserve the exact replicated position, explosion radius, armed/detonation timing, ring boundary, and gameplay/server/protocol ownership.
- Add a bounded fuse-driven viscous surface for the blob body in normal quality, with a readable dimensional silhouette and deterministic authored motion.
- Reduced keeps a lower-cost recognizable body and exact warning geometry; Primitive/ReducedPrimitive keep stable mesh fallbacks.
- Keep fuse timing driven by AuthoritativeTick; no wall-clock randomness or gameplay coupling.
- Add a read-only sticky VfxLab fixture that spawns the real StickyBlobState and invokes the production reconcile/update/pose systems. The existing U toggle selects the Big Blob ultimate-secondary context.
- Verify production and VfxLab install the same material animation/update path and add focused lifecycle/selection regression coverage.

No detonation gameplay, damage, collision, authority, protocol, ownership-color, or editable-workbench changes are in scope. BRL-0107 retains ownership-color integration. BRL-0104 remains doing.

## Phase 2 Sticky Blomb / Big Blob implementation evidence — 2026-09-01

Implemented the retained sticky-payload slice through the existing StickyPayload signature and production lifecycle owner.

- Normal and Reduced add a deterministic procedural viscous shell with fuse-driven emissive veins and orbiting blob droplets; Normal uses four droplets and full detail, Reduced uses two droplets and a lower-cost shader path.
- Primitive and ReducedPrimitive retain the stable original sphere body. All variants preserve the exact replicated delayed-blast ring and authoritative fuse progression.
- Split the warning presentation into a bright exact boundary and a restrained translucent progress fill after the first native pass exposed the old broad opaque plate as a map-readability problem.
- Each live sticky owns one custom material asset. Reconciliation removes it with the owner, avoiding retained asset growth.
- Production and VfxLab install the same StickyBlobMaterial plugin, reconcile system, AuthoritativeTick animation/update path, and pose writer.
- Added the read-only sticky VfxLab lifecycle fixture. U switches between the primary Sticky Blomb body and the smaller Big Blob ultimate-secondary context; normal, reduced, primitive, and ultimate captures all use production materialization.

Native paired-capture evidence:
- target/brl-0104-sticky-v2-normal/
- target/brl-0104-sticky-v2-reduced/
- target/brl-0104-sticky-v2-primitive/
- target/brl-0104-sticky-v2-ultimate/

Verification passed:
- Focused combat presentation tests: 50/50.
- cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings.
- cargo test --locked --no-default-features --features client --all-targets: 608 library tests plus client binary targets passed.
- just _check-server and just _server-features passed; the server graph excludes the new client presentation capability.
- cargo fmt --all and git diff --check passed.

The retained sticky implementation is complete. Native user visual feedback remains open, and the remaining Phase 2 families remain in scope; BRL-0104 stays doing.

## Phase 2 next slice — Dash retained presentation — 2026-09-01

Production inspection identifies Dash as the next uncompleted neutral carrier family. Its current client presentation is one translucent cuboid stretched only between the fighter's previous and current rendered positions; it has no accumulated wake, authored motion language, controlled linger, or isolated VfxLab lifecycle fixture.

Implement one production-owned Dash wake through the existing AttachedStatus/Travel signature and direct fighter-to-trail lifecycle:

- preserve the fighter's replicated `AbilityState::Dashing`, position, exact movement endpoint, authority, collision, and damage behavior;
- replace the normal/reduced cuboid with a deterministic planar procedural wake that grows from the authoritative dash origin to the current fighter position, tapers behind the fighter, and layers a hot core, moving filaments, edge distortion, and sparse speed cuts without obscuring the fighter;
- drive animation and a short post-dash dissipation from `AuthoritativeTick`; use no wall-clock randomness and allocate no unbounded particles or trail segments;
- keep Primitive/ReducedPrimitive as the stable cheap mesh fallback; Reduced retains the silhouette with lower shader detail;
- add a read-only `--fixture dash` scenario which constructs replicated-shape fighter/loadout/ability facts and moves them over the built-in Dash duration while production reconciliation/material animation owns the visible effect;
- remove per-instance procedural material assets when their trail owner expires or disappears, and cover lifecycle, variant selection, deterministic timing, production/VfxLab scheduling, and bounded cleanup with focused tests.

No server, gameplay, protocol, ownership-color, map-preview, or editable-workbench changes are in scope. BRL-0107 retains ownership-color integration and BRL-0108 retains production-map VfxLab preview.

## Phase 2 Dash implementation evidence — 2026-09-01

Implemented the production-owned Dash wake through the existing AttachedStatus/Travel signature and fighter-to-trail lifecycle.

- Normal uses one bounded procedural planar wake accumulated from the observed Dash origin to the current fighter position. The shader layers a hot central lane, deterministic turbulent filaments, tapered edges, and sparse speed cuts.
- Reduced uses the same silhouette and authoritative lifecycle with narrower geometry and the expensive filament/cut branch disabled. Primitive and ReducedPrimitive keep the original simple translucent cuboid fallback.
- The built-in 18-tick Dash duration remains authoritative. A 15-tick presentation-only dissipation window is driven entirely by AuthoritativeTick; the endpoint geometry is finalized even when the first observed post-sample state is already Charging.
- Each procedural Dash owns one material asset. Normal completion and orphan cleanup remove that asset; focused ECS coverage verifies release to zero.
- VfxLab now supports --fixture dash. It constructs the real resolved Dash loadout, AbilityState, fighter identity, position, and tick facts, moves over the built-in duration, and invokes the same production reconciler/material plugin. Retained sample jumps now materialize the activation facts once before advancing, preserving origin-relative visuals.
- Evidence snapshots now report dash_trail_materials alongside the retained root count.

Verification:

- cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings passed.
- cargo test --locked --no-default-features --features client --all-targets passed: 612 library tests, client/VfxLab targets, and map public API.
- just _check-server and just _server-features passed; the server graph excludes client presentation capabilities.
- cargo fmt --all -- --check and git diff --check passed.
- Native normal/reduced/primitive/dissipation captures are under target/brl-0104-dash-v3-normal, target/brl-0104-dash-v3-reduced, target/brl-0104-dash-v3-primitive, and target/brl-0104-dash-v4-dissipating. Normal shows the animated layered wake; reduced preserves its tapered identity; primitive remains the stable cheap fallback; the post-dash sample confirms bounded linger.

The Dash implementation slice is complete. Native user visual feedback and the remaining Phase 2 families remain open; BRL-0104 stays doing.

## Phase 2 next slice — dual-purpose Persistent Splash presentation — 2026-09-01

Production inspection confirms the built-in Splash area carries Damage plus Heal as the ordered semantic accents Restoration plus Neutral. The generic elemental-area path selects the first element, so the weapon currently looks like a Restoration ultimate field and loses its mixed hostile/support identity. VfxLab has no PersistentSplash lifecycle fixture.

Implement one composable dual-purpose PersistentArea style selected from carrier plus accent semantics, never from the built-in Splash preset ID:

- Normal uses a deterministic animated fluid surface with restrained center coverage, traveling ripples/caustics, a readable exact boundary, and bounded fluid droplets that communicate both restorative teal and neutral impact energy.
- Reduced preserves the fluid silhouette and dual-color identity with lower shader/accent detail. Primitive and ReducedPrimitive use a stable disc/ring mesh fallback with no custom shader animation.
- AuthoritativeTick drives all surface and droplet motion. The replicated PersistentSplashState remains the only area geometry, pulse, and expiry authority.
- Add --fixture splash to VfxLab using a real circle PersistentSplashState with the production Damage plus Heal effects and the production reconciler/material plugins.
- Verify semantic selection, variants, production/VfxLab scheduling, root cleanup, and bounded asset/entity counts.

No gameplay, server, payload, ownership-color, map-preview, protocol, or editable-workbench changes are in scope. Rectangle support remains on the existing generic fallback because no authored current weapon demonstrates that shape.

### Phase 2 Splash implementation evidence — 2026-09-01

- Implemented a semantic dual-purpose persistent Splash style selected from the existing `PersistentArea` carrier plus Restoration and Neutral accents; no built-in preset-ID branch was added.
- Normal presentation uses an AuthoritativeTick-driven fluid material with traveling ripples and caustics, exact cyan gameplay boundary, restorative inner motion, and a bounded deterministic droplet set. Reduced removes caustic detail and lowers droplet count; primitive remains a stable disc-and-ring fallback.
- Production and VfxLab share `reconcile_persistent_splash_visuals`, the Splash material plugin, retained animation systems, and lifecycle cleanup. The VfxLab `--fixture splash` creates the real Damage + Heal PersistentSplash state.
- Focused verification: `cargo test --locked --no-default-features --features client client::presentation_3d::combat --lib` — 56 passed.
- Canonical client verification: `cargo test --locked --no-default-features --features client --lib` — 614 passed.
- Strict client lint: `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings` — passed.
- Server isolation: `cargo check --locked --no-default-features --features server --all-targets` and strict server Clippy — passed.
- Native evidence captured at playback tick 60 for normal, reduced, and primitive at 1280x720. Each capture reports one persistent Splash root, one visual root, two bounded shared Splash materials, zero particle emitters, and the authored Damage(36 Hostiles) + Heal(24 AlliesAndOwner) facts. Visual inspection confirmed readable fighter silhouettes, exact outer boundary, distinct degradation tiers, and no ownership overlay.
- Feedback gate remains open for subjective native review; BRL-0104 stays doing.

## Phase 2 next slice — retained Sentry presentation — 2026-09-01

- Scope is the existing replicated Sentry deployable lifecycle and an isolated `--fixture sentry`; no authoritative targeting, cadence, projectile, protocol, ownership-language, or Balance Lab changes.
- Selection remains semantic: active `Deployable` + Neutral signature with deployable context. No ultimate-definition-ID branch is permitted.
- Normal presentation will replace the three primitive cylinders/cuboid with a layered readable turret silhouette: grounded plinth, dimensional armored body, aim-aligned barrel, emissive power core, and bounded rotating energy/gimbal accents. Animation derives only from `AuthoritativeTick`; aim continues to derive from replicated `Rotation`.
- Reduced presentation keeps the dimensional silhouette, exact aim direction, and core pulse with fewer motion accents. Primitive preserves the current base/body/direction fallback.
- VfxLab will construct the real replicated `Sentry`, `SentryIdentity`, `SentryDeadline`, Position, and Rotation facts and run the production reconcile, pose, animation, and cleanup systems.
- Verification will cover semantic style selection, deterministic bounded accents, production/VfxLab scheduling, lifecycle cleanup, normal/reduced/primitive native captures, strict client checks, and server feature isolation.
- Sentry fire/removal transients remain part of the later Phase 2 cue sweep so the retained slice stays independently reviewable.

### Phase 2 retained Sentry implementation evidence — 2026-09-01

- Implemented semantic active Deployable + Neutral selection with no ultimate-definition-ID branch. Normal/reduced use the new retained Sentry style; primitive/reduced-primitive preserve the existing three-mesh fallback.
- The production root still follows replicated Position and Rotation. New animation is presentation-only and AuthoritativeTick-driven: a bounded power-core pulse and four deterministic orbit nodes in normal, one node in reduced.
- The first native capture was rejected during implementation because the bright existing materials flattened the chassis into a toy-like cyan/white primitive stack. It was corrected with dark metallic plinth/body materials, restrained team armor panels, a smaller hot core, vertical gimbal, and emissive muzzle rail.
- Added `--fixture sentry`, backed by real Sentry, SentryIdentity, SentryDeadline, TeamId, Position, Rotation, and AuthoritativeTick facts. The Lab runs production reconcile, pose, animation, and cleanup systems.
- Focused verification: 57 combat-presentation tests and 6 VfxLab tests passed. Strict client all-target Clippy passed.
- Canonical client verification: 615 tests passed. Server all-target check and strict server Clippy passed.
- Native normal/reduced/primitive captures at playback tick 60 each report exactly one Sentry visual root, the Deployable semantic signature, zero particle emitters, and bounded shared assets. Visual inspection confirmed aim direction, machine/energy contrast, readable fighter-scale silhouette, and distinct primitive degradation.
- User visual feedback remains open. Sentry fire/removal transients and remaining Phase 2 families remain in scope; BRL-0104 stays doing.

## Phase 2 next slice — Impact Blade melee sweep — 2026-09-01

- Scope is the existing Impact Blade `MeleeArc` delivery: activation sweep plus contact impact, production/VfxLab parity, and normal/reduced/primitive degradation. No gameplay damage, hit selection, protocol, fighter animation, ownership-language, or Balance Lab changes.
- The semantic signature cache will retain the authored melee reach and angle. At `AttackAccepted`, the client combines those immutable recipe facts with the replicated source fighter Rotation and cue origin to emit exact `Melee` cone geometry; `MeleeContact` remains a point impact.
- Add a reusable catalog renderer family for melee sweep geometry. It consumes `VfxGeometry::Cone`, uses a bounded cache of quantized authored arc meshes, and scales/orients from the signature rather than recognizing Impact Blade or a preset ID.
- Normal uses a layered fast energy blade sweep with a bright leading edge, softer body/afterimage, and bounded contact sparks. Reduced keeps exact sweep silhouette plus compact contact. Primitive keeps the exact ordinary mesh silhouette with particles suppressed by the existing variant policy.
- Add `--fixture melee`, staging production `combat.muzzle` and `combat.impact` requests with the built-in 120-unit/100-degree authored geometry and fighter-size references.
- Verification will cover cache-derived geometry, registry validation, bounded mesh caching, recipe selection, VfxLab cycling/evidence, strict client checks, server isolation, and native normal/reduced/primitive captures.

## Phase 2 Impact Blade melee sweep implementation evidence — 2026-09-01

Implemented the Impact Blade activation/contact slice without gameplay, protocol, fighter-animation, ownership-language, or Balance Lab changes.

- `AttackAccepted` now combines the cached authored `MeleeArc` reach/angle with the replicated source fighter rotation and cue origin to emit `VfxGeometry::Cone`; non-melee and missing-style cases retain the point fallback.
- The production catalog selects neutral Melee activation/contact recipes by semantic carrier and phase. Normal activation is a layered cool-white/cyan tapered cutting crescent with a delayed afterimage; contact is a compact flash, ground ring, and deterministic spark burst. Reduced retains two essential mesh layers with no particles. Primitive retains the deterministic crescent/contact silhouettes with particle materialization suppressed by the existing variant contract.
- `MeleeArc` is a reusable catalog renderer driven by `GeometryReach`, not an Impact Blade or preset-ID branch. Catalog validation reserves geometry-reach scaling for the melee renderer and registry resolution fails closed to the recipe fallback when cone geometry is absent.
- Authored angles use a tenth-degree, maximum-64-entry mesh cache. The generated 48-segment annular crescent stays within the authored angular envelope, tapers to sharp tips, and reaches the authored range at its cutting crest.
- VfxLab now supports `--fixture melee`, explains the 120-unit/100-degree sweep and separate contact event, and emits the same production registry requests used by gameplay.

Verification:

- `cargo fmt --all -- --check` — pass.
- `cargo test --features client --lib client::vfx` — 46 passed.
- `cargo test --features client --lib client::presentation_3d::combat` — 60 passed.
- `cargo test --features client --lib client::vfx_lab` — 6 passed.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings` — pass.
- `just _test-client` — canonical client suite passed: 620 library tests plus all client targets.
- `just _server-features` — pass; server graph remains presentation-free.
- `just _v3-world-presentation` — pass; no retired renderer contract reintroduced.
- VfxLab normal/reduced/primitive captures completed and were visually reviewed under `target/brl-0104-melee-v2-normal-mid`, `target/brl-0104-melee-v2-reduced-mid`, and `target/brl-0104-melee-v2-primitive`. The first filled-sector attempt was rejected during self-review as a cheap targeting wedge; the retained version uses a layered tapered blade silhouette with an independently readable contact event.

The slice is ready for native gameplay feedback. BRL-0104 remains `doing`; later Phase 2 families and closeout criteria remain open.

## Phase 2 next slice — Demolition Strike breach VFX — 2026-09-01

Baseline capture `target/brl-0104-demolition-baseline` confirms the immediate 64-unit map-destruction ultimate is still represented by one short-lived uniformly scaled orange sphere; at the standard automatic sample it has already vanished, and VfxLab labels it as a neutral generic one-shot.

Implement one independently reviewable Demolition Strike activation slice through the existing `ability.demolition-strike` request and `Demolition` semantic signature:

- Preserve the cue center and exact authored destruction radius as the critical ground boundary. Do not change the immediate server-authoritative `DestroyMap` world effect, activation, targeting, cue, protocol, or balance values.
- Replace the sphere recipe with a staged breach: exact radius boundary, fast ground fracture/flash, compact hot vertical core, expanding shockwave, bounded outward sparks/debris, and trailing smoke. Reuse existing typed mesh and GPU-particle primitives; do not add a Demolition-specific renderer branch.
- Normal uses the full mesh/particle composition. Reduced retains the exact boundary, fracture motif, and compact flash with no particles. Primitive uses the same ordinary mesh recipe while the existing variant policy suppresses particles.
- Make VfxLab `--effect ability.demolition-strike --radius 64` emit the production Demolition accent/context and explain that the visual accompanies an already-resolved authoritative map-destruction event rather than a pending telegraph.
- Verify catalog bounds/fallback, semantic production/VfxLab parity, normal/reduced/primitive captures, focused tests, strict client checks, and server feature isolation.

Ownership-language integration remains BRL-0107; production-map VfxLab preview remains BRL-0108. BRL-0104 stays `doing`.

## Phase 2 Demolition Strike implementation evidence — 2026-09-01

Implemented the Demolition Strike activation slice through the existing production `ability.demolition-strike` request. Gameplay remains an immediate server-authoritative `DestroyMap` world effect at the authored center/radius; no targeting, cue, protocol, map mutation, or balance behavior changed.

- Replaced the single orange sphere with a six-layer normal recipe: exact 64-unit thin boundary, radial ground fracture, short ground flash, white-hot flash, orange core, and expanding shockwave. Existing bounded explosion sparks and smoke supply the two decorative GPU-particle layers.
- Reduced uses three mesh-only layers (exact boundary, fracture, compact flash). Primitive uses the normal ordinary-mesh composition while the existing variant contract suppresses particle materialization.
- VfxLab now freezes the request to the production `Demolition + PersistentArea + Activation + DemolitionStrike` signature, regardless of the general elemental accent control. Its guide identifies the exact destruction radius and states that this is presentation for an already-resolved authoritative event, not a pending telegraph.
- Baseline `target/brl-0104-demolition-baseline` was empty at the normal automatic sample because the prior uniform sphere had already despawned. Retained visual evidence is under `target/brl-0104-demolition-v2-normal`, `target/brl-0104-demolition-v2-reduced`, `target/brl-0104-demolition-v2-primitive`, and `target/brl-0104-demolition-v2-late`. Visual review confirmed the immediate boundary/flash beat, the later fracture/shockwave/smoke beat, deterministic reduced identity, and particle-free primitive behavior.

Verification:

- `cargo test --features client --lib client::vfx` — 48 passed.
- `cargo test --features client --lib client::vfx_lab` — 7 passed.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings` — pass.
- `just _test-client` — canonical client suite passed: 622 library tests plus all client targets.
- `just _server-features` and `just _v3-world-presentation` — pass.
- `cargo fmt --all` and `git diff --check` — pass.

The slice is ready for native gameplay feedback. Reveal/concealment, Sentry fire/removal, remaining status/expiry cues, the complete overlap/resource matrix, and closeout remain open; BRL-0104 stays `doing`.

## Phase 2 next slice — Reveal Scan activation and retained deadline — 2026-09-01

Baseline `target/brl-0104-reveal-baseline` confirms Reveal Scan is one static opaque magenta ring at the full 192-unit radius for the entire authoritative reveal deadline. VfxLab mislabels it as a neutral generic one-shot. Gameplay itself performs one instantaneous area scan, then retains forced-reveal deadlines on the affected fighters; attached Reveal status presentation already owns those target markers.

Implement a layered scan recipe through the existing `ability.reveal-scan` request:

- Preserve the exact authored radius and authoritative deadline. The exact outer boundary may remain as a restrained deadline-owned reference, while short fixed-lifetime concentric rings and a compact center flash communicate the activation sweep. Do not imply repeated scanning, damage, or a persistent gameplay field.
- Use existing generic ring/sphere renderers; no Reveal-specific renderer or gameplay branch. Normal uses layered magenta scan waves. Reduced keeps the exact boundary and one activation wave. Primitive uses the same deterministic meshes and remains particle-free.
- VfxLab `--effect ability.reveal-scan --radius 192` must freeze the production `Reveal + PersistentArea + Activation + RevealScan` signature, label the effect correctly, and explain the split between the area activation and attached target reveal markers.
- No reveal rules, observer visibility, concealment ownership, cue/protocol, status lifecycle, balance, BRL-0107 ownership language, or BRL-0108 map-preview work is in scope.
- Verify recipe/deadline requirements, production/VfxLab semantic parity, normal/reduced/primitive captures, focused tests, strict client checks, and server isolation.

## Phase 2 Reveal Scan implementation evidence — 2026-09-01

Implemented the Reveal Scan activation slice through the existing production `ability.reveal-scan` request without changing reveal authority, visibility, cues, protocol, balance, or target-marker ownership.

- Replaced the single static opaque ring with a four-layer normal recipe: a thin exact 192-unit deadline-owned boundary, a fast full-radius acquisition wave, a delayed thinner echo, and a compact planar origin reticle. Only the exact boundary uses `AuthoritativeDeadline`; every activation layer has a fixed lifetime so the presentation cannot imply repeated scanning.
- Reduced retains the exact deadline-owned boundary plus one activation wave. Primitive uses the same deterministic ordinary meshes and remains particle-free through the existing variant contract.
- All layers use existing generic ground-ring/spark renderers; no Reveal-specific renderer, gameplay branch, or custom material was added.
- VfxLab now freezes `--effect ability.reveal-scan --radius 192` to the production `Reveal + PersistentArea + Activation + RevealScan` signature. Its guide explicitly distinguishes the one-time area scan from the separately attached Reveal markers on affected fighters.
- Baseline `target/brl-0104-reveal-baseline` showed the former single static full-radius ring. Reviewed normal/late/reduced/primitive paired evidence is under `target/brl-0104-reveal-v3-normal`, `target/brl-0104-reveal-v3-late`, `target/brl-0104-reveal-v3-reduced`, and `target/brl-0104-reveal-v3-primitive`. The first origin-sphere revision was rejected during self-review as a flat magenta puck and replaced with the bounded ground reticle.

Verification:

- Focused VFX catalog/registry tests: 42 passed.
- Focused VfxLab tests: 8 passed.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings` — pass.
- `just _test-client` — canonical client suite passed: 624 library tests plus all client targets.
- `just _server-features` — pass; server graph remains presentation-free.
- `just _v3-world-presentation` — pass; no retired renderer contract was reintroduced.
- `cargo fmt --all -- --check` and `git diff --check` — pass.

The slice is ready for native gameplay feedback. Reveal target-marker polish, Sentry fire/removal, remaining status/expiry cues, the complete overlap/resource matrix, and closeout remain open; BRL-0104 stays `doing`.

## Phase 2 next slice — Sentry fire and removal transients — 2026-09-01

Production inspection confirms `CombatCue::SentryFired` already emits `combat.muzzle` with the semantic `Deployable + Activation + Neutral + Sentry` signature, while `CombatCue::DeployableRemoved` emits `combat.impact` as `Deployable + Expiry` for natural expiry or `Deployable + Interruption` for every other cleanup reason. Both currently miss an intentional style mapping and collapse to the generic one-layer muzzle/impact fallbacks.

Implement one independently reviewable transient completion for the existing Sentry lifecycle:

- Preserve the authoritative fire cadence, projectile emission, target selection, cue positions, cleanup-reason priority, replicated deployable lifecycle, protocol, and balance.
- Add semantic catalog mappings for Sentry fire, graceful expiry, and interrupted cleanup using carrier, phase, Neutral accent, and the existing ultimate/Sentry context; do not branch on the built-in ultimate ID.
- Fire should read as a compact charged muzzle discharge attached to the retained machine language, with a fast core, contained bloom, ground echo, and bounded directional-looking sparks. The replicated projectile remains the delivery visual; do not invent a hitscan beam.
- Natural expiry should read as an orderly energy shutdown/contraction. Interruption covers destroyed, replaced, owner/lifecycle, and match cleanup reasons, so it must use a compact electrical collapse rather than an explosion that would misrepresent every reason as damage.
- Normal uses layered meshes plus bounded existing particle primitives where appropriate. Reduced keeps the essential mesh timing and silhouettes with particles removed. Primitive uses the same deterministic ordinary meshes with particle suppression.
- Extend the existing read-only Sentry VfxLab fixture to stage the production semantic fire and cleanup requests alongside the retained turret for isolated inspection, with a clear guide explaining that these are cue presentations rather than authority.
- Verify signature routing for activation/expiry/interruption, fallback/degradation, VfxLab staging, focused tests, normal/reduced/primitive evidence, strict client checks, and server feature isolation.

No Sentry gameplay, protocol, ownership-color, map-preview, editable-workbench, or retained-turret redesign is in scope. BRL-0107 retains ownership-language integration and BRL-0108 retains production-map VfxLab preview.

## Phase 2 Sentry fire and removal implementation evidence — 2026-09-01

Implemented intentional Sentry fire, natural-expiry, and interrupted-cleanup transients through the existing production combat cues and semantic request boundary. No fire cadence, targeting, projectile delivery, cleanup priority, authority, protocol, or balance behavior changed.

- `Deployable + Activation + Neutral + Sentry` now selects a compact four-layer charge discharge with a short core/bloom, planar muzzle reticle, echo ring, and bounded existing neutral sparks. The replicated projectile remains the delivery visual; no beam or hitscan presentation was introduced.
- `Deployable + Expiry + Neutral + Sentry` selects a particle-free contracting ring/core/reticle shutdown. `Deployable + Interruption` selects a sharper electrical fracture/ring with bounded sparks. The interruption style is deliberately not a large explosion because that phase also covers replacement, owner loss, match cleanup, and restart.
- Normal, reduced, and primitive variants use the existing catalog degradation contract. Reduced recipes keep the essential mesh timing with no particles; primitive keeps deterministic ordinary meshes while suppressing particle materialization.
- Production now resolves the current replicated Sentry pose and offsets `SentryFired` presentation to the 47-unit aim-aligned muzzle rail. Missing retained state safely falls back to the authoritative cue center. A focused producer test locks the pose-derived attachment.
- The existing `--fixture sentry` VfxLab scenario stages the retained production turret, its fire request, and a separate cleanup request. U now toggles natural expiry versus interruption, and the guide explains each cue's non-authoritative presentation role.
- First-pass captures were refined after self-review: the initial discharge was too large and the interruption core read as a detached bubble. The final discharge was made compact and the cleanup core subordinated to the electrical fracture silhouette.

Verification:

- Focused VFX catalog/producer tests: 44 passed.
- Focused VfxLab tests: 8 passed.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings` — pass.
- `just _test-client` — canonical client suite passed: 626 library tests plus all client targets.
- `just _server-features` — pass; the server graph remains presentation-free.
- `just _v3-world-presentation` — pass.
- `cargo fmt --all -- --check` and `git diff --check` — pass.
- Final visual evidence: natural expiry and primitive under `target/brl-0104-sentry-cues-v2-expiry` and `target/brl-0104-sentry-cues-v2-primitive`; refined normal interruption and reduced under `target/brl-0104-sentry-cues-v3-interrupted` and `target/brl-0104-sentry-cues-v3-reduced`.

The Sentry transient slice is ready for native gameplay feedback. Remaining status/expiry cues, concealment coverage, the complete overlap/resource matrix, documentation reconciliation, and BRL-0094 closeout remain open; BRL-0104 stays `doing`.


## Phase 2 next slice — retained attached combat status language (2026-09-01)

Outcome: replace the current single static ring for combat-owned attached statuses with a bounded, deterministic layered visual language that remains readable around the fighter and is driven only by replicated authoritative state.

Scope and decisions:

- Cover Slow, Knockback, Reveal, Cold/Frozen, Poison, and Fire. Effect-tile occupancy remains on its existing simple marker path because the map tile is already the primary visual source.
- Continue deriving existence, semantic accent, timing, and identity from `ActiveEffects`, `KnockbackFeedback`, `ConcealmentPresentationState`, `AuthoritativeTick`, and the existing `CombatVisualSignature`; no gameplay mutation or weapon-ID branching enters presentation.
- Normal presentation may use a shared boundary plus a small status-specific motif. Reduced presentation keeps the boundary and at most one motif. Primitive presentation remains the current single-ring fallback.
- Animation is deterministic from authoritative tick plus stable semantic phase. It must be installed through the shared retained-animation composition used by both production gameplay and VfxLab, not through a Lab-only path.
- Keep all geometry bounded and fighter-readable: no opaque center fill, no per-frame spawning, no unbounded particles, and no more than eight render descendants per attached status in Normal quality.
- VfxLab's status fixture must describe and exercise the production status materializer rather than duplicating a showcase-only construction.

Acceptance and verification for this slice:

- Each covered combat status has a distinguishable silhouette or motion cue in Normal quality; Cold and Frozen are recognizably related but not identical.
- Status roots follow fighter pose, appear and disappear at the authoritative lifecycle boundary, and animate identically in production and VfxLab.
- Reduced and primitive variants satisfy their layer budgets and remain semantically readable.
- Focused ECS tests cover deterministic animation and bounded layer plans; strict client clippy, client tests, server-feature isolation, formatting, and native Normal/reduced/primitive VfxLab evidence pass before commit.


### Attached status slice implementation and evidence (2026-09-01)

Implemented the production retained-status renderer as a semantic layered style shared by gameplay and VfxLab:

- Slow: cyan boundary plus slowly orbiting drag motes.
- Knockback: orange boundary plus outward-pulsing impulse bars.
- Cold: cyan boundary plus orbiting vertical ice splinters.
- Frozen: related cyan boundary intensified into a six-shard locking crown with deterministic shiver.
- Poison: acidic boundary plus independently bobbing bubbles.
- Fire: hot boundary plus independently flickering flame tongues.
- Reveal: wider magenta detection boundary plus fast rotating sensor sparks.

All motion reads the owning fighter's replicated `AuthoritativeTick`; replaying the same tick produces the same transform. Normal is bounded to at most seven render layers, Reduced uses one boundary plus one motif, and Primitive/ReducedPrimitive use one static boundary. Effect-tile markers remain on their prior simple-ring path. The animation system is registered through `add_retained_material_animation_systems`, which is used by both `WorldPresentationPlugin` and `VfxLabLifecyclePresentationPlugin`.

VfxLab's status fixture now builds the same authoritative component shapes used by production. C cycles Slow/Knockback, Cold/Frozen, Fire, Poison, and Reveal families; U selects the alternate Slow↔Knockback and Cold↔Frozen states. The guide names and explains the actual status rather than labeling the old fixed Slow fixture as Neutral.

Verification:

- `cargo test --locked --no-default-features --features client status_style --lib`: 3 passed, including bounded degradation and same-tick deterministic ECS replay.
- `cargo test --features client vfx_lab --lib`: 8 passed.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings`: passed.
- `just _test-client`: 629 library tests plus all client targets passed.
- `./scripts/check-server-features.sh`: passed; server feature graph excludes client presentation capabilities.
- `./scripts/check-v3-world-presentation.sh`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- Native paired VfxLab captures reviewed: `target/brl-0104-status-v2-fire`, `target/brl-0104-status-v2-frozen`, `target/brl-0104-status-v2-reveal`, `target/brl-0104-status-v2-reduced`, and `target/brl-0104-status-v2-primitive`. No hierarchy warnings remained after adding visibility ownership to the layered root.
