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
