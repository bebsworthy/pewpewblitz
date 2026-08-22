# Fighter and build specification

## Purpose and authority

This document defines the durable fighter, brawler-build, loadout-resolution, and target-owned
runtime-state contracts. It distinguishes the supported gameplay foundation from envisioned product
extensions without treating the current implementation as the limit of the design.

[Weapons and abilities](./03-weapons-and-abilities.md) owns combat selections, operational weapon
recipes, delivery, payloads, and ability execution. [Network architecture](./08-network-architecture.md)
owns replication and authority boundaries. Versioned implementation documents retain delivery
history and verification evidence.

## Terms and data lifecycles

Keep authored definitions, player choices, accepted identity, resolved gameplay data, and mutable
runtime state distinct. They have different authors, validation rules, and lifecycles; this is a
data-model distinction, not a requirement for separate crates or architectural layers.

1. **Fighter and content definitions:** developer-authored body values, compatible capabilities,
   legal bounds, slots, effects, costs, and stable presentation references.
2. **Brawler build recipe:** a bounded player choice composed from stable definition IDs and typed
   specifications. A built-in preset is an ordinary legal recipe authored by the team.
3. **Selected build identity:** the accepted preset identity or canonical recipe fingerprint and
   revision used across process and network boundaries.
4. **Resolved match loadout:** the immutable, server-validated gameplay snapshot used to instantiate
   a fighter for one match.
5. **Runtime fighter state:** mutable ECS state such as health, pose, weapon economy, ability charge,
   effects, team, and lifecycle phase.

```text
FighterDefinition
  Stable definition identity
  Base fighter values
  Compatibility and slot rules
  Stable presentation reference

BrawlerBuildRecipe
  Bounded weapon selection or specification
  Ultimate selection
  Two passive selections
  Future equipment selections, when that capability exists

SelectedBuild
  Preset identity, when applicable
  Canonical recipe fingerprint
  Balance/content revision

ResolvedMatchLoadout
  SelectedBuild identity
  Resolved fighter stats
  Resolved primary weapon
  Resolved ultimate
  Resolved passive grants

FighterRuntime
  Position and facing
  Current health and shields
  Weapon economy and deadlines
  Ability charge and execution phase
  Passive and status runtime state
  Team and fighter lifecycle
```

Stable presentation references are gameplay-facing IDs that client presentation resolves to visual
and audio assets. The authoritative server validates those references where needed but does not load
the assets.

## Supported build contract

The supported foundation is:

- one primary-weapon selection;
- one ultimate selection;
- exactly two passive selections;
- a fixed 12-point budget shared by the weapon, ultimate, and passives;
- duplicate and incompatible-family rejection;
- four built-in build/weapon presets resolved through the same paths as non-presets;
- one bounded custom Pulse specification with discrete power, reach, and magazine choices.

The player-facing selection is not the operational weapon recipe. It contains bounded IDs and typed
choices; the server derives the numeric weapon configuration described in
[Weapons and abilities](./03-weapons-and-abilities.md). This prevents clients from directly choosing
unbounded damage, collision behavior, lifetimes, presentation profiles, or other authoritative
values.

An active-item slot is not part of the supported loadout. It is a possible future extension that
should be added only when a player-visible active-item capability and its input, cooldown,
presentation, balance, and lifecycle rules are specified together.

Do not allow unrestricted allocation of every numeric attribute. Builds should expose a few legible
decisions, and the server must resolve every choice against explicit engine ceilings, catalog policy,
compatibility rules, and budget constraints.

## Resolution and authority

Selecting a preset or submitting a custom build is intent. At each authority boundary, the server:

1. decodes a bounded candidate shape;
2. validates stable IDs, revisions, field bounds, and supported combinations;
3. canonicalizes order-insensitive choices and derives a reproducible identity;
4. resolves weapon, fighter, ultimate, passive, and future equipment values;
5. enforces slot, family, compatibility, and point-budget rules;
6. applies ownership or entitlement checks when those product systems exist;
7. creates an immutable resolved match loadout.

Code-owned ceilings bound collection sizes, numeric ranges, and serialized snapshots. Authored
catalog or balance policy may narrow those ceilings but cannot widen them. A client cannot declare a
candidate legal or directly install resolved values on a fighter.

Combat systems consume only the resolved loadout and runtime components. They do not query editor,
inventory, acquisition, rarity, account, or entitlement state.

## Fighter attributes

Attributes belong to one of three levels: authored or derived loadout values, mutable runtime state,
or envisioned capabilities. Do not place current values and immutable definitions in one generic
attribute map.

### Supported resolved values

- maximum health;
- movement speed;
- weapon damage, reach, speed, economy, and recovery values through the resolved weapon;
- ultimate definition and cost;
- two passive definitions and their resolved grants.

### Supported runtime state

- current health and alive/defeated state;
- authoritative planar position and facing;
- ammo or charges, cooldown, reload, and recharge deadlines;
- ultimate charge and execution phase;
- passive trigger windows and other bounded effect state;
- team and match participation state.

### Envisioned attribute families

Future content may justify additional authored, resolved, or runtime values:

- **survivability:** armor, shields, healing multipliers, regeneration, knockback resistance, and
  status resistance;
- **mobility:** acceleration, stopping response, turn rate, dash modifiers, attack movement, and
  terrain permissions;
- **weapon performance:** charge time, falloff, hit-stun, spread, projectile width, or additional
  economy forms;
- **ability economy:** additional charge sources, resource costs, active-item charges, durations,
  and cooldowns;
- **information and interaction:** vision, concealment, reveal, targeting priority, pickup radius,
  objective interaction, objective damage, and carrying rules.

Add one coherent capability family when it creates a readable build tradeoff. Armor, critical hits,
lifesteal, vision manipulation, and generalized status resistance are not implied merely by their
presence in this inventory.

Vision and concealment are authoritative information rules, not rendering preferences. When
implemented, the server derives visibility for each observer and subject and uses network interest
management to withhold secret live spatial state. See
[Environment gameplay direction](./09-environment-gameplay.md#concealment-gameplay-model).

## Target-owned status state

Some attacks and regions may contribute to an internal meter rather than trigger an immediate
effect. Keep definition, resolved rules, and target runtime separate:

```text
StatusDefinition
  Stable status identity
  Threshold and decay policy
  Triggered effect
  Reset, lockout, and immunity policy

ResolvedStatusRules
  Definition-derived values
  Fighter and equipment modifiers

StatusRuntime
  Current value
  Last-contribution tick
  Trigger or lockout deadline
  Runtime resistance or immunity state
```

The target owns `StatusRuntime`. Weapons, abilities, persistent regions, and allied sources may emit
contributions with the same stable status identity, as specified in
[Weapons and abilities](./03-weapons-and-abilities.md#effects-and-status-contributions).

A first systemic-status slice should implement one complete interaction, such as cold accumulating
into a temporary freeze. It must prove multi-source contribution, decay, threshold behavior,
cleanup, resistance or immunity, and readable feedback before a generalized status framework is
justified.

## Fighter lifecycle

The resolved match loadout remains immutable for the active selection. Mutable state derived from it
must have explicit initialization, reset, and cleanup rules:

- activation and respawn restore the resolved maximum health and weapon economy;
- ability and passive state reset according to their declared match and fighter lifecycles;
- defeat prevents authoritative attack or ability activation;
- disconnect removes or transfers ownership-sensitive runtime objects according to their definition;
- build replacement is allowed only in a server-owned phase and reinitializes all build-derived
  runtime state;
- match restart and teardown remove transient effects, deployables, deadlines, and target-held
  status state without relying on client presentation.

Presentation may observe these transitions but must never be their authority.

## Arsenal direction

The long-lived product direction is an arsenal of saved brawlers. Each saved brawler has a stable
build identity and revision and contains its bounded weapon selection plus the rest of its loadout.
Selecting a brawler for a match remains intent: the server retrieves or receives the candidate,
validates it against the active content and balance policy, and creates a new resolved match
loadout.

The established match path does not require accounts or persistence. Persistent arsenal storage,
editing history, acquisition, currency, loot, unlocks, and migration policy are later product
capabilities and must not leak into combat systems. The canonical candidate is tracked in
[the backlog](./backlog.md#canonical-cross-version-candidate-index).

## Collectible equipment extension

A future arsenal may include collectible, equippable items that grant bounded stat modifiers,
passive effects, or capabilities. This extends the existing loadout pipeline rather than replacing
it. Keep these concerns distinct:

1. **Item definition:** developer-authored grants, slot and family tags, compatibility rules,
   presentation references, and balance revision.
2. **Item instance:** a player-owned stable identity referencing an item definition, plus only the
   persistent properties the product explicitly supports.
3. **Equipment selection:** item-instance identities proposed for legal brawler slots.
4. **Resolved equipment grants:** immutable definition-derived modifiers, effects, and capabilities
   folded into the match loadout after server validation.
5. **Equipment runtime:** cooldowns, trigger windows, charges, and active effects created by those
   grants during play.

The server validates ownership or entitlement, slots, conflicts, stacking, caps, and revisions. An
item instance ID must never become authority for a gameplay value. Combat behavior should branch on
resolved grants or capabilities, not rarity, acquisition history, or a particular item ID.

Pre-match equipment is the expected extension. Equipping loot during an active match would require
pickup, inventory mutation, loadout transition, replication, presentation, and balance rules and is
therefore a separate product decision.

## Roles as outcomes, not classes

Avoid hard-coded fighter classes. Roles should emerge from build choices:

- **Skirmisher:** mobile, medium range, consistent damage;
- **Bruiser:** durable, short range, disruptive;
- **Marksman:** fragile, long range, high accuracy reward;
- **Controller:** area denial and crowd control;
- **Support:** healing, shielding, or team utility.

Role tags are useful for analytics, onboarding, and matchmaking explanation, but they do not unlock
separate simulation rules. Tags may overlap—for example `Tank + Controller` or
`Assassin + Damage`—and should describe matchup expectations rather than constrain legal builds.
