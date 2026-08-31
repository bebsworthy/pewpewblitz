# Scope

- Implement the frozen typed catalog tables, recipes, reusable layers, semantic mappings, validation bounds, fallback graph, readability classes, deterministic counter-hash sampling, and phase-one timelines.
- Materialize up to eight ordered retained mesh/material layers per recipe under one bounded visual root.
- Migrate all current `VfxRequest`/`VfxProfile` transient effects to recipes with visual-equivalence fixtures; keep gameplay producers presentation-agnostic.
- Apply normal/reduced/primitive degradation, total/per-recipe caps, deterministic oldest-first eviction, authoritative deadline support, and leak-free cleanup.
- Make the same runtime directly consumable by VfxLab.

# Acceptance criteria

- Existing transient cues resolve through recipes with no gameplay/protocol change and no unintended presentation redesign.
- Invalid catalogs fail with precise diagnostics for every frozen bound and reference rule.
- Repeated spawn/evict/cleanup cycles stay within root and retained-resource budgets.
- Deterministic inputs select identical authored variants; layer order is stable.
- Client/server role checks and native equivalence evidence pass.

# Verification

Focused catalog/resolver/timeline/ECS tests, client/server checks and linting, production presentation guards, VfxLab replay evidence, and representative native before/after captures.

# Implemented design

- Upgraded the client-only catalog to schema 4 with bounded semantic mappings, recipes, and reusable mesh layers: 64 mappings, 128 recipes, 256 layers, eight layers per recipe, 96 active roots, 1-96 per-recipe concurrency, and a 10-second authored timeline ceiling.
- Preserved all 15 semantic request mappings and migrated the 29 normal/reduced profiles to visually equivalent one-layer recipes. No gameplay type, producer, protocol, authoritative geometry, or content fingerprint changed.
- Added typed readability, renderer/material, anchor, scale, shadow, delay, duration, easing, and three-point scale-keyframe policy. Per-instance animated alpha and particles remain outside this phase because retained/material-pool and particle-cap contracts are not yet demonstrated.
- Added strict duplicate, missing-reference, value, capacity, expansion, reduced-cycle, executable authoritative-fallback-cycle, producer coverage/capability, and critical-geometry degradation validation.
- Recipe resolution expands owned layers in authored order. One transient root owns all layer children; allocation and cleanup operate at root granularity with deterministic oldest-first total/per-recipe eviction.
- Added counter-based deterministic variation sampling from presentation identity, recipe ID, layer ID, occurrence index, and property salt. Adding a sampled property does not advance or perturb a global random stream.
- VfxLab uses the production resolver/materializer and reports the resolved recipe, ordered layers, readability, and deterministic sample in its inspector and paired JSON evidence.
- Updated the durable art/presentation contract for schema 4, layered root ownership, evidence, and degradation.

# Verification evidence

- `cargo test --locked --no-default-features --features client client::vfx`: 26 passed. Covers schema/value/reference/capacity validation, critical-geometry retention, timelines, deterministic property-local samples, registry coverage/capabilities, reduced resolution, and data-only extension.
- `cargo test --locked --no-default-features --features client client::presentation_3d::combat`: 31 passed. Covers authored layer order/delay/animation, retained primitive materialization, deterministic request ordering, total/per-recipe eviction, authoritative geometry/deadlines, reduced behavior, evidence, and root-plus-child cleanup.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings`: passed.
- `just _check-server`, `just _server-features`, and `just _v3-world-presentation`: passed; schema/runtime/VfxLab remain client-only and the production 3D ownership guard is intact.
- Native automated paired captures passed and exited cleanly under `target/brl-0099-vfx-lab-point`, `target/brl-0099-vfx-lab-explosion`, and `target/brl-0099-vfx-lab-reduced`. Captures used presentation identity 77 and covered normal `combat.damage`, normal `world-object.explosion`, and reduced+primitive `combat.muzzle`.
- Visual comparison against the BRL-0098 baseline found no unintended redesign: the red damage sphere, red explosion ring, and small yellow reduced muzzle sphere retain their accepted silhouettes. JSON records catalog fingerprint `12764622961921089492`, one recipe root, authored layer index 0, and stable samples `9198271320727395341`, `15490731113707698093`, and `1702556922641202391` respectively.

# Feedback disposition and learning review

No new subjective redesign feedback was required because this ticket is an equivalence migration; richer compositions remain ordered follow-on work. During implementation, a test initially treated Bevy entity-bit ordering as authored child order. Entity allocation is not an authoring contract, so each layer now carries an explicit bounded `layer_index`, evidence sorts by it, and the ECS test asserts that index. The reusable lesson is to encode presentation ordering as data at materialization time rather than infer it from process-local entity identity or query iteration.

# Final downstream check

- `cargo test --locked --no-default-features --features client --lib`: 571 passed. This broad run covers the client composition/downstream surface identified by the structural-risk review; the single logged invalid-map warning is expected negative-test evidence.
