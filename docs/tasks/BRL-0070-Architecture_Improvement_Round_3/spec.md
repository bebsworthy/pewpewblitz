# BRL-0070 technical specification — architecture improvement round 3

## Outcome

Complete the next architecture-remediation round identified by the current-source audit at `077dde6`: preserve Brawler's server-authoritative, deterministic, role-isolated design while making remaining balance policy authored and making semantic behavior families locally extensible through typed Bevy plugins, resources, components, messages, and focused transactions.

Completion means that existing content instances remain data-only, audited semantic families have real plugin-owned registration seams, authoritative coordinators are split into independently testable plan/commit stages, duplicate combat authority paths are removed, and ownership-heavy modules are decomposed without hiding schedule order.

BRL-0070 is the coordination and end-state ticket. Each independently reviewable implementation slice must be tracked by a linked BRL ticket before its code changes begin. BRL-0070 remains `doing` until every acceptance criterion is implemented here or explicitly deferred to a linked backlog ticket with rationale.

## Baseline

The current code already provides strict client/server feature isolation, one global protocol compatibility handshake, explicit authoritative fixed-tick phases, validated and fingerprinted RON catalogs, focused resolved runtime projections, staged combat effect application, and data-only addition of content instances composed from existing typed primitives.

This round does not repeat completed BRL-0061/BRL-0069 work. It addresses the remaining live-source findings:

- private/static mode, bot behavior, terminal reaction, tile behavior, and presentation-family inventories;
- retired build-point machinery already owned by BRL-0043, plus code-owned concealment timing, diagnostic loadout, formation timing, and duplicated tick conversions;
- monolithic match admission and duplicated human/Practice fighter assembly;
- map explosions directly committing combatant damage and defeat beside combat effects;
- manually closed gameplay-content fingerprint composition;
- broad projectile, client-flow, Balance Lab, lobby, and presentation coordinators where ownership differs.

BRL-0043 remains the sole owner of deleting retired point costs, budget, over-budget outcomes, and total-point reporting. BRL-0070 must not externalize or otherwise preserve that obsolete model.

## Governing constraints

1. Preserve dedicated-server authority. Clients continue to send intent only.
2. Preserve the global compatibility handshake, routed topology, stable wire identities, and process isolation.
3. Routed mode-ID migration is out of scope. Local mode registration must project to the existing wire enum; a wire migration needs a separate architecture decision and ticket.
4. Keep typed, bounded schemas. Do not use opaque `Any`, untyped payload maps, stringly typed effect blobs, or executable data.
5. Preserve deterministic ordering, stable IDs, bounded collections, capacity rejection, event-ID reservation, fixed-tick phases, `ApplyDeferred` boundaries, restart transactions, telemetry, and evidence.
6. Keep authoritative mutation atomic. Planning helpers may be pure, but health, defeat, map terminal state, facts, and cues commit in one explicitly ordered transaction.
7. Prefer Bevy plugins, resources, components, messages, observers, schedules, `QueryData`, and stable-ID function registries over trait-object or dependency-injection frameworks.
8. Keep plugin selection and schedule ordering visible in `mod.rs` and role roots; move owned implementation behind them.
9. Preserve public module paths and wire contracts unless a child-ticket decision explicitly approves a compatible change.
10. Preserve player-visible values when moving them into data. A schema/fingerprint bump changes the source of truth, not balance.
11. Engine ceilings, packet/serialization bounds, protocol versions, stable identity formats, and overflow policy remain code-owned.
12. Use synthetic registration tests instead of adding speculative gameplay families.
13. Split files only for ownership, role, phase, lifecycle, or independent testability—not line count.
14. Preserve unrelated user worktree changes.

## Target architecture

```text
authored RON/operator configuration
        |
        v
content plugins register validated definitions + fingerprint contributions
        |
        v
resolved immutable definitions/capabilities
        +--> behavior plugins register handlers/producers by stable ID
        |
        v
focused ECS runtime components/resources
        |
        v
authoritative phase coordinator
  collect -> validate -> plan -> commit -> publish
        |
        v
facts/messages/replication -> presentation requests -> renderer/audio registries
```

Open/Closed has two levels:

- **Authored instances:** a new weapon, map, object, bot policy, or presentation profile using existing mechanics is data-only.
- **Semantic families:** a new supported behavior family needs one typed schema addition plus a focused plugin/handler and owned projections; unrelated coordinators do not gain another built-in-ID branch.

Wire schemas remain deliberately closed and versioned.

## Implementation plan

### Stage 1 — residual policy data and shared timing

1. Keep retired build-point removal in linked BRL-0043 and ensure BRL-0070 adds no new budget dependency.
2. Add validated, fingerprinted concealment rules for attack- and damage-reveal ticks; preserve values `90` and `120` and make authority consume the resource.
3. Replace hardcoded direct-diagnostic fighter/weapon/ultimate/passive selection with a validated authored starter/diagnostic recipe independent of catalog ordering.
4. Introduce or reuse tick conversion helpers based on `SIMULATION_TICK_HZ`; remove audited literal `60` conversions in Balance Lab/HUD/flow paths.
5. Consolidate lobby match-formation deadlines in one validated/operator-owned `MatchFormationTiming` source so advertised and enforced values cannot drift.

### Stage 2 — plugin-populated behavior registries and semantic projections

1. Promote `BotBehaviorRegistry` to a Bevy resource populated by behavior plugins or an `App` extension. Registrations own stable ID, contributor function, deterministic metadata, and fallback capability.
2. Validate `bots.ron` policy against the finalized registry rather than a duplicate static ID array. Reject duplicates, missing policy/handler, excessive count, and absent fallback.
3. Add a synthetic behavior test proving another registration participates without editing the reducer.
4. Replace bot object asset-ID recognition with semantic attackable/objective/valuable/defending-team projections. Mode plugins publish a bounded mode-neutral `BotObjectiveView`.
5. Make `TerminalReactionRegistry` crate-visible and plugin-populated. Authored objects resolve to a stable reaction ID and typed parameters/profile. Explosion and pickup plugins own registration.
6. Narrow terminal extension handlers to typed plans or an explicit bounded context. Arbitrary `&mut World` remains only at the transaction commit boundary.
7. Test duplicate, missing, capacity, deterministic-order, fallback, and synthetic registrations.

### Stage 3 — admission planning and authoritative fighter assembly

1. Characterize rejection precedence, routed/direct admission, identifier allocation, team/spawn selection, idempotence, diagnostics, and replication ownership.
2. Extract `validate_match_hello`, `resolve_join_loadout`, and `plan_match_join`, returning `FighterJoinPlan` or `MatchJoinRejection`.
3. Introduce `AuthoritativeFighterSpawnSpec` and one `spawn_authoritative_fighter` path shared by human and Practice creation where component contracts overlap.
4. Leave `process_client_hellos` responsible only for deterministic message ordering, plan invocation, commit, outcome send, and session transition.
5. Preserve fail-closed admission, allocation order, clearance, physics, replication/interpolation, loadout projection, and map/match membership.

### Stage 4 — one combatant damage authority path

1. Characterize weapon/environment damage for fighters and deployables, including lineage, credit, friendly/self policy, protection, modifiers, defeat cleanup, facts, cues, telemetry, and ordering.
2. Keep map authority responsible for object health, terminal state, chain-reaction targeting, and map transitions.
3. Route explosion combatant damage through typed environment payload/damage plans consumed by combat effects, or one shared combat-owned `CombatDamageCommitPlan`.
4. Remove direct map-owned combatant health, defeat, collision, active-effect, combat-fact, and combat-cue mutation.
5. Build bounded identity/entity snapshots once per batch instead of repeated full-world scans.

### Stage 5 — local mode, tile, content, and presentation seams

1. Add a startup-built local `ModeRegistry` populated by mode plugins. Registrations own local ID mapping, topology, rule validation/resolution, map compatibility, installer, advertised summary, bot objective, and HUD projections as applicable.
2. Preserve routed/wire `GameMode`; central wire conversion may remain exhaustive, but local consumers use registry projections rather than duplicate mode switches.
3. Resolve effect tiles into focused movement multiplier, periodic damage, healing block, traversal cost, and presentation capabilities. Consumers query capabilities rather than concrete tile variants.
4. Add a sorted `GameplayContentFingerprintRegistry` of stable domain ID, schema version, and canonical material. Reject duplicate IDs and missing required domains.
5. Introduce stable-key `VfxRequest` and `AudioRequest`; feature presentation plugins translate domain facts while renderer/audio plugins resolve profiles and renderer keys.
6. Add synthetic local mode, tile, fingerprint, and presentation registrations proving reuse without a new central built-in branch.

### Stage 6 — coordinator and ownership decomposition

1. Refactor projectile execution around a per-tick `DeliveryWorldSnapshot` and focused straight/sticky, lob/splash, and melee planners. Retain deterministic coordination and commit/publication; do not add a trait per weapon.
2. Split client-flow ownership into connection/shell, brawler/profile, equipment, matchmaking/Practice, and results/navigation reducers. Retain one small precedence and `FlowCommit` coordinator.
3. Co-locate Balance Lab tuning metadata with typed schema families using small descriptors or a narrow macro, not general reflection.
4. Split Balance Lab apply into prepare/validate, persist, authoritative commit, and restart publication while preserving fail-closed rollback.
5. Split lobby, 3D presentation, and brawler-screen implementations only along demonstrated ownership boundaries.
6. Use named `QueryData` views and focused snapshots for stable broad query shapes; do not create mega-components.

## Verification

Per child ticket:

- Add characterization or contract tests before authoritative changes.
- Run focused domain tests under applicable feature sets.
- Run `cargo fmt --all` and `git diff --check`.
- Run `just check` after plugin, feature, module, or import changes.
- Run catalog/fingerprint and Balance Lab tests after schema/data changes.
- Run separate-App/network tests after admission, combat, map, mode, or replication changes.
- Record exact commands, results, unavailable evidence, and feedback disposition in the child ticket and BRL-0070.

Final gates:

- `just check`
- `just test`
- `just lint`
- `just ci`
- `git diff --check`

Native evidence is required for player-visible presentation, audio, HUD, tile-feedback, timing, or control changes. Exercise affected normal/reduced presentation, relevant delivery families, all modes, and Practice behaviors. Record every observation as accepted, corrected, deferred, rejected with rationale, or awaiting evidence.

## Acceptance criteria

### Data and balance

- Retired point machinery is removed through BRL-0043; BRL-0070 adds no new dependency on it.
- Concealment timing, diagnostic loadout, formation deadlines, and audited tick conversions have one validated authored or canonical source.
- Player-visible values remain unchanged unless separately approved.
- Schema/fingerprint changes reject stale or invalid content deterministically.
- Engine/protocol safety limits remain code-owned and are not mislabeled as balance.

### ECS and authority

- Client/server role isolation and complete composition remain intact.
- Admission planning is independently testable and `process_client_hellos` no longer owns loadout, spawn, and fighter-bundle algorithms inline.
- Human and Practice fighters share invariant authoritative assembly where their contracts match.
- Map explosions no longer implement combatant damage/defeat beside combat effects.
- Facts, cues, telemetry, replication, capacity behavior, and fixed-tick order remain deterministic and bounded.

### Extensibility

- Bot behavior and terminal reaction inventories are plugin-populated resources with duplicate, coverage, fallback, and capacity validation.
- AI consumes semantic object/mode capabilities rather than exact reusable asset IDs.
- Local mode gameplay/presentation installation is registry-owned while wire IDs remain compatible.
- Effect-tile consumers query resolved capabilities rather than exhaustively matching authored variants.
- Gameplay fingerprinting is contributor-based and cannot silently omit an installed catalog.
- Presentation producers request stable VFX/audio profiles without knowing renderer/material families.
- Synthetic extension tests prove each supported seam accepts an additional registration without a central built-in branch.

### Organization and closeout

- Composition roots retain visible plugin and schedule relationships.
- Large implementations split only across documented ownership/lifecycle boundaries.
- Public paths and wire contracts remain compatible unless separately approved.
- Focused and final automated gates pass.
- Required native evidence and feedback disposition are recorded.
- Durable documentation describes resulting data and extension contracts.
- A learn-from-errors review records regressions, causes, prevention, and reusable lessons.
- Deferred audit items have linked backlog tickets and rationale.
- `ticket sync` completes without conflict before `done`.

## Non-goals

- Dynamic executable mod ABI or runtime-loaded Rust plugins.
- Opaque payload schemas.
- Routed mode protocol migration.
- Per-message compatibility versions or decoders.
- General dependency-injection, service-locator, DDD, or hexagonal framework.
- Speculative content solely to exercise abstractions.
- Balance changes beyond source-of-truth migration.
- File splits driven only by line count.

## Delivery sequence

1. Residual policy sources and timing, excluding budget machinery owned by BRL-0043.
2. Bot behavior registry and semantic AI projections.
3. Terminal reaction registration.
4. Admission/spawn decomposition.
5. Environment damage unification.
6. Local registries/capability projections.
7. Coordinator/module decomposition.
8. Full gates, native evidence where required, documentation, learning review, and sync.

Create and link only the next independently reviewable child ticket when its implementation is ready to begin.


## Progress record — 2026-08-30

- Linked BRL-0071 completed the first implementation slice: concealment attack/damage reveal timing is validated, fingerprinted gameplay data with preserved 90/120 values; focused tests and `just check` pass.
- Planning found that BRL-0043 already owns removal of the retired build-point model. BRL-0070 was corrected not to externalize or preserve that obsolete budget and is linked to BRL-0043.


## Implementation progress — BRL-0072 complete

The Practice bot behavior extension seam is implemented and closed under linked ticket BRL-0072. The static complete behavior slice and duplicate registered-ID inventory were replaced by a bounded server-only registry populated by plugins and sealed during Bevy plugin finalization. Shared `BotCatalog` validation/fingerprinting is now handler-independent through `BotCatalogResource`; exact policy/handler coverage remains a fail-closed server invariant. A synthetic eighth behavior proves extension through one plugin registration plus one authored policy entry without arbiter changes. Focused bot/content/protocol tests, role-specific checks, formatting, diff validation, and `just check` pass.


BRL-0072's final organization keeps registry build/finalization ownership in `src/bots/registry.rs` and behavior algorithms/arbitration in `src/bots/behaviors.rs`; verification was rerun after that extraction.


## Implementation progress — BRL-0073 complete

Linked ticket BRL-0073 moved direct-diagnostic fallback loadout ownership from server admission literals into one validated `BuildCatalog` policy authored in `content/catalogs/builds.ron`. The stable ordered weapon rotation is now data-owned and resolved through the ordinary saved-brawler path by a focused server API. Build catalog schema 17 and the global gameplay-content fingerprint cover the policy, while unchanged player recipe fingerprints remain byte-compatible. Focused build/content/network tests and `just check` pass; no native evidence was required because accepted values and behavior are unchanged.


## Implementation progress — BRL-0074 complete

Linked ticket BRL-0074 completed Stage 1 residual timing ownership. Shared simulation conversion APIs now drive Balance Lab and game-selection presentation without literal 60 Hz assumptions. Operator catalog schema 5 owns validated 30-second loading and 10-second route-grant formation timing; one resolved value drives Practice/product advertisement and server enforcement while public protocol shapes and the public game-type catalog revision remain unchanged. Focused timing/catalog/Practice/client/Balance Lab tests and `just check` pass; no native evidence was required because accepted timing and copy are unchanged.


## Implementation progress — BRL-0075 complete

Linked ticket BRL-0075 completed the terminal-reaction portion of Stage 2. Shared authored terminal behaviors now project stable reaction IDs; a crate-visible, plugin-populated registry seals deterministic bounded registrations against authored map content. Explosion and restoration-pickup plugins own registration, and handlers receive a bounded transaction context rather than arbitrary `World` access. Synthetic registration, all existing barrel/chest runtime behavior, client/server role graphs, `just check`, and `just lint` pass. The lint gate also prompted a focused cleanup of the prior diagnostic-policy test and bounded fractional timing helper.


## Stage 2 progress — semantic AI projections complete

- `BRL-0076` moved Practice AI from concrete barrel/chest IDs and mode-specific state inspection to terminal-plugin object semantics, defending-team objective components, and mode-plugin-owned `BotObjectiveView` projections.
- Real-schedule Practice workers for Wipeout, Hot Zone, and Heist preserve directed input; role, authority, network, and performance gates are green.

## Round 3 progress (2026-08-31)

BRL-0077 completed the admission/fighter-assembly phase: match hello validation and planning are pure, shared fighter construction is controller-neutral, connected and Practice ownership remain explicit, and canonical verification passed. One timing-sensitive performance sample was transient; all 12 isolated performance gates passed immediately afterward.

## BRL-0078 phase completion

Completed the environment-combat authority phase. Barrel explosion combatant damage now flows through one combat-owned atomic transaction shared with damage tiles; map code owns only deterministic selection, LOS, world-object chaining/reactions, and map telemetry. The previously inert authored chain limit is effective. Focused, role, lint, canonical, full network, and performance verification passed. Child BRL-0078 is complete; the next planned slice is focused effect-tile capability projection.

## BRL-0079 phase completion

Completed the focused effect-tile capability projection phase. Authored Speed, Slow, Damage, and None remain the closed catalog/editor vocabulary, while production map, movement, bot, recovery predicate, and presentation paths consume orthogonal resolved capabilities. A composite-capability test and two-client authoritative damage-tile scenario prove the extension seam without schema or protocol changes. All focused, role, lint, canonical, network, and performance gates passed after a package-scoped Cargo artifact cleanup resolved host disk exhaustion. Child BRL-0079 is complete; plugin-populated tile registration or the larger ModeRegistry lifecycle remains the next Stage 5 decision.

## BRL-0080 phase completion

Completed the contributory gameplay-content compatibility phase. Seven shared content domains now register stable IDs, domain-owned schema versions, and read-only live-World canonical material through their owning headless-safe plugins; deterministic sorted envelope 27 framing rejects duplicate, missing, excessive, invalid, or unavailable contributions. Routing identity, worker validation, and Startup installation use the same evaluator, closing the weapon-part/combat-condition embedded-reload gap while preserving the one global handshake and Balance Lab exclusion. Focused, role, lint, canonical, all 97 network, and all 12 performance gates passed. Child BRL-0080 is complete; the local ModeRegistry lifecycle is the next justified Stage 5 slice, while a tile-handler registry remains deferred until a second active tile mechanic exists.

## BRL-0081 phase completion

Completed the process-local mode registration and installation lifecycle. Wipeout, Hot Zone, and Heist now own registration metadata, operator-rule resolution, and authoritative installation; a bounded Bevy registry seals deterministically during plugin finalization, while pure pre-App consumers use the same mode-owned constants. Synthetic local registration proves extension without inventing routed identity, and the stable protocol/routing surface remains intentionally closed. Exact catalog digest, rejection precedence, map topology, role isolation, all focused suites, `just check`, `just lint`, all 97 network cases, and all 12 performance gates passed. Child BRL-0081 is ready to close; coordinator/module decomposition is the next delivery-sequence phase.

## Phase completion: BRL-0082 VFX request boundary

BRL-0082 completed the next architecture-improvement phase. Client gameplay/domain systems now emit registered stable-key VFX requests, while the generic renderer exclusively owns authored profile resolution and concrete Bevy materialization. The closed `VfxCueFamily` inventory was removed, exact presentation values and authority/protocol boundaries were preserved, final automated/native evidence passed, and independent review corrections were incorporated.

The next planned child is the separate audio request/playback boundary. Audio will reuse the proven bounded registration and feature-owned producer pattern but remain a sibling implementation because its occurrence deduplication, asset readiness, playback reservations, speed/volume policy, and one-shot lifecycle are not VFX renderer concerns.
