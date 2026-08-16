# Milestone 09 — Hot Zone

## Tracking

| Field | Value |
|---|---|
| Version | v1 — gameplay MVP |
| Roadmap | [roadmap.md](./roadmap.md) |
| Status | Specification review |
| Research | Complete; product/network requirements, the live M08 review worktree, pinned dependency material, installed exact-version sources, primary versioned documentation, and specification-review feedback incorporated through 2026-08-16 |
| Specification validation | Awaiting user approval; production implementation must not begin before approval |
| Implementation | Not started; additionally gated on M08 feedback closeout and an exact green starting baseline |
| Verification | Not started |
| User validation/playtest | Not started |

Milestone 08 remains in feedback review. Researching and reviewing this specification in parallel is
safe, but M09 implementation must begin from the accepted M08 result rather than from a moving review
worktree. Before entering `Implementing`, record the exact starting commit, reconcile all accepted M08
feedback, and rerun the M08 automated technical gate.

## Outcome

The dedicated server can start either Wipeout or one-zone Hot Zone through explicit validated process
configuration. Both modes use the same map resolver, waiting/countdown/active/completed/restart
lifecycle, teams, builds, fighters, movement, weapons, abilities, respawns, protection, combat facts,
and cleanup. Only the installed mode rules, required map anchor, scoring/progress state, presentation,
and telemetry differ.

During an active Hot Zone match, the server evaluates living fighter centers against one resolved
area anchor once per authoritative fixed tick. Exactly one represented team advances one integer
progress unit; an empty or contested zone advances neither team. The first team to 1,800 progress
ticks (30 seconds of uncontested control at 60 Hz) wins. A 10,800-tick active limit (three minutes)
awards the win to the higher progress total or a draw when totals are equal. Durable replicated state
lets current, late, and reconnecting clients reconstruct the zone, occupancy status, progress, timer,
and result without replaying capture history.

## Decisions requiring specification validation

1. Extract M07's reusable lifecycle from the Wipeout composition without replacing the ECS runtime
   model. `AuthoritativeMatchPlugin` owns common phases, roster, activation, respawn, disconnect,
   forfeit precedence, mode-outcome consumption, completion, restart, cleanup, and summary
   boundaries. Focused
   `WipeoutModePlugin` and `HotZoneModePlugin` systems own only mode scoring and completion rules.
2. Add an explicit server `GameMode` configuration (`wipeout` default, `hot-zone` optional). The
   selected mode determines both the installed rule plugin and a compatible built-in map preset.
   Clients do not request or vote on the mode in M09; they learn it from replicated map/match state.
3. Extend the embedded map catalog with a second, objective-focused variant of Crossroads Facility.
   It keeps the established arena bounds, geometry, spawn safety, and presentation theme, selects the
   Hot Zone mode definition, and adds exactly one circular `ModeAnchorShape::Area` at the central
   objective. The map recipe places data; it does not carry executable capture rules.
4. Use fighter-center containment against the normalized resolved area shape. Count only fighters in
   the current match with `ActiveCombatant`, positive authoritative `CurrentHealth`, a valid team 0/1,
   finite authoritative `Position`, and a `NetworkEntityId` present in the common roster's accepted,
   currently connected participant set for that match. Defeated/respawning/selecting fighters,
   sentries, projectiles, dummies, disconnected or stale-but-not-yet-despawned fighter entities, and
   other match generations never occupy the zone. Spawn-protected living fighters do count.
5. Resolve the complete occupancy snapshot once per fixed tick after authoritative movement/physics
   and combat damage are visible. Empty means `[0, 0]`; single-team means at least one occupant for
   exactly one team; contested means both teams have at least one occupant. Simultaneous entry is not
   ordered by entity iteration or input arrival: the final same-tick snapshot is contested immediately.
6. Progress is non-decaying, non-stealable, and independent per team. One uncontested team advances
   exactly one checked/saturating integer unit per active fixed tick, regardless of whether one or two
   teammates occupy the zone. Empty and contested ticks advance zero. No client event or duplicated
   input can directly advance progress.
7. Add `HotZoneState` as a durable replicated component on the existing `MatchRoot` entity rather than
   introducing capture messages or a parallel snapshot resource. It contains the stable zone anchor
   identity, bounded occupant counts, `Empty | Controlled(TeamId) | Contested` status, both progress
   totals, target progress, and one plain next-evaluation tick initialized at activation.
8. Keep `MatchState` as the mode-neutral phase/result/identity envelope. Its existing Wipeout
   `team_scores` and `target_score` fields move into a Wipeout-specific replicated state component;
   clients and summaries branch on stable `ModeDefinitionId`, never on fighter/weapon/build code.
   This is an intentional protocol migration that removes misleading generic score fields before a
   second scoring model entrenches them.
9. Use ordinary Lightyear component replication for Wipeout/Hot Zone durable state. Hot Zone progress
   is not predicted or interpolated and uses no gameplay cue as recovery truth. Audio/VFX cues may be
   lossy presentation reactions; late join and reconnect recover solely from the replicated match,
   map snapshot, and `HotZoneState`.
10. Reuse the existing HUD hierarchy and arena presentation. Render a persistent world-space zone
    boundary/fill from the resolved anchor, a two-team 0–100% objective bar, explicit EMPTY/CONTESTED/
    CONTROLLED feedback, timer, and results. Presentation may smooth a displayed bar between received
    values but never extrapolates authoritative progress beyond the last replicated value.
11. Extend the existing bounded match telemetry and process evidence path. Do not add an analytics
    service, objective input, physics sensor collider, mode scripting framework, or new dependency.
12. Add a generation-tagged `MatchClock { match_id, completed_tick }` to the match root. Keep the
    existing fighter `AuthoritativeTick` unchanged. Client match deadlines are presentable only when
    the clock, match envelope, and concrete mode state carry the same match ID.
13. Treat `Active { ends_at_tick }` as the half-open interval from activation tick through
    `ends_at_tick - 1`. The activation tick is the first eligible objective evaluation and
    `ends_at_tick` is the first tick that cannot add progress, giving exactly `active_limit_ticks`
    evaluations. On the boundary tick, a pre-game mode deadline system checks existing/recovered
    threshold state before timeout and common completion locks the match before input or simulation.

Changing the authority boundary, occupancy eligibility, same-tick rule, progress model, target or
timer semantics, map/mode selection ownership, durable recovery shape, or common/mode plugin boundary
returns M09 to specification review. Numeric presentation tuning and objective colors may change
during implementation/playtest if recorded and if gameplay semantics remain unchanged.

## Source requirements

- [Product direction](../../00-product-direction.md): combat readability, short matches, reusable
  content primitives, meaningful builds, and server-authoritative networking.
- [Gameplay MVP](../../05-gameplay-mvp.md): Hot Zone must prove that the same fighter, weapon,
  ability, and lifecycle code works under spatial-control rules.
- [Network architecture](../../08-network-architecture.md): the server owns objectives, scores,
  mode rules, victory, and recovery; map recipes place required anchors but cannot author executable
  rules; durable objective state should use registered replicated components.
- [Version 1 roadmap](./roadmap.md): one-zone scope, required cases, telemetry, reuse criteria, and
  measurable exit criteria.
- [Milestone 06](./milestone-06.md): typed map recipe/resolution, stable mode/anchor identities,
  exact-generation installation, client reconstruction, and future-compatible validation.
- [Milestone 07](./milestone-07.md): authoritative common lifecycle, mode-neutral combat facts,
  respawn/protection, restart cleanup, HUD, four-client harness, and match telemetry.
- [Milestone 08](./milestone-08.md): immutable loadouts, abilities/passives/deployables, expanded
  combat facts, presentation, evidence, and the review baseline that M09 must inherit.

## Scope boundaries

### In scope

- explicit Wipeout/Hot Zone server selection and validation, with Wipeout as the compatibility default;
- one Hot Zone mode definition, layout schema, area-anchor definition, and built-in arena variant;
- exact area-anchor validation, normalized circle/rectangle containment helpers, and stable anchor ID;
- common lifecycle extraction sufficient to compose either mode without duplicating phase, roster,
  respawn, protection, disconnect, completion, restart, or cleanup machinery;
- Wipeout-specific score state migration and one replicated `HotZoneState` on the match root;
- fixed-tick authoritative occupancy, progress, threshold completion, timeout, tie, forfeit, restart,
  and repeated-match reset;
- world-space zone presentation, objective HUD, controller/keyboard-readable state, placeholder audio,
  and results integration;
- objective telemetry, deterministic unit/App/network tests, multi-process verification, performance
  evidence, visual checks, controller checks, and a focused Hot Zone playtest;
- regression proof that the accepted Wipeout and M08 build/ability paths remain unchanged in behavior.

### Out of scope

- multiple zones, rotating/moving zones, capture ordering, sequential hills, overtime, lockouts,
  comeback multipliers, progress decay, progress stealing, capture speed by headcount, neutralization,
  payload escort, king-of-the-hill rounds, or best-of-series rules;
- an interact button for capture, client-authored occupation/capture events, objective prediction,
  rollback, lag compensation, or client-side hit/zone authority;
- objective-specific build bonuses, passives, status effects, healing, pickups, hazards, environmental
  damage, destructible objective geometry, or terrain changes from M10;
- mode voting, lobby browsing, matchmaking, map rotation, user-authored maps, map editor/publishing,
  persistent playlists, remote configuration, or live balance services;
- production art/audio, announcer systems, spectator UI, replay, ranked scoring, or analytics upload;
- bots. Existing deterministic harness clients and scripted process inputs are sufficient for M09;
  the separate bot design is not a prerequisite or hidden implementation task.

## Research questions and conclusions

### Can the current map model express Hot Zone without a new map abstraction?

Yes. `ModeAnchorPlacement` already separates stable anchor identity from `Point` and `Area` shapes,
and `ResolvedMap` indexes anchors by definition. M06 deliberately reserved this boundary. M09 only
needs a Hot Zone mode/anchor definition, an `area_only` requirement, a second preset, and stricter
validation that the complete area's extents remain inside playable bounds.

The objective should be a mode anchor, not a `MapRegionPlacement`: regions currently describe
terrain reservations and carry presentation/collision profiles. Reusing that type would conflate a
mode-required semantic location with mutable terrain planning. The anchor supplies gameplay geometry;
a client-only zone visual is derived from it.

### Should occupancy use Avian sensors or an ECS geometry query?

Use a pure geometry query over authoritative `Position`. Hot Zone needs one bounded area and at most
four active fighters in v1. A sensor collider would add contact lifecycle, collision-layer, deferred
event, and respawn cleanup states without improving correctness. The pure `contains_point` helper is
deterministic, cheap, directly testable, and works for both supported `MapShape` variants.

Avian remains authoritative for movement and wall collision. The objective system reads positions
after the physics step; it does not become a movement or collision owner.

### What is the reusable match boundary exposed by the live M08 worktree?

The data boundary is already close: `MatchState` carries identity/phase/mode/result, combat emits
mode-neutral facts, and fighter lifecycle is separate. The composition is not yet reusable because
`WipeoutPlugin` initializes the root, owns common lifecycle and timeout/restart, and resolves Wipeout
scoring in one file; map startup also hardcodes preset 1 plus `MapLayoutRequirements::wipeout()`.

Extract only demonstrated common ownership. Do not create trait objects, a generic rule interpreter,
or one crate per mode. Static Bevy plugins and explicit system sets are sufficient for two modes.

### What progress representation avoids drift and duplicate advancement?

Store integer controlled ticks, not floating-point percentages or elapsed wall time. At the fixed
60 Hz simulation rate, a target of 1,800 is exactly 30 seconds of uncontested control. The objective
system runs once for each `SimulationTick`, advances a plain `next_evaluation_tick`, and refuses to
evaluate a tick twice. HUD percentage is `progress * 100 / target` in presentation code.

This makes packet duplication irrelevant because inputs only affect server-owned positions. It also
keeps progress deterministic under varied render rates and process impairment profiles.

### How should same-tick damage, movement, and capture be ordered?

Evaluate after Avian's `PhysicsSystems::StepSimulation` and combat `Damage`, within the existing
fixed-post outcome transaction. Eligibility checks positive `CurrentHealth` directly so a fighter
reduced to zero in the same tick cannot capture even if the deferred `Defeated` marker is not yet
visible. Evaluate the full sorted roster first, then perform one progress mutation.

The required order is:

```text
fixed lifecycle -> pre-game forfeit/deadline outcome and completion when due
fixed input/abilities/movement/fire
  -> Avian physics
  -> projectile/melee/payload damage and current-health mutation
  -> ability/passive outcome observers
  -> Hot Zone occupancy snapshot and one progress step
     OR Wipeout defeat scoring
  -> one eligible-tick mode threshold outcome consumption
  -> lifecycle cleanup, telemetry/cues, authoritative tick publication
```

### How do late join, reconnect, and restart recover?

The replicated map snapshot contains the exact area anchor; `MatchState.phase` contains the relevant
countdown/active/restart deadline or completed result; the match root's generation-tagged `MatchClock`
supplies the shared deadline clock; and `HotZoneState` contains current progress and occupancy.
Ordinary component replication therefore supplies a current-state snapshot to every accepted client,
while matching match IDs prevent separately arriving revisions from being combined across a restart.
No historical capture event is required. M07's no-session-resumption policy remains: reconnect creates
a new participant under the current admission rule, while an active match rejects joining.

Restart allocates the new match ID, resets both progress totals/status/occupants/evaluation tick,
retains the installed map/mode/loadouts under the established lifecycle rules, and clears all
mode-scoped telemetry. A stale state from the prior match ID is ignored by presentation and evidence.

### Do the exact dependency versions support the plan?

Yes. Installed Bevy 0.19.1 defines `FixedUpdate` for fixed-rate game rules and `FixedPostUpdate` for
reacting after fixed main logic; explicit set ordering and `ApplyDeferred` remain available. The
pinned Lightyear 0.29 material confirms registered components on a replicated entity converge as
durable state, including for a newly connected receiver. Avian 0.7 exposes authoritative `Position`
and its physics schedule; M09 requires no additional spatial-query API.

The checked-in Bevy repository is 0.20-dev, so exact API transfer must continue to use installed
Bevy 0.19.1 source or version-pinned documentation. The local Lightyear and Avian snapshots match the
Cargo pins.

## Research log

| Date | Source | Finding | Decision |
|---|---|---|---|
| 2026-08-15 | `docs/{00-product-direction,05-gameplay-mvp,08-network-architecture}.md` and `docs/implementation/v1/roadmap.md` | M09 is a reuse/authority test, not a content-expansion milestone. | Keep one zone, one arena variant, fixed rules, and require combat/lifecycle reuse. |
| 2026-08-15 | Live `src/{map,matchplay,combat,movement,abilities,client,server,protocol}.rs`, `content/v1/maps.ron`, and network/performance tests | Stable area anchors, resolved snapshots, match root, mode-neutral combat facts, fixed schedule sets, and four-client harness already exist. Map and match composition still hardcode Wipeout. | Extend the existing seams and extract only the common lifecycle/map selection needed by a second mode. |
| 2026-08-15 | [Milestone 08](./milestone-08.md) review worktree | Builds, ultimates, passives, sentries, generic combat sources, and expanded evidence are implemented but M08 remains in feedback review. | Permit M09 specification review in parallel; gate implementation on accepted M08 closeout and a green exact baseline. |
| 2026-08-15 | `references/lightyear/book/src/concepts/replication/{protocol,replicate}.md` and [Lightyear 0.29 documentation](https://docs.rs/lightyear/0.29.0/lightyear/) | Registered ordinary components on a replicated entity provide durable current-state convergence; no event history is needed for objective recovery. | Replicate mode state on the existing match root; reserve cues for presentation. |
| 2026-08-15 | Installed Bevy 0.19.1 `bevy_app/src/main_schedule.rs` and [versioned `FixedPostUpdate` docs](https://docs.rs/bevy/0.19.1/bevy/app/struct.FixedPostUpdate.html) | Fixed game rules belong in fixed schedules, and post-fixed logic can explicitly react after movement/physics/damage. | Add an explicit mode-rule set inside the existing fixed-post transaction. |
| 2026-08-15 | Installed Avian 0.7 source and [Avian 0.7 documentation](https://docs.rs/avian2d/0.7.0/avian2d/) | Authoritative `Position` is already the simulation pose; one simple zone does not justify sensors or another query pipeline. | Use normalized point-in-shape math after physics and keep the objective non-colliding. |
| 2026-08-16 | User-provided M09 specification review, checked against live `MatchPhase`, `AuthoritativeTick`, map anchor, fact consumption, roster, telemetry, and schedule code | The root clock lacked a generation tag; deadline-tick eligibility was ambiguous; disconnected fighter eligibility, common fact draining/respawn ownership, diagnostic ownership, and mode-summary meanings needed explicit contracts. | Add `MatchClock`, use a half-open active interval, gate occupancy through the connected roster, define a shared read-then-clear fact transaction and common respawn system, keep outcome diagnostics common, and fully type mode summaries. |

## Technical specification

### Application and module composition

Keep one package and the current role feature gates. Evolve the focused boundaries as follows:

```text
content/v1/maps.ron
src/map/
  definitions/          Hot Zone IDs, layout requirements, two validated presets
  model.rs              reusable normalized area containment helper
  server.rs             selected preset/requirements installation from server config
  client.rs             resolved map reconstruction; no hardcoded Wipeout validation
src/matchplay/
  mod.rs                 common sets/plugins and intentional public API
  model.rs               mode-neutral match envelope/result/participant state
  lifecycle.rs           common fighter lifecycle (existing ownership retained)
  server.rs              common match root, roster, phases, forfeit, restart, cleanup
  wipeout.rs             Wipeout rules, state, scoring, mode plugin
  hot_zone.rs            Hot Zone rules/state, occupancy/progress, mode plugin
  telemetry.rs           common summaries plus tagged mode-specific summaries
src/client/hud.rs        mode-dispatched HUD/results text
src/map/client.rs        world-space objective boundary/fill presentation
tests/network/hot_zone.rs
```

Recommended plugins:

| Plugin | Installed in | Responsibility |
|---|---|---|
| `MapContentPlugin` | client/server/tests | Validate both recipes, stable definitions, fingerprints, and layout schemas |
| `AuthoritativeMapPlugin` | server/tests | Resolve the configured compatible preset and install one exact map generation |
| `AuthoritativeMatchPlugin` | server/tests | Common root, roster, lifecycle, activation, completion envelope, restart, cleanup, and summary boundaries |
| `WipeoutModePlugin` | Wipeout server/tests | Defeat scoring, Wipeout state, threshold/timeout result |
| `HotZoneModePlugin` | Hot Zone server/tests | Area resolution, occupancy, progress, Hot Zone threshold/timeout result |
| existing `ProtocolPlugin` | client/server/tests | Register migrated match state and both concrete mode-state components |
| existing client presentation plugins | windowed client | Mode-aware zone/HUD/audio presentation only |

`ServerNetworkPlugin` must stop unconditionally installing Wipeout. The production composition reads
validated configuration once during app construction and installs exactly one mode plugin. Tests may
construct focused apps directly. No runtime hot-swap occurs inside a process in M09.

### Server configuration and compatible map selection

Add shared configuration shapes:

```text
GameMode
  Wipeout
  HotZone

MatchRulesProfile
  Production
  ProcessVerification

ServerNetworkConfig additions
  game_mode: GameMode = Wipeout
  match_rules_profile: MatchRulesProfile = Production
```

Expose `--mode <wipeout|hot-zone>` and migrate `--wipeout-rules` to the mode-neutral
`--match-rules <production|verification>`. During one compatibility release, accepting the old flag
as an alias is permitted if conflicting flags fail validation; documentation and scripts use the new
name. The client needs no mode flag.

Use a code-owned match setup table:

| Mode | Mode definition | Built-in preset | Layout requirements | Mode plugin |
|---|---:|---:|---|---|
| Wipeout | 2 | 1 | `MapLayoutRequirements::wipeout()` | `WipeoutModePlugin` |
| Hot Zone | 3 | 2 | `MapLayoutRequirements::hot_zone()` | `HotZoneModePlugin` |

Configuration validation rejects an unknown mode/profile or an incompatible explicit preset if map
override is later exposed. M09 does not expose arbitrary map selection.

### Map content and validation

Add stable catalog definitions:

- `HOT_ZONE_MODE_DEFINITION = ModeDefinitionId(3)`;
- `HOT_ZONE_OBJECTIVE_ANCHOR_DEFINITION = ModeAnchorDefinitionId(2)`;
- `HOT_ZONE_LAYOUT_SCHEMA_VERSION = 1`;
- `HOT_ZONE_MAP_PRESET = MapPresetId(2)`.

Preset 2 reuses Crossroads Facility's bounds, six permanent obstacles, floor tiling, destructible
reservation, team spawn areas, and eight spawn points. It has a distinct recipe ID/revision, selects
mode 3, and adds one circular area anchor at `(0, 0)` with radius `160`. Add
`HOT_ZONE_OBJECTIVE_PRESENTATION_PROFILE` to the catalog. Client presentation uses one code-owned
mapping from `HOT_ZONE_OBJECTIVE_ANCHOR_DEFINITION` to that profile; the anchor continues to own exact
geometry and identity, while the profile owns only color/material/line styling. This does not add a
presentation field to the anchor wire shape or assign gameplay meaning to the destructible-reservation
region. Catalog validation and a client test require the mapping and profile to exist.

Catalog validation accepts exactly the two known ascending presets for this gate and resolves every
preset through its mode-specific requirements. `MapLayoutRequirements::hot_zone()` requires teams
0/1, one spawn area and 3–8 spawn points per team, exactly one matching area anchor, existing allowed
region/entity profiles, and no unsupported anchors.

Replace `RequiredAnchor.point_only` with an explicit `RequiredAnchorShape::{Point, Area}` constraint.
For area anchors, validate finite normalized position/shape, code-owned radius/extent bounds, the
complete extents inside playable bounds, and no intersection with permanent terrain after adding the
standard fighter radius clearance. `ModeAnchorShape::Area` has no rotation in the current wire shape,
so rectangle objectives remain axis-aligned in M09. The resolved snapshot retains the canonical
anchor unchanged for client reconstruction.

### Shared lifecycle and mode state

Recommended shared state:

```text
MatchState component
  match_id
  mode_definition_id
  phase:
    Waiting
    Countdown { starts_at_tick }
    Active { ends_at_tick }
    Completed { completed_at_tick, restart_unlocked_at_tick, result }
  rules_revision

MatchClock component on MatchRoot
  match_id
  completed_tick

WipeoutState component (present only for Wipeout)
  team_scores: [u16; 2]
  target_score: u16

HotZoneStatus
  Empty
  Controlled { team: TeamId }
  Contested

HotZoneState component (present only for Hot Zone)
  match_id
  zone_anchor_id: ModeAnchorId
  occupants: [u8; 2]
  status: HotZoneStatus
  progress_ticks: [u16; 2]
  target_progress_ticks: u16
  next_evaluation_tick: u64
```

The phase variants are the replicated deadline/result contract: countdown, active timeout, completed
result, and restart unlock remain fields of `MatchPhase`, not separate optional fields on
`MatchState`. Register and replicate the new `MatchClock`; do not reuse or change the fighter-owned
`AuthoritativeTick`. Update `MatchClock.completed_tick` in fixed finalize before
`SimulationTick` advances. The explicitly chained restart transaction assigns the new match ID and
current tick to the clock before any downstream system observes the new generation.

Match HUDs derive countdown/remaining/restart time only from the phase deadline minus
`MatchClock.completed_tick`, using saturating arithmetic and `SIMULATION_TICK_HZ`. They do not use a
client-local `SimulationTick`, wall time, render time, or Lightyear interpolation timeline. A client
uses the clock only when `MatchClock.match_id == MatchState.match_id` and the concrete mode state's
match ID also agrees. Because replicated component updates are not an atomic multi-component
transaction, every other arrival order displays `syncing` rather than combining a new match phase
with the prior generation's tick.

The mode state lives on the `MatchRoot` and carries the same `match_id`. Exactly one of
`WipeoutState` and `HotZoneState` is present for the process-selected mode. Restart does not remove and
reinsert that component. A common prepare system allocates one `PendingMatchRestart { previous_id,
next_id, restart_tick }`; the installed mode's reset system mutates its existing component in place;
and a common commit system updates `MatchState`, `MatchClock`, and participants and consumes the slot.
These systems are an explicit chain with no intervening observer, deferred flush, or replication
extraction, and downstream systems run only after commit. Thus the fixed schedule exposes one fully
committed generation, while client readiness still treats separately arriving network component
revisions as non-atomic and requires matching IDs. Runtime mode switching is out of scope.

Common code never interprets or resets a mode's totals. The only common↔mode *completion-result*
handoff is this bounded server-only resource:

```text
ModeRuleOutcome resource
  pending: Option<PendingModeRuleOutcome>

PendingModeRuleOutcome
  match_id
  evaluated_tick
  cause: Threshold | Timeout
  result: MatchResult
```

The installed mode may write the empty slot in exactly two places: its pre-game deadline system in
`MatchSet::DeadlineRules` when `SimulationTick >= ends_at_tick`, or its post-damage scoring/progress
system in `MatchSet::ModeRules` on an eligible active tick. The common consumers in
`MatchSet::PreGameOutcomes` and `MatchSet::Outcomes` always use `take()`, validate the current match ID
and exact `SimulationTick`, and either commit one result or discard it as stale after incrementing a
saturating diagnostic. Common pre-game forfeit resolution has precedence and still takes/discards any
same-tick deadline outcome. The slot is explicitly empty after either consumer and cleared on
activation, completion, and restart. A second write in one tick is rejected and counted; there is no
dynamic dispatch, unbounded retention, or outcome that can survive into gameplay or the next tick.

Extract common rule values into validated `MatchLifecycleRules`: team capacity, countdown, active
limit, respawn delay, spawn protection, completed-input lock, movement telemetry epsilon, and bounded
retention limits. `WipeoutRules` retains target score; `HotZoneRules` adds target progress. Production
defaults share the existing three-minute active limit and lifecycle timings. The verification profile
uses a 30-tick progress target and otherwise shortens deadlines without changing semantics.
`HotZoneRules::validate` requires `target_progress_ticks >= 2` and
`target_progress_ticks <= active_limit_ticks.min(u64::from(u16::MAX))`, so an uncontested match can
complete before timeout. `MatchState.rules_revision` is the revision of the installed, validated
common-plus-mode rules composition, not only the lifecycle or mode fragment. Other lifecycle
deadlines retain their nonzero and checked-combination validation.

If activation occurs at tick `A` with active limit `L`, the phase stores
`ends_at_tick = A.checked_add(L)`. Objective evaluation is permitted exactly for ticks
`A..ends_at_tick`; the activation tick is eligible after that tick's movement, physics, and damage,
and the deadline tick is not. Thus an uninterrupted controller receives exactly `L` opportunities and
a target of `L` completes on `ends_at_tick - 1`. At `ends_at_tick`, a mode first recognizes any
already-present target state produced by recovery/injected test setup and emits `Threshold`; otherwise
it emits `Timeout`. The pre-game common consumer commits that result and locks fighters before input,
abilities, movement, fire, or physics for the boundary tick. This makes threshold-over-timeout
precedence explicit without granting an extra capture or combat evaluation. Waiting, countdown, and
completed phases never evaluate occupancy.

Forfeit remains common and takes precedence once a team has no connected participants. Threshold
completion takes precedence over timeout on the same tick. A mode result is resolved once, then the
common completion helper locks fighters and produces one final result.

### Occupancy and progress algorithm

Resolve the configured zone anchor during match-root initialization and fail startup if it is absent,
duplicated, wrong-shaped, or from a map/mode mismatch. Cache only the stable normalized resolved area
in a server resource scoped to the installed map instance; the replicated anchor remains the source
for client presentation.

The common roster snapshot carries a sorted set of connected `NetworkEntityId` values for its match
ID. A fighter enters that set only while its owning session is accepted, its controlling link is
present and not disconnected, and its `MatchParticipant.match_id` matches the root. Both forfeit and
Hot Zone occupancy read this same fixed-tick snapshot. Session cleanup may despawn the fighter later;
loss of connected-roster membership alone makes a lingering fighter ineligible on the fixed tick that
resolves the forfeit.

Each active tick:

1. The pre-game deadline phase has already completed the match when the current tick is at or after
   `ends_at_tick`; the post-damage Hot Zone system therefore does not run for a deadline tick.
2. If `SimulationTick < next_evaluation_tick`, return without mutation and increment the duplicate-
   evaluation counter. If it is greater, record the skipped-tick distance but evaluate the current
   state only once; never catch progress up for positions that were not observed.
3. Iterate eligible fighters and reject a participant whose match ID differs, whose network identity
   is absent from the matching connected-roster set, whose team is outside 0/1, whose health is zero,
   or whose position is non-finite.
4. Test the fighter center against the zone in zone-local coordinates. A point on the circle or
   rectangle boundary counts as inside. Increment each team count with checked/saturating `u8` math.
5. Derive `Empty`, `Controlled(team)`, or `Contested` from the complete counts.
6. If controlled, increment only that team's progress by one, capped at target. Otherwise increment
   neither. Write occupant counts, status, progress, and
   `next_evaluation_tick = SimulationTick.saturating_add(1)` together. Activation initializes the
   field to its first eligible authoritative tick; restart resets it for the next activation.
7. Resolve threshold result after the mutation. If both totals are at target due only to injected or
   recovery test state, compare totals and return draw when equal; normal play cannot advance both on
   one tick.

Occupancy is independent of attack events and client input sequence count. It reads only current
authoritative ECS state. Entity iteration order cannot affect the outcome. Circle inclusion is
server-only `delta.length_squared() <= radius * radius` in `f32` with no epsilon that would silently
expand the zone. Exact-boundary tests use exactly representable axis-aligned points on the radius-160
circle (`center +/- (160, 0)` and `center +/- (0, 160)`); near-boundary tests use deliberately separated
inside/outside values. Rectangle comparisons use inclusive component bounds. Clients may repeat this
math only for presentation and never author occupancy.

`HotZoneDiagnostics` uses saturating scalar counters only for duplicate evaluation, skipped evaluation
ticks, invalid/ineligible fighters, and occupant-count saturation. Common
`MatchOutcomeDiagnostics` owns stale-mode-outcome, duplicate-mode-outcome, wrong-match outcome, and
wrong-tick outcome counters because either mode can trigger them. Neither resource appends an
allocation or raw record per fault. Any sampled raw evidence goes through the existing bounded match
record deque and its dropped-record counter, so repeated impairment cannot grow memory.

### Scheduling and deferred-command boundaries

Extend `MatchSet` with a visible mode-rule phase while preserving the current combat transaction:

```text
FixedUpdate
  MatchSet::Lifecycle
    common roster -> waiting/countdown -> activation
    -> restart prepare -> installed-mode in-place reset -> common restart commit/cleanup
  MatchSet::DeadlineRules
    installed mode checks existing threshold, otherwise timeout, only at/after ends_at_tick
  MatchSet::PreGameOutcomes
    common forfeit check/precedence -> take/validate deadline outcome -> one result commit/lock
  MatchSet::FighterLifecycle
    respawn/reset/protection
  Gameplay input -> abilities -> movement -> fire

FixedPostUpdate
  Avian StepSimulation
  CombatSet::ProjectileSweep
  CombatSet::Damage
  AbilitySet::ObserveOutcomes
  MatchSet::ModeRules
    prepare/sort current combat facts without draining; clear/validate empty ModeRuleOutcome slot
    Wipeout reads current combat facts for scoring OR Hot Zone evaluates occupancy/progress
    write at most one current-match/current-tick PendingModeRuleOutcome
  MatchSet::Outcomes
    take/validate eligible-tick mode outcome -> one result commit
    -> ApplyDeferred -> common defeat/respawn handling
    -> common and installed-mode telemetry read current combat facts
    -> clear current combat facts exactly once -> ApplyDeferred -> cleanup -> summary
  CombatSet::Lifecycle -> TelemetryAndCues -> Finalize
```

Hot Zone eligibility reads `CurrentHealth`, so it does not depend on seeing a deferred `Defeated`
insert. `CombatOutcomeFacts` is a combat-owned, current-tick observation buffer, not a common↔mode
result handoff. Ability observers, Wipeout scoring, common combat telemetry, and Hot Zone near-zone
telemetry read it without draining it. A common preparation system sorts the buffer once before mode
readers; every scoring/telemetry reader independently rejects wrong-tick or wrong-match facts under
the common diagnostic policy. The Wipeout plugin retains its bounded per-match scored-event ID set so
duplicate facts cannot score twice. A single common finalizer records bounded common telemetry and
then clears the buffer after every registered reader has run, including when no match is active or a
root is missing.

`DeadlineRules` and `PreGameOutcomes` are chained after lifecycle/restart and before fighter lifecycle
and every authoritative gameplay set. They preserve M07 behavior: `ends_at_tick` is the first tick
with no accepted match input, ability activation, movement, firing, damage, or objective evaluation.
Only the installed mode reads its durable totals to distinguish recovered threshold from timeout;
common code applies forfeit precedence and commits the opaque `MatchResult` through the same helper
used by post-damage completion.

Damage continues to insert `Defeated` through deferred commands. After result commitment, the first
`ApplyDeferred` makes that marker visible. A common defeat-lifecycle system—not either mode plugin—then
adds `RespawnState` only to newly defeated fighters belonging to the still-active current match. If
forfeit or a mode outcome completed the match, it leaves those fighters locked with the rest of the
match and schedules no respawn. Its commands are applied by the second explicit `ApplyDeferred` before
later lifecycle cleanup. Sentry cleanup remains separate and does not enter fighter respawn handling.

Add a schedule trace test that proves deadline completion precedes all gameplay on `ends_at_tick`;
eligible objective evaluation happens after movement/physics and damage; fact readers run before the
one clear; deferred defeat becomes visible before common respawn handling; and all cleanup precedes
simulation-tick advancement. A second trace runs both pre-game and post-damage outcome consumers twice
and advances another tick to prove `take()` prevents duplicate or stale completion.

### Protocol, replication, and recovery

Register `WipeoutState`, `HotZoneState`, and `MatchClock` with ordinary `.replicate()` and migrate
`MatchState` in both roles. Bump the application protocol version/fingerprint and gameplay content
fingerprint for the catalog changes. Do not register objective input or capture messages.

The authoritative root is replicated to all accepted clients. A current/late client is ready to
present Hot Zone only when it has a matching-generation `ResolvedMapSnapshot`, `MatchState`,
`HotZoneState`, and `MatchClock` whose mode/map/match identities agree. Mismatches—including new
`MatchState` plus an old-generation clock—remain in a bounded "syncing objective" presentation state
and cannot leak stale totals or timers. In-place restart reset and both relative arrival orders are
tested; component removal/replacement on mode changes is not a production M09 behavior because the
selected mode never hot-swaps.

Client mutation of replicated state is local only and never travels as authority. Network tests must
mutate client `HotZoneState`, zone presentation entities, and HUD cache and prove the server and other
client converge back to authoritative values.

### Client HUD, arena presentation, and audio

Derive the world-space objective visual from the exact resolved area anchor. For the circular preset,
render a low-alpha floor fill, readable boundary ring, and compact status tint. Keep it behind fighters,
projectiles, and combat effects. Presentation entities carry the existing exact map-generation marker
so replacement/despawn cannot accumulate stale zones.

The existing HUD dispatches by `mode_definition_id`:

- Wipeout retains score/target text through `WipeoutState`;
- Hot Zone shows both team percentages, authoritative active-time remaining, local-team control cue,
  and EMPTY/CONTESTED/ENEMY CONTROL status without obscuring health/build/ultimate information;
- completed phase shows winner/draw, final percentages, and the existing restart prompt;
- missing/mismatched mode state shows a noninteractive syncing label rather than invented zeros.

Use distinct bounded cues/audio for control gained, control lost/contested, 50%, 90%, and completion.
Threshold cues are deduplicated per match/team locally and do not drive gameplay. Avoid per-tick sound
or text churn. Controller and keyboard require no new action mapping because capture is positional.

### Telemetry and evidence

Keep identity, participants, active duration, general combat/build/ability aggregates, disconnects,
bounded-record counts, and result in common `MatchSummary`. Add `mode_definition_id` and replace its
Wipeout-only `final_scores`/`score_margin` fields with one tagged mode summary rather than nullable
fields on every common summary:

```text
ModeSummary
  Wipeout(WipeoutSummary)
  HotZone(HotZoneSummary)

WipeoutSummary
  final_scores: [u16; 2]
  target_score: u16
  score_margin: u16

HotZoneSummary
  final_progress_ticks: [u16; 2]
  target_progress_ticks: u16
  first_entry_tick_by_team: [Option<u64>; 2]
  first_progress_tick_by_team: [Option<u64>; 2]
  controlled_ticks_by_team: [u64; 2]
  occupant_fighter_ticks_by_team: [u64; 2]
  empty_ticks: u64
  contested_ticks: u64
  control_gained_transitions_by_team: [u32; 2]
  longest_consecutive_control_ticks_by_team: [u64; 2]
  near_zone_damage_suffered_by_team: [u64; 2]
  near_zone_defeats_suffered_by_team: [u32; 2]
```

One `controlled_ticks_by_team` unit means one evaluated tick whose completed status is
`Controlled(team)`, regardless of headcount. `occupant_fighter_ticks_by_team` adds the bounded occupant
count each evaluated tick, so two teammates present for one tick contribute two. A control-gained
transition occurs when the current evaluated status becomes `Controlled(team)` and the previous
evaluated status was not `Controlled(team)`; empty and contested states break a consecutive run.
Activation starts with no previous controller, and restart clears every accumulator and prior-status
field.

Define near-zone combat as an applied hostile damage/defeat fact whose fighter target's authoritative
position at telemetry observation lies inside the objective shape expanded outward by 240 world units.
Attribute the count and applied damage amount to the target/suffering team; ignore deployable targets,
protected contacts, friendly/invalid facts, and source proximity. Record this exact definition in
reports. Continue bounded raw evidence and dropped-record counters. Telemetry observes state/facts and
never changes capture, respawn, or victory.

Extend the existing process match report with mode ID, map identity, final mode summary, restart
generation, and client convergence. The local/typical/adverse profiles must prove one unopposed team
can complete, contested time does not advance either side, and all clients receive the same result.

## Trackable implementation plan

Implement in five green vertical slices, rerunning affected role checks and the accepted M08
regression subset after each slice.

### Prerequisite and common boundary

- [ ] Close M08 feedback review, record the exact accepted M09 starting commit, and rerun the complete
  M08 automated technical gate without claiming its remaining supervised observations passed.
- [ ] Add `GameMode`/mode-neutral rules-profile configuration, CLI validation, Wipeout-compatible
  defaults, and focused configuration tests.
- [ ] Extract `AuthoritativeMatchPlugin`, `MatchLifecycleRules`, `WipeoutModePlugin`, and
  `WipeoutState` while keeping every accepted Wipeout behavior and test green. Split the existing
  combined fact/scoring/respawn system into non-draining mode scoring, common fact finalization, and
  common deferred defeat/respawn handling; add the chained pre-game deadline/outcome sets so Wipeout
  still completes before gameplay on its deadline tick, all before adding Hot Zone.

### Map and protocol foundation

- [ ] Add Hot Zone stable definitions, area-only layout requirement, full-area/terrain-clearance
  validation, preset 2, objective anchor-to-presentation-profile mapping, canonical fingerprints, and
  pure map resolver/profile-mapping tests.
- [ ] Make authoritative map startup and client reconstruction select/validate requirements from
  stable mode configuration/state rather than hardcoded Wipeout calls.
- [ ] Add `HotZoneState` and generation-tagged `MatchClock`, migrate/register mode states, bump
  protocol/content versions, and prove serialization/registration plus mismatched-arrival syncing
  before implementing progress.

### Authoritative Hot Zone vertical slice

- [ ] Implement pure normalized containment, connected-roster eligibility, complete occupancy
  snapshot, status, exactly one progress mutation on each eligible half-open active tick,
  threshold/timeout/tie/forfeit resolution, and correctly owned common/Hot Zone diagnostics.
- [ ] Integrate explicit fixed-post ordering, restart/reset/cleanup, repeated-match behavior, and
  mode-tagged summaries without adding Hot Zone branches to movement/combat/build/ability code.
- [ ] Add focused rule/App/schedule tests and deterministic multi-client authority/recovery cases.

### Client presentation and evidence

- [ ] Add exact-generation zone visuals, mode-dispatched HUD/results, syncing state, deduplicated
  threshold/control feedback, and bounded placeholder audio.
- [ ] Migrate `MatchSummary` to the fully typed `ModeSummary`, extend process reports, harness helpers,
  and `network-match.sh` mode selection; add local/typical/adverse Hot Zone evidence and preserve
  Wipeout report meaning through `WipeoutSummary`.
- [ ] Add performance/entity-growth checks plus 16:9, 16:10, 4:3, small-window, controller, and
  keyboard visual/usability checks.

### Gate review and handoff

- [ ] Run format, both role-specific Clippy/test/build graphs, server feature isolation, complete
  deterministic network/performance suites, both mode process profiles, and relevant native visuals.
- [ ] Verify the same named and custom M08 builds can complete Wipeout and Hot Zone with no
  mode-specific combat implementation or protocol authority path.
- [ ] Enter `User playtest`, collect/triage objective readability and combat-flow feedback, rerun
  affected verification, perform the learning review, and mark complete only after exit criteria.

## Test plan

### Pure validation and rule tests

- [ ] Map catalog rejects unknown/duplicate mode or anchor IDs, wrong preset count/order, wrong mode,
  absent/duplicate/point objective anchors, non-finite/zero/oversized/out-of-bounds areas, permanent-
  terrain overlap, unsupported anchors, missing/wrong objective presentation-profile mapping, unsafe
  spawns, and serialized-size/fingerprint violations.
- [ ] Circle and axis-aligned-rectangle containment cover center, interior, exact boundary, just
  outside, negative coordinates, and non-finite points. Circle boundary cases use exactly representable
  axis-aligned radius-160 coordinates; no approximate-equality assertion defines gameplay semantics.
- [ ] Occupancy covers empty, team 0 only, team 1 only, multiple same-team occupants, contested,
  simultaneous entry, boundary presence, defeated/zero-health/respawning/wrong-match/invalid-team/
  non-finite/disconnected-roster exclusions, a lingering disconnected entity, and spawn-protected
  inclusion.
- [ ] Progress covers exactly one unit per controlled tick independent of headcount, zero when empty or
  contested, cap/checked arithmetic, duplicate same-tick evaluation, threshold, timeout leader, tie,
  injected simultaneous threshold, and precedence rules. With activation `A` and limit `L`, tests
  prove exactly `L` eligible evaluations on `A..A+L`, no mutation at `A+L`, target `L` completion at
  `A+L-1`, recovered/injected threshold precedence over timeout at `A+L`, and no accepted gameplay or
  combat outcome on that boundary tick.
- [ ] Rules validate all nonzero deadlines, target progress from 2 through the active-limit/`u16`
  ceiling, capacities, retention bounds, checked deadline combinations, production defaults, and the
  exact 30-tick verification target.
- [ ] Mode-summary rules prove Wipeout score/target/margin preservation; occupant fighter-ticks versus
  controlled ticks; first-entry/first-progress values; control-gained and consecutive-run semantics;
  target-team attribution for near-zone hostile fighter damage/defeats; and reset at restart.

### Small-App/ECS and schedule tests

- [ ] Plugin composition installs exactly one mode state and one compatible resolved map; mismatched
  mode/map configuration fails before gameplay starts.
- [ ] Schedule trace proves deadline outcome consumption/locking precedes all boundary-tick gameplay;
  final authoritative movement and same-tick zero health affect eligible-tick occupancy; progress
  mutates before post-damage completion; every combat-fact reader runs before the one common clear;
  `ModeRuleOutcome` is taken exactly once by either consumer and cannot survive a second consumer run
  or next tick; deferred `Defeated` is visible before common respawn handling; its commands are applied
  before cleanup; and tick advancement is last.
- [ ] Waiting/countdown/completed ticks never advance progress; activation begins from zero; the
  chained restart prepare/mode-reset/common-commit transaction creates a new match ID and resets the
  existing mode component in place. Trace observation at every allowed boundary and after the schedule
  proves downstream systems see matching `MatchState`/`MatchClock`/mode IDs and exactly one mode state
  (never both or neither); repeated restarts do not accumulate entities/resources/data.
- [ ] Disconnect/forfeit, respawn, protection, build selection/readiness, dash crossing, sentry
  presence, and simultaneous defeat/capture follow the specified eligibility and common lifecycle
  rules. A disconnected fighter entity deliberately retained inside the zone loses occupancy through
  connected-roster membership before despawn; a sentry positioned wholly inside explicitly neither
  occupies nor contests it.
- [ ] Existing Wipeout rule, lifecycle, telemetry, HUD, and schedule tests pass after extraction with
  equivalent assertions against `WipeoutState` and `WipeoutSummary`; Hot Zone is not required for
  common fact clearing or fighter respawn.

### Deterministic network tests

- [ ] Two and four client Apps converge on mode/map/match/zone identity, occupants, status, progress,
  generation-tagged match clock, derived timer/result, and restart generation. Delayed/missing clock,
  new state plus old clock, and new clock plus old state all produce `syncing` rather than a stale or
  client-local countdown.
- [ ] Scripted positions cover unopposed control, contested hold, simultaneous entry/exit, threshold,
  timeout leader/tie, defeat in zone, respawn return, disconnect forfeit, and repeat match.
- [ ] Duplicate/stale input and packet impairment cannot advance progress more than once per server
  tick; client mutation of objective/map/HUD state cannot alter server or peer state.
- [ ] Current clients recover after delayed component arrival; active join remains rejected under M07;
  allowed reconnect/restart paths converge without capture history.
- [ ] The same preset and custom build scenarios run through both modes while fighter, movement,
  weapon, ability, passive, sentry, damage, and lifecycle behavior stays shared.

### Process, performance, visual, controller, and audio verification

- [ ] Extend the existing dedicated-server/multi-client process recipe for `wipeout` and `hot-zone`;
  run shortened local/typical/adverse profiles with exact reports and no manual state mutation.
- [ ] Run a production-rules Hot Zone session long enough to measure match duration, control/contest
  cadence, progress pacing, near-zone combat, client convergence, and clean restart.
- [ ] Compare fixed-tick cost with empty, controlled, and contested zones at supported participant
  capacity; assert bounded telemetry and no entity/resource accumulation across repeated matches.
- [ ] Inspect zone/HUD/result readability at 16:9, 16:10, 4:3, and the supported minimum window across
  waiting/countdown/empty/controlled/contested/completed/restart states.
- [ ] Verify keyboard/mouse and a physical Xbox-like controller can complete the normal selection,
  ready, fight, results, and restart flow with no objective-specific input.
- [ ] Judge control/contest/threshold audio under simultaneous combat and confirm audio pool/caps avoid
  churn. Record unavailable physical/perceptual checks as unresolved, never as passing.

### Evidence rules

- Use Bevy fixed-time advancement or explicit schedule execution; do not wait on wall-clock sleeps in
  unit/App/network tests.
- Record exact command, commit/worktree state, mode/profile, participant/build configuration, seed,
  report path, and outcome for process/performance evidence.
- Visual automation may prove presence/layout/state transitions; it cannot claim human readability,
  fun, controller feel, or perceptual audio quality.
- A skipped/unavailable check remains open or receives an explicit feedback/backlog disposition.

## Implementation and verification evidence

Not started. Populate this section only after the user validates the specification and M08 closes.

## Playtest handoff requirements

When implementation reaches `User playtest`, provide:

- exact server/client commands for Wipeout and Hot Zone, including the production-rules Hot Zone path;
- controller and keyboard controls (unchanged from M08), match setup, expected 30-second uncontested
  target, and restart procedure;
- a focused scenario that compares at least two contrasting named/custom builds under empty,
  controlled, contested, and comeback states;
- requested observations for zone boundary readability, control ownership, progress/timer clarity,
  spawn-to-objective travel, cover/crossfire quality, sentry/dash interaction, match length, and audio;
- known limitations and every supervised observation still deferred from prior milestones.

## Feedback review

Not started. Record every item as implemented now, deferred with a roadmap backlog target, rejected
with rationale, or awaiting evidence.

## Learn-from-errors review

Not started. Before closeout, record implementation/specification mistakes, causes, prevention, and
whether a recurring lesson justifies updating repository guidance or a reusable skill.

## Risks and follow-up decisions

| Risk | Mitigation / decision |
|---|---|
| M09 begins from a moving M08 review worktree | Specification may be reviewed now; implementation is explicitly gated on M08 closeout, exact commit, and green baseline. |
| Common lifecycle extraction regresses Wipeout | Extract before adding Hot Zone behavior, migrate tests to `WipeoutState`, and require behavior-equivalent Wipeout process reports. |
| `MatchState` protocol migration causes partial-state HUD bugs | Bump protocol, add the generation-tagged `MatchClock`, gate presentation on matching clock/mode/match/map generations, and test both relative arrival orders around restart. |
| Common/mode extraction drops facts or changes respawn behavior | Make combat facts read-only until one ordered common clear, move fighter respawn after deferred defeat visibility into common lifecycle, and preserve equivalent Wipeout fact/respawn tests before adding Hot Zone. |
| A disconnected fighter lingers in the objective | Use the same accepted connected-roster snapshot for occupancy and forfeit; test a retained stale fighter entity inside the zone. |
| Area anchor overlaps terrain or differs between server/client | Strengthen resolver validation and derive both authority/presentation from the same canonical resolved anchor. |
| Capture depends on ECS iteration or deferred defeat order | Collect the complete occupancy snapshot, sort/aggregate by stable team semantics, read positive health, and mutate once per tick. |
| Objective favors spawn protection or one build excessively | Keep protection semantics consistent for M09, place the zone away from spawns, capture telemetry by build, and tune only through recorded feedback. |
| Three-minute timeout plus 30-second target creates too-short/long matches | Treat values as explicit initial hypotheses; collect normal-session control/contest distributions before tuning. |
| Process clients cannot exercise a meaningful zone flow | Extend existing scripted client inputs for unopposed and contested phases; do not smuggle bots into M09. |
| Per-tick replicated progress wastes bandwidth | The state is tiny and correctness-first for four players; measure before adding throttling/delta compression. |

## Exit checklist

- [ ] Specification is validated by the user before production implementation begins.
- [ ] M08 is closed, the exact M09 starting commit is recorded, and its accepted automated gate is
  green on that baseline.
- [ ] Server configuration installs one compatible Wipeout or Hot Zone map/rule composition, defaulting
  safely to Wipeout.
- [ ] One validated area anchor drives both authoritative occupancy and exact client presentation.
- [ ] Empty, single-team, contested, simultaneous-entry, threshold, timeout, tie, forfeit, and restart
  semantics are deterministic and server-owned, with exactly `active_limit_ticks` eligible objective
  evaluations and no deadline-tick progress.
- [ ] Durable objective state converges for all current/recovery scenarios without capture history or
  client-authored results; mismatched `MatchClock`/state generations display `syncing`.
- [ ] Wipeout and Hot Zone reuse match lifecycle, fighters, movement, combat, builds, abilities,
  respawns, protection, and cleanup; no fighter/weapon/ability system contains Hot Zone victory logic.
- [ ] Objective HUD/world/audio feedback is controller-readable and does not become gameplay truth.
- [ ] Telemetry captures bounded, definition-tested occupancy fighter-ticks, control ticks/transitions,
  progress, contest, and target-team-attributed near-zone combat evidence; Wipeout reports retain their
  prior meaning through `WipeoutSummary`.
- [ ] Format, both role graphs, feature isolation, complete tests, deterministic network/performance,
  both-mode process evidence, and required visual/controller/audio checks have explicit outcomes.
- [ ] The user playtest is triaged, affected verification reruns, learning review completes, and deferred
  work is visible before M09 is marked `Complete`.
