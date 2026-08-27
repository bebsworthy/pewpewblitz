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
2. **Saved brawler:** server-owned persistent player choice composed from stable fighter-profile,
   weapon-base, ultimate, passive, and equipped-part identities.
3. **Selected brawler identity:** the accepted saved-brawler ID and revision used across process and
   network boundaries. The superseded full-build preset/recipe identity remains only in legacy
   internals pending `MAINT-LEGACY-BUILD-SYSTEM` removal.
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

AdvertisedBrawlerCatalog
  Stable legal fighter, weapon, ultimate, and passive identities
  Bounded display/preview metadata and selection limits
  Canonical server-derived revision

SavedBrawler
  Stable server-owned identity and revision
  Permanent fighter profile and weapon base
  Editable name, ultimate, two passives, and up to four equipped part instances

SelectedBrawler
  Saved-brawler identity and revision
  Canonical resolved identity and content revision

ResolvedMatchLoadout
  Selected-brawler/resolved identity
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

## Legacy full-build contract

The following contract describes the superseded preset/custom-build path retained temporarily for
diagnostics and fixtures. It is not the player-facing arsenal; V7 saved brawlers and the
server-advertised catalog below own ordinary product selection. Its removal is tracked as
`MAINT-LEGACY-BUILD-SYSTEM`.

The retained legacy foundation is:

- one primary-weapon selection;
- one ultimate selection;
- exactly two passive selections;
- a fixed 12-point budget shared by the weapon, ultimate, and passives;
- duplicate and incompatible-family rejection;
- four built-in build/weapon presets resolved through the same paths as non-presets;
- one bounded custom Pulse specification with discrete power, reach, and magazine choices.

The legacy selection is not the operational weapon recipe. It contains bounded IDs and typed
choices; the server derives the numeric weapon configuration described in
[Weapons and abilities](./03-weapons-and-abilities.md). This prevents clients from directly choosing
unbounded damage, collision behavior, lifetimes, presentation profiles, or other authoritative
values.

An active-item slot is not part of the supported loadout. It is a possible future extension that
should be added only when a player-visible active-item capability and its input, cooldown,
presentation, balance, and lifecycle rules are specified together.

Do not allow unrestricted allocation of every numeric attribute. Builds should expose a few legible
decisions, and the server must resolve every choice against explicit engine ceilings, catalog policy,
compatibility rules, ownership, and slot constraints.

## Resolution and authority

Creating, editing, equipping, selecting, or admitting a saved brawler is intent. At each authority
boundary, the server:

1. decodes a bounded candidate shape;
2. validates stable IDs, revisions, field bounds, and supported combinations;
3. validates referenced definitions against its active advertised catalog;
4. canonicalizes order-insensitive choices and derives a reproducible identity;
5. resolves weapon, fighter, ultimate, passive, and equipment values;
6. enforces ownership, slot, uniqueness, applicability, and compatibility rules;
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
- health recovery rate and accepted-attack idle delay;
- weapon damage, reach, speed, economy, and recovery values through the resolved weapon;
- ultimate definition and cost;
- two passive definitions and their resolved grants.

The current canonical fighter-profile movement speeds are expressed in world units per second:

| Profile | Movement speed |
|---|---:|
| Default | 100 |
| Lightweight | 110 |
| Reinforced | 90 |

All three canonical profiles initially recover 10 health per second after 3 seconds without a
server-accepted player attack. Taking damage does not restart that attack-idle delay. Recovery is
server-owned, fixed-tick accumulated, clamped to maximum health, and reset with fighter lifecycle.

### Supported runtime state

- current health and alive/defeated state;
- authoritative planar position and facing;
- ammo or charges, fire cooldown, and the independent next-ammunition recovery interval;
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
[Concealment and reveal specification](./17-concealment.md), including V9's resolved
observer-owned reveal-proximity attribute and bounded bonus/malus contract.

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

Any change to fighter profiles, resolved fighter properties, build resolution, or build-derived
runtime state must review and update the development
[Balance Lab](./15-balance-lab.md#required-maintenance-contract) in the same change, or explicitly
document why that property is intentionally unavailable there. This includes snapshot exposure,
validation, apply/reset initialization, replication, and focused verification.

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

V7 established the long-lived arsenal product contract. An account may own at
most 16 saved brawlers. Each brawler has a stable server-owned identity and revision with:

- a non-unique, editable display name;
- one fighter profile and one weapon base, both permanently fixed at creation;
- up to four equipped weapon-part instances;
- one freely swappable ultimate and two freely swappable passives.

A saved brawler may be deleted. Name, equipment, ultimate, and passives may be edited outside queue,
but the server rejects every edit while the player is queued. Queue admission freezes the canonical
brawler revision and immutable resolved loadout used by the match. Creating a new brawler, rather
than mutating its body or weapon identity, is the intended way to try another permanent combination.

V7 starts with fresh server-side profile data and does not import today's locally saved build. It
also removes the 12-point budget and retires Runner, Bruiser, Controller, and Duelist as
player-facing builds rather than converting them into starter templates. The launch
fighter-profile and weapon-base inventory is developer-authored server content rather than a
client-owned numeric range.

Selecting a brawler for a match remains intent: the server-side profile authority retrieves the
candidate, validates ownership and the active content revision, and creates a new resolved match
loadout. In V7 a client or test supplies an opaque `AccountId`; after bounded format validation, the
server atomically loads that profile or creates it when absent. Normal clients generate and store an
ID per logical server, while tests may use deterministic IDs. This seam has no proof-of-possession,
security check, recovery, or profile-creation rate limit and is not a production credential. The
long-lived lobby worker owns the logical-server-local `ProfileAuthority`; its ECS systems exchange
bounded commands and results with a dedicated storage executor that exclusively owns SQLite.

### Advertised brawler catalog and connection lifecycle

V9 made the selectable inventory and its player-facing metadata server authoritative. At lobby
startup, `ProfileAuthority` derives one bounded `AdvertisedBrawlerCatalog` from the active build and
weapon catalogs. It contains a canonical revision, saved-brawler/slot limits, weapon policy, and
stable IDs, keys, display names, kinds, eligibility, and preview/stat data for fighter profiles,
weapon bases, ultimates, and passives. It is derived from active content rather than maintained as a
second client list.

`LobbyJoinOutcome::Accepted` carries the catalog atomically with the bounded profile snapshot and
game-type advertisement. The client validates and installs the complete accepted membership before
entering Dashboard, then uses it for names, loadout summaries, creation choices, ability editing,
weapon previews, selection cycling, and automation. Disconnect, server change, rejected welcome,
or a replacement lobby generation clears the connection-scoped snapshot; reconnect loads a fresh
authoritative profile and catalog. A stale local catalog is never used as a fallback.

Profile storage validates persistent structure: bounds, revisions, identities, ownership,
uniqueness, and equipment shape. Lobby profile authority separately validates every referenced
fighter, weapon, ultimate, passive, and part definition against active content before accepting a
mutation or admitting a match. Invalid stored content fails closed and is not silently rewritten.
Clients send stable-ID intent and may defensively reject malformed advertisements, but they never
define content legality.

The established match path does not require accounts or persistence and must stay isolated from
them. V7's first persistence baseline is transactional SQLite with WAL, versioned migrations, an
operator backup command, and a tested restore path. Storage unavailability fails fast. Corruption
preserves the database for recovery, rejects unsafe records, reports the fault, and never silently
resets owned data. The supervisor may supply the database path but cannot open profile data, and
match workers receive only immutable admitted loadouts. Progression, currency, loot, unlocks, and
acquisition systems remain later capabilities and must not leak into combat systems.

## Weapon-part equipment and later collectible extension

V7 adds exactly four generic weapon-part slots to every saved brawler. Slots have no gameplay type,
family, or semantic position. Empty slots are legal; distinct owned instances of the same part
definition may be equipped together, while one instance cannot occupy more than one slot. Part
type, name, icon, and model metadata exist only for inventory presentation in V7 and do not attach a
part model to the in-match fighter or weapon.

Weapon parts grant bounded stat modifiers, effects, or implemented capabilities. They extend the
existing loadout pipeline rather than replacing it. Keep these concerns distinct:

1. **Part definition:** developer-authored grants, applicability rules, presentation references, and
   balance revision.
2. **Part instance:** a player-owned stable identity referencing a definition, plus only the
   persistent properties the product explicitly supports.
3. **Equipment selection:** up to four part-instance identities proposed for generic brawler slots.
4. **Resolved equipment grants:** immutable definition-derived modifiers, effects, and capabilities
   folded into the match loadout after server validation.
5. **Equipment runtime:** cooldowns, trigger windows, charges, and active effects created by those
   grants during play.

The server validates ownership, uniqueness, applicability, stacking, caps, and revisions. An effect
that cannot apply to the permanent weapon base rejects the candidate; it is never silently ignored.
Flat modifiers sum first, the combined percentage applies second, and resolution clamps and rounds
once. Repeated status grants aggregate into one bounded effect per status kind. A part instance ID
must never become authority for a gameplay value. Combat behavior branches on resolved grants or
capabilities, not rarity, acquisition history, presentation type, or a particular instance ID.

V7 seeds a fixed authored starter inventory and caps an account at 128 part instances. It preserves
each generated roll across balance updates unless an explicit versioned migration changes it; no
update silently rerolls owned equipment. Initial Frost parts map to the supported slow effect, while
accumulating Frost remains a later status capability. Balance should favor readable sidegrades and
capped tradeoffs.

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
