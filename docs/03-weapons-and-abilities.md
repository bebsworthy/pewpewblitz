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
  Supported preset identity or bounded typed specification
  Permanent weapon-base identity plus four equipped part instances

WeaponConfiguration
  Operational recipe plus an approved presentation-profile reference

WeaponPreset
  Named developer-authored legal configuration

ResolvedWeapon
  Immutable server-validated configuration, identity, and fingerprint

WeaponState
  Ammo or charges, fire cooldown, and next-ammunition recovery interval during play
```

The supported selection contract does not submit a low-level operational recipe. The custom Pulse
exposes discrete power, reach, and magazine choices; the server converts them into a configuration
and then runs the ordinary weapon validator. Future editors may expose more typed primitives, but
clients must never select executable systems, presentation assets, arbitrary serialized components,
scripts, or unbounded numeric maps.

Built-in presets and player-authored selections use the same configuration validation and runtime
execution paths. Preset identity may affect naming, onboarding, analytics, or presentation lookup;
it must not select a separate combat implementation.

## Weapon-base and part customization direction

The intended long-lived brawler model fixes one fighter profile and one weapon base when the player
creates that brawler. The fighter-profile identity and persistence lifecycle belong to
[Fighter and build specification](./02-fighter-model.md); this document owns how the fixed weapon
base and its equipped parts resolve into an operational weapon. The current six reference weapon
presets are the initial weapon bases. Lobby authority advertises their stable IDs, display names,
presentation profile keys, and validated base configurations as part of the connection-scoped
brawler catalog. More bases may be added as complete playable recipes without changing the
part-slot model or adding a client-owned inventory table.

The current canonical base values affected by ordinary play balance are:

| Weapon | Economy | Recovery | Firing | Delivery | Direct damage |
|---|---|---:|---|---|---:|
| Pulse Sidearm | 4-shot magazine | 1.0 s/round | Single | 500 speed, 2 radius, 320 range | 200 |
| Scatter Cannon | 3-shot magazine | 1.2 s/round | 5 over 30° | 600 speed, 2 radius, 320 range | 120/projectile before falloff |
| Arc Launcher | 3-shot magazine | 1.6 s/round | Single | 520-distance lob | 40 area |
| Impact Blade | 3 charges | 1.0 s/charge | Single | 120-reach melee arc | 34 |
| Sticky Blomb | 3-shot magazine | 1.5 s/round | Single | 320-range sticky projectile | 36 impact plus persistent pulses |
| Spray | 3-shot magazine | 1.5 s/round | Single spritz | 480-speed, 240-reach, 70° cone | 40/pulse before falloff |

Durations are displayed in seconds but remain positive 60 Hz fixed-tick values in authoritative
content. Editors accept ordinary decimal seconds and save the nearest fixed tick; for example,
`0.17 s` becomes 10 ticks. The embedded catalogs, not this summary, are the executable source of
truth.
Arc Launcher damage no longer destroys terrain. Generic delivery-level `DestroyMap` remains a
validated weapon capability for future authored recipes, but no built-in weapon currently grants
it.

Every weapon has exactly four interchangeable part slots:

```text
SavedBrawlerWeapon
  Permanent WeaponBaseId
  EquippedPartInstanceIds [0..4]
```

A slot has no type, family, or gameplay role. Any legal weapon part may occupy any free slot, and
moving the same equipped parts between slot positions must not change the resolved weapon or its
gameplay fingerprint. One owned part instance cannot occupy more than one slot at the same time.

Part type, generated name, icon, and model profile are presentation and inventory metadata. For
example, a part presented as `Heavy Magazine of Frosting` may sort under a `Magazine` inventory
label, while its authoritative effects grant two rounds, add a bounded frost effect, and reduce
firing speed. Neither the `Magazine` label nor the generated name grants, restricts, or selects
gameplay behavior. V7 does not render equipped parts on the in-match fighter or weapon; composited
part models are a later presentation improvement and therefore have no mount-conflict rule today.

Keep ownership, presentation, generated properties, resolved gameplay, and runtime state distinct:

```text
WeaponBaseDefinition
  Stable identity
  Complete legal base WeaponConfiguration

WeaponPartInstance
  Stable player-owned instance identity
  Presentation metadata
  Bounded persisted authored effect selections

EquippedWeaponParts
  Up to four owned part-instance identities

ResolvedWeapon
  Base configuration after canonical part-effect resolution and ordinary validation

WeaponState
  Mutable match economy and deadlines; no inventory or part-instance state
```

Acquisition source—such as a level reward, loot box, purchase, or trade—only determines how an
account obtains a part instance. It must not change effect execution. V7 seeds fixed authored starter
parts and caps an account at 128 part instances; acquisition systems remain deferred. Generation and
persistence are server-owned: clients may propose owned instance identities but cannot submit
arbitrary effect values, recipes, presentation assets, or generated names as authoritative data.
Persisted rolls survive balance changes unless an explicit versioned migration changes them; content
updates never silently reroll owned parts.

Part effects use a closed typed vocabulary over implemented weapon properties and capabilities. V7
implements bounded flat or percentage changes to capacity, damage, fire interval, refill/recharge
interval, semantic reach, and one bounded Slow contribution. It does not yet expose projectile
speed/radius, spread, knockback, or new payload/status families. The vocabulary must not use string
field paths, scripts, arbitrary numeric maps, or serialized ECS components. A new effect kind is
added only with explicit combination, validation, presentation, lifecycle, and verification rules.

Resolution is deterministic and independent of slot position:

1. Load the permanent weapon base and the four or fewer equipped instances from server-owned data.
2. Validate ownership, uniqueness, revisions, bounds, and applicability to the base recipe.
3. Canonically order the equipped effects and aggregate each property using its declared stacking
   rule. Numerical modifiers sum flat values, apply the combined percentage, then clamp and round
   once. Repeated status contributions aggregate into one bounded effect per status kind.
4. Apply the aggregate to a copy of the base `WeaponConfiguration`.
5. Run the ordinary structural, capability, compatibility, balance, and engine-ceiling validator.
6. Produce the existing immutable `ResolvedWeapon` and canonical gameplay fingerprint.

An equipped part whose effect cannot apply to the selected weapon base makes the candidate invalid;
the resolver must not silently discard an authoritative effect. Inventory UI may prevent or explain
that selection, but the server remains the legality authority. Combat, movement, networking, and
presentation systems consume the resolved recipe and grants rather than querying inventory records,
part names, cosmetic types, acquisition history, or instance identity.

V7 removes the shared 12-point budget. Its balance controls are the four-slot ceiling, bounded
effect ranges, applicability validation, engine caps, and authored sidegrade tradeoffs. Empty slots
are legal. Distinct instances of one part definition may be equipped together; the same owned
instance may not appear twice.

The current custom Pulse power, reach, and magazine choices are a useful first migration seam: their
existing bounded changes can become starter part effects while retaining the same base recipe,
validator, `ResolvedWeapon`, and combat execution. Accumulating frost is a separate target-owned
status capability; until that capability is implemented, a frost-themed part may only grant an
already-supported bounded effect such as slow, with presentation text that accurately describes the
actual rule.

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
   fighter compatibility, mutual exclusions, required tradeoffs, and—only for the current pre-V7
   build contract—the build budget. V7 weapon-part resolution has no point budget.
3. **Entitlement validation:** the account may use the selected options. This belongs to a future
   arsenal/account boundary, not to combat execution.

Code-owned engine ceilings bound collection sizes, numeric fields, target counts, combat-effect deadlines, and
serialized resolved data. Authored policy may narrow those limits but cannot widen them. Stable
fingerprints derive from canonical operational data, not local ECS identity or display names.
Per-ammunition refill/recharge duration has no gameplay tuning ceiling; it must only be positive
and representable by the authoritative tick deadline.

Presentation-profile IDs are approved as part of server-known content or server resolution. A
client may request a legal cosmetic choice when that product capability exists, but it cannot attach
an arbitrary presentation profile to change combat readability or asset loading.

## Supported weapon capability set

The established weapon foundation supports:

- magazine and charge economies;
- single and spread firing patterns;
- straight projectile, lobbed projectile, melee-arc, sticky-area, and propagating cone-spray delivery;
- direct and bounded area target selection;
- damage, knockback, and strongest-refreshes slow effects;
- no falloff and linear damage falloff;
- hostile and bounded hostile-plus-owner recipient policies;
- one bounded terrain-destruction world effect;
- explicit positive-damage eligibility for live environment objects and hostile Heist objectives;
- authoritative fire cooldown, one-at-a-time ammunition recovery, lifetime, collision, and outcome attribution;
- stable presentation cues resolved independently by the client.

The six reference presets are:

| Weapon | Pattern | Strength | Cost or weakness |
|---|---|---|---|
| Pulse sidearm | Single direct projectile | Reliable mid-range pressure | Low burst |
| Scatter cannon | Short spread of pellets | Excellent close-range burst | Poor range and falloff |
| Arc launcher | Lobbed splash projectile | Punishes cover and groups | Slow delivery and recovery |
| Impact blade | Melee arc | Strong duel pressure and displacement | Must enter danger range |
| Sticky Blomb | Sticky projectile and persistent area | Denies a bounded position | Delayed repeated value |
| Spray | Instant stationary cone spritz | Sustained close-range area pressure | Must commit aim and origin at firing time |

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
bounded landing point and flight deadline while clients may present an arc independently. Melee arcs
are deliveries without projectile entities. The fighter does not own the delivery after creation;
stable source identity preserves attribution across entity lifecycles.

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
  damage fighters or world objects.

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

The supported elemental family adds four mutually exclusive weapon modules: Cryogenic builds a
meter that freezes at its threshold, Incendiary and Toxin refresh bounded fire and poison damage,
and Healing converts a straight single shot into allied healing. Poison suppresses attack-idle
health recovery while active. Cryogenic Insulation, Filtered Circulation, and Heat Shielding are
mutually exclusive resistance passives; each reduces only its named buildup or damage-over-time
contribution.

Every authored fighter profile owns an independently tunable Cold capacity plus Cold, Poison, and
Fire resistance baselines. The current profiles intentionally begin at 1,000 Cold capacity and 0%
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
