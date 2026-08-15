# Milestone 08 — Bounded brawler builds and abilities

## Tracking

| Field | Value |
|---|---|
| Version | v1 — gameplay MVP |
| Roadmap | [roadmap.md](./roadmap.md) |
| Status | Specification review |
| Research | Complete; product, live M07 worktree, pinned references, exact dependency sources, primary documentation, and external specification review incorporated on 2026-08-15 |
| Specification validation | Awaiting user review |
| Implementation | Not started; awaiting specification validation and an exact green M08 starting baseline |
| Verification | Not started |
| User validation/playtest | Not started |

Milestone 07 completed on 2026-08-15 after feedback triage, remediation, technical-gate verification,
learning review, and explicit user closeout. Its remaining supervised physical-controller, perceptual-
audio, full-aspect/state-matrix, and normal-duration observations are tracked for M11 and are not
represented as passing evidence. M08 is now the current delivery milestone; reconcile its starting
baseline with the accepted M07 changes before entering `Implementing`.

## Outcome

Before a Wipeout match, a player selects one of four named brawler builds or edits one bounded
non-preset Pulse Sidearm recipe. Every candidate is a typed, finite recipe sent as intent to the
dedicated server. The server validates content IDs, slot counts, mutual exclusions, recipe axes,
compatibility, and a fixed 12-point budget, then installs one immutable resolved match loadout.

Each legal build contains one primary weapon, one ultimate, and exactly two distinct passive items.
Runner, Bruiser, Controller, and Duelist are ordinary legal preset recipes rather than runtime
classes. Six passives create body, positioning, response-window, economy, and control tradeoffs. A
damage-earned ultimate meter powers either a collision-safe forward dash or one destructible,
bounded-lifetime sentry. Build, ability, passive, deployable, damage, cleanup, and match outcomes
remain server-authoritative and recover through replicated durable state.

This is the first product-iteration gate: it must demonstrate that composing a brawler changes the
way a player approaches a fight, not merely the label above the existing weapon preset.

## Decisions requiring specification validation

1. Keep one package and add cohesive `builds` and `abilities` module families. `builds` owns authored
   inventory, candidate recipes, validation, deterministic resolution, fingerprints, and immutable
   loadouts. `abilities` owns ultimate/passive runtime systems and deployables. Do not add an arsenal
   service, generic ability scripting framework, trait-per-item architecture, or a crate boundary.
2. Embed and synchronously validate `content/v1/builds.ron` alongside weapon and map content. The
   build catalog references stable weapon, ultimate, passive, presentation, and preset IDs and
   contributes canonical material to the shared gameplay-content fingerprint.
3. Accept only a bounded typed `BrawlerBuildRecipe`: one `WeaponChoice`, one `UltimateDefinitionId`,
   and exactly two `PassiveDefinitionId`s. A custom weapon choice exposes three discrete Pulse axes;
   it does not accept arbitrary floats, maps, component names, scripts, asset paths, or resolved
   values from the client.
4. Use a fixed 12-point budget. Weapon, ultimate, and passive costs all contribute. Duplicate
   passives and the two frame-family passives together are illegal. A build may spend fewer than 12
   points; unused points grant no hidden benefit.
5. Preserve the four M05 weapon configurations unchanged as preset weapon choices. Add one custom
   Pulse recipe family with discrete power, reach, and magazine axes. Runtime fire/delivery/effect
   systems execute the resulting `ResolvedWeapon` without a preset-ID branch.
6. Implement exactly six passive definitions: Lightweight Frame, Reinforced Frame, Adrenal Response,
   Close Quarters, Quick Cycle, and Tenacity. Do not add an active item in M08; the first product gate
   does not yet contain evidence that another input, cooldown, and HUD economy improves the loop.
7. Use one 0–1,000 ultimate meter. During active play, applied hostile primary-weapon damage grants
   five charge per damage and hostile fighter damage received grants three. Self, friendly-invalid,
   protected, environmental, deployable-target, dash, and sentry damage grant none. Charge survives
   defeat/respawn, resets on accepted activation and match restart, and is capped deterministically.
8. The dash travels up to 360 world units over 18 fixed ticks along the committed activation facing,
   stops at the furthest terrain-safe point, deals 35 damage plus 450 world units/s knockback for six
   ticks once to each of at most eight crossed hostiles, grants no invulnerability, and prevents
   primary fire while active.
9. The sentry uses a 20-unit-radius non-blocking target collider and searches backward in 8-unit
   steps from a preferred 96-unit owner-facing offset to a minimum 56-unit offset. It has 80 health,
   lasts at most 720 ticks, acquires the nearest visible hostile within 480 units with stable
   tie-breaking, and fires a 10-damage straight projectile every 30 ticks. It can be damaged by
   hostile primary weapons and abilities, awards no Wipeout point when destroyed, and despawns on
   owner defeat/disconnect, match completion/restart, expiry, or replacement. Activation and every
   later sentry shot count as owner-authored attacks for spawn-protection break rules.
10. A legal build may be submitted or replaced only while the current match is `Waiting` and that
    participant is not ready. Ready locks the loadout for countdown/active/completed. Restart retains
    the prior recipe as the default but clears ready and reopens legal replacement.
11. Reuse M07's mode-neutral combat facts as one ordered per-tick fan-out. Ultimate charge, passive
    triggers, match scoring, and telemetry inspect the same sorted facts before the match consumer
    clears them. No consumer scrapes cues, HUD state, or another telemetry aggregate.
12. Extend the existing controller-first selection overlay into a compact preset/custom build flow.
    D-pad/stick and keyboard navigation must edit every field, show point cost and exact validation
    feedback, submit with South/A or Space/Enter, and return with East/B or Escape. No mouse or
    pointer is required.

Changing authority, candidate grammar, budget/slot policy, fixed lifecycle semantics, charge
sources, deployable targetability, cleanup, wire shapes, or the active-item decision returns M08 to
specification review. Numeric tuning may change during implementation/playtest when the data shape,
tradeoff direction, and authority contracts remain intact and the change is recorded here.

## Source requirements

- [Product direction](../../00-product-direction.md): player-authored builds, bounded power,
  recognizable play patterns, short feedback cycles, combat readability, and network-first rules.
- [Fighter model](../../02-fighter-model.md): content/build/resolved/runtime separation, recommended
  M08 attributes, initial loadout slots, in-memory customization, and no persistence/entitlements.
- [Weapons and abilities](../../03-weapons-and-abilities.md): typed compositional recipes, validation
  layers, payload/runtime separation, two initial ultimates, passive quality bar, and lifecycle rules.
- [Gameplay MVP](../../05-gameplay-mvp.md): four named builds, two ultimates, four-to-six passives,
  one bounded non-preset weapon, controller parity, telemetry, and first-iteration acceptance.
- [Network architecture](../../08-network-architecture.md): selection as intent, immutable server
  loadouts, authoritative ability/deployable outcomes, stable identities, and recovery.
- [Version 1 roadmap](./roadmap.md): M08 scope, automated verification, exit criteria, and the M07
  combat vertical-slice gate.
- [Milestone 05](./milestone-05.md): weapon recipe catalog/resolver, four preset-independent runtime
  paths, selection request epoch, combat effects, fingerprints, telemetry, and process evidence.
- [Milestone 07](./milestone-07.md): live match phase/roster, outcome facts, respawn/protection,
  restart cleanup, HUD, telemetry, process automation, and unresolved playtest gates.

## Scope boundaries

### In scope

- one embedded, versioned, typed build/ability/passive/preset catalog and one code-owned engine
  ceiling policy;
- `BrawlerBuildRecipe`, four ordinary build presets, stable build identity/revision, canonical
  fingerprint, `ResolvedMatchLoadout`, and separate runtime ability/passive/deployable state;
- one primary weapon, one ultimate, exactly two passive slots, 12 points, duplicate rejection, and
  frame-family mutual exclusion;
- discrete bounded custom Pulse power/reach/magazine axes resolved through the M05 weapon validator;
- server-side waiting-phase build replacement, request idempotency, lock rules, input epoch reset,
  and explicit rejection outcomes;
- build-aware maximum health and movement speed, with lifecycle reset/respawn reading resolved stats;
- ultimate charge from authoritative combat facts, dash execution, sentry placement/targeting/fire,
  ability damage attribution, and deployable damage/destruction;
- six passive definitions and the minimum runtime state required for their exact rules;
- replicated loadout identity, resolved public snapshot, ultimate/passive state, deployable identity,
  health, deadlines, and presentation cues;
- build editor/selection overlay, loadout/point summary, ultimate meter/state, passive feedback,
  sentry health/lifetime, controller-readable rejection, placeholder visuals, and bounded audio;
- build/ability/passive/deployable telemetry, deterministic summaries, local network/performance
  evidence, and the first product-iteration gate review.

### Out of scope

- accounts, authentication, entitlements, inventories, acquisition, currencies, loot, unlocks,
  persistence, saved arsenals, build revision history, cloud storage, sharing, import/export, or a
  production weapon editor;
- arbitrary numeric sliders, custom scripts, behavior graphs, user-defined effects, asset upload,
  remote content, or client-selected resolved ECS/component data;
- active items, consumables, cooldown item slot, equipment pickups, ultimate swapping during active
  play, build counter-picking during countdown, or loadout changes after ready;
- additional weapon delivery/payload primitives, accumulating statuses, shields, healing, stealth,
  vision, critical hits, armor, lifesteal, objective interaction bonuses, or terrain permissions;
- more ultimates, multiple simultaneous sentries per owner, sentry upgrades, sentry movement,
  sentry navigation, sentry repair, ability prediction, or lag compensation;
- character classes, role-restricted systems, multiple body sizes/colliders, cosmetics, animations,
  bespoke models, production VFX/audio, rumble, aim assist, or accessibility settings;
- Hot Zone objective rules from M09, destructible-terrain behavior from M10, matchmaking, production
  analytics upload, balance automation, bots, replay, spectators, or internet hosting.

## Research questions and conclusions

### What M07 gate must M08 satisfy before implementation?

M07 is complete and its accepted implementation exposes the match, outcome, lifecycle, replication,
HUD, evidence, and cleanup seams M08 needs. The closeout explicitly deferred physical-controller,
perceptual-audio, full-aspect/state-matrix, and normal-duration observations to M11; those observations
may inform later presentation or tuning, but they do not reopen the accepted M07 authority contract.

Keep M08 in `Specification review` until the user validates this specification. Before it becomes
`Implementing`, record the exact post-M07 starting commit and rerun the accepted baseline.

### Why a build catalog in addition to the weapon catalog?

M05 correctly owns weapon grammar and validation, while M08 introduces cross-domain composition:
fighter stats, a resolved weapon, an ultimate, passives, budget, slot families, presentation, and
named brawler presets. Adding these rules to `weapons.ron` would make weapons own fighter/ability
policy. A small `builds.ron` can reference stable weapon IDs and custom weapon choices while leaving
the existing weapon catalog authoritative for structural recipe ceilings.

Build resolution therefore has two passes: weapon resolution first, then loadout compatibility and
balance. The weapon resolver remains usable independently; entitlement validation remains absent.

### Why discrete custom axes instead of direct numeric fields?

M05's full `WeaponRecipe` is intentionally capable but exposes many coupled values and safety bounds.
Sending arbitrary floats would create a poor controller editor and make the first balance surface too
large. Three typed axes provide 27 combinations while the server remains the sole author of exact
numeric transformations and cost. The resolved result is still an ordinary non-preset M05 recipe
with `source_preset_id = None`.

The initial custom Pulse axes are:

| Axis | Choice | Resolved change from Pulse preset 1 | Cost delta |
|---|---|---|---|
| Power | Light | damage 20, fire cooldown 9 ticks | 0 |
| Power | Balanced | damage 25, fire cooldown 12 ticks | 0 |
| Power | Heavy | damage 30, fire cooldown 15 ticks | +1 |
| Reach | Compact | speed 1,020, range 750 | 0 |
| Reach | Standard | speed 900, range 900 | 0 |
| Reach | Long | speed 780, range 1,050 | +1 |
| Magazine | Quick | capacity 4, refill 42 ticks | 0 |
| Magazine | Standard | capacity 6, refill 60 ticks | 0 |
| Magazine | Expanded | capacity 8, refill 78 ticks | +1 |

The resolver starts from preset 1's developer-authored configuration, applies the axes in fixed
power/reach/magazine order, sets straight-projectile lifetime to
`ceil(range * SIMULATION_TICK_HZ / speed)` (45/60/81 ticks for Compact/Standard/Long), clears preset
identity, and runs the normal structural resolver. Checked arithmetic and the existing lifetime
ceiling reject a value that cannot be represented safely. This is not runtime branching on preset
identity.

### What makes the 12-point budget meaningful?

The initial inventory is deliberately small and legible:

| Choice | Cost |
|---|---:|
| Pulse / Impact Blade weapon | 4 |
| Scatter Cannon / Arc Launcher weapon | 5 |
| Custom Pulse | 4 plus selected axis deltas |
| Dash ultimate | 3 |
| Sentry ultimate | 4 |
| Lightweight / Reinforced / Adrenal / Close Quarters / Quick Cycle | 2 each |
| Tenacity | 1 |

The most expensive weapon/ultimate/two-passive combinations exceed 12, and a fully upgraded custom
Pulse cannot be paired with an ultimate and two legal passives. The budget therefore excludes real
combinations without requiring players to optimize a large spreadsheet. Exact costs are balance
hypotheses; the fixed-budget and explicit-cost contract is the specification decision.

### How do the named builds avoid becoming classes?

The four presets are catalog entries containing ordinary `BrawlerBuildRecipe`s:

| Preset | Weapon | Ultimate | Passives | Cost | Intended pattern |
|---|---|---|---|---:|---|
| Runner | Pulse Sidearm | Dash | Lightweight Frame, Adrenal Response | 11 | mobile mid-range pressure and escape windows |
| Bruiser | Scatter Cannon | Dash | Reinforced Frame, Tenacity | 11 | durable close-range commitment with control resistance |
| Controller | Arc Launcher | Sentry | Quick Cycle, Tenacity | 12 | cover punishment and temporary area denial |
| Duelist | Impact Blade | Dash | Close Quarters, Lightweight Frame | 11 | fragile engage/disengage melee pressure |

Resolution never switches on these IDs. Telemetry may attribute a preset ID, while runtime behavior
comes only from the resolved weapon, stats, ultimate, and passive inventory.

### What are the exact passive rules?

| Passive | Rule |
|---|---|
| No frame passive (base fighter) | maximum health 100; movement speed 320 |
| Lightweight Frame | maximum health 85; movement speed 360; mutually exclusive with Reinforced |
| Reinforced Frame | maximum health 120; movement speed 288; mutually exclusive with Lightweight |
| Adrenal Response | when rearmed, hostile fighter damage grants +15% directed movement for 90 ticks and sets rearm to trigger tick + 240; damage during rearm neither refreshes nor stacks; external motion unchanged |
| Close Quarters | primary damage is 115% at up to 240 units, 85% at 480 or more, linearly interpolated between; distance is immutable attack origin to each affected target center when its payload applies; round once after clamping |
| Quick Cycle | credited hostile primary-weapon fighter defeat primes one 40%-shorter next `Magazine` refill or `Charges` recharge; Impact Blade therefore benefits; one prime maximum; cleared on owner defeat/restart |
| Tenacity | hostile slow duration is reduced by 35%, rounded up to at least one tick; slow magnitude/stacking is unchanged |

Body stats resolve once into the immutable loadout. Reactive windows are bounded runtime state.
Close Quarters intentionally gives Impact Blade its close-range bonus for every legal melee target.
For Arc Launcher and other area payloads, each target receives its own modifier from the launch-time
attack origin rather than from the impact center or the owner's later position. Passives never read
client-local presentation state or directly change match score.

### How should multiple systems consume M07 combat facts?

The live M07 resource is a bounded same-tick vector drained by Wipeout scoring. M08 needs the same
facts for charge and passive triggers. Multiple independent drains would make results depend on
system order, while copying facts into domain-specific telemetry streams would risk divergence.

Keep one authoritative vector, sort once by stable `CombatEventId`, and expose ordered systems in
`MatchSet::Outcomes`: passive/charge observation, Wipeout scoring/result, build telemetry, then one
clear. Each stateful consumer records the relevant stable event ID where idempotency is needed.
Bevy 0.19 messages support multiple readers, but converting this already-bounded current-tick
transaction adds no value unless implementation evidence shows the resource fan-out is unwieldy.

### How should dash movement interact with terrain and combat?

Use Avian's one-shot `SpatialQuery` shape cast against terrain at activation to select the furthest
safe endpoint for the fighter circle. Advance a bounded `DashState` along that segment for 18 fixed
ticks and sweep the travelled segment for hostile fighter/deployable contacts after pose movement.
Stable target identity and a bounded server-local hit set enforce once-per-dash damage.

Do not teleport through cover, reuse render interpolation as gameplay, add invulnerability, or hide
dash inside ordinary input shaping. The movement owner reads resolved speed for normal movement and
explicit dash state for dash movement; knockback/external motion does not steer the dash.

### What owns a sentry and how does it recover?

The server spawns a replicated sentry with stable `DeployableId`, `MatchId`, owner `PlayerId` and
`NetworkEntityId`, team, definition ID, health, expiry, pose, and ability-source identity. It does
not use the owner's process-local fighter entity on the wire. A server-local owner index supports
bounded cleanup and one-sentry enforcement.

Use ordinary server `Replicate::to_clients(All)`, not `ControlledBy`: sentry behavior is never client
controlled, and M08 explicitly destroys it on owner disconnect/defeat. Durable health/deadline data
supports late replication; transient acquire/fire/impact/destruction presentation uses bounded cues.
The sentry's non-blocking target collider is on a dedicated deployable layer.

### Do current dependencies support the plan?

Yes. The exact Bevy 0.19.1 source identifies `FixedUpdate` for physics, AI, networking, and game
rules and `FixedPostUpdate` for reacting to fixed-step changes. Avian 0.7 supplies the shape casts,
intersection queries, filters, and move-and-slide already exercised by Brawler. Lightyear 0.29
replicates registered components durably and propagates server despawns. No dependency addition is
justified.

The checked-in Bevy tree is 0.20-dev, so exact code transfer must continue to use the installed
0.19.1 crate source or version-pinned official documentation. The local Lightyear and Avian
snapshots match the Cargo versions.

## Research log

| Date | Source | Finding | Decision |
|---|---|---|---|
| 2026-08-15 | `docs/{00-product-direction,02-fighter-model,03-weapons-and-abilities,05-gameplay-mvp,08-network-architecture}.md` and `docs/implementation/v1/roadmap.md` | M08 is the first gate for bounded player-authored buildcraft: one weapon, one ultimate, two passives, four presets, and a non-preset weapon variation. | Keep editor/persistence small and in-memory; require observable tradeoffs and server validation. |
| 2026-08-15 | Live `src/{combat,movement,matchplay,server,client,protocol,content}.rs`, `content/v1/weapons.ron`, and tests | M05 already resolves arbitrary configurations; M07 adds waiting/ready lock, fixed outcome facts, lifecycle cleanup, stable IDs, HUD, four-client harness, and process evidence. Current selection and movement still assume a single preset/fighter speed. | Extend the existing seams; replace weapon selection with waiting-phase build resolution and move runtime stats into the resolved loadout. |
| 2026-08-15 | [Milestone 07](./milestone-07.md) closeout record | Automated/network/role-isolation/performance/process gates and the remediated technical review are green. The user closed M07 with supervised physical-controller, perceptual-audio, full-aspect/state-matrix, and normal-duration observations explicitly deferred to M11. | M08 becomes the current specification review; gate implementation on specification approval and exact baseline reconciliation, not on the deferred M11 observations. |
| 2026-08-15 | `references/lightyear/book/src/concepts/replication/{protocol,replicate}.md`, `references/lightyear/examples/avian_3d/src/{server,shared}.rs`, and installed Lightyear 0.29 source `lightyear_replication/src/control.rs` | Registered components converge durable entity state; server despawn propagates; `ControlledBy(SessionBased)` is for link-controlled lifetime and automatically despawns on disconnect. | Replicate sentries as server-owned entities without `ControlledBy`; clean them through explicit stable owner/match lifecycle rules. |
| 2026-08-15 | `references/avian/crates/avian2d/examples/{move_and_slide_2d,kinematic_character_2d/plugin}.rs`, installed Avian 0.7 source, and [Avian 0.7 `SpatialQuery`](https://docs.rs/avian2d/0.7.0/avian2d/spatial_query/struct.SpatialQuery.html) | One-shot filtered shape casts and intersections fit activation/targeting, while current Brawler movement already uses `MoveAndSlide` and combat sweeps after physics refresh. | Shape-cast dash endpoints/placement, keep schedule-visible sweeps, and add a dedicated deployable query layer. |
| 2026-08-15 | Installed Bevy 0.19.1 `bevy_app/src/main_schedule.rs` and [Bevy 0.19 messages](https://docs.rs/bevy/0.19.0/bevy/ecs/message/struct.Messages.html) | Fixed gameplay belongs in fixed schedules; messages allow multiple readers but require explicit producer/consumer ordering. | Preserve the bounded current-tick fact transaction and configure an explicit fan-out/clear chain. |
| 2026-08-15 | [Lightyear 0.29 documentation](https://docs.rs/lightyear/0.29.0/lightyear/) and version-pinned local examples | Ordinary replicated components and server entity despawn cover immutable loadouts, dynamic ability state, deployable state, and late/current recovery; no prediction is required. | Extend `protocol.rs`, bump protocol/content versions, and keep ability execution server-only. |
| 2026-08-15 | External M08 specification review checked against current M05–M07 data and runtime rules | Architecture was accepted; missing base stats, placement/knockback numbers, passive interactions, lifetime formula, acquisition cadence evidence, and charge-pacing target were identified. Impact Blade already uses the `Charges` economy rather than having no refill behavior. | Pin the missing values and interactions, add cadence coverage and a soft charge-pacing gate, and retain the approved architecture. |

## Technical specification

### Application and module composition

Keep the package and role features. Add focused ownership boundaries:

```text
content/v1/builds.ron
src/builds/
  mod.rs                 shared composition and intentional public API
  model.rs               stable IDs, candidate recipe, identity, resolved loadout/stats
  definitions/           embedded catalog, validation, normalization, costs, resolution, tests
  server.rs              waiting-phase request validation and atomic loadout installation
  telemetry.rs           bounded build/preset/recipe aggregates and summaries
  tests.rs               pure and small-App build/lifecycle tests
src/abilities/
  mod.rs                 ability sets, shared runtime state, plugin composition
  charge.rs              combat-fact charge and accepted-use economy
  dash.rs                activation endpoint, runtime movement/contact, cleanup
  sentry.rs              placement, ownership, targetability, targeting/fire, cleanup
  passives.rs            resolved modifiers and bounded trigger/runtime rules
  client.rs              previews/cues/HUD helpers (client-gated)
  tests.rs               fixed-schedule ability/passive tests
tests/network/builds.rs  selection, authority, recovery, ability, deployable scenarios
```

Keep combat responsible for generic attack delivery, payload application, damageable-target routing,
combat facts, and cues. Movement remains the sole owner of final fighter pose. Matchplay remains the
owner of phase, score, respawn, completion, and restart. `abilities` supplies explicit inputs to
those owners and contains no Wipeout victory branch.

Recommended plugins:

| Plugin | Installed in | Responsibility |
|---|---|---|
| `BuildContentPlugin` | client/server/tests | Load and validate catalog; expose definitions and canonical fingerprint material; no runtime mutation |
| `BuildModelPlugin` | client/server/tests | Register shared local data/messages needed by composition |
| `ServerBuildPlugin` | server/tests | Process link-scoped build requests, resolve/replace waiting loadouts, reset input/runtime epochs, record selection evidence |
| `ServerAbilityPlugin` | server/tests | Charge/passive fact observation, dash/sentry activation/execution, deployable targetability and cleanup |
| `ClientAbilityPresentationPlugin` | windowed client | Observe replicated state/cues and update overlay/HUD/visual/audio only |
| existing `ProtocolPlugin` extension | client/server/tests | Register requests/outcomes, components, cues, and fingerprint changes |

Do not add one plugin per passive or one system type per definition. Passive behavior is a bounded
enum implemented by focused systems grouped by trigger phase.

### Authored, selected, resolved, and runtime data

Recommended shared shapes (field visibility remains minimal):

```text
BuildPresetId(u16)
BuildRevision(u16)
BuildRecipeFingerprint(u64)
UltimateDefinitionId(u16)
PassiveDefinitionId(u16)
DeployableId(u64)

BrawlerBuildRecipe
  weapon: WeaponChoice
  ultimate: UltimateDefinitionId
  passives: [PassiveDefinitionId; 2]

WeaponChoice
  Preset(WeaponPresetId)
  CustomPulse { power: PulsePower, reach: PulseReach, magazine: PulseMagazine }

SelectedBuild component
  source_build_preset_id: Option<BuildPresetId>
  recipe_fingerprint: BuildRecipeFingerprint
  revision: BuildRevision

ResolvedMatchLoadout component
  identity
  total_points
  fighter_stats: ResolvedFighterStats
  primary_weapon: ResolvedWeapon
  ultimate: ResolvedUltimate
  passives: [ResolvedPassive; 2]

AbilityState component
  charge: u16
  phase: Charging | Ready | Dashing { ends_at_tick } |
         Deployed { deployable_id, expires_at_tick }

PassiveRuntimeState component
  adrenaline_until_tick / rearm_at_tick
  quick_cycle_primed
```

The actual resolved loadout is immutable outside a legal waiting replacement. Mutable charge,
deadlines, triggers, health, weapon economy, pose, and deployable state never live in the candidate
recipe. Enforce a code-owned maximum serialized candidate and resolved snapshot size (candidate
target at most 128 bytes; resolved target at most 4 KiB) and fail closed if exceeded.

Retire the M05 `SelectedWeapon`/`SelectingWeapon` selection contract after migrating compatibility
tests. Preserve public re-exports only where integration tests or current sibling modules need them;
do not retain two simultaneous selection authorities.

### Catalog validation and deterministic resolution

`BuildContentCatalog::validate` must enforce:

- supported schema/revision and code-owned collection/serialized-size ceilings;
- exactly two known ultimate definitions, six known passive definitions, and four ascending named
  preset IDs/keys/presentation references;
- finite positive numeric definition values and bounded tick/damage/radius/range/health/speed fields;
- stable unique IDs/keys, valid display metadata, cost bounds, and known cross-catalog references;
- valid named preset recipes under the same public resolver used by non-presets;
- no authored policy can widen engine ceilings.

`resolve_build_recipe` performs, in order:

1. structural shape/ID/slot/duplicate/family validation;
2. preset or custom Pulse weapon configuration resolution through M05 policy and fighter bounds;
3. passive-derived fighter-stat resolution with final safe health/speed checks;
4. ultimate/passive compatibility and total point calculation using checked arithmetic;
5. canonicalization of ID arrays and signed zero where applicable;
6. fingerprinting of normalized selected choices plus catalog schema/balance revision;
7. bounded immutable `ResolvedMatchLoadout` creation.

Sorting passives for canonical fingerprinting must not change UI slot order unless slot order gains
meaning. Equivalent recipes fingerprint identically. Preset identity is attribution metadata and is
not part of runtime behavior selection.

### Selection transaction and match lock

Replace `WeaponSelectionRequest` with:

```text
BuildSelectionRequest
  request_id
  match_id
  selection: Preset(BuildPresetId) | Custom(BrawlerBuildRecipe)

BuildSelectionOutcome
  request_id
  match_id
  decision
  accepted_identity: Option<SelectedBuild>
  accepted_total_points: Option<u8>

Decision
  Accepted | Stale | WrongMatch | WrongPhase | ReadyLocked |
  UnknownId | InvalidSlots | InvalidCombination | OverBudget |
  ResolutionFailed | CandidateTooLarge
```

Requests remain link-scoped and target no entity/player. Repeat request IDs return the prior outcome;
older IDs are stale. The server resolves from its catalogs, atomically replaces selected/resolved
components and base runtime, destroys an existing waiting sentry if any, clears buffered/current
input and freshness, and installs a post-selection input epoch. Rejection changes nothing.

M07 readiness requires a resolved loadout. Setting ready locks replacement. If readiness is cancelled
during countdown and M07 returns to waiting, replacement becomes legal after `ready=false`. Active or
completed requests are rejected. Restart retains the selected recipe/identity, rebuilds fresh runtime
from it, clears charge/passive state, and permits replacement before ready.

### Resolved fighter stats and lifecycle

Replace the global one-speed runtime assumption with per-fighter `ResolvedFighterStats`. Ordinary
movement reads resolved speed, then applies directed-movement slow and Adrenal multipliers in a
documented order. Dash/external motion remain separate and are not multiplied by these passives.

Initial activation and every respawn restore maximum health, weapon economy, ability/passive runtime,
pose, collision, and input epoch from the immutable resolved loadout. Ultimate charge is the one
exception: it survives ordinary defeat/respawn. Match restart and waiting loadout replacement reset
charge to zero. With neither frame-family passive, the resolved base is the existing 100 maximum
health and 320-units/second movement speed. `CurrentHealth` remains mutable health; HUD maximum comes
from resolved stats.

### Generic combat source and damageable targets

Extend combat source identity without putting process-local entities on the wire:

```text
CombatSourceIdentity
  PrimaryWeapon { player, network_id, team, preset_id?, recipe_fingerprint }
  Ultimate { player, network_id, team, ultimate_id, activation_id }
  Deployable { player, network_id, team, ultimate_id, deployable_id }
  Environment { definition_id? }

CombatTargetIdentity
  Fighter { network_id, team }
  Deployable { deployable_id, owner_network_id, team }
```

Primary attack tracking may retain its focused weapon aggregate, but pending payloads, applied
damage facts, defeat/destruction cues, and telemetry carry the generic identities. Fighter defeat
continues to emit exactly one `Defeat` fact per life. Sentry destruction emits a distinct
`DeployableDestroyed` outcome and never reaches Wipeout fighter-score logic.

Projectile, melee, and area candidate collection can include the dedicated deployable collider
layer. Filter owner/allies and sort mixed candidates by a stable target key before applying existing
maximum-target limits. Effects unsupported by deployables (slow and knockback in M08) are ignored by
explicit target policy, not by query accident.

### Outcome fan-out and passive ordering

Within the authoritative fixed-post outcome transaction:

```text
combat damage writes sorted facts
  -> validate/deduplicate fact batch
  -> apply ultimate charge and passive triggers
  -> Wipeout scoring/completion/respawn decision
  -> build/ability/match telemetry observation
  -> clear current-tick facts
  -> combat lifecycle/cues/finalize/tick advance
```

Close Quarters and Tenacity modify pending damage/effect values before outcome facts are authored.
Adrenal, Quick Cycle, and charge observe applied outcome facts. Quick Cycle reacts only to a credited
hostile fighter defeat attributed to that owner; sentry/dash defeat may score but does not prime it,
keeping the passive tied to primary-weapon decisions. If an owner is defeated in the same batch,
lifecycle cleanup clears a newly primed Quick Cycle state after all facts are evaluated.

### Ultimate activation and charge

Use the existing `FighterInput::ULTIMATE` bit and server-side rising-edge state. Accept at most one
activation per press and only when the participant is active, alive, post-selection input-fresh,
fully charged, not already executing/deployed, and the selected ultimate can start legally.

For sentry, validate placement before spending charge. For dash, a zero-length terrain-safe segment
is rejected without spending charge. An accepted activation atomically consumes all 1,000 charge,
breaks spawn protection as an attack, allocates stable activation/deployable/combat IDs, changes
durable state, emits one cue, and blocks primary acceptance that tick. Duplicate/buffered/stale
input cannot activate twice.

Charge calculations use checked/saturating integer arithmetic against applied damage facts sorted by
event ID. No wall-clock timer, client estimate, HUD meter, or weapon telemetry total can grant charge.
Initial balance evidence targets median first full charge between 2,700 and 5,400 active ticks
(45–90 seconds) and roughly one to three accepted ultimate uses per participant in a normal
10,800-tick match. These are soft playtest gates for the 5/3 tuning ratio, not authority rules.

### Dash execution

At activation, commit the current authoritative facing and terrain-only cast endpoint. Store only
bounded server runtime needed for start/end tick, origin/endpoint, stable activation ID, and up to
eight already-hit stable targets. Replicate charge/phase/deadline and pose; the hit set stays server
local.

Each due movement tick advances to the deterministic segment fraction and validates the resulting
pose against playable bounds. After the Avian query tree reflects movement, sweep the travelled
capsule segment for hostile targets, sort by stable key, and enqueue the authored dash payload once
per target. Damage is resolved through the same generic effect/outcome pipeline as primary attacks.

Normal directed input and external motion do not move the fighter during dash; fresh aim may update
only after dash ends. The fighter remains damageable. Defeat, match completion/restart, disconnect,
or loadout replacement clears dash state immediately. No respawn can inherit a dash deadline.

### Sentry execution and cleanup

Placement searches backward along the owner's facing from the 96-unit preferred offset to the
56-unit minimum safe offset in deterministic 8-unit steps using the sentry's 20-unit-radius circle,
terrain layers, playable bounds, living fighter/deployable occupancy, and existing owner sentry
count. It rejects an occupied/invalid result rather than clipping or consuming charge. Spawned
sentries are server-owned, match-member entities with a dedicated non-blocking target collider.

Every six ticks, a sentry collects living active hostile fighters within 480 units, rejects terrain-
occluded targets, and orders by squared distance then `NetworkEntityId`. At its 30-tick deadline it
fires at the current best target through a focused straight-delivery definition and generic combat
source. It does not accept client input, lead targets, rotate gameplay instantly from presentation,
or target another sentry in M08.

Sentry activation breaks owner spawn protection immediately. Every later sentry shot is also an
owner-authored accepted attack and would break protection before damage resolution; with the M08
owner-defeat cleanup policy a sentry cannot normally survive into that owner's next protected life.

Hostile primary/dash/sentry payloads may damage a sentry only where the source policy explicitly
allows; friendly fire remains off. Destruction/expiry/owner defeat/disconnect/completion/restart/
replacement all converge on one idempotent cleanup helper, remove the sentry and its live deliveries
and pending work, remove collider/runtime/replication state, and emit at most one appropriate cue.
Cleanup uses stable match/owner/deployable identities and exact queries, never broad recursive
despawn.

### Replication and recovery

Register and bump protocol/content fingerprints for:

- build request/outcome and stable build/ability/passive/deployable IDs;
- `SelectedBuild`, bounded `ResolvedMatchLoadout`, `AbilityState`, and `PassiveRuntimeState`;
- sentry marker/identity/health/deadline/pose and ability presentation cues;
- generic combat source/target changes carried by replicated attack/cue shapes.

Use ordinary replication for waiting-replaceable resolved loadouts and dynamic state; stable IDs
inside a loadout remain immutable during locked match phases. Sentry spawn/despawn is ordinary
server entity replication. Late/current clients reconstruct build names, maximum health, passive
inventory, ultimate meter/phase, and live sentry health/lifetime from durable components without
replaying selection, charge, or spawn history.

Clients cannot replicate candidate recipes or resolved state back to the server. Authority tests
must mutate every registered client copy and prove the server remains unchanged.

### Client build editor, HUD, presentation, and audio

Extend the existing selection panel and HUD rather than create a second full-screen menu system:

- preset row: Runner, Bruiser, Controller, Duelist, Custom;
- custom page: Pulse power, reach, magazine, ultimate, passive slot 1, passive slot 2;
- live server-equivalent point subtotal and local explanatory preview, clearly labelled provisional
  until the server accepts;
- exact server decision/status and accepted build fingerprint/revision diagnostic in debug detail;
- persistent compact loadout icons/names, ultimate 0–100% meter, ready/executing/deployed state,
  passive trigger deadlines, sentry health/lifetime, and input prompt;
- distinct readable dash trail/contact, sentry owner/team marker, targeting/fire, damage, destruction,
  expiry, charge-ready, activation-rejected, and passive-trigger feedback;
- existing audio pool/caps with a bounded charge-ready, ultimate activation, sentry fire/destroy, and
  passive trigger mapping; missing audio remains non-blocking.

Deadline displays use `AuthoritativeTick`. Opponents may see selected build/passive/ultimate and live
sentry state; M08 contains no concealed information. Visual state never chooses targets, applies
damage, advances charge, or extends a deadline.

### Telemetry and evidence

Extend bounded match summaries with:

- build preset ID or custom build fingerprint/revision, total/spare points, weapon fingerprint,
  ultimate ID, passive IDs, team/player, result, active time, defeats/deaths, and damage;
- ultimate first/full-charge tick, charge earned by dealt/received source, ready-to-use delay,
  attempts/accepts/rejections by reason, uses, damage, targets, credited defeats, and wasted charge;
- dash distance requested/actual, terrain truncations, targets contacted, and interruptions;
- sentry placement rejection, lifetime, shots, hits, damage, destructions, owner-cleanup reason, and
  concurrent-count high-water mark;
- passive trigger count, active ticks, modified damage/effect/refill amounts, and unused triggers;
- matchup results keyed by stable build fingerprints without using telemetry to drive balance.

The first product-iteration report compares the 2,700–5,400-tick median first-full-charge target and
one-to-three-use normal-match target against raw per-participant distributions. A miss requires
recorded tuning/feedback review, not a change to charge authority or event sources.

Archive up to the existing match-summary bound and retain explicit dropped-record counters. Extend
the one process verification path and four-client harness; do not create a separate ability verifier
or process exit owner.

## Trackable implementation plan

Implement as six green vertical slices. Re-run affected role checks and the accepted M07 regression
subset after each slice.

### Prerequisite and content foundation

- [x] Reconcile this specification with M07 feedback remediation, technical-gate acceptance, learning
  review, explicit user closeout, and the supervised observations deferred to M11.
- [ ] Record the exact M08 starting commit after the accepted M07 implementation is committed.
- [ ] Re-run the complete accepted M07 format, role Clippy/tests/builds, server feature graph,
  deterministic network/performance suites, process profiles, and relevant visual baseline.
- [ ] Add build stable IDs, catalog/engine limits, candidate/resolved shapes, canonical fingerprint,
  shared content envelope contribution, and four legal named recipes with pure validation tests.

### Build resolution and selection

- [ ] Implement preset/custom Pulse resolution, discrete transformations, budget/slot/family rules,
  body-stat resolution, wire-size bounds, and exhaustive invalid/equivalent-recipe tests.
- [ ] Replace the one-time weapon transaction with link-scoped waiting build selection/replacement,
  request idempotency, ready lock, atomic runtime install, input epoch reset, and restart retention.
- [ ] Migrate server/client/harness/config/automation selection callers without retaining parallel
  weapon/build selection authorities or preset-specific runtime branches.

### Passive and outcome foundation

- [ ] Add generic combat source/target identity and deployable damage routing while preserving all
  primary weapon, score, cue, telemetry, recovery, and one-defeat-per-life behavior.
- [ ] Add resolved per-fighter stats and all six passive rules with explicit fixed-post fact fan-out,
  deterministic rounding, trigger/rearm state, lifecycle cleanup, and schedule trace tests.
- [ ] Add ultimate charge observation/economy and replicated ability/passive state; prove every
  excluded charge source and same-tick/idempotency rule.

### Dash vertical slice

- [ ] Implement server activation validation, terrain-safe cast endpoint, fixed-tick dash movement,
  primary/motion gating, bounded target sweep, generic payload/outcome, interruption, and cleanup.
- [ ] Add dash cues, HUD state, client trail/contact presentation, telemetry, pure geometry tests,
  small-App schedule tests, deterministic network authority/recovery, and process evidence.

### Sentry vertical slice

- [ ] Implement clear placement, stable IDs/ownership index, dedicated collision layer, replication,
  health/damage, stable hostile acquisition, line of sight, straight fire, and one-owner bound.
- [ ] Unify expiry/destruction/owner defeat/disconnect/completion/restart/replacement cleanup and add
  sentry cues, HUD/world presentation, audio, telemetry, recovery, and accumulation tests.

### Client editor, verification, and gate review

- [ ] Complete controller/keyboard preset/custom editor, cost/rejection feedback, loadout HUD,
  ultimate/passive/deployable presentation, aspect-ratio layouts, and headless automation controls.
- [ ] Run all automated, process, performance, feature-isolation, visual, physical-controller, audio,
  normal-match, repeated-match, and all-build comparison gates; record exact evidence.
- [ ] Enter `User playtest`, collect and classify feedback, rerun affected verification, complete the
  learn-from-errors review, and conduct the first product-iteration gate before marking M08 complete.

## Test plan

### Pure validation and rule tests

- [ ] Catalog rejects schema, count, duplicate ID/key, unknown cross-reference, invalid finite/bound,
  authored ceiling widening, illegal preset, cost overflow, and serialized-size violations.
- [ ] Candidate validation covers unknown IDs, duplicate passives, wrong slot count, frame-family
  conflict, every budget boundary, custom axis enum, incompatible choice, and all four presets.
- [ ] All 27 custom Pulse combinations resolve deterministically; legal results pass M05 structural
  validation, have no preset identity, fingerprint canonically, produce exact 45/60/81-tick
  Compact/Standard/Long lifetimes, and never exceed engine bounds.
- [ ] Passive arithmetic covers health/speed resolution, damage distance boundaries/interpolation/
  rounding, Adrenal trigger/rearm/non-refresh behavior, `Magazine` and `Charges` Quick Cycle prime/
  consume, Tenacity minimum duration, and simultaneous trigger/defeat cleanup.
- [ ] Charge covers cap/overflow, dealt/received multipliers, all excluded sources, simultaneous
  facts, use/reset, defeat retention, restart clearing, and stable event idempotency.
- [ ] Dash geometry covers clear/full/truncated/zero paths, bounds, stable target ordering, maximum
  targets, once-per-target 35 damage plus 450 world units/s knockback lasting six ticks, deadline,
  and interruption.
- [ ] Sentry rules cover the 96/88/80/72/64/56 placement sequence, 20-unit radius, obstruction,
  occupancy, acquisition distance/LOS/stable tie, cooldown/lifetime, affiliation,
  health/destruction, owner lookup, and every cleanup reason. A fixed-schedule test proves no
  acquisition before the six-tick boundary and stable reacquisition ordering across two consecutive
  acquisition windows independently of the 30-tick fire deadline.

### Small-App/ECS and schedule tests

- [ ] Explicit trace proves build replacement precedes input epoch, ultimate activation precedes
  movement/fire, dash/sentry contacts precede damage, fact observers precede Wipeout drain, lifecycle
  follows scoring, and tick advancement remains last.
- [ ] Atomic accepted replacement installs exactly one resolved loadout and fresh runtime; every
  rejected or duplicate request leaves the prior entity state byte-equivalent.
- [ ] Movement/health/respawn use resolved stats; charge survives defeat but all other transient
  ability/passive/dash/sentry state follows the specified lifecycle.
- [ ] Primary weapons behave identically without passives and apply each passive only through the
  resolved inventory; no preset/build ID switches execution.
- [ ] Dash cannot move/fire/damage outside active phase, through terrain, twice per press, after
  defeat, or beyond its target/deadline bounds.
- [ ] Sentry cannot spawn illegally, target allies/defeated/inactive/occluded fighters, exceed one
  owner instance, survive cleanup, or award a Wipeout point when destroyed. Accepted activation and
  a later attributed shot both exercise the ordinary owner spawn-protection break rule.
- [ ] Repeated build replacement and at least three match restarts retain exact catalog/map/process
  roots while leaving zero stale ability/deployable/projectile/effect/fact/input entities/state.

### Deterministic network tests

- [ ] Four clients select the four named builds, converge accepted identities/resolved loadouts,
  ready, complete/restart a match, and may legally replace builds in the next waiting phase.
- [ ] A legal custom recipe resolves/replicates/fires through ordinary systems; over-budget/invalid/
  stale/wrong-match/wrong-phase/ready-locked requests are link-scoped and cannot mutate another player.
- [ ] Clients cannot authoritatively change build identity, resolved stats/weapon, passive inventory,
  charge, ability phase, dash pose/damage, sentry owner/target/health/deadline, or cleanup.
- [ ] Delay/loss/duplication/reordering converge current loadout, ability/passive state, sentry state,
  charge, cues where guaranteed, match score/result, and restart cleanup without replay history.
- [ ] Owner defeat/disconnect and sentry destruction/expiry are distinct and identical on all peers;
  late/current replication recovers live sentry and HUD state.
- [ ] All preset/custom telemetry keys and outcome attribution use stable IDs/fingerprints and agree
  with authoritative state without relying on local `Entity` identity.

### Process, performance, visual, controller, and audio verification

- [ ] Dedicated server plus four headless clients completes shortened local/typical/adverse matches
  exercising four builds, both ultimates, every passive across profiles, one custom recipe, restart,
  clean port reuse, and zero evidence drops.
- [ ] Fixed-step p95 remains `< 16.67 ms` in a four-fighter worst case with maximum legal primary
  deliveries, four simultaneous dashes or sentries as allowed, mixed damageables, telemetry, and HUD.
- [ ] Targeting, outcome fan-out, passive work, and cleanup remain bounded by explicit live-entity/
  record ceilings and do not scan archived summaries per tick.
- [ ] Isolated server features exclude window/render/UI/text/assets/audio/device input and run without
  an asset tree; no new dependency or client feature enters the server graph.
- [ ] At 1280x720, 1440x900, 1024x768, and 960x540, editor, cost/error detail, match strip, loadout,
  meter, passive state, sentry health, scoreboard, respawn, and results remain legible.
- [ ] Physical Xbox-like controller and keyboard/mouse can select every preset, edit every custom
  field, understand invalid/over-budget feedback, ready, aim/use both ultimates, and restart without
  a pointer or conflicting match commands.
- [ ] Dash/sentry/charge/passive audio and visuals are distinguishable from primary fire/score/
  defeat, obey caps/deduplication, communicate owner/team/deadline, and degrade safely when missing.
- [ ] Controlled normal-duration Wipeout comparisons produce usable preset/custom matchup, ultimate,
  passive, and charge-time telemetry, compare median first readiness against 2,700–5,400 active ticks
  and accepted uses against one to three per participant, and demonstrate at least two visibly
  different play patterns.

### Evidence rules

- Authority assertions inspect server resolved/runtime components, generic combat facts, match state,
  and exact live entities. HUD text, cues, audio, and telemetry never prove authority.
- Candidate legality is proved by the public pure resolver and server decision; a client-side preview
  is convenience only and may not install or bless a loadout.
- Ability damage uses ordinary client input and server simulation. Tests may inject short definitions
  or ticks but may not directly mutate target health, charge, sentry state, score, or result in an
  end-to-end proof.
- Charge/passive/scoring totals are checked independently against the same immutable fact batch so a
  shared consumer bug cannot falsely validate every aggregate.
- Stable build/recipe/ability/passive/deployable/player/network/match IDs cross worlds; local Bevy
  entities never appear in reports or wire contracts.
- Visual/controller/audio checks complement automated authority, recovery, timing, and cleanup tests;
  they do not replace them.

## Playtest handoff requirements

When verification is green, provide:

- exact dedicated-server and one/two-client commands plus a four-headless-client evidence command;
- controller and keyboard build-editor/combat controls, including back/ready/scoreboard conflicts;
- one scenario for each named build, both ultimates, all six passives, and one legal custom Pulse;
- a suggested sequence that exposes dash wall/contact behavior, sentry counterplay/cleanup, charge
  pacing, passive timing windows, budget constraints, match restart, and build replacement;
- known balance hypotheses and any implementation limitations;
- requested observations: whether choices are understandable, costs force real sacrifice, builds
  feel distinct, ultimate readiness/use is clear, sentry ownership/counterplay is readable, dash is
  controllable, and normal match duration/counterplay remain healthy.

## Feedback review

Not started. Record every user item as implemented now, deferred to the v1/future backlog, rejected
with rationale, or awaiting evidence. Authority, wire, lifecycle, budget, slot, charge-source,
deployable-targetability, or active-item changes require renewed specification review.

## Learn-from-errors review

Not started. Before closeout, record mistakes, causes, prevention, and reusable lessons. Improve a
project/Codex skill only when the learning recurs beyond this milestone and is genuinely reusable.

## Risks and follow-up decisions

- **Deferred supervised observations:** M11 controller/audio/normal-duration findings may motivate M08
  presentation or tuning changes, but do not alter the accepted M07 authority contract. Triage any
  resulting change against the active milestone instead of silently revising the baseline.
- **Cross-domain resolver growth:** weapon, build, body, ability, and passive validation can collapse
  into one large module. Keep pure structural weapon validation separate from build compatibility/
  cost while exposing one intentional resolve entry point.
- **Damage abstraction regression:** adding deployable targets and ability sources touches mature M05
  combat paths. Land generic identity/routing with all old tests green before implementing a sentry.
- **Fact fan-out ordering:** charge/passives/scoring can diverge or double-consume events. A single
  sort/fan-out/clear transaction and explicit schedule trace are exit requirements.
- **Dash tunnelling/readability:** fast movement can cross walls or targets between ticks. Cast the
  endpoint, sweep each travelled segment, cap contacts, and verify at low tick counts and corners.
- **Sentry accumulation:** replicated entities, colliders, target scans, and cues can leak across
  owner/match lifecycles. One-per-owner, hard lifetime, exact cleanup tests, and high-water telemetry
  bound the risk.
- **Snowballing charge:** damage-dealt charge rewards winning while received charge rewards comeback.
  The 5/3 ratio is a test hypothesis; compare its 2,700–5,400-tick median first-readiness and
  one-to-three-use targets against raw playtest distributions before adding other sources or passive
  modifiers.
- **Controller editor density:** six fields plus budget can overwhelm the pre-match overlay. Keep
  choices discrete, preserve an immediate preset path, and test compact/tall layouts physically.
- **Preset identity leakage:** analytics/UI convenience can become class branching. Runtime tests must
  remove preset identity and still reproduce behavior from the resolved loadout.
- **M09 reuse:** Hot Zone must consume the same immutable loadout, charge, ability, passive, fighter
  lifecycle, deployable, and cleanup systems. Objective-specific passives remain out of M08 so this
  milestone does not pre-implement M09 rules.

## Exit checklist

- [ ] User validates this specification before production implementation begins.
- [x] M07 feedback/learning and combat vertical-slice technical gate are complete; the user explicitly
  accepted its supervised-observation deferral and closed the milestone.
- [ ] M08 records and revalidates the exact starting baseline.
- [ ] Catalog, candidate, resolved loadout, and runtime state are distinct, bounded, fingerprinted,
  deterministic, and server-authoritative.
- [ ] Four named presets and at least one legal non-preset custom Pulse use the same resolver/runtime
  paths; no preset-ID behavior branch exists.
- [ ] The 12-point budget, exact slots, duplicate/family rules, and server rejection create visible
  legal/illegal tradeoffs with readable controller feedback.
- [ ] Six passives, two ultimates, charge, per-fighter stats, ability damage, and deployables obey the
  specified fixed-tick authority, attribution, recovery, and lifecycle contracts.
- [ ] Dash is collision-safe/readable; sentry targeting, targetability, ownership, lifetime, and every
  cleanup path are deterministic and bounded.
- [ ] Four clients and real processes converge loadout/ability/deployable/match state under impairment,
  reject forged client authority, restart cleanly, and retain isolated headless server composition.
- [ ] Visual, physical-controller, keyboard/mouse, audio, normal-match, performance, telemetry, and
  repeated-match gates are complete with recorded evidence.
- [ ] User feedback is triaged, affected verification rerun, learning review complete, and the first
  product-iteration gate concludes that builds create recognizable combat patterns.
