---
id: BRL-0094
title: Add composable layered combat VFX recipes
status: doing
theme:
release:
created: 2026-08-31T07:44:11Z
modified: 2026-09-01T20:01:29Z
closed:
revision: 87a46737c5f20d9b
blocks: []
related: [BRL-0001, BRL-0003, BRL-0021, BRL-0040, BRL-0046, BRL-0059, BRL-0070, BRL-0082, BRL-0096, BRL-0097, BRL-0098, BRL-0099, BRL-0100, BRL-0101, BRL-0102, BRL-0103, BRL-0104, BRL-0107, BRL-0108]
---

# Description

# Outcome

Add a bounded, client-only composable visual-recipe system that gives authored and player-composed weapons, ultimates, persistent areas, projectiles, impacts, status feedback, and selected world effects distinctive readable styles without moving authority into presentation or turning each weapon into a one-off renderer.

Gameplay recipes remain authoritative and semantic. Client presentation derives a visual signature from stable gameplay facts, resolves that signature through validated layered VFX recipes, and materializes the result through lifecycle-specific renderer adapters. The framework must preserve server/client isolation, exact gameplay geometry, deterministic bounded allocation, late-join and recovery behavior, reduced-effects accessibility, primitive degradation, and current presentation fallbacks.

# Why this work is needed now

The completed VFX reorganization in BRL-0082 created the correct first seam:

- feature-owned producers translate gameplay facts into validated client-only `VfxRequest` messages;
- stable request keys are registered through a sealed bounded registry;
- the renderer alone resolves catalog profiles and concrete Bevy assets;
- deterministic producer ordering, total capacity, per-profile caps, authoritative deadlines, fallbacks, and reduced-effects variants are preserved;
- gameplay definitions and protocol state do not carry mesh handles, material handles, source paths, renderer families, or client-local entity identities.

That seam is necessary but not yet a rich visual-recipe system. The current transient catalog selects exactly one profile containing one renderer family, one material, one scale policy, one anchor policy, and one lifetime. The concrete transient vocabulary is currently limited to a sphere or ground ring. Durable projectile, elemental-field, persistent Splash, sticky-blob, cone, dash-trail, concealment-field, and fighter-status presentation still use separate hard-coded materialization paths.

The practical result is that the gameplay weapon recipe is meaningfully customizable while its presentation remains comparatively bland:

- muzzle, impact, damage, and reset effects are generic;
- all elemental-field activation cues currently select one transient semantic key even though the cue carries the elemental kind;
- straight and lobbed projectiles use a small fixed mesh/material vocabulary;
- persistent fields are differentiated mostly through flat material color;
- gameplay effect composition does not yet produce a corresponding reusable presentation composition;
- the held weapon silhouette and attachment remain a separate presentation limitation beyond transient VFX.

# Review of the proposed visual direction

The following principles are accepted as the intended visual direction:

1. Build effects from a small number of readable layers rather than one monolithic particle system or bespoke shader per weapon.
2. Use meshes when exact shape and counterplay matter: area boundaries, fills, cones, shockwaves, projectile bodies, tracers, beams, ground shadows, and telegraphs.
3. Use particles for irregular motion and texture: sparks, smoke, embers, snow, shards, debris, dust, flame wisps, poison motes, and restorative motes.
4. Keep AoE centers visually restrained and emphasize exact boundaries so fighters, objectives, projectiles, and map geometry remain readable.
5. Treat projectile body, trail, shadow, impact, and explosion as separate reusable layers.
6. Sequence explosions as flash, expanding body or ground flash, shockwave, fragments/sparks, debris, and smoke instead of one undifferentiated burst.
7. Author scale, alpha, rotation, translation, delay, duration, and easing as bounded timeline data where the renderer can support them safely.
8. Preserve readability when bloom is disabled; emissive output may enhance but must not define the silhouette.
9. Use element-specific motion and silhouette as well as hue:
   - Cold: crystalline edges, shards, restrained drift, sharp pulses.
   - Fire: fast upward motion, embers, irregular flicker, energetic pulses.
   - Poison: slower spores, bubbles, viscous or drifting motion.
   - Restoration: clean inward/upward motes and a stable positive rhythm.
10. Keep relationship/team readability separate from elemental identity. A rim, outline, marker, or secondary band should carry ally/enemy meaning without erasing the element.

Several generic suggestions must be adapted to Brawler's architecture:

- Do not introduce a closed `ImpactKind` enum or central exhaustive effect inventory. BRL-0082 replaced that fanout with stable request registration and catalog mappings.
- Do not integrate projectile movement in VFX code. Straight and lobbed visuals must follow replicated authoritative planar motion; the existing `LobbedFlight` fact remains the source of arc timing and height.
- Do not replace projectile delivery with a hitscan tracer unless a future gameplay ticket adds an authoritative hitscan delivery family. A trail may exaggerate motion while the visible projectile body continues matching the authoritative collider.
- Do not route persistent fields through one-shot spawn-and-timeout messages. Replicated state must remain the source for late join, recovery, restart, owner replacement, expiry, and teardown.
- Do not place camera shake inside generic VFX recipes. Camera feedback requires a separate local, proximity-aware, reduced-motion/accessibility policy.
- Do not replace the existing bounded allocation transaction, authoritative deadlines, reduced-effects selection, or deterministic producer ordering.
- Do not create an unrestricted particle/shader/effect graph. A bounded typed layer vocabulary is sufficient for the demonstrated content.

# Recommended architecture

## Data flow

```text
Authoritative recipe, cue, or replicated state
                    |
                    v
      Client presentation signature
  carrier + phase + accent + relationship
                    |
                    v
             VFX recipe catalog
      ordered, bounded collection of layers
                    |
       +------------+-------------+
       |            |             |
       v            v             v
 transient burst  attached view  persistent state view
 muzzle/impact    projectile     field/sticky/status
 explosion        trail/shadow   exact telegraph
```

The three materialization paths share a recipe and layer vocabulary, but retain different lifecycle owners.

## Client presentation signature

Presentation should compose from semantic gameplay ingredients rather than from render handles:

- carrier or delivery family: straight, lobbed, sticky, melee, cone, Splash, field, deployable, environment;
- phase: muzzle, body, trail, impact, activation, active area, pulse, expiration, status;
- accent: neutral, Cold, Fire, Poison, Restoration, Demolition, concealment, relationship-only;
- observer relationship: local, ally, enemy, neutral;
- geometry: point, oriented point/hit normal, circle, rectangle, segment, cone, or attached owner;
- optional explicit override for a distinctive preset, weapon part, ultimate definition, deployable, or world-object family.

Examples include:

```text
Straight + Fire + Enemy + ProjectileBody
Straight + Cold + Ally + Impact
Lobbed + Neutral + Enemy + GroundShadow
Circle + Restoration + Ally + ActiveArea
Circle + Fire + Enemy + Pulse
```

The gameplay `WeaponRecipe` already composes economy, firing, delivery, payload bundles, and world effects. Client presentation should derive a bounded visual signature from these semantic ingredients without making renderer data part of damage, collision, gameplay fingerprints, or server execution.

New semantic combinations should normally resolve through carrier defaults plus one or two accent layers. An explicit content override is reserved for cases that need a distinctive silhouette or timing language not implied by the ordinary carrier/effect combination. Precedence must be deterministic and validated, for example:

1. exact ultimate or authored content override;
2. exact weapon preset override;
3. composed carrier plus payload accent;
4. carrier default;
5. safe catalog fallback.

Multi-effect recipes need a bounded deterministic accent policy. The first implementation should support at most two selected accents and define stable precedence rather than concatenating an arbitrary number of layers.

## Stable source identity

`WeaponDefinitionId` alone is not sufficient for exact player-composed weapon presentation because non-preset recipes currently use a compatibility weapon definition. The client already receives `ResolvedMatchLoadout`, including the resolved weapon recipe and recipe fingerprint, so the first implementation may derive and cache presentation signatures from the source fighter's replicated loadout.

The design must explicitly handle cues or projectiles that outlive a source entity or a live loadout replacement. If current replicated facts cannot freeze the spawn-time semantic recipe safely, add the existing semantic `WeaponRecipeFingerprint` to the smallest necessary projectile/cue facts through the normal global protocol compatibility migration. Do not add mesh, material, particle, or VFX recipe IDs to the gameplay protocol merely for renderer convenience.

Ultimate and persistent-area style should use existing semantic facts such as ultimate definition/kind, `ElementalFieldKind`, `StickyBlobKind`, payload effects, geometry, team, and authoritative deadlines.

## Layered recipes

A VFX recipe is a bounded ordered collection of typed reusable layer specifications. Conceptually:

```rust
struct VfxRecipe {
    id: VfxRecipeId,
    layers: BoundedVec<VfxLayerSpec>,
    concurrency_cap: usize,
    fallback_recipe: VfxRecipeId,
    reduced_recipe: Option<VfxRecipeId>,
}

enum VfxLayerKind {
    Mesh(MeshLayer),
    Particle(ParticleLayer),
    Trail(TrailLayer),
}
```

Each layer may author only validated bounded values:

- delay and duration;
- renderer/asset/material key;
- anchor and geometry policy;
- start/peak/end scale;
- start/peak/end alpha where the renderer can implement it without unbounded material churn;
- rotation and translation/drift policy;
- named easing;
- readability classification;
- per-layer capacity, fallback, and reduced behavior.

Prefer a small normalized start/peak/end timeline or another bounded typed form over arbitrary curves or scripts. The renderer adapter remains responsible for implementing timeline data efficiently. Asset pooling, custom material uniforms, or another per-instance alpha technique should be chosen from measured implementation needs rather than hidden behind an unproven general framework.

The current `VfxProfile` can evolve into a reusable layer profile, while request mappings resolve to recipes rather than one concrete profile.

## Lifecycle adapters

### Transient requests

Continue using ordered `VfxRequest` messages for:

- muzzle flashes;
- impacts and contact bursts;
- explosions;
- activation, pulse, and expiration bursts;
- short tracers and shockwaves;
- pickup and objective feedback.

The request contract may grow a bounded typed geometry/style context. Avoid a proliferating key for every cross-product such as `combat.impact.fire.scatter.enemy`; retain semantic phase keys and carry a validated presentation signature or equivalent typed context.

### Attached views

Owner-reconciled visual roots handle:

- projectile body and trail;
- lobbed projectile ground shadow;
- sentry/deployable attachments;
- dash trails;
- sticky-blob body and fuse treatment.

Attached roots cache the resolved spawn-time recipe/signature and follow authoritative replicated position, rotation, and lifecycle facts. Decorative children cannot influence collision or authority.

### Persistent state views

Replicated-state reconciliation remains the owner for:

- elemental fields;
- persistent Splash areas;
- concealment fields;
- sticky explosion telegraphs;
- cone volumes;
- fighter status overlays.

These views use exact authoritative geometry and deadlines. Activation/pulse/expiry bursts may additionally use transient requests, but the mandatory persistent telegraph is never reconstructed solely from an ephemeral cue.

## Readability and accessibility

Every area or warning recipe distinguishes:

- readability-critical layers: exact boundary, exact geometry, relationship cue, activation/armed state, authoritative duration;
- decorative layers: mist, flame wisps, embers, shards, smoke, debris, secondary pulses.

Reduced-effects mode may remove, shorten, or thin decorative layers. It must preserve mandatory geometry and counterplay. Primitive degradation must also retain the mandatory layer without requiring imported assets, particle assets, bloom, or custom shaders.

Color alone is not sufficient for Cold/Fire/Poison/Restoration or ally/enemy distinction. Native playtesting must evaluate dense 3v3 overlap, concealment interaction, objective visibility, and common color-vision ambiguity.

## Hanabi boundary

`bevy_hanabi` 0.19 is compatible with Bevy 0.19 and Rust 1.95, matching Brawler's Bevy 0.19.1 and Rust 1.95 toolchain. It supports a client-only 3D configuration and is a strong candidate for the particle renderer.

Hanabi must remain a renderer adapter rather than the architecture:

- optional/client-only dependency with default features disabled and the 3D feature enabled;
- no dependency in server, headless, or gameplay-content graphs;
- named bounded particle assets such as `fire-rim`, `frost-shards`, `impact-sparks`, and `restoration-motes`;
- catalog references and validated parameter overrides rather than an exposed general expression graph;
- deterministic CPU-side request order and capacity policy even though individual GPU particle motion is decorative;
- explicit reduced-effects counts/rates and a non-particle fallback for readability-critical effects.

Adoption is gated by a focused macOS/Metal spike covering startup, rendering correctness, teardown, primitive degradation, maximum expected 3v3 overlap, reduced effects, frame evidence, and dependency/build impact. A failed spike must leave the mesh-layer architecture usable and record the alternative or deferral.

# Representative target compositions

## Cold field

- exact low-opacity ground fill;
- exact bright boundary ring;
- activation burst;
- restrained rim or edge mist;
- slow snow/crystalline particles;
- periodic cold pulse;
- expiry contraction/fade.

## Fire field

- exact low-opacity ground fill;
- exact irregular-looking but geometry-faithful boundary treatment;
- activation flash;
- circumference-biased flame particles;
- ember particles;
- faster pulse/flicker;
- expiry collapse.

## Straight weapon

- held weapon muzzle socket or current fallback origin;
- very short muzzle flash;
- authoritative projectile body;
- short directional trail;
- impact flash;
- accent-specific fragments/sparks;
- team/relationship rim or secondary marker.

## Lobbed or explosive weapon

While flying:

- authoritative lobbed projectile body;
- optional short trail;
- ground-projected shadow that remains planar while the projectile follows its replicated visual arc.

On impact:

- immediate flash;
- expanding flash body or ground flash;
- exact shockwave ring;
- accent fragments/sparks;
- debris;
- delayed smoke;
- optional separate accessibility-aware camera feedback.

# Held weapon presentation

Transient VFX alone will not fully solve bland weapon identity. A later phase or closely related follow-up should define a client-only `WeaponPresentationProfile` owning:

- held model or primitive silhouette;
- attachment transform;
- muzzle socket;
- material/style family;
- animation compatibility;
- imported-asset readiness and primitive fallback;
- associated VFX recipe family.

This must remain separate from authoritative weapon behavior. The first VFX ticket should establish its integration seam and may deliver one demonstrative held-weapon variation only if the active specification includes it without compromising the core layered VFX vertical slice.

# Delivery strategy

Deliver player-visible vertical slices rather than an infrastructure-only framework:

1. Evolve the existing request/catalog/renderer seam into bounded layered mesh recipes while preserving current output and tests.
2. Prove semantic reuse with distinctive Cold and Fire presentation shared between weapon payloads and matching ultimate fields.
3. Gate and, if accepted, add the Hanabi particle renderer through a macOS/Metal spike.
4. Prove attached and explosive lifecycles with lobbed projectile shadow/trail and layered impact/explosion treatment.
5. Expand across remaining weapon, ultimate, persistent-area, status, deployable, and world-object families based on measured readability and performance.
6. Address held-weapon silhouette through the smallest validated presentation-profile seam.

# Architectural rule

Keep gameplay recipes authoritative and semantic; derive a client-only visual signature from their reusable ingredients; resolve that signature into bounded layered VFX recipes; and materialize those recipes through lifecycle-specific adapters.

This extends BRL-0082 without regressing to one-off shaders, a closed central effect enum, renderer data in gameplay definitions, duplicated gameplay timing, or an unrestricted effect graph.
