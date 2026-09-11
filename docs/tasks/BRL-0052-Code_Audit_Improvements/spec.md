The codebase has a strong server-authoritative Bevy foundation, but it does not yet fully satisfy the stated data-driven/Open-Closed constraints. The biggest problems are not directory layout—they are “false configuration” surfaces, closed inventory validation, and gameplay depending on presentation cues.

No production code was changed. The audit is recorded in [BRL-0051](</Users/boyd/wip/brawler/docs/tasks/BRL-0051-Audit_Bevy_Architecture_And_Data_Driven_Extensibility/spec.md>).

## 1. High-Level Architectural View & Paradigm Audit

```text
RON catalogs / map recipes
          ↓
validated authored definitions
          ↓
immutable resolved loadouts / maps
          ↓
server-authoritative ECS runtime
          ↓
authoritative facts and replicated components
          ↓
network cues
          ↓
client HUD / audio / 3D presentation
```

This is the correct overall direction. In particular:

- Client/server role isolation is explicit at the crate root in [src/lib.rs](/Users/boyd/wip/brawler/src/lib.rs:3).
- Client and server construct separate worlds with role-appropriate plugins in [src/client/mod.rs](/Users/boyd/wip/brawler/src/client/mod.rs:548) and [src/server/mod.rs](/Users/boyd/wip/brawler/src/server/mod.rs:1481).
- Fixed-tick ordering and deferred-command boundaries are unusually clear in [src/gameplay.rs](/Users/boyd/wip/brawler/src/gameplay.rs:19), [src/combat/server.rs](/Users/boyd/wip/brawler/src/combat/server.rs:72), and [src/matchplay/mod.rs](/Users/boyd/wip/brawler/src/matchplay/mod.rs:96).
- Authored definitions, resolved loadouts, and runtime state are meaningfully separated in [src/builds/model.rs](/Users/boyd/wip/brawler/src/builds/model.rs:53).
- Maps have a particularly good catalog → resolution → runtime pipeline in [src/map/catalog.rs](/Users/boyd/wip/brawler/src/map/catalog.rs:458) and [src/map/resolution.rs](/Users/boyd/wip/brawler/src/map/resolution.rs:58).
- Combat uses typed messages and authoritative outcome facts rather than a service-object architecture in [src/combat/model.rs](/Users/boyd/wip/brawler/src/combat/model.rs:541) and [src/combat/outcomes.rs](/Users/boyd/wip/brawler/src/combat/outcomes.rs:1).

### Principle scorecard

| Area | Assessment | Main issue |
|---|---|---|
| ECS ownership | Strong | One important gameplay-from-cue dependency |
| Plugin topology | Strong | Some central composition roots know concrete behaviors |
| Single Responsibility | Mixed | Several very large transaction/UI systems |
| Law of Demeter | Mixed | Systems query and mutate too many neighboring domains |
| Open/Closed | Partial | Existing primitive composition is good; new kinds fan out widely |
| Dependency Inversion | Partial | Facts/sets are good; concrete ability/mode dependencies remain |
| Data-driven balance | Insufficient | Several authored-looking values remain code-owned or code-locked |

The Bevy-native direction should be preserved: schedule phases, components, resources, messages, and focused plugins are preferable here to trait-heavy service layers.

## 2. Large File & Code Smell Deep-Dive

### Highest-priority findings

#### P0 — Effect-tile configuration is not actually configurable

The RON catalog contains numeric speed, slow, damage, and cadence values in [map_gameplay_profiles.ron](/Users/boyd/wip/brawler/content/catalogs/map_gameplay_profiles.ron:151). However:

- Validation accepts only the exact constants compiled into Rust in [src/map/effect_tiles.rs](/Users/boyd/wip/brawler/src/map/effect_tiles.rs:53).
- `EffectTileOccupancy` retains only the kind, discarding the resolved numeric behavior in [src/map/effect_tiles.rs](/Users/boyd/wip/brawler/src/map/effect_tiles.rs:85).
- Movement reads the code constants rather than authored values in [src/movement/authority.rs](/Users/boyd/wip/brawler/src/movement/authority.rs:107).

Changing `1250`, `700`, `10`, or `30` in RON either fails validation or does not drive the relevant runtime calculation. This directly violates the balancing constraint.

#### P0 — Catalog validation freezes the current inventory

Weapons are genuinely recipe-driven, but production validation requires exactly seven presets and IDs/profile IDs `1..=7` in [src/combat/definitions/mod.rs](/Users/boyd/wip/brawler/src/combat/definitions/mod.rs:231).

The build catalog similarly requires exactly:

- seven weapon costs;
- eleven ultimates;
- nine passives;
- specific IDs, kinds, and costs.

See [src/builds/definitions.rs](/Users/boyd/wip/brawler/src/builds/definitions.rs:120) and [src/builds/definitions.rs](/Users/boyd/wip/brawler/src/builds/definitions.rs:275).

Consequently, an eighth weapon using only existing supported primitives is rejected until Rust validation is modified. Inventory snapshots belong in tests or release fixtures, not generic production validation.

#### P0 — Ability and passive balance remains code-owned

Several ability definitions are only nominally data-driven:

- `UltimateParameters::Dash` and `Sentry` are marker variants, while actual tuning is compiled into [src/abilities/dash.rs](/Users/boyd/wip/brawler/src/abilities/dash.rs:3) and [src/abilities/sentry.rs](/Users/boyd/wip/brawler/src/abilities/sentry.rs:5).
- Sentry builds a weapon recipe inline, including damage and projectile geometry, in [src/abilities/sentry.rs](/Users/boyd/wip/brawler/src/abilities/sentry.rs:942).
- Ultimate charge is hardcoded in [src/abilities/charge.rs](/Users/boyd/wip/brawler/src/abilities/charge.rs:3).
- Passive durations and formulas are hardcoded in [src/abilities/passives.rs](/Users/boyd/wip/brawler/src/abilities/passives.rs:1).
- Resistance bonuses are embedded in build resolution in [src/builds/definitions.rs](/Users/boyd/wip/brawler/src/builds/definitions.rs:627).
- The adrenaline movement multiplier is a literal `1.15` in [src/movement/authority.rs](/Users/boyd/wip/brawler/src/movement/authority.rs:118).

All player-facing balance values should come from validated definitions/resources. Code should retain only engine safety ceilings and algorithms.

#### P0 — Authoritative gameplay depends on presentation/network output

`CombatCue` is explicitly documented as ordered presentation output in [src/combat/cues.rs](/Users/boyd/wip/brawler/src/combat/cues.rs:167). Nevertheless:

- Concealment reads `CombatOutbox` and `CombatCue::AttackAccepted` to mutate reveal deadlines in [src/concealment/mod.rs](/Users/boyd/wip/brawler/src/concealment/mod.rs:95).
- Self Cloak reads the same outbox to decide whether authoritative cloak state ends in [src/abilities/self_cloak.rs](/Users/boyd/wip/brawler/src/abilities/self_cloak.rs:167).

A cue filtering, delivery, or schema change could therefore alter gameplay. Combat should emit an authoritative attack fact/component; concealment, cloak lifecycle, telemetry, and cue projection should independently consume that fact.

### Open/Closed extension matrix

| Addition | Current state |
|---|---|
| Weapon using existing delivery/effect primitives | Mostly data-driven, but blocked by exact-seven validation |
| New delivery or payload primitive | Requires changes across validation, attack, effects, preview, parts, UI |
| New ultimate | Requires enum/schema, central validation, plugin registration, input, preview, bots, editor |
| New game mode | Mode rules are pluginized, but identity/mapping/HUD/maps/bots fan out centrally |
| New map using known assets | Strongly data-driven |
| New object using known durability/terminal behavior | Data-driven |
| New terminal object behavior | Requires editing the central exclusive transaction |
| New effect tile kind | Closed enum plus movement/runtime/presentation matches |
| New VFX/audio profile | Central code and material/asset fields must change |
| New AI tactic/entity policy | Requires editing the existing priority cascade and closed enums |

Game-mode implementations themselves are good examples of modularity: [Wipeout](/Users/boyd/wip/brawler/src/matchplay/wipeout.rs:95), [Hot Zone](/Users/boyd/wip/brawler/src/matchplay/hot_zone.rs:255), and [Heist](/Users/boyd/wip/brawler/src/matchplay/heist.rs:231) are focused plugins.

The remaining fan-out is visible in:

- the closed mode enum/parser in [src/config.rs](/Users/boyd/wip/brawler/src/config.rs:105);
- central server installation in [src/server/mod.rs](/Users/boyd/wip/brawler/src/server/mod.rs:1517);
- repeated lobby mappings in [src/server/lobby/mod.rs](/Users/boyd/wip/brawler/src/server/lobby/mod.rs:47);
- closed map-mode validation in [src/map/resolution.rs](/Users/boyd/wip/brawler/src/map/resolution.rs:105);
- mode-specific bot observation in [src/bots/controller.rs](/Users/boyd/wip/brawler/src/bots/controller.rs:243).

Some central edit is unavoidable for a new wire-level protocol kind. The goal should be one stable descriptor/registration site plus a new plugin—not coordinated changes across unrelated systems.

### Largest files

| File | Lines | Assessment |
|---|---:|---|
| [server/lobby/mod.rs](/Users/boyd/wip/brawler/src/server/lobby/mod.rs:1) | 3,089 | Pure state, Bevy adapters, formation, identity, timing, and tests should be separated |
| [presentation_3d/mod.rs](/Users/boyd/wip/brawler/src/client/presentation_3d/mod.rs:1) | 2,624 | Composition is clear, but map/fighter/material implementations remain centralized |
| [brawlers.rs](/Users/boyd/wip/brawler/src/client/flow/screens/brawlers.rs:1) | 2,062 | Browsing, creation, editing, equipment, deletion, and previews are one screen module |
| [balance_lab/mod.rs](/Users/boyd/wip/brawler/src/server/balance_lab/mod.rs:1) | 1,986 | Multiple tuning families and transaction concerns |
| [client/shell.rs](/Users/boyd/wip/brawler/src/client/shell.rs:1) | 1,802 | Settings, overlays, persistence, credits, actions, and errors |
| [server/lobby/queue.rs](/Users/boyd/wip/brawler/src/server/lobby/queue.rs:1) | 1,792 | Large but closer to one queue lifecycle |
| [server/mod.rs](/Users/boyd/wip/brawler/src/server/mod.rs:1) | 1,678 | Composition plus session/admission behavior |
| [combat/attack.rs](/Users/boyd/wip/brawler/src/combat/attack.rs:1) | 1,215 | Economy, acceptance, six delivery families, telemetry, world targeting |

Large composition files are not automatically defects. In particular, the explicit plugin and schedule code should stay visible. Implementations should move behind those composition points.

### Heaviest systems/functions

Approximate body spans from a brace-based source scan:

- `resolve_flow_action`: 1,119 lines, [src/client/flow/reducer.rs](/Users/boyd/wip/brawler/src/client/flow/reducer.rs:54).
- `sweep_composed_projectiles`: 527 lines, [src/combat/delivery.rs](/Users/boyd/wip/brawler/src/combat/delivery.rs:121).
- `worker_control_receive`: 384 lines, [src/server/worker.rs](/Users/boyd/wip/brawler/src/server/worker.rs:652).
- `tick_sentries`: 381 lines, [src/abilities/sentry.rs](/Users/boyd/wip/brawler/src/abilities/sentry.rs:638).
- `emit_attack_deliveries`: 377 lines, [src/combat/attack.rs](/Users/boyd/wip/brawler/src/combat/attack.rs:275).
- `apply_composed_records`: 374 lines, [src/combat/effects/application.rs](/Users/boyd/wip/brawler/src/combat/effects/application.rs:210).
- `sample_local_input`: 311 lines, [src/client/input.rs](/Users/boyd/wip/brawler/src/client/input.rs:53).
- `authoritative_composed_fire`: 279 lines, [src/combat/attack.rs](/Users/boyd/wip/brawler/src/combat/attack.rs:808).

The most important decompositions are:

- `resolve_flow_action`: session observation, profile mutation, persistence, brawler editing, queueing, and match flow.
- `sweep_composed_projectiles`: lob flight, splash spawning, sticky behavior, straight collision, damage, telemetry, and cleanup.
- `worker_control_receive`: inbound receive, three outbound queues, backpressure/error handling, and heartbeat.
- `tick_sentries`: owner lookup, visibility, targeting, LOS, fire validation, weapon construction, and spawning.

There is also repeated activation validation across Dash, Sentry, Cloak, Scan, Demolition, and field abilities, and repeated literal input-staleness value `12` despite `MovementTuning` already owning that policy.

## 3. Refactoring Plan & Opportunities

### Quick wins — do first

1. **Make configuration truthful.**

   - Remove exact inventory counts and expected-cost arrays from production validators.
   - Validate uniqueness, sorted IDs, supported behavior kinds, bounds, references, and encoded size.
   - Keep current inventory assertions in tests.

2. **Move remaining balance into authored data.**

   - Add complete `DashParameters` and `SentryParameters`.
   - Add passive/charge tuning to the build catalog.
   - Load `BotProfile` from `content/catalogs/bots.ron`.
   - Move match defaults/profile values into the existing operator game-type configuration.
   - Replace effect-tile equality validation with bounded numeric validation.

3. **Preserve resolved tile behavior at runtime.**

   Store the authored multiplier/damage/cadence in the resolved occupancy/runtime component. Movement and damage systems must consume those values.

4. **Introduce an authoritative attack fact.**

   Remove all gameplay reads from `CombatOutbox`. Add regression tests proving cue filtering cannot affect reveal or cloak state.

5. **Remove duplicated literals and predicates.**

   - Use `SIMULATION_TICK_HZ` instead of literal `60`.
   - Use shared authoritative input freshness policy.
   - Extract shared world-target payload construction.
   - Extract shared ability activation gating.

### Structural changes

6. **Decompose systems by transaction stage.**

   Use `collect → validate → plan → commit → publish`, while retaining one schedule-facing coordinator where precedence or atomicity matters.

7. **Split ability behavior plugins.**

   `DashAbilityPlugin`, `SentryAbilityPlugin`, and other behavior plugins should register into stable `AbilitySet` phases. A new ability should add its module/plugin and serialized definition, without editing unrelated ability code.

8. **Introduce neutral authoritative schedule phases.**

   Promote phases such as delivery, effects, environment, objectives, visibility, publication, and finalization into `gameplay.rs`. Domain plugins retain private sub-phases.

9. **Centralize mode descriptors.**

   One descriptor should associate stable mode ID/key, rules resolver, compatible-map policy, server installer, and optional HUD/bot projection. Preserve fixed wire enums where required, but remove repeated mappings.

10. **Make object terminal reactions additive.**

    Keep the exclusive atomic world-damage transaction, but have it produce deterministic terminal plans. Focused handlers execute Explosion, DropPickup, or future reactions.

11. **Move VFX/audio policy into catalogs.**

    Keep code-owned renderer families, but data-drive material key, mesh family, size, lifetime, audio key, and concurrency cap. A genuinely new renderer remains a plugin change.

12. **Evolve bots toward additive deterministic behaviors.**

    Each behavior produces a bounded scored intent. A stable arbiter chooses by score and behavior ID and commits one `FighterInput`. This permits mode/object/ability policy plugins without rewriting `choose_intent`.

### Before vs. after

#### Catalog validation

```rust
// Before
if self.presets.len() != 7 {
    return Err("requires exactly seven weapon presets".into());
}
if !matches!(preset.id.0, 1..=7) {
    return Err("invalid preset".into());
}
```

```rust
// After
validate_sorted_unique(self.presets.iter().map(|p| (p.id, &p.key)))?;

for preset in &self.presets {
    preset.configuration.validate(&self.recipe_policy, engine_limits)?;
    validate_presentation_reference(preset.presentation_profile_id)?;
}
```

#### Authoritative fact instead of presentation cue

```rust
// Before: presentation output becomes gameplay input
let attacked = outbox.0.iter().any(|cue| {
    matches!(cue, CombatCue::AttackAccepted { source, .. } if source == network_id)
});
```

```rust
#[derive(Resource, Default)]
struct AttackCommittedFacts(Vec<AttackCommittedFact>);

// Combat commit stage
attack_facts.0.push(AttackCommittedFact {
    tick,
    source: network_id,
    attack_id,
});

// Concealment / cloak lifecycle
let attacked = attack_facts.0.iter().any(|fact| fact.source == *network_id);

// Publication is separately derived from the same fact.
combat_outbox.0.push(CombatCue::AttackAccepted { /* ... */ });
```

#### Data-owned deployable tuning

```rust
// Before
UltimateParameters::Sentry;
const SENTRY_FIRE_INTERVAL_TICKS: u64 = 30;

let shot = WeaponRecipe {
    delivery: DeliveryMethod::Straight {
        speed: 900.0,
        radius: 6.0,
        range: 480.0,
        lifetime_ticks: 32,
        // ...
    },
    // hardcoded damage
};
```

```rust
// After
UltimateParameters::Sentry {
    lifetime_ticks: u64,
    maximum_health: u16,
    radius_milliunits: u32,
    acquisition_range_milliunits: u32,
    acquisition_interval_ticks: u64,
    shot: WeaponRecipe,
}

let config = SentryConfig::try_from(loadout.ultimate.parameters)?;
spawn_projectile_from_recipe(commands, source, &config.shot, tick);
```

#### Additive ability plugin

```rust
pub struct DashAbilityPlugin;

impl Plugin for DashAbilityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            activate_dash.in_set(AbilitySet::Activation),
        )
        .add_systems(
            FixedUpdate,
            advance_dash.in_set(AbilitySet::Movement),
        );
    }
}
```

This preserves explicit Bevy ordering while making new behavior additive.

## Recommended execution order

1. Truthful catalog validation and effect-tile runtime values.
2. Authoritative attack facts replacing cue reads.
3. Dash/Sentry/passive/bot balance migration.
4. Characterization tests around the large systems.
5. Decompose `resolve_flow_action`, projectile sweep, worker control, and sentry logic.
6. Ability/mode/object extension registries and neutral schedule phases.
7. VFX/audio catalogs and additive bot policies.

Verification should include catalog mutation tests, role-specific builds, schedule ambiguity checks, focused ECS tests for each extracted transaction, routed authority tests, protocol/content fingerprint assertions, and native presentation checks only where visual policy changes.
