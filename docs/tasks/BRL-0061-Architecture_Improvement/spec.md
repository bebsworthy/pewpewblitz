# BRL-0061 architecture remediation specification

## Outcome

Make Brawler's current architecture truthfully data-driven and locally extensible without changing gameplay semantics, weakening server authority, or replacing typed Bevy/Rust contracts with a general framework.

Completion means:

- production has one authored source for fighter, weapon, match, bot, and presentation tuning;
- authoritative systems consume focused runtime components rather than legacy defaults or deep loadout traversal;
- application composition owns gameplay while transport and protocol plugins own only their boundaries;
- delivery, effect, behavior, reaction, and presentation extension points have explicit plugin-owned registration seams;
- large modules are split only where ownership differs, while fixed-tick order and transaction boundaries remain visible;
- current automated, routed, performance, role-isolation, and required native evidence pass.

BRL-0061 is the coordination and end-state ticket for the remediation program. Each stage below is independently reviewable and must be created as a linked BRL implementation ticket before code changes begin. BRL-0061 remains open until every acceptance criterion is implemented here or explicitly deferred to a linked backlog ticket with rationale.

## Baseline and rationale

The audit at commit `97d9cc1` found a strong server-authoritative ECS foundation and confirmed that earlier BRL-0051/BRL-0052 issues such as exact catalog cardinality locks, effect-tile value loss, cue-driven attack authority, and several large coordinators were already corrected.

The highest current risk is a live dual balance path:

- `content/catalogs/builds.ron` and `content/catalogs/weapons.ron` own the canonical resolved loadout;
- `FighterDefinitions::default` and `WeaponDefinitions::default` remain initialized and consumed by production admission, spawn, restart, Practice, pickup, HUD, verification, and Balance Lab paths;
- the code defaults materially disagree with the RON values.

The remaining architecture work concerns dependency direction, focused ECS projections, family-specific transaction decomposition, plugin-populated registries, and residual code-owned policy.

## Governing constraints

- Preserve dedicated-server authority. Clients continue to send intent only; they do not author pose, hits, damage, status, score, map mutation, or match lifecycle.
- Preserve the current global protocol compatibility handshake, stable wire identities, routed topology, and process isolation. Do not introduce per-message versions or untyped compatibility decoders.
- Keep typed serialized enums for bounded wire/content schemas. Open/Closed compliance means localizing a new mechanic to a schema addition plus a plugin/handler registration; it does not mean accepting arbitrary dynamic RON payloads.
- Preserve fixed-tick ordering, deterministic iteration and tie-breaking, ID reservation, `ApplyDeferred` boundaries, physics refresh, restart transactions, outcome facts, cues, telemetry, and evidence.
- Preserve server feature isolation from rendering, windowing, audio, device input, and client assets.
- Keep `mod.rs` and role application builders as composition surfaces. Move owned implementation behind them; do not hide schedule relationships inside a generic service container.
- Prefer Bevy components, resources, messages, observers, schedules, and plugins. Function-pointer registries and pure planning helpers are acceptable; trait objects, command buses, reducers, or dependency-injection frameworks require a demonstrated need.
- Do not add a new weapon, mode, ability, tile, object, VFX family, audio cue, or bot behavior merely to prove extensibility. Use synthetic registration tests where a second implementation is required.
- Do not use line-count reduction as an acceptance target. Split only where state ownership, execution role, schedule phase, or independently testable policy differs.
- Preserve current player-visible balance unless a value is already contradicted by the live canonical catalog. Migration must make the RON/runtime result authoritative, not select a new tuning value.
- No required native or subjective verification may be replaced by automated tests.

## Target architecture

### Content and runtime ownership

```text
authored RON/assets
        |
        v
GameplayContentPlugin
  parse -> validate -> cross-catalog coverage -> fingerprint
        |
        v
resolved match/map/loadout definitions
        |
        v
spawn/build-commit projection
  FighterRuntimeStats / FighterBody / WeaponRuntimeSpec
  CombatDefense / DamageModifiers / ConditionResistances
        |
        v
authoritative fixed-tick systems -> committed facts/cues/telemetry
```

Authored definitions remain immutable. Runtime projection components are immutable for an active generation and are rebuilt only when the waiting-phase build transaction commits or a new fighter generation is spawned. Mutable health, ammunition, cooldowns, effects, pose, and match state remain separate components/resources.

`ResolvedMatchLoadout` may remain attached for identity, diagnostics, or presentation, but authoritative systems should depend on the smallest capability component they consume rather than traversing the aggregate.

### Role and plugin composition

The role application builder owns plugin selection:

```text
ServerAppPlugin
  GameplayContentPlugin
  ServerAuthoritativeGameplayPlugin
    movement / combat / abilities / concealment / map / match modes / bots
  ServerSessionTransportPlugin
  RoutedWorkerPlugin
  DiagnosticsPlugin

ClientAppPlugin
  GameplayContentPlugin
  ClientReplicatedGameplayPlugin
  ClientSessionTransportPlugin
  ClientPresentationPlugin
```

`ProtocolPlugin` registers messages, components, interpolation, and directions only. Session/transport plugins own links, admission envelopes, connection lifecycle, transport errors, and replication setup only. They do not install gameplay or presentation plugins.

## Technical work plan

### Stage 1 — one authoritative gameplay-data path

1. Add characterization tests that exercise current product admission, match activation, respawn/restart, Practice dummy/bot spawn, restoration pickup, client HUD fallback, Balance Lab, and verification paths with resolved loadouts. Record every production use of `FighterDefinitions`, `WeaponDefinitions`, `STANDARD_FIGHTER_DEFINITION`, and `PULSE_SIDEARM_DEFINITION`.
2. Define the smallest canonical authored home for the remaining standard fighter body geometry. Prefer extending the existing validated build/fighter profile schema over creating another catalog. Spawn facing comes from the selected map spawn. Lifecycle timing belongs to match lifecycle policy. Weapon damage/economy/delivery comes only from `WeaponCatalog`.
3. Introduce focused immutable runtime projections at build commitment/spawn:
   - `FighterRuntimeStats`: maximum health, recovery values, movement speed, and other resolved fighter gameplay values;
   - `FighterBody`: validated body radius or other invariant collision geometry;
   - `WeaponRuntimeSpec`: resolved primary weapon identity, economy, delivery, and payload reference/value needed by authority;
   - narrower combat modifier/resistance components where Stage 4 demonstrates a consumer.
4. Remove production fallback selection from `fighter_runtime_values`, match activation/restart/respawn, pickups, admission, Practice, HUD, verification, and Balance Lab. A product fighter without a resolved runtime projection fails admission/activation with a bounded actionable error; it never silently receives code defaults.
5. Remove `FighterDefinitions` and `WeaponDefinitions` as production resources. Retain stable identity newtypes only where they are part of a current wire/runtime contract. Confine any transitional fixture to test-only code and name it as a fixture, not a definition catalog.
6. Split `MovementTuning` into engine/process policy and entity gameplay values. Iteration limits, collision skin, and input freshness policy may remain bounded code/config resources; speed, body geometry, and spawn facing come from resolved entity/map state.
7. Add cross-catalog startup validation proving that every selectable profile/base/preset resolves exactly once and that presentation/AI references used by the resolved loadout exist.

Stage 1 must land before any broader abstraction work because later stages must not preserve or wrap the conflicting fallback path.

### Stage 2 — move residual tuning into validated policy data

1. Extend the operator game-type catalog with a validated common lifecycle policy for spawn protection and completed-match input lock. Keep capacity ceilings and retained diagnostic buffer bounds code-owned.
2. Move Wipeout recent-hostile credit, Heist critical-health feedback threshold, and any player-affecting Hot Zone proximity/timing policy into the owning mode rules. Use basis points or bounded integer ticks where deterministic representation is preferable.
3. Extend `bots.ron` with behavior arbitration entries keyed by stable behavior ID, including base score, commitment bonus or policy, and enablement. Validate unique IDs, bounded score arithmetic, exact registered-behavior coverage, and deterministic tie-breaking.
4. Put lob minimum-flight policy in the validated weapon recipe policy or the delivery definition, rather than a hidden attack-system literal.
5. Remove duplicate effect-tile tuning constants that are not the runtime source. Keep only schema bounds and stable asset/type IDs in code.
6. Extend VFX/audio presentation profiles so configurable scale, anchor offset, lifetime policy, and asset path/key live in validated manifests. Either consume and validate weapon `presentation_profile_id` end to end or remove it from the schema in an intentional compatibility change.
7. Add round-trip, invalid-bound, missing-reference, duplicate-ID, and runtime-observation tests for every new authored field. Bump the owning catalog schema/revision and update durable documentation.

### Stage 3 — correct plugin dependency direction

1. Extract `GameplayContentPlugin` from `ProtocolPlugin`. It owns build, weapon-part, map, weapon, bot, audio/VFX where role-appropriate, validation, and content fingerprint initialization.
2. Add `ServerAuthoritativeGameplayPlugin` and `ClientReplicatedGameplayPlugin` composition roots. Move gameplay plugin installation out of `ServerNetworkPlugin` and `ClientNetworkPlugin`.
3. Keep `RoutedWorkerPlugin` at server application/process composition unless a concrete transport-only responsibility justifies placement elsewhere.
4. Add plugin-composition tests proving:
   - protocol registration works without installing gameplay systems;
   - transport/session plugins do not install combat, map, ability, profile, or presentation systems;
   - the complete server and client app builders install each required plugin exactly once;
   - headless/server builds remain free of client dependencies.
5. Preserve current schedule sets and ordering. This stage is an ownership move, not a schedule redesign.

### Stage 4 — focused ECS dependencies and authoritative transactions

1. Replace deep `ResolvedMatchLoadout` traversal in movement, combat target state, condition resistance, passive modifiers, and bot observation with the Stage 1 runtime projections. Do not duplicate mutable state; projections are immutable resolved inputs.
2. Introduce named `#[derive(QueryData)]` views for large stable query shapes where doing so clarifies ownership. Do not hide optional dependencies or schedule effects inside opaque `SystemParam` wrappers.
3. Extract one pure ultimate activation gate for freshness, latch, active/defeated state, charge, and generation reservation. Each ability plugin continues to own target validation, runtime components, execution, rejection context, and ability-specific telemetry.
4. Refactor attack delivery into family-owned planning/commit modules:
   - straight and sticky-straight;
   - lobbed and splash;
   - melee arc;
   - cone spray.
   The schedule-facing coordinator retains admission result, deterministic family order, bounded ID allocation, command publication, facts, cues, and telemetry.
5. Refactor effect application into `plan -> commit -> project`:
   - planning performs recipient/gating/modifier calculations without mutation;
   - commit performs one ordered authoritative mutation transaction;
   - projection emits facts, cues, telemetry, and evidence from the committed result.
   Do not split atomic mutations into unordered systems.
6. Add before/after characterization for delivery ordering, collision outcomes, sticky/lob/splash limits, protected contacts, passive modifiers, defeat precedence, healing, conditions, telemetry, and replicated cues.

### Stage 5 — real extension registries

Introduce registries only for demonstrated audited seams, using stable IDs and duplicate/coverage validation.

1. `BotBehaviorRegistry` is a resource populated by behavior plugins. A registration owns its contributor function and metadata; authored arbitration policy remains in `bots.ron`.
2. `TerminalReactionRegistry` becomes crate-visible and plugin-populated. Authored object data selects a reaction ID/profile; explosion and pickup plugins own validation, planning, commit, and presentation facts for their reactions.
3. Effect-tile resolution installs composable runtime components such as movement-multiplier or periodic-damage occupancy. Runtime systems and presentation consume resolved behavior/presentation tags rather than exact asset IDs.
4. Local `ModeRegistry` registration is plugin-owned and carries installer, topology policy, rule parser/validator, bot projection, and advertised summary projection. Keep current wire mode identity in this ticket; any stable numeric-mode protocol migration requires a separate architecture decision and ticket.
5. VFX renderer and audio asset registries use stable manifest keys and map-backed handles. Semantic producer plugins register cue/profile mappings; renderer plugins own renderer-specific transforms.
6. Ability definition registration may be introduced only after the activation-gate and consumer fan-out work demonstrates the minimal fields needed. Do not replace typed `UltimateParameters` with untyped data.

Synthetic extension tests must prove a fourth local registration can reuse an existing supported topology/renderer/reaction/behavior without another built-in-ID branch in the central consumer.

### Stage 6 — ownership-based module decomposition

Perform file moves after behavior and extension boundaries are characterized.

1. Split `server/lobby/mod.rs` into admission/profile authority, queue coordination, match formation, activation/grants, and composition while preserving its visible schedule.
2. Split `client/presentation_3d/mod.rs` into presentation foundation/assets, static map, dynamic objects/pickups/objective, fighter/animation, and projected UI.
3. Split `client/flow/screens/brawlers.rs` by list/detail, create/edit, equipment, deletion, and preview ownership.
4. Separate Balance Lab snapshot/apply/shape validation and schema-field generation by owning catalog family where this removes duplicated schema knowledge.
5. Move server admission/session implementations behind `server/mod.rs`; keep application and schedule composition in the root.
6. Re-evaluate `resolve_flow_action`, `sample_local_input`, and `process_client_hellos` after the ownership moves. Extract only independently testable decision helpers or device/domain systems; do not chase a target line count.

## Verification strategy

### Per-stage verification

- Add failing characterization or contract tests before changing authoritative behavior.
- Run the focused library tests for every touched domain and both applicable role feature sets.
- Run `cargo fmt --all` and `git diff --check` at each checkpoint.
- After plugin or import moves, run `just check` immediately to catch client/server feature leakage.
- After catalog/schema changes, run catalog validation, fingerprint, Balance Lab persistence/apply, profile migration, admission, and recovery tests.
- After authoritative transaction changes, run separate-App/network scenarios covering the affected delivery/effect/match behavior.

### Final automated gates

- `just check`
- `just test`
- `just lint`
- `just ci`
- `git diff --check`

The final evidence must identify the exact implementation commit/state and include role-specific checks, routed 1v1/2v2/3v3 and Practice coverage already owned by `just ci`, deterministic/performance gates, and dedicated-server isolation.

### Native and subjective verification

Native evidence is required for stages that change presentation profiles, VFX/audio asset resolution, effect-tile presentation, HUD fallback behavior, or player-visible timing. Exercise at least:

- one straight, lobbed, splash, sticky, melee, and cone weapon;
- one damage, healing, condition, and knockback payload;
- Wipeout, Hot Zone, and Heist;
- normal and reduced-effects presentation;
- Practice bots using healing, pickup, objective, retreat, and pressure behaviors;
- audible cue confirmation if audio registry/loading changes.

Record every observation as accepted, corrected, deferred to a linked ticket, rejected with rationale, or awaiting evidence.

## Acceptance criteria

### Canonical data and runtime state

- No production path initializes or reads code-authored fighter/weapon balance defaults.
- Match admission, activation, restart, respawn, Practice, pickups, HUD, Balance Lab, and verification use the same validated resolved catalog values.
- Missing resolved runtime data fails closed with an actionable bounded error; no silent fallback exists.
- Movement speed, body geometry, health, ammunition, weapon delivery, and payload values have one documented authored source.
- Cross-catalog validation rejects missing, duplicate, out-of-range, or unconsumed gameplay/presentation/AI references.

### ECS and dependency direction

- Protocol and transport/session plugins do not install gameplay, content, or presentation plugins.
- Complete role application builders install all required plugins exactly once, with unchanged fixed-tick and deferred ordering.
- Server-only builds remain free of rendering, windowing, audio, device input, and client assets.
- Authoritative combat/movement systems consume focused immutable runtime capability components instead of traversing unrelated loadout fields.
- Mutable runtime state remains separate from authored/resolved definitions.

### Extensibility and transactions

- Existing weapon/map/build content remains addable through data when it uses existing mechanics.
- Delivery and effect coordinators retain deterministic orchestration and publication but do not contain every family-specific algorithm.
- Effect mutation remains one ordered authoritative transaction, with facts/cues/telemetry projected from committed outcomes.
- Common ultimate activation policy has one tested implementation; ability-specific behavior remains in focused plugins.
- Bot behavior, terminal reaction, and applicable presentation/mode registrations are plugin-populated resources with duplicate and coverage validation.
- AI and presentation consume authored capabilities/tags rather than exact map asset identities where semantics are intended to be reusable.
- Synthetic registrations demonstrate reuse of an existing mechanic without adding another built-in-ID branch to its central consumer.

### Organization and evidence

- Large files are split along documented ownership boundaries without obscuring schedule composition.
- Public module paths and wire contracts are preserved unless a separately recorded architecture decision explicitly approves a change.
- All focused and final automated gates pass.
- Required native evidence and feedback disposition are recorded.
- Durable product/architecture documents describe the resulting source of truth and extension contracts.
- A learn-from-errors review records regressions, causes, prevention, and reusable lessons.
- `ticket sync` completes without conflict before closeout.

## Non-goals

- Designing a fully dynamic mod/plugin ABI.
- Loading executable gameplay code from data.
- Replacing typed content enums with stringly typed effect blobs.
- Rewriting the routed protocol or adding per-message compatibility layers.
- Introducing a general DDD, hexagonal, service-locator, or dependency-injection architecture.
- Adding speculative behavior families or abstractions without a current audited consumer.
- Changing accepted player balance beyond resolving the existing canonical-data conflict.
- Splitting cohesive files solely to satisfy a line-count or Clippy threshold.

## Delivery and closeout

Before implementation begins, create one linked BRL ticket per stage or smaller independently reviewable vertical slice. Each child ticket must restate its concrete outcome, affected contracts, focused acceptance criteria, verification, and native requirements. Move BRL-0061 to `doing` when the first child implementation begins; keep it open while any required child, feedback, documentation, or evidence remains.

Every audit finding must end as implemented, explicitly deferred to a linked backlog ticket, or rejected with evidence. Close BRL-0061 only after all acceptance criteria are satisfied, the child tickets are reconciled, the learning review is recorded, and `ticket sync` succeeds.

## Remediation progress — 2026-08-30

- Stage 1 is implemented and verified under linked ticket BRL-0062.
- The live code-authored fighter/weapon balance catalogs and fallbacks are removed.
- Catalog-authored fighter stats/body and resolved weapon data now project atomically into focused generation-local components used by authority and presentation.
- Movement engine policy no longer owns player speed, body radius, or spawn facing.
- Build/content schemas were intentionally advanced without changing the existing replicated loadout or saved-profile wire shapes.
- BRL-0062 passed formatting, checks, Clippy/role isolation, all role and routed tests, performance gates, routed product 1v1/2v2/3v3, all nine Practice combinations, and paired native Metal render evidence.
- Stages 2–6 remain in BRL-0061 scope and must continue through separately linked implementation tickets. BRL-0061 remains `doing`.

## Remediation progress — Stage 2 implementation, 2026-08-30

- Stage 2 implementation is complete under linked ticket BRL-0063. Match lifecycle/mode, bot arbitration, lob delivery, VFX, and audio policies now have validated authored owners; duplicate effect-tile defaults and the dead weapon presentation-profile chain are removed.
- Values cross the routed boundary explicitly, authoritative consumers have non-default runtime proofs, client presentation catalogs have strict complete coverage and arithmetic safety, and compatibility/fingerprint versions were advanced where shared/authored shapes changed.
- Final automated verification is green: formatting, checks, all role-specific lint/isolation gates, 488 client tests, 428 server tests, 450 Balance Lab tests, 94 network scenarios, 12 performance gates, routed product 1v1/2v2/3v3, and all nine Practice combinations.
- Native default and reduced-effects release-client reports pass. Reduced-effects Practice evidence records live combat (`effect_high_water=3`, `projectile_high_water=2`) with `reduced_effects=true`.
- BRL-0063 remains `doing` only for human confirmation that the changed manifest-backed ready/fire/impact cues are audible. No automated or native visual defect remains open.
- After that confirmation, Stage 2 can close. Stages 3–6 remain: plugin dependency direction, focused ECS dependencies/transactions, extension registries, and ownership-based module decomposition.

## Remediation progress — Stage 2 accepted, 2026-08-30

- The user confirmed the reduced-effects Practice run produced audible sound.
- BRL-0063 now satisfies its final native audio criterion and is closed as `done`.
- BRL-0061 Stages 1 and 2 are complete. Stages 3–6 remain in scope.

## Remediation progress — Stage 3 complete, 2026-08-30

- Stage 3 is implemented and verified under linked ticket BRL-0064.
- Shared content and its fingerprint are now owned by a headless-safe `GameplayContentPlugin`; `ProtocolPlugin` is wire-registration only.
- Explicit `ClientReplicatedGameplayPlugin` and `ServerAuthoritativeGameplayPlugin` composition roots now own role gameplay selection. Client/server session plugins no longer install gameplay, content, presentation, or routed-worker plugins, and pending ability cleanup is installed by the authoritative gameplay root.
- The authoritative app and minimum lobby worker select routed transport and shared content explicitly. The lobby remains free of match gameplay; client/server feature isolation and compatibility fingerprints are preserved.
- The extracted session-to-ability cleanup boundary retains its exact transaction order through named network-lifecycle, gameplay-cleanup, and deferred-flush sets with schedule coverage.
- `just check`, `just lint`, `just test`, `just ci`, formatting, and diff hygiene pass. Routed 1v1/2v2/3v3 and all nine Practice topologies reached Active; no native playtest was required for the organization-only change.
- BRL-0061 Stages 1–3 are complete. Stages 4–6 remain: focused ECS dependencies and authoritative transactions, extension registries, and ownership-based module decomposition.


## Stage 4 progress — focused projection cleanup started

BRL-0065 owns the first Stage 4 slice: remove authoritative reads of the replicated loadout aggregate where Stage 1 already installed the exact fighter-stat, weapon, or generation-presence projection. Ultimate/passive projection, shared ultimate admission, and combat transaction decomposition remain subsequent Stage 4 work.


## Stage 4 progress — BRL-0065 complete

The first Stage 4 slice is complete. Recovery, public roster projection, sentry owner visibility, ready admission, and fighter-runtime construction now use the focused Stage 1 components instead of traversing `ResolvedMatchLoadout`. Projection-only characterization and the full canonical suite passed. Remaining Stage 4 work is ultimate/passive capability projection, shared ultimate admission, and combat delivery/effect transaction decomposition.


## Stage 4 progress — ultimate capability/admission started

BRL-0066 owns the next Stage 4 slice: atomically project `ResolvedUltimate`, migrate ultimate authority off the replicated aggregate, and consolidate the duplicated rising-edge/common admission/generation policy without moving semantic targeting or execution out of ability plugins.


## Stage 4 progress — BRL-0066 complete

Ultimate authority now consumes an atomically installed `ResolvedUltimate` capability. Eight activation coordinators share one pure rising-edge/common admission policy with preserved rejection precedence and raw latch semantics; checked generation rollover is centralized. Match readiness and Balance Lab reconciliation include the new projection. Full server, focused Balance Lab, networked Dash/Sentry, role, lint, and isolation verification passed. Remaining Stage 4 work is passive capability projection and combat delivery/effect transaction decomposition.


## Stage 4 passive capability projection completed — 2026-08-30

BRL-0067 completed the passive portion of focused authoritative loadout projection. `ResolvedPassives` is installed atomically with each resolved generation; passive telemetry, Adrenal Response movement, Quick Cycle recovery, Close Quarters, Tenacity, and resistance-aware effect application consume focused components rather than `ResolvedMatchLoadout`. Match readiness and Balance Lab reconciliation enforce the seven-component generation boundary. Full role-specific checks, lint, 492 client / 439 server / 461 Balance Lab tests, 94 network scenarios, and 12 performance gates pass.

The remaining BRL-0061 remediation is the structural combat transaction work: decompose the large delivery and effect coordinators into focused deterministic family planning/commit helpers while preserving their explicit fixed-tick ordering and atomic publication. Later registry/module-splitting recommendations remain lower-priority follow-up candidates and should be ticketed separately if retained after that transaction work.

## Stage 4 delivery emission completed — 2026-08-30

BRL-0068 completed the delivery-emission portion of the authoritative combat transaction remediation. `authoritative_composed_fire` remains one admission/economy/reservation/publication transaction, while a private exhaustive delivery coordinator delegates straight/sticky, blocked contact, lobbed/splash, melee, and cone-spray commits to focused helpers. Lobbed and Splash share one launch primitive, and routed characterization now covers all six delivery families, including a new melee/no-projectile proof. Full checks, strict lint, 492 client / 439 server / 461 Balance Lab tests, 95 network scenarios, and 12 performance gates pass.

The remaining required BRL-0061 work is effect application `plan -> commit -> project` decomposition with atomic mutation and outcome/cue/telemetry ordering preserved. Projectile sweep decomposition is a useful follow-up only if inspection shows a safe independently reviewable ownership boundary; later registry and broad module-splitting recommendations remain non-blocking follow-up candidates.

## Final remediation reconciliation and closeout — 2026-08-30

All required findings are now implemented or explicitly dispositioned:

- Stage 1 / BRL-0062 removed conflicting code-authored fighter/weapon defaults and established validated catalog-owned runtime projections with fail-closed generation boundaries.
- Stage 2 / BRL-0063 moved residual gameplay and presentation tuning into validated authored policies, removed duplicate/dead policy paths, and completed native default/reduced-effects evidence. The user confirmed the manifest-backed sound path is audible.
- Stage 3 / BRL-0064 corrected dependency direction: protocol and transport/session plugins no longer choose gameplay/content/presentation; explicit role composition roots own installation while preserving schedule and feature isolation.
- Stage 4 / BRL-0065 through BRL-0069 replaced deep loadout reads with focused fighter, weapon, ultimate, and passive capabilities; centralized common ultimate admission; decomposed all attack delivery families; and completed batch-wide composed-effect `plan -> commit -> project` authority. Final transaction implementation is commit `9aeaae1`.
- Stage 5's demonstrated extension seams were already completed and verified by BRL-0058: plugin-owned ability composition, a validated mode descriptor registry, a stable terminal-reaction registry, generic lifecycle cleanup facts, neutral authoritative phases, and additive/coverage tests. Speculative dynamic registries for typed wire effects or renderer internals are rejected for this remediation: they have no demonstrated second implementation and would weaken the explicit typed protocol/presentation boundaries without improving a current extension path.
- Stage 6's concrete high-risk ownership findings were already completed and verified by BRL-0057: flow, projectile sweep, worker control, and Sentry coordinators were decomposed into focused planning/commit responsibilities. Broad splitting of the cohesive lobby, 3D presentation, Balance Lab, and screen modules solely by file size is rejected under the repository's ownership/no-overengineering rules. Inspection did not establish a remaining independent owner, execution role, lifecycle, or second use that warrants another migration; schedule/composition roots remain intentionally visible. A future feature that establishes such a boundary should receive its own scoped ticket rather than speculative churn here.

The resulting durable contracts live in the owning catalog/schema, gameplay/network architecture documentation, role composition roots, focused capability components, neutral phase/registry APIs, and transaction module documentation. Public module paths and wire contracts were preserved except for the explicitly versioned authored/catalog changes owned by their child tickets. Server authority, client/server dependency isolation, bounded state, stable network identity, and deferred schedule boundaries remain intact.

### Final verification and feedback disposition

- Final `just ci` passed on the closing implementation state: role checks and strict lint; 492 client tests, 441 server tests, 463 Balance Lab tests, 95 serial network scenarios, and 12 performance gates.
- Routed product 1v1/2v2/3v3 and all nine Wipeout/Hot Zone/Heist Practice 1v1/2v2/3v3 topologies reached Active.
- Focused final composed-effects coverage passed 13/13, including pure boundary, recipient, resistance/Tenacity, stacking, event-reservation, and runtime-effect behavior.
- The native presentation/audio changes were exercised during BRL-0063; visual reports passed and the user's audible-cue confirmation is recorded. Later stages were organization-only and required no additional native playtest.
- No audit finding remains silently pending: required correctness, dependency, capability, extension, and transaction work is implemented; unsupported speculative registries and line-count-only file splits are rejected with the rationale above.

### Parent learn-from-errors review

The initial audit stated an overly broad Open/Closed aspiration for protocol-visible mechanics. Remediation refined it to a practical contract: typed schema changes remain local, while behavior is installed through owned plugins/registrations without unrelated coordinator rewrites. This preserves explicit compatibility and authority rather than hiding wire evolution behind dynamic dispatch.

The most subtle implementation risk appeared in the final effect transaction: final ECS equivalence was insufficient because telemetry and cues also depend on per-record snapshots and deferred `Commands` visibility. Independent semantic review found and corrected the fallback healing-ceiling and terminal record-local effects/motion edges before closeout. Future architecture remediation should define observable parity across final state, intermediate event order, telemetry snapshots, reservations, and deferred boundaries before extracting a transaction.
