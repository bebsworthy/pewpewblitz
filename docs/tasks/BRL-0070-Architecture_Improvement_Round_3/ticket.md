---
id: BRL-0070
title: Architecture improvement round 3
status: doing
theme:
release:
created: 2026-08-30T18:38:17Z
modified: 2026-08-31T00:11:58Z
closed:
revision: 33dfa2e8180699d3
blocks: []
related: [BRL-0043, BRL-0071, BRL-0072, BRL-0073, BRL-0074, BRL-0075, BRL-0076, BRL-0077, BRL-0078, BRL-0079]
---

# Description

## Executive assessment

The codebase has a strong server-authoritative ECS foundation and is already highly data-driven for balancing existing gameplay primitives. Its main architectural weakness is narrower:

> Content instances are open through data, but new behavior families are still closed behind central enums, static registries, and exhaustive matches.

In practice, adding another weapon preset, map, object instance, bot tuning profile, or VFX profile is often data-only. Adding a new game mode, AI behavior, tile mechanic, object reaction, VFX renderer family, or ultimate behavior still requires coordinated edits across core modules.

Audit basis: current `HEAD` `077dde6`. Both role-isolation checks passed:

- `cargo check --no-default-features --features server --lib`
- `cargo check --no-default-features --features client --lib`

No files or Ticket state were changed.

## 1. High-level architecture and paradigm audit

The dominant data flow is healthy:

```text
RON/config/assets
    ↓ validation + fingerprints
Resolved immutable definitions
    ↓ runtime projection
Focused ECS components/resources
    ↓ fixed authoritative phases
Facts/messages/replication
    ↓
Client-only presentation
```

### Architecture scorecard

| Area | Assessment | Notes |
|---|---|---|
| Client/server isolation | Strong | Feature-gated composition prevents rendering, input, audio, and client assets from entering the server build. |
| ECS modeling | Strong | State is predominantly components/resources; interactions use messages, facts, observers, and explicit schedule phases. |
| Server authority | Strong | Clients submit intent; gameplay mutation remains server-owned. |
| Single Responsibility | Mixed | Most domain modules are focused, but admission, client flow, Balance Lab, and projectile delivery contain large coordinators. |
| Law of Demeter | Mixed-good | Runtime projection has reduced deep loadout traversal, but exclusive `World` handlers and broad query/`ParamSet` systems remain. |
| Open/Closed | Partial | Strong for instances composed from existing primitives; weak for adding new semantic families. |
| Dependency Inversion | Good internally | Neutral authoritative phases and typed facts provide useful inversion. Some consumers still depend directly on concrete mode/tile/object variants. |
| Data-driven balance | Strong, not complete | Most values are externalized. A few genuine player-facing rules remain constants. |

### Particularly strong decisions

- Role gating and composition are clean in [lib.rs](/Users/boyd/wip/brawler/src/lib.rs:30), [server/mod.rs](/Users/boyd/wip/brawler/src/server/mod.rs:96), and [client/mod.rs](/Users/boyd/wip/brawler/src/client/mod.rs:105).
- Fixed-tick ordering is explicit through neutral phases in [gameplay.rs](/Users/boyd/wip/brawler/src/gameplay.rs:16), including deliberate deferred-command boundaries.
- Combat uses typed delivery/payload messages and a staged plan–commit–project transaction in [combat/effects/mod.rs](/Users/boyd/wip/brawler/src/combat/effects/mod.rs:148).
- Weapon recipes are genuinely authored in [weapons.ron](/Users/boyd/wip/brawler/content/catalogs/weapons.ron:1) and validated through [combat/definitions/mod.rs](/Users/boyd/wip/brawler/src/combat/definitions/mod.rs:215).
- Map gameplay profiles cleanly separate authored behavior from runtime state in [map/catalog.rs](/Users/boyd/wip/brawler/src/map/catalog.rs:229).
- Game-type tuning is externally owned by [game-types.ron](/Users/boyd/wip/brawler/config/server/game-types.ron:1).
- Bot numerical policy is extensively externalized in [bots.ron](/Users/boyd/wip/brawler/content/catalogs/bots.ron:1).
- Presentation remains downstream of gameplay facts. Concealment, for example, no longer relies on presentation cues for authority.

### Current extension matrix

| Change | Current result |
|---|---|
| New weapon using existing delivery/effect primitives | Data-only |
| New map using existing asset/gameplay profiles | Data-only |
| New object using existing collision/explode/drop primitives | Data-only |
| New VFX variant in an existing family | Data-only |
| New bot tuning or arbitration weights | Data-only |
| New weapon delivery primitive | Schema and central dispatch edits |
| New game mode | Broad cross-crate edits |
| New effect-tile mechanic | Map, movement, AI, presentation, Balance Lab edits |
| New object terminal reaction | Central enum, mapping, and registry edits |
| New bot behavior | Two central registries plus observation logic |
| New VFX renderer/cue family | Catalog enum, cue translation, mesh/material dispatch edits |

OCP should not mean making the network protocol dynamically typeless. Stable wire contracts should remain intentionally closed and versioned. The goal is to reduce unrelated local fanout around those contracts.

## 2. Large files and code-smell deep dive

There are 30 Rust source files of at least 1,000 lines, accounting for roughly 41% of `src/**/*.rs`. Size alone is not a defect—[abilities/sentry.rs](/Users/boyd/wip/brawler/src/abilities/sentry.rs:1) is large but comparatively cohesive—but several files contain genuinely mixed ownership.

### Priority 1: closed behavior registries

#### Game modes

Modes have focused runtime plugins, but registration remains globally closed:

- `GameMode` parsing and identity: [config.rs](/Users/boyd/wip/brawler/src/config.rs:105)
- fixed descriptor inventory and mode-specific installer signature: [modes.rs](/Users/boyd/wip/brawler/src/modes.rs:53), [modes.rs](/Users/boyd/wip/brawler/src/modes.rs:117)
- operator-rule dispatch: [server/lobby/catalog.rs](/Users/boyd/wip/brawler/src/server/lobby/catalog.rs:219)
- duplicated routing identity: [manifest.rs](/Users/boyd/wip/brawler/packages/brawler-routing/src/manifest.rs:42)
- bot projection: [bots/controller.rs](/Users/boyd/wip/brawler/src/bots/controller.rs:244)

A fourth mode must modify routing, parsing, installation, validation, bots, HUD, and allocation.

#### Bot behaviors

[behaviors.rs](/Users/boyd/wip/brawler/src/bots/behaviors.rs:42) uses function pointers—a good foundation—but production registrations are a private static slice. Registered IDs are duplicated in [profile.rs](/Users/boyd/wip/brawler/src/bots/profile.rs:17), whose validation requires exact coverage.

This is registry-shaped code without a registration seam.

#### Effect tiles and world-object reactions

Effect-tile values are data-driven, but the vocabulary is a closed enum in [effect_tiles.rs](/Users/boyd/wip/brawler/src/map/effect_tiles.rs:14). Concrete variants are matched again by authority, movement, bot navigation, healing, presentation, and Balance Lab.

World-object reactions similarly map a closed enum to two centrally installed handlers in [object_authority.rs](/Users/boyd/wip/brawler/src/map/runtime/object_authority.rs:68). The handler signature also receives `&mut World`, giving reaction code access to every resource and entity.

#### VFX/audio

Existing profile tuning is excellent, but new semantic families remain closed through `VfxCueFamily`, renderer, and material enums in [vfx_catalog.rs](/Users/boyd/wip/brawler/src/client/presentation_3d/combat/vfx_catalog.rs:13), followed by central cue translation in [effects.rs](/Users/boyd/wip/brawler/src/client/presentation_3d/combat/effects.rs:461).

### Priority 1: monolithic match admission

`process_client_hellos` spans [server/mod.rs](/Users/boyd/wip/brawler/src/server/mod.rs:634) through line 981. It handles:

- protocol and content compatibility;
- routed admission and capacity;
- manifest decoding and loadout resolution;
- identity allocation;
- team and spawn selection;
- fighter ECS, physics, replication, and ownership assembly;
- session transitions, diagnostics, and outcomes.

Practice bots independently assemble much of the same fighter shape in [practice.rs](/Users/boyd/wip/brawler/src/server/practice.rs:95).

This is the clearest SRP and testability hotspot. It also contains hardcoded diagnostic build identities at [server/mod.rs](/Users/boyd/wip/brawler/src/server/mod.rs:813).

### Priority 1: duplicated environment damage authority

Map explosions directly mutate combatant health, defeat state, collision layers, effects, facts, and cues in [object_authority.rs](/Users/boyd/wip/brawler/src/map/runtime/object_authority.rs:775).

That creates a second damage/defeat implementation beside combat effects. Future spawn protection, resistances, passives, telemetry, or new target classes could diverge between weapon damage and environment damage.

Map authority should select chain reactions and environmental targets, but combat should own the actual combatant damage commit.

### Priority 2: other heavy coordinators

- [combat/delivery.rs](/Users/boyd/wip/brawler/src/combat/delivery.rs:359): `sweep_composed_projectiles` still combines indexing, lobs, persistent areas, sticky arming, straight sweeps, payload routing, world damage, telemetry, and despawning.
- [client/flow/reducer.rs](/Users/boyd/wip/brawler/src/client/flow/reducer.rs:694): central reducer handles connection, persistence, brawler CRUD/equipment, matchmaking, practice, overlays, and navigation. Every new feature action modifies this switch.
- [server/balance_lab/editor.rs](/Users/boyd/wip/brawler/src/server/balance_lab/editor.rs:508): editor metadata manually mirrors every weapon, delivery, effect, ultimate, passive, and world schema. `add_weapon_fields` alone is about 540 lines.
- [server/balance_lab/mod.rs](/Users/boyd/wip/brawler/src/server/balance_lab/mod.rs:642): transaction application mixes validation, resolution, persistence, ECS mutation, resource replacement, and restart publication.
- [client/flow/screens/brawlers.rs](/Users/boyd/wip/brawler/src/client/flow/screens/brawlers.rs:753): details, creation, editing, and equipment screens repeat substantial shell/header/footer construction.
- [client/presentation_3d/mod.rs](/Users/boyd/wip/brawler/src/client/presentation_3d/mod.rs:461): composition root still owns dynamic objects, map materialization, meshes, characters, and animation implementations.
- [server/lobby/mod.rs](/Users/boyd/wip/brawler/src/server/lobby/mod.rs:1141): the visible schedule composition is good, but the file also owns sessions, profiles, queueing, practice/product formation, and extensive tests.

### Hardcoded policy that should become data

These are real player- or operator-facing policy:

- build budget `12`: [builds/definitions.rs](/Users/boyd/wip/brawler/src/builds/definitions.rs:23)
- attack and damage reveal durations: [concealment/model.rs](/Users/boyd/wip/brawler/src/concealment/model.rs:4)
- diagnostic fighter/weapon/ultimate/passive selection: [server/mod.rs](/Users/boyd/wip/brawler/src/server/mod.rs:813)
- lobby formation timing duplicated across enforcement and advertisement
- several UI/Balance Lab conversions hardcode `60` instead of using `SIMULATION_TICK_HZ`
- animation clip names and attachment offsets in [presentation_3d/mod.rs](/Users/boyd/wip/brawler/src/client/presentation_3d/mod.rs:2005), if visual asset substitution is intended to be data-driven.

These should remain code-owned:

- serialization and collection capacities;
- maximum map or packet sizes;
- engine weapon safety bounds;
- stable protocol versions;
- deterministic ordering and overflow policies.

The fixed `FighterStatProfiles` and `CustomPulseTuning` fields in [builds/definitions.rs](/Users/boyd/wip/brawler/src/builds/definitions.rs:49) are also not additive. This is acceptable while exactly three choices are a product invariant; otherwise they should become bounded ID-keyed vectors.

## 3. Refactoring plan

### Quick wins

1. **Externalize remaining balance policy.**

   Add `point_budget` to `BuildCatalog`, add fingerprinted concealment rules, and create an authored diagnostic/starter loadout. Replace literal 60 Hz conversions with shared timing helpers.

2. **Split admission planning from ECS mutation.**

   Extract pure compatibility/loadout/team/spawn planning and centralize fighter component assembly. Reuse it for human and Practice fighters.

3. **Add semantic AI projections.**

   Replace asset-ID comparisons with components/facts such as `BotObjectRole`, `AttackableObjective`, `ValuablePickup`, and a mode-neutral `BotObjectiveView`.

4. **Reduce query-access complexity.**

   Use `#[derive(QueryData)]` for repeated target views and named snapshot helpers. Do not replace focused components with a mega-component.

5. **Decompose implementation files without hiding composition.**

   Keep plugin installation and schedule ordering in `mod.rs`; move presentation objects, map materialization, characters, lobby sessions, formation, and individual brawler screens into owned submodules.

### Structural changes

1. **Create genuine plugin-populated registries.**

   Start with bot behaviors and terminal reactions. Then add a local `ModeRegistry` while preserving existing wire IDs. Move to extensible routed mode identities only as an explicit protocol migration.

2. **Resolve tiles into capabilities/components.**

   Author data should produce orthogonal components such as `MovementTileEffect`, `PeriodicDamageTileEffect`, `BlocksHealing`, `TerrainTraversalCost`, and `TilePresentationProfile`. Consumers query capabilities rather than match every tile kind.

3. **Unify combatant damage commits.**

   Map explosions should submit typed environment payload/damage plans into the combat effect transaction instead of directly implementing defeat and cue rules.

4. **Make content compatibility contributory.**

   Replace the manual catalog tuple in [content.rs](/Users/boyd/wip/brawler/src/content.rs:44) with sorted, stable fingerprint contributions registered by content plugins.

5. **Co-locate Balance Lab metadata with schemas.**

   Use small descriptors or a narrowly scoped macro to generate validation/editor metadata alongside each typed variant. Avoid an untyped reflection framework.

6. **Decouple presentation producers from renderers.**

   Feature presentation plugins should emit `VfxRequest`/`AudioRequest` messages keyed by stable profile IDs. Generic renderer/audio plugins resolve those requests through startup-built registries.

### Representative before/after

Bot behavior registration:

```rust
// Before
const BEHAVIORS: &[BehaviorRegistration] = &[
    HEALING, PRESSURE, OBJECT, FALLBACK, OBJECTIVES, PICKUPS, RETREAT,
];
```

```rust
// After
#[derive(Resource, Default)]
struct BotBehaviorRegistry(BTreeMap<BotBehaviorId, BotBehaviorFn>);

impl Plugin for PickupBehaviorPlugin {
    fn build(&self, app: &mut App) {
        app.register_bot_behavior(
            BotBehaviorId::PICKUPS,
            pickups::contribute,
        );
    }
}
```

Admission:

```rust
// Before
fn process_client_hellos(/* broad system state */) {
    // validate, resolve, allocate, spawn, replicate, diagnose...
}
```

```rust
// After
struct FighterJoinPlan {
    player: PlayerId,
    team: TeamId,
    spawn: SpawnCandidate,
    loadout: ResolvedMatchLoadout,
    display_name: String,
}

fn plan_join(
    hello: &MatchHello,
    context: &JoinContext,
) -> Result<FighterJoinPlan, MatchJoinRejection>;

fn commit_join(
    commands: &mut Commands,
    plan: FighterJoinPlan,
) -> Entity;
```

Extensible tile capabilities:

```rust
// Before
match tile.behavior {
    Speed { multiplier } => { /* movement */ }
    Slow { multiplier } => { /* movement */ }
    Damage { amount, .. } => { /* authority */ }
}
```

```rust
// After: authored definitions resolve into focused runtime capabilities
commands.spawn((
    EffectTile,
    MovementTileEffect { multiplier },
    TerrainTraversalCost(cost),
    TilePresentationProfile(profile_id),
));

// A damage-tile plugin independently queries PeriodicDamageTile.
```

The central architectural rule I would adopt is:

> Keep authored instances open through data, semantic capabilities open through plugins/components/systems, and protocol contracts deliberately closed and versioned.

That preserves Brawler’s strongest properties—server authority, deterministic ordering, bounded state, and strict role isolation—while delivering the extensibility model you want. Related repository history is tracked in `BRL-0051` and `BRL-0061`; this audit evaluates the post-remediation code rather than repeating their already-fixed findings.
