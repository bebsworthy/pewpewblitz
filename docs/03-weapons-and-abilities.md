# Weapons and abilities

## Weapon composition

Represent a weapon as a composition of input, firing, delivery, collision, and payload rules.

```text
Weapon
  InputBehavior
  FiringPattern
  DeliveryMethod
  CollisionPolicy
  Payloads
  Economy
```

## Projectile model

Model projectile behavior as composable authored data, ECS components, and focused systems so trajectory, collision, gameplay payload, and presentation can evolve separately. These concerns do not imply separate crates or service layers.

```text
Projectile
  Trajectory
  Movement
  CollisionPolicy
  Lifetime
  ImpactRules
  Payloads
  PresentationCueIds
```

Presentation cue IDs are stable gameplay-facing references. Client systems resolve them to visual, audio, camera, or controller feedback; authoritative projectile systems do not load or mutate presentation assets.

### Trajectory types

- straight/direct;
- ballistic/lobbed arc;
- curved or steered;
- homing;
- boomerang/returning;
- bouncing or ricocheting;
- piercing;
- delayed or timed;
- stationary/deployable.

The first implementation only needs straight and ballistic trajectories. The selected definition/component representation should allow later trajectory systems without rewriting fighter behavior or unrelated weapon data.

### Impact rules

A projectile may:

- stop on the first collision;
- bounce a limited number of times;
- pierce a limited number of targets;
- split into additional projectiles;
- return to its owner;
- explode in an area;
- create a persistent damage, healing, or control zone;
- apply a destruction brush to terrain;
- trigger a summon or deployable.

Impact rules should be data-driven and expose authoritative outcomes through gameplay components or registered messages as appropriate. Their systems must not load or mutate particles, sounds, camera shake, or UI assets.

### Presentation effects

Client presentation systems may observe replicated gameplay components or registered gameplay messages and produce independent presentation effects:

- muzzle flash;
- projectile trail;
- impact flash;
- explosion animation;
- debris and particles;
- hit marker;
- screen shake;
- sound effect;
- terrain crater edge.

Presentation effects can be placeholders during the MVP and replaced later without changing authoritative collision or damage systems.

### Input behavior

- instant fire;
- hold to charge;
- channel while held;
- release to fire;
- quick-fire toward an automatically selected target;
- aim-and-release.

### Firing patterns

- single shot;
- burst;
- cone spread;
- radial burst;
- repeated beam ticks;
- lobbed area attack;
- deployable.

### Delivery methods

- melee arc;
- melee dash;
- direct projectile;
- arcing projectile;
- beam or ray;
- persistent area;
- trap;
- turret;
- summon.

### Payloads

- direct damage;
- damage over time;
- healing;
- shield;
- slow;
- stun;
- root;
- knockback or pull;
- reveal;
- buff or debuff;
- terrain modification;
- summon creation.

### Effects and status meters

An impact may apply an immediate effect or contribute to a target's internal status meter.

Immediate effects include:

- slow;
- stun or interrupt;
- knockback or pull;
- reveal or mark;
- damage over time;
- healing reduction;
- shield break;
- silence or ability lockout.

Every immediate effect needs explicit duration, stacking or refresh behavior, ownership/attribution, and cleanup rules. Effects must resolve consistently on expiry, defeat, respawn, disconnect, match restart, and removal of their source. These lifecycle rules belong to the reusable effect system rather than individual weapon scripts.

Accumulating effects use a status contribution instead of directly triggering the final effect:

```text
EffectApplication
  StatusId
  Amount
  Duration / TickInterval
  FalloffRule
  SourceTags
  TriggerPolicy
```

The target owns the current status value. Different weapons and abilities with the same `StatusId` contribute to the same meter, including direct hits and persistent zones.

Example:

```text
Ice pellet  → cold + 12 on hit
Ice zone    → cold + 4 every 0.25 seconds while inside
cold >= 100 → frozen for 1.5 seconds
```

The threshold effect should define what happens to the meter after triggering: reset, partial reduction, lockout, or temporary immunity. These rules must be explicit so players can understand why a target did or did not freeze.

Terrain destruction is a world-level payload. A weapon emits a destruction brush—such as a circle, capsule, rectangle, or authored shape—with a position, size, and optional material effect. The terrain subsystem applies that brush to its mask and regenerates affected collision chunks. Weapons must not know about tiles or directly edit collision polygons.

## Projectile phase scope

### Combat vertical slice and first product iteration

- straight pulse projectile;
- short-range pellet spread;
- ballistic/lobbed splash projectile;
- circular explosion payload;
- basic projectile trail and impact feedback;
- collision with fighters, terrain, and objectives.

### Later projectile content

- bouncing and ricocheting;
- homing and curved/steered paths;
- boomerang projectiles;
- piercing and projectile splitting;
- delayed projectiles and warning telegraphs;
- richer particle systems, debris, camera shake, and material-specific impact effects.

## Status interaction phase scope

### Combat vertical slice and first product iteration

- immediate effects may be represented by simple payloads such as damage, knockback, or a basic slow;
- no accumulating status meter is required for the first combat sandbox.

### Systemic-status milestone

- target-owned status meters;
- contributions from multiple weapon types and persistent zones;
- decay delay and decay rate;
- threshold-triggered effects;
- resistance, immunity, and trigger cooldowns;
- one complete cold-to-freeze interaction as the reference implementation.

## Initial weapon set

| Weapon | Pattern | Strength | Cost or weakness |
|---|---|---|---|
| Pulse sidearm | Single direct projectile | Reliable mid-range pressure | Low burst |
| Scatter cannon | Short cone of pellets | Excellent close-range burst | Poor range and falloff |
| Arc launcher | Lobbed splash projectile | Punishes cover and groups | Slow projectile and reload |
| Impact blade | Melee arc | Strong duel pressure and displacement | Must enter danger range |
| Optional fifth: charge rifle | Charge-and-release projectile | Long-range accuracy reward | Vulnerable while charging |

The first four are sufficient for the initial combat test. Add the charge rifle only if the map provides enough sightlines to evaluate it fairly.

## Ultimate abilities

Use the same compositional model as weapons. Initial candidates:

- forward dash with impact damage;
- temporary personal shield;
- healing or repair field;
- deployable sentry;
- area pull or knockback;
- short-lived wall placement.

Implement only two in v1 Milestone 08: a dash and a bounded-lifetime deployable sentry. They exercise mobility, collision, targeting, ownership, lifetime, cleanup, and counterplay. Every named first-iteration preset must be constructible from these two ultimates and the implemented passive inventory.

## Passive items

Passives should alter a decision or timing window, not simply increase every number.

Good first candidates:

- gain movement speed briefly after taking damage;
- gain a small shield after using the ultimate;
- reload the next shot faster after an elimination;
- increase damage within short range while reducing long-range damage;
- reduce incoming crowd-control duration;
- improve objective interaction speed while below half health.

## Active items

Active items are optional after the core loop works and are not required for the first product-iteration gate. Candidates:

- short burst of speed;
- emergency shield;
- temporary reveal pulse;
- instant partial reload;
- deployable decoy.

Every active item needs a clear cooldown indicator and an obvious audiovisual response.

## Balance checklist for each weapon

- What distance does it want?
- What does it do when an opponent closes distance?
- What map geometry counters it?
- What is its burst window?
- What is its recovery window?
- How much aim precision does it demand?
- Can its damage be understood from the hit feedback?
- Does it create a reason to move rather than hold one position?
