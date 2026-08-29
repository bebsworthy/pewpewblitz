# Specification status

Detailed technical draft prepared 2026-08-28. The authority, data, schedule, lifecycle, replication, presentation, bot, capacity, and verification contracts below incorporate the accepted answers to `q-000005` and `q-000006`.

# Accepted product decisions — 2026-08-28

1. A fighter defeated solely by a neutral Damage tile credits the most recent qualifying hostile damager within the bounded attribution window. Tile source attribution remains neutral; Wipeout scoring consumes separate bounded recent-hostile-damage memory rather than fabricating the hostile player as the tile's damage source.
2. Feature Yard Wipeout is the first player-visible proving layout. That map exists to exercise the current environment feature set, so revise and re-admit it instead of creating a separate effect-tile map or changing the purpose-built 3v3 maps.

BRL-0036 remains `todo`: this work prepares scope and decisions; implementation has not started.

# Outcome

Maps can author bounded, server-owned gameplay effect tiles that create meaningful route choices through ordinary-locomotion speed boosts, ordinary-locomotion slows, or neutral periodic damage. Players can identify each tile before entering it and can tell when a fighter is currently affected. The server alone resolves membership, movement composition, pulse timing, health, defeat, attribution, scoring inputs, and cleanup.

# Player-visible first slice

- Add exactly three typed capabilities: Speed, Slow, and Damage.
- Add one stable map asset per capability. Recipes place stable `MapAssetId` values; recipes cannot supply multipliers, damage, pulse intervals, recipient rules, or executable behavior.
- Provisional balance values for the first playtest:
  - Speed: `1_250` milli, or 125% ordinary locomotion.
  - Slow: `700` milli, or 70% ordinary locomotion, matching the current readable weapon-slow scale.
  - Damage: 10 health every 30 simulation ticks after one full interval of occupancy (20 health/second at the current 60 Hz fixed tick).
- The values are authored and validated catalog content, not hard-coded asset-ID branches. Tune them only from focused/native evidence and record any change in this spec.
- Revise Feature Yard Wipeout so a neutral route, a faster exposed route, and a hazardous/contested route are materially different. Preserve its existing Wipeout 1v1/2v2 product role; do not advertise it as a new 3v3 map in this ticket.

# Non-goals

The first slice does not add conveyors, acceleration surfaces, ice momentum, forced displacement, teleporters, healing tiles, shields, silence, projectile modification, elemental reactions, arbitrary effect graphs, runtime/player-authored parameters, moving tiles, dynamic enable/disable switches, client prediction, or client-authored membership/outcomes.

Speed and Slow change ordinary locomotion maximum speed only. They do not change Dash, knockback/external motion, projectile velocity, firing cadence, animation authority, aim rotation, or collision geometry.

# Authored content model

## Typed capability

Extend `MapGameplayProfile` with one explicit fixed-shape field:

```rust
pub enum MapEffectTileBehavior {
    None,
    Speed { movement_multiplier_milli: u16 },
    Slow { movement_multiplier_milli: u16 },
    Damage { damage_per_pulse: u16, pulse_interval_ticks: u16 },
}
```

The enum is shared/headless-safe catalog data. Do not introduce a string capability key, generic effect list, behavior graph, callback, or renderer-owned rule.

Each approved effect-tile `MapAssetDefinition` remains the stable recipe primitive and resolves through its referenced `MapGameplayProfile`. Proposed new stable IDs are the next available IDs (`MapGameplayProfileId(11..=13)`, `MapAssetId(35..=37)`, and corresponding client visual profiles), but implementation must re-check the catalog before committing those numbers.

## Asset constraints

An effect tile is a one-cell, non-colliding `Feature` placement over an ordinary Surface. Reusing the `Feature` slot is intentional: it prevents a cell from simultaneously containing a wall, tall grass, damageable object, or another effect tile without adding a fifth map slot.

Catalog validation requires an effect-tile asset/profile to have all of the following:

- `MapAssetSlot::Feature`;
- a `1x1` footprint;
- `PlayerCollision::Pass` and `ProjectileCollision::Pass`;
- `MapColliderShape::None`;
- indestructible destruction and durability behavior;
- no interaction, concealment, placement parameters, or surface tag of its own;
- an explicit, sorted set of allowed underlying surface tags;
- all four quarter turns allowed unless the visual profile later proves orientation meaningful.

Speed validation accepts `1_001..=2_000` milli. Slow accepts `100..=999` milli. Damage accepts `1..=100` health and `6..=600` ticks. `None` remains the only legal behavior for profiles that do not satisfy the effect-tile asset contract.

The default surface asset cannot be an effect tile. Existing per-cell slot validation guarantees at most one effect tile per cell. Add an explicit `MAX_EFFECT_TILE_PLACEMENTS` ceiling, provisionally 4,096, and measure that exact ceiling before closeout; change the ceiling rather than waiving the gate if evidence requires a smaller supported bound.

## Recipe and spawn validation

The existing sparse recipe and `filled_rects` syntax remain unchanged. No arbitrary effect parameters enter map RON.

Canonical resolution must additionally reject:

- an effect tile that shares a cell with a player-spawn marker;
- a Damage tile in the eight neighboring cells around a player spawn;
- an effect tile whose underlying surface tag is not allowed;
- more than the effect-tile ceiling;
- any blocking/navigation interpretation of an effect tile;
- a proving map without three safe spawn points per team and at least one tile-free exit route per spawn group.

Feature Yard Wipeout is the proving layout. Bump its recipe revision and admission revision, retain its stable preset/recipe identity, and re-run its advertised Wipeout 1v1/2v2 admission and balance evidence. Do not change Feature Yard Hot Zone or Heist merely to make the family visually uniform; those modes receive effect tiles only under later mode-specific design.

Hot Zone/Heist anchor overlap is not needed for this slice; until a later owned use specifies it, reject effect-tile cells intersecting a mode anchor or Heist-safe footprint.

Adding the capability changes the map catalog schema and canonical gameplay-content fingerprint. Plan for `MAP_CATALOG_SCHEMA_VERSION` 7 and `MAP_FINGERPRINT_FORMAT_VERSION` 9 after checking concurrent changes. `MAP_RECIPE_SCHEMA_VERSION` can remain 5 because recipe syntax is unchanged. Adding replicated state is an incompatible protocol change; bump the one global `SUPPORTED_PROTOCOL_VERSION` (currently 32) once, with no per-message version or compatibility decoder.

# Resolved map data and spatial rule

Add a resolved server/headless-safe fact:

```rust
pub struct ResolvedEffectTile {
    pub placement_id: MapPlacementId,
    pub cell: MapCell,
    pub behavior: MapEffectTileBehavior,
}
```

`ResolvedMap` owns a canonical `BTreeMap<MapCell, ResolvedEffectTile>` (or equivalently bounded sorted lookup proven by tests). Resolution derives it from placements and catalog profiles. Runtime systems never scan the full placement list per fighter per tick and never switch on asset IDs.

Membership uses the authoritative fighter center (`Position`) and the grid cell containing that center. Use a shared `world_to_cell` helper with half-open cell bounds (`min <= p < max`), except that the playable outer maximum clamps to the last legal cell. This prevents an exact shared edge from belonging to two adjacent cells. Non-finite or out-of-bounds positions produce no membership.

A tile does not use Avian collision events or renderer geometry to establish occupancy. Fighter radius does not expand membership in the first slice; the center-cell rule is the exact gameplay boundary and the visual must match it.

# Authoritative runtime state

Add one bounded replicated fighter component owned by the map-effect runtime:

```rust
pub struct EffectTileOccupancy {
    pub generation: MapDynamicGeneration,
    pub placement_id: MapPlacementId,
    pub kind: EffectTileKind,
    pub entered_at_tick: u64,
    pub next_pulse_at_tick: Option<u64>,
}
```

`EffectTileKind` is the public Speed/Slow/Damage identity. Numeric behavior remains canonical catalog content resolved from the placement/profile. The component is absent when the fighter has no current tile effect.

Server reconciliation compares `(map generation, placement ID, kind)`:

- no tile to tile: insert occupancy, set `entered_at_tick = current_tick`, and for Damage set `next_pulse_at_tick = current_tick + interval`;
- same tile: preserve entry and pulse deadlines;
- tile to no tile: remove occupancy immediately at the explicit deferred boundary;
- tile A to tile B: treat as exit followed by a fresh entry, even when both have the same kind;
- defeated, inactive, respawning, map-mismatched, non-finite, or torn-down fighter: remove occupancy and private pulse state.

Adjacent authored effect tiles therefore do not geometrically overlap under the center-cell rule. Required overlap behavior concerns composition with existing fighter effects and deterministic same-tick damage sources.

# Fixed-tick schedule

Keep ownership and deferred visibility explicit.

## FixedUpdate

1. Existing match lifecycle/respawn/reset work runs in `GameplaySet::Lifecycle` and its current `ApplyDeferred` boundary completes.
2. Practice bots capture/commit ordinary input through the existing path.
3. A new map-owned `MapEffectSet::ResolveOccupancy` runs after `GameplaySet::Input` and before `GameplaySet::Simulation`.
4. `(resolve_effect_tile_occupancy, ApplyDeferred).chain()` makes the new/removal state visible atomically.
5. `authoritative_movement` reads the committed occupancy and computes the tick's desired velocity.

Sampling at the start of movement is intentional: crossing into or out of a tile during tick T changes the modifier beginning on tick T+1. This produces a deterministic one-tick boundary with no mid-movement state visible to combat or modes.

## FixedPostUpdate

Damage pulses run in `CombatDamageSet::EnvironmentReactions`, after direct combatant damage, world-object reactions/explosions, and mode-objective damage, but before restoration-pickup application and before mode rules consume outcomes.

Same-tick precedence is therefore:

1. direct/melee/projectile combat;
2. damageable-world reactions and explosions;
3. mode-objective damage;
4. due effect-tile Damage pulses;
5. restoration pickup collection;
6. combat lifecycle, cues/facts, concealment observation, and mode scoring through existing ordered sets.

A target defeated by an earlier phase is ineligible for a tile pulse. A due tile pulse can defeat a fighter before a same-tick pickup can restore health. Tests must lock this order rather than relying on incidental Bevy insertion order.

# Movement composition

Extend the existing pure `resolved_movement_velocity` rule with one terrain multiplier. The reviewed formula is:

```text
ordinary velocity = normalized input
                  × resolved loadout speed
                  × strongest active combat Slow multiplier
                  × effect-tile movement multiplier
                  × Adrenaline multiplier

final velocity = ordinary velocity + unmodified ExternalMotion
```

Speed uses `1.250`, Slow uses `0.700`, and Damage/no tile uses `1.000`. The existing target-owned combat Slow keeps its strongest-refreshes semantics. Combat Slow and a tile multiplier compose multiplicatively; no asset-ID precedence branch is allowed. Validate the final ordinary multiplier as finite and clamp it to a reviewed engine range of `0.10..=2.50` before applying it. ExternalMotion and Dash remain outside this multiplication.

Pure tests cover combat Slow + Speed, combat Slow + Slow, Adrenaline + each tile, zero input, external motion, expiry boundary, and the final clamp.

# Damage pulse transaction and attribution

Damage tiles affect living `Fighter` entities that are active combatants in the active match. They are neutral: team and owner do not filter eligibility. They do not affect sentries, projectiles, damageable map objects, pickups, or Heist objectives in this slice.

Spawn protection blocks a pulse's damage. A blocked due pulse still advances its deadline, emits no damage/defeat outcome, and cannot accumulate into a burst when protection ends.

Entry is non-damaging. The first pulse is due after one complete authored interval. At a due tick, apply at most one pulse and advance to the next future deadline using saturating integer arithmetic. The fixed schedule normally cannot miss ticks; recovery/defensive tests prove that a stale deadline never applies catch-up burst damage.

Before mutation, collect candidates and sort by `(placement_id, target_network_id)`. Plan the whole due batch, reserve stable `AttackId`/`CombatEventId` identities, and fail the batch without partial health mutation if identity reservation cannot complete. One placement/pulse shares one environment attack identity; each affected target receives its ordered damage event and optional defeat event.

Combat remains the owner of health, `Defeated`, collision disablement, effect/motion clearing, outcome facts, and combat cues. Implement a focused combat-owned environment-fighter damage helper/transaction and call it from the map runtime. Do not copy the existing fighter-defeat mutation into a second map-only path. If the oil-barrel explosion path is safely adapted to the same helper, preserve its exact behavior and tests; that refactor is permitted only as the demonstrated second use, not as a broader combat rewrite.

For every applied tile pulse:

- clamp applied damage to current health;
- emit `CombatOutcomeFact` with `CombatSourceKind::Environment`, `source_player = None`, `source_network_id = None`, `source_team = None`, exact target facts, and the tile pulse attack ID;
- emit existing `CombatCue::Damage`/`Defeat` with `DamageSource::Environment { map_instance_id, generation, placement_id, initiating_player: None, initiating_fighter: None }`;
- allow existing positive-damage concealment reveal observation to see the outcome;
- grant no dealt/received primary-weapon ultimate charge and no weapon telemetry credit;
- record bounded map-effect telemetry for entries, exits, due/applied/blocked pulses, damage, defeats, and rejected/stale state.

## Recent hostile damage credit for Wipeout

The tile damage fact continues to supply no source player, fighter, or team. Wipeout owns a separate bounded `RecentHostileDamageCredits` resource keyed by target `NetworkEntityId`; it is not replicated and never changes combat attribution, cues, concealment, ultimate charge, or weapon telemetry.

A credit record contains match ID, source player, source fighter, source team, target fighter/team, source damage event ID, damage tick, and `expires_at_tick`. The initial attribution window is 300 simulation ticks (5 seconds) and belongs to validated Wipeout rules/tuning so playtesting can change it without altering the generic tile definition.

During Wipeout mode-rule processing, consume current-tick `CombatOutcomeFacts` in event-ID order:

- positive fighter Damage from a valid hostile source team updates the target's record; neutral damage, self damage, same-team damage, protected contact, deployable damage, and healing do not;
- a later qualifying hostile Damage event replaces the earlier record, including multiple hits in one tick;
- player-initiated environment damage such as a hostile-triggered barrel may qualify when its outcome fact carries valid hostile source identity/team;
- a direct hostile Defeat uses its ordinary source-team credit and does not double count the memory;
- a neutral Damage-tile Defeat with no ordinary credited source falls back to the target's newest record only when the match/target/team still match and `tick < expires_at_tick`;
- after any defeat, remove that target's record; prune expired/wrong-match/nonparticipant records and clear the resource on restart/teardown.

This lets same-tick hostile damage followed by a tile pulse credit correctly because the earlier damage event is processed first. A disconnected or subsequently defeated attacker may still earn team credit within the window, matching posthumous source attribution, but the tile cue remains neutral. If no valid record exists, the neutral defeat remains unscored. Tests lock direct-source precedence, exact expiry, same-tick ordering, multiple attackers, initiated environment damage, disconnect, respawn, restart, and no double score.

Tile damage does not reset the attack-idle health-recovery origin because that resource tracks accepted player attacks, not received environmental damage.

# Lifecycle matrix

- Match waiting/countdown: no occupancy and no pulses.
- Active match entry: derive occupancy at the next pre-movement phase.
- Exit: remove status before that tick's movement; no lingering Speed/Slow or pulse deadline.
- Defeat: combat clears tile occupancy/status in the same terminal transaction or the immediately ordered lifecycle boundary; no posthumous pulse.
- Respawn: start with no occupancy/deadline, then derive from the authoritative spawn cell; validation keeps effect tiles off spawn cells.
- Spawn protection: movement effects may apply if the fighter later enters a Speed/Slow tile; Damage pulses are blocked as above.
- Match restart: clear every occupancy/pulse state before the restarted active phase.
- Map generation reset with unchanged static placements: invalidate old-generation occupancy and re-derive; no old deadline survives.
- Map replacement/instance change: authoritative map teardown clears the resolved lookup and all fighter occupancy before installing the next map.
- Disconnect/roster removal/worker teardown: ordinary entity/resource teardown owns cleanup; no process-global source history remains.
- Late join/reconnect: snapshot reconstruction supplies static tiles; the replicated fighter component supplies current authoritative occupancy/deadline without replaying entry history.

# Replication, recovery, and security

Register `EffectTileOccupancy` as server-to-client replicated state in `protocol.rs`. Clients receive static placement and profile identity through the existing `ResolvedMapSnapshot` plus their build-embedded catalog. No new tile-placement message or generic environment event stream is required.

Existing reliable Combat cues carry applied Damage/Defeat feedback. Entry/exit is durable component convergence, not a correctness-critical transient message. Client presentation may create a local bounded entry/exit cue from component change, but gameplay and evidence never depend on it.

Component removal/addition must converge for late join, reconnect, map replacement, defeat, and restart. Observer-specific concealment remains authoritative: a hidden fighter and all attached status state follow the existing relevance boundary; tile visuals themselves are public static map knowledge. No cue, HUD element, overhead marker, or bot observation may reveal a concealed fighter that the observer is not permitted to know.

Network tests attempt client-authored membership, deadlines, health, and damage facts and prove the server ignores them. Keep the one global compatibility handshake; do not add per-component versions or legacy decoders.

# Presentation and accessibility

## Before entry

Extend the client-only map visual catalog with a generated effect-tile visual family. Each capability has both color and non-color language:

- Speed: forward/repeating chevrons and a cool/positive motion palette;
- Slow: transverse drag bands or inward marks and a distinct cool/heavy palette;
- Damage: warning border plus repeated hazard marks and a warm hostile palette.

The exact one-cell authoritative boundary must remain readable at supported gameplay camera framing. Decorative motion stays inside the boundary and cannot imply a larger gameplay area.

Batch generated effect-tile geometry by map instance and visual/kind rather than spawning one expensive scene hierarchy per tile. Reconciliation keys include map instance/fingerprint and cleanly remove stale batches on replacement. Primitive fallback uses the same generated boundary/pattern and requires no imported asset.

## While affected

Use replicated `EffectTileOccupancy` to add a bounded fighter-status treatment and controlled-fighter HUD label (`BOOST`, `SLOWED`, or `HAZARD`). Damage continues to use existing health delta, hit, and defeat feedback. Status presentation is observer-relative and must not bypass concealment.

Reduced Effects removes optional shimmer/particles and shortens redundant entry feedback, but retains the static boundary/pattern, affected-fighter marker, and controlled-fighter HUD label. Reduced Motion freezes nonessential tile animation. Missing material/shader/imported assets fall back to static generated geometry; visual failure never affects authority.

Add focused tests for exact footprint scale, kind distinction, batching/capacity, cleanup, reduced-effects/reduced-motion behavior, primitive fallback, and concealment-safe status materialization.

# Practice bots

Effect tiles are public static map facts. Extend `BotNavigationSnapshot` with canonical cell effect metadata derived from `ResolvedMap`; do not copy presentation state or query runtime ECS from pure navigation.

Weighted route search uses deterministic integer costs:

- neutral cell time multiplier: 1,000;
- Speed/Slow traversal time: reciprocal of the authored movement multiplier (`1_000_000 / multiplier_milli`, rounded deterministically);
- Damage: neutral traversal time plus a bounded hazard surcharge derived from authored damage/interval and the bot's delayed observed current/maximum health.

Damage cells remain traversable. The bot may accept hazard cost when objective/pressure utility justifies it; low health raises the penalty. The exact risk coefficient belongs in validated `BotProfile`, not map content or presentation.

Current direct-line shortcutting must not bypass weighted terrain. On a map with any effect tile, use weighted A* (or prove an equivalent cost-aware shortcut), preserve stable node/neighbor tie order, and ensure route compression never replaces a chosen safe path with an unchecked line through higher-cost cells. Work remains resumable and within existing expansion/point budgets.

Bots do not receive hidden membership or perfect opponent knowledge. Their own delayed observation already carries health/pose; public map effects are static map knowledge. Tests cover Speed preference, Slow avoidance, health-sensitive Damage avoidance, unavoidable hazard traversal, equal-cost stability, insertion-order stability, and unchanged behavior on maps without effect tiles.

# Capacity and performance

The implementation must remain bounded by:

- maximum 4,096 authored effect-tile placements per resolved map (provisional measured ceiling);
- at most one Wipeout recent-hostile-damage credit record per active roster fighter, pruned by match/tick and cleared on defeat/restart;
- at most one occupancy component and one pulse deadline per fighter;
- at most the active roster's fighter lookups per fixed tick, independent of total ordinary placements;
- at most one due pulse candidate per fighter per tick;
- bounded telemetry/outbox/event planning using existing combat/map limits;
- client visual batching by kind/profile rather than one hierarchy per tile;
- bot search under existing per-tick expansion and route-point budgets.

Add a performance gate at maximum supported 3v3 roster and the effect-tile ceiling, with all fighters crossing or occupying Damage tiles and bot weighted routing active. Report resolution time, occupancy phase, pulse phase, bot navigation, client entity/mesh high water, and total fixed-tick budget. The test must demonstrate no linear per-fighter scan across all map placements.

# Expected implementation ownership

Likely files/modules, subject to source inspection during implementation:

- `src/map/catalog.rs`: typed authored behavior, validation, canonical resolution, schema/fingerprint, resolved lookup.
- `content/catalogs/map_gameplay_profiles.ron` and `map_assets.ron`: three approved profiles/assets.
- `src/map/effect_tiles.rs`: shared identities, occupancy component, and pure spatial/deadline/composition helpers if that ownership remains cohesive.
- `src/map/runtime/effect_tiles.rs` and `runtime/mod.rs`: authoritative occupancy, pulse orchestration, telemetry, schedule composition, reset/teardown hooks.
- `src/combat/authority.rs` or a focused `src/combat/environment.rs`: combat-owned neutral environment damage/defeat transaction.
- `src/matchplay/wipeout.rs`: bounded recent-hostile-damage credit memory and deterministic neutral-defeat fallback.
- `src/movement/authority.rs` and focused movement tests: terrain multiplier input and formula.
- `src/protocol.rs`: replicated component registration and global compatibility bump.
- `src/client/presentation_3d/environment_assets/{catalog,runtime}.rs`, map materialization, fighter feedback, and HUD: generated/batched tile and affected state.
- `assets/catalogs/map_asset_visuals.ron`: exact client visual coverage.
- `src/bots/{navigation,controller,model,profile,tests}.rs`: public weighted terrain and risk cost.
- `content/maps/builtin/feature-yard-wipeout.ron`, its index admission revision, and affected lobby/admission tests: revise the existing Feature Yard Wipeout preset for effect-tile routes without creating a new map identity.
- `tests/network/effect_tiles.rs`, `tests/network.rs`, `tests/performance.rs`, and existing harness helpers.

Keep `mod.rs` files as composition/public surfaces. The new map-effect concern merits a named module; do not grow `map/catalog.rs`, `map/runtime/mod.rs`, movement, or presentation coordinators with an unrelated giant system.

# Verification plan

## Pure and focused ECS

- catalog parse, exact profile/asset coverage, invalid multipliers/damage/intervals, invalid slot/collider/durability/parameters, unknown profile, surface incompatibility, placement ceiling, spawn/anchor safety, and stable fingerprints;
- half-open world-to-cell boundaries, negative/out-of-bounds/non-finite inputs, deterministic placement lookup;
- entry, same-tile persistence, exit, adjacent handoff, generation mismatch, first pulse, exact due tick, stale deadline, blocked pulse advancement, and overflow failure;
- movement formula and external-motion/Dash exclusions;
- whole-batch event reservation and deterministic target order;
- health clamp, defeat cleanup, neutral tile source, no tile charge, reveal fact, and damage-before-pickup order;
- Wipeout bounded hostile-credit update/fallback, direct-source precedence, newest-event selection, exact expiry, clear/prune lifecycle, and no double scoring;
- restart, respawn, map replacement, disconnect, and teardown cleanup;
- weighted bot routing and work bounds;
- client batching, exact boundary, status cleanup, reduced settings, and fallback.

## Network/separate-App

- server-only membership from authoritative pose;
- client spoof attempts cannot create membership, damage, health, or defeat;
- Speed/Slow authoritative pose convergence under impairment;
- exact Damage pulse/health/defeat/cue attribution;
- late join while occupying each kind;
- reconnect after exit, defeat/respawn on or near a tile, restart, generation reset, and map replacement;
- concealment does not leak hidden affected-fighter state or cues;
- Wipeout recent-hostile credit covers direct-source precedence, same-tick hostile hit then tile defeat, newest attacker, exact 300-tick expiry, no-credit fallback, disconnect, respawn, and restart.

## Canonical and routed

Run focused tests during implementation, then the repository-owned gates:

- `just fmt`
- `just check`
- `just lint`
- `just test`
- `just e2e 2`, `just e2e 4`, `just practice-e2e wipeout-1v1`, and `just practice-e2e wipeout-2v2` for the revised Feature Yard path;
- representative `just e2e 6` / purpose-built 3v3 regression evidence because shared runtime and protocol changes still affect 3v3, without moving the proving tiles onto those maps.

Record exact command outputs/evidence in the ticket. Run `ticket sync` before handoff and stop on conflicts.

## Native playtest

Provide the exact run path, selected game/map, controls, and route scenarios. Verify with keyboard/mouse and controller:

- each tile is identifiable before entry without relying only on color;
- entry/exit timing and affected status match the exact cell boundary;
- Speed offers a useful but exposed route, Slow changes pursuit/escape decisions, and Damage creates fair denial rather than unavoidable spawn punishment;
- combat Slow, Adrenaline, Dash, knockback, concealment, recovery, pickup, defeat, and respawn remain understandable;
- Feature Yard 1v1/2v2 overlap remains readable in normal, Reduced Effects, Reduced Motion, and forced primitive/fallback modes;
- Practice bots use and avoid routes plausibly without perfect knowledge.

Keep the ticket `doing` while required subjective feedback or accepted corrections remain.

# Documentation and closeout

Reconcile durable behavior in:

- `docs/09-environment-gameplay.md` for exact tile semantics and promotion from candidate to implemented capability;
- `docs/04-maps-and-game-modes.md` for authored asset/profile/validation and the accepted proving map;
- `docs/08-network-architecture.md` for replicated occupancy and recovery/security boundary;
- `docs/10-bots.md` for public weighted terrain;
- `docs/11-art-and-presentation-direction.md` for boundary/status/reduced/fallback language;
- any player UX/control or map-builder contract materially affected by the accepted slice.

Before `done`, disposition every playtest item, record the final tuning/map rationale, verification evidence, known limitations/deferred tickets, and a substantial-work learn-from-errors review.

# Acceptance criteria

- Three stable authored assets resolve only through validated typed Speed, Slow, and Damage profiles; recipes cannot inject rules or derive gameplay from visuals.
- Server center-cell occupancy, entry/exit, adjacent handoff, pulse deadlines, overlap composition, and lifecycle behavior are deterministic and bounded.
- Speed/Slow affect ordinary locomotion through the one documented formula without changing Dash, knockback, projectiles, attacks, or collision.
- Damage produces exact neutral map/generation/placement attribution, combat-owned health/defeat state, existing cues/outcomes, no fabricated player credit, and the approved Wipeout scoring behavior.
- Spawn protection, defeat, respawn, restart, generation reset, map replacement, disconnect, late join, reconnect, and teardown converge without stale state or catch-up bursts.
- Clients cannot claim membership or outcomes; replicated state/cues are sufficient for HUD/world presentation and do not leak concealed fighters.
- Tile boundary/kind and affected status are distinguishable before entry and while affected in normal, Reduced Effects, Reduced Motion, and primitive/degraded presentation.
- Practice bots use public effect facts in deterministic bounded weighted navigation without hidden information or a parallel gameplay path.
- Revised Feature Yard Wipeout creates meaningful route choices, retains its stable identity, increments recipe/admission revisions, and passes routed/native Wipeout 1v1/2v2 readability, balance, recovery, and safe-spawn evidence; shared 3v3 regressions still pass.
- Focused rule/ECS/network tests, canonical checks, maximum-density performance evidence, documentation reconciliation, feedback disposition, and learning review pass before the ticket moves to `done`.

## Implementation reconciliation — 2026-08-29

Repository inspection before implementation found that adjacent completed work advanced the live compatibility and content ranges beyond the draft's examples. BRL-0036 therefore uses the next live values rather than the stale draft numbers: global protocol version 38 (from 37), map catalog schema version 7 (from 6), map fingerprint format version 9 (from 8), effect-tile asset IDs 35-37, and client visual IDs 50-52. The map recipe schema remains version 5. Existing combat Fields and Conditions already occupy explicit authoritative damage phases; effect-tile pulses join CombatDamageSet::EnvironmentReactions through a combat-owned neutral-environment damage helper. These are implementation reconciliations, not product-scope changes.

## Implementation evidence — 2026-08-29

Implemented the accepted slice with authored `Speed`, `Slow`, and `Damage` gameplay profiles; six proving placements in Feature Yard Wipeout; server-owned half-open occupancy; movement composition; neutral periodic damage through the combat transaction; recent-hostile Wipeout credit; weighted bot navigation; replicated occupancy; and distinct tile/fighter presentation. Compatibility advanced to protocol 38, map catalog schema 7, fingerprint format 9, and Feature Yard Wipeout admission revision 3.

Verification passed:

- `just lint` — formatting, web checks/build, Clippy for routing/client/server/network/Balance Lab, server feature isolation, and cleanup checks.
- `just check` — all independent build roles.
- `just test` — routing (83 tests plus process/runtime/isolation suites), client-only (446), server-only (371), Balance Lab (390 plus its network case), network integration (90), and performance (12).
- `just practice-e2e wipeout-1v1` — Practice Wipeout 1v1 reached Active with one human participant.
- `just e2e 4` — routed Feature Yard 2v2 reached Active with the exact roster.
- `just e2e 6` — routed 3v3 regression reached Active with the exact roster.

The ticket remains `doing` because native visual/game-feel acceptance is still required. The playtest must check idle and occupied readability, exact cell boundaries, Speed/Slow feel, Damage first-pulse/cadence and spawn protection, recent-hostile Wipeout credit, bot route reasonableness, and reduced-effects stability.

Learning review:

- The repository had advanced beyond the ticket's draft compatibility numbers, so live catalog/protocol revisions were rechecked before mutation and the spec was reconciled first.
- A historical shared-geometry test assumed all Feature Yard variants had identical placements. It now compares structural geometry while the Wipeout effect placements have their own exact assertions.
- Applying the reduced weighted-path heuristic globally caused a max-grid regression. The existing heuristic is retained for unweighted maps and the admissible reduced heuristic is selected only when weighted terrain is present.
- Clippy exposed orchestration growth. Movement inputs were bundled into `MovementModifiers`, helpers were extracted, and the only retained complexity exception is narrowly attached to the atomic exclusive combat transaction.

## Accepted playtest-layout correction — 2026-08-29

User feedback requested larger Speed and Slow areas so their effects can be observed over a sustained section rather than a single-cell crossing. This supersedes the six-placement proving layout recorded above: each of the two mirrored Speed anchors and two mirrored Slow anchors is now a complete 3-by-3 patch (18 Speed cells and 18 Slow cells total), while the two Damage cells remain unchanged. Feature Yard Wipeout recipe/admission revision advances from 3 to 4, including the routed allocation default. Exact catalog tests assert every cell in all four patches as well as the 38-tile aggregate.

### Enlarged-patch verification — 2026-08-29

- `cargo fmt --all -- --check` — passed after formatting the new exact-cell assertion.
- `cargo test --features server,client feature_yard_variants_share_geometry_and_own_only_legal_mode_anchors` — passed; validates two exact 3-by-3 Speed patches, two exact 3-by-3 Slow patches, two Damage cells, and structural parity with the other Feature Yard variants.
- `cargo test -p brawler-routing` — passed all 83 unit tests, 4 supervisor tests, 5 process tests, 5 runtime-process tests, and 3 two-worker isolation tests with revision 4 routing defaults.
- `just practice-e2e wipeout-1v1` — passed; the revised routed Practice match reached Active with one human participant.

## Accepted spawn-area playtest correction — 2026-08-29

User feedback reports that the central proving patches cannot be evaluated reliably while Practice bots are firing. Add one mirrored rear-of-spawn testing area per team: separate 3-by-3 Speed, Slow, and Damage patches placed behind the team's spawn line, reachable immediately but not overlapping any spawn. Damage cells must retain the existing greater-than-one-cell spawn clearance. Preserve the central route-choice patches, symmetry, and ordinary authoritative behavior; this is map content, not a special safe-mode rule. Advance Feature Yard Wipeout recipe/admission and routed allocation revisions from 4 to 5, assert every new patch cell, and rerun catalog, routing, and routed Practice verification.

### Spawn-area correction verification — 2026-08-29

Implemented two mirrored rear-of-spawn testing areas. Each side has separate 3-by-3 Speed, Slow, and Damage patches, bringing the proving map to 36 Speed cells, 36 Slow cells, and 20 Damage cells while retaining the central patches. Feature Yard Wipeout recipe/admission and routing defaults are revision 5. Damage safety validation passes with every hazard at least two cells from the nearest spawn.

- `cargo test --features server,client feature_yard_variants_share_geometry_and_own_only_legal_mode_anchors` — passed; validates all 92 exact effect cells and shared structural geometry.
- `cargo test -p brawler-routing allocation::tests` — passed all 5 focused allocation tests with revision 5 defaults.
- `just practice-e2e wipeout-1v1` — passed; revised routed Practice reached Active with one human participant.

## Accepted Balance Lab tuning extension — 2026-08-29

The user accepted adding effect-tile tuning to the development-only Balance Lab. Extend its existing authoritative snapshot/editor/apply/restart/persistence transaction with exactly four numeric controls: Speed movement multiplier, Slow movement multiplier, Damage per pulse, and Damage pulse interval displayed in seconds and stored as simulation ticks. Do not add tile placement editing or a second runtime mutation path.

The canonical production catalog remains fixed at 1.250 Speed, 0.700 Slow, 10 Damage, and 30 ticks. Balance Lab candidates may use the already specified safe behavior ranges: Speed 1.001-2.000, Slow 0.100-0.999, Damage 1-100, and interval 6-600 ticks. Candidate validation must preserve the three stable profile identities and behavior variants, validate the otherwise canonical map catalog before installing development-only effect values, persist and restore the four values with the Balance Lab snapshot, expose them in a dedicated map/effect-tile editor subject, show canonical diffs, and apply them only through the existing atomic match restart.

Verification must cover manifest paths/ranges/units, snapshot round trip and shape rejection, invalid values, successful apply and restore, revised map resolution after restart, web type/build/tests, server Balance Lab tests, and the canonical Balance Lab/Practice gates proportional to the change.

### Balance Lab tile-tuning implementation evidence — 2026-08-29

Implemented a dedicated **Maps → Effect tiles** Balance Lab surface with four server-described controls: Speed multiplier (`1.001×..=2.000×`), Slow multiplier (`0.100×..=0.999×`), Damage per pulse (`1..=100 health`), and pulse interval (`0.10..=10.00 seconds`, stored as `6..=600` fixed ticks). Snapshot schema advanced 16→17, persistence envelope 11→12 with canonical migration for existing sessions, and editor manifest 7→8. Apply validates the ordinary canonical map catalog, installs only the bounded development-worker behavior values, persists them, and uses the existing atomic Practice restart. Restore defaults clears persistence and reinstalls canonical values. Durable operator guidance was updated in `docs/15-balance-lab.md`.

Verification passed:

- Focused Balance Lab suite: 21 tests covering manifest paths/ranges/units, JSON shape, invalid bounds, catalog installation and exact resolved-map behaviors, atomic apply, persistence round trip/follow-up, and sequential migration.
- Balance Lab web: 10 tests, TypeScript check, and production Vite build.
- `just lint` — full formatting, web, Clippy role matrix, feature isolation, and cleanup gates passed.
- `just check` — all independent roles passed.
- `just test` — routing/process/isolation, client 446, server 371, Balance Lab 392, Balance Lab network case, network 90, and performance 12 all passed.
- Final focused Balance Lab Clippy and resolved-map test passed after the full suite.

The first full `just test` run exposed a stale `FEATURE_YARD_WIPEOUT_ADMISSION_REVISION` constant left at 3 while the recipe, index, and routed policy were already revision 5 from the accepted spawn-area corrections. The three admission failures were deterministic consistency failures. Updating the exported server constant to 5 fixed all 12 focused admission tests and the subsequent full suite. Future map-revision corrections should update and test recipe, index, routed policy, and exported server admission constant as one atomic set.

BRL-0036 remains `doing` for native Balance Lab/UI and gameplay feedback: confirm the Maps tab is clear, apply each extreme and a representative midpoint, observe the clean reset, verify Speed/Slow and Damage cadence on the rear spawn patches, reconnect to confirm persistence, and restore canonical defaults.

## Accepted Balance Lab section correction — 2026-08-29

Move the Effect tiles subject from the standalone Maps section into World objects. Remove the Maps section identity and navigation tab from the Rust editor manifest and React model/UI/canonical-diff labels; retain the four field paths, controls, snapshot/persistence schemas, apply behavior, and editor manifest schema version 8. Update tests and operator documentation. Rationale: Balance Lab edits effect behavior rather than map placement, and a one-subject Maps tab incorrectly suggests layout editing while World objects already owns map-authored gameplay content.

### World-objects grouping correction evidence — 2026-08-29

Moved Effect tiles into the existing World objects section and removed the standalone Maps section identity from the Rust manifest, TypeScript model, navigation, help copy, and canonical-difference labels. Field paths, snapshot/persistence schemas, editor manifest schema 8, and authoritative apply behavior are unchanged. The operator guide now names **World objects → Effect tiles**.

Verification passed: all 6 focused editor-manifest tests, all 10 web tests, TypeScript/Vite production build, Rust formatting, and diff whitespace validation. Source search confirms no retired Maps section identity remains in the Balance Lab implementation or guide.

## Accepted Damage-tile healing suppression — 2026-08-29

User feedback requires an occupying Damage tile to block healing. Treat `EffectTileOccupancy { kind: Damage }` as the one authoritative suppression fact. While present, every server-owned positive-health path must leave current health unchanged: passive health recovery, healing payloads and restoration fields, and restoration-pickup collection. Blocked healing must not consume a pickup or emit a successful heal outcome/cue; existing damage pulses, spawn protection, defeat, exit, restart, and replication behavior remain unchanged. Speed/Slow tiles do not suppress healing. Add focused schedule/rule coverage and update durable behavior documentation.

The Balance Lab Speed ceiling remains `2.000×` pending an explicit replacement value. It originated in BRL-0036's accepted safe tuning range, while the downstream combined movement clamp is `2.5×`; this is a policy ceiling rather than a representation or physics limit.

## Movement ceiling clarification — 2026-08-29

Live-source and history review corrected the earlier characterization: the `2.50×` combined ordinary-movement clamp is specified but is not implemented in `resolved_movement_velocity`. The proposed value was only a defensive safety envelope with headroom for a `2.00×` Speed tile combined with the existing `1.15×` Adrenaline multiplier (`2.30×`); it is not a physics, transport, or representation requirement and has no older evidence-derived rationale. Do not introduce that clamp while the user is reviewing the cap policy. The current Balance Lab `2.00×` Speed ceiling likewise remains unchanged until the user supplies an explicit replacement policy/value.

## Damage-tile healing suppression implementation evidence — 2026-08-29

Implemented one authoritative `EffectTileOccupancy::blocks_healing` rule and consumed it at every positive-health gameplay path. Damage occupancy now clears/blocks idle health-recovery accumulation, prevents composed healing payloads from reserving or emitting a healing event, prevents Restoration Field healing before event allocation, and excludes the fighter from restoration-pickup collection. A blocked pickup remains live unless its ordinary expiry is due; no collection fact, cue, telemetry, or health mutation occurs. Speed and Slow occupancies remain non-blocking. Updated the durable map and map-asset behavior documentation.

Focused evidence passed:

- `cargo test --locked --no-default-features --features server map::effect_tiles::tests::only_damage_occupancy_blocks_healing --lib`
- `cargo test --locked --no-default-features --features server map::pickups::tests::damage_tile_blocks_collection_without_consuming_the_pickup --lib`
- `cargo test --locked --no-default-features --features server combat::effects::tests::damage_tile_suppresses_composed_heal_event_reservation --lib`
- `cargo test --locked --no-default-features --features server --lib` — 374 passed, 0 failed
- `just lint` — Balance Lab web tests/build, all Rust role Clippy lanes, and architecture guards passed
- `just check` — routing, client, server, network-test, Balance Lab, and web test/build checks passed
- `git diff --check` passed

BRL-0036 remains `doing` because the existing native effect-tile playtest/acceptance is still open.

## Durable application-documentation clarification — 2026-08-29

Update the owning application guides under `docs/` to describe the implemented effect-tile pipeline end to end: the three supported variants (Speed, Slow, Damage; no Heal tile), catalog resolution, server-owned fixed-tick occupancy, movement composition, neutral damage pulses, Damage-owned healing suppression, replication/presentation, spawn safety, and Balance Lab controls. Document the current implementation honestly: Damage amount/interval tuning is consumed from the resolved catalog, while authoritative Speed/Slow movement still reads the canonical 1.250/0.700 constants, so persisted Balance Lab Speed/Slow edits are not yet effective gameplay tuning. This documentation correction does not authorize a code change.

## Durable application-documentation update evidence — 2026-08-29

Updated `docs/04-maps-and-game-modes.md`, `docs/16-grid-map-asset-system.md`, and `docs/15-balance-lab.md`. The guides now document the three supported tile identities and absence of a Heal tile, placement/spawn-safety rules, resolved behavior ownership, fixed-tick server occupancy and boundary timing, movement composition, neutral Damage pulse lifecycle, healing suppression, replication and presentation boundaries, Balance Lab units/ranges, and the current Speed/Slow runtime-wiring limitation. No application source or content was changed. `git diff --check` passed.
