---
id: BRL-0061
title: Architecture improvement
status: doing
theme:
release:
priority: none
created: 2026-08-30T08:01:29Z
modified: 2026-08-30T13:49:31Z
closed:
revision: 56b1b473428f4a05
blocks: []
related: []
---

# Description

# Executive verdict

Brawler has a strong server-authoritative ECS foundation, clear client/server feature isolation, stable network identities, and increasingly mature data catalogs. Content built from existing primitives—weapon recipes, maps, object profiles, bot timing, abilities, audio, and VFX profiles—is generally data-driven.

It does not yet fully satisfy the stricter Open/Closed goal for introducing new mechanics. The most important current problem is more immediate: live code-authored fighter and weapon defaults coexist with the canonical RON catalogs and materially disagree with them. That creates two production gameplay truths.

Audit performed read-only at `97d9cc1`; no source or Ticket state was changed.

## 1. High-Level Architectural View & Paradigm Audit

### Current architecture

```text
RON/config/assets
        │
        ▼
Content plugins → validation/resolution → resolved loadouts/maps/policies
                                             │
Client intent ──protocol──► authoritative fixed-tick ECS
                                             │
                         outcomes/facts/cues/replicated components
                                             │
                                             ▼
                                 client HUD/audio/3D presentation
```

The broad shape is good:

- Client and server composition is feature-gated in [Cargo.toml](/Users/boyd/wip/brawler/Cargo.toml:13) and [src/lib.rs](/Users/boyd/wip/brawler/src/lib.rs:1). The server dependency graph does not pull rendering, windowing, audio, or client input.
- Server and client use separate Bevy worlds and explicit plugins in [src/server/mod.rs](/Users/boyd/wip/brawler/src/server/mod.rs:1454) and [src/client/mod.rs](/Users/boyd/wip/brawler/src/client/mod.rs:548).
- Fixed-tick authoritative phases and deferred-command boundaries are visible in [src/gameplay.rs](/Users/boyd/wip/brawler/src/gameplay.rs:6) and [src/combat/server.rs](/Users/boyd/wip/brawler/src/combat/server.rs:40).
- Authored selection, resolved loadouts, and mutable runtime state are separated in [src/builds/model.rs](/Users/boyd/wip/brawler/src/builds/model.rs:53).
- Combat effects already follow a valuable staged model—collection, planning, application, and commit—in [src/combat/effects/mod.rs](/Users/boyd/wip/brawler/src/combat/effects/mod.rs:1).
- Authoritative attack facts now decouple gameplay consumers from presentation cues through [AcceptedAttackFacts](/Users/boyd/wip/brawler/src/combat/outcomes.rs:7).

### Plugin topology concern

Transport/session plugins currently install gameplay plugins:

- [ServerNetworkPlugin](/Users/boyd/wip/brawler/src/server/mod.rs:354) installs combat, concealment, abilities, and routed-worker gameplay at [line 413](/Users/boyd/wip/brawler/src/server/mod.rs:413).
- [ClientNetworkPlugin](/Users/boyd/wip/brawler/src/client/session/mod.rs:119) installs combat, maps, queue, and profile behavior.
- [ProtocolPlugin](/Users/boyd/wip/brawler/src/protocol.rs:686) installs content plugins as well as registering wire types.

This reverses the desired ownership direction: application composition should depend on transport, while transport should not decide which gameplay exists.

Recommended topology:

```text
ServerAppPlugin
├── GameplayContentPlugin
├── ServerGameplayPlugin
├── MatchModePlugins
├── ServerTransportPlugin
└── DiagnosticsPlugin
```

The same split should exist on the client between replicated gameplay state, session transport, and presentation.

### ECS principle scorecard

| Principle | Assessment | Main evidence |
|---|---|---|
| ECS separation | Strong | Server authority, client intent, replicated state, presentation cues, and authored data are distinct. |
| Single Responsibility | Mixed–strong | Most domains have focused plugins and schedules; projectile delivery and effect application remain large mixed transactions. |
| Law of Demeter | Mixed | Most systems use explicit queries, but combat frequently reaches through the full `ResolvedMatchLoadout` aggregate. |
| Open/Closed | Good for content, partial for mechanics | Existing recipes are additive; new semantic families still expand central enums and matches. |
| Dependency Inversion | Mixed | Facts, messages, phases, and stable IDs are good abstractions; transport-to-gameplay composition and concrete cross-domain queries weaken it. |
| Data-driven balance | Partial | RON coverage is substantial, but live legacy defaults and several tactical/lifecycle constants remain code-owned. |

### Open/Closed extension matrix

| Extension | Current state |
|---|---|
| Weapon using existing delivery/effect primitives | Good: add RON preset and build-cost data. |
| New delivery or payload mechanic | Closed: edit enums, validation, attack dispatch, effect application, bots, and presentation. |
| Map using existing assets and behaviors | Very good: discovered and loaded from map/catalog data. |
| Tile using an existing speed/slow/damage behavior | Good: values are authored and retained in runtime occupancy. |
| New tile behavior | Closed: edit behavior enum, runtime, bots, presentation, and Balance Lab schema. |
| Object using an existing explosion/pickup reaction | Mostly data-driven. |
| New object terminal reaction | Closed behind a private central registry and exhaustive mapping. |
| New game-type instance using an existing mode | Good: operator configuration owns maps, team sizes, timings, and objectives. |
| New game mode | Partial: requires changes across gameplay, routing, wire enums, lobby rule summaries, bots, and UI. |
| VFX/audio profile using an existing family | Data-driven. |
| New renderer/material/audio key | Closed code-owned enums and handle fields. |
| Bot tuning | Mostly RON-driven. |
| New AI behavior | Requires editing the private static behavior array and central observation projections. |

A protocol-visible mode or effect cannot realistically require zero schema change. The useful Open/Closed target is that the schema change is local and the behavior arrives through one plugin/registration—not that wire compatibility becomes dynamically typed.

## 2. Large File & Code Smell Deep-Dive

### Critical: two conflicting gameplay catalogs

The strongest finding is the live fallback path formed by [FighterDefinitions](/Users/boyd/wip/brawler/src/combat/mod.rs:267) and [WeaponDefinitions](/Users/boyd/wip/brawler/src/combat/mod.rs:327).

They are initialized in production by [ServerCombatPlugin](/Users/boyd/wip/brawler/src/combat/server.rs:53), overwrite movement tuning at [line 159](/Users/boyd/wip/brawler/src/combat/server.rs:159), and remain fallback inputs for match activation, respawn, pickups, admission, Practice, HUD, and Balance Lab.

They already disagree materially:

| Value | Code default | Canonical RON |
|---|---:|---:|
| Default fighter health | 100 | 1000 |
| Pulse capacity | 6 | 4 |
| Pulse damage | 25 | 200 |
| Projectile speed | 900 | 500 |
| Projectile radius | 6 | 2 |
| Projectile range | 900 | 320 |

See [src/combat/mod.rs:271](/Users/boyd/wip/brawler/src/combat/mod.rs:271), [builds.ron:5](/Users/boyd/wip/brawler/content/catalogs/builds.ron:5), and [weapons.ron:35](/Users/boyd/wip/brawler/content/catalogs/weapons.ron:35).

The fallback is visible directly in [fighter_runtime_values](/Users/boyd/wip/brawler/src/matchplay/lifecycle.rs:47) and match activation at [src/matchplay/server.rs:661](/Users/boyd/wip/brawler/src/matchplay/server.rs:661).

This should be treated as a correctness issue, not merely cleanup.

### Largest production modules

File size alone is not a defect; explicit schedule composition should remain visible. These files, however, contain ownership boundaries worth extracting:

| File | Lines | Assessment |
|---|---:|---|
| [server/lobby/mod.rs](/Users/boyd/wip/brawler/src/server/lobby/mod.rs:1) | 3,073 | Mixes lobby state, authentication/profile storage, queue orchestration, match formation, activation, and grant delivery. |
| [client/presentation_3d/mod.rs](/Users/boyd/wip/brawler/src/client/presentation_3d/mod.rs:1) | 2,627 | Mixes plugin composition, map/object presentation, fighter import/animation, materials, and projected UI. |
| [server/balance_lab/mod.rs](/Users/boyd/wip/brawler/src/server/balance_lab/mod.rs:1) | 2,158 | Snapshot, mutation, validation, and persistence boundaries could be separated. |
| [client/flow/screens/brawlers.rs](/Users/boyd/wip/brawler/src/client/flow/screens/brawlers.rs:1) | 2,062 | List, detail, create, edit, equipment, delete, and preview screens share one file. |
| [client/flow/reducer.rs](/Users/boyd/wip/brawler/src/client/flow/reducer.rs:1) | 1,808 | Improved substantially, but one coordinator still owns nearly every flow resource. |
| [server/mod.rs](/Users/boyd/wip/brawler/src/server/mod.rs:1) | 1,610 | Composition belongs here; admission and session implementation can move behind focused modules. |

### Heaviest system/function hotspots

| Approx. lines | Function | Main concern |
|---:|---|---|
| 382 | [apply_composed_records](/Users/boyd/wip/brawler/src/combat/effects/application.rs:211) | Gating, passives, damage, healing, conditions, defeat, telemetry, facts, and cues in one transaction. |
| 377 | [emit_attack_deliveries](/Users/boyd/wip/brawler/src/combat/attack.rs:282) | Central delivery-family dispatcher; lobbed and splash creation are structurally similar. |
| 358 | [process_client_hellos](/Users/boyd/wip/brawler/src/server/mod.rs:601) | Admission, validation, identity/session setup, and response handling. |
| 325 | [resolve_flow_action](/Users/boyd/wip/brawler/src/client/flow/reducer.rs:694) | Broad resource tuple and cross-screen dispatch. |
| 324 | [sweep_composed_projectiles](/Users/boyd/wip/brawler/src/combat/delivery.rs:359) | Indexing, trajectory, collision, splash limits, sticky handling, expiry, damage, and publication. |
| 311 | [sample_local_input](/Users/boyd/wip/brawler/src/client/input.rs:53) | Keyboard, mouse, gamepad, automation, and context shaping. |
| 286 | [authoritative_composed_fire](/Users/boyd/wip/brawler/src/combat/attack.rs:823) | Admission, economy, caps, ID allocation, delivery dispatch, telemetry, facts, and cues. |
| 276 | [capture_observations](/Users/boyd/wip/brawler/src/bots/controller.rs:93) | Converts many concrete game concepts into a closed bot worldview. |

`apply_composed_records` should not simply be split into unordered Bevy systems; its atomic ordering matters. The right extraction is an immutable `plan → commit → project` transaction.

### Repeated logic

Ultimate activation repeats the same checks across Dash, Sentry, Self Cloak, Reveal Scan, concealment fields, demolition, elemental fields, and Big Blob:

- input freshness;
- activation latch;
- defeated/inactive state;
- charge availability;
- generation reservation;
- rejection telemetry.

Examples include [dash.rs:214](/Users/boyd/wip/brawler/src/abilities/dash.rs:214), [sentry.rs:381](/Users/boyd/wip/brawler/src/abilities/sentry.rs:381), and [reveal_scan.rs:114](/Users/boyd/wip/brawler/src/abilities/reveal_scan.rs:114).

This is a good candidate for a small pure helper, not an ability service framework.

### Remaining hardcoded policy

Values that should move to validated configuration include:

- Bot arbitration weights and commitment bonus in [bots/behaviors.rs:9](/Users/boyd/wip/brawler/src/bots/behaviors.rs:9).
- Spawn protection and completed-match input lock in [matchplay/server.rs:53](/Users/boyd/wip/brawler/src/matchplay/server.rs:53).
- Recent hostile damage credit window in [matchplay/wipeout.rs:78](/Users/boyd/wip/brawler/src/matchplay/wipeout.rs:78).
- Heist critical-health feedback threshold in [matchplay/heist.rs:14](/Users/boyd/wip/brawler/src/matchplay/heist.rs:14).
- Hot Zone near-combat telemetry radius in [matchplay/hot_zone.rs:15](/Users/boyd/wip/brawler/src/matchplay/hot_zone.rs:15).
- Minimum lob duration in [combat/attack.rs:84](/Users/boyd/wip/brawler/src/combat/attack.rs:84).
- VFX base geometry and anchoring in [combat/effects.rs:244](/Users/boyd/wip/brawler/src/client/presentation_3d/combat/effects.rs:244).

Conversely, engine limits, bounded queue capacities, stable IDs, collision layers, and numeric safety ceilings should remain code-owned.

There are also misleading configuration surfaces:

- Tile default constants in [map/effect_tiles.rs:8](/Users/boyd/wip/brawler/src/map/effect_tiles.rs:8) coexist with the actual RON values.
- Effect-tile presentation matches concrete asset IDs in [presentation_3d/mod.rs:1709](/Users/boyd/wip/brawler/src/client/presentation_3d/mod.rs:1709).
- Weapon `presentation_profile_id` is propagated through combat, but no matching client presentation consumer was found.

## 3. Refactoring Plan & Opportunities

### Quick wins

1. **Eliminate the dual balance source.**

   Require a resolved loadout/runtime specification in production lifecycle paths. Remove `FighterDefinitions` and `WeaponDefinitions` as balance fallbacks; retain only genuinely invariant identity or collision data.

2. **Move residual policy to existing catalogs.**

   Add match lifecycle and bot arbitration sections to validated RON. Remove unused tile defaults and label any unavoidable code value explicitly as an engine bound, not a balance default.

3. **Extract one shared ultimate activation gate.**

   Centralize common admission checks while leaving ability-specific target validation and execution in each plugin.

4. **Make configuration truthful.**

   Either consume and validate `presentation_profile_id` or remove it. Replace presentation and bot checks on exact map asset IDs with authored presentation/AI tags.

5. **Use named Bevy query types.**

   Convert the largest anonymous query/`ParamSet` signatures into `#[derive(QueryData)]` views. This improves readability without hiding schedule dependencies.

### Structural changes

1. **Separate application composition from networking.**

   Introduce `ServerGameplayPlugin`, `ClientReplicatedGameplayPlugin`, and `GameplayContentPlugin`. Transport plugins should own links, sessions, serialization, and replication only.

2. **Project resolved loadouts into focused runtime components.**

   At build commitment/spawn, install components such as:

   - `FighterVitalStats`
   - `MovementProfile`
   - `WeaponRuntimeSpec`
   - `CombatDefense`
   - `DamageModifiers`
   - `ConditionResistances`

   Combat and movement systems then depend on the capabilities they use rather than traversing `ResolvedMatchLoadout`.

3. **Decompose delivery and effect families.**

   Keep typed serialized enums. Move each delivery/effect family into focused `plan_*` and `commit_*` handlers, with one deterministic coordinator responsible for ordering and publication.

4. **Convert static registries into plugin-populated resources.**

   Prioritize bot behaviors, terminal reactions, VFX renderers, audio assets, and local mode descriptors. Validate duplicate IDs and required coverage at startup.

5. **Make AI consume affordances, not content identities.**

   Resolve map objects into components such as `BotHealingTarget`, `BotHazard`, `BotDestructible`, and `BotObjectiveTarget` rather than matching `OIL_BARREL_ASSET` and `TREASURE_CHEST_ASSET`.

6. **Split mixed large modules by ownership.**

   Keep `mod.rs` files as schedule/composition surfaces. Extract lobby admission, formation, grants, presentation materials, map-object presentation, fighter presentation, and individual brawler screens.

### Before vs. after: remove balance fallback

```rust
// Before: two possible gameplay truths
let values = loadout.map_or_else(
    || fighter_runtime_values(fighter_id, build, &fighters, &weapons),
    |loadout| Some((
        loadout.fighter_stats.maximum_health,
        loadout.primary_weapon.recipe.economy.capacity(),
    )),
);
```

```rust
// After: resolved once, then required by runtime systems
#[derive(Component)]
struct FighterRuntimeSpec {
    maximum_health: u16,
    ammunition_capacity: u8,
    movement_speed: f32,
}

fn reset_fighter(spec: &FighterRuntimeSpec, /* runtime state */) {
    // No balance fallback and no catalog traversal.
}
```

### Before vs. after: ability activation

```rust
// Before: repeated in every ultimate
if defeated.is_some() || !input_is_fresh || latch.is_active() {
    telemetry.reject(...);
    return;
}
if charge.current < charge.maximum {
    telemetry.reject(...);
    return;
}
```

```rust
struct ActivationContext {
    active: bool,
    defeated: bool,
    input_fresh: bool,
    latched: bool,
    charge: u16,
    required_charge: u16,
}

fn evaluate_activation(
    context: ActivationContext,
) -> Result<ActivationPermit, AbilityRejectionReason> {
    // Pure, deterministic common policy.
}

// The ability plugin still owns targeting, execution, and its runtime components.
let permit = evaluate_activation(context)?;
activate_dash(permit, target, parameters);
```

### Before vs. after: correct plugin dependency direction

```rust
// Before
impl Plugin for ServerNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ServerCombatPlugin,
            ServerConcealmentPlugin,
            ServerAbilityPlugin,
            RoutedWorkerPlugin,
        ));
    }
}
```

```rust
// After
impl Plugin for ServerAppPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            GameplayContentPlugin,
            ServerGameplayPlugin,
            ServerTransportPlugin,
        ));
    }
}
```

## Recommended priority

1. Remove the conflicting fighter/weapon fallback catalogs.
2. Add cross-catalog startup validation and regression tests proving runtime values equal authored values.
3. Move bot and match lifecycle policy into RON.
4. Extract shared ability admission.
5. Split transport from gameplay composition.
6. Project resolved loadouts into focused runtime components.
7. Decompose delivery/effect transactions.
8. Introduce plugin-populated behavior/reaction/rendering registries.
9. Split large mixed-owner modules.

Several findings from the earlier BRL-0051/0052 audits are now genuinely fixed: exact catalog cardinality locks are gone, effect-tile values reach runtime, authoritative attack facts replaced cue-driven logic, significant ability/bot tuning moved to data, and the worker/sentry/flow coordinators were substantially decomposed. The plan above targets the remaining current-HEAD risks rather than reopening those resolved issues.
