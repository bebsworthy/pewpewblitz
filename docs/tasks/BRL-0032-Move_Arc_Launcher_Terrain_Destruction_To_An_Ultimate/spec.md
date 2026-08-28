# Outcome

Arc Launcher remains a controllable lobbed area weapon with its current ammunition, cadence, range, flight, hostile damage, knockback, and slow behavior, but its attacks no longer destroy map placements. A new saved-brawler-selectable Demolition Strike ultimate spends full charge on one confirmed target point and applies the existing bounded authoritative map-destruction transaction there.

The first slice is complete only when the content is playable through the routed product and Practice paths, connected and recovering clients converge, the effect is readable before confirmation and at impact, and the historical Arc Launcher terrain-reservation feedback is closed with native balance evidence.

# Research basis

## Current implementation

- Arc Launcher is weapon preset 3 in `content/catalogs/weapons.ron`. Its single lobbed delivery currently carries `DestroyMap(radius: 48.0)` beside a radius-150 hostile damage/knockback/slow payload.
- Weapon-level world effects are authored in `WeaponRecipe.world_effects`, normalized and validated with the weapon recipe, copied through delivery execution, committed as one `CombatWorldEffectFact` per delivery effect, and consumed by `src/map/runtime/dynamics.rs`.
- Map authority sorts and deduplicates committed facts, evaluates only placements whose `MapDestructionBehavior` permits removal or replacement, updates `MapDynamicState` atomically, removes affected colliders, publishes ordered mutation events, and serves bounded recovery snapshots. Hit-point objects, objectives, mode anchors, spawns, indestructible placements, and out-of-bounds cells are not bypassed by this path.
- Targeted ultimates already use a two-phase client interaction: press Ultimate to arm, aim with the ordinary quantized aim axis/distance, press Primary to confirm, or cancel/press Ultimate again. The server receives only the existing one-tick ultimate intent and derives the center authoritatively.
- Reveal Scan and Concealment Field already share `reveal_scan_center`: committed aim when present, facing fallback, requested distance clamped to authored range, and center clamped to playable bounds.
- Ultimate activation runs in `AbilitySet::Activation` during `FixedUpdate`, after authoritative input and before gameplay simulation. Map destruction runs in `MapRuntimeSet::ApplyDestruction` during `FixedPostUpdate`, after combat damage and ability outcome observation and before map publication/mode rules.
- `ResolvedMatchLoadout` and `AbilityState` already replicate. Targeting does not require a new input message or a new persistent ability phase. Map mutation/recovery remain the durable result; a combat cue is the transient presentation fact.
- The saved-brawler catalog, profile validation, Dashboard/editor flow, Balance Lab snapshot/editor, protocol registry, cue codec, automation, bots, evidence, and tests contain closed ultimate inventories or exhaustive matches that must advance together.
- Practice bots currently submit ordinary validated `FighterInput`, but their policy treats the ultimate bit as Dash-shaped intent. Demolition Strike needs an explicit deterministic policy based only on delayed visible targets/objectives and public map state; it must not gain a direct map-mutation path.
- The historical V1 playtest explicitly reserved terrain destruction for a future ultimate or special item. It also found the Arc Launcher's radius-150 combat area versus radius-48 crater hard to read and recorded 64 units as the established authored brush ceiling to reconsider with the new carrier.

## Risk findings

- Repowise local retrieval was high quality but synthesis was degraded because no LLM provider was configured; conclusions above were therefore verified against live source and checked-in documentation.
- `src/builds/definitions.rs` is a churn hotspot with a prior fix in `validate_ultimate_inventory`; inventory count, ordered IDs, kind/parameter pairing, fingerprint material, advertised catalog, and Balance Lab topology require focused characterization tests.
- `src/combat/definitions/mod.rs` has broad structural reach. This ticket changes only Arc Launcher's authored effect and the resulting exact-version fingerprints; it does not remove or prohibit generic weapon world effects.
- `activate_reveal_scan` already carries high complexity and duplication. Demolition Strike gets its own focused ability module and reuses a renamed generic target-center helper rather than adding another branch to Reveal Scan.
- Map destruction is cohesive and already tested. The change at that boundary is limited to accepting a semantically correct ultimate source key in the existing bounded transaction.

# Product decisions and initial tuning

| Property | Decision |
| --- | --- |
| Stable definition | `UltimateDefinitionId(6)`, key `demolition-strike`, display name “Demolition Strike” |
| Selection cost | None; saved brawlers use slot-only selection with no point budget |
| Activation | Existing targeted two-phase interaction; confirmation resolves instantly on the next authoritative activation tick |
| Maximum range | 520 world units (`520000` milliunits), matching Arc Launcher's established maximum lob distance |
| Destruction radius | 64 world units (`64000` milliunits), the established authored safety ceiling |
| Charge | Full `ULTIMATE_CHARGE_MAX`; one accepted activation resets charge exactly once |
| Recipients/outcomes | Terrain only; no fighter, deployable, objective, or damageable-object damage; no slow, knockback, status, or charge gain |
| Target legality | Any finite point derived from valid input and clamped to playable bounds, including a point inside currently destructible cover |
| Empty target | Valid but wasteable: an accepted activation consumes charge and presents impact even when no eligible placement changes |
| Timing | No wind-up, projectile, interruptible cast, persistent field, or new `AbilityPhase`; accepted use returns immediately to `Charging` |
| Persistence | The map mutation is durable and recoverable; the activation/impact cue is transient and is not replayed to late joiners |
| Presentation | Pre-confirmation range line plus 64-unit landing ring; accepted impact burst/ring and ordinary map transition; reduced-effects equivalent |
| Audio | Reuse the established bounded impact one-shot in the first slice; no new asset is required |

The 64-unit radius is an initial balance value, not a new engine ceiling. Native 3v3 testing may tune it downward within the established 8–64 unit authored range without changing architecture or scope.

# Scope and technical design

## 1. Preserve generic weapon world effects; remove only Arc Launcher's destruction

- Remove `DestroyMap` only from the canonical Arc Launcher recipe. Preserve all other Arc Launcher values and its presentation profile.
- Keep `WorldEffectDefinition`, `WeaponRecipe.world_effects`, their policy/engine limits, delivery propagation, Balance Lab support, and validation available for other present or future weapon recipes.
- Replace the old “only Arc Launcher carries destruction” test with two separate contracts:
  1. canonical Arc Launcher has no world effect and repeated committed lobs cannot request map destruction;
  2. a valid synthetic single-fire lobbed weapon can still carry a bounded `DestroyMap` effect, proving this ticket did not create a global weapon prohibition.
- Arc Launcher no longer exposes a map-destruction radius in Balance Lab because its authored world-effect list is empty. Generic weapon world-effect editing and shape validation remain intact.

## 2. Add a sixth authored ultimate

Extend the closed build model with:

`UltimateKind::DemolitionStrike`

`UltimateParameters::DemolitionStrike { maximum_range_milliunits: u32, radius_milliunits: u32 }`

- Its activation style is `Targeted`.
- Build and advertised-catalog validation require the kind/parameter pair, stable ordered ID 6, maximum range in the existing bounded targeted range, and a radius from 8,000 through 64,000 milliunits in 4,000-milliunit steps.
- Add the canonical definition and update inventory-count assertions, saved-brawler/catalog tests, display-name tests, and wire-size ceilings.
- Existing saved profiles remain valid because they store stable ultimate IDs and IDs 1–5 retain their meaning and order.
- Rename `reveal_scan_center` to a generic ability-owned helper such as `targeted_ultimate_center`; keep its current deterministic input/facing/range/bounds behavior and focused tests.

## 3. Authoritative activation and exact-once source identity

Create `src/abilities/demolition_strike.rs` and register its activation system in the existing explicit `AbilitySet::Activation` chain.

For each fighter equipped with Demolition Strike:

1. Edge-detect the existing ultimate intent through `UltimateInputLatch`.
2. Record one activation attempt.
3. Reject stale input, defeated/inactive fighters, non-ready or non-full charge, or exhausted event/generation identity without changing charge or map state.
4. Resolve maximum range/radius from the validated loadout and derive the target center from authoritative position, facing, committed aim, quantized requested distance, and playable bounds.
5. Reserve one stable combat event ID before accepting.
6. Commit acceptance atomically: increment the existing ultimate generation, reset ability to zero-charge `Charging`, remove spawn protection, append one destruction fact, append one activation cue, and record one Demolition Strike acceptance.

A client-side targeting cancellation never sends the ultimate intent and therefore never reaches this system. Because accepted activation is instantaneous, there is no post-acceptance interruption state. “Rejected or canceled” means no charge consumption, no destruction fact, and no impact cue.

Do not fabricate a weapon `AttackSource`. Generalize the server-only `CombatWorldEffectFact` source into an explicit bounded source enum:

- weapon delivery: existing `AttackSource` plus delivery/effect indices;
- ultimate activation: event ID, owner network entity ID, and ultimate definition ID.

The fact continues to carry tick, authoritative position, and `WorldEffectDefinition`. Sorting/deduplication uses a stable key that includes source kind and the existing weapon key or ultimate event ID. No process-local `Entity` enters the fact or wire protocol.

## 4. Reuse the existing map transaction and recovery path

- Demolition Strike emits `WorldEffectDefinition::DestroyMap { radius: 64.0 }` into `CombatWorldEffectFacts`; `MapRuntimeSet::ApplyDestruction` remains the only authority that decides placement outcomes.
- Preserve transaction capacity, sorted/deduplicated processing, revision increments only for non-empty transition sets, collider removal, replacement behavior, reliable mutation publication, restart restoration, recovery admission/rate limits, and late-join convergence.
- An accepted no-op remains a destruction request in telemetry but creates no `MapMutationEvent` and does not advance map revision.
- Existing protections remain structural: only `RemoveOnMapDestruction` and `ReplaceOnMapDestruction` placements transition. Indestructible placements, hit-point world objects, Heist safes, spawns, objectives, mode anchors, and already-terminal placements are unchanged.
- Restart/teardown clears pending world-effect facts through the existing restoration path. Defeat, disconnect, or build replacement after an accepted instantaneous activation does not roll back an already committed request.
- Add source-aware telemetry sufficient to distinguish Demolition Strike requests, applied requests, no-ops, and placements changed while retaining the existing aggregate map totals.

## 5. Protocol, compatibility, and content floors

This is one exact-version cutover with no compatibility decoder.

Advance together:

- `SUPPORTED_PROTOCOL_VERSION` 31 → 32 because replicated loadouts/cues contain new enum variants;
- build catalog schema and fingerprint format 7 → 8;
- build catalog balance revision;
- weapon catalog schema 4 → 5 and weapon fingerprint format 2 → 3 because Arc Launcher's canonical recipe changes;
- gameplay content envelope 17 → 18;
- advertised brawler catalog format/revision material 1 → 2;
- Balance Lab snapshot schema 9 → 10, persistence schema 4 → 5, and editor schema 2 → 3.

Do not bump the saved-profile model or database schema: existing saved brawlers remain representable and should load unchanged. Do not add per-message versions or stale-content migration. Stale peers/content/Balance Lab snapshots fail closed through their existing exact-version boundaries.

Register no new map protocol message. Add one ordered `CombatCue::DemolitionStrikeActivated { event_id, tick, source, center, radius_milliunits }` and matching `CombatCueKind`/codec support. Durable state continues through existing replicated `AbilityState`, `ResolvedMatchLoadout`, `MapDynamicState`, mutation messages, and recovery snapshots.

## 6. Client interaction, HUD, 3D presentation, audio, and evidence

- Include Demolition Strike in the existing targeted-ultimate input gate. Ultimate arms targeting; Primary confirms while suppressing the weapon shot until release; Cancel or Ultimate cancels.
- Extend the targeted preview match to read Demolition Strike parameters. Show the existing origin-to-center range line and an area ring at the authoritative-equivalent center. The ring is not repaired away from destructible cover because cover is a legal target.
- Give the demolition preview/impact a distinct high-energy terrain color/material from Reveal Scan and Concealment Field while retaining primitive and reduced-effects fallbacks. Visual absence never affects authority.
- Consume the new cue through existing ordered deduplication, process capture, cue encode/decode, transient-effect capacity, and audio capacity. Render a short 64-unit impact ring/burst; map visuals then rebuild from authoritative mutation state.
- The HUD continues to use replicated charge/phase and catalog display name. While targeting, it must distinguish “confirm Demolition Strike” from ordinary weapon fire. Not-ready/defeated/inactive replicated state cannot arm targeting. Rare server-side stale/identity rejections remain visible in telemetry/evidence; no general ability-rejection wire framework is added in this slice.
- Add `DemolitionStrikeAccepted` and specific use totals to bounded ability telemetry. Add demolition source totals to map telemetry and the server verification report. Existing cue stream plus map revision/digest proves presentation/mutation convergence.
- Update evidence capture/codec tests and any exhaustive cue/source matches. Do not create a second terrain evidence format.

## 7. Practice bots and automation

- Bots continue to write only ordinary `FighterInput` and pass through the same freshness, charge, target-center, and activation validation as humans.
- Extend the delayed bot observation with the equipped ultimate kind/range needed for policy; do not expose hidden opponents or server-only mutation decisions.
- For Demolition Strike, a bot may confirm only when ready and it has an ordinary permitted aim target from its delayed observation (visible hostile fighter or public hostile objective) within authored range. It aims at that public target and may legitimately produce a no-op if no destructible placement overlaps.
- Keep Dash's existing distance/escape policy unchanged. Do not reinterpret all ultimate kinds through the current Dash flag; use a small explicit Demolition Strike decision branch and cooldown latch.
- Headless product automation must be able to equip ultimate ID 6, charge it through existing test controls/evidence fixtures, and emit the same ultimate input. No direct test-only map mutation is accepted as feature evidence.

## 8. Balance Lab and saved-brawler UX

- Advertise Demolition Strike in the server-owned brawler catalog so creation/edit cycling can select it without a hard-coded client list.
- Include its maximum range and destruction radius in Balance Lab's ultimate section with world-unit labels and the 8–64/step-4 radius constraint.
- Applying revised tuning must preserve stable identity and parameter topology, recompute build/content/catalog fingerprints, update admitted loadouts through the existing reset workflow, and reject malformed/out-of-range values.
- Arc Launcher no longer exposes a destruction field, while a synthetic/destructive weapon recipe remains valid under generic weapon policy tests.
- Restore-canonical-defaults and persisted-session version handling must cover the new schemas.

# Implementation plan

## Phase 1 — Characterize and cut content/schema floors

- Add/retain characterization tests for Arc Launcher combat values, the current destruction transaction, targeted center calculation, map recovery, cue codec, catalog bounds, and saved-profile compatibility.
- Add Demolition Strike enums/parameters/definition and advance the exact-version constants.
- Remove only Arc Launcher's authored `DestroyMap`; update weapon/build/catalog/profile/Balance Lab fixtures and fingerprint expectations.
- Prove another valid synthetic weapon can still carry `DestroyMap`.

Exit: catalogs load, old stable IDs remain unchanged, Demolition Strike resolves into saved-brawler loadouts, Arc Launcher has no world effect, and focused definition/profile tests pass.

## Phase 2 — Authoritative vertical slice

- Add the focused activation module and generic targeted-center helper.
- Generalize server-only world-effect source identity and emit the ultimate destruction fact/cue/telemetry.
- Integrate source-aware sorting/deduplication and telemetry into the unchanged map transaction.
- Add focused ECS/schedule tests for acceptance, every rejection class, cancellation/no intent, accepted no-op, exact-once charge/fact/cue, eligible placement transitions, protected placements, same-tick ordering, restart clearing, and identifier/capacity exhaustion.

Exit: one authoritative confirmed input changes eligible terrain exactly once in the intended fixed tick; rejected/canceled inputs change nothing.

## Phase 3 — Product presentation, bots, and Balance Lab

- Extend targeted input/preview, HUD prompt, cue ingestion/codec, 3D impact, reduced effects, and audio.
- Add saved-brawler display/selection and Balance Lab descriptors/apply/restore/persistence support.
- Add the explicit bot/automation branch using permitted delayed observations and ordinary input.
- Update process/server evidence fields and tests.

Exit: keyboard/mouse, gamepad-shaped tests, headless automation, and Practice bots all exercise the same validated path; presentation agrees with authority.

## Phase 4 — Routed convergence, performance, documentation, and feedback

- Add a network scenario that equips ultimate ID 6, reaches full charge, confirms on destructible cover, and proves charge, cue, map revision/transition, connected-client convergence, late-join/recovery convergence, restart restoration, and a second-generation use.
- Add a regression scenario that repeatedly lands Arc Launcher attacks without changing map state.
- Extend maximum-overlap performance coverage to six simultaneous radius-64 activations on the densest supported 3v3 map, including no-op and overlapping brushes.
- Run canonical gates, routed 3v3 product and Practice evidence, Balance Lab operator verification, and native readability/balance playtesting.
- Reconcile durable docs and record feedback/learning before closeout.

Exit: every acceptance criterion and required evidence below is recorded, feedback is dispositioned, and `ticket sync` is clean.

# Verification

## Focused automated verification

- Build/catalog/profile tests:
  - six ordered ultimates with stable IDs 1–6;
  - Demolition Strike kind/parameter/range/radius/step validation;
  - saved-profile compatibility and advertised-catalog byte bounds;
  - Arc Launcher unchanged except empty world effects;
  - synthetic non-Arc weapon `DestroyMap` remains valid;
  - expected fingerprint/version changes and stale-version rejection.
- Ability/ECS tests:
  - targeted center aim/facing/distance/bounds behavior;
  - exact input edge/latch behavior;
  - accepted use emits one cue and one ultimate-source fact, consumes charge once, removes spawn protection, and returns to `Charging`;
  - stale, defeated, inactive, uncharged, repeated-held, invalid identity, and canceled paths emit no fact/cue and consume no charge;
  - accepted empty target consumes charge and records a no-op without map revision.
- Map tests:
  - stable cross-source ordering/deduplication;
  - radius-64 whole-placement overlap;
  - remove/replace exact-once transitions;
  - indestructible/durable/objective/spawn/anchor protection;
  - capacity deferral, restart clearing/restoration, collider updates, mutation byte bounds, and recovery.
- Client tests:
  - arm/confirm/cancel and primary suppression;
  - preview center/range/radius parity including targeting inside destructible cover;
  - cue deduplication/codec, effect capacity/lifetime, reduced-effects fallback, HUD prompt, and audio key.
- Bot/Balance Lab tests:
  - bot uses ordinary ultimate input only for permitted delayed public targets;
  - no hidden target access or direct mutation;
  - ultimate fields are editable with correct units/bounds;
  - identity/topology, apply/reset, persistence, stale schema, and fingerprint propagation.

## Integration and performance

- Representative separate-App/network scenario for authoritative activation and two clients.
- Connected plus late-joining/recovering client map convergence.
- Match restart and second-generation activation.
- Arc Launcher no-destruction network regression.
- Six simultaneous/overlapping radius-64 activations and dense-placement transaction capacity/performance.
- Protocol registration, serialization byte ceilings, server-only feature isolation, and deterministic schedule ambiguity checks.

## Canonical commands

Run and record, using repository commands rather than substitutes:

1. `just fmt`
2. focused Rust tests during each phase
3. `just check`
4. `just lint`
5. `just test`
6. `just e2e 6`
7. `just practice-e2e wipeout-3v3` (or the exact supported 3v3 Practice identifier reported by the CLI)
8. `just balance-lab` for operator/native tuning verification

If the Practice command does not accept a 3v3 identifier, use the nearest canonical 3v3 Practice preset documented by `just --list`/the operator help and record the exact command rather than inventing a new script.

# Native playtest handoff

Use a saved brawler equipped with Arc Launcher and Demolition Strike on a map with destructible cover.

Verify with keyboard/mouse and, when available, a physical controller:

1. Arc Launcher lobs retain aim, landing, damage, knockback, slow, cadence, ammunition, and audio but never change cover.
2. Ultimate press clearly enters Demolition targeting; the line and 64-unit ring follow aim/distance and can sit inside destructible cover.
3. Cancel exits cleanly and does not fire the weapon or spend charge.
4. Confirm produces one readable impact, spends full charge once, changes only eligible cover, and immediately returns the weapon controls.
5. Empty, edge, corner, already-destroyed, indestructible, durable-object, spawn, objective, and mode-anchor targets behave predictably.
6. Connected observers see the same crater/replacement; restart restores the map; late join/recovery matches the authoritative revision.
7. In 3v3 overlap, the effect is distinguishable from Arc Launcher combat area, Reveal Scan, Concealment Field, explosions, and ordinary map-object destruction.
8. Judge whether radius 64 creates meaningful route change without erasing too much cover for one charged use. Tune downward only with recorded feedback and affected tests rerun.

Requested observations: targeting clarity, confirmation/cancel feel, crater predictability, sound/impact strength, cover removed per charge, counterplay, accidental no-op frequency, and 3v3 visual load.

# Durable documentation

Update after behavior is verified:

- `docs/03-weapons-and-abilities.md`: Arc Launcher behavior and Demolition Strike definition/tuning/counterplay.
- `docs/16-grid-map-asset-system.md`: ultimate-source destruction entry while preserving map authority/recovery and generic weapon capability.
- `docs/08-network-architecture.md`: only if the new ultimate-source fact/cue changes an enduring authority/replication statement; otherwise record that the existing boundary held.
- `docs/13-player-ux.md`: targeted ultimate arm/confirm/cancel and HUD feedback if not already generic enough.
- `docs/15-balance-lab.md`: Demolition fields, bounds, persistence/version behavior, and Arc Launcher field removal.
- `docs/10-bots.md`: permitted Demolition observation/action rule.
- `docs/11-art-and-presentation-direction.md`: distinct preview/impact and reduced-effects degradation if the existing general cue contract is insufficient.

Do not reopen or rewrite historical V1–V12 implementation records. Reference the historical terrain-reservation decision from this ticket and update only current durable behavior.

# Non-goals

- No global prohibition on weapon-authored `DestroyMap`; only Arc Launcher loses it.
- No removal of `WorldEffectDefinition`, `WeaponRecipe.world_effects`, generic weapon validation, or Balance Lab support for another valid destructive weapon.
- No fighter damage, healing, knockback, slow, elemental/status effect, structural collapse, chained destruction, durability damage, objective damage, or arbitrary brush authoring from Demolition Strike.
- No wind-up/projectile/persistent field, rollback, client prediction, client-side membership/outcome claim, or new input message.
- No compatibility decoder, per-message version, stale saved-content migration, or parallel terrain runtime.
- No new audio/mesh/texture asset requirement; existing deterministic assets and reduced-effects fallbacks are sufficient for the first slice.
- No general bot-ultimate strategy redesign beyond the explicit Demolition Strike branch.
- No unrelated map-catalog decomposition from BRL-0024.

# Acceptance criteria

- [ ] Canonical Arc Launcher has no `DestroyMap`; repeated focused/network/native lobs preserve all other accepted behavior and never mutate map state.
- [ ] Generic weapon-level `DestroyMap` remains supported and is proven by a valid non-Arc synthetic recipe test.
- [ ] Demolition Strike is stable ultimate ID 6, selectable in saved-brawler flows with no point cost or budget, targeted, 520-unit range, and initially radius 64.
- [ ] An accepted fully charged confirmation emits exactly one ultimate-source destruction fact and one ordered cue, consumes charge exactly once, and changes each eligible placement at most once.
- [ ] Client cancellation and server rejection paths emit no destruction/cue and consume no charge; held/redundant input cannot duplicate activation.
- [ ] Accepted no-op targeting spends charge, presents impact, records a no-op, and does not advance map revision.
- [ ] Only existing removable/replaceable placements transition; indestructible and hit-point objects, safes, spawns, objectives, anchors, bounds, and already-terminal placements remain protected.
- [ ] Fixed-tick ordering, transaction capacity, restart restoration, collider state, mutation publication, connected clients, late join, recovery, teardown, and second-generation use converge through existing map authority.
- [ ] Target preview matches authoritative center/range/radius, permits aiming inside destructible cover, suppresses accidental primary fire, and has distinct normal/reduced-effects presentation and bounded audio.
- [ ] Practice bots and headless automation use ordinary validated input and only permitted observations; no parallel authority path is introduced.
- [ ] Build/weapon/content/advertised/Balance Lab/protocol compatibility floors advance together; stale peers/snapshots fail closed and existing saved profiles remain valid.
- [ ] Focused tests, `just check`, `just lint`, `just test`, routed 3v3 product/Practice evidence, maximum-overlap performance, Balance Lab operator checks, and native readability/balance playtesting pass.
- [ ] Feedback is recorded as implemented, deferred to another ticket, rejected with rationale, or awaiting evidence; affected verification is rerun after accepted corrections.
- [ ] Durable documentation reflects final behavior, a proportional learn-from-errors review is recorded, and `ticket sync` completes without conflicts before the ticket moves to `done`.

## Implementation evidence — 2026-08-28

- Removed only Arc Launcher preset 3 `DestroyMap`; generic weapon world-effect definitions, validation, planning, and map authority remain.
- Added stable ultimate ID 6 `Demolition Strike` with targeted input, 520-unit range, radius 64, full-charge spend, typed ultimate provenance, ordered cue, distinct preview/impact material, bounded audio, and Demolition-specific map telemetry.
- Reused the existing map destruction transaction for eligible placement mutation, no-op handling, collider removal, restart restoration, publication, and recovery.
- Added saved-brawler advertisement/selection, protocol/content schema floors, Balance Lab editing and v4 snapshot migration, and an ordinary-input Practice bot branch.
- Updated durable weapon/ability, bot, Balance Lab, and grid-map documentation.

## Verification evidence — 2026-08-28

- `cargo test --lib --features server,client`: 604 tests passed after focused changes.
- Focused Demolition activation/held/stale rejection, map mutation/no-op/revision, bot input, saved-brawler, Arc regression, and generic synthetic weapon-destruction tests passed.
- `just check`: passed, including client-only, server-only, network-test, Balance Lab, and web build.
- `just lint`: passed with warnings denied and feature/map cleanup guards.
- Canonical Balance Lab all-target tests: 357 passed; persisted schema 4/8 state migrates recovery values, removes legacy Arc destruction, and adds canonical Demolition tuning.
- Canonical separate-App network suite: 88 passed; performance suite: 12 passed.
- `just practice-e2e wipeout-3v3`: passed; reached Active with one human and five manifest bots.
- `just e2e 6`: blocked by pre-existing harness assignment of weapon presets 5 and 6. Recorded as related backlog BRL-0044; the command timed out after both invalid clients exited.
- `git diff --check`: passed. Repowise classified the live change Typical at the 48.3 risk percentile with moderate review priority; its inferred impacted test families were covered by the canonical suites above.

## Feedback and remaining evidence

- User correction implemented: only Arc Launcher loses destruction; generic weapon destruction remains.
- User correction implemented: saved-brawler selection has no point cost/budget. Residual catalog point fields remain solely as legacy schema machinery and are deferred to related backlog BRL-0043.
- Native keyboard/mouse/controller readability and balance playtesting remains required before closing this ticket. The six-client E2E gate also remains pending BRL-0044.

## Learn-from-errors review

- Initial Balance Lab schema bump handled current snapshots but not the complete historical 3→4→5 migration chain. Cause: updating version constants before modeling each old shape. Prevention: migrate one historical schema step at a time and test removal/addition of changed topology, not only numeric fields.
- The first canonical run also exposed stale editor field-count and Arc topology expectations. Prevention: when removing an authored field while preserving its generic capability, audit editor descriptors and topology tests separately from engine-level validation.
