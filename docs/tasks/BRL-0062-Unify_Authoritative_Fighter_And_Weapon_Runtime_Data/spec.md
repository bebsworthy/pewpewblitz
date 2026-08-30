# BRL-0062 implementation specification

## Outcome

Remove the live dual fighter/weapon balance path. Every production fighter generation must receive validated runtime values derived from the embedded build and weapon catalogs, and lifecycle or presentation code must never silently fall back to divergent code-authored defaults.

## Scope and decisions

- Preserve current stable wire identity types and protocol compatibility.
- Preserve server authority, fixed-tick schedules, deterministic reset behavior, and client presentation boundaries.
- The canonical sources are the validated build catalog, weapon catalog, selected saved-brawler recipe, resolved match loadout, and map spawn data.
- Spawn facing comes from the map spawn. Health, movement speed, recovery, resistance, ultimate/passive values come from the resolved fighter/loadout. Ammunition, delivery, payload, cooldown, and recovery come from the resolved primary weapon.
- Fighter body radius remains a validated authored gameplay value. Place it in the smallest existing fighter/build schema that can resolve it without introducing another catalog.
- Runtime components are immutable projections for a fighter generation; mutable health, ammunition, cooldowns, effects, and pose remain separate.
- Missing resolved runtime data fails admission or activation with an actionable error. It does not select a legacy default.
- Keep engine-only movement policy such as collision iterations, skin width, and input freshness separate from entity gameplay tuning.
- Do not perform the later BRL-0061 plugin-topology, delivery/effect, registry, or large-file work in this ticket.

## Work plan

1. Characterize product admission, lobby/queue formation, profile resolution, match activation, restart/respawn, Practice, pickups, HUD, Balance Lab, and verification paths.
2. Extend the fighter profile/build schema with the required body geometry and validate/normalize/fingerprint it.
3. Add focused immutable runtime projection components derived from each resolved loadout.
4. Install projections at the existing waiting-phase build/spawn transaction and replicate only fields already required by client behavior.
5. Replace all production fallback reads of FighterDefinitions and WeaponDefinitions with resolved/projection data.
6. Remove those resources and their balance-bearing defaults from production; retain stable IDs only when required by existing contracts.
7. Split MovementTuning so global engine policy cannot override authored entity movement/body values.
8. Update test fixtures to use embedded or intentionally constructed resolved catalog data.
9. Add cross-path tests proving the canonical RON values reach admission, activation, respawn, Practice, pickups, HUD, Balance Lab, and verification.
10. Run focused role tests and canonical verification, document the resulting single-source contract, record feedback/learning, and sync Ticket.

## Acceptance criteria

- No non-test source initializes FighterDefinitions or WeaponDefinitions.
- No product path reads the legacy default health, ammunition, damage, projectile geometry, movement speed, spawn facing, or reset delay.
- The embedded default fighter and Pulse Sidearm values observed after activation and respawn equal builds.ron and weapons.ron.
- Product lobby/queue, direct test fixture admission, Practice, pickups, HUD, Balance Lab, and process verification use the same resolved values or fail closed.
- Fighter body radius is authored, bounded, fingerprinted, and used consistently for collider creation, weapon recipe validation, bounds, and presentation.
- Map spawn facing remains the only spawn-facing source.
- Engine movement policy contains no player balance speed/body/spawn defaults.
- Existing saved profiles and current wire contracts remain compatible.
- Missing/invalid cross-catalog references produce bounded actionable startup/admission errors.
- Focused tests, just check, just test, just lint, just ci, cargo fmt, and git diff --check pass.
- Durable documentation and learn-from-errors notes are recorded, and ticket sync is conflict-free.

## Non-goals

- New weapons, fighter profiles, abilities, modes, or balance changes.
- General runtime registry or dependency-injection work.
- Delivery/effect transaction decomposition.
- Protocol identity redesign.
- File splitting unrelated to removing the fallback path.


## Research decisions — 2026-08-30

- Keep FighterDefinitionId, WeaponDefinitionId, STANDARD_FIGHTER_DEFINITION, and PULSE_SIDEARM_DEFINITION as stable identity/wire compatibility types. They no longer carry balance.
- Add one bounded fighter_body_radius field to BuildCatalog rather than ResolvedFighterStats. Brawler currently has one canonical one-cell fighter footprint, and a top-level field avoids changing the already replicated ResolvedMatchLoadout wire shape.
- Include fighter body radius in the BuildCatalog schema/fingerprint and AdvertisedBrawlerCatalog revision so server resolution and client weapon-part previews consume the same authored geometry.
- Replace FighterDefinition arguments throughout build, profile, weapon, and weapon-part resolution with the validated body radius. Do not introduce a replacement definition catalog.
- Use ResolvedMatchLoadout as the existing immutable generation projection for health, movement speed, recovery, resistance, weapon economy, delivery, and payload. Adding duplicate runtime-stat components is not justified in this stage. Add a focused body component only if a concrete runtime consumer cannot safely use the authored catalog or collider.
- Remove speed, radius, and spawn_facing from MovementTuning. Movement requires a resolved loadout; body geometry comes from authored BuildCatalog/collider, and repair facing comes from SpawnState.
- Keep the body radius immutable through Balance Lab in this ticket. Balance Lab must validate and use the baseline authored radius for weapon/build resolution, but a new live collider-resizing editor is outside scope.
- Client HUD, overhead UI, and prediction remain hidden/suspended while the replicated loadout is absent; they do not display or simulate numeric fallback values.
- Retain existing global compatibility handling. Build and advertised-catalog schema/revision changes are intentional; MatchBuildSnapshotV3 and saved-profile shapes remain unchanged.

## Implementation decisions and delivered state — 2026-08-30

This section supersedes the earlier research note that rejected focused local runtime-stat components. Full compile and consumer tracing demonstrated concrete generation-local consumers in movement, pickups, match activation/restart/respawn, bot authority, presentation geometry, and Balance Lab atomic replacement.

- `ResolvedMatchLoadout` remains the unchanged replicated aggregate and stable wire contract.
- `MatchLoadoutProjection` now installs focused, non-independently-replicated `ResolvedFighterStats`, `FighterBody`, and `ResolvedWeapon` components from that aggregate at the existing generation transaction.
- Balance Lab replaces the aggregate and every focused projection in the same authoritative transaction.
- Missing projection data fails closed in authority and suspends transitional client prediction/HUD presentation; it never selects numeric defaults.
- `FighterDefinitions`, `WeaponDefinitions`, their validation/default resources, `default_fighter_runtime`, and `STANDARD_FIGHTER_RADIUS` were removed. Stable fighter/weapon identity newtypes and constants remain for compatibility only.
- Build catalog schema 15 owns the validated and fingerprinted one-cell fighter body radius. The content envelope is bumped to 23. Existing saved-profile and `ResolvedMatchLoadout` serialized shapes are unchanged.
- `MovementTuning` now contains engine/process policy only. Entity speed comes from `ResolvedFighterStats`, collision radius from `FighterBody`, and repair facing from map-authored `SpawnState`.
- Admission, lobby/queue, saved profiles, product and Practice spawn, match lifecycle, pickups, bots, combat delivery, client prediction, HUD/overhead presentation, 3D fighter geometry, verification, and Balance Lab now resolve through the same catalog/loadout path.
- The opt-in verification dummy retains an explicitly named `TestDummyFixture` policy (100 health, 90-tick reset) and atomically projects that intentionally constructed resolved loadout. It is not a product fighter fallback or definition catalog; its reset delay is carried by the dummy component rather than hardcoded in damage systems.

Implementation state: uncommitted working-tree change based on `97d9cc1b9913`. Unrelated pre-existing workspace changes were preserved.

## Verification evidence — 2026-08-30

Passed:

- `just check`
- `just test`
  - client: 481 tests
  - server: 421 tests
  - Balance Lab: 443 tests
  - routed network: 91 tests
  - performance: 12 tests; worst reported p95 remained below 3.3 ms in the final CI run
- `just lint`, including `-D warnings`, server feature isolation, V3 presentation guard, and V8 map cleanup guard
- `just ci`
  - routed product 1v1, 2v2, and 3v3 reached Active
  - Practice Wipeout, Hot Zone, and Heist at 1v1, 2v2, and 3v3 all reached Active with one human and manifest bots
- `just v3-render-evidence target/brl-0062-render-evidence.txt`
  - both native Metal clients passed at 2560x1440
  - 1,801 samples each; p95 16.893 ms and 16.887 ms; zero frames over 25 ms
  - reports: `target/brl-0062-render-evidence.txt` and `target/brl-0062-render-evidence.txt.peer`
- `cargo check --features client,owner-prediction --all-targets`
- focused revised-catalog replication, Balance Lab atomic replacement, HUD synchronization, projectile fixture, combat reset, and late-join recovery tests
- `cargo fmt --all -- --check`
- `git diff --check`
- removal search found no `FighterDefinitions`, `WeaponDefinitions`, `default_fighter_runtime`, `STANDARD_FIGHTER_RADIUS`, or player speed/radius/spawn-facing reads from `MovementTuning`.

The native evidence exercises the existing accepted gameplay presentation using the catalog-owned body and loadout-driven HUD path. No intentional player balance or wire-shape change was introduced. The only explicit non-product tuning retained is the named verification-dummy fixture policy.

## Feedback disposition and learning review

- No new user correction was received during implementation.
- Existing player-visible balance remains authored by the checked-in catalogs. Where legacy code defaults contradicted the catalog, runtime now observes the catalog rather than preserving the contradiction.
- The first full routed run exposed five integration fixtures that mutated only `ResolvedMatchLoadout` or assumed the legacy 100-health dummy. Root cause: fixture helpers were not updated atomically with the new local projections.
- Correction: fixtures now mutate `ResolvedWeapon` alongside the aggregate when deliberately constructing noncanonical delivery geometry; unchanged-health assertions capture the fixture value; the late-join defeat scenario explicitly owns a vulnerable target; the verification dummy policy is named and projection-coherent.
- Prevention: future generation fixtures should construct or replace `MatchLoadoutProjection` as one bundle, and feature-role plus routed tests should run immediately after introducing a projection. A removal grep must accompany migrations away from live default catalogs.
- A second lint pass caught that adding explicit fixture policy pushed a harness constructor over the line threshold. Extracting `TestDummyFixture::standard` kept the policy named and avoided a broad Clippy suppression.

## Final review corrections and affected revalidation — 2026-08-30

An independent diff review found and corrected two remaining fail-closed gaps before closeout:

- authoritative concealment no longer substitutes a hardcoded 160-unit reveal radius; replicated fighters enter observer visibility decisions only with `ResolvedFighterStats`;
- match roster readiness now requires the resolved aggregate, fighter stats, fighter body, resolved weapon, and map spawn projection before countdown/activation. A focused regression test proves every projection is mandatory.

The reviewer rechecked the corrected diff and reported no remaining blocking issue or analogous fallback in the reviewed authority paths.

Passed on the final corrected state:

- `just check`
- `just lint`
- focused projection-readiness server test
- complete routed network suite: 91/91, including concealment and match lifecycle
- `_e2e-matrix`: product 1v1, 2v2, and 3v3 reached Active
- `_practice-e2e-matrix`: Wipeout, Hot Zone, and Heist at 1v1, 2v2, and 3v3 reached Active
- `cargo fmt --all -- --check`
- `git diff --check`
- final removal search found no production legacy catalogs/default helper/body constant or concealment reveal fallback.
