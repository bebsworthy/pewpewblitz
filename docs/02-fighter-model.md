# Fighter model

## Data categories and lifecycles

Keep four kinds of data distinct because they have different authors, validation rules, and
lifecycles. This is a data-model distinction, not a requirement for separate crates or
architectural layers:

1. **Content and rule definitions:** developer-authored body values, available weapon primitives,
   legal bounds/combinations, slots, effects, and presentation references.
2. **Brawler build:** player-authored choices, including a compositional weapon recipe. Built-in
   presets are ordinary legal builds authored by the team.
3. **Resolved match loadout:** the immutable, server-validated snapshot used to instantiate a
   fighter for a match.
4. **Runtime state:** health, ammo, cooldowns, effects, position, and team during a match.

```text
FighterDefinition
  BaseStats
  SlotRules
  PresentationDefinitionId

BrawlerBuild
  WeaponRecipe
  Ultimate
  PassiveItems
  ActiveItems

ResolvedMatchLoadout
  StableBuildIdentity / Revision
  ValidatedWeaponConfiguration
  ResolvedBody / Ability / Item Choices

FighterState
  Position / facing
  Health / shields
  Ammo / reload timers
  Ability charge / cooldowns
  Active effects
  Team / alive state
```

`PresentationDefinitionId` is a stable reference that client presentation resolves to visual and audio assets. The authoritative server does not need to load those assets.

The resolved match loadout is gameplay data, not mutable combat state. The authoritative server
creates it only after resolving the requested preset or stored brawler against the current content
catalog, balance policy, and—when accounts exist—ownership/entitlement data. A client may propose a
recipe in a future editor flow, but it cannot declare that recipe legal or directly install resolved
values on a fighter.

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

Vision radius and concealment/reveal behavior are future information rules, not client-rendering
preferences. When implemented, the server derives visibility for each observer and subject and uses
network interest management to withhold secret live spatial state. See
[Environment, surface, and tile ideas](./09-environment-and-tile-ideas.md#concealment-gameplay-model).

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

- one compositional primary-weapon recipe;
- one ultimate;
- two passive item slots;
- one active item slot only when playtest evidence justifies adding it after the combat sandbox is stable;
- a fixed number of build points;
- mutually exclusive item families where combinations create obvious balance problems.

The four initial weapons are presets constructed from the same recipe representation and resolver;
they are not permanent hard-coded weapon classes. Milestone 08 must exercise at least one bounded
non-preset weapon variation so the first product iteration tests customization rather than only
preset selection. The exact budget allocation and editor interaction are specified when that
milestone becomes next.

Do not allow unrestricted free allocation of every numeric attribute. Players should make a few
legible decisions, and the server must resolve each choice against explicit bounds and compatibility
rules.

## Arsenal lifecycle

Eventually a human player owns an arsenal of brawlers. Each saved brawler references a stable build
identity and revision and contains its weapon recipe plus the rest of its loadout. Selecting a
brawler for a match is intent: the server retrieves or receives the candidate build, validates it,
and creates an immutable resolved match loadout before the fighter becomes active.

The v1 combat sandbox does not require accounts or persistence. Milestone 05 uses four built-in
preset recipes to establish the composition/resolution/runtime boundaries. Milestone 08 introduces
bounded in-memory build customization. Persistent arsenal storage, editing history, acquisition,
currency, loot, and unlock policy remain later work and must not leak into combat systems.

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
