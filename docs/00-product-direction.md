# Product direction

## One-sentence pitch

PewPew Blitz is a short-match arena shooter where players create a brawler around a customized
primary weapon and win through movement, aim, positioning, and tactical use of equipment.

## Design pillars

1. **Combat first.** Every system must improve the feel, clarity, or depth of the moment-to-moment fight.
2. **Meaningful builds.** A customized weapon, ultimate, and two passives should create a
   recognizable play pattern, not merely add invisible numerical power.
3. **Readable competition.** Players should understand why they were damaged, slowed, defeated, or denied an objective.
4. **Short feedback cycles.** A match should expose a build's strengths and weaknesses quickly.
5. **Content through composition.** New weapon recipes, payloads, items, and map layouts should
   combine from bounded primitives without requiring a bespoke fighter class or a custom mode
   script for each map. Genuinely new game modes may still require developer-authored authoritative
   rules.
6. **Network-first simulation.** The product is a networked competitive game; offline and local testing must exercise the same server-authoritative simulation path.

## Differentiation

The reference genre usually begins with a predefined character and then adds character-specific
upgrades. PewPew Blitz reverses that relationship:

- the body is a relatively small, understandable foundation;
- the player composes the weapon's behavior and bounded specifications, which determine the primary
  combat pattern;
- the ultimate and passives create specialization;
- players trade strengths and weaknesses within a bounded budget.

The player's long-lived collection is an arsenal of authored brawlers, each with its own validated
weapon configuration and loadout. This creates a buildcraft game without requiring a large roster
of developer-defined heroes.

## Creator direction

An eventual player-facing map builder lets users author arena layouts from server-known content:
visual themes/models/decorations, terrain, bounded geometry, placeable entities, gameplay regions,
spawn points, and mode-required anchors. Users choose a supported game mode while authoring a map
recipe; they do not author scoring, victory, respawn, shrinking-boundary, objective, or other
executable mode rules.

Built-in maps and user-authored maps must use the same map-recipe and server-validation path.
Establishing that shared representation does not imply an editor, persistence, publishing, asset
upload, discovery, moderation, or procedural generation. A user map builder is therefore compatible
with the non-goal of procedural map generation: one is deliberate bounded authoring, while the
other is automatic generation.

Team count and participants per team are resolved from the chosen game mode and a compatible map,
not fixed globally. The mode defines legal team topology and participant ranges; the map proves it
has the matching team slots, spawn capacity, playable space, and required anchors. Ordinary matches
are expected to center on 3v3, while the architecture must also accommodate larger-group layouts
such as 12 solo competitors, two teams of five, or three teams of three when a mode and map
explicitly support them.

## Balance principle

Customization must not mean that every stat can be maximized independently. Each build should spend from a power budget, use slot restrictions, or accept explicit opportunity costs.

Examples:

- high health can cost movement speed;
- a high-damage weapon can have low reload speed or short range;
- a strong crowd-control item can have a long cooldown;
- a mobility item can occupy the same slot as a defensive item.

Built-in weapon and brawler presets and player-authored builds must use the same compositional recipe
and validation path. Bounded customization uses an explicit budget whose exact allocation belongs
to the specification that introduces or revises it. Acquisition mechanics such as currency, loot,
unlocks, and progression decide which options a player owns, not whether a weapon is legal in
combat, and are later product problems. Automatic balancing and procedural item generation are
also later problems.

## Product boundaries

PewPew Blitz's identity does not depend on delivering these concerns before its combat,
build-learning, match, and creator loops are valuable:

- production matchmaking and party services;
- persistent progression or currencies;
- ranked play;
- cosmetics and account systems;
- a large fighter roster;
- procedural map generation;
- a complete mobile control scheme;
- a production-grade backend;
- exhaustive high-fidelity content.

## IP boundary

PewPew Blitz can be inspired by the genre's camera, match length, and objective structure, but its
name, art, characters, maps, terminology, sounds, ability descriptions, and balance should be
original. External references are used to understand design patterns, not to reproduce protected
content.
