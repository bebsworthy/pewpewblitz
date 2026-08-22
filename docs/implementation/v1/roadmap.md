# Version 1 implementation roadmap

## Purpose and delivery gates

This roadmap is the source of truth for implementation order. The other design documents define product scope, gameplay rules, and architectural constraints. Each milestone design must link back to the requirements it implements so scope changes do not silently diverge from those documents.

The work is divided into three early gates:

1. **Combat vertical slice — Milestones 01–07.** Two teams can complete a short, server-authoritative Wipeout match with four weapons on one readable arena.
2. **First product iteration — Milestones 01–08.** The vertical slice also includes bounded builds,
   one constrained non-preset weapon configuration, two ultimates, and named presets. This is the
   first gate that tests Brawler's player-authored buildcraft differentiation.
3. **Gameplay MVP verification — Milestones 01–10.** Hot Zone and quantized destructible terrain
   prove that combat code works across mode rules, mutable map geometry, and the complete supported
   playable-map size range. This gate satisfies the full acceptance scope in
   [Gameplay MVP](./gameplay-mvp.md).

Milestone 11 hardens and closes the v1 MVP. Additional mode families and systemic status interactions are future-version candidates, not hidden v1 commitments.

The milestones are not release promises. A milestone may be split during its technical-design pass, but its exit criteria may not be silently moved to a later gate.

Milestone 07 completed on 2026-08-15 after accepted external-review remediation passed its automated,
network, role-isolation, performance, and process gates and the user approved closeout. Its remaining
supervised physical-controller, perceptual-audio, complete aspect-ratio/state-matrix, and human
normal-duration observations were carried into M11; the final basic v1 playtest was accepted and
detailed release polish is not represented as passing evidence. Milestone 08 completed through the
same M11 closeout disposition. Milestone 09 completed on 2026-08-16 from baseline `90ef47a`:
its automated gate, both-mode real-process runs (local/typical/adverse), objective-state
performance matrix, and automated playtest are recorded in [milestone-09.md](./milestone-09.md),
with the remaining supervised observations deferred through the backlog below. Milestone 06 completed on
2026-08-15 after green automated, process-network, performance, visual, controller, and audio
verification plus an approved user playtest.

The milestone sections below are outcome briefs and research prompts, not prevalidated technical specifications. Type names, plugins, schedules, package boundaries, data formats, and algorithms remain provisional until the milestone research and specification are approved.

## Version status

- **Version:** v1 — gameplay MVP
- **Overall status:** Complete 2026-08-18. The server-authoritative v1 gameplay MVP passed the full automated, process, network, performance, soak, and schema-3 closeout matrix plus the user's basic final playtest. The user accepted basic v1 behavior and explicitly deferred release-quality improvement/tuning work to `POST-V1-RELEASE-POLISH`; this is MVP completion, not a release-ready claim.
- **Current milestone:** None — v1 is complete; v2 M01 research is next.
- **Last completed milestone:** Milestone 11 — MVP playtest hardening and closeout

The roadmap status values are `Not started`, `Researching`, `Specification review`, `Implementing`, `Verifying`, `User playtest`, `Feedback review`, `Complete`, and `Blocked`. Update the overview and current-milestone fields whenever a milestone changes phase.

## v1 backlog

Record deferred implementation and playtest feedback here. Every item needs its source, rationale, and intended review point.

| ID | Source | Item | Rationale | Review target |
|---|---|---|---|---|
| M03-PRED | Milestone 03 implementation decision | Run the impairment/latency comparison for owner prediction; the M04 harness records the authoritative owner baseline and keeps prediction disabled until a predicted comparison exists. | **Resolved 2026-08-17 (M11): measured and deferred.** The experimental `owner-prediction` candidate passed latency (4/4/5→1 ticks), correction-magnitude (p95 12–18 ≤ 24 units), convergence (1 tick), and static-arena gates, but crossed still-solid destructible cells for 142 ticks where the authoritative pose never does, so prediction stays out of supported v1; `just prediction-comparison` reruns the matrix. | Closed by M11 with a rerun path for a terrain-aware candidate |
| M07-SUPERVISED | Milestone 07 user closeout decision, 2026-08-15 | Run a supervised physical-controller and keyboard/mouse normal-duration 1v1; inspect the complete match-state HUD at 1440x900 and supported layouts; judge simultaneous combat audio and weapon/arena counterplay. | **Closed by user disposition 2026-08-18.** Basic testing was okay; detailed controller/audio/layout polish is not claimed as passing and moves to `POST-V1-RELEASE-POLISH`. | Closed by M11 MVP acceptance |
| M09-SUPERVISED | Milestone 09 feedback review, 2026-08-16 | Run a supervised Hot Zone session with a physical controller and human perceptual audio judgment under simultaneous combat; both paired controllers were disconnected and no human listener was available during the automated playtest. | **Closed by user disposition 2026-08-18.** Basic testing was okay; detailed controller/audio/Hot Zone polish is deferred to `POST-V1-RELEASE-POLISH`. | Closed by M11 MVP acceptance |
| M09-BALANCE | Milestone 09 feedback review, 2026-08-16 | Collect normal-session Hot Zone control/contest distributions before tuning the 1,800-tick target or 10,800-tick cap; automation rarely holds uncontested control (one production match timed out at 3%). | **Deferred with user approval 2026-08-18.** Current values are acceptable for the v1 MVP; collect human distributions and tune before release under `POST-V1-RELEASE-POLISH`. | Pre-release polish |
| M08-ENV-SOURCE | Milestone 08 technical review, 2026-08-15 | Add `Environment` to stable combat source identity with an explicit attribution/exclusion policy when the first authoritative environmental damage source is introduced. | **Reviewed and retained 2026-08-18 (M11).** V1 still has no environmental outcome author; adding a dormant wire variant would create churn without exercised behavior. | First authoritative environmental-damage milestone |
| M08-BUILD-BOUNDARY | Milestone 08 technical review, 2026-08-15 | Extract the waiting-phase build transaction from `server/mod.rs` into `builds/server.rs` and retire the legacy replicated combat `SelectedBuild` in favor of one loadout identity/runtime authority model. | **Resolved 2026-08-18 (M11).** The authority transaction now belongs to `builds/server.rs`; the compatibility component was retired and the protocol/network matrix was rerun. | Closed by M11 |
| M10-ORG-TERRAIN-SPLITS | Milestone 10 implementation review, 2026-08-16 | Separate terrain client presentation from recovery/convergence and separate network convergence rules from server request handling while preserving the public terrain API, wire schema, and reset schedule. | **Resolved 2026-08-18 (M11).** Client presentation and network convergence were split into focused submodules with the public API and reset ordering preserved. | Closed by M11 |
| M11-ORG-COMBAT-CLIENT | External architecture review validated 2026-08-17 | Decompose `combat/client.rs` into preview, cue ingestion, world visuals, HUD/status, transient effects, and evidence ownership; preserve the existing chain first, then relax only measured dependencies behind schedule tests. | **Resolved 2026-08-18 (M11).** The client combat concerns were split into focused submodules while retaining explicit composition and schedule coverage. | Closed by M11 |
| M11-ORG-MOVEMENT-EFFECTS | External architecture review validated 2026-08-17 | Move authoritative movement out of `movement/mod.rs`, extract testable movement-decision helpers, and stage the composed payload transaction into planning, application, outcome, and commit helpers without changing its atomic schedule boundary. | **Resolved 2026-08-18 (M11).** Movement authority and composed-payload orchestration were decomposed with the atomic schedule boundary preserved and tested. | Closed by M11 |
| M11-LINT-SCOPE | External architecture review validated 2026-08-17 | Remove production module-wide `type_complexity` and `needless_pass_by_value` suppressions where decomposition makes them unnecessary; keep any unavoidable Bevy signature exception item-scoped and justified. | **Resolved 2026-08-18 (M11).** Broad production suppressions were removed or narrowed to reviewable item-scoped Bevy signatures; both role-specific Clippy gates pass. | Closed by M11 |
| GAP-DESIGN-TERRAIN-RESERVATION | Milestone 10 user playtest, 2026-08-17 | Move terrain destruction from the provisional Arc Launcher carrier to an ultimate/special-item design; revisit crater radius/core-versus-ring feedback and the melee/terrain policy together. | The playtest established the product direction but v1 has no replacement carrier. Keeping the current Arc behavior preserves an exercised MVP path without prematurely designing the future item/ultimate. | Next destructive ultimate/item design; readability also reviewed in pre-release polish |
| POST-V1-RELEASE-POLISH | Final v1 user playtest, 2026-08-18 | Improve controller feel, audio mix, HUD/layout, combat and terrain readability, weapon/build balance, match length, and Hot Zone pacing before release. | Basic v1 testing was okay and no MVP blocker was reported; the user explicitly requested that improvement/tuning work happen later rather than expand v1 closeout. | Pre-release product/polish milestone |
| FUT-ARSENAL | Product direction clarification, 2026-08-14 | Persistent account-owned arsenal of brawlers, saved build identity/revisions, production weapon editor, and acquisition/entitlement flow. | M05 establishes recipe/resolution/runtime boundaries and M08 proves bounded in-memory customization; accounts, storage, currency, loot, and unlock policy are outside v1. | Post-v1 product planning |
| FUT-MAP-BUILDER | Product direction clarification, 2026-08-14 | Player-facing builder for bounded map recipes plus persistence, publishing, discovery, moderation, asset policy, and version migration. Players arrange approved presentation, terrain, geometry, entities, regions, spawns, and objective anchors but cannot author mode rules. | M06 must establish the recipe/preset/resolved/runtime boundary and server validation without expanding v1 into editor or platform services. | Future-version planning after M06 evidence |

## Planning and evidence rule

Every milestone has a `milestone-NN.md` file in this directory. That file is the source of truth for its research, validated technical specification, trackable implementation tasks, test evidence, playtest handoff, feedback decisions, and closeout learning.

Create the next milestone file just before research begins. Do not pre-author detailed specifications for distant milestones because earlier results may change their design.

Every milestone begins with research, a short technical design, and a small implementation plan. The design must answer:

- which documented requirements are in scope;
- which behavior is deliberately out of scope;
- ECS data ownership and lifecycle, including authored data, components, resources, and entities;
- relevant plugin composition across authoritative gameplay, networking, client input, and presentation;
- network inputs, replicated components, messages, authority, and recovery behavior when networking is in scope;
- Bevy state transitions, schedules, system-set ordering, deferred-command visibility, and cleanup rules where relevant;
- automated, headless, local-network, controller, performance, and visual test strategy as applicable;
- measurable exit criteria.

Each milestone follows the same loop:

1. Set the milestone to `Researching`; investigate dependencies, alternatives, risks, and open questions.
2. Write the technical specification, validated Bevy/Cargo composition, implementation tasks, tests, and exit criteria.
3. Set the milestone to `Specification review` and obtain user validation before production implementation begins.
4. Set the milestone to `Implementing`; complete the tracked tasks in small vertical slices across the applicable authoritative-gameplay, networking, client-input, and presentation concerns.
5. Set the milestone to `Verifying`; run unit and integration tests plus network and visual checks as applicable.
6. Set the milestone to `User playtest`; provide the user with a build/run path, scenario, controls, known limitations, and requested observations.
7. Set the milestone to `Feedback review`; incorporate accepted feedback and record deferred items in the roadmap backlog with rationale.
8. Run a learn-from-errors review, update the milestone record, and create or improve reusable skills when justified.
9. Mark the milestone `Complete` only after its exit criteria, feedback triage, evidence, and learning review are complete.

Art, UI, authoring tools, and infrastructure should support the current gameplay gate rather than outrun it.

## Bevy architecture and dependency policy

Architecture decisions follow the product and authority requirements first, then the checked-in Lightyear 0.29 material, verified Bevy 0.19 APIs, Bevy-native ECS/plugin/schedule patterns, and finally general Rust dependency hygiene. Server-oriented DDD or hexagonal architecture is not the default model for this game.

Use Bevy's `World` as the runtime model rather than maintaining a parallel domain model solely for layering:

- **Authored content/rules:** serializable fighter, weapon primitive, effect, map, mode, validation,
  and preset definitions or Bevy assets/configuration.
- **Player-authored build data:** a bounded brawler build and weapon recipe, distinct from both the
  content catalog and the immutable server-resolved match loadout.
- **Player-authored map data:** a bounded map recipe distinct from the map-content catalog,
  immutable server-resolved map, runtime map state, and developer-authored mode plugins.
- **Runtime gameplay state:** components, resources, entities, and state machines in the authoritative server `World`; client worlds contain replicated or predicted copies plus local presentation state.
- **Gameplay behavior:** focused systems grouped into cohesive plugins and scheduled explicitly, normally in fixed-step schedules when behavior affects networked simulation.
- **Networking:** Lightyear registration plugins for the concrete inputs, replicated components, and messages the game actually uses.
- **Client presentation:** client-only systems and components that observe replicated gameplay state or presentation messages and own rendering, audio, camera, HUD, and local feedback.
- **Composition roots:** client and server entry points that select Bevy base plugins, Brawler plugins, Cargo features, and process-level configuration.

Do not introduce service/use-case layers, repository abstractions, ports/adapters, facade crates, or duplicate DTO versions of ECS state without a concrete external boundary or tested need. It is acceptable for a serializable gameplay component to also be a Lightyear-replicated component when that is the simplest correct representation.

The Milestone 01 design must compare package, target, module, plugin, and Cargo-feature topologies. Create another package or crate only when it enforces a real platform, feature-isolation, compile-time, testing, or reuse boundary. Cargo feature unification must be evaluated explicitly.

Systems that genuinely execute on both the authoritative server and a predicted client should be reusable through a shared plugin or module. Server-only rules should remain server-only systems; client presentation need not execute gameplay systems. Test visibility and public APIs should remain as small as the current consumers permit.

Do not create generic registries, exhaustive plugin hierarchies, or system-set taxonomies before concrete behavior needs them. Declare ordering only where correctness requires it, and profile queries, archetypes, schedules, and asset behavior before adding performance-oriented structure.

The server is authoritative even when client and server run in the same process. Local and offline configurations must exercise the same authoritative gameplay path and validation; host-client convenience must not bypass server-owned outcomes.

## Physics and collision policy

Use Bevy and Lightyear from Milestone 01. Add Avian 2D when collision queries, contact handling, or generated terrain colliders justify it; do not let a general-purpose physics engine define weapon cadence, projectile trajectories, damage, payloads, status meters, or terrain brushes.

Recommended division:

- **Avian 2D, if adopted:** fighter/world shapes, static obstacles, overlap queries, raycasts, generated terrain colliders, and collision filtering.
- **Brawler ECS gameplay:** movement intent, projectile trajectories, firing cadence, hit validation, damage, payloads, effects, status meters, mode rules, and terrain brushes.
- **Lightyear:** input transport, replication, authoritative events, interpolation, and prediction/reconciliation where measured need justifies them.

The Milestone 03 design must choose the initial collision approach and define a collision matrix for fighters, projectiles, indestructible terrain, destructible terrain, objectives, pickups, hazards, and deployables. It must also define owner collision, friendly fire, and self-damage rules before combat content depends on them.

Prefer static and kinematic bodies. Full rigid-body simulation requires a specific gameplay need and a decision record.

## Network and protocol policy

Clients send intent, never authoritative results. The server owns build/weapon-recipe and map-recipe
validation/resolution, movement, firing, hits, damage, effects, deaths, abilities, terrain,
objectives, scores, respawns, mode rules, and victory.

The protocol design must address these concerns as they become relevant:

- protocol/build compatibility during connection;
- stable player, definition, match, and network-entity identifiers;
- finite-value, range, rate, sequence, and ownership validation for client input;
- ordered/reliable versus unordered/unreliable delivery by message class;
- duplicate and stale input/event handling;
- cleanup after rejection, timeout, disconnect, match restart, and process shutdown;
- join-in-progress and reconnect behavior;
- recovery when a client misses state-changing events;
- diagnostics for tick, latency, packet loss, entity ownership, and connection state.

Prefer Lightyear-native input handling, replicated ECS components, messages, observers, and registration plugins. Add custom transport DTOs or networking abstraction layers only when the concrete wire contract differs from the ECS representation or an external integration requires them.

Milestone 02 must give reconnect an explicit outcome. Session resumption may be deferred, but a reconnect must either cleanly join under documented rules or be deliberately rejected. Later systems such as destructible terrain must not assume that every client observed every historical event.

Support these development configurations without changing gameplay authority:

1. Dedicated server plus one client.
2. Dedicated server plus multiple local clients.
3. In-process loopback server/client for fast tests where Lightyear support makes this practical.

## Test and measurement policy

Hardening is continuous. Every milestone adds tests at the lowest useful level:

- focused pure-function tests where the rule is naturally independent of ECS;
- small `App`/`World` tests that run the relevant Bevy schedule for component, resource, lifecycle, and state-transition behavior;
- plugin-composition and system-order tests where configuration or ordering is part of correctness;
- serialization and protocol registration tests for network contracts;
- headless server/client integration tests for authority and replication;
- multi-process local tests for real connection lifecycle behavior;
- latency, loss, duplication, and jitter profiles once combat input exists;
- controller and visual checks for behavior that automation cannot judge well.

Tests that depend on time should advance Bevy fixed time or explicitly run the relevant schedule rather than wait on wall-clock sleeps. Random gameplay decisions must use an explicit seeded resource when repeatability matters.

Telemetry begins with combat in Milestones 04–05 and match metrics in Milestone 07. Milestone 11 improves analysis and reproduction; it does not postpone evidence collection until the end of v1.

## Milestone overview

| Milestone | Status | Deliverable | Plan |
|---|---|---|---|
| 01 | Complete | Rust and Bevy application foundation | [milestone-01.md](./milestone-01.md) |
| 02 | Complete | Network connection and replication sandbox | [milestone-02.md](./milestone-02.md) |
| 03 | Complete | Movement, aiming, and greybox collision | [milestone-03.md](./milestone-03.md) |
| 04 | Complete | Combat core | [milestone-04.md](./milestone-04.md) |
| 05 | Complete | Weapon composition and preset selection | [milestone-05.md](./milestone-05.md) |
| 06 | Complete | First map-recipe arena and presentation baseline | [milestone-06.md](./milestone-06.md) |
| 07 | Complete | Wipeout match loop | [milestone-07.md](./milestone-07.md) |
| 08 | Complete | Bounded brawler builds and abilities | [milestone-08.md](./milestone-08.md) |
| 09 | Complete | Hot Zone | [milestone-09.md](./milestone-09.md) |
| 10 | Complete | Quantized destructible terrain | [milestone-10.md](./milestone-10.md) |
| 11 | Complete | MVP playtest hardening and closeout | [milestone-11.md](./milestone-11.md) |

Create each milestone file just before its research phase so its specification reflects current evidence and prior milestone lessons. Use the zero-padded form `milestone-NN.md`.

## Milestone 01 — Rust and Bevy application foundation

### Deliverable

The smallest Bevy/Rust project topology that launches a macOS client and dedicated headless server predictably, with Bevy-native plugin composition and verified Cargo feature isolation.

### Scope

- pin the Rust toolchain and the exact Bevy and Lightyear versions; retain `Cargo.lock`;
- compare single-package and small-workspace options using actual Cargo feature graphs;
- select and document package, target, module, plugin, and Cargo-feature boundaries without assuming one crate per concern;
- compose a windowed client and minimal headless server through explicit Bevy plugins or composition functions;
- establish minimal protocol-registration and gameplay-plugin placement sufficient to prove composition and feature isolation;
- establish fixed-tick scheduling, required system-set ordering, and one source for tick configuration;
- configure rustfmt, Clippy, tests, structured logging, clean shutdown, and CI commands;
- document commands for one blank client and one blank dedicated server;
- document future asset/data/provenance locations without creating unused placeholder directories;
- defer transport endpoints, client identity, multi-client orchestration, Lightyear networking validation, and Avian 2D to the milestones that exercise them.

### Automated verification

- formatting, Clippy, and tests run in CI for every supported package/target/feature combination;
- `cargo metadata` and `cargo tree -e features` evidence proves the dedicated server does not pull client rendering, windowing, audio, device-input, or asset-presentation features;
- client, server, and minimal shared-plugin smoke tests construct their expected plugin sets without gameplay content;
- fixed-tick configuration and any declared schedule/system-set ordering are verified.

### Exit criteria

- the macOS client and headless server compile independently;
- the minimal gameplay and protocol-registration plugins compose into both applications without requiring separate crates;
- binaries contain composition and process setup rather than gameplay rules;
- a blank server and client launch and shut down predictably from documented commands;
- dependency versions, supported feature combinations, plugin composition, and feature-graph evidence are recorded.

## Milestone 02 — Network connection and replication sandbox

### Deliverable

Two local clients connect to one authoritative server and observe replicated server-owned placeholder entities.

### Scope

- Lightyear transport and connection lifecycle;
- protocol registration and protocol/build compatibility handshake;
- stable player and network-entity identifiers;
- server-owned player identity, ownership, and placeholder spawn;
- replicated transforms or placeholder state;
- connection, rejection, timeout, disconnect, reconnect, and shutdown outcomes;
- cleanup of all entities owned by a disconnected connection;
- explicit join-in-progress policy for the sandbox;
- minimal Lightyear registration for the replicated components, inputs, messages, and channels actually used by this sandbox, with documented delivery semantics;
- local multi-process test harness;
- in-process loopback harness where practical.

### Automated verification

- protocol registration succeeds in client and server test apps, and transmitted types serialize and round-trip where applicable;
- two clients connect and receive both server-owned players;
- rejected and disconnected connections leave no owned entities behind;
- duplicate connect/disconnect transitions are safe;
- reconnect follows the documented outcome.

### Exit criteria

- two clients connect to one server and see consistent entity identity;
- only the server creates and removes authoritative player entities;
- connection failure and rejection are visible to the client rather than hanging silently;
- disconnect, reconnect, and shutdown behavior are explicit and repeatable;
- the headless server runs without client assets or rendering.

## Milestone 03 — Movement, aiming, and greybox collision

### Deliverable

Controller-first movement and right-stick aiming, with keyboard/mouse support, operate authoritatively in a minimal replicated arena.

### Scope

- abstract actions for movement, aiming, primary fire, active item, ultimate, interact, cancel, pause, and scoreboard;
- Xbox-like controller mapping and device detection;
- WASD/mouse mapping using the same gameplay actions;
- controller deadzone, aim threshold, and last-valid-aim behavior;
- fixed-tick movement intent and facing;
- validation of non-finite, out-of-range, stale, duplicate, and excessive input;
- greybox bounds, static walls, team spawn markers, and collision layers;
- collision approach decision: custom queries or Avian 2D;
- owner, friendly-fire, and self-collision policy for future combat entities;
- camera follow, bounds, remote interpolation, and local render interpolation;
- measured decision on local prediction/reconciliation;
- client-side pause-menu behavior while the authoritative server continues running.

### Automated verification

- a fixed-schedule `App`/`World` test produces the expected movement for a known sequence of input ticks;
- invalid input cannot create non-finite or out-of-bounds state;
- two clients moving simultaneously remain server-authoritative;
- collision prevents fighters leaving bounds or crossing static walls;
- delayed, duplicated, and reordered input has a documented result.

### Exit criteria

- two players can move and aim simultaneously with consistent server state;
- controller and keyboard/mouse use the same actions and gameplay systems;
- neutral-stick behavior preserves the last valid aim direction;
- remote and fixed-tick render interpolation are visually acceptable;
- prediction is either implemented from evidence or explicitly deferred with captured test results.

## Milestone 04 — Combat core

### Deliverable

A single pulse weapon supports authoritative firing, hit resolution, damage, defeat, and sandbox reset.

### Scope

- distinct authored fighter data, selected build data, and runtime ECS components/resources without requiring separate architectural layers or crates;
- one fighter body profile with health and movement values;
- pulse weapon definition and runtime weapon state;
- fire-intent validation, ammo, cooldown, and reload;
- straight projectile movement, collision, ownership, and lifetime;
- authoritative hit, damage, defeat, and attribution events;
- sandbox defeat delay and reset, distinct from formal mode respawn rules;
- lifecycle rules for projectiles, effects, ammo, cooldowns, and ownership on defeat or disconnect;
- friendly-fire, self-damage, simultaneous-hit, and environmental-attribution decisions;
- fixed test dummy for repeatable hit tests;
- debug health/ammo HUD, hit confirmation, hit flash, and defeat feedback;
- initial combat telemetry: shots, hits, damage, defeats, and distance band;
- network delay, loss, duplication, and jitter profiles for combat testing.

### Automated verification

- ammo, reload, cooldown, damage, defeat, and reset state transitions have focused fixed-schedule ECS tests, using pure functions only where they are the natural unit;
- clients cannot fabricate firing cadence, hits, damage, deaths, or ammo;
- two clients receive the same authoritative impact and health outcome;
- duplicate or late fire intent cannot create extra authoritative shots;
- repeated defeat/reset cycles leave no stale projectile or ownership state.

### Exit criteria

- the complete one-weapon combat loop works through the dedicated server path;
- projectile outcomes remain authoritative under the initial impairment profiles;
- a fighter can be defeated and reset repeatedly without corrupting state;
- players can identify a hit and defeat from placeholder feedback;
- telemetry can be written or inspected locally.

## Milestone 05 — Weapon composition and preset selection

### Deliverable

Composable weapon-recipe, projectile, effect, and payload data/components/systems support four
server-validated preset choices without making those presets permanent weapon classes.

### Scope

- a typed weapon configuration (recipe plus approved presentation profile), four stable preset
  identifiers, code-owned safety ceilings, and validated authored primitive/rule data;
- content/rules, player-authored recipe shape, server-resolved weapon, selected build identity, and
  per-fighter runtime state remain distinct;
- firing patterns and delivery methods needed by the first four weapons;
- straight pulse projectile;
- short-range pellet spread;
- ballistic/lobbed splash projectile and circular explosion payload;
- melee arc;
- payload composition for direct damage, area damage, knockback, and basic slow;
- duration, stacking, refresh, and cleanup rules for immediate effects;
- collision behavior against fighters and terrain;
- server-validated weapon selection before a test round;
- deterministic preset-independent structural and fighter-context activation resolution used by
  every preset and shaped for later bounded custom recipes;
- controller-readable aim/range feedback, including a landing indicator for the lobbed weapon;
- presentation systems observe gameplay components/messages and own visual or audio effects; gameplay systems do not load or mutate presentation assets;
- weapon telemetry: use, shots, hit rate, damage, distance, defeats, and self-damage where allowed.

### Automated verification

- content/rules and preset recipes reject duplicate IDs, invalid ranges, and unsupported
  combinations, and authored policy cannot widen code-owned safety/wire bounds;
- every preset passes through the same configuration resolver, and a non-preset fixture proves that the
  representation is not coupled to the four preset IDs even though M05 exposes only presets;
- the four weapons use composable data, components, and focused systems without duplicating whole fighter-specific weapon implementations; genuinely different behavior may use a specialized system;
- pellet, splash, melee, knockback, and slow rules have repeatable fixed-schedule ECS tests;
- selected preset source, resolved recipe fingerprint/public configuration, and runtime state agree
  across server and clients;
- effects clean up correctly on expiry, defeat, disconnect, and sandbox reset.
- the generalized pipeline preserves M04's pre-combat disconnect cleanup, same-tick projectile
  collision, deterministic contact/event ordering, bounded evidence, and authoritative tick rules.

### Exit criteria

- all four weapons can be selected and played in the networked sandbox;
- values can be changed in data without rewriting combat code;
- adding a legal recipe in a test/content fixture does not require a new weapon-specific system or
  enum branch on preset identity;
- server and client agree on preset source, resolved recipe identity, and presentation events;
- each weapon has a measurable preferred distance, burst window, recovery window, and counterplay profile;
- bouncing, homing, curved steering, piercing, splitting, boomerang behavior, and accumulating status meters remain deferred.

## Milestone 06 — First map-recipe arena and presentation baseline

### Deliverable

One readable symmetrical built-in arena is parsed, validated, resolved, and instantiated from the
same bounded map-recipe representation intended for a future player map builder, using replaceable
provisional assets and minimal combat audio.

### Scope

- typed separation between map-content catalog, user-authorable `MapRecipe`, built-in `MapPreset`,
  immutable server-owned `ResolvedMap`, runtime map state, and developer-owned mode rules;
- versioned, canonical, round-trippable map-recipe format with stable map/preset, presentation,
  geometry, terrain, entity, region, spawn, and mode-anchor IDs;
- a deterministic preset-independent server resolver with bounds/count/complexity, reference,
  spawn-safety, and mode-compatibility validation;
- rectangular playable bounds, two team spawn areas, central open space, two side routes, permanent cover, and at least one chokepoint;
- a clearly marked region reserved for the later destruction milestone;
- authored static collision and camera bounds;
- provisional Sci-Fi Facility and Shape Characters assets, or documented primitive fallbacks;
- consistent pixel scale and filtering policy;
- team colors, health, ammo, selected weapon, aiming, and match-information HUD layout for controller play;
- connection and error-state presentation needed for local testing;
- projectile, impact, hit, defeat, and reload feedback;
- placeholder fire, hit, defeat, and session/readiness audio cues; formal match-state audio waits for
  Milestone 07's match lifecycle;
- asset manifest with source, author, license, URL, and import date;
- client-only visual/audio loading kept out of the headless server;
- no player-facing editor, arbitrary asset upload/path, script, custom component blob, or custom
  game-mode rule in v1.

### Automated and visual verification

- map data rejects invalid bounds/coordinates, excessive geometry/entity/region counts, duplicate
  IDs, blocked/unsafe spawn points, missing mode anchors, and unsupported references/combinations;
- canonical serialize/parse round trips preserve a recipe and semantically equivalent recipes
  resolve identically, so a future editor need not serialize an ECS `World`;
- the built-in arena and a legal non-preset map fixture resolve through the same path, and no map or
  mode system branches on the built-in preset ID;
- the server loads all gameplay-relevant map state without loading client visuals;
- client presentation is reconstructed from stable presentation references while collision,
  regions, spawns, and entities come from the authoritative resolved map;
- replacement of a fighter sprite or terrain texture does not change collision or gameplay state;
- controller play verifies HUD legibility, aim feedback, team recognition, and combat readability;
- common window aspect ratios preserve the playable view and critical HUD information.

### Exit criteria

- players can understand walkable space, cover, teams, health, ammo, weapon state, and incoming damage;
- the arena supports the intended range profiles of all four weapons;
- team re-entry routes do not obviously force immediate spawn trapping;
- placeholder art and audio can be replaced without changing simulation code;
- changing a legal map layout, visual references, geometry, regions, entities, or spawn/anchor
  placement in recipe data does not require a new system or mode-rule change;
- all server-required map data is independent of client-only assets.

## Milestone 07 — Wipeout match loop

### Deliverable

A complete, repeatable, short Wipeout-style match that supports a 2v2 test configuration.

### Scope

- a Wipeout rules plugin with mode resources/components and systems that respond to shared match lifecycle facts without adding Wipeout branches to fighter or weapon systems;
- explicit lobby/waiting, countdown, active, completed, and restart states using validated Bevy state or resource patterns;
- team assignment and team capacity;
- server-owned spawn selection, respawn delay, and spawn-protection policy;
- takedown attribution including simultaneous, self, and environmental outcomes;
- team score, match timer, score threshold, timeout resolution, and tie rules;
- disconnect handling during each match state;
- scoreboard overlay, results screen, and controller-accessible restart flow;
- match restart without process restart;
- deterministic simple combat bots only if needed to fill a 2v2 test; fixed dummies do not count as match participants;
- match telemetry: duration, time to first damage, fight duration, hit rate, damage by distance, defeat rate, respawn-to-defeat time, movement time, and score margin;
- cleanup of fighters, projectiles, effects, scores, timers, and ownership between matches.

### Automated verification

- mode-rule tests cover threshold victory, timeout, tie, simultaneous scoring, disconnect, and restart;
- repeated matches begin from clean state without entity or resource accumulation;
- four participants can complete a match using local clients, bots, or a documented combination;
- all clients receive identical final score, result, and match identifier;
- clients cannot authoritatively change teams, scores, respawns, timer, or victory state.

### Exit criteria

- a match starts, plays, ends, shows results, and restarts without restarting a process;
- a repeatable 2v2-capable test exists;
- the server owns all match rules and results;
- a normal match lasts roughly two to four minutes;
- captured telemetry is sufficient for the first weapon and arena comparison;
- the Wipeout implementation establishes reusable match lifecycle state and a plugin-composition pattern for later rule sets.

### Combat vertical-slice gate review

Before Milestone 08, conduct a playtest and technical review. Resolve blocking authority, input, collision, readability, cleanup, or match-loop problems before adding the build layer. Content quantity alone is not evidence that the gate passed.

## Milestone 08 — Bounded brawler builds and abilities

### Deliverable

Players choose a server-validated bounded brawler build whose weapon recipe, ultimate, and passive
choices create a recognizable combat pattern.

### Scope

- distinct content/rule definitions, player-authored build/weapon recipe, server-resolved match
  loadout, fighter base data, and runtime ECS state;
- one compositional primary-weapon recipe, one ultimate, and two passive item slots;
- fixed build-point budget and mutually exclusive families where needed;
- at least one bounded non-preset weapon variation using M05 primitives; precise editable axes,
  costs, and UI are decided during M08 research rather than pre-authored here;
- four to six passive items that change decisions or timing windows;
- at least one mobility modifier and one defensive modifier;
- ultimate resource and explicit charge-source rules;
- dash ultimate;
- bounded-lifetime deployable sentry ultimate;
- targeting, ownership, lifetime, cleanup, and defeat behavior for deployables;
- Runner, Bruiser, Controller, and Duelist presets using only implemented weapons, ultimates, and passives;
- server validation for build legality and ability use;
- ability HUD, ready/cooldown/charge feedback, and placeholder audio;
- build and ultimate telemetry, including usage, charge time, damage or utility, defeats, and preset outcomes;
- one active item slot only if playtest evidence justifies it; active items are not required for this gate.

### Automated verification

- invalid weapon values/combinations, point totals, slot counts, IDs, and mutually exclusive
  combinations are rejected;
- dash and deployable rules have repeatable fixed-schedule ECS tests;
- deployables are removed on expiry, match cleanup, and all documented owner lifecycle events;
- build identity and runtime ability state replicate correctly;
- all four named presets are constructible from the implemented content inventory.
- a legal non-preset weapon recipe resolves, replicates, and plays through the same systems as the
  four presets without a preset-ID behavior branch.

### Exit criteria

- builds create visibly different combat behavior rather than only larger numbers;
- at least one bounded player-authored weapon variation changes behavior or a meaningful timing/
  range tradeoff and remains server-authoritative;
- players can understand ability availability and activation from a controller;
- the fixed budget creates explicit tradeoffs;
- preset-level match telemetry can be compared after a Wipeout match;
- ability, deployable, and passive state remains server-authoritative.

### First product-iteration gate review

Milestones 01–08 are the first iteration that tests the product direction. Review combat feel, network authority, controller usability, weapon counterplay, preset differentiation, match length, technical debt, and captured telemetry before expanding modes.

## Milestone 09 — Hot Zone

### Deliverable

One-zone Hot Zone reuses the existing combat, map, build, and match infrastructure.

Hot Zone intentionally precedes Heist and Gem Grab because it is the earliest direct test that fighter and weapon code can operate under a spatial-control mode rather than only elimination scoring.

### Scope

- capture volume represented as a mode-required anchor/region in a map recipe; Hot Zone rules own
  its meaning and validation requirements;
- authoritative occupancy evaluation;
- per-team progress and progress rate;
- contested, empty, and simultaneous-entry rules;
- progress HUD and objective feedback;
- timer expiry and tie handling;
- one objective-focused variant of the existing arena;
- mode-specific telemetry for occupancy, contest time, progress, and combat near the zone.

### Automated verification

- tests cover empty, single-team, contested, simultaneous, timeout, tie, and completion states;
- progress cannot advance more than once for duplicated input or events;
- Wipeout and Hot Zone reuse the same fighter, weapon, ability, and lifecycle components/plugins/systems; only their mode-rule composition differs;
- no fighter or weapon implementation contains Hot Zone-specific victory logic.

### Exit criteria

- the zone creates meaningful movement and combat decisions;
- all objective state is server-authoritative and replicated clearly;
- simultaneous occupancy and timer expiry behave predictably;
- the same build can enter either mode by selecting the appropriate rule plugin/configuration without substituting combat code.

## Milestone 10 — Quantized destructible terrain

### Deliverable

Server-authoritative quantized terrain destruction supports visual reconstruction, generated
collision, client state recovery, and bounded operation across every supported playable-map size.

### Scope

- fixed 8-world-unit cells on a global grid, 4-unit brush-center/radius quantization, sparse
  32×32-cell (256×256-unit) chunks, and `[u64; 16]` initial/current occupancy per chunk;
- sparse chunk allocation derived from destructible regions rather than whole-map allocation;
- support for the complete engine-owned playable-size range, currently 1024–4096 units wide and
  720–3072 units high, without treating the 192×192-unit built-in reservation as a capacity target;
- support for arbitrary legal map offsets up to 221 intersected chunks and 196,608 occupied cells,
  with recipe validation rejecting aggregate terrain outside those explicit ceilings;
- one visible built-in destructible terrain region plus multi-chunk, multi-region, minimum-map, and
  maximum-map verification fixtures;
- initial terrain occupancy/shape, placement, and stable chunk identities sourced from the resolved map
  recipe while runtime destruction/revisions remain server-owned;
- one delivery-level Arc Launcher `DestroyTerrain { radius: 48 }` world effect, kept separate from
  per-target damage/knockback/status payloads;
- deterministic circular brush rasterization from integer half-cell coordinates;
- authoritative occupancy bitsets and monotonically increasing revision;
- one nearest-filtered 32×32 RGBA client image per allocated chunk, with quantized crater/edge updates
  independent of collision generation;
- Avian/Parry voxel collision per chunk, with fresh prospective shapes, conditional orthogonal-
  neighbor reconciliation only for changed boundary topology, and atomic collider replacement after
  damage and before mode rules;
- projectile and fighter collision against changed terrain;
- defensive deterministic unstuck behavior for invalid injected/recovered states, with any use in a
  valid scenario treated as a test failure because v1 destruction only removes solidity;
- a dedicated bidirectional ordered-reliable Lightyear terrain channel carrying generation-tagged,
  revisioned integer brush events of at most 96 serialized bytes;
- revision-gap detection and recovery with a full current allocated-chunk bitset snapshot bounded to
  48 KiB, a 64-event client buffer, and rate-limited requests;
- client playability gated on matching immutable-map and terrain generations;
- terrain restoration inside the existing restart transaction before the new match commits;
- crater/debris presentation that does not affect gameplay truth, reusing the existing deduplicated
  Arc landed-impact audio rather than adding another authoritative cue or asset;
- a resolved map/mode capacity contract rather than a terrain assumption about team count or team
  size, with terrain verified through 24 simultaneously active fighters;
- engine-owned budgets including 24 admitted brushes and at most 221 distinct allocated collider
  rebuilds per fixed tick, four affected occupancy chunks per brush, and 64 cosmetic debris entities;
- dirty-chunk rebuild, collider-complexity, memory, recovery-size, and event-size measurements.

### Automated and network verification

- brush application and revision ordering have repeatable focused or `App`/`World` schedule tests;
- only occupancy-changed chunks and the conditionally required boundary-neighbor union rebuild;
- server collision changes match the authoritative occupancy grid;
- two connected clients plus an impaired or newly accepted recovery fixture reach the same terrain
  revision and crater state without changing the current active-match admission policy;
- duplicate, missing, and out-of-order terrain events trigger safe deduplication or recovery;
- terrain changes do not mutate unrelated objectives, props, or fighter state;
- minimum- and maximum-size maps, a near-maximum destructible region, multiple separated regions,
  brushes crossing chunk boundaries, varied team topologies, and 24 simultaneous fighter brushes
  remain within the declared memory, rebuild, collider, fixed-tick, and recovery budgets;
- recipe validation rejects aggregate destructible terrain that exceeds declared budgets.

### Exit criteria

- explosions create readable quantized holes and passages without making visible tiles authoritative;
- server collision remains the gameplay authority;
- clients can recover the current terrain state rather than requiring every historical event to have arrived live;
- collision rebuilds are bounded to dirty chunks and do not visibly stall the test scenario;
- unstuck behavior is predictable and cannot be client-authored;
- every supported playable-map size can host and recover destructible regions without whole-map
  allocation or demo-specific fixed capacity.

Defer structural collapse, fluids, material simulation, persistent terrain saves, internet-scale bandwidth optimization, and production snapshot compression.

### Gameplay MVP verification gate

At Milestone 10, verify every acceptance criterion in [Gameplay MVP](./gameplay-mvp.md) against a repeatable test or playtest result. Any criterion intentionally removed from the MVP must be updated there rather than silently ignored here.

## Milestone 11 — MVP playtest hardening and closeout

### Deliverable

A stable v1 MVP with useful measurement, diagnostics, repeatable test scenarios, and a completed feedback-and-learning cycle.

### Scope

- consolidate combat, build, match, Hot Zone, and terrain telemetry;
- fixed-tick replay or deterministic event logs where practical;
- named packet-loss, latency, duplication, jitter, and reconnect profiles;
- server load, tick-time, bandwidth, and entity-count measurements;
- a reproducible direct-UDP single-match baseline plus an audit of the dedicated server's
  composition, launch/readiness, process-global assumptions, and shutdown contract for v2 M01;
- build matchup and map/mode reports;
- automated fixed-schedule ECS tests for all implemented mode rules;
- crash and structured error reporting for local development;
- debug overlays for tick, latency, connection, authority, and entity ownership;
- input remapping, deadzone, aim-threshold, and controller settings;
- repeated-match, late-join, and reconnect soak scenarios;
- close the still-open Milestone 01–03, 05, 08, and 10 verification, playtest, feedback, and
  learning gates from one explicit ledger without rewriting their historical evidence;
- extract the server-owned build-selection transaction into `builds/server.rs`, remove the legacy
  replicated combat build component, and leave one selected-build/resolved-loadout/runtime-state
  authority model with an intentional protocol migration;
- decompose mixed combat-client, authoritative-movement, combat-effect, and terrain
  presentation/recovery concerns while preserving public paths, role gates, fixed-tick ordering,
  message ordering, and deferred-command visibility;
- replace broad production complexity/pass-by-value Clippy suppressions with narrow justified
  exceptions after the owning systems have been inspected and decomposed;
- final user playtest and feedback triage;
- technical-debt review and evidence-based handoff to the proposed next-version architecture;
- explicitly no v2 supervisor, router, IPC, worker manifest, lobby, queue, or multi-worker
  implementation during v1 closeout;
- learn-from-errors review and justified skill creation or improvement.

### Automated and playtest verification

- run the full unit, integration, network-impairment, and repeated-match suite;
- lock current authority, schedule, protocol, recovery, and telemetry behavior before each
  organization slice, then prove equivalence or explicitly validate an intended migration;
- visually verify the supported controller and keyboard/mouse paths;
- deliver a documented playtest build and scenario to the user;
- record each feedback item as implement now, backlog, rejected with rationale, or needs more evidence.

### Exit criteria

- major combat, networking, terrain, and mode bugs can be reproduced from logs or deterministic scenarios;
- balance decisions use captured data and playtests;
- repeatable two-client, 2v2, and broader multi-client sessions are documented;
- server tick and bandwidth limits are measured for the current content set;
- the proposed v2 architecture has a bounded handoff record and direct-UDP worker comparison
  baseline without expanding M11 into v2 infrastructure;
- build selection, movement, combat payload resolution, and client presentation have cohesive
  ownership boundaries with schedule and protocol invariants covered by focused tests;
- production module-wide complexity suppressions touched by M11 are removed or have an explicit
  reviewed disposition; green Clippy is not achieved by widening suppression scope;
- every earlier non-complete v1 milestone is either completed from recorded evidence or remains
  visibly blocked/open with a user-approved disposition, so M11 cannot conceal an earlier gate;
- user feedback is incorporated or triaged with rationale;
- milestone learnings and recurring errors are recorded;
- useful skills are created or improved when the learning is reusable;
- v1 is accepted by the user and next work is prioritized by evidence rather than content volume.

### v1 completion gate

Version 1 is complete only when Milestones 01–11 are marked `Complete`, the gameplay MVP acceptance criteria have evidence, the final playtest feedback is triaged, and the closeout learning review is recorded.

## Future-version candidate backlog

These candidates preserve the longer-term design direction without assigning scope or order before v1 evidence exists:

- **Heist:** durable team objectives with independently balanced objective damage.
- **Gem Grab:** authoritative pickups, carrier state, exact-once drops, and a winning countdown.
- **Solo Showdown:** free-for-all placement, loot, and a shrinking boundary; duo/trio variants later.
- **Systemic status interaction:** one target-owned cold-to-freeze meter contributed to by compatible projectiles and areas.
- **Advanced projectiles:** bouncing, homing, curved steering, piercing, splitting, boomerang, and delayed behavior.
- **Environment surfaces and concealment:** tall grass or another concealment region, a spell-created concealment area, speedway and slow surfaces, one readable hazard, and server-owned per-client network visibility following the [environment gameplay direction](../../09-environment-gameplay.md).
- **Player map builder:** edit and preview bounded map recipes using approved presentation, terrain,
  geometry, entity, region, spawn, and mode-anchor catalogs; server validation remains authoritative
  and game modes remain developer-authored.

Move a candidate into `docs/implementation/vN/roadmap.md` only when a future version is intentionally scoped.

## Explicitly outside v1

- production matchmaking, parties, authentication, and session services;
- accounts, persistence, progression, currencies, and monetization;
- ranked and live-operations systems;
- final art direction and production animation;
- complete touch controls;
- platform certification and store release work;
- production anti-cheat, internet fleet orchestration, and global hosting;
- procedural maps or automatic balance generation;
- production user-map editing, persistence, distribution, publishing, discovery, moderation,
  arbitrary asset upload, and map-version migration;
- persistent terrain saving, structural collapse, fluids, and material simulation.
