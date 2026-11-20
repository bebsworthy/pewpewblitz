# Product direction

## One-sentence pitch

PewPew Blitz is a short-match arena shooter where players pick a brawler from an original roster,
choose one of its two ultimates and one of its two passives, and win through movement, aim,
positioning, and tactical use of that kit.

## Design pillars

1. **Combat first.** Every system must improve the feel, clarity, or depth of the moment-to-moment fight.
2. **Meaningful picks.** Each character's signature weapon, stats, and role should create a
   recognizable play pattern, and each ultimate or passive option should change how it plays, not
   merely add invisible numerical power.
3. **Readable competition.** Players should understand why they were damaged, slowed, defeated, or denied an objective.
4. **Short feedback cycles.** A match should expose a pick's strengths and weaknesses quickly.
5. **Content through composition.** New characters, weapon recipes, payloads, items, and map
   layouts should combine from bounded primitives without requiring a bespoke fighter class or a
   custom mode script for each map. A roster character is an authored composition of stats, a
   weapon base with optional parts, and option pairs of existing ultimates and passives. Genuinely new game modes may still require developer-authored authoritative
   rules.
6. **Network-first simulation.** The product is a networked competitive game; offline and local testing must exercise the same server-authoritative simulation path.

## Differentiation

Players meet PewPew Blitz through a small, readable roster: nine original characters at launch, each
with a role (Damage, Tank, Helper, Controller), a body (Light, Normal, Heavy), its own stats, and a
signature weapon. Every character is available; there are no unlocks or progression. The choice a
player makes before a match is the character plus one of two ultimates and one of two passives,
remembered per character.

Underneath, characters stay compositions rather than bespoke hero code:

- the body and stats are a small, understandable foundation;
- the signature weapon, built from shared weapon bases and authored parts, determines the primary
  combat pattern;
- the ultimate and passive options create specialization within that character;
- a new character is a catalog entry over existing primitives, so the roster can grow without new
  combat systems.

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

Characters must not be strictly better than one another. Each character and option should use
bounded legality, explicit tradeoffs, and authored sidegrades rather than a power budget.

Examples:

- high health can cost movement speed;
- a high-damage weapon can have low reload speed or short range;
- a strong crowd-control item can have a long cooldown;
- a mobility item can occupy the same slot as a defensive item.

Player picks, Practice bots, diagnostics, and Balance Lab must use the same compositional recipe and
validation path. Acquisition mechanics such as currency, loot, unlocks, and progression would decide
which characters a player may pick, not whether a loadout is legal in combat, and are later product
problems. Automatic balancing and procedural item generation are
also later problems.

## Product boundaries

PewPew Blitz's identity does not depend on delivering these concerns before its combat,
character-learning, match, and creator loops are valuable:

- production matchmaking and party services;
- persistent progression or currencies;
- ranked play;
- cosmetics and account systems;
- a large roster beyond the launch characters;
- procedural map generation;
- a complete mobile control scheme;
- a production-grade backend;
- exhaustive high-fidelity content.

## IP boundary

PewPew Blitz can be inspired by the genre's camera, match length, and objective structure, but its
name, art, characters, maps, terminology, sounds, ability descriptions, and balance should be
original. External references are used to understand design patterns, not to reproduce protected
content.
