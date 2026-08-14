# Milestone 05 — Weapon composition and preset selection

## Tracking

| Field | Value |
|---|---|
| Version | v1 — gameplay MVP |
| Roadmap | [roadmap.md](./roadmap.md) |
| Status | Specification review |
| Specification validation | Awaiting user validation |
| Implementation | Not started |
| Verification | Not started |
| User validation/playtest | Not started |

Update this table and the roadmap together whenever the milestone changes phase.

## Outcome

Four server-authoritative primary-weapon presets can be selected before entering the networked
combat sandbox: the existing pulse sidearm, a short-range scatter cannon, a fixed-range lobbed arc
launcher, and an impact blade. Each is an ordinary typed weapon recipe resolved through the same
server-owned validation path intended for later player-authored recipes. They share weapon economy,
attack acceptance, payload application, attribution, lifecycle, replication, cues, and telemetry
while using focused delivery systems where their geometry genuinely differs.

This milestone proves the customization seam but does not expose player editing. It adds exactly the
firing, delivery, targeting, payload, and immediate-effect primitives required by the first four
presets, plus a deterministic resolver that is independent of preset identity. M08 will select the
first editable axes and balance-budget policy. Persistent brawler arsenals, acquisition,
entitlements, currencies, loot, and unlocks remain later product work.

Milestone 04 is complete by explicit user approval with its recorded fixes. Milestone 05
implementation begins by re-running that accepted baseline. Unobserved M03/M04 hardware cases are
not retroactively marked as passed; M05's own controller and display checks gather fresh evidence
for the expanded weapon system.

## Source requirements

- [Product direction](../../00-product-direction.md): combat readability, meaningful tradeoffs,
  content through composition, short feedback cycles, and network-first simulation.
- [Fighter model](../../02-fighter-model.md): content/rules, player-authored build recipe,
  server-resolved match loadout, and runtime state remain separate; v1 balance stays bounded.
- [Weapons and abilities](../../03-weapons-and-abilities.md): input/firing/delivery/collision/
  payload composition; straight and ballistic trajectories; direct, area, knockback, and slow
  payloads; stable presentation references; the first four preset recipes.
- [Gameplay MVP](../../05-gameplay-mvp.md): four weapon presets, future bounded non-preset
  configuration, two initial trajectory families,
  controller-first combat, keyboard/mouse parity, readable counterplay, and authoritative outcomes.
- [Network architecture](../../08-network-architecture.md): clients send selection and fire intent;
  the server owns recipe lookup/resolution, selected loadout, attacks, projectiles, targeting,
  payloads, effects, damage, defeat, and cleanup.
- [Version 1 roadmap](./roadmap.md): Milestone 05 deliverable, scope, verification, and exit criteria.
- [Milestones 01–04](./milestone-04.md): exact feature topology, session lifecycle, fixed-tick input,
  canonical pose, Avian collision layers, authoritative movement, combat schedule, stable identity,
  durable replication, cue recovery, telemetry, impairment harness, and recorded lessons.

## Scope boundaries

### In scope

- one validated, build-embedded RON weapon content catalog with typed capability/safety bounds,
  stable numeric preset and presentation-profile IDs, semantic content fingerprinting, and exactly
  four built-in preset recipes;
- explicit separation between content/rules, `WeaponRecipe`, immutable server-resolved weapon,
  selected build identity, and per-fighter runtime economy/effects;
- deterministic structural and fighter-context activation resolution used for all four presets
  plus non-preset configuration test fixtures;
- single and deterministic spread firing patterns;
- straight swept projectiles, non-colliding top-down lobbed flight with a server-owned landing
  point, landing explosion, and an instantaneous terrain-occluded melee sector;
- direct and circular-area target selection plus damage, linear damage falloff, knockback, and one
  basic slow immediate effect;
- explicit friendly, owner, self-damage, terrain-occlusion, stacking, refresh, expiry, defeat,
  reset, disconnect, and late-join behavior;
- a connection-owned, server-validated initial sandbox weapon selection phase and client selection
  overlay; weapon switching after confirmation is not allowed;
- controller- and keyboard/mouse-readable aim/range previews for all four weapons, including scatter
  cone, launcher landing/explosion indicator, and blade sector;
- stable-ID transient combat feedback, durable current weapon/effect/projectile state, and per-weapon
  telemetry that distinguishes accepted attacks, emitted deliveries, contacts, and damage;
- focused module decomposition inside the existing package, deterministic tests, two-client
  network tests, impairment runs, performance checks, and window/controller visual checks.

### Out of scope

- formal lobby, ready check, team allocation, match phases, score, respawn, victory, or restart;
- player-facing recipe editing, build point/cost policy, ultimate/passive/active item slots, brawler
  presets, or weapon switching during a sandbox life; bounded editing belongs to M08;
- accounts, persistent brawler arsenal/storage, saved build revision history, ownership,
  entitlement, currency, loot, unlock, or progression rules;
- file watching or live hot-reload of authoritative gameplay definitions;
- runtime content download, mod support, content signing, remote authoring, or a public scripting API;
- charge, channel, release-to-fire, beam, trap, summon, turret, persistent zone, healing, shield,
  damage-over-time, stun, root, reveal, or terrain-modification payloads;
- accumulating status meters, threshold triggers, status resistance/immunity, or cold/freeze;
- bouncing, homing, curved steering, ricochet, boomerang return, piercing, splitting, delayed
  projectiles, or projectile-projectile collision;
- random pellet spread, critical hits, armor, lifesteal, manual reload, per-shell reload, ammo
  pickups, or reload cancellation;
- projectile/attack prediction, lag compensation, server rewind, or changing the open Milestone 03
  prediction decision without new comparison evidence;
- production art, audio, camera shake, vibration, aim assist, accessibility settings, or the authored
  arena/presentation baseline planned for Milestone 06;
- another crate, service/repository layer, generic behavior graph, or generalized effect engine.

## Research questions and conclusions

### Versions, dependencies, feature isolation, and authored data

- [x] Retain Rust 1.95, Bevy `=0.19.1`, Lightyear `=0.29.0`, Avian 2D `=0.7.0`, the existing
  package/feature topology, and the current 60 Hz authoritative tick.
- [x] Add direct `ron = "0.12"` use for typed catalog parsing; the locked graph currently resolves
  RON 0.12.2 on the client through Bevy. The server may depend on RON without enabling
  `bevy_asset`, file watching, rendering, windowing, audio, or device input.
- [x] Store the gameplay catalog at `content/v1/weapons.ron` and compile it into both roles with
  `include_str!`. Parse and validate synchronously before either role connects or spawns gameplay.
  Data edits require a rebuild but not combat-code changes or runtime file paths.
- [x] Compute a semantic `GameplayContentFingerprint` from the validated typed catalog serialized
  in canonical ID order, not from raw RON bytes. Comments and whitespace therefore do not change
  compatibility; a rule value, enum, or ID does. Use postcard bytes and an explicit fixed-seed
  64-bit FNV-1a compatibility hash; this detects accidental mismatch and is not a
  cryptographic integrity claim.
- [x] Canonicalization includes an explicit fingerprint-format version, sorts every ID-keyed
  collection, rejects nonfinite floats, and normalizes `-0.0` to `0.0`. Recipe fingerprints exclude
  preset identity and display metadata; the global content fingerprint includes schema, engine
  limits, catalog policy, presets, recipes, and presentation-profile references.
- [x] Extend `ClientHello` and join rejection with the content fingerprint. Registry fingerprints
  prove wire-type compatibility; the content fingerprint separately prevents server/client numeric
  or presentation-rule disagreement.
- [x] Defer live authoritative hot-reload. Applying a changed cooldown, payload, or trajectory to
  existing fighters/projectiles on only one process is unsafe, and Bevy file watching would also
  break the server's accepted no-client-asset feature contract. A later design may adopt catalog
  revisions at a server-owned match boundary.

### Composition boundary and module shape

- [x] Replace the pulse-only flat definition with a typed `WeaponRecipe` containing concrete enums
  for economy, firing pattern, delivery, target selection, and payload effect. Validation permits
  only combinations and bounded values implemented in this milestone.
- [x] Treat Pulse, Scatter, Arc, and Blade as named `WeaponPresetDefinition` records containing
  recipes, not as behavior enum variants or permanent classes. Combat algorithms never branch on a
  preset ID.
- [x] Add a pure deterministic configuration resolver that accepts a recipe and approved
  presentation-profile reference without requiring a preset identity. Structural validation is
  definition-independent; activation also validates the configuration against the selected fighter
  definition (including muzzle clearance). M05 preset selection looks up the server's configuration
  before invoking this resolver; clients cannot submit numeric specs.
- [x] Put non-negotiable allocation and numeric ceilings in code-owned `EngineWeaponLimits`.
  Catalog policy may disable capabilities or narrow a ceiling, but malformed content cannot widen
  the engine limits. This keeps future player recipes bounded even if policy data is wrong.
- [x] Separate structural/capability validation now from balance-budget and entitlement validation
  later. The seam must allow M08 to add budget policy and future arsenal services to add ownership
  checks without moving combat authority out of the server `World`.
- [x] Do not create a generic interpreter, dynamic behavior graph, trait-object hierarchy, or one
  system per weapon. Shared systems accept attacks, advance economy, resolve payloads/effects,
  replicate state, and record telemetry. Straight/lobbed/melee delivery geometry remains in focused
  systems because the algorithms genuinely differ.
- [x] Split the growing `src/combat.rs` into a `src/combat/` module family during implementation:
  definitions, attack/economy, delivery, payload/effects, telemetry/protocol-facing types, server
  composition, and client presentation. Keep one public `combat` module and the existing package.
- [x] Continue using Avian only for collider geometry and spatial queries. Weapon cadence,
  trajectory progress, area rules, payload order, slow, knockback, and attribution remain Brawler
  ECS rules.

### Attack identity, economy, and firing patterns

- [x] Allocate one stable `AttackId` for every accepted trigger/cooldown action. A joint checked
  reservation commits the attack ID and attack-accepted cue ID together before resource/cooldown
  mutation; exhaustion rejects the action without advancing either allocator or partially mutating
  gameplay. A spread attack
  emits multiple deliveries identified by `(AttackId, delivery_index)`; it is still one attack for
  economy, primary hit-rate, muzzle feedback, and use telemetry.
- [x] Replace pulse-specific `ShotId` semantics rather than pretending each pellet, lob, and melee
  swing is an independent player action. Projectile sources carry attack identity and delivery
  index; combat outcomes retain `CombatEventId` ordering.
- [x] Keep held-primary-fire semantics. The server accepts at most one attack per fighter per tick
  after selection, alive/input-freshness, resolved-recipe, resource, cooldown, and reload/recharge
  validation. Clients cannot supply attack IDs, spread angles, landing points, targets, payloads,
  damage, or effect state.
- [x] Support magazine and charge economy with the same replicated `WeaponState`: current resource
  count plus ready/cooldown/reloading-or-recharging phase and authoritative deadline. Consuming the
  last unit begins full refill; completion precedes that tick's held-fire decision, preserving M04.
- [x] Spread angles are authored, symmetric, and deterministic. Seven pellets use indices from
  left to right across the total cone; there is no RNG resource or client-authored seed.

### Straight, lobbed, and melee delivery

- [x] Straight pulse and pellet entities retain server-owned circular shapecasts after Avian's
  current-tree refresh. Every pellet has its own finite radius/range/lifetime and stops on its first
  valid hostile or terrain hit; allies, owner, and defeated fighters remain pass-through.
- [x] Model the arc launcher as top-down ballistic presentation over deterministic planar flight,
  not as an Avian dynamic rigid body. The server advances ground projection from launch to landing
  over an authored number of ticks; clients derive visual height as `4h*t*(1-t)` from replicated
  flight endpoints/current pose. It ignores fighter and terrain collision while airborne and
  resolves one landing explosion.
- [x] Launcher aim is fixed-range along preserved authoritative facing. This is controller-readable
  with the existing directional input and does not invent cursor-only distance selection. Clamp the
  desired point to arena bounds; if the landing circle overlaps solid terrain, search backward
  along the aim ray for the furthest clear point. A clear point beyond intervening cover remains
  valid, so the lob can punish cover.
- [x] A melee attack is one instantaneous multi-target sector query after current movement and
  Avian refresh. Use `shape_intersections` for bounded fighter candidates, a pure circle-versus-
  sector test for reach/angle, and terrain-only line-of-sight query per candidate. Sort by stable
  network identity and affect every hostile candidate; allies, owner, defeated targets, and targets
  occluded by solid terrain are excluded.
- [x] All three delivery systems emit the same internal impact/target records and payload pipeline.
  Delivery code does not mutate health, movement effects, telemetry counters, presentation assets,
  or HUD state directly.
- [x] Preserve M04's same-tick spawn/collision contract: straight deliveries spawned after
  `ApplyDeferred` are visible to the refreshed Avian query tree and may sweep, hit, and damage in
  that same tick. Before spawning beyond the owner, a terrain-only circle sweep from the owner to
  the intended muzzle rejects starting penetration and resolves a blocked delivery against the
  first wall instead of allowing a shot through cover.

### Area targeting, damage, knockback, and slow

- [x] Circular area targeting uses Avian `shape_intersections` against fighter colliders after the
  collider tree is current. It sorts candidates by `NetworkEntityId`, filters affiliation/defeat,
  and performs a terrain-only center-to-target line-of-sight check when authored.
- [x] Friendly fire remains off. Direct attacks never affect their owner. The arc explosion affects
  hostile fighters normally, applies 50% damage but full knockback to its owner, applies no slow to
  its owner, and never affects allies. Preserve M04 affiliation exactly: the neutral dummy is
  hostile to every non-owner team and can receive every otherwise-valid hostile payload.
- [x] Damage falloff is a validated payload rule. Scatter damage scales linearly by delivery travel
  distance between authored start/end values and clamps to an authored minimum. Pulse, launcher,
  and blade use flat damage in this milestone.
- [x] Knockback is server-owned external motion, not a physics impulse. A runtime component stores
  finite velocity and expiry; movement combines it with player velocity and uses the existing
  move-and-slide terrain path. Same-tick/overlapping knockbacks add in deterministic payload order,
  clamp to 900 units/s, and retain the latest expiry. Slow multiplies player-directed movement only,
  never external motion.
- [x] Runtime slow state uses a replicated `ActiveEffects { slow: Option<...> }`; M05 therefore has
  an exact capacity of one active slow per fighter rather than a process-lifetime vector. The only
  v1 policy is `StrongestRefreshes`: the lowest speed multiplier wins and expiry is extended to the
  later deadline. A stronger application owns the durable magnitude attribution; equal/weaker
  refreshes retain that source. Expiry occurs before movement on the due tick. No accumulating
  meter or immunity exists.
- [x] Resolve damage first for every stable target/order key, mark same-tick defeats, then apply
  knockback and slow only to surviving targets. This preserves mutual defeat while preventing a
  lethal hit from leaving hidden movement effects on a defeated fighter.
- [x] Defeat and sandbox selection clear slow/external motion immediately; reset starts clean.
  Target disconnect despawns the state with the fighter. An already-applied timed effect retains
  stable source attribution and naturally expires if its source disconnects; it contains no local
  source entity reference.

### Selection, replication, recovery, and presentation

- [x] Add a narrow per-fighter `SelectingWeapon` phase at initial accepted join. The server spawns
  the fighter identity/pose/health but with no `SelectedBuild` or `WeaponState`, no collision
  interactions, and no movement/fire. This is a sandbox readiness boundary, not a match state.
- [x] The client cycles its local candidate with left/right or A/D/arrow keys and confirms with
  controller South/A or Space/Enter. It sends `WeaponSelectionRequest { request_id, preset_id }`
  on the existing ordered-reliable bidirectional session channel. The message contains no target;
  the server maps the receiving link to its owned fighter.
- [x] The server accepts one known/selectable preset ID while the owned fighter is selecting, looks
  up and resolves its own catalog recipe, atomically inserts `SelectedBuild`, `ResolvedWeapon`, and
  full `WeaponState`, restores fighter collision, removes `SelectingWeapon`, and replies with an
  idempotent outcome. The same transaction clears buffered action state and installs an activation
  barrier: movement/fire stay neutral until a native input tick newer than the server's acceptance
  watermark arrives. This prevents input received on another channel before selection from becoming
  an action afterward. Unknown, stale, foreign-context, or post-lock requests do not mutate
  gameplay. Reconnect creates a fresh identity and selection phase.
- [x] A normal window without an explicit weapon shows the selector. Headless automation, network
  tests, and reproducible demo commands supply an explicit weapon and send the same request path;
  `--combat-demo` defaults its explicit selection to pulse only for backward-compatible demos.
- [x] Register `SelectedBuild` and the bounded public `ResolvedWeapon` snapshot with on-change
  replication rather than M04's replicate-once rule. Register selection phase, active effects,
  projectile flight/source/presentation profile, and existing durable state with the appropriate
  insert/remove or on-change semantics. Late join sees selecting versus active, preset source,
  recipe fingerprint/public configuration, current economy/effects, and active straight/lobbed
  projectiles without historical cues or a preinstalled per-player recipe.
- [x] Retain M04's replicated `AuthoritativeTick`. Every replicated deadline—including refill,
  slow, lob launch/landing, and defeat/reset—uses that timebase. The server publishes the completed
  tick before incrementing `SimulationTick`, so reconnect and
  late-join HUD calculations cannot be one tick ahead.
- [x] Keep the ordered-reliable combat cue stream from M04. Generalize cues around attack accepted,
  delivery impact/landing/melee, applied damage/effect, defeat, and reset with stable attack/event/
  weapon identities. Durable state, not historical cues, remains recovery truth. Measure scatter
  cue volume/head-of-line latency before considering batching or another cosmetic channel.
- [x] Client presentation resolves stable weapon/profile IDs and never drives collision or damage.
  Placeholder previews show pulse range, scatter cone, launcher landing and blast radius, and blade
  sector. Projectile/attack/impact/effect visuals remain client-only and bounded; no audio asset is
  required before Milestone 06.

### Telemetry semantics

- [x] Record per preset source and recipe fingerprint: accepted selections, accepted attacks,
  emitted deliveries, hostile delivery contacts, attacks with at least one hostile contact, applied
  hostile damage, self-damage, defeats, and engagement distance bands. Keep bounded exact outcome
  records with stable IDs. Preserve M04's fixed 512-record evidence/history limit, bounded cue
  deduplication, aggregate counters, and explicit dropped-record counts; no telemetry or diagnostic
  vector grows for the process lifetime.
- [x] Primary hit rate is `attacks_with_hostile_contact / accepted_attacks`; it cannot exceed 100%
  for spread or multi-target melee. Delivery contacts remain a separate diagnostic and may exceed
  attacks. An active per-attack tracker lives only until all deliveries resolve/expire/clean up.
- [x] Preferred-distance evidence uses source-to-target engagement distance at damage time.
  Projectile travel remains separately loggable for collision diagnostics. This distinction is
  necessary for area and melee attacks and replaces the pulse-only assumption that path distance
  and engagement distance are always equivalent.

## Research log

| Date | Source | Finding | Decision impact |
|---|---|---|---|
| 2026-08-14 | `docs/{00-product-direction,02-fighter-model,03-weapons-and-abilities,05-gameplay-mvp,08-network-architecture}.md` and `docs/implementation/v1/{roadmap,milestone-03,milestone-04}.md` | M05 must add exactly four readable weapon profiles while keeping selection/build/runtime state and presentation/authority separate. M03 already fixes friendly/self collision policy; M04 fixes stable attribution and recovery. | Preserve those contracts and limit new primitives to spread, lob/area, melee, knockback, and slow. |
| 2026-08-14 | User product-direction clarification and re-audit of the same product/architecture documents | The intended product is an arsenal of player-authored brawlers whose weapons vary in behavior and bounded specs. The earlier M05 draft composed developer-authored definitions internally but selected only immutable weapon IDs, leaving no explicit player-recipe/resolution boundary. | Recast the four M05 weapons as preset recipes, add preset-independent structural resolution and replicated resolved configuration, require one non-preset fixture, place first bounded player editing at M08, and defer persistence/acquisition without coupling them to combat. |
| 2026-08-14 | Completeness pass against the user-approved M04 fixes and current combat/network tests | The first M05 draft weakened several proven invariants: pre-combat disconnect cleanup, same-tick projectile collision, impact-fraction ordering, atomic event-ID reservation, globally ordered outcomes, bounded evidence, and completed-tick publication. Selection also needed a fresh-input barrier, and presentation identity was still coupled to presets. | Make every M04 fix a hard M05 regression contract; add exact engine ceilings, canonical float/fingerprint rules, preset-independent weapon configuration, atomic selection activation, deterministic lob repair, bounded telemetry, and measurable load/latency gates. |
| 2026-08-14 | Current `Cargo.toml`, `src/{combat,client,server,movement,protocol}.rs`, `tests/{network,performance}.rs`, README, Justfile, and scripts | M04 already supplies held native input, replicated selected/runtime state, swept projectiles, ordered cues, fixed reset, impairment profiles, and stable IDs. `SelectedBuild` is currently replicate-once and the pulse-only `combat.rs` is already large. | Evolve rather than duplicate the pipeline; make selection mutable/insertable and split the combat module by cohesive responsibilities. |
| 2026-08-14 | `references/lightyear/examples/README.md`, `simple_setup`, `simple_box`, and book `src/SUMMARY.md`, `concepts/{reliability/channels,advanced_replication/replication_logic}.md` | Typed messages on an ordered-reliable channel fit discrete selection; component updates and entity actions have different recovery/ordering behavior. | Reuse `SessionChannel` for selection and replicated components for current selected/effect/projectile state. |
| 2026-08-14 | Cargo-resolved Lightyear 0.29 `lightyear_{replication,messages}-0.29.0/src/{registry/replication.rs,send.rs}` | `component::<C>().replicate()` is on-change while `replicate_once()` is initial/insert-only; `MessageSender` buffers a typed message on a chosen channel. | Change `SelectedBuild` to on-change registration and map selection to the receiving connection instead of carrying an entity target. |
| 2026-08-14 | Cargo-resolved Avian 2D 0.7 `spatial_query/{system_param,query_filter}.rs`, local Avian examples, and [released SpatialQuery docs](https://docs.rs/avian2d/0.7.0/avian2d/spatial_query/struct.SpatialQuery.html) | Released on-demand APIs include closest shapecast and exact shape-intersection candidate queries; intersection result order is not a gameplay ordering contract. | Retain shapecasts for moving circles, use circle intersections for area/melee candidates, and explicitly sort stable target IDs. |
| 2026-08-14 | `references/bevy/examples/asset/{custom_asset,hot_asset_reloading}.rs`, Cargo-resolved Bevy 0.19.1 asset source, and [Bevy 0.19 release](https://bevy.org/news/bevy-0-19/) | Custom runtime assets require the asset subsystem; file watching is feature/platform dependent. Bevy 0.19 also introduces BSN but does not ship a first-party `.bsn` asset loader. | Do not make authoritative weapon rules a runtime Bevy asset or BSN scene; embed validated typed RON and preserve server feature isolation. |
| 2026-08-14 | Cargo graph plus [RON 0.12.2 documentation](https://docs.rs/ron/0.12.2/ron/) | RON 0.12.2 supports typed Serde structures/enums and is already resolved on the client through Bevy, but is absent from the server graph. | Add a direct RON dependency for both roles; hash canonical typed data rather than raw text. |
| 2026-08-14 | [Lightyear book channel setup](https://cbournhonesque.github.io/lightyear/book/tutorial/setup.html) and [replication logic](https://cbournhonesque.github.io/lightyear/book/concepts/advanced_replication/replication_logic.html) | Channels encode reliability/ordering; replication groups keep component state coherent while entity actions are ordered reliable and updates are sequenced. Public web indexing lags the pinned 0.29 API. | Use local/Cargo-resolved 0.29 source for exact spelling and current primary pages for the behavioral model. |

## Technical specification

### Decisions

| Concern | Decision |
|---|---|
| Gameplay content | Typed rules plus four preset recipes in RON, build-embedded in both roles |
| Compatibility | Semantic gameplay-content fingerprint added to existing handshake |
| Customization seam | Pure server-owned configuration resolver independent of preset identity; M05 exposes presets only |
| Architecture | Rules/recipe/resolved/runtime separation; shared pipeline; focused delivery systems |
| Selection | Connection-owned preset request; server lookup + resolve; locked after acceptance |
| Attack identity | One `AttackId` per accepted action; bounded delivery indices within it |
| Projectile identity | Stable `(AttackId, delivery_index)` plus replicated entity lifecycle |
| Area/melee queries | Avian intersections plus stable-ID sort and explicit terrain occlusion |
| Knockback | Server external-motion component through existing move-and-slide |
| Slow | Replicated bounded immediate effect; strongest value refreshes duration |
| Recovery | Replicated current state; cues remain live presentation enhancement |
| Hot reload | Deferred until a server-owned safe catalog-revision boundary exists |
| Prediction | Existing authoritative/interpolated baseline; measure, do not assume |

### Application and plugin composition

The package remains one library plus independently built client/server binaries. The implementation
adds no crate, facade, service layer, or second runtime gameplay model.

```text
Both roles
  GameplayPlugin (fixed schedules/system sets)
  WeaponCatalogPlugin (embedded parse, structural resolver, semantic fingerprint)
  ProtocolPlugin (selection/cue/component registration)

Dedicated server only
  ServerNetworkPlugin (session-owned selection request/outcome)
  ServerCombatPlugin (attack/economy, delivery, payload/effect, telemetry modules)

Client only
  ClientNetworkPlugin (selection request/outcome lifecycle)
  ClientWeaponSelectionPlugin (local input context and overlay)
  ClientCombatPlugin (previews, HUD, projectile/effect/cue presentation)
```

These names describe cohesive responsibilities; source modules do not become separate plugins
without a concrete composition or testing need. Only server composition installs attack acceptance,
delivery collision, payload mutation, slow/knockback, selection validation/resolution, or telemetry
authority. Both roles parse the same shared catalog and register the same wire protocol; the client
uses built-in presets for selection text/previews and treats the replicated resolved public
configuration as presentation input, never combat truth.

### Authored catalog contract

The exact Rust layout may adapt to derive constraints, but these semantic fields are required:

```text
WeaponCatalogFile {
  schema_version: u16,
  recipe_policy: WeaponRecipePolicy,
  presets: Vec<WeaponPresetDefinition>
}

WeaponRecipePolicy {
  implemented_capabilities,
  numeric_safety_bounds,
  max_deliveries_per_attack,
  max_payload_bundles,
  max_effects_per_bundle
}

WeaponPresetDefinition {
  id: WeaponPresetId,
  key: String,
  display_name: String,
  configuration: WeaponConfiguration
}

WeaponConfiguration {
  presentation_profile_id: WeaponPresentationProfileId,
  recipe: WeaponRecipe
}

WeaponRecipe {
  economy: Magazine { capacity, reload_ticks }
         | Charges { capacity, recharge_ticks },
  fire_cooldown_ticks: u64,
  firing: Single | Spread { delivery_count, total_angle_degrees },
  delivery: Straight { speed, radius, range, lifetime_ticks, muzzle_offset }
          | Lobbed { distance, flight_ticks, visual_arc_height,
                      landing_clearance_radius, muzzle_offset }
          | MeleeArc { reach, angle_degrees },
  payload_bundles: Vec<PayloadBundleDefinition>
}

ResolvedWeapon {
  source_preset_id: Option<WeaponPresetId>,
  recipe_fingerprint: WeaponRecipeFingerprint,
  presentation_profile_id: WeaponPresentationProfileId,
  recipe: WeaponRecipe
}

PayloadBundleDefinition {
  target: Direct | Area { radius, terrain_occlusion },
  effects: Vec<PayloadEffectDefinition>
}

PayloadEffectDefinition =
  Damage { amount, falloff, recipients }
  Knockback { speed, duration_ticks, recipients }
  Slow { movement_multiplier, duration_ticks, stacking, recipients }
```

Recipient policy is explicit per effect: `Hostiles` or `HostilesAndOwner { owner_scale }`.
Allies are excluded in M05. Damage falloff is `None` or
`Linear { start_distance, end_distance, minimum_scale }`.

Code-owned `EngineWeaponLimits` are a safety envelope, not initial balance targets:

| Limit | Hard ceiling |
|---|---|
| deliveries per attack | 16 |
| payload bundles / effects per bundle | 4 / 4 |
| active M05 slow entries per fighter | 1 |
| magazine or charge capacity | 1–32 |
| fire/refill/effect deadline | 1–3,600 ticks |
| straight lifetime / lob flight | 1–600 ticks |
| damage per effect | 1–1,000 |
| world distance/speed field | finite, nonnegative, at most 4,096 units or units/s |
| projectile/landing/area radius | finite, positive, at most 512 units |
| authored knockback speed | finite, nonnegative, at most the 900 units/s runtime clamp |
| spread or sector angle | finite and in `(0, 180]` degrees |
| canonical replicated resolved weapon | at most 2,048 postcard bytes |

The checked-in M05 catalog policy may be stricter. Validation fails when a policy exceeds the
engine envelope, a bounded collection exceeds either limit, or canonical serialization exceeds the
wire-size ceiling. Keys are lowercase ASCII kebab-case, 1–32 bytes; display names are valid
non-control UTF-8, 1–48 bytes. These fields are trusted built-in presentation metadata, not future
player-authored text.

Catalog validation rejects:

- wrong catalog schema version, invalid/incomplete policy, empty/missing/duplicate/nonascending
  preset IDs or keys, missing required preset IDs 1–4, unknown presentation profiles, or a fifth
  built-in M05 preset;

Structural recipe validation—called for both catalog presets and configurations without a preset
identity—rejects:

- zero counts/durations/capacities/damage/radii/ranges, nonfinite values, negative values, angle not
  in `(0, 180]`, multiplier outside `(0, 1]`, owner scale outside `[0, 1]`, or invalid falloff order;
- a straight one-tick step larger than its range, flight outside bounded tick limits, delivery or
  payload vectors above the catalog/engine limits, or a resolved snapshot above its wire bound;
- spread with non-straight delivery, lobbed delivery without an area bundle, melee without a direct
  bundle, area effects on straight direct impact without an implemented detonation rule, or any
  payload/economy/recipient variant not supported in M05.

Activation resolution then accepts the structurally valid `WeaponConfiguration`, the selected
fighter definition, and optional source-preset metadata. It rejects a straight muzzle offset below
`fighter_body_radius + projectile_radius` and any future fighter-specific incompatibility. It
returns a canonical immutable snapshot plus semantic `WeaponRecipeFingerprint`; it does not consult
a preset ID to choose behavior. This two-stage boundary avoids baking the standard M05 body into a
future arsenal recipe. M05 has no balance-cost or entitlement input, so passing activation
resolution means “implemented, safe, and compatible with this fighter,” not “owned by this account”
or “legal under a future competitive budget.”

The recipe fingerprint includes the fingerprint-format version and canonical gameplay recipe, but
not source preset or presentation profile. It is for semantic comparison, replication diagnostics,
and telemetry; it is not an authorization token or persistent build identity. M05 also carries the
authoritative source preset ID. A future persistent arsenal uses a server-issued brawler/build ID
plus revision and still resolves the stored configuration before match activation.

Invalid embedded content is a startup/test failure. A client with valid but different shared rules
or built-in presets is rejected during handshake before it receives an owned fighter. Future
per-player recipe data is not folded into this global fingerprint; accepted resolved public
configuration is replicated from the server.

### Provisional weapon preset recipes

Values are initial playtest tuning at 60 Hz, not balance commitments. Numeric data belongs in each
preset's recipe; algorithms do not branch on preset ID.

| Weapon | Economy/cadence | Firing/delivery | Payload | Intended profile |
|---|---|---|---|---|
| Pulse sidearm `1` | magazine 6; cooldown 12 ticks; reload 60 | single straight; speed 900; radius 6; range 900; lifetime 60; muzzle 34 | direct 25, no falloff | reliable mid range; six-shot pressure; low burst; cover/rushing counter it |
| Scatter cannon `2` | magazine 4; cooldown 36; reload 72 | 7 straight pellets over 30°; speed 850; radius 4; range 360; lifetime 30; muzzle 32 | direct 12 each; linear falloff 140–360 to 50% | close burst up to 84; cone/falloff and reload punish range/misses |
| Arc launcher `3` | magazine 3; cooldown 48; reload 96 | single lob; distance 520; flight 45; visual height 140; clearance radius 10; muzzle 34 | terrain-occluded area radius 150: hostile damage 40, knockback 300 for 8 ticks, slow 0.70 for 45; owner damage 50%, knockback 100%, no owner slow | fixed-range cover/group punish; visible 0.75 s landing; dead zone up close; wall/boundary clamping can self-hit |
| Impact blade `4` | 3 charges; cooldown 18; recharge 60 | single melee sector; reach 120; angle 100° | direct 34 and knockback 650 for 6 ticks | three-swing close burst; displacement; must enter kiteable danger range |

All authored deadlines use simulation ticks. Damage remains integer. Falloff scales in `f32`, then
rounds once to nearest integer with a minimum of one for a valid hostile contact.

### ECS ownership and lifecycle

| Data/entity | Server authority | Client role | Lifetime/recovery |
|---|---|---|---|
| validated fighter/weapon content catalogs | capability/safety rules, presets, and fingerprint | selector/debug/presentation lookup and fingerprint | app lifetime; immutable after startup |
| `SelectingWeapon` | inserted on accepted join; suppresses gameplay | replicated selector gate | until one accepted selection; recovered on late observation |
| `AwaitingPostSelectionInput` | server-only accepted-input watermark; suppresses movement/fire | absent | acceptance until one newer native input tick |
| `SelectedBuild` | stable build/weapon identity inserted only by validated server flow | replicated read-only | active fighter; recovered on join/reconnect |
| `ResolvedWeapon` | immutable resolved recipe used by combat | replicated bounded public configuration for HUD/previews | selection through fighter lifetime; recovered on join/reconnect |
| `WeaponState` | resource count and phase/deadline | replicated HUD | from selection through fighter life; reset/refill rules explicit |
| `ActiveEffects` | slow entries and stable attribution | replicated visual/status truth | until expiry, defeat/reset, or target removal |
| `ExternalMotion` | finite knockback velocity/deadline | absent; movement is observed via pose | until expiry, defeat/reset, or target removal |
| straight/lobbed projectile | server spawn/advance/impact/despawn | replicated/interpolated presentation copy | attack delivery lifetime; current active set recovers |
| lobbed flight description | server-authored endpoints/visual height | replicated-once visual arc derivation | projectile lifetime |
| melee attack record | queued/resolved server-only | transient cue only | accepted tick through fixed-post payload resolution |
| internal target/payload buffers | produced/consumed in ordered fixed-post sets | absent | same tick only |
| per-attack telemetry tracker | delivery completion/hit aggregation | absent | until every delivery resolves/expires/cleans up |
| combat cues/effects | stable outcomes only | deduplicated bounded presentation | live reliable stream; no historical replay |
| `AuthoritativeTick` | completed server simulation tick | replicated deadline/recovery timebase | app/match lifetime; published before simulation increment |

Selecting fighters keep stable identity and pose but have collision masks disabled and do not move,
aim authoritatively, fire, take damage, or receive effects. The neutral dummy remains a fully
selected active fighter using the resolved pulse preset and never enters selection.

### Runtime and protocol shapes

```text
AttackId(u64)

AttackSource {
  attack_id,
  player_id,
  owner_network_entity_id,
  team_id,
  weapon_recipe_fingerprint,
  source_preset_id,
  origin,
  facing
}

ProjectileSource {
  attack: AttackSource,
  delivery_index: u8
}

LobbedFlight {
  launch,
  landing,
  launched_at_tick,
  lands_at_tick,
  visual_arc_height
}

WeaponState {
  resource: u8,
  phase: Ready
       | Cooldown { ready_at_tick }
       | Refilling { ready_at_tick }
}

SelectedBuild {
  primary_weapon_recipe_fingerprint,
  source_preset_id
}

ResolvedWeapon {
  source_preset_id,
  recipe_fingerprint,
  presentation_profile_id,
  recipe
}

ActiveEffects {
  slow: Option<SlowEffect>
}

SlowEffect {
  source: stable AttackSource subset,
  movement_multiplier,
  expires_at_tick
}

ExternalMotion { velocity, expires_at_tick }
```

Register/fingerprint:

- client-to-server `WeaponSelectionRequest { request_id, preset_id }` and server-to-client
  `WeaponSelectionOutcome { request_id, decision, accepted_preset_id,
  accepted_recipe_fingerprint }` on `SessionChannel`; `decision` is a stable enum covering
  `Accepted`, `UnknownPreset`, `NotSelecting`, `StaleRequest`, and `ResolutionFailed`. No recipe
  values are accepted from the client in M05;
- generalized server-to-client combat cues on `CombatChannel`;
- `SelectingWeapon`, on-change `SelectedBuild` and bounded public `ResolvedWeapon`, `WeaponState`,
  `ActiveEffects`, projectile marker/source/flight description, and existing stable fighter/
  projectile pose/state components;
- the existing replicated `AuthoritativeTick` and every new deadline-bearing component;
- `GameplayContentFingerprint` in `ClientHello` and `ContentMismatch` in `JoinRejection`;
- new protocol constants for incompatible serialized types.

Selection request IDs are monotonically increasing per client process. The server stores the last
handled request/outcome per session so duplicate delivery is idempotent even though the channel is
reliable. Repeating the last ID returns the byte-equivalent prior outcome; a lower ID is `StaleRequest`;
a higher ID after activation is `NotSelecting` and cannot switch the weapon. A request never carries
player, fighter, Bevy entity identity, behavior enums, numeric
specifications, payloads, or presentation values. The server maps the preset to its own recipe,
resolves it, and installs the result atomically. The accepted outcome is queued only after an
exclusive mutation or explicit `ApplyDeferred` boundary has made the selected build, resolved
weapon, initialized economy, collision restoration, neutral action state, and activation barrier
visible together. The client closes selection only from durable replicated state; the outcome alone
is an acknowledgement and rejection explanation.

### Fixed-tick schedule and ordering

```text
PreUpdate
  Lightyear receive/validate/apply
  session/link cleanup
    after transport receive, remove disconnected fighters' deliveries and trackers
    before any fixed combat; delivery systems also reject missing/disconnected owners

FixedUpdate
  GameplaySet::Lifecycle
    expire slow/external motion due this tick
    reset due defeated fighters and clear all effects
  apply_deferred
  GameplaySet::Input
    selected active fighters only: fresh input -> facing
  GameplaySet::Simulation
    player velocity * active slow + external motion -> existing move-and-slide
  GameplaySet::Fire
    advance economy -> validate held fire/landing -> reserve AttackId + accepted-cue EventId
    -> mutate resource/cooldown
    -> spawn straight/lobbed deliveries or queue melee attack
  apply_deferred

FixedPostUpdate
  Avian PhysicsSystems::Prepare -> StepSimulation
  CombatSet::Delivery
    stable-order straight sweeps, lob progress/landing, melee candidate resolution
  CombatSet::Targeting
    direct/area target selection and terrain occlusion -> pending payloads
  CombatSet::Payload
    stable target/contact/source/effect order -> reserve outcome EventIds
    -> all damage/defeats first -> surviving knockback/slow
  apply_deferred
  CombatSet::Lifecycle
    defeat/non-targetable transition; expiry; attack tracker finalization
  CombatSet::TelemetryAndCues
    exact bounded records/counters/logs -> globally EventId-sorted cue outbox
  CombatSet::Finalize
    invariants -> publish completed AuthoritativeTick -> one SimulationTick increment

Update (server)
  ordered selection request processing mapped to receiving session -> atomic activation boundary

Update/PostUpdate (client)
  selection input/UI or neutral gameplay input
  replicated pose/state -> aim previews, HUD, projectile/effect presentation
  cue receive/deduplicate -> bounded transient feedback
```

System sets are ordered where outcomes depend on current geometry or prior mutations. Delivery
systems may prepare independent records in parallel only if final records are stable-sorted before
payload application. Time tests advance the fixed schedule, not wall clock.

### Delivery and targeting algorithms

#### Spread and straight projectiles

For `n > 1`, delivery angle `i` is
`facing - total_angle/2 + i * total_angle/(n-1)`. Spawn indices in ascending angle and resolve
projectiles by `(attack_id, delivery_index)`. Before entity insertion, terrain-only circle sweeps
from the owner pose to each intended muzzle; a blocked segment resolves that delivery at the first
terrain contact and never spawns it beyond cover. Every spawned projectile then uses the existing
clamped circular sweep, explicit masks, owner/ally/defeated predicate, first valid hit, finite
range/lifetime, and terrain consumption. Projectiles inserted before the fixed-post Avian refresh
participate that tick. Apply authored damage falloff from actual delivery travel.

#### Lobbed flight and landing

1. Derive desired landing from server pose/facing and fixed authored distance.
2. Clamp to playable bounds inset by clearance radius.
3. If the landing circle intersects solid terrain, sample backward from desired toward the minimum
   launch clearance in fixed 5-unit intervals, at most 128 samples. The first clear sample is the
   furthest known clear point; refine its blocked/clear interval with eight fixed bisection steps.
   These search constants are code-owned and covered by boundary tests.
4. If no legal point exists, reject the held-fire attempt before allocating IDs or changing
   resource/cooldown; it may retry next tick while held. Otherwise accept exactly that resolved
   landing point.
5. Spawn one replicated lob description/projectile with no gameplay collider.
6. Advance planar ground pose by normalized elapsed authoritative ticks. Clients derive visual
   height from `AuthoritativeTick`, `launched_at_tick`, and `lands_at_tick`; the replicated planar
   pose remains convergence truth.
7. On the exact landing tick, queue landing/despawn intent, query the authored area,
   terrain-occlude candidates, and queue payload effects in stable target order. Outcome commit
   removes the projectile only after the tick's complete event-ID range is reserved.

Airborne lobs cannot hit fighters or walls. Losing the projectile entity or cue cannot lose applied
damage: payloads are server outcomes and current health/effects replicate independently.

#### Melee sector

1. Query circle intersections at the owner center with authored reach.
2. Filter active hostile fighters and evaluate exact circle-versus-sector inclusion using target
   center/radius, including targets whose collider edge crosses the sector boundary.
3. Reject candidates with solid terrain between owner boundary and target center.
4. Sort by stable network identity and emit one direct target record for every accepted candidate.
5. Apply shared payload effects; the swing is one attack regardless of target count.

Every accepted target record carries a finite within-tick `contact_fraction`: the current sweep's
impact fraction for straight delivery and `1.0` for instantaneous landing/melee contact. This
preserves the approved M04 closest-contact ordering while giving all delivery families one key.

### Payload ordering and lifecycle

Pending effects sort by:

```text
(target NetworkEntityId,
 contact_fraction using finite f32 total order,
 attack_id,
 delivery_index,
 payload_bundle_index,
 effect_index)
```

For a completed tick:

1. Materialize and stable-sort all pending records before mutating targets.
2. Dry-run delivery contacts, requested damage/falloff/recipient scalars, lethal transitions, and
   survivor effects to calculate every impact/landing/melee, damage, defeat, and applied-effect
   event ID required. Reserve the complete checked event-ID range before projectile removal,
   health, telemetry, or effect mutation; exhaustion aborts the outcome batch without partial state.
   Assign reserved IDs in sorted-record order and, within one record, impact/contact, damage,
   defeat, then surviving effects.
3. Apply every damage record in sorted order, record actual non-overkill damage and self-damage, and
   retain the first lethal record for a target as its defeat attribution. This permits reciprocal
   lethal damage and posthumous delivery outcomes.
4. Mark every same-tick defeated target non-targetable/non-colliding, then suppress all non-damage
   payloads for that defeated set.
5. For survivors, combine/clamp knockback in sorted order, then apply/refresh strongest slow.
6. Commit the internal `ProjectileImpact -> PendingDamage -> DamageApplied/FighterDefeated` chain,
   effect outcomes, telemetry, structured logs, and cues from authoritative applied values. Globally
   sort the cue outbox by `CombatEventId`, never by target/system iteration order.

Reset restores full definition health and selected weapon resource/ready state, clears slow and
external motion, and preserves the selected weapon. Defeat clears effects and suppresses weapon
advance/input until reset but preserves already-launched straight/lobbed deliveries and their stable
posthumous attribution. Owner disconnect cleanup runs in `PreUpdate` after receive and before fixed
combat, removes every active delivery, and completes its bounded attack tracker; sweep/landing code
also refuses an owner missing from the current connected-owner authority set. Already-applied timed
effects on other fighters retain stable attribution and expire normally.

### Presentation and selection UX

The selection overlay presents four horizontally cycled entries with name, pattern, range,
resource/recovery, short strength, and short counterplay. Keyboard A/D or arrows act on key edges.
Controller D-pad acts on button edges; left-stick horizontal also cycles once past `0.6` and must
return inside `0.3` before cycling again. This prevents analog drift/repeat. It shows server
rejection without leaving selection and closes only when durable replicated selection state
confirms acceptance. Gameplay input remains neutral until the post-selection input barrier clears;
controller disconnect falls back to keyboard without accepting a weapon or firing.

During gameplay, client-only previews derive from the replicated resolved public recipe and rendered
fighter pose; the source preset ID supplies name/icon only:

- pulse: center line/end marker at maximum range;
- scatter: cone boundaries and short-range end arc;
- launcher: fixed landing marker, flight path hint, and explosion-radius ring; blocked/clamped
  landing uses the shared greybox shape helpers where possible;
- blade: reach/angle sector that clearly communicates danger range.

Previews are subtle outside active aiming/fire, do not require a cursor, and work at compact window
sizes. Placeholder projectile shapes/colors distinguish all weapon profiles and retain source color.
Damage, slow, knockback, area impact, reload/recharge, and defeat must remain understandable without
reading logs. Milestone 06 owns replacement assets and audio.

Preview landing repair uses the same pure bounded search helper against the client's current
greybox, but remains advisory: the replicated server landing is authoritative and replaces any
local marker as soon as it arrives. A repaired/clamped or currently invalid preview is visually
distinct. No preview result is sent as gameplay input.

The generalized cue variants are `AttackAccepted`, `DeliveryImpact` (fighter or terrain),
`LobLanded`, `MeleeContact`, `DamageApplied`, `EffectApplied`, `FighterDefeated`, and `FighterReset`.
Each contains its own `CombatEventId`, completed authoritative tick, stable attack/weapon/source
identity, and only the variant-specific target/position/applied-value/deadline fields. Delivery
expiry needs no historical cue; replicated despawn removes its visual. Clients retain M04's bounded
event-ID deduplication and bounded visual lifetimes.

### Telemetry contract

Per-weapon summary:

```text
selections
accepted_attacks
emitted_deliveries
hostile_delivery_contacts
attacks_with_hostile_contact
attack_hit_rate
hostile_damage
self_damage
defeats
close / mid / long engagement hits and damage
```

Outcome records include completed tick, attack/event ID, delivery index when applicable, source,
target, weapon, origin/contact/landing, requested/applied value, engagement distance, delivery
travel, effect/recipient policy, and resulting durable state. Terrain contacts and expiry resolve a
delivery but do not count as hostile contacts. One multi-target blade swing increments
`attacks_with_hostile_contact` once.

`emitted_deliveries` counts each straight pellet/projectile, lob, or melee swing. A hostile delivery
contact counts each accepted hostile target contact, so area and melee deliveries may contribute
more than one; owner self-contact is excluded and reported through self-damage. The per-attack
tracker folds any positive hostile contact into one `attacks_with_hostile_contact` increment after
all of that attack's deliveries resolve.

Exact run-evidence histories retain the first 512 entries, matching M04's reproducible capture
policy, report discarded-entry totals, and never control gameplay. Active attack trackers are
bounded by accepted unresolved deliveries and are removed on full resolution, expiry, owner
disconnect, or app stop. Graceful summaries report
resolved hits/misses and still-in-flight/canceled deliveries separately instead of silently calling
an unresolved delivery a miss. Wall-clock epoch samples remain opt-in evidence instrumentation;
fixed-tick tests assert authoritative completed ticks directly.

## Trackable implementation plan

### Prerequisite and content foundation

- [ ] Re-run and record the locked M04 format, role-specific Clippy/tests/builds, network,
  performance, UDP/process, impairment, presentation, and server-feature baseline. Do not relabel
  unobserved older hardware cases as passed; record M05's expanded hardware checks as fresh evidence.
- [ ] Add the direct RON dependency and `content/v1/weapons.ron`; implement typed rules, recipe,
  configuration, preset, and resolved-snapshot shapes; code-owned engine limits; canonical catalog
  validation; pure preset-independent structural/activation resolution and fingerprinting; exactly
  four built-in presets; and failure tests without enabling Bevy assets on the server.
- [ ] Split the combat module into cohesive submodules while preserving the existing public module,
  plugin composition, M04 behavior, and feature gates.
- [ ] Generalize preset/recipe identity, authored enums, resolved weapon, runtime economy, attack
  source, internal buffers, and stable ordering; migrate pulse through lookup + resolution before
  adding a second preset.

### Selection and protocol

- [ ] Add content fingerprint handshake/rejection and bump protocol constants; prove mismatch
  cleanup uses the existing controlled rejection path.
- [ ] Add preset-selection request/outcome registration on `SessionChannel`, per-session request
  state, owner mapping, server-side preset lookup/recipe resolution, idempotent validation,
  selecting fighter lifecycle, atomic activation, and fresh-input barrier; reject any attempted
  client-authored specs.
- [ ] Change selected-build registration to on-change and register the bounded public resolved
  weapon plus selection/effect/lobbed state; retain protocol fingerprint equality in all supported
  roles.
- [ ] Implement client selector input/context/UI, explicit CLI/demo selection, neutral input while
  selecting, rejection display, and replicated confirmation.

### Weapon delivery and payload vertical slices

- [ ] Preserve pulse through preset lookup, recipe resolution, and the generalized single/straight/direct pipeline and prove no M04
  cadence, collision, reset, attribution, cue, or telemetry regression.
- [ ] Add deterministic spread, per-delivery identity, pellet falloff, scatter presentation, and
  attack-versus-delivery telemetry.
- [ ] Add server landing resolution, lobbed planar flight, replicated arc description, landing
  explosion/occlusion, area damage, self policy, launcher indicator, and cleanup/recovery.
- [ ] Add melee sector intersection, terrain occlusion, multi-target stable ordering, blade charge
  economy, swing presentation, and no-projectile lifecycle.
- [ ] Add shared payload application, knockback through authoritative movement, replicated slow,
  stacking/refresh/expiry, M04-compatible contact ordering and event reservation, globally ordered
  cues, and defeat/reset/pre-combat-disconnect cleanup.

### Verification, telemetry, and handoff

- [ ] Extend per-weapon telemetry/logging, fixed 512-record histories, drop counters, and bounded
  per-attack trackers; update graceful summary and exact metric tests.
- [ ] Extend Crossbeam authority/recovery/impairment tests and real UDP automation so two clients
  select different weapons and exercise every delivery/payload path.
- [ ] Extend worst-case headless performance cases for pellet bursts, simultaneous landing areas,
  melee candidates, and active effects; retain the 16.67 ms p95 target on the recorded machine.
- [ ] Extend README/Justfile/scripts/CI for interactive selection and four explicit weapon demos,
  without weakening supervised cleanup or server feature isolation.
- [ ] Run 30/60/high render-profile, keyboard/mouse, physical-controller, two-client readability,
  and weapon counterplay scenarios; record observations and tuning changes.
- [ ] Set `User playtest` only after automated/network/visual gates pass and provide the exact
  commands, controls, scenarios, known limitations, and requested observations.

## Test plan

### Pure and small-App tests

- [ ] RON parses exact rules/presets and rejects every catalog/value/combination invariant;
  semantically equal text produces the same content fingerprint and one numeric change produces a
  different fingerprint. Reordered keyed data and `-0.0` normalize identically; NaN/infinity,
  over-wide policy, oversized vectors/strings/wire snapshots, and unknown profiles fail closed.
- [ ] All four presets and a legal non-preset configuration fixture produce canonical resolved snapshots;
  identical recipes share a semantic recipe fingerprint regardless of preset identity, and no
  combat algorithm branches on a preset ID. Fighter-context muzzle compatibility is tested
  separately from structural recipe validity.
- [ ] Magazine/charge cooldown, last-unit refill, completion-tick fire, selection initialization,
  defeat/reset, and held/missing input boundaries use explicit ticks.
- [ ] Seven spread angles are finite, symmetric, ordered, bounded, and create one attack with seven
  unique delivery indices; falloff boundary rounding is exact.
- [ ] Straight sweeps retain no-tunneling, first-hit, range, ally/owner/defeated, neutral-hostility,
  and terrain rules; a blocked owner-to-muzzle path cannot spawn beyond cover, and a newly spawned
  projectile may hit/damage in its accepted tick after Avian refresh.
- [ ] Lob landing is exact at 45 ticks, clear beyond cover, clamped at bounds, repaired out of solid
  landing, non-colliding in flight, and explodes once.
- [ ] Area queries include circle-edge overlap, exclude allies, apply owner scalars, respect terrain
  occlusion, sort targets, and never apply one payload twice.
- [ ] Melee circle-sector math covers center/edge/tangent/outside cases, multiple stable targets,
  owner/allies/defeated exclusion, and wall occlusion.
- [ ] Damage-first ordering preserves mutual defeat; lethal targets receive no slow/knockback;
  overkill and integer falloff remain bounded. Target/contact-fraction/attack ordering matches M04,
  lethal event IDs reserve atomically before mutation, exhaustion produces no partial state, and
  the cross-target cue stream is globally monotonic by event ID.
- [ ] Knockback combines/clamps deterministically, collides/slides through the movement path, ignores
  slow scaling, and expires exactly. Strongest slow refreshes, weaker slow cannot replace it,
  expiry restores speed, and reset/defeat clears it.
- [ ] Every schedule boundary proves movement precedes Avian refresh, delivery precedes targeting,
  all damage/defeat precedes surviving effects, cues/telemetry reflect applied values,
  `AuthoritativeTick` publishes the completed tick, and `SimulationTick` increments once.
- [ ] Selector and all four aim previews/HUD states have focused headless presentation tests,
  including bounded effect cleanup and compact text constraints.

### Deterministic separate-App network tests

- [ ] A selecting fighter is stable/replicated but cannot move, collide, fire, take damage, or
  author effects; one valid owned preset request atomically activates the server-looked-up resolved
  recipe/state. Buffered pre-selection input remains neutral until a strictly newer native input
  tick clears the activation barrier.
- [ ] Unknown, duplicate, stale, post-lock, and forged state/selection attempts cannot change
  another fighter, bypass selection, switch weapons, submit behavior/spec values, alter economy, or
  create an attack.
- [ ] Two clients select different presets and agree on preset source IDs, recipe fingerprints/public
  configuration, economy, active projectiles,
  impacts, health, effects, knockback pose, defeat, reset, attribution, and telemetry.
- [ ] Spread creates one authoritative attack/seven deliveries; duplicate/reordered native input
  cannot multiply attacks or bypass cooldown/refill.
- [ ] Launcher landing, area/self damage, occlusion, slow, and knockback converge on both clients;
  no client-authored landing/area/effect value is accepted.
- [ ] Blade affects all and only stable visible sector targets with no projectile entity and cannot
  hit through terrain.
- [ ] Late join during selection, scatter flight, lobbed flight, refill, slow, knockback, and defeat
  receives correct durable current state and authoritative deadline progress without historical
  cues; lob height/progress derives from replicated launch/landing ticks.
- [ ] Disconnect/reconnect removes owned deliveries and trackers, retains/expiring already-applied
  stable effects as specified, creates one fresh selection identity, and leaves no orphan state.
  A disconnect received immediately before impact is cleaned before fixed combat, and sweep/landing
  guards also reject a missing or disconnected owner.
- [ ] Shared content fingerprint mismatch rejects before fighter spawn and cleans the session exactly like
  existing version/registry mismatches.
- [ ] Every M02–M04 connection, input, collision, combat, impairment, and recovery assertion retains
  its meaning for pulse.

### Statistical, process, performance, and visual verification

- [ ] Run local, typical, and adverse profiles with repeated results for all four weapons; require
  authoritative counts, no duplicate payloads, two-client convergence, and bounded cue/state drain.
- [ ] Report per-weapon attack-to-feedback and state-convergence median/p95 plus cue volume. If
  scatter p95 exceeds pulse p95 by more than one fixed tick under the same profile, or any cue/state
  convergence gate fails, return to specification review before changing channel/batching semantics.
- [ ] Real supervised UDP runs select and exercise each weapon through normal client messages/native
  input and exit only after server outcome and both-client observations are proven.
- [ ] Format, role-specific Clippy/tests/builds, network tests, server feature isolation, prior
  performance, process shutdown, and fixed-port cleanup all pass on the final tree.
- [ ] Recorded worst-case fixtures retain at least 100 valid active fighters and cover: 32 concurrent
  scatter attacks (224 pellets), 16 same-tick lob landings with populated area candidates, 32
  simultaneous blade sectors with populated candidates, and 100 active slow/external-motion states.
  Record each focused case and one combined case; every case keeps p95 authoritative fixed step
  below 16.67 ms and asserts its intended population/resolution survived the sample.
- [ ] Windowed keyboard/mouse at 30/60/high render profiles confirms selection, every aim preview,
  distinct delivery/impact/effect feedback, HUD economy, and no stale visuals.
- [ ] Physical Xbox-like controller confirms selection navigation/confirm, right-stick previews,
  RT cadence, landing readability, close-range blade/scatter readability, slow/knockback feedback,
  and pause/controller reconnection behavior on target hardware.

### Evidence rules

- Every accepted attack must originate from a selected fighter's Lightyear native input. Direct
  runtime/effect mutation is valid only in narrowly named unit tests.
- Selection authority evidence must enter through the registered request on the session's real
  receiver; tests may not directly insert `SelectedBuild` and call that selection validation.
- Geometry tests exercise Avian sweep/intersection/terrain queries plus the shared pure boundary
  math; spawning an already-overlapping target alone is insufficient.
- Network assertions compare stable player/network/attack/definition/event identity, never local
  Bevy entity IDs.
- Durable replicated selection/effect/projectile state and transient readable feedback are both
  required; neither substitutes for the other.
- Statistical conditioner results include profile, direction, repetitions, run IDs, median/p95,
  cue volume, and convergence/failure result. Exact duplicate/reorder cases use the deterministic
  fixture.
- Attack-to-feedback is measured from the server's accepted-attack epoch sample to each client's
  matching `AttackAccepted` cue by stable `AttackId`. State convergence is measured from the
  server's authoritative mutation epoch to each client's matching durable replicated state by
  stable target/event identity. Epoch sampling is evidence-only and never used for game rules.
- History-capacity tests exceed 512 outcomes and the maximum cue-dedup window, then prove retained
  ordering, drop counters, ongoing aggregate accuracy, and absence of stale presentation.
- Visual/controller checks complement authority and lifecycle automation and cannot replace it.

## Visual and user smoke-test plan

The playtest handoff will provide a supervised one-server/two-client command plus one explicit demo
per weapon. The requested scenario:

1. Join without an explicit weapon, navigate all four entries with controller and keyboard, confirm,
   and verify the fighter cannot act before server confirmation.
2. Compare pulse at close/mid/long range; identify its six-shot pressure, cooldown, reload, and
   projectile/impact source.
3. Use scatter at point-blank, edge-of-range, and beyond range; identify the cone, pellet paths,
   falloff, four-attack burst, and reload vulnerability.
4. Aim the launcher across central cover and near a wall/boundary; compare indicator to landing,
   dodge the telegraph, observe blast occlusion, owner damage/knockback, hostile slow, and long
   recovery.
5. Use the blade on one and multiple targets near open ground and behind cover; identify the sector,
   three-charge burst, knockback, recharge, and inability to hit through walls.
6. Have two clients fire different weapons simultaneously; verify source/weapon readability,
   friendly filtering, mutual/self defeat attribution, reset, and no stale effects.
7. Disconnect/reconnect during spread/lob/slow, then repeat under the adverse profile; report
   orphan visuals, duplicate/missing feedback, selection issues, cue delay, and state convergence.
8. Rank each weapon's preferred distance, burst/recovery clarity, strongest counterplay, controller
   readability, and whether any value feels obviously nonviable or dominant.

Known limitations must state: no owner/projectile prediction or lag compensation; selection is an
initial four-preset sandbox gate rather than a lobby or weapon editor; clients cannot submit custom
recipes; weapon switching requires reconnect/restart; lobs use fixed directional range; melee is
instantaneous; slow is one immediate non-accumulating effect; art/audio are placeholders; the
greybox is not the Milestone 06 authored arena; no score/match loop, build budget, ultimate,
passive, persistent arsenal, acquisition, advanced trajectory, or production content hot-reload
exists.

## Exit checklist

- [x] Research questions are resolved or explicitly deferred with rationale.
- [ ] Technical specification is validated by the user.
- [ ] The locked M04 baseline is green before production implementation begins.
- [ ] Content/rules and preset recipes are validated, content-fingerprinted, headless-safe, and
  change values without combat-code rewrites.
- [ ] Code-owned engine ceilings cannot be widened by content policy; canonical resolved snapshots
  remain bounded on the wire and every runtime/evidence collection has an explicit lifetime/cap.
- [ ] Preset-independent structural and fighter-context activation resolution create immutable
  server-owned snapshots and resolve a legal non-preset configuration fixture without adding a new
  weapon-specific system or preset-ID branch.
- [ ] All four weapons use the shared economy/attack/payload/lifecycle pipeline and only focused
  delivery-specific geometry.
- [ ] Preset selection is server-looked-up/resolved, owner-mapped, idempotent, locked after
  acceptance, rejects client-authored specs, and is usable with controller and keyboard/mouse.
- [ ] Straight spread, lob/area, melee, damage/falloff, knockback, and slow rules pass fixed-schedule
  and network authority tests.
- [ ] M04's approved disconnect-before-combat, same-tick projectile, impact-fraction ordering,
  atomic event-ID, global cue ordering, neutral-hostility, bounded-evidence, and completed-tick
  publication contracts remain proven for the generalized pipeline.
- [ ] Durable selected/effect/projectile state and stable transient feedback recover/converge under
  late join, reconnect, loss, duplication, reordering, latency, and jitter.
- [ ] Cleanup is correct on expiry, defeat, reset, target/source disconnect, reconnect, and stop.
- [ ] Per-weapon attack/use/hit/damage/distance/defeat/self-damage telemetry is exact and locally
  inspectable.
- [ ] Each weapon has observed preferred distance, burst window, recovery window, and counterplay;
  numeric tuning changes are recorded without changing authority contracts.
- [ ] Windowed and physical-controller checks make all selection, aim/range, economy, hit, area,
  effect, defeat, and reset states understandable.
- [ ] Dedicated server remains free of renderer/window/audio/device-input/Bevy-asset features and
  the final performance/process gates pass.
- [ ] Player-facing recipe editing, balance costs, persistent arsenal, entitlements, currency,
  loot, and progression remain outside M05 while their integration seams are explicit.
- [ ] Bouncing, homing, curved, piercing, splitting, boomerang, and accumulating status behavior
  remains deferred.
- [ ] User feedback is triaged, affected verification reruns, learn-from-errors is recorded, and
  roadmap/current milestone status is updated before completion.
