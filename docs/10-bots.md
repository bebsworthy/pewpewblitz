# Bots

## Status

Autonomous player bots remain a **future-version candidate**. V2 implemented server-hosted practice
matches with inert `Bot N` roster fillers. Those deterministic fixtures already own stable roster,
team, build, spawn, match, and lifecycle state, but they do not yet perceive, decide, navigate, or
produce input. V3 and V4 did not add bot behavior. A future milestone must explicitly validate and
authorize playable practice bots.

## Decision

The first playable bots are **server-hosted practice controllers** for the existing manifest bot
fighters. A controller observes a bounded allowlist of authoritative match facts, runs a pure
deterministic policy, and produces ordinary bounded `FighterInput`. Existing authoritative systems
remain the only owners of movement, collision, attacks, damage, abilities, scores, respawns, match
outcomes, and terrain mutation.

Server-hosted input production is not network-equivalent to a human client. Practice bot input does
not traverse Lightyear transport and therefore does not exercise connection ownership, sequence,
freshness, rate, or hostile-packet validation. It enters the same authoritative gameplay consumption
path after input acquisition and must pass shared finite, range, and button validation before being
installed in `ActionState<FighterInput>`. The controller also advances the fighter's local
`InputFreshness` marker for that simulation tick so existing movement, attack, ability, and
post-selection gates consume the validated state. This marker is controller-produced freshness,
not evidence of a network packet. Tests and documentation must preserve this distinction rather
than claiming that practice bots are ordinary network clients.

This decision follows from the existing product flow:

- Start Practice already asks the lobby to allocate one match worker containing the human and
  manifest bots in the remaining roster positions.
- The worker already materializes each manifest bot as an ordinary fighter with resolved build and
  lifecycle state plus neutral `ActionState<FighterInput>`.
- Making those entities produce intent requires no bot admission protocol, route capability,
  subprocess orchestration, check-in exception, or second combat simulation.

The policy boundary remains independent of Bevy, Lightyear, and server types. A later external
headless client may construct the same `BotObservation`, call the same policy, and install the
result through `PendingLocalActions`. That is a separate hosting adapter and validation milestone,
not a requirement for playable practice bots.

Alternatives remain:

1. **Server-hosted practice controller (chosen first)** — fits the existing practice roster and has
   the smallest lifecycle and process surface. Its explicit observation adapter must prevent the
   policy from exploiting arbitrary authoritative state.
2. **External client host (deferred)** — useful for network/load evidence, third-party policy work,
   or future multiplayer bot filling. Seamless Start Practice would require supervisor-managed bot
   processes or another explicit admission/orchestration design.
3. **Learned policy (deferred)** — there is no replay dataset, gameplay is still evolving, and the
   product values readable combat over optimal play.

## Architecture readiness

No preparatory architecture refactor is required before the bot milestone. The current production
shape already provides the required integration points:

- the practice allocation and manifest own stable bot identity, team, build snapshot, and roster
  capacity;
- worker startup materializes manifest bots as full fighters with `ActionState<FighterInput>` and
  `InputFreshness`;
- the fixed schedule already orders `GameplaySet::Lifecycle`, the deferred-command boundary,
  `GameplaySet::Input`, `GameplaySet::Simulation`, and `GameplaySet::Fire`;
- `ResolvedMap`, `MatchState`, Wipeout/Hot Zone state, fighter lifecycle markers, resolved loadouts,
  poses, health, and combat state provide the facts needed by a bounded observation adapter; and
- existing movement, combat, ability, mode, terrain, replication, and presentation systems already
  consume or present the resulting authoritative outcomes.

The bot milestone owns these small integration seams as part of the feature, not as a separate
architecture phase:

1. Replace `InertPracticeBotPlugin` with a focused practice-bot composition that installs private
   controller state only on manifest bot fighters.
2. Add the private pure policy/model/navigation module described below; do not make it a public API
   or compile it into the client before a client execution host exists.
3. Expose the existing complete decoded-input validity rule through one narrow `pub(crate)` helper
   so the controller reuses finite, radial-range, and button checks instead of duplicating them.
4. Commit validated `ActionState<FighterInput>` and controller-produced `InputFreshness` together in
   the current fixed input phase.
5. Track a private monotonic controller life generation derived from authoritative transitions among
   `Defeated`, `RespawnState`, and `ActiveCombatant`; do not add a shared or replicated lifecycle
   component solely for bots.
6. Change practice manifest bot build assignment from the current rotating presets to the one
   representative preset selected by the milestone.

This work does not require changes to routing or supervisor protocols, match-manifest wire shapes,
global compatibility, lobby admission/check-in, protocol registration, authoritative gameplay
ownership, or the dedicated-server feature boundary. If implementation evidence later contradicts
that assessment, the milestone must return to specification review before changing one of those
contracts.

## First playable slice

- **Existing Start Practice flow.** The player starts practice normally; no bot CLI, helper process,
  lobby pool, matchmaking, or additional check-in is introduced.
- **One code-owned behavior profile.** There are no player-facing difficulty presets or skill
  ranking. The target is readable opposition with delayed reactions, imperfect aim, and bounded
  tactical commitments.
- **One representative bot build.** Manifest bots use one validated built-in build chosen by the
  milestone. The human may practice with any legal build. Support for additional bot-controlled
  delivery and ability capabilities is added from evidence rather than promised up front.
- **Both current mode goals.** Wipeout supplies survival and pressure goals; Hot Zone supplies
  contest and defend goals. The first policy remains small, but a practice bot must not become an
  inert nearest-enemy shooter in an objective mode.
- **Small map-aware steering.** Use resolved map bounds and collision geometry for direct travel,
  deterministic obstacle-corner steering, desired range, and bounded stuck recovery. Do not build a
  general waypoint, portal, or navigation framework before playtest evidence requires it.
- **Seeded policy traces.** Given the same profile, seed, match/life identity, and canonical
  observation sequence, the policy produces the same decisions.

The first playtest should validate a useful vertical slice before expanding to every build,
delivery type, active item, ultimate, projectile-avoidance behavior, or advanced navigation case.

## Runtime shape

```text
authoritative practice-match World
  -> server-owned bounded observation adapter
  -> bounded reaction-delay history
  -> canonical owned BotObservation
  -> mode goal + small deterministic navigation
  -> pure utility policy + committed BotState
  -> BotDecision
  -> shared FighterInput validation
  -> existing ActionState<FighterInput> + local InputFreshness
  -> authoritative movement / combat / abilities / modes
```

Lobby practice allocation, manifest validation, roster construction, resolved builds, match
lifecycle, scoring, replication, and presentation remain existing behavior. Bot work must not add
an alternate movement, combat, ability, score, or respawn path.

## Perception contract

The policy must not query Bevy ECS state. A server-owned adapter constructs an **owned, bounded,
canonical** `BotObservation` from an allowlist of policy-eligible facts:

- match ID, authoritative simulation tick, mode, phase, and bounded objective state;
- the controlled fighter's stable ID, team, pose, health, public resolved build capabilities,
  weapon/ability state, defeat/respawn state, and protection state;
- policy-visible fighters with stable ID, team, pose, public combat state, and observation tick;
- policy-visible deployables and, only when a supported behavior needs them, bounded projectile
  facts with stable identity, public owner/trajectory data, pose, and observation tick;
- validated resolved map bounds and collision-relevant geometry;
- explicit completeness and controller-life-generation indicators.

Collections are sorted by stable network, projectile, deployable, placement, or definition IDs
before policy evaluation. Process-local Bevy `Entity` values, raw ECS queries, private diagnostics,
unbounded message history, and mutable authoritative resources never cross the pure policy
boundary.

For the first slice, “policy-visible” means present in the authoritative match and permitted by the
adapter; it never means Bevy rendering `Visibility`. When concealment or observer-specific reveal
rules exist, the adapter must apply those rules before constructing observations. Public roster
knowledge must not silently become live spatial knowledge.

Because the server has exact current state, fairness comes from an explicit bounded observation
history. At simulation tick `T`, the policy evaluates the canonical observation captured at
`T - reaction_delay_ticks`. Target acquisition, recognition of target movement, aim correction,
and tactic changes all use that delayed observation. Aim error is sampled and held for a bounded
commitment interval; it is not independently resampled every tick into visible jitter.

## Tick and schedule contract

Practice bot decisions use the authoritative fixed `SimulationTick`:

1. Match lifecycle systems apply defeat, respawn, reset, and match-generation transitions.
2. After the existing deferred-command boundary, construct and append at most one canonical
   observation per living manifest bot for the current simulation tick.
3. In `GameplaySet::Input`, evaluate at most once per bot and simulation tick using the delayed
   observation selected by the profile.
4. Validate the emitted axes, aim distance, and button mask, then atomically install the result in
   the bot fighter's existing `ActionState<FighterInput>` and set its local
   `InputFreshness.last_fresh_tick` to the current simulation tick before
   `GameplaySet::Simulation` and `GameplaySet::Fire` consume it. Do not fabricate a Lightyear input
   buffer, remote tick, sequence, or transport event.
5. Emit neutral input while the observation history is incomplete, the match is not active, or the
   fighter is defeated or waiting to respawn. Do not consume entropy or advance commitments twice
   for the same tick.

Keep this ordering visible at the server composition point and cover it with schedule-trace tests.
The implementing milestone must verify the exact owning systems and deferred boundaries before
choosing final system labels.

A later external-client adapter requires its own clock contract. It must name a specific Lightyear
interpolation/replication timeline, attach source tick and age to non-atomic replicated facts, and
must not describe a client World as one coherent authoritative snapshot without proving that
property.

## Navigation contract

The first navigation implementation is deliberately small and deterministic. From the validated
`ResolvedMapSnapshot` or equivalent resolved map resource it must:

- clamp goals to playable bounds and inflate permanent collision geometry by fighter clearance;
- distinguish line of travel from line of fire;
- steer directly when travel is clear;
- when blocked, choose a deterministic reachable obstacle corner using stable placement identity
  and a stable tie-breaker;
- support approach, desired-range, retreat, strafe, and objective movement;
- detect lack of progress over a bounded tick window and try one deterministic alternate route.

This is planning data only. Avian and the authoritative movement systems remain the owners of
collision outcomes. Introduce a waypoint graph, portal graph, mesh, or general pathfinding framework
only after supported maps demonstrate that the small steering model is insufficient. Dynamic
terrain, hazards, surfaces, and concealment extend the observation/navigation contract when their
own milestone makes them relevant; bot behavior must not use hard-coded map coordinates.

## Policy design

The first policy is **hand-written utility AI with a small committed-duration state**. It has no
Bevy ECS dependency and requires no AI framework.

- **Utility scoring chooses a tactic.** Initial tactics are `approach`, `hold_range`, `retreat`,
  `strafe`, and `contest_objective`, scored from a small set of validated curves over health,
  distance, travel/fire visibility, objective pressure, and observation age.
- **Committed state prevents dithering.** Target lock, strafe direction, retreat burst, aim-error
  hold, obstacle corner, stuck window, and reaction deadline live in one explicit `BotState` returned
  by the transition.
- **Combat begins with the representative build.** Primary fire and aim are evaluated from its
  resolved public capabilities rather than its preset ID. Unsupported active-item and ultimate
  buttons remain neutral until a later capability slice specifies their behavior.
- **Mode goals are small adapters.** Wipeout and Hot Zone produce bounded goals consumed by the
  common scorer rather than scattering mode branches throughout navigation and combat.
- **No behavior-tree framework.** A later tree or learned policy may consume the same observation
  and emit the same decision shape if a demonstrated need justifies it.

The boundary is a deterministic state transition rather than a function that mutates hidden RNG or
ECS state:

```rust
fn decide(
    observation: &BotObservation,
    state: BotState,
    entropy: BotEntropy,
    profile: &BotProfile,
) -> BotDecision
```

`BotDecision` contains the next `BotState`, ordinary `FighterInput`, selected stable target/path
identities, chosen tactic, and one bounded diagnostic reason. Diagnostics support tests and tuning;
they do not become bot-specific gameplay protocol messages.

## Profile and deterministic entropy

Ship one code-owned, validated `BotProfile` containing only values owned by the first behavior:
desired range, retreat threshold, reaction delay, aim-error bounds/hold duration, tactic commitment,
navigation clearance, and stuck detection. Add profile fields only with the behavior that consumes
them. This is typed code configuration, not player-facing difficulty or authored gameplay content.

Use a project-owned pinned deterministic sampler with an algorithm/version tag. Derive independent
entropy samples for target tie-breaking, tactics, aim error, and timing from:

```text
explicit bot seed + match ID + controller life generation + simulation tick + stream ID
```

Generate a fixed `BotEntropy` sample set before conditional policy evaluation. Adding an aim sample
must not perturb navigation or tactic choices. Aim samples affect behavior only when the committed
aim-error interval starts. Derive the default seed from the manifest bot's stable player identity;
logs and test artifacts report the effective seed, algorithm version, profile version, match ID,
and controller life generation. These versions belong to internal trace/replay artifacts, not
separate application-protocol compatibility fields.

## Lifecycle and practice integration

The existing Start Practice transaction remains the entry point. Manifest bot rows continue to own
stable display name, player ID, team, selected build snapshot, and spawn assignment. Worker
materialization additionally installs private controller and policy-state components for those
rows; connected human participants never receive them.

Use a lifecycle key consisting of match ID and a private monotonic controller life generation
derived after authoritative lifecycle transitions are visible. On defeat or respawn waiting,
immediately install neutral input. Increment the generation and construct fresh observation history
and `BotState` once on the transition back to an `ActiveCombatant`; do not reset independently on
both defeat and respawn. Changing match ID resets all match-scoped state and derives new entropy
streams. Match completion and restart retain existing authoritative outcomes. A missing or rejected
bot decision fails closed to neutral input and bounded diagnostics; it must not panic or stall the
match worker.

External client launch flags, bot network admission, bot check-in, supervisor sidecars, and bot
process shutdown are not part of this lifecycle.

## Suggested ownership

Keep pure behavior separate from the server adapter without creating a generic AI framework:

```text
bots/
  mod.rs          private composition and narrow model exports
  model.rs        BotObservation, BotState, BotDecision, stable policy facts
  policy.rs       pure utility scoring and deterministic transition
  navigation.rs   small map-derived steering and stuck recovery
  profile.rs      validated first-profile values and version
  entropy.rs      pinned deterministic independent samples

server/practice/
  mod.rs          manifest bot materialization and practice composition
  controller.rs   ECS observation allowlist, lifecycle, schedule, input installation
```

The exact file split remains subject to implementation evidence. The required boundary is that pure
policy/navigation code cannot query or mutate a Bevy `World`, while the server adapter cannot become
a second gameplay implementation. Until a second execution host exists, compile the bot module only
where the practice server uses it rather than broadening client or public APIs speculatively.

## First-slice verification

A first playable bot milestone must include:

- pure transition tests for canonical synthetic observations and committed state;
- deterministic ordering/tie tests with ECS insertion and collection orders permuted;
- reaction-history tests proving target movement, aim correction, and tactic changes use the
  configured delayed tick;
- entropy-stream tests proving one stream cannot perturb another and aim error remains held for its
  commitment interval;
- invariants proving all emitted axes, distances, and buttons are finite and bounded;
- neutral-input tests for incomplete history, dead, respawning, non-active, invalid-output, and
  lifecycle-reset cases;
- no-target tests proving target-dependent attack/ability buttons remain neutral while valid
  objective or committed movement may continue;
- fixed-schedule traces proving lifecycle/deferred changes precede one decision and input
  installation precedes authoritative movement/fire;
- map steering tests for direct travel, one blocked route, stable equal-choice ordering, and bounded
  stuck recovery on each practice-supported built-in map;
- focused Wipeout and Hot Zone goal tests;
- representative-build tests for range holding, aim imperfection, and primary fire;
- worker-level practice tests proving Start Practice creates one human plus active manifest bots,
  bots affect the match only through validated `FighterInput` plus its local freshness marker, and
  defeat/respawn/restart remain authoritative;
- repeated restart evidence for bounded observation history, state, diagnostics, and entity cleanup;
- a human playtest evaluating readability, aim imperfection, retreat/strafe telegraphing, objective
  behavior, whether bots navigate accepted maps, and whether practice is useful and fun.

Record the effective seed and canonical observation/decision trace when a policy failure needs
deterministic replay. Do not treat wall-clock process or network traces as policy golden tests.

## Later capability gates

Add later behavior only when its owning slice is implemented and tested:

- additional straight, lobbed, and melee bot-controlled builds;
- active items, Dash, Sentry, deployable placement, and other ability primitives;
- bounded projectile awareness and avoidance without perfect tracking;
- navigation graphs or dynamic-terrain planning when simple steering proves insufficient;
- concealment tests proving hidden state cannot enter `BotObservation`;
- an external headless-client adapter with explicit replication/interpolation clock semantics;
- real subprocess/UDP and impairment tests for that external adapter;
- supervisor-managed bot processes only if automatic multiplayer or practice hosting requires them.

## Deferred and non-goals

- Assigning bots to a milestone before a future version is scoped.
- Lobby fill detection, matchmaking, in-match backfill, or automatic multiplayer bot substitution.
- External bot subprocess orchestration in the first playable practice slice.
- Player-facing difficulty presets, skill ranking, adaptive difficulty, or automatic balance tuning.
- Bot-specific movement, combat, damage, score, respawn, or terrain authority.
- Bot-only gameplay protocol messages or per-message compatibility versions.
- Learned-policy training, replay datasets, inference runtimes, or remote policy services.
- A generic public bot SDK before an external consumer demonstrates the boundary.
- Behavior-tree or utility-AI dependencies while the focused policy remains maintainable.

Anything outside the official server or a future official client adapter that later speaks the
network protocol must implement Brawler's global compatibility/content handshake, session
transactions, native input buffering, replication rules, and admission contracts. Protocol
compatibility alone does not make an implementation a supported bot host.
