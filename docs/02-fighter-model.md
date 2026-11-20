# Fighter and build specification

## Purpose and authority

This document defines the durable roster-character, loadout-resolution, and target-owned
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

1. **Character and content definitions:** developer-authored roster characters (stats, signature
   weapon, option pairs), weapon bases, parts, ultimates, passives, legal bounds, and stable
   presentation references.
2. **Character choice:** the server-owned persistent player pick: one `CharacterId` plus one of
   that character's two ultimates and one of its two passives, remembered per character.
3. **Accepted loadout identity:** the canonical recipe fingerprint and balance revision resolved
   from the admitted choice at the server authority boundary.
4. **Resolved match loadout:** the immutable, server-validated gameplay snapshot used to instantiate
   a fighter for one match.
5. **Runtime fighter state:** mutable ECS state such as health, pose, weapon economy, ability charge,
   effects, team, and lifecycle phase.

```text
CharacterDefinition (content/catalogs/characters.ron)
  Permanent CharacterId, key, display name
  Role (Damage, Tank, Helper, Controller) and body (Light, Normal, Heavy) labels
  Fighter stats
  Signature weapon: one weapon base plus up to four authored parts
  Exactly two ultimate options and two passive options

AdvertisedCharacterCatalog
  The roster with stats, weapon base, and option identities
  Weapon, ultimate, and passive display metadata
  Canonical server-derived revision

CharacterChoice
  CharacterId, ultimate, passive

ResolvedMatchLoadout
  Accepted identity
  CharacterId (the replicated fighter identity)
  Resolved fighter stats
  Resolved primary weapon
  Resolved ultimate
  Resolved passive grant

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
the assets. A fighter's model, tint, scale, and weapon socket come from the client-only
`assets/catalogs/fighter_characters.ron`, keyed by `CharacterId`; an unknown ID draws the fallback
model untinted. A character without its own model yet is a tinted placeholder (Light bodies on Riff,
Normal and Heavy bodies on Bumper); its art ticket changes only its entry there.

## Roster and option choice

Players pick one of the authored roster characters, then one of its two ultimates and one of its
two passives. Every character is available: there are no unlocks, progression, or acquisition, and
duplicate characters may appear on both teams and within a team. Composition stays the authoring
model: a character is a curated combination of the existing primitives (fighter stats, weapon base,
parts, ultimates, passives), not a new combat capability. A new character is a catalog entry; a new
behavior family still follows [Weapons and abilities](./03-weapons-and-abilities.md).

`CharacterId` values are permanent wire and storage identities: never reused or renumbered,
independent of display order. Display order is catalog order. The default pick (a new profile, or a
stored pick the active catalog no longer offers) is the first character with its first options.

The loadout has exactly one passive slot (`PASSIVE_SLOT_COUNT`). Lightweight Frame and Reinforced
Frame scale the character's maximum health and movement speed by bounded basis-point parameters; the
other passives keep their runtime grants.

## Resolution and authority

Selecting a character or option, and admitting a player to a match, is intent. At each authority
boundary, the server:

1. decodes a bounded candidate shape;
2. validates the stable IDs and the profile revision;
3. requires the character to exist in the active catalog and to offer the chosen ultimate and
   passive;
4. aggregates the character's authored parts into one bounded weapon-modifier set and applies a
   frame passive to the character's stats;
5. resolves weapon, ultimate, and passive values through the single resolver
   `builds::resolve_loadout`, deriving a reproducible identity;
6. creates an immutable resolved match loadout.

The identity hashes the pick (character, weapon base and modifiers, ultimate, passive) and the
balance revision, not tuned values: the gameplay content fingerprint covers stat and parameter
changes. Code-owned ceilings bound collection sizes, numeric ranges, and serialized snapshots.
Authored catalog or balance policy may narrow those ceilings but cannot widen them. A client cannot
declare a candidate legal or directly install resolved values on a fighter.

Combat systems consume only the resolved loadout and runtime components. They do not query editor,
profile, account, or entitlement state.

Player picks, Practice bots, the direct-network diagnostic fallback, and Balance Lab re-resolution
all use this one path. Practice bot ordinal `n` takes catalog character `n mod roster size` with its
first options. The diagnostic fallback's ordered character picks are validated authored policy in
`content/catalogs/builds.ron`; match admission never derives diagnostic content from definition-
vector order. The policy participates in global gameplay-content compatibility.

An active-item slot is not part of the supported loadout. It is a possible future extension that
should be added only when a player-visible active-item capability and its input, cooldown,
presentation, balance, and lifecycle rules are specified together.

## Fighter attributes

Attributes belong to one of three levels: authored or derived loadout values, mutable runtime state,
or envisioned capabilities. Do not place current values and immutable definitions in one generic
attribute map.

### Supported resolved values

- maximum health;
- movement speed;
- health recovery rate and accepted-attack idle delay;
- weapon damage, reach, speed, economy, and recovery values through the resolved weapon;
- ultimate definition;
- one passive definition and its resolved grant.

Each character authors its own stats in `content/catalogs/characters.ron`. The launch roster is
seeded per body; per-character tuning belongs to Balance Lab character editing (BRL-5v3c):

| Body | Maximum health | Movement speed (world units/s) | Reveal radius | Recovery (health/s) |
|---|---:|---:|---:|---:|
| Light | 850 | 77 | 192 | 100 |
| Normal | 1,000 | 70 | 160 | 100 |
| Heavy | 1,200 | 63 | 128 | 100 |

### Health recovery contract

Every launch character recovers `100` health per second after waiting `3.0` seconds without a
server-accepted player attack. The rate is authored in
health points per second; the delay is authored as a positive fixed-tick duration and presented in
seconds.

Recovery is server-owned and follows these rules:

- an accepted primary attack, Dash activation, or Sentry deployment resets the attack-idle origin
  and discards partial fractional healing progress;
- aiming, movement, damage received, rejected or dry fire, and automatic shots from a deployed
  sentry do not reset it;
- once the delay expires, a fixed-point accumulator applies the authored per-second rate on the
  60 Hz authoritative schedule without depending on render rate;
- only whole health points are published, health never exceeds the resolved maximum, and no
  recovery runs while the fighter is defeated or outside active combat; and
- spawn, respawn, match restart, build replacement, and disconnect cleanup initialize or clear the
  runtime accumulator with the rest of fighter state.

The rule is deliberately attack-idle rather than damage-idle. A fighter that has not attacked for
the configured delay may therefore continue recovering while receiving damage. Adding a separate
damage-idle delay would be a distinct balance rule, not an implicit client-side exception.

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

The first systemic-status slice is implemented as target-owned Cold buildup into temporary Freeze,
with authored decay, threshold, thaw, post-thaw immunity, cleanup, and readable feedback. Weapons,
abilities, and persistent regions contribute through the same bounded condition path; Balance Lab
edits the shared numeric rules. This proven capability does not imply an open-ended status
framework or make source effects owners of target runtime state.

## Fighter lifecycle

Any change to character stats, resolved fighter properties, build resolution, or build-derived
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
- loadout replacement is allowed only in a server-owned phase and reinitializes all build-derived
  runtime state;
- match restart and teardown remove transient effects, deployables, deadlines, and target-held
  status state without relying on client presentation.

Presentation may observe these transitions but must never be their authority.

## Character selection and profile lifecycle

The server-owned profile stores the selected character and, per character, the last chosen
ultimate and passive, so switching back to a character restores its options. The profile authority
normalizes stored rows against the active catalog on load: an unknown character or option is
dropped and the selection falls back to the default pick, so every snapshot sent to a client is
valid. Each selection is one ordered `ProfileCommand` round trip carrying the expected profile
revision; the server rejects it while the player is queued or in a match. Queue and Practice
admission carry the profile revision and freeze the immutable resolved loadout the match uses.

In the current development seam a client or test supplies an opaque `AccountId`; after bounded
format validation, the server atomically loads that profile or creates it when absent. Normal clients
generate and store an ID per logical server, while tests may use deterministic IDs. This seam has no
proof-of-possession, security check, recovery, or profile-creation rate limit and is not a
production credential. The long-lived lobby worker owns the logical-server-local `ProfileAuthority`;
its ECS systems exchange bounded commands and results with a dedicated storage executor that
exclusively owns SQLite.

### Advertised character catalog and connection lifecycle

At lobby startup, `ProfileAuthority` derives one bounded `AdvertisedCharacterCatalog` from the
active character, build, and weapon catalogs. It carries a canonical revision and the roster with
stats, weapon base, and option identities, plus weapon, ultimate, and passive display metadata. It is
derived from active content rather than maintained as a second client list.

`LobbyJoinOutcome::Accepted` carries the catalog atomically with the bounded profile snapshot and
game-type advertisement. The client validates and installs the complete accepted membership before
entering Dashboard, then uses it for the Brawlers select screen, Dashboard summary, weapon preview,
and automation. Disconnect, server change, rejected welcome, or a replacement lobby generation
clears the connection-scoped snapshot; reconnect loads a fresh authoritative profile and catalog. A
stale local catalog is never used as a fallback. Clients send stable-ID intent and may defensively
reject malformed advertisements, but they never define content legality.

The established match path does not require accounts or persistence and must stay isolated from
them. Profile persistence is transactional SQLite with WAL, versioned schemas, an operator backup
command, and a tested restore path (schema 4: `profiles` with the selected character and
`character_choices`; opening a schema-3 database drops the retired brawler and part tables and keeps
published maps; older schemas fail closed). Storage unavailability fails fast. Corruption preserves
the database for recovery, rejects unsafe records, reports the fault, and never silently resets
owned data. The supervisor may supply the database path but cannot open profile data, and match
workers receive only immutable admitted loadouts. Progression, currency, loot, unlocks, and
acquisition systems remain later capabilities and must not leak into combat systems.

## Weapon parts as authoring primitives

A character's signature weapon is one weapon base plus up to four authored weapon parts. Parts grant
bounded stat modifiers or implemented effects and fold into the loadout during resolution; they are
not player-owned items. The launch roster authors no parts.

The resolver validates applicability, stacking, and caps. An effect that cannot apply to the weapon
base rejects the catalog at load; it is never silently ignored. Flat modifiers sum first, the
combined percentage applies second, and resolution clamps and rounds once. Repeated status grants
aggregate into one bounded effect per status kind. Combat behavior branches on resolved grants or
capabilities, never on a part's presentation name.

Player-owned parts, equipment slots, and the starter inventory were retired with saved brawlers
(BRL-egn6). Collectible equipment would be a separate product decision covering ownership,
inventory, selection, loadout transition, replication, presentation, and balance.

## Roles

Every character carries an authored role label (Damage, Tank, Helper, Controller) and a body label
(Light, Normal, Heavy). They are presentation and matchmaking-explanation metadata: they do not
unlock separate simulation rules, and a character's behavior comes only from its resolved stats,
weapon, ultimate, and passive.
