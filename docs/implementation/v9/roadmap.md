# Version 9 implementation roadmap

## Purpose and scope

V9 promotes authoritative concealment and reveal into the playable product. It delivers one
observer-specific visibility rule shared by concealing terrain, a self-cloak ultimate, an allied
concealment-area ultimate, attack and damage reveal, fighter-owned proximity reveal, and a targeted
reveal-scan counter ultimate. Hidden live spatial state is withheld by the server rather than
merely faded by client presentation.

The durable gameplay contract is [Concealment and reveal specification](../../17-concealment.md).
V9 stages the work as complete player-visible slices: first the security and terrain vertical
slice, then the self-cloak/reveal-scan build pair, then the public allied concealment field and
closeout. It does not build general fog of war, vision occlusion, a region scripting framework,
spectator mode, or unrelated environment capabilities.

## Version status

| Field | Value |
|---|---|
| Status | Complete |
| Current milestone | Complete — M03 accepted on 2026-08-24 |
| Entry gate | Satisfied: V8 M04/V8 completed and the user approved V9 M01 implementation on 2026-08-23 |
| Completion gate | Terrain, self-cloak, allied concealment area, proximity, attack/damage reveal, and reveal scan use one server-owned observer decision; unauthorized clients receive no hidden live spatial state or subject-derived leaks; all lifecycle, routed, impairment, capacity, native, feedback, and learning gates pass |

Allowed statuses are `Not started`, `Researching`, `Specification review`, `Implementing`,
`Verifying`, `User playtest`, `Feedback review`, `Complete`, and `Blocked`.

## Accepted product decisions

1. Concealment comes from placed terrain, a self-cloak ultimate, or a targeted allied concealment
   area ultimate.
2. The allied concealment area is public and visually readable for its whole active duration. It
   includes the caster and friendly teammates inside it.
3. Terrain and allied-area concealment are proximity-revealed independently for each enemy
   observer. Self cloak is not proximity-revealed.
4. Reveal proximity is a resolved attribute of the observing fighter. Fighter profiles provide the
   base and validated bonuses/maluses may modify it within bounded rules.
5. An accepted attack reveals terrain/area-concealed fighters for `M` ticks and permanently consumes
   the current self cloak. An applied positive damage outcome does the same using `N` ticks.
6. Attack and damage reveal locks suppress every concealment source until the latest lock expires,
   preventing immediate fallback from a broken self cloak into grass or an allied field.
7. Reveal scan is a targeted instant ultimate. It applies a lingering forced reveal to every enemy
   fighter in the accepted area, including currently visible fighters, and the effect persists
   after leaving the area.
8. Reveal scan has no pre-acceptance warning, reaction window, cleanse, or initial counter-counter.
   Its activation footprint and affected state are readable after acceptance.
9. Reveal scan does not consume underlying concealment. An unexpired source may work again after
   forced reveal ends.
10. Self and allies always see one another. Proximity reveal benefits only the individual observer;
    reveal scan benefits the caster's entire team.
11. Objective carriers are initially ineligible for concealment. Defeated ordinary players do not
    gain unrestricted secret enemy views.
12. Hidden gameplay is a replication/privacy rule. A client-only invisible model is not an accepted
    implementation.

## Milestone overview

| Milestone | Status | Player-visible deliverable | Plan |
|---|---|---|---|
| 01 | Complete | One accepted map contains real concealing terrain: allies remain visible, sufficiently distant enemies disappear at the network boundary, and the observer's resolved reveal-proximity attribute determines close reveal | [milestone-01.md](./milestone-01.md) |
| 02 | Complete | Self-cloak and reveal-scan ultimates form one complete build/counter pair with attack, damage, charge, targeting, HUD, cue, recovery, and balance behavior | [milestone-02.md](./milestone-02.md) |
| 03 | Complete | A public targeted allied concealment field hides its caster and teammates through the same observer rule; V9 closes with full cross-source, lifecycle, security, performance, and playtest evidence | [milestone-03.md](./milestone-03.md) |

## Completion record

V9 completed and was accepted on 2026-08-24. Terrain concealment, Self Cloak, Concealment Field,
observer-owned reveal proximity, attack/damage reveal, and Reveal Scan now share one authoritative
visibility decision and withhold unauthorized spatial state at the network boundary. The user
confirmed Concealment Field in gameplay after the server-authoritative brawler catalog and
full-screen saved-brawler flow corrections. All playtest feedback is reconciled, the verification
record is retained in the milestone documents, and the V9 learning review is complete.

## Ordering rationale

### M01 — Observer-specific visibility and terrain concealment

M01 proves the hard security boundary before adding ability variants. The existing `TALL_GRASS`
map asset deliberately gains concealment, and its accepted Tidal Garden recipe exercises placement
resolution, authoritative overlap, per-observer proximity, Lightyear while-visible
despawn/reappearance, cue filtering, client cleanup, late join, reconnect, restart, and map
replacement. The fighter attribute is resolved through the real V7 profile/build pipeline so later
bonuses and maluses do not require a second contract.

Gate:

- `TALL_GRASS` has explicit server-known concealment and honest normal/fallback presentation, with
  the semantic change carried by revised schema/catalog/content identity;
- self and allies retain the fighter while two hostile observers can receive different current
  spatial results according to their resolved proximity radii;
- an unauthorized client never receives a hidden-before-relevance fighter and loses a previously
  visible fighter with ordinary despawn semantics;
- attack and positive-damage reveal locks work on exact ticks, while rejected attacks, misses, and
  zero outcomes do not reveal;
- public participant state remains available without the cullable spatial fighter;
- fighter hierarchies, cues, projectiles, HUD, audio, sentries, diagnostics, and ordinary player
  telemetry pass the first leak audit;
- late join, reconnect, defeat, respawn, restart, terrain removal/replacement, and map teardown do
  not leave stale visibility;
- focused, separate-App, routed impairment, client-World inspection, packet evidence, and native
  playtest gates pass before M02 begins.

### M02 — Self cloak and reveal scan

Build on the proven observer decision with two complete ultimate families rather than shipping an
uncountered cloak. Self cloak owns duration and permanent consumption on accepted attack or positive
damage. Reveal scan owns bounded targeting, instant hostile selection, team-wide forced-reveal
deadlines, and readable activation/result cues. Both use the established ability charge,
resolution, lifecycle, immutable match-loadout, Dashboard arsenal, and recovery paths.

Gate:

- both ultimates are selectable, previewed, validated, persisted, admitted, charged, activated,
  replicated, recovered, reset, and cleaned up through existing product flows;
- self cloak does not proximity-reveal and is consumed permanently by accepted attack or positive
  damage;
- the global attack/damage lock prevents another active source from immediately hiding the fighter;
- reveal scan affects hidden and visible hostile fighters in the accepted area, remains after they
  leave, suppresses every source, refreshes to the latest deadline, and does not consume sources;
- scan activation has no pre-warning or cleanse but remains legible after acceptance;
- source-derived cues and targeting never precede the authoritative reveal decision;
- the pair passes build tradeoff, counterplay, charge cadence, readability, routed, recovery,
  performance, and user playtest gates.

### M03 — Allied concealment field and closeout

Add one public runtime area whose bounded lifetime, team ownership, membership, teardown, and
presentation use the M01 observer rule. The field includes the caster and allies, uses observer
proximity, and is suppressed by attack/damage locks and reveal scan. M03 then exercises every source
combination and closes V9.

Gate:

- the targeted field clamps or rejects its point through one explicit range policy and its public
  boundary is readable in normal, reduced-effects, and primitive fallback presentation;
- only living caster-team fighters inside are eligible, leaving is immediate, proximity remains
  observer-specific, and objective carriers remain ineligible;
- expiry, owner defeat/disconnect, restart, replacement, recovery, reconnect, and shutdown remove
  field-owned state without stale hidden fighters or visuals;
- overlapping sources and fields obey one deterministic priority/deadline rule with bounded active
  instances and visibility churn;
- bots and sentries consume permitted visibility rather than absolute server state where their
  supported behavior is exercised;
- routed 1v1/2v2/3v3, mixed-build, impairment, repeated lifecycle, memory, cue-fan-out, native
  presentation, and security audits pass;
- every feedback item is implemented, deferred, rejected with rationale, or marked as needing
  evidence, and the learn-from-errors review is complete before V9 is accepted.

## Cross-version dependency decisions

- V8 must close first because M01 extends the one surviving `MapGameplayProfile` and map runtime;
  it does not restore terrain regions or a second map path.
- V7's resolved fighter profile/loadout is the sole owner of reveal proximity. V9 may add bounded
  part modifier definitions but combat and concealment never query profile storage or inventory.
- V6 Balance Lab must expose any newly balanceable fighter/ultimate fields before the corresponding
  milestone closes; manual file editing is not the accepted tuning workflow.
- V2 routed practice and multiplayer remain the product paths. No direct-UDP-only concealment
  implementation or client-local shortcut is accepted.
- Playable bots remain a separate candidate. M01 must audit inert practice fixtures and sentry
  targeting; if autonomous practice bots are promoted later, their observation adapter must consume
  the V9 visibility decision.

## Version-wide architecture boundaries

```text
authored fighter/map/ultimate definitions
        |
        v
immutable resolved map and match loadout
        |
        v
server-owned source membership + reveal deadlines
        |
        v
observer x subject visibility decision
        |
        +--> Lightyear per-connection fighter visibility
        +--> per-connection cue/message filtering
        +--> bot/sentry permitted-target adapter
        |
        v
client presentation of only permitted and public facts
```

The authoritative server keeps absolute fighter state. Observer decisions use stable gameplay and
connection mappings internally, never process-local entity identity on the wire. Public participant
state remains separate from cullable spatial state. `VisibilityExt::lose_visibility` is used for
secret spatial entities; retained and always-present policies are forbidden.

## Verification strategy

Every milestone uses the smallest relevant layers:

- pure truth-table, modifier, deadline, geometry, and stable-order tests;
- small `App`/`World` fixed-schedule and lifecycle tests with explicit simulation ticks;
- separate server/client App tests inspecting the unauthorized client World;
- routed product tests with multiple observers and different outcomes;
- delay/loss/duplication/jitter plus late-join/reconnect checks;
- message/cue/hierarchy/projectile/audio/diagnostic leak assertions;
- bounded entity, pair-cache, transition, queue, message, recovery-byte, CPU, and memory evidence;
- native normal, primitive-fallback, reduced-effects, HUD, and controller playtests.

Visual absence alone never proves concealment. At least one M01 security test must inspect received
client state or packet-decoded application facts to prove that the secret pose was not delivered.

## Explicitly deferred beyond V9

- Removal of the superseded full-build preset system, including named presets such as
  `Veilkeeper`, the unreachable legacy Build Editor, standalone build persistence, and obsolete
  direct-session selection surfaces. This is tracked as `MAINT-LEGACY-BUILD-SYSTEM` in the
  [canonical backlog](../../backlog.md#trigger-bound-technical-maintenance); the active saved-brawler
  profile/loadout path and weapon-base presets remain.
- Wall/line-of-sight vision, fog of war, lighting-based stealth, last-known-position UI, target
  memory, replay, kill-cam, and spectator permissions.
- Team-shared proximity reveal, reveal cleanses/immunity, counter-counter abilities, concealed
  objectives, projectiles, deployables, or arbitrary effects.
- General environment-area scripting, universal perception, arbitrary map-authored rules, or a
  generic status/dispelling framework.
- Automatic practice bots, bot matchmaking fill, or advanced bot navigation.
- Final release balance. V9 produces accepted defaults and a maintained Balance Lab surface, not a
  permanent numeric claim.

## Research sources

Local, version-pinned sources inspected for V9 preparation:

- `references/lightyear/examples/network_visibility/src/server.rs`;
- `references/lightyear/crates/replication/replication/src/visibility/immediate.rs`;
- `references/lightyear/crates/replication/replication/src/send.rs`;
- `references/lightyear/book/src/concepts/advanced_replication/interest_management.md`;
- `src/gameplay.rs`, `src/server/mod.rs`, `src/combat/server.rs`, `src/combat/authority.rs`,
  `src/combat/attack.rs`, `src/combat/effects/application.rs`, `src/builds/model.rs`, and
  `src/builds/definitions.rs`;
- `content/catalogs/builds.ron`, `content/catalogs/map_assets.ron`, and
  `content/catalogs/map_gameplay_profiles.ron`;
- [network architecture](../../08-network-architecture.md#interest-management-and-concealment),
  [environment gameplay](../../09-environment-gameplay.md#concealment-gameplay-model), and
  [grid map-asset specification](../../16-grid-map-asset-system.md).

Primary released sources checked after the local snapshot:

- [Lightyear 0.29 replication crate](https://docs.rs/crate/lightyear_replication/0.29.0), which
  documents per-client `gain_visibility`/`lose_visibility`, hierarchy propagation, and ordinary
  remote despawn behavior;
- [Lightyear 0.29 network-visibility example](https://github.com/cBournhonesque/lightyear/tree/0.29.0/examples/network_visibility),
  matching the checked-in example used by the current architecture document.
