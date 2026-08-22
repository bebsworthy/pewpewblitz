# Weapons and abilities specification

## Purpose and authority

This document defines the durable combat-content contract: player weapon selections, operational
weapon recipes, delivery and payload primitives, ability execution, effect attribution, and combat
lifecycle rules. It records both the supported foundation and bounded extension points.

[Fighter and build specification](./02-fighter-model.md) owns loadout resolution, fighter runtime,
target-held status state, arsenal, and equipment. [Engine specification](./01-engine-decision.md)
owns Bevy application composition and fixed schedules. [Network architecture](./08-network-architecture.md)
owns authority and replication boundaries. Versioned implementation documents retain delivery
history, exact milestone scope, and verification evidence.

## Combat authoring layers

Keep player choices, developer-authored operational data, resolved match data, and mutable runtime
state distinct:

```text
WeaponContentCatalog
  Typed primitives, compatibility, policy, costs, and engine-safe bounds

PlayerWeaponSelection
  Preset identity or bounded typed specification

WeaponConfiguration
  Operational recipe plus an approved presentation-profile reference

WeaponPreset
  Named developer-authored legal configuration

ResolvedWeapon
  Immutable server-validated configuration, identity, and fingerprint

WeaponState
  Ammo or charges, cooldown, reload, and recharge state during play
```

The supported selection contract does not submit a low-level operational recipe. The custom Pulse
exposes discrete power, reach, and magazine choices; the server converts them into a configuration
and then runs the ordinary weapon validator. Future editors may expose more typed primitives, but
clients must never select executable systems, presentation assets, arbitrary serialized components,
scripts, or unbounded numeric maps.

Built-in presets and player-authored selections use the same configuration validation and runtime
execution paths. Preset identity may affect naming, onboarding, analytics, or presentation lookup;
it must not select a separate combat implementation.

## Operational weapon recipe

The supported operational recipe composes economy, firing, delivery, target selection, payloads,
and world effects:

```text
WeaponConfiguration
  PresentationProfileId
  WeaponRecipe
    Economy
    FireCooldown
    FiringPattern
    DeliveryMethod
    PayloadBundles
      TargetSelection
      Effects
    WorldEffects
```

Input devices are deliberately outside this recipe. Client input becomes abstract attack intent;
the authoritative server decides whether the resolved weapon may fire and expands an accepted
attack into deliveries. A future charge, channel, release, or aim-and-release behavior may add a
typed input/activation policy, but it must remain independent of keyboard, mouse, or controller
bindings.

The recipe may grow only through implemented, bounded primitives that create a demonstrated
player-visible capability. Do not prebuild a general behavior graph or one ECS system per content
definition.

## Validation and resolution

Weapon resolution is deterministic and separates three concerns even when one cohesive module owns
them:

1. **Structural and capability validation:** IDs exist, fields are finite and bounded, collection
   sizes are safe, and every selected primitive has an implemented authoritative execution path.
2. **Balance and compatibility validation:** the configuration obeys current catalog policy, build
   budget, fighter compatibility, mutual exclusions, and required tradeoffs.
3. **Entitlement validation:** the account may use the selected options. This belongs to a future
   arsenal/account boundary, not to combat execution.

Code-owned engine ceilings bound collection sizes, numeric fields, target counts, deadlines, and
serialized resolved data. Authored policy may narrow those limits but cannot widen them. Stable
fingerprints derive from canonical operational data, not local ECS identity or display names.

Presentation-profile IDs are approved as part of server-known content or server resolution. A
client may request a legal cosmetic choice when that product capability exists, but it cannot attach
an arbitrary presentation profile to change combat readability or asset loading.

## Supported weapon capability set

The established weapon foundation supports:

- magazine and charge economies;
- single and spread firing patterns;
- straight projectile, lobbed projectile, and melee-arc delivery;
- direct and bounded area target selection;
- damage, knockback, and strongest-refreshes slow effects;
- no falloff and linear damage falloff;
- hostile and bounded hostile-plus-owner recipient policies;
- one bounded terrain-destruction world effect;
- authoritative cooldown, reload or recharge, lifetime, collision, and outcome attribution;
- stable presentation cues resolved independently by the client.

The four reference presets are:

| Weapon | Pattern | Strength | Cost or weakness |
|---|---|---|---|
| Pulse sidearm | Single direct projectile | Reliable mid-range pressure | Low burst |
| Scatter cannon | Short spread of pellets | Excellent close-range burst | Poor range and falloff |
| Arc launcher | Lobbed splash projectile | Punishes cover and groups | Slow delivery and recovery |
| Impact blade | Melee arc | Strong duel pressure and displacement | Must enter danger range |

They exercise reusable composition primitives and are not permanent weapon classes. The bounded
custom Pulse proves that a non-preset selection resolves into the same operational representation.

## Delivery and projectile model

An accepted attack produces one or more authoritative deliveries. Projectile-like deliveries keep
trajectory, movement, collision, lifetime, payload, and presentation observation separate:

```text
AttackSource
  Stable attack, player, team, build, and weapon identity

Delivery
  Delivery index and method
  Authoritative origin and targeting data
  Lifetime or completion rule
  Payload bundles and world effects
  Stable presentation references or cues
```

Straight deliveries move through authoritative planar simulation. Lobbed deliveries resolve a
bounded landing point and flight deadline while clients may present an arc independently. Melee arcs
are deliveries without projectile entities. The fighter does not own the delivery after creation;
stable source identity preserves attribution across entity lifecycles.

Possible future delivery families include:

- charged or delayed projectiles with warning telegraphs;
- beams or repeated ray ticks;
- persistent areas and traps;
- homing or steered projectiles;
- bouncing, piercing, splitting, boomerang, or returning projectiles;
- deployables, turrets, and summons.

Add one coherent family at a time. A new family needs bounded definition data, authoritative
execution and cleanup, network behavior, presentation cues, counterplay, and verification. Existing
fighter behavior and unrelated recipes should not require rewrites.

## Collision and impact rules

Every delivery declares or derives bounded collision and completion behavior. Possible rules include:

- stop at the first blocking collision;
- resolve at a bounded landing point;
- affect a direct target or bounded area;
- bounce or pierce a limited number of times;
- split into a bounded number of child deliveries;
- create a persistent region;
- emit a terrain brush;
- create a summon or deployable.

Impact systems produce authoritative gameplay outcomes through components or registered messages as
appropriate. They must not load or mutate particles, sounds, camera shake, controller feedback, or
UI assets.

## Effects and status contributions

An impact applies an immediate effect, emits a contribution to target-held status, or creates a
world effect.

Immediate effects may include:

- damage or healing;
- shield creation or removal;
- slow, stun, root, interrupt, or ability lockout;
- knockback or pull;
- damage over time;
- mark, reveal, buff, or debuff.

Every implemented effect needs explicit recipient policy, magnitude, duration, stacking or refresh
behavior, attribution, and cleanup. Expiry, defeat, respawn, disconnect, build replacement, match
restart, and source removal must produce deterministic outcomes.

An accumulating interaction emits a bounded contribution rather than directly triggering the final
effect:

```text
StatusContribution
  Stable status identity
  Amount
  Source and attribution
  Optional tick interval or contribution duration
```

The target-owned definition, resolved rules, meter value, decay, threshold, lockout, resistance, and
immunity contracts live in
[Fighter and build specification](./02-fighter-model.md#target-owned-status-state). Multiple weapons,
abilities, allied sources, and persistent regions may contribute to the same status identity.

For example:

```text
Ice pellet  -> cold +12 on hit
Ice zone    -> cold +4 every 15 simulation ticks while occupied
Cold rules  -> freeze when the target-owned meter reaches its threshold
```

The threshold policy must explicitly define reset, partial reduction, trigger cooldown, and temporary
immunity so the interaction is deterministic and explainable to players.

## Terrain and environment effects

Terrain destruction is a world-level effect rather than a per-target payload. A delivery emits a
bounded brush—such as a circle, capsule, rectangle, or authored shape—with a position, size, and
optional material operation. The terrain subsystem deterministically quantizes it into authoritative
occupancy and regenerates affected collider chunks. Weapons do not know about grid cells, rendered
tiles, chunk meshes, or collider entities.

Smoke, darkness, grass-like cover, speed fields, healing areas, and other environment regions should
follow the authored/runtime ownership boundaries in
[Environment gameplay direction](./09-environment-gameplay.md). An ability may create or
configure a region; it must not implement a parallel client-only visibility, movement, or objective
rule.

## Ability model

Weapons, ultimates, passives, active items, and equipment should reuse common combat primitives when
their semantics match: stable source identity, targeting, payload effects, costs, deadlines,
attribution, outcome facts, presentation cues, and lifecycle cleanup.

They are not required to share one universal recipe schema. A dash owns authoritative movement and
interruption behavior; a sentry owns placement, targeting, firing, health, lifetime, ownership, and
cleanup. Focused typed systems are preferable to forcing unlike capabilities through weapon firing
and delivery fields.

An ability definition should declare stable identity, activation requirements, cost or charge
policy, compatible fighter/build rules, presentation references, and the typed capability it grants.
Resolved ability data belongs to the immutable match loadout. Charge, cooldown, activation phase,
deployed objects, and trigger windows are runtime state.

## Supported ultimates and passives

The supported ultimate set contains:

- **Dash:** authoritative directed movement with collision, interruption, contact attribution, and
  cleanup;
- **Sentry:** bounded placement, one owned deployable, autonomous targeting and fire, health,
  lifetime, destruction, and lifecycle cleanup.

The supported passive set contains:

- Lightweight Frame;
- Reinforced Frame;
- Adrenal Response;
- Close Quarters;
- Quick Cycle;
- Tenacity.

Passives should alter a meaningful decision, range, risk, or timing window rather than merely raise
every number. Their authored definitions grant bounded modifiers or typed capabilities; runtime
systems read resolved grants and do not branch on inventory rarity or acquisition history.

Possible future ultimate families include shielding, healing or repair fields, area pull or
knockback, and temporary wall placement. Each should be introduced as a complete readable gameplay
slice rather than as unused framework capacity.

## Active-item extension

Active items are not part of the supported loadout. Candidate capabilities include a short speed
burst, emergency shield, reveal pulse, partial reload, or deployable decoy. Adding the slot requires:

- an abstract input action and controller/keyboard parity;
- authoritative activation, cooldown, rejection, and cleanup;
- a clear HUD deadline or charge indicator;
- distinct audiovisual and controller feedback;
- loadout cost, slot, conflict, and compatibility rules;
- replication, reconnect, restart, and process-evidence coverage.

Collectible ownership and equipment resolution are specified in
[Fighter and build specification](./02-fighter-model.md#collectible-equipment-extension). Combat
systems consume resolved grants and runtime state, not item instances.

## Presentation contract

Client presentation systems may observe replicated gameplay state and registered combat cues to
produce:

- muzzle flashes, trails, impact flashes, explosions, debris, and particles;
- hit markers, damage feedback, warning telegraphs, and projected world UI;
- screen shake and controller feedback;
- weapon, impact, ability, and status audio;
- terrain-crater and persistent-region presentation.

Presentation may degrade to placeholders or primitive assets without changing authority. Missing,
late, duplicated, or suppressed presentation cues must not change collision, damage, status,
cooldowns, scoring, navigation, or cleanup.

## Combat lifecycle and boundedness

All combat capabilities must define behavior for acceptance, execution, completion, expiry, defeat,
respawn, disconnect, build replacement, match completion, restart, and teardown. Ownership-sensitive
objects such as deployables must have explicit cleanup and attribution after their owner changes
state.

Runtime collections and fan-out are bounded by code-owned ceilings: deliveries per attack, targets
per delivery, payloads and effects, live deployables, persistent regions, child spawns, lifetimes,
and telemetry. Content policy may impose stricter limits.

## Balance checklist

For each weapon or active combat capability, answer:

- What distance, timing, or position does it want?
- What does an opponent do to counter it?
- What map geometry strengthens or weakens it?
- What are its burst and recovery windows?
- How much aim or movement precision does it demand?
- What build budget and slot opportunity does it consume?
- Can its damage, control, ownership, and expiry be understood from feedback?
- Does it create a reason to move or make a decision rather than hold one position?
- Does it remain bounded under many participants, deliveries, or persistent objects?
