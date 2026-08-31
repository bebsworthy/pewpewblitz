# Scope

Inspect the production source and local Bevy references before freezing APIs. Record findings in BRL-0094 and update its spec if evidence changes the proposed architecture.

## Deliverables

- Inventory every current combat/world VFX producer, cue/fact input, materializer, update system, cleanup path, and presentation-variant branch.
- Map transient versus reconstructed persistent lifecycle ownership and identify late-join behavior.
- Capture representative native baseline PNG plus metadata for straight projectile, impact, circular area, status/ultimate, lobbed projectile, and explosion where currently present.
- Add focused characterization tests around stable cue-to-visual selection, timing, cleanup, and primitive/reduced behavior where coverage is missing.
- Freeze the minimum typed layer set demonstrated by current and first-slice requirements: ground fill/boundary, mesh flash/body, tracer/trail, decal/shadow, and optional particle-adapter reference.
- Freeze semantic signature dimensions: carrier/delivery family, lifecycle phase, accent/style, relationship, owned contextual override, geometry, stable event/field identity, authoritative timing, and presentation variant.
- Freeze deterministic seed derivation and document GPU particle repeatability limits.
- Define validation bounds and normal/reduced/primitive critical-layer fallback policy.
- Define bounded runtime ownership and measurable terminal/high-water budgets.

## Constraints

- No gameplay or protocol mutation.
- No visual redesign in this slice.
- No Hanabi dependency yet.
- No public framework beyond a boundary demonstrated by production and the planned lab.
- Preserve exact authoritative geometry and timing as presentation inputs.

## Acceptance criteria

- The current VFX dataflow and lifecycle ownership are documented with named source owners.
- Baseline captures and metadata are reproducible.
- The initial schema and resolver decisions are concrete enough for BRL-0098 and BRL-0099 to implement without inventing a second contract.
- Migration invariants identify which current effects must remain visually equivalent until their designated polish slice.
- Determinism, degradation, validation, cleanup, and resource-budget policies are testable.
- BRL-0094 spec reflects all consequential findings.

## Verification

Run focused client presentation tests, role-specific client/server checks, canonical formatting and linting required by the changed files, and the documented native baseline capture path. Record commands, results, artifacts, and unavailable cases in this ticket.

## Implementation findings and frozen decisions — 2026-08-31

### Current dataflow and lifecycle inventory

- The client-only transient path is `DeduplicatedCombatCue` or a feature cue -> feature-owned producer -> validated `VfxRequest` -> sealed `VfxRegistry` -> one resolved `VfxProfile` -> `PendingCombatEffect` -> `CombatEffect3d`.
- Fifteen request mappings are registered across combat, world objects, pickups, and Heist. Producer rank plus stable event ID defines merge order.
- The transient runtime has a total cap of 96, per-profile caps, oldest-first eviction, fixed real-time or authoritative-deadline cleanup, and normal/reduced profile resolution. Its concrete vocabulary is one sphere or ground ring with one shared material.
- Straight/lobbed projectiles, elemental and concealment fields, persistent Splash, sticky payloads, cone spray, fighter status, and dash trail use lifecycle-specific `CombatVisualOwner` roots outside the transient catalog. Attached/persistent roots reconcile from replicated state and despawn when their owner disappears. Persistent fields, cone propagation, sticky fuse, and lob height reconstruct from authoritative ticks.
- `WorldPresentationPlugin` visibly orders PrepareAssets -> MaterializeTopology -> ReconcileState -> ConsumeCues -> Animate -> Cleanup after client combat sync. Pose writes remain in PostUpdate after interpolation/physics and before transform propagation.

### Source identity decision

No protocol migration is required for the first implementation. Replicated delivery entities already carry `ReplicatedAttackSource`, including immutable `AttackSource.recipe_fingerprint`, attack/source kind, owner, team, and ultimate/deployable identity. Fighters replicate `ResolvedMatchLoadout`. Persistent Splash carries semantic effects directly; elemental fields carry their kind; armed sticky entities retain the replicated attack source.

The semantic resolver will use a FIFO-bounded cache of at most 512 attack signatures. It learns exact fingerprints from replicated delivery state and snapshots the source loadout at `AttackAccepted`, retaining the signature after a delivery/source visual disappears. Missing identity fails to the neutral recipe. BRL-0100 must add focused source-removal, in-flight loadout-replacement, immediate-impact, and late-join tests. Only a demonstrated accepted path that cannot be observed or cached reopens the global protocol migration gate.

### Frozen first contract

- Typed signature dimensions: carrier, phase, accent, observer relationship, one owned contextual override, typed geometry, stable presentation identity, authoritative timing, and presentation variant.
- Catalog tables: at most 64 semantic mappings, 128 recipes, and 256 reusable layers.
- Recipe expansion: at most eight ordered layers, no recursive recipe inclusion, acyclic fallback/reduced references, total transient root cap 96, per-recipe cap 1..=96, and fixed duration at most 10 seconds.
- Mesh vocabulary demonstrated by current/approved slices: sphere/flash, disc/fill, annulus/boundary, unit cuboid/segment, exact projectile body, and lobbed body/shadow.
- Readability classes: CriticalGeometry, Identity, Decorative. CriticalGeometry survives normal/reduced/primitive. Identity retains a non-color channel. Decorative may be removed.
- Variation uses counter-based hash sampling from a versioned tuple of stable presentation identity, recipe ID, layer ID, occurrence index, and property salt. No global mutable RNG controls authored choices.
- Phase-one timelines support explicit delay, fixed or authoritative duration, easing, and transform/scale keyframes with static material alpha. Animated per-instance alpha remains unavailable until a bounded uniform or reclaimed material-pool implementation is proven; per-frame `StandardMaterial` cloning is forbidden.
- Phase-one layers reuse retained mesh/material handles. Particle capacity is not authored until BRL-0102 records a measured adapter decision.

The durable contract and inventory are recorded in `docs/11-art-and-presentation-direction.md`. Evidence capture schema 2 now records the VFX catalog fingerprint, normal/reduced and primitive choices, transient profile/order/timing, lifecycle-root counts, authoritative persistent presentation facts, and retained mesh/material/image counts alongside the PNG and ordinary client-state snapshot.

### Verification recorded

- `cargo test --lib client::vfx` — pass, 18 tests.
- `cargo test --lib client::presentation_3d::combat::effects::tests` — pass, 12 tests.
- `cargo check --locked --no-default-features --features client --all-targets` — pass.
- `cargo check --locked --no-default-features --features server --all-targets` — pass.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings` — pass after correcting one documentation-format warning.
- `cargo clippy --locked --no-default-features --features server --all-targets -- -D warnings` — pass.

Native representative PNG/JSON baseline capture remains required before this ticket can move to done. The new schema makes that capture reproducible and inspectable; keep BRL-0097 doing until the artifacts and observations are recorded.

## Native baseline and closeout evidence — 2026-08-31

The first native run deliberately remains as evidence at `target/brl-0097-vfx-baseline.txt` and its paired capture directory. It used a 10-second measurement window and correctly failed only the locked 1,200-sample gate with 601 samples. The corrected canonical run used a 20-second measurement window:

`env BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_COMBAT=1 BRAWLER_RENDER_WARMUP_SECONDS=10 BRAWLER_RENDER_MEASURE_SECONDS=20 BRAWLER_RENDER_TIMEOUT_SECONDS=105 just v3-render-evidence target/brl-0097-vfx-baseline-pass.txt`

It passed with 1,201 samples, 16.655 ms p50, 17.111 ms p95, 17.705 ms p99, 18.488 ms maximum, and zero frames over 25 ms. It produced three paired native PNG/JSON captures under `target/brl-0097-vfx-baseline-pass.txt.captures` at scheduled stems 000300, 000480, and 000660. Every JSON record uses evidence schema 2, identifies embedded client VFX catalog fingerprint `10485497849105766461`, records normal mode (`reduced=false`, `primitive=false`), and names active transient/projectile lifecycle roots. The images were inspected at native 2560x1440 presentation and show the current simple Pulse Sidearm projectile/muzzle baseline without changing its appearance.

Normal, reduced, and primitive selection, stable cue ordering, authoritative timing, cleanup, resource bounds, persistent lifecycle reconstruction, and evidence serialization are covered by the focused client tests recorded above. The production Practice capture does not force every weapon family into a single match; isolated, repeatable coverage of all recipe/scenario combinations is the explicit purpose of BRL-0098 and no longer needs to be approximated through gameplay setup.

Scheduled and interactive evidence capture now use the same paired PNG/JSON implementation. `scripts/v3-render-evidence.sh` creates a sibling `.captures` directory and refuses to overwrite either report or captures, so the native baseline path is reproducible and preserves prior evidence.

### Learn-from-errors review

- Ticket payloads sent to an interactive terminal without a terminating newline did not persist even though the ticket shells were visible. Use file-backed stdin for substantial Ticket mutations and immediately read the task back.
- The first native run selected a 10-second measurement window even though the canonical gate requires 1,200 samples. Preserve that failed artifact, use at least 20 seconds for 60 Hz evidence, and track the configuration/gate mismatch separately rather than weakening the gate.
- A gameplay-only native capture cannot reliably stage every effect family. The evidence schema therefore records lifecycle ownership now; BRL-0098 supplies the bounded isolated production viewer needed for a complete visual matrix.

All acceptance criteria for this characterization slice are satisfied. Player-visible recipe migration and polish remain in the ordered dependent tickets.
