# Fighter model

## Data categories and lifecycles

Keep three kinds of data distinct because they have different authors, validation rules, and lifecycles. This is a data-model distinction, not a requirement for separate crates or architectural layers:

1. **Definition:** authored base values and available slots.
2. **Build:** the selected weapon, ultimate, and items.
3. **Runtime state:** health, ammo, cooldowns, effects, position, and team during a match.

```text
FighterDefinition
  BaseStats
  SlotRules
  PresentationDefinitionId

Build
  Weapon
  Ultimate
  PassiveItems
  ActiveItems

FighterState
  Position / facing
  Health / shields
  Ammo / reload timers
  Ability charge / cooldowns
  Active effects
  Team / alive state
```

`PresentationDefinitionId` is a stable reference that client presentation resolves to visual and audio assets. The authoritative server does not need to load those assets.

## Attribute inventory

### Survivability

- maximum health;
- current health;
- armor or damage reduction;
- shield capacity and recharge;
- healing received multiplier;
- regeneration delay and rate;
- knockback resistance;
- status resistance.

### Mobility

- movement speed;
- acceleration and stopping response;
- turn rate;
- dash distance and duration;
- dash cooldown;
- movement while attacking;
- terrain permissions.

### Weapon performance

- damage;
- shots or pellets per attack;
- ammo capacity;
- reload time;
- attack cooldown;
- charge time;
- range;
- projectile speed;
- projectile width;
- spread;
- damage falloff;
- hit-stun or impact force.

### Ability economy

- ultimate meter maximum;
- charge gained from damage, healing, proximity, time, or objectives;
- active-item charges;
- cooldowns;
- duration;
- resource cost.

### Internal status state

Some effects should not trigger immediately. A fighter may accumulate an internal status meter from multiple attacks, zones, or allied weapons. The meter belongs to the target fighter, while any weapon or ability may contribute to it.

```text
StatusState
  StatusId
  CurrentValue
  Threshold
  DecayDelay
  DecayRate
  TriggeredState
  TriggerCooldown
  Resistance / Immunity
```

Example:

```text
Ice pellet       → +cold on hit
Ice zone         → +cold per second while inside
Ally ice weapon  → +cold from a different source
Cold threshold   → frozen for a defined duration
```

The status behavior should support contribution from multiple sources, accumulation over time, decay after exposure, threshold-triggered effects, and temporary immunity or resistance after triggering. The first systemic-status milestone in a future version should implement one status meter such as `cold` with focused components, resources, and systems grouped in a cohesive plugin rather than starting with a large generalized framework.

### Information and interaction

- vision radius;
- concealment or reveal behavior;
- targeting priority;
- pickup radius;
- objective interaction speed;
- objective damage multiplier;
- carry capacity.

## Recommended MVP attributes

Only these should affect balance in the first product iteration through v1 Milestone 08:

- maximum health;
- movement speed;
- weapon damage;
- weapon range;
- projectile speed;
- reload time;
- ammo capacity;
- ultimate charge;
- one defensive modifier;
- one mobility modifier.

Armor, critical hits, lifesteal, terrain permissions, vision manipulation, and complex status resistance should wait until the base combat loop is measurable.

## Loadout rules

Initial build rules, introduced in v1 Milestone 08 after the combat vertical slice is stable:

- one primary weapon;
- one ultimate;
- two passive item slots;
- one active item slot only when playtest evidence justifies adding it after the combat sandbox is stable;
- a fixed number of build points;
- mutually exclusive item families where combinations create obvious balance problems.

Do not allow unrestricted free allocation of every numeric attribute. Players should make a few legible decisions.

## Roles as outcomes, not classes

Avoid hard-coded classes initially. Roles should emerge from build choices:

- **Skirmisher:** mobile, medium range, consistent damage;
- **Bruiser:** durable, short range, disruptive;
- **Marksman:** fragile, long range, high accuracy reward;
- **Controller:** area denial and crowd control;
- **Support:** healing, shielding, or team utility.

These labels are useful for analytics and onboarding, but they should not constrain the simulation.

Useful overlapping role tags include Tank, Assassin, Marksman, Artillery, Damage Dealer, Support, and Controller. A build may carry more than one tag, such as `Tank + Controller` or `Assassin + Damage`. Tags should describe matchup expectations rather than unlock separate rule systems.

## Status and interaction library

Keep the first status library small and reusable:

- damage;
- healing;
- shield;
- slow;
- stun or interrupt;
- knockback;
- damage over time;
- mark or reveal;
- persistent zone;
- summon.

These primitives can support many weapons and abilities without creating a bespoke implementation for every item.
