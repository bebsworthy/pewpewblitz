# Gameplay loops

## Purpose

This document describes how PewPew Blitz's player-facing loops fit together, from second-to-second
combat through matches, practice, build refinement, and envisioned long-lived product loops. It is a
design contract, not a delivery checklist or implementation-status report.

[Product direction](./00-product-direction.md) owns the product promise and design pillars.
[Fighter and build specification](./02-fighter-model.md),
[weapons and abilities](./03-weapons-and-abilities.md), and
[maps and modes](./04-maps-and-game-modes.md) own the rules and data used by these loops.
[Player UX](./13-player-ux.md) owns exact navigation and admission behavior. Version roadmaps own
completed scope, evidence, and future delivery plans.

## Loop hierarchy

The loops operate at different cadences but reinforce one another:

| Cadence | Loop | Player question |
|---|---|---|
| Seconds | Combat decision | What should I do now? |
| Tens of seconds | Encounter and life | Can I win this exchange or create an advantage? |
| Minutes | Objective and match | How does our team convert advantages into victory? |
| One or more matches | Build learning | What did this brawler do well, and what should I change? |
| Play session | Session flow | Do I practice, compete, refine, play again, or leave? |
| Long term | Arsenal and creator loops | What do I want to master, collect, author, or share next? |

The inner loops must be satisfying before an external loop asks the player to repeat them. Rewards,
progression, social pressure, or content volume cannot compensate for combat that lacks clarity,
agency, or useful feedback.

## Core combat loop

The moment-to-moment loop is:

```text
Read the arena and opponents
        ↓
Choose range, route, target, and timing
        ↓
Move and aim
        ↓
Attack or use an ability
        ↓
Server resolves movement, collision, cost, and outcomes
        ↓
Receive immediate visual, audio, and HUD feedback
        ↓
Reposition and adapt
```

The player should repeatedly make understandable tradeoffs among pressure, safety, positioning,
cooldowns, ammunition, ultimate charge, teammates, objectives, and terrain. Mechanical execution
matters, but success should also come from anticipating routes, choosing favorable distance, and
recognizing when to commit or disengage.

The supported combat action set consists of movement, aiming, primary fire, and ultimate use, plus
menu or context actions outside authoritative combat. Passive effects modify or react to the loop
without becoming additional moment-to-moment buttons. Active items are an envisioned extension and
must earn a clear input, readability, and balance role before joining the supported action set.

## Combat-loop requirements

The core loop should provide:

- immediate response to movement and aim intent;
- a clear preferred range and counterplay for every primary weapon;
- legible attack anticipation, travel, impact, damage, control, and defeat feedback;
- resource and cooldown states visible early enough to affect decisions;
- deterministic server-owned outcomes under the same rules in practice and multiplayer;
- enough movement and cover options to recover from a poor position without making commitment
  meaningless;
- attribution that lets a player understand why damage, displacement, control, or defeat occurred;
- bounded effects and presentation that remain readable with the maximum supported roster.

Input devices map to abstract actions. Controller and keyboard/mouse may use different sampling,
deadzone, cursor, and focus behavior, but they do not create separate gameplay implementations. Aim
assist or target selection is an explicit server-compatible gameplay rule rather than hidden input
correction.

## Authority and feedback

The client samples intent and presents replicated facts. The authoritative match worker validates
and resolves movement, attacks, abilities, collision, map mutation, damage, status, defeat,
objectives, scoring, and victory.

Presentation may predict or interpolate where the network architecture explicitly permits it, but
feedback must converge on the authoritative outcome. A responsive local cue cannot award a hit,
change terrain, advance a score, or hide a rejected action.

Every important authoritative transition should have a player-facing explanation at the appropriate
layer:

- world presentation communicates movement, attacks, impacts, hazards, objectives, and terrain;
- fighter UI communicates health, ammunition, cooldowns, status, and ultimate readiness;
- match HUD communicates teams, score, progress, time, and phase;
- results communicate the authoritative outcome and a useful next choice.

## Encounter and fighter-life loop

A fighter life begins at a mode-approved spawn or re-entry point. The player establishes position,
engages, creates or contests an advantage, and either survives to continue pressure or is defeated.
The mode then decides whether the fighter respawns, waits for a round transition, or remains
eliminated.

```text
Spawn or re-enter
      → establish position
      → engage or contest
      → survive and continue
          or
        suffer defeat
      → mode-owned respawn, round, or elimination policy
```

Defeat should create a meaningful cost without turning a short match into extended inactivity.
Respawn timing, protection, placement, and route safety must prevent immediate unfair re-defeat while
preserving earned enemy pressure. Modes without respawn must replace the re-entry loop with clear
spectating, round, or results behavior when that mode is introduced.

## Objective loop

Combat creates temporary advantages; a mode explains how those advantages become victory.

- In Wipeout, teams convert favorable fights into defeats and score while managing re-entry timing.
- In Hot Zone, teams convert space and survival into uncontested or favorable objective occupancy.
- In Heist, teams convert pressure and lane control into damage on the opposing durable objective
  while defending their own.
- Future modes may convert pressure into carried resources, territory, rounds, or survival, but
  each must define one readable conversion loop.

An objective should concentrate decisions rather than replace combat. Players need enough time and
space to choose approaches, defend counters, and understand progress. Scoring rules, contest state,
timeouts, and victory are server-owned and visible before they become surprising outcomes.

## Match loop

The supported match loop is:

```text
Validated roster, builds, map, mode, and rules
        ↓
Loading and check-in
        ↓
Countdown and readable initial state
        ↓
Active combat and objective play
        ↓
Authoritative completion
        ↓
Results
        ↓
Play Again with the exact compatible selection, or return to Dashboard
```

Matches are intentionally short enough that build strengths, weaknesses, and counters become
visible without a long commitment. Roughly two to four minutes remains the ordinary design target,
although a concrete mode may justify another bounded duration.

The match lifecycle is common only where modes share behavior. A mode owns its scoring, objective,
respawn/elimination, timeout, and victory rules. Common match systems own roster identity, broad
phase transitions, results handoff, cleanup, and restart behavior that is genuinely identical.

A completed match must terminate cleanly, preserve one authoritative result, release its isolated
worker resources, and offer a clear player choice. **Play Again** requests new admission using the
exact game type and saved-brawler identity/revision only while the fresh game-type and brawler
catalogs remain compatible; it is not recorded-match playback. No-result disconnect and recovery
paths return to a usable product state rather than pretending that an outcome occurred.

## Player session loop

The supported ordinary session begins at the connected Player Dashboard:

```text
Dashboard
   ├─ choose game type and saved brawler → multiplayer admission → match
   ├─ choose game type and saved brawler → server-hosted practice → match
   ├─ inspect, create, or customize saved brawlers
   └─ adjust settings or recover connection

match → results → Play Again with an exact compatible selection, or Dashboard
```

Multiplayer and practice share validated game types, resolved builds, authoritative match workers,
mode rules, maps, combat, and results. Practice removes dependence on a populated human queue; it
does not create a client-authoritative or reduced-rule simulation.

The session loop should minimize repeated configuration without hiding meaningful choices. Lobby
admission loads the server-owned profile plus bounded brawler and game-type advertisements before
Dashboard entry. The client mirrors those connection-scoped facts, exposes their current validity,
and makes Play and Practice the two ordinary admission actions. Queueing, loading, cancellation,
confirmed leave, disconnect, results, and Play Again must converge predictably rather than creating
separate dead-end flows.

## Build-learning loop

Buildcraft becomes valuable through use, observation, and revision:

```text
Choose or configure a legal brawler
        ↓
Test its intended range and rhythm in practice
        ↓
Use it against human decisions and mode pressure
        ↓
Read outcomes and identify a strength, weakness, or tradeoff
        ↓
Adjust one bounded choice or select another saved brawler
        ↓
Repeat
```

Each saved brawler permanently owns one advertised fighter profile and weapon base, and may change
its name, ultimate, exactly two passives, and four generic weapon-part slots outside queue. The
server validates stable-ID mutations against its active catalog and resolves the selected brawler
into an immutable match loadout. The client never supplies gameplay values or defines the legal
inventory.

Build feedback should emphasize decisions rather than only aggregate power. Useful questions include:

- Did the player fight at the weapon's intended distance?
- Did ammunition, fire cadence, or cooldown create the expected opening?
- Did the ultimate change position or pressure at a meaningful moment?
- Did each passive produce an observable advantage with an opportunity cost?
- Did the selected mode or map expose a weakness that another build choice could address?

Practice is the fastest experimentation path, but it must eventually provide opponents that exercise
movement, range, pressure, objectives, and counterplay. Inert participants validate lifecycle only;
they do not complete the build-learning promise.

## Saved-brawler arsenal loop

V7 established an arsenal of player-authored brawlers rather than a roster of fixed heroes. A
player can create, name, save, inspect, customize, delete, and select multiple validated brawlers
for different modes, maps, team needs, or personal styles through the full-screen Dashboard child
flow.

Persistence and collection do not alter combat legality. The server-advertised catalog defines
which definitions the current lobby permits, profile authority validates ownership and selection,
and the same resolver produces the immutable match loadout. Future acquisition or entitlement may
narrow availability but cannot move legality to the client.

The arsenal should reward mastery and expression without forcing constant replacement of understood
builds. New content should create new decisions, counters, or play patterns rather than merely higher
numbers.

## Envisioned progression and social loops

Progression, ranking, parties, teams, challenges, cosmetics, sharing, and live events may motivate
additional sessions, but they are external to authoritative combat. When introduced, they should:

- direct players toward interesting builds, modes, and learning goals;
- avoid granting unbounded competitive power;
- preserve honest matchmaking and outcome ownership;
- fail without corrupting or blocking the core local/server-authoritative match loop;
- use stable match summaries rather than becoming a second source of gameplay truth.

These systems are candidates, not prerequisites for the product identity. Their exact shape belongs
to the version that demonstrates a concrete player need.

## Envisioned creator loop

The map-builder direction creates another external loop:

```text
Choose a supported mode and theme
        → author a bounded recipe
        → validate
        → playtest
        → revise
        → save or publish through future product services
        → other players discover and play
        → gather feedback and iterate
```

Validation and playtesting are part of creation, not a late publishing check. Built-in and
player-authored maps use the same recipe and resolution path, while persistence, distribution,
discovery, moderation, asset licensing, and migration remain separate services. Players author
space, not executable mode rules.

## Loop health principles

Across all supported and envisioned loops:

1. **Return quickly to a meaningful choice.** Defeat, match completion, cancellation, and recoverable
   failure should not strand the player.
2. **Make repetition informative.** A new life, match, or build test should reveal something the
   player can act on.
3. **Keep choices bounded and legible.** More options are useful only when their consequences can be
   understood in play.
4. **Preserve one authority path.** Practice, multiplayer, Play Again admission, creator playtests,
   and automation use server-owned validation and simulation.
5. **Separate behavior from motivation.** Progression and rewards may encourage a loop but cannot
   define hits, scores, legal builds, or match outcomes.
6. **Prefer complete vertical loops.** A new mode, content family, or external system should end in a
   player-visible experience rather than only generalized infrastructure.

## What to observe

Metrics support playtest interpretation; they do not define fun by themselves. Useful combat and
match observations include:

- time to first engagement and first damage;
- fight duration and time between meaningful decisions;
- hit rate and damage by distance band and weapon configuration;
- defeat, survival, and objective contribution by build;
- ammunition pressure, reload downtime, and ultimate charge/use cadence;
- respawn-to-engagement and respawn-to-defeat time;
- objective contest frequency, uncontested progress, and comeback opportunities;
- score margin, match duration, early exits, rematches, and return-to-Dashboard behavior.

Useful build-learning observations include whether players can predict a configuration's tradeoff,
identify why it succeeded or failed, and make a deliberate next change. Useful external-loop
observations include whether practice leads naturally to build refinement or multiplayer, whether
results create a clear next action, and whether additional systems deepen the desire to play rather
than merely adding obligations.
