# Bots

## Status

Autonomous player bots are **implemented and accepted in completed V11 M01**. V2 implemented
server-hosted Practice matches with inert `Bot N` roster fillers; V11 promoted those same stable
roster fighters into active deterministic controllers that perceive, decide, navigate, and produce
ordinary validated input.

The gameplay foundation has grown since that deferral. Wipeout, Hot Zone, and Heist now expose
authoritative mode state; maps are resolved from sparse assets; concealment and observer-specific
reveal are authoritative; and V10 supplies damageable barrels and Heist safes, with treasure chests
and restoration pickups now completed. The playable implementation deliberately supports these
current facts rather than using the V2-era nearest-target sketch.

## Decision

The first playable bots are **server-hosted practice controllers** for the existing manifest bot
fighters. A controller observes a bounded allowlist of authoritative match facts, runs a pure
deterministic policy, and produces ordinary bounded `FighterInput`. Existing authoritative systems
remain the only owners of movement, collision, attacks, damage, abilities, scores, respawns, match
outcomes, pickups, and map mutation.

Use a **project-owned utility policy plus deterministic bounded resumable path planning**. Keep the
policy, team-plan, and path-search rules as pure Rust. Bevy remains the runtime ownership,
scheduling, and adapter layer; Avian remains collision authority; Lightyear remains irrelevant to
server-hosted input acquisition. Measured synthetic maximum-topology and maximum-roster p95
evidence did not justify the reviewed sector/portal hierarchy. Do not add a behavior-tree, GOAP,
navmesh, ML, LLM, or remote-policy dependency for the first playable slice.

This is the best current technology choice for a "smart" Brawler bot because intelligence here is
mostly accurate game modeling: fair perception, objective choice, team roles, range control,
predictive aim, ability timing, and reliable routes. A generic AI framework does not supply those
game-specific facts or decisions. The pure boundary preserves the option to compare another planner
or a learned policy later without changing gameplay authority.

Server-hosted input production is not network-equivalent to a human client. Practice bot input does
not traverse Lightyear transport and therefore does not exercise connection ownership, sequence,
freshness, rate, or hostile-packet validation. It enters the same authoritative gameplay consumption
path after input acquisition and must pass shared finite, range, and button validation before being
installed in `ActionState<FighterInput>`. The controller also advances the fighter's local
`InputFreshness` marker for that simulation tick so existing movement, attack, ability, and
post-selection gates consume the validated state. This marker is controller-produced freshness,
not evidence of a network packet.

This decision follows from the existing product flow:

- Start Practice already asks the lobby to allocate one match worker containing the human and
  manifest bots in the remaining roster positions.
- The worker already materializes each manifest bot as an ordinary fighter with a resolved
  saved-brawler snapshot, lifecycle state, and neutral `ActionState<FighterInput>`.
- Making those entities produce intent requires no bot admission protocol, route capability,
  subprocess orchestration, check-in exception, or second combat simulation.

The policy boundary remains independent of Bevy, Lightyear, and server types. A later external
headless client may construct the same canonical observations, call the same policy, and install
the result through `PendingLocalActions`. That is a separate hosting adapter and validation
milestone, not a requirement for playable practice bots.

Alternatives remain:

1. **Server-hosted practice controller (chosen first)** — fits the existing practice roster and has
   the smallest lifecycle and process surface. Its explicit observation adapter prevents the policy
   from exploiting arbitrary authoritative state.
2. **External client host (deferred)** — useful for network/load evidence, third-party policy work,
   or future multiplayer bot filling. Seamless Start Practice would require supervisor-managed bot
   processes or another explicit admission/orchestration design.
3. **Learned policy (deferred)** — there is no representative trace/replay dataset, gameplay is
   still evolving, and deterministic diagnosis matters more than opaque peak performance. A learned
   candidate may later compete behind the same observation/decision boundary.

## Architecture readiness

No preparatory architecture refactor is required before the bot milestone. The current production
shape already provides the required integration points:

- the practice allocation and manifest own stable bot identity, team, saved-brawler snapshot, and
  roster capacity;
- worker startup materializes manifest bots as full fighters with `ActionState<FighterInput>` and
  `InputFreshness`;
- the fixed schedule already orders `GameplaySet::Lifecycle`, the deferred-command boundary,
  `GameplaySet::Input`, `GameplaySet::Simulation`, and `GameplaySet::Fire`;
- `ResolvedMap`, all three mode states, fighter lifecycle markers, resolved loadouts, poses, health,
  ability state, concealment/reveal state, and public object state provide the facts needed by a
  bounded observation adapter; and
- existing movement, combat, ability, mode, terrain, replication, and presentation systems already
  consume or present the resulting authoritative outcomes.

The bot milestone owns these integration seams as part of the feature:

1. Replace `InertPracticeBotPlugin` with a focused practice-bot composition that installs private
   controller state only on manifest bot fighters.
2. Add the private pure observation, team-plan, policy, navigation, capability, profile, and entropy
   modules described below. Do not make them public or compile them into the client before another
   execution host exists.
3. Expose the existing complete decoded-input validity rule through one narrow `pub(crate)` helper
   so the controller reuses finite, radial-range, and button checks instead of duplicating them.
4. Commit validated `ActionState<FighterInput>` and controller-produced `InputFreshness` together in
   the current fixed input phase.
5. Track a private monotonic controller life generation derived from authoritative transitions among
   `Defeated`, `RespawnState`, and `ActiveCombatant`; do not add a shared or replicated lifecycle
   component solely for bots.
6. Replace the current four-preset rotation with one explicit, validated, saved-brawler-native
   canonical recipe. The first capability executor targets a Pulse weapon and Dash ultimate; it
   chooses behavior from the resolved snapshot, never from a legacy preset ID.
7. Provide the visibility adapter a bot observer path over the same pure observer-specific reveal
   rule used for clients. Do not create fake connections or infer visibility from the existing
   connection-keyed cache.

This work does not require changes to routing or supervisor protocols, match-manifest wire shapes,
global compatibility, lobby admission/check-in, protocol registration, authoritative gameplay
ownership, or the dedicated-server feature boundary. If implementation evidence later contradicts
that assessment, the milestone must return to specification review before changing one of those
contracts.

## First playable slice

- **Existing Start Practice flow.** The player starts practice normally; no bot CLI, helper process,
  lobby pool, matchmaking, or additional check-in is introduced.
- **One code-owned behavior profile.** There are no player-facing difficulty presets or skill
  ranking. The target is readable opposition with delayed reactions, imperfect aim, coordinated
  objective play, and bounded tactical commitments.
- **One canonical saved brawler.** Manifest bots use one validated built-in saved-brawler recipe
  with Pulse primary and Dash. The human may practice with any legal build. Additional bot weapon
  and ability capabilities are evidence-gated.
- **All current mode goals.** Wipeout supplies survival and pressure goals; Hot Zone supplies
  contest and defend goals; Heist supplies attack-safe, defend-safe, and lane-pressure goals.
- **Fair current-feature awareness.** Bots respect all concealment sources and reveal rules. They
  reason about public barrels, safes, chests, and restoration pickups under the completed V10
  contracts.
- **Scalable map-aware navigation.** Navigation is derived from resolved authoritative collision
  geometry and measured topology. Its contract is independent of today's grid dimension limits and
  map encoding.
- **Seeded policy traces.** Given the same profile, seed, match/life identity, canonical observation
  sequence, navigation revision, and work budgets, the policy produces the same decisions.

The first playtest should validate a useful vertical slice before expanding to every build,
delivery type, active item, ultimate, projectile-avoidance behavior, or advanced squad tactic.

## Runtime shape

```text
authoritative practice-match World
  -> server-owned visibility/object allowlist
  -> canonical per-tick observations
  -> bounded reaction delay + contact memory
  -> deterministic team goal/role plan
  -> pure per-bot utility policy
  -> tactical goal and capability intent
  -> revisioned navigation query / committed route
  -> capability executor and BotDecision
  -> shared FighterInput validation
  -> existing ActionState<FighterInput> + local InputFreshness
  -> authoritative movement / combat / abilities / modes
```

Lobby practice allocation, manifest validation, roster construction, resolved builds, match
lifecycle, scoring, replication, and presentation remain existing behavior. Bot work must not add
an alternate movement, combat, ability, object-damage, pickup, score, or respawn path.

## Perception contract

The policy must not query Bevy ECS state. A server-owned adapter constructs an **owned, bounded,
canonical** `BotObservation` from an allowlist of policy-eligible facts:

- match ID, authoritative simulation tick, mode, phase, and bounded objective state;
- the controlled fighter's stable ID, team, pose, health, public resolved build capabilities,
  weapon/ability state, defeat/respawn state, and protection state;
- policy-visible fighters with stable ID, team, pose, public combat state, and observation tick;
- policy-visible deployables and, only when a supported behavior needs them, bounded projectile
  facts with stable identity, public owner/trajectory data, pose, and observation tick;
- public Heist-safe, damageable-object, and chest/pickup facts needed by a supported goal;
- the shared static navigation revision plus delayed dynamic-blocker facts; and
- explicit completeness, observation-age, and controller-life-generation indicators.

Collections are sorted by stable network, projectile, deployable, placement, objective, pickup, or
definition IDs before policy evaluation. Process-local Bevy `Entity` values, raw ECS queries,
private diagnostics, unbounded message history, mutable authoritative resources, and presentation
state never cross the pure policy boundary.

“Policy-visible” means permitted by the same authoritative observer-specific concealment and reveal
rule that governs a player observer; it never means Bevy rendering `Visibility`. The existing
visibility cache is connection-keyed, while a server-hosted bot has no connection. The bot adapter
therefore invokes the owning pure visibility rule for the bot's fighter/team identity rather than
creating a fake connection or reading unfiltered server state. Public roster knowledge must not
silently become live spatial knowledge.

Because the server has exact current state, fairness comes from an explicit bounded observation
history. At simulation tick `T`, tactical evaluation uses the canonical observation captured at
`T - reaction_delay_ticks`. Target acquisition, recognition of target movement, dynamic-object
changes, aim correction, and tactic changes all use that delayed observation. Aim error is sampled
and held for a bounded commitment interval; it is not independently resampled every tick into
visible jitter.

Lost opponents become bounded contact memories containing stable identity, last permitted pose,
source observation tick, and confidence/expiry state. Hidden movement never refreshes a contact.
The bot may investigate a stale location or switch goals, but cannot aim at the opponent's current
concealed position. Teammate sharing, if enabled in the first milestone, may share only these same
permitted delayed contacts and must preserve their source tick.

Static map structure is public match knowledge and is not copied into every delayed observation.
Dynamic blockers and interactable state enter planning through delayed policy-visible facts. Map
generation changes are lifecycle events and invalidate all navigation state immediately.

## Tick and schedule contract

Practice bot work uses the authoritative fixed `SimulationTick` and several explicit cadences:

1. Match lifecycle systems apply defeat, respawn, reset, and match-generation transitions.
2. After the existing deferred-command boundary, construct and append at most one canonical
   observation per living manifest bot for the current simulation tick.
3. In `GameplaySet::Input`, advance committed route following and input production at most once per
   bot per tick. Re-evaluate tactics at a validated bounded interval or on a material interrupt such
   as target loss, objective transition, route failure, or ability completion. Recompute the shared
   team plan at a slower bounded interval or material mode change.
4. Submit route work only when the goal changes, the route/nav revision becomes invalid, or bounded
   stuck detection fires. Process route requests in stable order under the declared per-tick work
   budget; unfinished work resumes deterministically.
5. Validate emitted axes, aim distance, and button mask, then atomically install the result in the
   bot fighter's existing `ActionState<FighterInput>` and set
   `InputFreshness.last_fresh_tick` to the current simulation tick before
   `GameplaySet::Simulation` and `GameplaySet::Fire` consume it. Do not fabricate a Lightyear input
   buffer, remote tick, sequence, or transport event.
6. Emit neutral input while observation history is incomplete, the match is not active, or the
   fighter is defeated or waiting to respawn. Do not consume entropy, advance commitments, or
   duplicate planner work twice for the same tick.

All cadences and work ceilings are tick-based, synchronous, bounded, and deterministic. No policy
result may depend on wall-clock completion, task scheduling, or thread race. Keep ordering visible
at the server composition point and cover it with schedule-trace tests. The implementing milestone
must verify exact owning systems and deferred boundaries before choosing final system labels.

A later external-client adapter requires its own clock contract. It must name a specific Lightyear
interpolation/replication timeline, attach source tick and age to non-atomic replicated facts, and
must not describe a client World as one coherent authoritative snapshot without proving it.

## Navigation contract

The map-navigation contract must not treat the server's configured `MapDimensionLimits` as a bot
design invariant or difficulty guarantee. The checked-in server currently admits `20..=512` cells
per axis within the shared hard 512-cell engine ceiling, but operators may narrow that envelope.
Navigation derives and bounds work from the resolved topology; policy decisions must not assume
today's configured minimum, maximum, or built-in-map dimensions.

At map installation, a server-owned builder derives an immutable `BotNavigationSnapshot` from the
resolved authoritative playable bounds and collision geometry. The snapshot has a map-generation
identity, a navigation revision, stable cell/edge identities, fighter-clearance rules, and
validated measured capacities. The current sparse-grid recipe lowers into this private
representation, but render meshes and the 3D scene are not policy inputs.

The first planner uses deterministic bounded resumable routing:

- clamp goals to the resolved playable region and account for fighter clearance;
- distinguish line of travel from line of fire;
- steer directly when travel is clear;
- otherwise resume exact bounded stable shortest-path search across fixed ticks, retaining its open
  set, costs, parents, delayed-blocker snapshot, and expansion count;
- apply stable total tie-breaking to equal-cost nodes and routes; current-grid lowering must reject
  diagonal corner cutting;
- simplify the committed path with authoritative travel-clearance checks, then follow it with
  desired-range, retreat, strafe, arrival, and bounded local separation steering;
- treat entry into the outer playable perimeter as a latched recovery state: route toward a safer
  interior inset and do not let the next combat/retreat decision reverse that recovery until the
  bot reaches the release inset;
- replan on material goal change, path invalidation, or navigation revision; lack of progress over
  a bounded tick window enters a short deterministic clearance-valid escape before replanning, so
  the controller does not repeatedly submit the same blocked corner route; and
- combine shared static navigation with delayed policy-visible dynamic blockers. A newly destroyed
  barrel does not become traversable knowledge before its delayed observation, while an unexpected
  authoritative collision may trigger ordinary stuck recovery.

The standard fighter radius is 14 world units. Practice navigation adds one unit of conservative
safety, so its effective 15-unit clearance still traverses a 32-unit one-cell passage with one
unit remaining on each side. The allowance must not independently grow until it rejects geometry
that an ordinary fighter is explicitly intended to use.

Planning remains bounded without assuming a particular map size. Resolution validates declared
navigation capacities; runtime owns bounded route-request queues, expansions per tick, total work
per request, stored search state, route length, and route cache entries. Requests are processed by
stable bot/goal identity. Budget exhaustion fails safely: continue a still-valid committed route,
choose a bounded local fallback, or emit neutral movement. It must not panic, hitch the worker, or
silently search an entire future large map in one tick.

Topology-changing authoritative map state increments or replaces the navigation revision and
invalidates affected routes. Dynamic object overlays that are only state changes do not require
rebuilding immutable topology. A hierarchy or chunked rebuild belongs in a later reviewed design
only when an actual map or timing measurement demonstrates the need.

This is planning data only. Avian and authoritative movement remain the owners of collision
outcomes. Do not add Recast or another navmesh dependency while Brawler remains planar and the
resolved collision geometry can produce the required graph directly. Do not create a universal
navigation framework: implement the one snapshot/search boundary consumed by practice bots.

## Policy and coordination design

The first policy is **hand-written utility scoring with small committed-duration state**.
It has no Bevy ECS dependency and requires no AI framework.

- **Team planning assigns goals, roles, and reservations.** One pure batch transition considers all
  bot teammates in stable identity order and assigns bounded roles such as pressure, contest,
  attack-safe, defend-safe, or recover-pickup. It prevents every bot from selecting the same target
  or lane while preserving legal independent decisions when only one bot is present.
- **Per-bot utility chooses a tactic.** Initial tactics include `approach`, `hold_range`, `retreat`,
  `reposition`, `contest`, `defend`, `attack_objective`, and `recover_pickup`, scored from validated
  curves over health, distance, travel/fire visibility, objective pressure, observation age,
  capability readiness, and the assigned role. Retreat is bounded range control: it increases
  separation only until the weapon-derived preferred range rather than continually selecting a
  far goal at the playable boundary.
- **Committed state prevents dithering.** Target lock, role/goal reservation, strafe direction,
  retreat burst, aim-error hold, route identity, stuck window, and reaction deadline live in
  explicit returned state.
- **Combat uses resolved capabilities.** The first executor supports Pulse primary fire and Dash.
  It derives range, projectile speed, reload, charge, and activation facts from the resolved saved-
  brawler snapshot. Predictive aim uses permitted delayed target velocity/positions, retains bounded
  error, and checks line of fire. Dash has explicit engage, escape, and traversal safety rules.
  Demolition Strike adds one focused executor branch: when charged, a bot may aim it at an observed
  public target within the resolved maximum range and emits the same ordinary targeted-ultimate
  input as a player. It receives no terrain mutation shortcut or hidden map knowledge.
- **Mode goals are focused adapters.** Wipeout, Hot Zone, and Heist produce bounded goal candidates
  for the common planner rather than scattering mode branches through navigation and combat. An
  objective role retains its zone/safe anchor while opportunistically aiming at visible enemies;
  merely seeing an opponent cannot silently replace the assigned objective.
- **World interactions are typed capabilities.** Barrels, safes, chests, and pickups expose
  only the actions their authoritative contracts permit. A bot may intentionally use or avoid a
  barrel, damage a hostile safe, open a chest, or recover a useful pickup; attackable colliders use
  a weapon-derived stand-off point rather than their blocked center. It cannot invent generic object
  interaction semantics.

The boundaries are deterministic state transitions rather than functions that mutate hidden RNG or
ECS state:

```rust
fn plan_team(
    observation: &BotTeamObservation,
    previous: BotTeamPlan,
    entropy: BotTeamEntropy,
    profile: &BotProfile,
) -> BotTeamPlan

fn choose_tactic(
    observation: &BotObservation,
    team_plan: &BotTeamPlan,
    state: BotState,
    entropy: BotEntropy,
    profile: &BotProfile,
) -> BotIntent
```

`BotIntent` names a tactical goal, target, desired range, and requested capability. Navigation turns
the goal into a committed route; the capability executor combines intent, route progress, and
resolved capabilities into `BotDecision`. `BotDecision` contains the next explicit state, ordinary
`FighterInput`, selected stable identities, chosen tactic, and one bounded diagnostic reason.
Diagnostics support tests and tuning; they do not become bot-specific gameplay protocol messages.

## Profile and deterministic entropy

Ship one code-owned, validated `BotProfile` containing only values owned by the first behavior:
reaction delay and contact-memory lifetime; tactical and team-plan cadence; desired range and
retreat threshold; aim/intercept error and hold duration; tactic commitment; navigation clearance,
replan, and stuck thresholds; and bounded capability rules. Add profile fields only with behavior
that consumes them. This is typed code configuration, not player-facing difficulty or authored
gameplay content.

Planner memory and work capacities are code-owned safety limits validated against the selected
map's measured navigation topology. They are not difficulty knobs and must not be disguised map-
dimension limits.

Use a project-owned pinned deterministic sampler with an algorithm/version tag. Derive independent
entropy samples for team assignment, target tie-breaking, tactics, aim error, and timing from:

```text
explicit bot seed + match ID + controller life generation + simulation tick + stream ID
```

Generate a fixed entropy sample set before conditional policy evaluation. Adding an aim sample must
not perturb navigation or tactic choices. Aim samples affect behavior only when the committed aim-
error interval starts. Derive the default seed from the manifest bot's stable player identity; logs
and test artifacts report the effective seed, algorithm version, profile version, match ID,
controller life generation, navigation revision, and declared work budgets. These versions belong
to internal trace/replay artifacts, not separate application-protocol compatibility fields.

## Lifecycle and practice integration

The existing Start Practice transaction remains the entry point. Manifest bot rows continue to own
stable display name, player ID, team, selected saved-brawler snapshot, and spawn assignment. Worker
materialization additionally installs private controller and policy-state components for those
rows; connected human participants never receive them.

Use a lifecycle key consisting of match ID and a private monotonic controller life generation
derived after authoritative lifecycle transitions are visible. On defeat or respawn waiting,
immediately install neutral input. Increment the generation and construct fresh observation history
and `BotState` once on the transition back to an `ActiveCombatant`; do not reset independently on
both defeat and respawn. A new life clears target/contact, tactic, aim, route, and capability
commitments. It need not rebuild the immutable map-navigation snapshot.

Changing match ID or map generation clears all match/team/controller state and derives new entropy
streams. A changed navigation revision invalidates affected route/search state without pretending a
new fighter life occurred. Match completion and restart retain existing authoritative outcomes. A
missing or rejected bot decision fails closed to neutral input and bounded diagnostics; it must not
panic or stall the match worker.

External client launch flags, bot network admission, bot check-in, supervisor sidecars, and bot
process shutdown are not part of this lifecycle.

## Suggested ownership

Keep pure behavior separate from the server adapter without creating a generic AI framework:

```text
bots/
  mod.rs          private composition and narrow model exports
  model.rs        canonical observations, plans, state, intent, and decision facts
  policy.rs       pure utility scoring and per-bot transition
  team.rs         pure bounded role/goal assignment
  navigation.rs   revisioned topology, deterministic search, route following
  capability.rs   resolved Pulse/Dash intent-to-input rules
  profile.rs      validated first-profile values and version
  entropy.rs      pinned deterministic independent samples

server/practice/
  mod.rs          manifest bot materialization and practice composition
  controller.rs   ECS allowlist, lifecycle, schedule, and input installation
```

The exact file split remains subject to implementation evidence; cohesive navigation internals may
use focused submodules if graph construction, search, and route following have distinct ownership.
The required boundary is that pure policy/navigation code cannot query or mutate a Bevy `World`,
while the server adapter cannot become a second gameplay implementation. Until a second execution
host exists, compile the bot module only where the practice server uses it rather than broadening
client or public APIs speculatively.

## First-slice verification

A first playable bot milestone must include:

- pure team-plan, tactic, capability, and committed-state tests over canonical synthetic facts;
- deterministic ordering/tie tests with ECS insertion, collection, teammate, and navigation-node
  orders permuted;
- reaction-history tests proving target movement, dynamic-object changes, aim correction, contact
  memory, and tactic changes use the configured delayed tick;
- concealment tests for Self Cloak, Concealment Field, map concealment, reveal proximity, and Reveal
  Scan, proving hidden current poses cannot enter observation or contact updates;
- entropy-stream tests proving one stream cannot perturb another and aim error remains held for its
  commitment interval;
- invariants proving all emitted axes, distances, and buttons are finite and bounded;
- neutral-input tests for incomplete history, dead, respawning, non-active, invalid-output, planner-
  budget exhaustion, and lifecycle-reset cases;
- no-target tests proving target-dependent attack/ability buttons remain neutral while valid
  objective or committed movement may continue;
- fixed-schedule traces proving lifecycle/deferred changes precede observation and one input install,
  while authoritative movement/fire follow it;
- navigation derivation tests proving results depend on resolved geometry/topology rather than
  built-in-map dimensions, including the configured 512×512 engine ceiling;
- navigation behavior tests for direct travel, multi-turn paths, stable equal-cost choices, no
  diagonal corner cutting, route smoothing, stuck recovery, and navigation-revision invalidation;
- budget tests for stable queued/incremental path work, safe exhaustion, bounded retained search
  state, and no fixed-tick spike at the maximum declared supported topology;
- focused Wipeout, Hot Zone, and Heist goal/role/reservation tests, including safe attack/defense;
- representative saved-brawler tests for predictive but imperfect Pulse aim, range control, primary
  fire, and bounded Dash engage/escape/traversal behavior;
- barrel reasoning tests plus chest/pickup usefulness, contest, collection, expiry,
  and full-health rejection behavior through ordinary authoritative input;
- worker-level practice tests proving Start Practice creates one human plus active manifest bots,
  bots affect the match only through validated `FighterInput` plus its local freshness marker, and
  defeat/respawn/restart remain authoritative;
- performance evidence at the maximum supported practice roster and declared navigation capacity,
  reporting observation, team-plan, tactic, navigation, and total fixed-tick budgets separately;
- repeated restart evidence for bounded observation/contact history, plans, routes, search state,
  diagnostics, and entity cleanup; and
- a human playtest evaluating readability, aim imperfection, ability telegraphing, team/objective
  behavior, concealment fairness, navigation on accepted maps, and whether practice is useful and
  fun.

Record the effective seed and canonical observation/plan/route/decision trace when a policy failure
needs deterministic replay. Do not treat wall-clock process or network traces as policy golden
tests, and do not raise the production map-dimension limit merely to test navigation scalability.

## Later capability gates

Add later behavior only when its owning slice is implemented and tested:

- additional straight, lobbed, and melee bot-controlled saved-brawler recipes;
- active items, Sentry, Self Cloak, Concealment Field, and other ability primitives;
- bounded projectile awareness and avoidance without perfect tracking;
- richer squad tactics or map control after the first role/reservation planner is readable;
- an alternative graph builder or navmesh only if a real future map representation cannot lower
  cleanly into the existing navigation contract;
- a learned-policy evaluation only after a versioned representative trace corpus exists; it must
  use the same permitted observation and decision contracts and beat the deterministic baseline on
  declared quality, performance, reproducibility, and operability gates;
- an external headless-client adapter with explicit replication/interpolation clock semantics;
- real subprocess/UDP and impairment tests for that external adapter; and
- supervisor-managed bot processes only if automatic multiplayer or practice hosting requires them.

## Deferred and non-goals

- Expansion beyond the validated V11 M01 specification without returning to specification review.
- Lobby fill detection, matchmaking, in-match backfill, or automatic multiplayer bot substitution.
- External bot subprocess orchestration in the first playable practice slice.
- Player-facing difficulty presets, skill ranking, adaptive difficulty, or automatic balance tuning.
- Bot-specific movement, combat, damage, score, respawn, pickup, or terrain authority.
- Bot-only gameplay protocol messages or per-message compatibility versions.
- Learned-policy training, replay datasets, inference runtimes, LLM calls, or remote policy services.
- A generic public bot or navigation SDK before another consumer demonstrates the boundary.
- Behavior-tree, GOAP, navmesh, or utility-AI dependencies while the focused project-owned policy
  and planner remain maintainable.
- A permanent map width, height, area, or cell-count ceiling in bot policy code.

Anything outside the official server or a future official client adapter that later speaks the
network protocol must implement Brawler's global compatibility/content handshake, session
transactions, native input buffering, replication rules, and admission contracts. Protocol
compatibility alone does not make an implementation a supported bot host.
