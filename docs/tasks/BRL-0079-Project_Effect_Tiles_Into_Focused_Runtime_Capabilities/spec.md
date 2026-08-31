# Outcome

Effect-tile runtime consumers depend on focused resolved capabilities rather than the closed authored `MapEffectTileBehavior` variants. Adding a composite or new authored tile behavior requires extending one central projection, while movement, combat cadence, recovery blocking, bot traversal, spawn clearance, and presentation remain independent capability consumers.

# Scope and decisions

1. Keep `MapEffectTileBehavior` and `EffectTileKind` as the closed authored/serde vocabulary. Do not change RON, schema versions, fingerprints, protocol registration, replicated component fields, Balance Lab persistence, visual assets, or built-in content values.
2. Add crate-visible resolved capability values in `src/map/effect_tiles.rs`: ordinary-movement scaling, periodic neutral damage, healing blocking, traversal cost, spawn-clearance behavior, and presentation semantic. Project each authored behavior through one central `capabilities()` function.
3. Add `ResolvedEffectTile::capabilities()` and make occupancy predicates delegate to the projection without adding fields to replicated `EffectTileOccupancy`.
4. Convert runtime consumers in map resolution/runtime, movement authority, bot navigation, and client fighter feedback so they do not match concrete authored variants.
5. Leave exhaustive concrete matching in the central authored projection and typed Balance Lab editor, where the current approved schema is intentionally closed.
6. Preserve exact fixed-tick ordering and semantics: ordinary input movement only, no dash/knockback scaling; neutral damage in EnvironmentReactions; spawn protection respected; no catch-up pulses; all positive-health paths blocked on damaging occupancy; stable bot costs and feedback cues.
7. This phase creates the stable capability contract only. Plugin-populated tile projector/handler registration and ModeRegistry lifecycle changes remain later independently reviewable BRL-0070 work.

# Acceptance criteria

- Focused resolved types/fields represent movement, periodic damage, healing blocking, traversal, spawn clearance, and presentation capabilities.
- Runtime map, movement, bot, recovery predicate, and presentation consumers use capabilities rather than concrete Speed/Slow/Damage variant branching.
- A synthetic composite-capability test proves consumers/predicates do not assume capability families are mutually exclusive.
- Accepted Speed 1250, Slow 700, and Damage 10 every 30 values remain exact; occupancy cadence/order, protection, healing-blocking, movement composition, bot terrain costs, spawn clearance, and feedback mappings remain stable.
- `EffectTileOccupancy` serialization and replication shape is unchanged; no content fingerprint or protocol revision occurs.
- Source audit finds concrete effect-tile matches only in the central projection, typed Balance Lab editing, and focused authored-data tests.
- Focused map/runtime, movement, bot, client feedback, and representative network tests pass.
- Role checks, `just check`, `just lint`, `git diff --check`, and proportional canonical tests pass.

# Verification

- Focused server tests for `map::effect_tiles`, map runtime cadence, movement, and bots.
- Focused client tests for fighter feedback projection.
- Existing representative network effect-tile movement/damage scenarios.
- Client/server role checks, `just check`, `just lint`, and canonical tests proportional to the cross-role refactor.
- No native evidence unless output mappings or assets change materially.

## Implementation and verification evidence

Implemented one headless-safe authored-to-runtime projection in `src/map/effect_tiles.rs`. `EffectTileCapabilities` carries optional ordinary-movement and periodic-damage values plus independent healing-block, traversal, spawn-clearance, and presentation semantics. `ResolvedEffectTile` and `EffectTileOccupancy` delegate to this projection without changing their stored or replicated fields.

Converted production consumers in map capacity/spawn validation, effect-tile occupancy and damage cadence, movement authority, bot navigation, and client fighter feedback. Runtime code no longer branches on concrete Speed/Slow/Damage authored variants; concrete matching remains in the central projection, authored validation/editor ownership, and focused test fixtures. Durable map-system documentation records the authored-enum to orthogonal-capability boundary and explicitly defers plugin registration.

Added a synthetic composite capability proof and a representative two-client network scenario. The network scenario confirms server-owned occupancy, the exact full 30-tick first deadline, 10 neutral damage, replicated occupancy/health on both clients, and combat telemetry.

Verification passed:
- `map::effect_tiles::tests` - 2 passed, including the composite capability proof.
- `map::runtime::effect_tiles::tests` - 3 passed.
- `movement::tests` - 17 passed.
- `bots::tests` - 23 passed.
- Exact client feedback capability mapping test - passed.
- Exact two-client damage-tile authority/replication test - passed.
- Source audit found concrete runtime authored-variant construction only in focused test fixtures; production runtime consumers use capabilities.
- Client/server role checks and `just check` - passed.
- `just lint` - passed across routing, client, server, network-test, Balance Lab, and repository source audits.
- `just test` clean rerun - passed: client 502, server 467, Balance Lab 489, combined catalog replication, all 97 network scenarios, and all 12 performance gates.
- `cargo fmt --all` and `git diff --check` - passed.

## Feedback, limitations, and learning review

No native playtest is required because authored values, assets, material mappings, occupancy shape, and player-visible output are unchanged; focused client mapping and replicated gameplay evidence cover the refactor.

The first canonical rerun failed during LLVM output with `No space left on device`; this was a host artifact-capacity failure, not a code failure. The workspace Cargo target had grown to 183 GiB. `cargo clean -p brawler` removed only regenerable package artifacts (219.4 GiB reported by Cargo), restoring 167 GiB free, and the complete canonical suite then passed from a clean package state. Prevention: inspect disk headroom before consecutive multi-feature optimized test runs and prefer Cargo's package-scoped clean over direct artifact deletion when stale outputs approach the host limit.

The first implementation attempt used const `Option::map` and `Ord::max`, which Rust 1.95 does not permit in const functions. The role check caught this before tests; explicit const-compatible matching preserves the API. Clippy then caught a range-proven narrowing cast and nested presentation condition; the final code uses one narrowly documented cast exception and collapsed control flow. Future capability projections should be compiled against the repository toolchain before broad test execution.
