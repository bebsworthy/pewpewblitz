# V9 Milestone 03 — Allied concealment field and closeout

## Status

**Complete.** M02 completed and was accepted on 2026-08-24. M03 research and this
specification were prepared on 2026-08-24, the user approved implementation the same day, and the
player-visible slice plus Balance Lab surface are implemented. Canonical check, lint, unit,
separate-App network, soak, and performance gates pass. Native feedback identified a saved-brawler
catalog-authority defect; the user approved its remediation specification on 2026-08-24.

The remediation is implemented: protocol version 25 carries a bounded, revisioned brawler catalog
atomically with the profile; storage validates structure while lobby authority validates active
content; the client clears/rebinds connection-scoped catalog state and drives saved-brawler names,
statistics, choices, cycling, automation, and previews from the advertisement. Rejected create/edit
mutations remain on their owning screen. Focused profile/catalog/protocol/client-flow, canonical
role checks/lint, 513 all-feature library tests, 81 separate-App network tests, the routed catalog
handoff test, and 11 performance gates pass. On 2026-08-24 the user confirmed that Concealment
Field works in gameplay and accepted the revised full-screen brawler flow. Feedback is reconciled,
the learning review is complete, and M03 closes V9.

### Server-authoritative catalog verification — 2026-08-24

- `just check` — pass for routing, client, server, network-test, Balance Lab, and web assets;
- `just lint` — pass for formatting, all role-specific Clippy builds, server-feature isolation,
  renderer isolation, and V8 map cleanup;
- `cargo test --lib --all-features` — 513 passed;
- `just test` — pass: routing/process suites, 366 client tests, 282 server tests, 292 Balance Lab
  tests, the revised-catalog routed test, 81 separate-App network tests, and 11 performance tests;
- the active lobby welcome with the advertised catalog stays under the existing 64 KiB bound, and
  oversized game-type and brawler-catalog sequences fail during decode;
- the server rejects an unadvertised fighter ID without changing storage, and synthetic
  non-contiguous fighter/weapon IDs validate without range or modulo assumptions.

## Player-visible outcome

**Concealment Field** is a fifth selectable ultimate and the third V9 concealment source. Pressing
Ultimate at full charge enters the accepted two-phase targeting mode; Fire confirms a visible area
within range without firing the weapon. The accepted field remains publicly readable for its whole
duration and conceals living members of the caster's team whose fighter centers are inside it,
including the caster when the caster is actually inside the circle.

Enemy proximity reveals each concealed occupant only to that nearby observer. Accepted attacks,
positive damage, and Reveal Scan retain their existing higher-priority reveal behavior. Leaving the
field removes this source on the completed authoritative tick, and field expiry or owner removal
cleans up both concealment and the public visual.

M03 also closes V9: it verifies all three concealment sources together, corrects the sentry
perception seam to consume the same visibility rule, completes the missing ultimate Balance Lab
surface, and runs the full security, lifecycle, routed, capacity, presentation, feedback, and
learning gates.

## Scope

### In scope

- one stable Concealment Field ultimate definition and preset, saved-brawler/profile resolution,
  immutable routed handoff, charge, targeting, activation, HUD, audio, telemetry, and Balance Lab;
- one bounded, durable, server-owned field entity with private owner identity and public team,
  center, radius, activation tick, expiry tick, and stable field identity;
- living friendly center-in-circle membership, objective-carrier exclusion, immediate exit,
  observer-specific proximity reveal, and composition with terrain, Self Cloak, attack/damage
  reveal locks, and team-keyed Reveal Scan;
- exact-duration public field presentation in normal, reduced-effects, and primitive fallback
  modes, including late-join materialization and reliable cleanup;
- deterministic overlap, one-field-per-owner and global ceilings, lifecycle/recovery behavior,
  cue privacy, sentry targeting, and full V9 closeout evidence;
- reconciliation of V9 ultimate parameters with the V6 Balance Lab operator contract. M03 research
  found that the current Balance Lab snapshot contains fighter profiles and weapons but not the
  M02 Self Cloak/Reveal Scan parameters; M03 must expose all three V9 ultimate families before V9
  can close.

### Out of scope

- moving fields, field attachment to a fighter, follow-the-caster behavior, field health, field
  destruction, stacking strength, lingering concealment after exit, or an enemy-visible occupant
  list;
- generic runtime regions, arbitrary area effects, a status framework, fog of war, wall/line-of-
  sight vision, or terrain authored through the ability system;
- concealment of projectiles, sentries, objectives, deployables, map assets, or field entities;
- cleanses, reveal immunity, counter-counter abilities, team-shared proximity, spectator rules,
  target memory, or autonomous practice bots;
- final balance claims. M03 supplies tunable defaults and playtest evidence.

## Research findings and proposed decisions

### The field should be durable replicated state, not a long-lived cue

M02's Reveal Scan is an instant transaction, so its accepted cue legitimately carries the
authoritative visual deadline. Concealment Field is different: it affects membership every tick,
must be present for late join/reconnect, and has owner/removal lifecycle. A replicated public entity
is therefore the smallest honest authority boundary.

The server owns two shapes on that entity:

```text
private ConcealmentFieldOwner
  owner network id, owner generation, match id

public ConcealmentFieldState
  stable field id, team, center, radius, activated_at_tick, expires_at_tick
```

Only `ConcealmentFieldState` is registered and replicated to every client. The public field does not
contain its owner's identity, a member list, or links to concealed fighter entities. This keeps the
area and team readable without turning a hidden cast into an exact caster disclosure. A sanitized
public activation cue may repeat field identity and geometry for audio/initial emphasis, but it may
not name the hidden owner.

Late join receives every still-active public field through ordinary current-state replication.
The authoritative entity despawn removes it for all clients; local expiry by replicated simulation
tick is a defensive stale-visual bound, not the authority for gameplay removal.

### Targeting reuses M02's two-phase interaction and clamps consistently

No new input or targeting state is required. `UltimateKind::ConcealmentField` is `Targeted`, so full
charge displays `READY` without a reticle. Ultimate arms the local preview; Fire emits the existing
ultimate intent and remains suppressed until physical release; Ultimate again or Cancel disarms.
Server authority retains the simultaneous targeted-ultimate/primary-fire rejection.

The field adopts the same explicit range policy as Reveal Scan:

```text
direction = committed_aim(aim_update).unwrap_or(authoritative_facing)
distance = requested_distance.unwrap_or(max_range).clamp(0, max_range)
center = (fighter_position + direction * distance).clamp(playable_bounds)
```

Non-finite origin/facing/input, stale input, inactive or defeated lifecycle, wrong loadout,
non-ready charge, identifier exhaustion, or a reached active-field ceiling rejects without spending
charge or creating public facts. The circle may extend outside playable bounds because legal fighter
centers cannot. Preview and authority call one pure targeted-area helper so mouse, controller, and
server geometry cannot drift.

### Membership is center-based, inclusive, and has no caster exception

A fighter receives the field source exactly when all of these are true on the completed fixed tick:

- it is living and an `ActiveCombatant`;
- its team equals the public field team;
- it is not an `ObjectiveCarrier`;
- its authoritative planar center satisfies `distance_squared <= radius_squared`;
- the field is current for the same match and `tick < expires_at_tick`.

The caster is an ordinary friendly candidate, not an unconditional hidden beneficiary. This makes
“includes the caster” mean the caster is allowed to benefit when inside; targeting the field away
from oneself does not grant remote self concealment. Leaving removes the field source in the same
fixed-post source-resolution pass after authoritative movement. No collider, Avian sensor, or
contact event is needed: bounded center-distance evaluation is deterministic, inclusive at the
edge, and avoids one-tick contact-lifecycle lag.

M03 adds the narrow public `ObjectiveCarrier` marker required by the accepted V9 rule. Current
Wipeout and Hot Zone do not produce carriers, but pure/ECS tests attach the real marker and prove
that it blocks every concealment source. A future carrier mode must own adding/removing that marker;
M03 does not invent an objective inventory. Self Cloak activation rejects without spending charge
while carrying; terrain never conceals a carrier; a carrier may cast a Concealment Field for allies
but remains visible while carrying.

### Overlap is a bounded set, not duration or strength stacking

Each caster may own at most one active field. While the field exists, the caster's ability remains
in `AbilityPhase::FieldActive { field_id, expires_at_tick }`; charge may accumulate, but another use
cannot activate until cleanup settles the phase. With routed admission capped at eight participants,
the global active-field ceiling is eight.

Each fighter stores a sorted, deduplicated list of qualifying field IDs, bounded by that same
ceiling. Membership in one or many fields contributes one effective allied-area source. Fields do
not add concealment strength, suppress proximity, add durations, or survive one another; removing a
field removes only its ID. Stable field-ID ordering makes simultaneous activation, overlap, exit,
telemetry, and recovery deterministic regardless of ECS query order.

The source representation changes from the current four-variant terrain/Self Cloak enum to a small
closed value with `terrain`, `self_cloak`, and `allied_field` flags. The observer rule remains:

```text
self or ally                         => visible
no eligible concealment source       => visible
living observer + team forced reveal => visible
living observer + attack/damage lock => visible
any active Self Cloak source         => concealed; proximity does not apply
living observer inside own radius    => visible for terrain/field sources
otherwise                            => concealed
```

Thus Self Cloak plus a field still ignores proximity until Self Cloak ends. Reveal deadlines do not
consume fields or memberships. When a deadline ends, any still-current source applies again.

### Existing schedule seams are sufficient with one ordering hardening

Current M02 activation already occurs before authoritative movement, while concealment resolves in
`FixedPostUpdate` before cue filtering and queues visibility changes for `PostUpdate` before
Lightyear send. Field spawning can join `AbilitySet::Activation`; membership belongs to the existing
concealment source-resolution phase.

M03 must make defeat and field removal unambiguous by ordering source resolution after
`CombatSet::Lifecycle`, not merely after ability outcome observation. The fixed transaction becomes:

```text
FixedUpdate
  match/restart lifecycle
  targeted field activation -> ApplyDeferred
  pre-simulation owner/disconnect/expiry cleanup -> ApplyDeferred
  authoritative movement and attacks

FixedPostUpdate
  damage and ability outcome observation
  mode outcomes and fighter defeat/respawn lifecycle
  field cleanup -> ApplyDeferred
  terrain + field membership, source expiry, reveal deadlines -> ApplyDeferred
  observer x subject decision
  cue filtering and telemetry

PostUpdate
  queued gain/lose visibility -> ApplyDeferred -> Lightyear replication send
```

This guarantees that an owner defeated by this tick's damage cannot leave a field active for the
same tick's observer decision. Explicit `ApplyDeferred` boundaries make new fields visible to
membership and removed fields absent before visibility derivation. Bevy 0.19 documents both fixed-
post reaction semantics and ordered deferred-buffer application; the checked-in Lightyear 0.29
source confirms that ordinary `lose_visibility` despawns the remote hierarchy.

### Sentries need the shared observer rule; bots do not yet have a runtime

The current sentry acquisition path checks terrain membership and attack/damage locks directly. It
does not yet account for Self Cloak, Reveal Scan, or the new field, so V9 cannot close with that
parallel visibility rule.

M03 introduces a narrow permitted-target helper that builds the same `ObserverVisibilityInput` for
the sentry owner's team, position, and resolved reveal radius. Acquisition and target retention both
use it; losing permission clears the live target before firing. The sentry still applies its existing
line-of-sight and range rules after visibility permission. No autonomous player-bot runtime exists,
so M03 audits inert automation fixtures and records the future bot-adapter requirement without
inventing navigation or AI.

### Presentation should reconcile the public entity for its whole lifetime

The field uses code-native world geometry already proven by Hot Zone and Reveal Scan: a translucent
filled disc plus a crisp annular boundary placed above the floor, with no shadows. Concealment Field
uses a teal/green visual family distinct from Reveal Scan magenta and the mode objective. Friendly
versus hostile relation may adjust tint, but the persistent disc/ring shape communicates “active
area” without color alone.

The client reconciles one bounded visual root per replicated field ID. It creates visuals for
late-joined fields, updates relation styling after controlled-team changes, and removes them on
field despawn, match replacement, expiry, disconnect, or world teardown. Reduced effects removes
optional pulses only; it preserves exact center, radius, fill, boundary, and lifetime. Primitive
fallback uses the same meshes and materials, not a second renderer.

The targeting preview shows the same exact-radius ring/fill and origin-to-center line only while
locally armed. Accepted activation may add a short emphasis/audio cue, but the durable public entity
owns the full-duration visual. Occupant presentation continues to use the accepted 52% local/allied
fighter alpha through `ConcealmentPresentationState`; no overhead concealment or reveal text is
added.

### Product compatibility surfaces must advance together

Add `UltimateDefinitionId(5)` / `UltimateKind::ConcealmentField` and preset 7, **Veilkeeper**, to
the closed catalog inventory. Veilkeeper uses Arc Launcher, Concealment Field, Quick Cycle, and
Tenacity for a legal 12-point support/controller build. Saved profiles need no SQLite table
migration because they already store a bounded ultimate ID, but every legal-ID fixture,
creation/editing surface, preset rotation, operator count, help string, recovery snapshot, and
fail-closed catalog check must include it.

Because the new enum/component shapes change application compatibility, implementation advances the
global protocol version and affected build/profile routing snapshot schemas. The build catalog and
fingerprint format advance from 4 to 5. Existing saved brawlers remain valid because IDs 1–4 and
their meanings do not change; stale routed snapshots fail closed through the existing global
handshake/snapshot checks rather than compatibility decoders.

Balance Lab snapshot schema advances from 4 to 5 and gains a bounded ultimate tuning list for Self
Cloak, Reveal Scan, and Concealment Field. Apply/restore remains a validated, server-authoritative
restart transaction. Dash and Sentry remain code-owned entries in M03 because they do not yet have
authored parameter records to edit.

## Alternatives rejected

- **A transient field cue only:** cannot support authoritative membership, late join, lifecycle, or
  exact full-duration reconciliation.
- **Replicate owner and member identities publicly:** leaks hidden source/occupant facts not required
  to read the public area.
- **Avian sensor contacts:** add physics ordering and contact persistence where bounded pure circle
  geometry already supplies the accepted rule.
- **Caster always concealed while its remote field exists:** contradicts area membership and makes
  targeting away from oneself an unexplained second self-cloak.
- **One timer/deadline per overlapping field on the fighter:** area concealment ends immediately on
  exit and therefore has no subject-owned duration to stack.
- **Generic runtime area/effect framework:** there is still only one durable ability area with one
  owned behavior; a general language has no demonstrated second use.
- **Client-local opacity as field authority:** exposes hidden live state and violates the accepted
  M01 network boundary.

## Initial balance inputs

These are playtest defaults, not final balance claims:

| Input | Initial value | Rationale |
|---|---:|---|
| Ultimate ID | 5 | Next stable closed-catalog identity |
| Point cost | 4 | Same build commitment as Self Cloak and Reveal Scan |
| Maximum targeting range | 480 units | Shorter than Reveal Scan's 640-unit counter range |
| Field radius | 192 units | Same readable/counterable footprint as Reveal Scan |
| Active duration | 360 ticks / 6 seconds | Long enough for a team push without exceeding Self Cloak duration |
| Per-owner active fields | 1 | Ability phase owns one lifecycle |
| Global active fields | 8 | Equal to routed `MAX_PARTICIPANTS` |
| Membership IDs per fighter | 8 | One possible field per admitted participant |

Target center and fighter center use inclusive circle geometry. The existing observer reveal radii
(128/160/192 by resolved profile), attack lock (90 ticks), damage lock (120 ticks), Self Cloak
(360 ticks), and Reveal Scan (300 ticks) remain unchanged unless the M03 playtest exposes a direct
cross-source balance problem.

## ECS ownership and module composition

M03 uses the existing public crate API and these focused owners:

```text
src/abilities/concealment_field.rs
  stable request ordering, activation, field identity allocation, ability phase, cleanup

src/concealment/field.rs
  public/private field components, bounded memberships, pure targeting/membership geometry

src/concealment/authority.rs
  extracted server source resolution and observer derivation, including field memberships

src/concealment/network.rs
  unchanged per-connection visibility cache/application boundary

src/concealment/telemetry.rs
  bounded field/source/observer transitions and aggregates

src/client/presentation_3d/combat.rs
  targeted preview and durable public field visual reconciliation
```

`concealment/mod.rs` becomes composition, schedule sets, and narrow re-exports instead of absorbing
the field implementation. Extraction must preserve public paths and the accepted M01/M02 schedule
tests. Server-owned owner/membership state remains feature-gated; public immutable field state and
fighter presentation state remain shared protocol shapes.

Proposed core types:

```text
ConcealmentFieldId(u64)
ConcealmentFieldState { id, team, center, radius_milliunits, activated_at_tick, expires_at_tick }
ConcealmentFieldOwner { owner_network_id, owner_generation, match_id }
AlliedConcealmentMemberships(Vec<ConcealmentFieldId>)
AbilityPhase::FieldActive { field_id, expires_at_tick }
ObjectiveCarrier
```

No process-local `Entity` crosses the wire.

## Network and cue contract

- `ConcealmentFieldState` and `ObjectiveCarrier` are registered centrally in `protocol.rs`; the
  global protocol version/fingerprint advances once.
- Field entities replicate to `NetworkTarget::All` and are never passed through fighter
  concealment visibility. They carry no owner/member hierarchy.
- Fighters and their private descendants continue to use ordinary per-connection
  `gain_visibility`/`lose_visibility`; retained/always-present policies remain forbidden.
- `ConcealmentPresentationState` gains `inside_allied_concealment_field`. It is part of the cullable
  fighter and therefore cannot reveal a hidden occupant to an unauthorized client.
- The public activation cue contains event ID, tick, field ID, team, center, radius, and expiry—no
  owner or target list. Member entry/exit produces no player message or positional audio.
- Attack/damage/scan cue filtering continues after the completed observer decision. Field
  activation is independently public, while owner-derived private cues remain denied by default.
- Client cue deduplication, codec/evidence parsing, and bounded fan-out add the one new public cue.

## Lifecycle and recovery contract

Field cleanup reasons are `Expired`, `OwnerDefeated`, `OwnerDisconnected`, `BuildReplaced`,
`MatchCompleted`, `MatchRestarted`, and `Teardown`. At most one reason wins per field using that
stable priority order; cleanup is idempotent.

- expiry removes the field at `tick >= expires_at_tick` and settles the owner's accumulated charge;
- owner defeat/disconnect/build replacement removes the field even if its deadline remains;
- fighter exit, defeat, respawn, carrier acquisition, or team change removes only that fighter's
  membership on the completed source-resolution tick;
- restart/requeue/map or match replacement removes all old-generation field entities and visual
  roots before the next observer decision for the new generation;
- a late join or reconnect receives only still-current public fields. Because disconnect destroys
  the owner's field, reconnect never resurrects it from historical cues;
- field-ID exhaustion rejects safely without charge spend; worker shutdown/world teardown relies on
  ordinary ECS destruction and produces no persistence record;
- recovery snapshots remain bounded by eight immutable field states plus existing fighter state;
  stale client visuals are also removed by authoritative tick as a defensive bound.

## Telemetry and bounds

Ability telemetry adds accepted/rejected uses, target distance, active ticks, cleanup reason, and
active-field high-water. Concealment telemetry adds field membership enter/exit, effective source
transitions, overlap high-water, proximity reveal of field sources, and bounded drops. Process
summaries expose aggregates by stable owner/field identity but ordinary client diagnostics never
receive secret positions or member lists.

Hard ceilings:

- 8 active field entities;
- 8 field IDs per fighter membership;
- 8 fighters and 64 field-membership candidate pairs per tick;
- 64 observer–subject pairs per tick at the routed participant ceiling;
- existing 2,048 bounded visibility-transition records;
- one public activation cue per accepted field and no per-member network cue fan-out;
- one client visual root with a fixed child count per active field.

Any malformed or over-cap state fails closed: activation rejects, extra memberships are not added,
telemetry increments a bounded drop/rejection count, and concealment is never granted from data the
server could not validate.

## Implementation checklist

- [x] Rebaseline M01/M02 tests and add the source-after-lifecycle schedule assertion.
- [x] Add field kind/parameters/ID 5, preset 7, activation style, validation, resolution,
  fingerprints, routing/profile snapshots, product inventory surfaces, and fail-closed fixtures.
- [x] Add all V9 ultimate parameters to Balance Lab schema 5 with validation, transactional
  apply/restore, persistence, restart, and UI/API evidence.
- [x] Add field IDs, public/private field state, allocator, bounded memberships, objective-carrier
  marker, pure target/membership helpers, and protocol registration.
- [x] Implement stable targeted activation, charge/phase ownership, public field spawn,
  state-driven client activation audio, and every cleanup reason without primary-fire fallthrough.
- [x] Resolve field membership after movement/lifecycle and extend the one observer decision for
  all eight terrain/Self Cloak/field source combinations.
- [x] Route sentry acquisition and retention through the shared observer rule; audit bot fixtures.
- [x] Add durable field presentation, targeted preview, HUD/audio, normal/reduced/primitive styling,
  late-join reconciliation, and cleanup tests with no overhead text.
- [x] Add pure, ECS, protocol, profile, Balance Lab, separate-App, routed, impairment, recovery,
  security, capacity, performance, and native evidence below.
- [x] Triage every playtest item, rerun affected gates, complete the V9 learning review, and obtain
  user acceptance before marking M03 and V9 complete.

## Implementation and verification record — 2026-08-24

The implementation adds ultimate ID 5 and preset 7, durable public field entities, private owner
state, monotonic field IDs, bounded sorted memberships, the objective-carrier exclusion, shared
observer/sentry permission, field cleanup telemetry, targeted input/HUD behavior, blue/red
team-readable full-duration disc and boundary geometry, concealment alpha, and state-driven audio.
Protocol, build, routed profile, and Balance Lab schemas advanced together.

One implementation refinement replaces the proposed transient public activation cue with the
already-public replicated field state. `Added<ConcealmentFieldState>` drives activation audio and
the durable entity drives the visual for both current clients and late joiners. This avoids a
second delivery/deduplication path and exposes no owner or member identity.

Verification exposed two useful ordering/validation defects and both were corrected:

- damage facts are cleared during match lifecycle, so attack/damage reveal-lock observation now
  occurs before that clear while membership, field cleanup, and observer decisions remain after
  lifecycle; the M01 damage reveal regression and same-tick field teardown both pass;
- the build catalog originally locked the exact numeric ultimate defaults, which contradicted the
  new Balance Lab surface. It now locks stable ID/kind/cost and parameter topology while validating
  numeric values against the existing finite bounds.

Passing evidence:

- `just check` — routing, client-only, server-only, network-test, Balance Lab, and web build pass;
- `just lint` — formatting, web type/build, all Clippy feature graphs, server isolation, V3 renderer
  isolation, and V8 map cleanup pass;
- client-only 360 tests, server-only 276 tests, and Balance Lab 286 tests pass;
- all 81 separate-App network tests pass, including the new field cast/no-fire, public-state,
  distant concealment, proximity reveal, owner-defeat cleanup, and the retained M01/M02 scenarios;
- both 25-match restart soaks pass;
- all 11 performance gates pass; the combined 100-effect/32-scatter/16-lob/32-blade case measured
  p95 2.932 ms on the local aarch64 macOS host.

The user subsequently confirmed Concealment Field in gameplay and accepted the revised brawler
screens. No controller-specific observation was reported; the existing automated input coverage
and accepted native gameplay result are sufficient for this milestone, while controller and
reduced/primitive presentation remain part of the ordinary regression matrix rather than an open
V9 deliverable.

### Playtest feedback — 2026-08-24

- **Veilkeeper showed `ULTIMATE: Unknown` in the brawler creation/edit overlay — implemented now.**
  The overlay still used a four-entry hard-coded display-name table after ultimate ID 5 was added.
  It now resolves ultimate names from the authoritative build catalog, where ID 5 is
  `Concealment Field`. The selector was already catalog-driven and could reach the fifth ultimate.
  A focused regression test passes.
- **The Dashboard and blind `Select Next Brawler` menu made the active Concealment Field brawler
  impossible to identify confidently — implemented now.** The Dashboard card now presents the
  active brawler's named fighter profile, weapon, ultimate, and passives with a direct
  `VIEW BRAWLERS` action. Tapping it opens a full-screen, scrollable Brawlers List with
  human-readable loadouts and a selected marker; tapping a row opens a touch-oriented Brawler
  Details screen with fighter/weapon statistics, abilities, equipment state, Select, Customize,
  Delete, and Back actions. Creation, editing, equipment, confirmation, and empty-profile paths
  return to the owning brawler screen rather than the unrelated application menu. The application
  Menu now contains only application actions. Deleting the selected brawler chooses the next
  creation ordinal with wraparound in the same authoritative transaction; deleting the last leaves
  the empty list and Create action. Focused client-flow and server-storage regressions pass; native
  touch/layout feedback remains required.
- **The Brawlers List and Brawler Details still read as modal menu overlays rather than screens —
  implemented now.** Both destinations now own an opaque edge-to-edge viewport with screen-edge
  navigation and no centered outer dialog. The list uses a fixed top navigation band, full-height
  roster, and fixed Create action. Details uses a character-focused composition: fighter identity
  and base statistics, the existing live 3D fighter viewport, and the named loadout plus touch-size
  actions. Wide windows arrange these as three columns; compact windows stack them while preserving
  the large fighter stage. The live render target moves from Dashboard to Details and returns when
  Details closes, avoiding a duplicate preview renderer. The provided Brawl Stars image was used
  only as broad hierarchy/layout direction, not as an asset or literal screen specification.
- **Selecting Concealment Field appeared to save nothing, and creating a brawler with it produced
  no list entry — implemented now.** The saved-profile model still bounded ultimate IDs to the
  pre-M03 range `1..=4`, so authoritative storage rejected both mutations after the catalog and UI
  had already exposed ultimate ID 5. Profile validation now accepts the complete V9 ultimate
  inventory. Pure model and SQLite create/edit regressions cover Concealment Field explicitly.
- **A newly created brawler could remain visually stuck on `SELECTING...` — implemented now.**
  Brawler List and Details cached their rendered trees by profile revision but rendered controls
  from a separate pending-request flag. They now include that flag in their render identity, so an
  accepted or rejected server outcome immediately refreshes disabled controls. The client also
  tracks whether the outstanding mutation is specifically a selection, preventing unrelated
  create/edit/equipment requests from being mislabeled as selection. Membership replication no
  longer clears an outstanding request before its matching authoritative outcome arrives.
- **The selected brawler's `SELECTED FOR PLAY` action was disabled — implemented now.** The primary
  action remains enabled when inspecting the active brawler and returns directly to the Dashboard
  without sending a redundant selection mutation or advancing the authoritative profile revision.
- **Ability and weapon customization appeared as cramped modal dialogs, and Delete exposed the
  Dashboard behind its confirmation — implemented now.** Creation, ability customization, and
  weapon customization are opaque edge-to-edge screens with fixed screen-edge Back/Save actions,
  responsive wide/compact compositions, larger semantic sections, and touch-size controls. Weapon
  customization separates equipped slots from its scrollable owned inventory while retaining the
  live resolved preview. The small Delete confirmation remains contextual, but its originating
  Brawler screen stays mounted visibly underneath with background actions disabled. The canonical
  UX now bans substantial modal dialogs and permits only small, fully contextual overlays. Focused
  client-flow regressions verify the opaque creation screen, both customization destinations, and
  retained/disabled Brawler Details content beneath Delete confirmation; client test compilation,
  Clippy with warnings denied, formatting, and diff hygiene pass.
- **Native follow-up — accepted.** The user confirmed the revised full-screen brawler flow is
  better and that Concealment Field works in their gameplay test. This supplies the outstanding
  player-visible functional confirmation for field selection and activation. The remaining M03
  work is closeout reconciliation and the learning review; no new gameplay defect was reported.

### Feedback specification — server-authoritative brawler catalog

#### Decision and outcome

The selectable brawler inventory and its player-facing metadata become server authoritative and
connection scoped. The client must not decide legal fighter-profile, weapon-base, ultimate, or
mutable-passive IDs from numeric ranges, fixed arrays, or local display-name matches.

After lobby admission, the client owns one validated snapshot advertised by that server and uses it
for every Dashboard, Brawlers List, creation, editing, equipment, and accessibility choice. Profile
commands continue to carry stable IDs rather than authored values. The lobby validates those IDs
against the same active server catalog before persistence, and match admission resolves the saved
facts against the same catalog again.

This change centralizes product authority; it does not introduce downloadable gameplay code,
server-delivered models, hot content replacement, rolling protocol compatibility, or support for an
old client joining a server with unknown gameplay primitives. The existing exact application build,
registry, and gameplay-content fingerprint handshake remains mandatory.

#### Advertised catalog shape

Add one shared, bounded `AdvertisedBrawlerCatalog` wire/domain shape. Proposed contents:

```text
AdvertisedBrawlerCatalog
  revision/digest
  limits
    maximum saved brawlers
    weapon-part slot count
  fighter profiles [1..16]
    stable FighterProfileId
    bounded key and display name
    authoritative displayed fighter stats
  weapon bases [1..16]
    stable WeaponBaseId
    bounded key and display name
    presentation profile key/id
    authoritative base WeaponConfiguration needed for preview
  ultimates [1..32]
    stable UltimateDefinitionId
    bounded key and display name
    UltimateKind, activation style, and bounded parameters
  passives [2..32]
    stable PassiveDefinitionId
    bounded key and display name
    PassiveKind
    saved-brawler-selectable flag
```

All collections receive decode-time element limits; keys, names, and any later summaries receive
byte and grapheme limits. IDs must be nonzero and unique within their family. At least one fighter,
one weapon, one ultimate, and two distinct selectable passives are required. Every weapon
configuration and ability parameter is validated through its existing canonical rules before the
server may advertise the snapshot. The canonical digest covers every advertised field and ordering
is normalized by stable ID.

The server derives this snapshot from its active `BuildCatalog`, `WeaponCatalog`, fighter-profile
definitions, and saved-brawler policy. It is not a separately hand-maintained catalog. Frame-only
passives are excluded from saved-brawler selection by `PassiveKind`/explicit eligibility rather
than by assuming IDs `1..=2` and `3..=6`. Weapon display names come from `WeaponCatalog`; ultimate
and passive names come from `BuildCatalog`. Fighter profile metadata gains one server-owned
descriptor source so names, IDs, and displayed stats are no longer reconstructed in client code.

#### Connection and protocol lifecycle

`LobbyJoinOutcome::Accepted` carries the advertised brawler catalog beside the game-type catalog and
profile snapshot. Keeping one accepted envelope makes installation atomic: the client either
validates and installs membership, profile, game types, and brawler catalog together, or rejects the
welcome and tears down the session.

The implementation must prove the maximum profile snapshot (32 KiB), maximum advertised catalog,
game-type catalog, and envelope overhead remain below `MAX_LOBBY_WELCOME_BYTES` (64 KiB). Start with
a 16 KiB advertised-catalog bound. If measured worst-case encoding cannot satisfy the existing
welcome limit, use one additional reliable ordered lobby-catalog message and keep Dashboard entry
gated until both bounded pieces validate; do not silently raise transport/application bounds.

The catalog is immutable for the accepted lobby session. A server content restart requires a fresh
connection and compatibility handshake. Profile commands therefore do not need per-command catalog
versions: they are already tied to the authenticated connection, and the server validates against
that connection's active catalog. Dynamic in-session catalog replacement remains out of scope.

Changing `LobbyJoinOutcome`, membership state, and registered wire shapes advances the one global
`SUPPORTED_PROTOCOL_VERSION` and protocol-registry golden evidence. There is no old-message decoder
or optional fallback field. The gameplay-content fingerprint remains the proof that client runtime
code/assets understand the advertised kinds and configurations; the advertised digest proves the
installed selection snapshot itself is canonical.

#### Validation and storage ownership

Split current profile validation into two explicit responsibilities:

- structural validation owns names, stable-ID encoding, revisions, collection bounds, duplicate
  brawlers/parts, selected ownership, equipment uniqueness, and snapshot byte bounds;
- catalog validation owns whether fighter, weapon, ultimate, passive, and part-definition IDs exist,
  are eligible for saved brawlers, and resolve into a legal loadout.

`ProfileStorage` persists structurally valid facts and never defines content legality with ranges
such as `1..=5`. `ProfileAuthority` owns catalog validation before submitting a mutation and after
loading a stored snapshot. A stored profile that references content absent from the active server
catalog fails closed with an explicit profile-content rejection; it is never silently rewritten,
deleted, or replaced with defaults. The profile admin/restore path uses the same server catalog
validator before declaring restored data usable.

The client repeats structural and advertised-catalog consistency checks only to reject malformed or
contradictory server data. That defensive check is not gameplay authority. Server mutation outcomes
remain authoritative, and rejected create/edit operations stay on their owning screen with a visible
reason rather than navigating away and hiding the failure.

#### Client ECS and UI ownership

Install the accepted snapshot as connection-scoped client state, either inside
`ClientLobbyMembership` with a narrow accessor resource or as a resource bound atomically from that
component. Clear it on disconnect, server change, rejected welcome, or lobby-generation replacement.
No menu may fall back to embedded numeric inventories after connection.

Creation initializes from the first advertised fighter, weapon, ultimate, and first two distinct
selectable passives. Cycling walks the advertised stable-ID order with wraparound. Editing starts
from the selected brawler's advertised entries and cycles only eligible alternatives. List/detail
names, fighter stats, weapon-base preview, ultimate targeting labels, accessibility strings, and
equipment applicability all resolve through the installed snapshot.

Embedded client catalogs remain for compatible prediction/presentation code, asset lookup, and
local previews where required by the current exact-fingerprint architecture. They may be cross-
checked against the advertisement, but they no longer determine which saved-brawler options exist.
Presentation assets remain client owned and are selected through bounded presentation keys/IDs;
server metadata must never carry filesystem paths or Bevy handles.

#### Implementation sequence

1. Add bounded advertised catalog types, canonical validation/digest, server derivation, and focused
   pure tests without changing the connection flow.
2. Split structural profile validation from catalog validation. Inject the active server catalog
   into `ProfileAuthority`; remove numeric fighter/weapon/ultimate/passive legality from storage
   models while retaining fail-closed post-load validation.
3. Extend `LobbyJoinOutcome::Accepted` and `ClientLobbyMembership`, bump the global protocol, add
   decode bounds, and atomically validate catalog plus profile during admission.
4. Add the connection-scoped client catalog resource and lifecycle cleanup/rebind behavior.
5. Convert brawler creation, editing, list/detail presentation, accessible labels, and equipment
   previews to advertised lookups and collection traversal. Remove `fighter_profile_name`,
   `weapon_base_name`, numeric modulo/threshold cycling, and the temporary `1..=5` ultimate fix.
6. Convert Balance Lab combination validation and supported automation fixtures to enumerate
   authoritative catalogs. Handle or retire the legacy `--build-preset` mapping under the existing
   `MAINT-LEGACY-BUILD-SYSTEM` boundary rather than making it another product catalog.
7. Re-run protocol, role-isolation, profile persistence, routed lobby/match, UI/controller, size,
   and native visual checks; then update feedback and learning evidence.

#### Required verification

- every advertised field changes the catalog digest; duplicate, zero, unsorted, oversized,
  unsupported-kind, invalid-parameter, and insufficient-choice snapshots fail closed;
- every advertised fighter × weapon × ultimate × distinct selectable-passive pair passes profile
  catalog validation and saved-loadout resolution, while every non-advertised ID is rejected;
- a synthetic catalog with non-contiguous IDs proves creation/edit navigation contains no ordinal,
  modulo, fixed-count, or display-name assumptions;
- maximum catalog plus maximum profile plus maximum game types fits the welcome bound and oversized
  sequences fail during decode before allocation can exceed their cap;
- accepted welcome installs catalog and profile together; malformed catalog, profile/catalog
  disagreement, duplicate/conflicting welcome, reconnect, server change, and disconnect leave no
  stale selectable state;
- create and edit with Concealment Field pass through real client command, routed lobby authority,
  SQLite persistence, returned snapshot, list/detail refresh, and match admission;
- server rejects tampered/unadvertised IDs and queued mutations without storage changes; failed UI
  mutations remain visible on the owning screen with actionable copy;
- headless server builds retain no Bevy UI/assets, clients cannot author stats/configurations, and
  match workers still accept only immutable lobby-resolved snapshots;
- `just check`, `just lint`, full role-specific tests, separate-App network tests, routed E2E, and a
  native mouse/controller/touch-layout pass succeed.

#### Exit criteria

- no production saved-brawler path contains numeric catalog ranges or fixed display-name tables;
- the server-advertised snapshot is the sole source of selectable IDs and user-facing brawler
  metadata after connection;
- storage has no authored-content inventory and server authority validates every load/mutation;
- current and synthetic non-contiguous catalogs pass the contract matrix;
- protocol/bounds documentation and closeout learning explicitly record the authority correction;
- the user confirms creation, editing, selection, and Concealment Field behavior in the native UI.

## Verification plan

### Pure and catalog tests

- exact ultimate inventory, kind/parameter agreement, IDs/costs/bounds, preset legality, catalog
  fingerprint, routing/profile snapshot versions, stale/future rejection, and saved IDs 1–4 remain
  valid;
- target missing/zero/exact/max/over-max distance, arena-edge clamp, non-finite rejection, and
  preview/authority parity;
- inclusive field boundary, hostile rejection, dead/inactive/carrier rejection, caster-inside and
  caster-outside behavior, stable field ordering, deduplication, cap rejection, and ID exhaustion;
- observer truth tables for all source combinations, exact proximity boundary, forced reveal,
  attack/damage locks, dead observers, allies, and source resumption;
- Balance Lab rejects wrong schema/kind/range/radius/duration, preserves legal M02 values, applies
  field tuning only through restart, and restores the previous snapshot exactly.

### ECS and schedule tests

- ready targeted confirmation creates one field and spends once; held/stale/defeated/inactive/
  wrong-kind/uncharged/simultaneous-fire attempts create nothing and spend nothing;
- field spawn is applied before membership, while same-tick completed movement determines entry or
  exit; same-tick owner defeat removes the field before observer decisions and cue filtering;
- one active field per owner, eight global fields, stable simultaneous acceptance, exact expiry,
  accumulated-charge settlement, and every cleanup reason are idempotent;
- overlaps add/remove only their own stable IDs and never stack concealment or leave stale
  presentation state;
- attack/damage locks suppress a field member and expire exactly; Reveal Scan suppresses it only for
  the revealing team; Self Cloak plus field continues to ignore proximity until cloak end;
- acquiring/removing `ObjectiveCarrier`, defeat/respawn, team change, build replacement, restart,
  disconnect, requeue, and teardown update membership and field ownership exactly;
- sentries cannot acquire or retain a target hidden by terrain, Self Cloak, or field, but may do so
  through owner proximity, attack/damage reveal, or owner-team Reveal Scan.

### Separate-App and network security tests

- every client receives exact public field geometry while a distant enemy client lacks the hidden
  occupant entity and an ally retains it;
- two hostile observers at different distances receive different current fighter results without
  receiving different public field state;
- leaving, expiry, owner defeat/disconnect, and Reveal Scan expiry produce correct visible/hidden
  convergence without replaying concealed movement;
- late join sees a current field and only permitted occupants; reconnect after owner disconnect does
  not restore the removed field; restart/replacement removes old field entities and visuals;
- public field cues do not include owner/member identity, hidden source cues remain filtered, and
  client-World/packet evidence proves absent pose, hierarchy, HUD, projectile, audio, telemetry, and
  diagnostic leaks;
- jitter/loss/duplication and repeated visible-hidden-visible churn converge within existing bounds.

### Canonical, routed, capacity, and native gates

- run `just check`, `just lint`, `just test`, role feature isolation, V3 renderer isolation, profile
  backup/restore, Balance Lab persistence/restore, performance gates, and affected recovery checks;
- run routed 1v1, 2v2, and 3v3 mixed builds, including Hot Zone with tall grass, field overlap,
  Self Cloak, Reveal Scan, attack/damage reveal, disconnect, restart, and requeue;
- exercise impairment plus repeated eight-field creation/removal and assert entity, visual, message,
  cache, transition, recovery-byte, CPU, and memory high-water bounds;
- native playtest normal, reduced effects, and `BRAWLER_FORCE_PRIMITIVE_WORLD=1`, with keyboard/mouse
  and controller where available. Verify exact targeting, public lifetime, team readability,
  caster-inside rule, ally entry/exit, proximity, reveal suppression/resumption, objective visibility,
  and no stale ring/fill/model/nameplate/shadow/audio.

## Closeout learning review — 2026-08-24

### What went wrong and why

- The client exposed ultimate ID 5 while profile validation and display-name lookup still encoded
  the older four-ultimate inventory. The content catalog had become authoritative for gameplay but
  not for every selection and presentation surface, so duplicated numeric bounds drifted.
- Saved-brawler screen render identities omitted pending-request state even though button labels and
  enabled state depended on it. Profile membership could also arrive before the matching command
  outcome and clear the generic pending marker, leaving `SELECTING...` visually stale.
- Brawler editing reused the overlay shell intended for brief contextual questions. Opening Delete
  replaced the details tree, so the Dashboard—not the invoking Brawler screen—was exposed beneath
  the confirmation.
- Initial closeout wording treated a named legacy full-build preset as if it were part of the active
  saved-brawler product. That made the otherwise internal `Veilkeeper` fixture sound user-facing.
- Field scheduling initially observed damage after lifecycle cleanup and catalog validation locked
  tunable numeric defaults too tightly. Both issues came from tests covering each subsystem without
  first asserting the cross-system schedule and tuning ownership contracts.

### Corrections and prevention

- The accepted lobby snapshot now carries one bounded server-authoritative brawler catalog. Client
  choices, names, previews, and automation use stable advertised IDs; storage owns structure and
  lobby authority owns active-content legality. Non-contiguous-ID and Concealment Field persistence
  regressions prevent numeric-range assumptions from returning.
- Every retained UI tree includes all non-local facts that affect its rendering. Selection pending
  state is request-specific and clears only on its matching authoritative outcome; focused tests
  cover accepted and rejected refreshes.
- The canonical UX distinguishes opaque full-screen destinations from small contextual overlays.
  A confirmation must retain its invoking screen visibly underneath and disable background actions.
- Legacy full-build presets remain explicitly internal and are tracked for removal under
  `MAINT-LEGACY-BUILD-SYSTEM`; active product copy describes saved brawlers and their loadouts.
- Cross-source concealment schedule tests now assert damage observation, lifecycle cleanup, field
  membership, and observer decisions in their actual fixed-tick order. Catalog tests lock stable
  topology while Balance Lab-owned numeric values remain tunable within validated bounds.

### Reusable conclusion

Server authority includes discoverability metadata and legal selection inventory, not only runtime
combat mutation. Likewise, UI state ownership includes navigation context and asynchronous request
state, not only saved data. Future gameplay additions must therefore extend the authoritative
catalog, persistence validation, render identity, and originating-screen lifecycle as one vertical
slice before a player-facing selector is considered complete.

## User playtest record

The handoff used `just run 2` for targeting and counter interaction, with Hot Zone and tall grass
available for concealment checks. After the saved-brawler creation, selection, catalog, and screen
feedback was corrected, the user confirmed that Concealment Field works in gameplay and accepted
the revised brawler flow. No further gameplay or readability defect was reported.

## Exit criteria

- the user approves this specification before production implementation;
- Concealment Field is selectable, persisted, resolved, tuned, targeted, charged, activated,
  replicated, presented, recovered, reset, and cleaned through the routed product flow;
- only eligible living friendly centers inside receive the source, the caster has no outside-area
  exception, objective carriers remain visible, and exit/expiry/removal are exact;
- all source overlaps obey the one observer rule, priority, proximity, reveal, deadline, and bounded
  membership contracts;
- public area facts remain readable while unauthorized clients receive no hidden fighter state or
  source-derived leak; sentries consume permitted visibility;
- catalog/protocol/profile/Balance Lab compatibility advances fail closed while legal saved data is
  preserved;
- automated, routed, impairment, recovery, capacity, performance, native, feedback, and learning
  gates pass;
- every feedback item is implemented, deferred to the owning backlog, rejected with rationale, or
  marked as requiring evidence, and the user accepts M03 before M03 and V9 become Complete.

## Accepted specification decisions

M03 closed with these concrete decisions:

1. the display name is **Concealment Field**, ultimate ID 5, cost 4, with the **Veilkeeper** preset;
2. initial tuning is 480-unit range, 192-unit radius, and 360 ticks / 6 seconds;
3. target points clamp like Reveal Scan and circle edges may extend outside playable bounds;
4. the caster is concealed only while actually inside its field, exactly like another teammate;
5. fields are public by team and geometry, but owner/member identities are not public field data;
6. one field per owner and eight globally; overlaps are a sorted boolean source, not stacking;
7. owner defeat/disconnect/build replacement removes the field; ordinary damage only applies the
   existing temporary reveal lock unless it causes defeat;
8. M03 closes the missing V9 ultimate Balance Lab surface and routes sentries through the shared
   observer rule before V9 closeout;
9. objective carriers remain visible: Self Cloak rejects without charge spend, terrain is ignored,
   and a carrier's field may still conceal eligible allies.

## References

Local version-pinned sources inspected on 2026-08-24:

- `references/bevy/examples/README.md`, especially its warning to match examples to the Bevy
  release used by the application;
- `references/lightyear/examples/README.md` and
  `references/lightyear/examples/network_visibility/src/{server.rs,protocol.rs}`;
- `references/lightyear/book/src/SUMMARY.md` and
  `references/lightyear/book/src/concepts/advanced_replication/interest_management.md`;
- `references/lightyear/crates/replication/replication/src/visibility/immediate.rs`;
- `src/abilities/{mod.rs,charge.rs,reveal_scan.rs,self_cloak.rs,sentry.rs}`;
- `src/concealment/{mod.rs,model.rs,network.rs,telemetry.rs}`;
- `src/builds/{model.rs,definitions.rs}`, `content/catalogs/builds.ron`, `src/protocol.rs`,
  `src/matchplay/lifecycle.rs`, and `src/client/{input.rs,presentation_3d/combat.rs}`;
- `src/server/balance_lab/{mod.rs,http.rs,persistence.rs}` and
  `packages/brawler-routing/src/limits.rs`.

Primary released references checked after the local snapshot:

- [Bevy 0.19 `FixedPostUpdate`](https://docs.rs/bevy/0.19.0/bevy/app/struct.FixedPostUpdate.html),
  confirming it is the reaction schedule after fixed update logic;
- [Bevy 0.19 `ApplyDeferred`](https://docs.rs/bevy/0.19.0/bevy/ecs/schedule/struct.ApplyDeferred.html),
  confirming ordered deferred mutations become visible to dependent systems;
- [Lightyear replication 0.29.0](https://docs.rs/lightyear_replication/0.29.0/lightyear_replication/),
  for replication targets, hierarchy propagation, and network visibility;
- [Lightyear 0.29 visibility example](https://github.com/cBournhonesque/lightyear/tree/0.29.0/examples/network_visibility),
  matching the checked-in public/while-visible patterns.

The repository pins Bevy 0.19.1, Lightyear 0.29.0, and Avian 0.7.0. Exact APIs must follow those
locked sources rather than the checked-in Bevy 0.20-development examples.
