# Product decision

On 2026-09-01 the user clarified that tile identity must not be a privileged Rust enum. Authors must be able to create another tile with its own name, values, effect combination, and presentation without adding a built-in branch. The previous deferred trigger is superseded and BRL-0089 is active.

# Outcome

Effect-tile identities are stable data-authored definitions. A definition composes registered, independently valued effect-family definitions and remains independent from the map asset's client visual profile.

Adding a tile from registered families is content-only. Adding a genuinely new mechanic adds one focused typed family plugin/catalog and its owned runtime systems; it does not modify existing tile definitions, consumers, or a central Speed/Slow/Damage match.

# Authored model

Replace MapEffectTileBehavior with stable IDs and references:

- EffectTileDefinitionId identifies one authored tile definition.
- EffectTileFamilyId identifies one registered effect family.
- EffectTileEffectDefinitionId identifies typed values inside one family.
- EffectTileEffectRef pairs a family ID with a definition ID.
- EffectTileDefinition owns ID, key, display name, bounded effect references, bot traversal cost, and spawn clearance.
- MapGameplayProfile carries an optional EffectTileDefinitionId.

Each family owns a typed catalog. Initial registrations are ordinary movement multiplier, periodic neutral fighter damage, and positive-health modifier/block.

Built-in Speed, Slow, and Damage become ordinary definitions. Speed references movement 1250 milli. Slow references movement 700 milli. Damage references periodic damage 10/30 ticks plus a zero positive-health multiplier, preserving current behavior.

The map asset continues to own visual_profile_id. Presentation is independent from gameplay definition. A second asset may reuse a gameplay definition with another visual profile, and another definition may reuse a visual profile.

Authoring remains bounded: sorted unique IDs/keys, bounded definitions/effects, exact family validation, duplicate-family policy, spawn clearance, traversal cost, placement capacity, and canonical fingerprint material.

# Registration and ownership

Introduce a bounded startup-built/sealed EffectTileFamilyRegistry. A registration owns stable ID/key, typed definition coverage and validation, authoritative projection/system installation, and optional Balance Lab editor projection. Reject missing registry, late registration, duplicate ID/key, capacity overflow, unknown effect references, and missing required built-ins.

Pure catalog loading uses an immutable catalog assembled from the same family-owned registration constants used by Bevy plugins.

Do not add a dynamic Rust ABI, scripting, arbitrary World callbacks, or unvalidated runtime parameter maps. Generic authored references stop at the validated content boundary; resolved and runtime state is typed and family-owned.

# Resolution and runtime

Resolve placements into one cell-indexed lookup carrying placement ID and tile-definition ID. Do not rebuild the spatial index every fixed tick.

Install one server-owned runtime tile entity per resolved placement. Generic identity and cell components are shared; family projectors insert focused typed components. New family plugins insert their own components.

Replicated fighter occupancy carries map generation, placement ID, tile-definition ID, and entry tick. Pulse deadlines and family tuning remain server-only family state.

Occupancy remains center-cell, half-open, finite, and server-authoritative. Enter/exit reconciliation and its deferred boundary remain before authoritative movement.

Periodic damage candidates are sorted by placement ID and target network ID, grouped per placement/pulse, and committed through one combat-owned environment batch/attack identity. Reserve before health or deadline mutation; identity failure leaves the whole due group unchanged.

# Consumers

- Movement consumes only the movement-family projection.
- Damage consumes only periodic-damage family state.
- Positive-health paths consume only the health-modifier projection.
- Bots consume authored traversal cost, not tile identity.
- Spawn validation consumes authored clearance, not tile identity.
- Static tile presentation consumes the asset visual profile.
- Fighter feedback consumes definition/presentation semantics, never an asset-ID switch.
- No consumer matches Speed, Slow, Damage, or built-in asset IDs.

# Balance Lab and persistence

Expose effect-tile definitions/family values by stable definition identity instead of one global Speed/Slow/Damage singleton. Reuse family-owned bounds/editor metadata. Multiple definitions may use the same family with different values.

Bump Balance Lab snapshot/persistence schemas and migrate existing singleton values into the built-in definitions. Preserve safe apply/reset and authored catalog validation.

# Compatibility

This intentionally changes the catalog, fingerprint, and replicated occupancy shape. Bump relevant map/fingerprint/global protocol compatibility values once. Add no per-message versions or legacy network decoders.

# Acceptance criteria

- A synthetic fourth tile combines registered effects with distinct values and presentation using content/catalog data only.
- A second asset reuses an effect definition with another visual profile without code changes or incorrect rendering.
- Existing Speed, Slow, Damage gameplay, bot routing, healing suppression, feedback, maps, and Balance Lab values remain equivalent.
- A synthetic family registration seals and resolves without editing central registry/consumer implementation.
- Unknown, duplicate, excessive, and invalid definitions, registrations, references, and values fail deterministically.
- Multi-fighter damage pulses use one deterministic atomic batch per placement/pulse.
- Non-finite/out-of-bounds occupancy never resolves.
- Focused unit/ECS tests, role checks, lint, canonical tests, affected network tests, Balance Lab web tests/build, and native presentation evidence pass.
- Durable map and Balance Lab documentation describe the new model.
- Ticket evidence is recorded and ticket sync reports no conflicts.

# Implementation constraints

Preserve server authority, fixed ordering, bounded state, stable identity, role isolation, content fingerprints, combat-owned health/defeat mutation, observer-specific concealment, unrelated user changes, and public API where compatibility requires it. Prefer focused family modules and visible composition over a generic framework.

# Implemented architecture reconciliation

The accepted implementation keeps the registered-family/content boundary but uses a smaller runtime than the initial sketch. `EffectTileFamilyRegistry` is a bounded, deterministic startup resource. `register_effect_tile_family` rejects zero, duplicate, invalid-key, and excessive registrations before `MapContentPlugin` initializes the catalog. Family validators receive only one authored effect definition plus the typed capability projection; they are not arbitrary `World` callbacks.

The resolved map owns both ordered effect-tile facts and one prebuilt cell index. Existing shared mechanics are projected once into typed `EffectTileCapabilities`; the stable definition/family/effect references remain in the validated catalog for a new family's focused plugin. Runtime tile entities were not added because the resolved index plus replicated occupancy already provides one authoritative owner and avoids another entity lifecycle. This is the smaller Bevy-native implementation for the demonstrated mechanics.

Replicated occupancy now carries generation, placement, definition, visual profile, resolved public capabilities, and entry tick. Periodic deadlines are a server-only `EffectTilePulseState`. Positive-health blocking is an explicit zero multiplier effect rather than an implicit property of damage.

Presentation is driven by `MapVisualProfileId` and the client-only `GeneratedEffectTile` visual kind. Static material and fighter feedback no longer match gameplay tile identities or built-in asset IDs. Imported or generated visual profiles remain independently reusable.

The Balance Lab snapshot contains ordered tile definitions and effect definitions. Its manifest discovers registered built-in parameter records into comparison columns keyed by tile identity, while transaction validation freezes stable IDs, composition, family identity, and parameter topology. Version-13 singleton persistence migrates Speed, Slow, Damage, and interval values into the corresponding stable effect definitions.

# Verification evidence

- `just check` passed for routing, client, headless server, network-test, Balance Lab, the 15-test web suite, TypeScript, and the production web build.
- `just lint` passed formatting, all Clippy role matrices with `-D warnings`, server feature isolation, V3 presentation isolation, and V8 map cleanup.
- `just test` passed the final tree: routing plus client/server/Balance Lab unit and integration suites, 97 network tests, the combined revised-catalog network test, and 12 performance tests; zero failures.
- Focused tests passed for a synthetic fourth family/tile registration, composed independent families, invalid parameters, finite half-open occupancy, one atomic multi-fighter placement pulse, exact map compatibility digests, client visual catalog coverage, effect-tile bounds/spawn safety, version-13 persistence migration, and Balance Lab matrix fields.
- `npm run typecheck`, `npm test`, and the Vite production build passed under `tools/balance-lab-web`.
- `git diff --check` passed.
- Native visual playtesting was not required because this change preserves the accepted Speed/Slow/Damage appearance and changes selection ownership rather than pixels; client catalog coverage, materialization tests, and the client build cover the presentation risk.

# Learning review

The original design treated content instances (Speed, Slow, Damage) as the extension vocabulary. That made presentation, bot policy, healing suppression, tuning, and runtime scheduling infer semantics from identity. The audit also exposed four consequences: damage secretly blocked healing, the spatial index rebuilt every tick, simultaneous pulses created separate attack identities, and a private deadline was replicated.

The prevention rule is to distinguish instance extension from mechanic-family extension at the authored boundary. Stable definitions compose independently registered semantic families; consumers read only owned typed projections; visuals use client profiles; server-private scheduling remains private; and one environmental cause owns one deterministic combat transaction. Balance Lab comparison metadata must follow those semantic families rather than legacy field names or singleton identities.
