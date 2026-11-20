# Weapons and abilities specification

## Purpose and authority

This document defines the durable combat-content contract: character signature weapons, operational
weapon recipes, delivery and payload primitives, ability execution, effect attribution, and combat
lifecycle rules. It records both the supported foundation and bounded extension points.

[Fighter and build specification](./02-fighter-model.md) owns loadout resolution, fighter runtime,
target-held status state, and the roster character catalog. [Engine specification](./01-engine-decision.md)
owns Bevy application composition and fixed schedules. [Network architecture](./08-network-architecture.md)
owns authority and replication boundaries. Ticket owns active implementation scope and evidence;
versioned implementation documents retain historical delivery records.

## Combat authoring layers

Keep authored character weapons, developer-authored operational data, resolved match data, and
mutable runtime state distinct:

```text
WeaponContentCatalog
  Typed primitives, compatibility, policy, and engine-safe bounds

SignatureWeapon (per roster character, content/catalogs/characters.ron)
  One weapon-base identity plus up to four authored part definitions

WeaponConfiguration
  Operational recipe plus an approved presentation-profile reference

WeaponPreset
  Named developer-authored legal configuration (the weapon bases)

ResolvedWeapon
  Immutable server-validated configuration, identity, and fingerprint

WeaponState
  Ammo or charges, fire cooldown, and next-ammunition recovery interval during play
```

Players do not choose or submit a weapon: they pick a roster character, whose signature weapon is
authored content. Clients must never select executable systems, presentation assets, arbitrary
serialized components, scripts, or unbounded numeric maps. Every weapon base and part combination
uses the same configuration validation and runtime execution paths; preset identity may affect
naming, analytics, or presentation lookup but never selects a separate combat implementation.

## Weapon bases and authored parts

The seven reference weapon presets are the weapon bases. Lobby authority advertises their stable
IDs, display names, presentation profile keys, and each character's weapon base as part of the
connection-scoped character catalog. More bases may be added as complete playable recipes; a new
character may reuse any base.

The current canonical base values affected by ordinary play balance are:

| Weapon | Economy | Recovery | Firing | Delivery | Direct damage |
|---|---|---:|---|---|---:|
| Pulse Sidearm | 4-shot magazine | 1.0 s/round | Single | 500 speed, 2 radius, 320 range | 200 |
| Scatter Cannon | 3-shot magazine | 1.2 s/round | 5 over 30° | 600 speed, 2 radius, 320 range | 120/projectile before falloff |
| Arc Launcher | 3-shot magazine | 1.6 s/round | Single | 520-distance lob | 40 area |
| Impact Blade | 3 charges | 1.0 s/charge | Single | 120-reach melee arc | 34 |
| Sticky Blomb | 3-shot magazine | 1.5 s/round | Single | 640-range delayed sticky projectile | 200 hostile area damage on detonation |
| Spray | 3-shot magazine | 1.5 s/round | Single spritz | 480-speed, 240-reach, 70° cone | 40/pulse before falloff |
| Splash | 3-shot magazine | 1.5 s/round | Targeted lob | 480-range persistent circle | 36 hostile damage and 24 allied healing per pulse |

Durations are displayed in seconds but remain positive 60 Hz fixed-tick values in authoritative
content. Editors accept ordinary decimal seconds and save the nearest fixed tick; for example,
`0.17 s` becomes 10 ticks. The embedded catalogs, not this summary, are the executable source of
truth.
Arc Launcher damage no longer destroys terrain. Generic delivery-level `DestroyMap` remains a
validated weapon capability for future authored recipes, but no built-in weapon currently grants
it.

A signature weapon may name up to four authored weapon-part definitions
(`content/catalogs/weapon_parts.ron`). Parts are authoring primitives, not player-owned items: they
have no slots, ownership, instances, or inventory. Their order does not change the resolved weapon
or its gameplay fingerprint. The launch roster authors no parts.

Part effects use a closed typed vocabulary over implemented weapon properties and capabilities:
bounded flat or percentage changes to capacity, damage, fire interval, refill/recharge interval,
semantic reach, and one bounded Slow contribution. It does not yet expose projectile speed/radius,
spread, knockback, or new payload/status families. The vocabulary must not use string field paths,
scripts, arbitrary numeric maps, or serialized ECS components. A new effect kind is added only with
explicit combination, validation, presentation, lifecycle, and verification rules.

Resolution is deterministic:

1. Load the character's weapon base and authored parts from the active catalogs.
2. Validate bounds and each part's applicability to the base recipe.
3. Canonically order the effects and aggregate each property using its declared stacking rule.
   Numerical modifiers sum flat values, apply the combined percentage, then clamp and round once.
   Repeated status contributions aggregate into one bounded effect per status kind.
4. Apply the aggregate to a copy of the base `WeaponConfiguration`.
5. Run the ordinary structural, capability, compatibility, balance, and engine-ceiling validator.
6. Produce the existing immutable `ResolvedWeapon` and canonical gameplay fingerprint.

A part whose effect cannot apply to the weapon base makes the character catalog invalid at load; the
resolver never silently discards an authoritative effect. Combat, movement, networking, and
presentation systems consume the resolved recipe and grants, never part names or cosmetic types.

Saved brawlers, the custom Pulse specification, the player part inventory, and its starter parts
were retired with the roster (BRL-egn6). Accumulating frost is a separate target-owned status
capability; a frost-themed part may only grant an already-supported bounded effect such as slow,
with presentation text that accurately describes the actual rule.

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

Any change to weapon properties, recipe primitives, validation, resolution, fingerprints, or
weapon-derived runtime state must review and update the development
[Balance Lab](./15-balance-lab.md#required-maintenance-contract) in the same change, or explicitly
document why the capability is intentionally unavailable there. The lab must expose balance choices
while retaining only named engine, bounded-work, deterministic-geometry, and wire-safety limits.

Weapon resolution is deterministic and separates three concerns even when one cohesive module owns
them:

1. **Structural and capability validation:** IDs exist, fields are finite and bounded, collection
   sizes are safe, and every selected primitive has an implemented authoritative execution path.
2. **Balance and compatibility validation:** the configuration obeys current catalog policy,
   fighter compatibility, mutual exclusions, and required tradeoffs. Character resolution has no
   point budget, and catalog definitions carry no point costs.
3. **Entitlement validation:** every roster character is available today. Unlocks would belong to
   a future account boundary, not to combat execution.

Code-owned engine ceilings bound collection sizes, numeric fields, target counts, combat-effect deadlines, and
serialized resolved data. Authored policy may narrow those limits but cannot widen them. Stable
fingerprints derive from canonical operational data, not local ECS identity or display names.
Per-ammunition refill/recharge duration has no gameplay tuning ceiling; it must only be positive
and representable by the authoritative tick deadline.

Numeric weapon, passive, and ultimate bounds have one crate-private typed owner beside the authored
model and authoritative validator. Balance Lab's private editor adapters consume those same owners
and exhaustively project the editable leaves; the editor describes legal authoritative values but
does not become an independent validation authority.

Presentation-profile IDs are approved as part of server-known content or server resolution. A
client may request a legal cosmetic choice when that product capability exists, but it cannot attach
an arbitrary presentation profile to change combat readability or asset loading.

## Supported weapon capability set

The established weapon foundation supports:

- magazine and charge economies;
- single and spread firing patterns;
- straight projectile, lobbed projectile, melee-arc, delayed sticky, propagating cone-spray, and
  targeted persistent Splash delivery;
- direct and bounded area target selection;
- damage, healing, Cold, Fire/Poison damage-over-time, knockback, and strongest-refreshes slow effects;
- no falloff and linear damage falloff;
- explicit owner, ally, hostile, and bounded combined recipient policies;
- one bounded terrain-destruction world effect;
- explicit positive-damage eligibility for live environment objects and hostile Heist objectives;
- authoritative fire cooldown, one-at-a-time ammunition recovery, lifetime, collision, and outcome attribution;
- stable presentation cues resolved independently by the client.

The seven reference presets are:

| Weapon | Pattern | Strength | Cost or weakness |
|---|---|---|---|
| Pulse sidearm | Single direct projectile | Reliable mid-range pressure | Low burst |
| Scatter cannon | Short spread of pellets | Excellent close-range burst | Poor range and falloff |
| Arc launcher | Lobbed splash projectile | Punishes cover and groups | Slow delivery and recovery |
| Impact blade | Melee arc | Strong duel pressure and displacement | Must enter danger range |
| Sticky Blomb | Delayed sticky area blast | Threatens carriers and bounded positions | Fuse gives time to react |
| Spray | Instant stationary cone spritz | Sustained close-range area pressure | Must commit aim and origin at firing time |
| Splash | Targeted lob into a persistent field | Damages hostiles while sustaining its owner and allies | Delayed placement and fixed stationary footprint |

### Fire and ammunition-recovery lifecycle

Refill/recharge duration always means the time for one ammunition unit, never a duration
that restores the whole magazine.

- Spending ammunition starts an interval immediately when none is active and stock is below
  capacity.
- Firing again consumes available stock but does not restart, delay, or otherwise change the active
  interval. A round that was 90% recovered remains 90% recovered and continues toward the same
  server deadline.
- At the deadline the server restores exactly one unit. If stock is still below capacity, it starts
  the next full interval on that same authoritative tick; otherwise recovery becomes inactive.
- Fire cooldown and ammunition recovery are independent. A fighter may be cooling down while an
  ammunition interval advances, and a completed interval does not bypass fire cooldown.
- A passive that changes recovery duration applies when a new interval starts. It does not rewrite
  an interval already in progress.
- Spawn, respawn, match restart, and authoritative build replacement restore the resolved starting
  economy and clear stale deadlines.

`WeaponState` replicates current stock, the fire-cooldown deadline, and an optional
`AmmoRecovery { started_at_tick, ready_at_tick }`. The client interpolates the filling segment from
that interval and the latest replicated authoritative tick, but only replicated stock is usable.
A locally interpolated segment reaching its visual endpoint cannot create ammunition or authorize
a shot; the next server update confirms the restored unit.

They exercise reusable composition primitives and are not permanent weapon classes. In V7 they
become the initial permanent weapon-base choices; the named Runner, Bruiser, Controller, and Duelist
builds disappear rather than becoming starter templates. The bounded custom Pulse proves that a
non-preset selection resolves into the same operational representation and provides a migration
seam for authored starter part effects.

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

ProjectileBody
  Shape of the authoritative planar collision footprint

StraightFlight / future trajectory component
  Motion of that footprint, independently of its shape
```

Straight deliveries move through authoritative planar simulation. Lobbed deliveries resolve a
bounded landing point and flight deadline while clients may present an arc independently. The
weapon catalog's recipe policy owns the minimum lob-flight deadline; each Lobbed or Splash recipe
owns its maximum flight deadline, and validation rejects a recipe whose maximum cannot satisfy the
catalog floor. Melee arcs are deliveries without projectile entities. The fighter does not own the
delivery after creation; stable source identity preserves attribution across entity lifecycles.

A cone spray is also a delivery without projectile entities. Attack acceptance immediately fixes
an immutable world-space origin and facing, then a replicated gas volume grows from that point at
the authored propagation speed, remains through its bounded linger interval, and applies payloads
on authoritative pulse ticks. Moving or turning the fighter after firing never moves the spray.
Static and live blocking geometry clip only the angular rays they obstruct; other parts of the cone
continue to propagate. Occlusion is recomputed for every pulse, so destroying or removing a blocker
opens that angular portion up to the spray's current global reach without restarting propagation.
The client reconstructs the same clipped cone for presentation and evidence but never owns hits.

Spray damage can fall off with distance from its captured source. **Falloff start** is the distance
through which each pulse retains full damage. Between **Falloff start** and **Falloff end**, damage
decreases linearly. At and beyond **Falloff end**, **Minimum damage scale** is applied; a scale of
`0.5`, for example, means half damage. This changes pulse strength only—it does not change how far
the gas travels, which targets overlap the cone, or how geometry clips it.

The current straight body is `ProjectileShape::Circle { radius }`. The server constructs its Avian
collider and every fixed-tick sweep from that body and replicates it once with the projectile. The
client uses the same body for the visible solid footprint and local aim corridor. Shape and
trajectory are separate axes: a later capsule or rectangle does not imply a new movement family,
and a later curved or homing trajectory does not redefine collision geometry. A new shape is
complete only when authored validation, authoritative collider/sweep, replication, matching visual,
and matching aim trace are implemented together.

Eligible V10 world targets reuse these accepted deliveries without becoming fighters. Straight,
lobbed, and melee contact may apply positive damage to a live barrel or chest; hostile straight,
lobbed, and melee contact may damage a Heist objective according to its mode-owned policy. Status,
knockback, fighter passives, charge, and defeat semantics do not transfer merely because a target
has health. The complete policy is owned by
[Damageable world objects and Heist](./18-damageable-world-objects-and-heist.md).

Zombie hostile NPCs are a distinct combat-target family rather than enemy fighters. Player-owned
straight, lobbed, spray, splash, sticky, melee, sentry, field, and condition deliveries may target
them; authored slow, cold/freeze, damage-over-time, and external motion remain authoritative.
Fighter-only passives, healing, scoring, and build state do not transfer. Carried Zombie boost
stacks scale player-owned outgoing damage exactly once at effect planning and scale maximum health
through the shared effective-health helper; NPC and environment damage never inherit that bonus.

Possible future delivery families include:

- charged or delayed projectiles with warning telegraphs;
- beams or repeated ray ticks;
- traps and moving or channeled fields;
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

The fighter-origin-to-muzzle clearance segment follows the same first-contact contract as the
projectile sweep. If a live damageable object blocks that segment, the direct payload applies to
that first object and no projectile is created beyond it. Static cover still blocks the delivery
without becoming a damage target.

For straight weapons, the local aiming presentation sweeps the resolved `ProjectileBody` from the
fighter to the muzzle and then through maximum range. Its corridor is one projectile diameter wide,
starts at the muzzle when clearance succeeds, and terminates at the first client-observed
projectile blocker. It uses projectile collision policy—player-only water does not clip it—and may
include currently replicated hostile fighters and sentries. This is readability over observed
state, not hit authority or latency prediction; only the server sweep decides the result.

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
  lifetime, destruction, and lifecycle cleanup;
- **Self Cloak, Reveal Scan, and Concealment Field:** the concealment/reveal families described
  below;
- **Demolition Strike:** an instant server-authoritative targeted terrain brush with 520 world-unit
  maximum range and a 64 world-unit destruction radius. It spends full ultimate charge on any
  accepted activation, including a valid area where no destructible placement changes. It does not
  damage fighters or world objects;
- **Cryogenic Field, Fire Field, Poison Field, and Restoration Field:** targeted persistent regions
  that pulse the corresponding typed combat effect; and
- **Big Blob:** a targeted parent lob that emits six independently authored Sticky Blomb
  secondaries at landing.

The supported passive set contains:

- Lightweight Frame and Reinforced Frame, which scale the character's maximum health and movement
  speed by authored basis points (8,500 / 11,000 and 12,000 / 9,000);
- Adrenal Response;
- Close Quarters;
- Quick Cycle;
- Tenacity;
- Cryogenic Insulation;
- Filtered Circulation; and
- Heat Shielding.

A loadout carries exactly one passive, chosen from its character's two options. Passives should
alter a meaningful decision, range, risk, or timing window rather than merely raise every number. Their authored definitions grant bounded modifiers or typed capabilities; runtime
systems read resolved grants and do not branch on character identity.

Possible future ultimate families include shielding, healing or repair fields, area pull or
knockback, and temporary wall placement. Each should be introduced as a complete readable gameplay
slice rather than as unused framework capacity.

The supported elemental family adds four mutually exclusive weapon modules: Cryogenic builds a
meter that freezes at its threshold, Incendiary and Toxin refresh bounded fire and poison damage,
and Healing converts a straight single shot into allied healing. Poison suppresses attack-idle
health recovery while active. Cryogenic Insulation, Filtered Circulation, and Heat Shielding are
mutually exclusive resistance passives; each reduces only its named buildup or damage-over-time
contribution.

Every roster character owns an independently tunable Cold capacity plus Cold, Poison, and
Fire resistance baselines. The launch characters intentionally begin at 1,000 Cold capacity and 0%
for each resistance, but equal defaults are not a shared global rule. Cold contributions apply the
target's resolved Cold resistance and accumulate against that target's resolved capacity. The three
resistance passives add 30 percentage points to their matching baseline, clamped at 60%, rather
than replacing it. Fire and Poison do not have capacities: they remain duration-based
strongest-refresh damage-over-time conditions, so resistance is their fighter-owned tuning surface.
The shared Cold lifecycle is separately authored as global combat-condition content: buildup waits
90 ticks before decaying by 10 per authoritative tick, Freeze lasts 60 ticks, and thaw grants 90
ticks of immunity by default. Balance Lab exposes these four global rules independently from
fighter capacity and resistance; server systems consume the installed rule resource rather than
code constants.
While a fighter has positive Cold buildup, its projected overhead shows a compact cyan radial meter
beside the health bar. The visual has no numeric label, uses the resolved target capacity as its
denominator, and disappears when the buildup meter returns to zero or is consumed by Freeze.

### Sticky Blomb and Big Blob

Sticky Blomb is a straight primary projectile whose first fighter, obstacle, objective, or maximum-
range contact arms a delayed one-shot area explosion. A fighter hit carries the blob and its future
blast center until detonation; other contacts anchor it in the world. Its explosion deliberately
ignores map occlusion while retaining hostile-recipient and target-eligibility rules. A second
primary hit on the same carrier immediately detonates the existing primary and attaches the new
blob. Ultimate secondary blobs do not trigger that chain rule.

Every armed blob replicates its current center, source role, fuse interval, and authored radius.
Clients render the full future-blast boundary plus a nonnumeric fill that grows with fuse progress;
the telegraph follows an attached carrier and remains distinct from persistent field effects.

Big Blob is an independently selectable targeted lob ultimate. The accepted landing uses the common
range, bounds, gamepad distance, and map-clearance contract. Landing deals no damage itself: it
consumes the parent and emits exactly six straight secondary Sticky Blombs at fixed 60-degree world-
space intervals. Primary and secondary speed, range, collision size, fuse, blast radius, damage, and
active ceilings are authored values rather than presentation or runtime constants.

Cryogenic, Fire, Poison, and Restoration Field are targeted ultimates. Their replicated regions
pulse immediately and on a fixed interval, use the same elemental rules as weapon delivery, remain
after owner defeat or disconnect, and expire or clear on restart/build replacement. Direct hits,
field pulses, poison ticks, fire ticks, and defeat resolution have a stable authoritative order.

### Splash persistent areas

Splash is a targeted primary lob that becomes a stationary persistent combat area on its validated
landing. The authored delivery owns either circular or oriented rectangular geometry, duration,
pulse interval, map occlusion, target ceiling, and active-area ceiling. It pulses immediately on
landing and at fixed authoritative deadlines through expiry. Clients render and recover replicated
geometry and timing facts but never report occupancy or effect application.

The canonical preset places a 96-unit-radius circle up to 480 units away. It lasts 240 ticks and
pulses every 30 ticks, including both its landing and expiry deadlines, for nine pulses total. Each
owner may have at most two Splash deliveries across flight and active-area state, and a match may
hold at most 16 active areas. Each pulse considers no more than six live fighters, ordered by
distance and stable network identity after footprint intersection and current map-occlusion checks.

One Splash area carries one bounded payload bundle with at most two distinct typed effects. Each
effect retains its ordinary owner/ally/enemy recipient policy and composed-combat semantics. The
initial Splash damages eligible hostiles while healing its owner and allies on the same pulse;
healing remains clamped, cannot resurrect, and grants no damage-based ultimate charge. Cold,
Fire/Poison, Slow, Damage, and Heal reuse their existing target-owned rules and attribution rather
than creating area-specific effect implementations.

An accepted area remains after its owner is defeated or disconnects. Match completion, restart,
generation replacement, or expiry removes it. The server replicates the stable attack source plus
public center, facing, shape, activation/next-pulse/expiry ticks, interval, occlusion rule, target
ceiling, and effect identities. Private recipe/runtime state and occupancy never cross the wire.

V9 delivered three additional ultimate families: a self cloak that is permanently consumed by an
accepted attack or positive applied damage, and an instant targeted reveal scan that applies a
team-wide forced-reveal deadline to hostile fighters in its accepted area, plus a public targeted
allied concealment field. Their shared observer-specific rule, proximity exclusions,
attack/damage locks, counter relationship, privacy boundary, and staging are owned by the
[Concealment and reveal specification](./17-concealment.md) and
[completed V9 roadmap](./implementation/v9/roadmap.md).

## Active-item extension

Active items are not part of the supported loadout. Candidate capabilities include a short speed
burst, emergency shield, reveal pulse, partial reload, or deployable decoy. Adding the slot requires:

- an abstract input action and controller/keyboard parity;
- authoritative activation, cooldown, rejection, and cleanup;
- a clear HUD deadline or charge indicator;
- distinct audiovisual and controller feedback;
- loadout cost, slot, conflict, and compatibility rules;
- replication, reconnect, restart, and process-evidence coverage.

Authored weapon parts and the retired collectible direction are described in
[Fighter and build specification](./02-fighter-model.md#weapon-parts-as-authoring-primitives). Combat
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
- Which character or slot opportunity does it take, and what does it displace?
- Can its damage, control, ownership, and expiry be understood from feedback?
- Does it create a reason to move or make a decision rather than hold one position?
- Does it remain bounded under many participants, deliveries, or persistent objects?
