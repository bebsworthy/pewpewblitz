# V9 Milestone 01 — Observer-specific visibility and terrain concealment

## Status

**Complete.** Research and this specification were prepared on 2026-08-23. V8 M04/V8 completed,
the user approved production implementation, implementation plus automated verification completed,
and the user accepted both the core concealment behavior and the requested hidden-fighter alpha
signifier on 2026-08-23.

## Player-visible outcome

Tidal Garden's existing tall grass becomes real concealing terrain. A living fighter inside the
grass remains visible to itself and allies but disappears for enemy fighters outside their own
resolved reveal-proximity radius. Moving close reveals the hidden fighter only to that observer;
moving away conceals it again. Accepted attacks and positive applied damage reveal a grass occupant
to every enemy for bounded authored durations.

The terrain boundary remains public and readable in imported and primitive-fallback presentation.
An unauthorized client does not retain or receive the concealed fighter's live spatial state.

M01 does not add self cloak, reveal scan, the allied concealment field, autonomous practice bots,
line-of-sight vision, or final balance values.

## Research questions and findings

### Can the pinned network stack withhold one fighter from one observer?

Yes. Lightyear 0.29's `VisibilityExt::gain_visibility` and ordinary `lose_visibility` operate on one
authoritative entity and one link/sender entity. Ordinary loss despawns an already replicated
remote entity and prevents initial replication while hidden. Visibility propagates through the
replication hierarchy. Retained and always-present policies pause updates while preserving a
remote entity, so they are rejected for secret spatial state.

The checked-in `network_visibility` example demonstrates the exact pinned API. Lightyear's
`ReplicationSystems::Send` is in `PostUpdate`; M01 can derive completed fixed-tick decisions and
apply deferred visibility commands before that send boundary. `RoomPlugin` remains unnecessary for
the match-local pair rule and would be too coarse because two enemies in the same match need
different visibility outcomes.

### Is hiding the current fighter entity sufficient?

No. Today the fighter also carries roster identity/name/team/readiness used by the client scoreboard,
and combat cues are broadcast identically to every link. Projectiles and sentries also carry public
source/target facts. M01 must separate always-public participant facts from the cullable spatial
fighter and replace global cue broadcast with per-connection policy filtering. Merely toggling Bevy
render `Visibility` or the fighter's replication bit would either leak state or break the roster.

### Where does reveal proximity belong?

The current immutable `ResolvedFighterStats` owns maximum health and movement speed and is carried
through build/profile resolution into the match worker. Reveal proximity belongs there as a finite
positive world-unit attribute of the observing fighter. Fighter profiles author its base value.
The resolver must support bounded positive and negative modifier fixtures and produce one clamped,
rounded value; M01 does not add an unrelated player-owned item merely to demonstrate the seam.

The implementation audit must reconcile the current V7 production representation before editing:
the working tree is completing V8 and may already contain later profile/content changes not present
in historical snapshots. M01 preserves the one current immutable loadout path.

### Which map should prove the rule?

Tidal Garden already demonstrates vegetation, water, shaped boundaries, and 2v2 routing. Its
`TALL_GRASS` was accepted as non-concealing in V8 because complete concealment did not exist yet,
not because non-concealment was intended as the asset's permanent identity. M01 deliberately adds
the missing server-known capability to `TALL_GRASS`; it does not create a parallel dense-bush asset.
The layout can remain unchanged while schema/catalog/content identity and current gameplay
documentation change normally. Every map containing tall grass is revalidated, and Tidal Garden
receives a new native gameplay/readability playtest.

### When can attack and damage safely reveal?

Attack reveal begins only from the existing accepted-attack transaction in `GameplaySet::Fire`.
Damage reveal begins only after the combat payload transaction computes positive applied damage in
`CombatSet::Damage`. M01 records both as authoritative reveal facts, derives the final observer
decision after damage, filters cues, and applies Lightyear visibility before `PostUpdate` send.
This makes a legal accepted-attack or applied-damage cue coincide with visibility of its subject and
prevents rejected input or zero outcomes from becoming information signals.

## Research sources

Local, version-pinned sources:

- `references/lightyear/examples/network_visibility/src/server.rs` — per-link gain/loss and
  while-visible/retained/always-present behavior;
- `references/lightyear/crates/replication/replication/src/visibility/immediate.rs` — exact
  `VisibilityExt` semantics and hierarchy propagation;
- `references/lightyear/crates/replication/replication/src/send.rs` and
  `references/lightyear/crates/replication/replication/src/lib.rs` — `PostUpdate` send boundary;
- `references/lightyear/book/src/concepts/advanced_replication/interest_management.md` — interest
  management and room distinction;
- `src/gameplay.rs` and `src/combat/server.rs` — fixed gameplay/combat set chains;
- `src/combat/attack.rs`, `src/combat/effects/application.rs`, and `src/combat/authority.rs` —
  accepted attack, applied damage, cue outbox, and current global cue fan-out;
- `src/server/mod.rs`, `src/matchplay/model.rs`, and `src/client/hud.rs` — fighter replication,
  participant facts, control ownership, and roster dependence;
- `src/builds/model.rs`, `src/builds/definitions.rs`, `src/profiles/model.rs`, and
  `src/server/balance_lab/**` — resolved fighter attributes and tuning consumers;
- `content/catalogs/map_assets.ron`, `content/catalogs/map_gameplay_profiles.ron`,
  `content/maps/builtin/tidal-garden.ron`, and client visual/theme catalogs — V8 map seams;
- [Concealment specification](../../17-concealment.md),
  [network architecture](../../08-network-architecture.md#interest-management-and-concealment), and
  [environment gameplay](../../09-environment-gameplay.md#concealment-gameplay-model).

Primary released sources:

- [Lightyear 0.29 replication crate](https://docs.rs/crate/lightyear_replication/0.29.0), confirming
  that ordinary visibility loss despawns the remote entity and propagates to replicated hierarchy;
- [Lightyear 0.29 network-visibility example](https://github.com/cBournhonesque/lightyear/tree/0.29.0/examples/network_visibility),
  matching the checked-in source.

The local snapshot was sufficient for exact code planning; the released sources were checked to
confirm that the documented behavior belongs to the pinned release rather than an unpublished
local change.

## Alternatives rejected

- **Client opacity or Bevy render visibility:** leaks exact state and makes the client authoritative
  for a gameplay rule.
- **One replicated `Invisible` boolean:** cannot express different outcomes for two enemies and
  still delivers the hidden transform.
- **Lightyear rooms per bush:** too coarse and churn-heavy for observer-specific proximity; rooms
  remain an optional broad partition, not the gameplay decision.
- **Retained hidden entities:** preserve last-known remotely inspectable state and contradict the
  security outcome.
- **Add a parallel dense-bush asset:** duplicates the established vegetation identity, leaves
  `TALL_GRASS` misleadingly inert after the missing concealment system exists, and creates an
  unnecessary visual distinction for one intended terrain rule.
- **Add every concealment source in M01:** mixes the network/privacy proof with two new ultimate
  families and a runtime area before the foundation is accepted.
- **Line-of-sight proximity:** adds geometry vision and wall semantics before playtest demonstrates
  that planar distance is inadequate.
- **Generic perception, status, or region framework:** no second implemented owner demonstrates the
  abstraction.

## Technical specification

### Content and schema changes

M01 extends the current production schemas directly:

- `ResolvedFighterStats` gains `reveal_proximity_radius` in world units;
- each fighter profile provides a validated base radius;
- the fighter-stat resolver accepts a bounded canonical modifier with flat milliunits and percentage
  basis points, then clamps and rounds once;
- the build/profile/content fingerprint and immutable match snapshot versions increment;
- Balance Lab validation, HTTP/edit surface, snapshot persistence, preview, apply-to-fighter path,
  and tests include reveal proximity;
- `MapGameplayProfile` gains only the explicit concealment capability required by M01, preferably a
  bounded enum/option rather than an independent boolean;
- the existing `TALL_GRASS` map asset keeps its stable map-asset and visual identities but is
  rebound to a new explicit concealing gameplay profile; V8's shared inert pass/pass profile is not
  mutated because ground, rubble, and other non-concealing assets also reference it;
- Tidal Garden keeps its authored tall-grass placements unless implementation playtest identifies
  a concrete layout problem; every affected map is revalidated under the new rule;
- relevant schema, catalog, content, and protocol fingerprints/revisions change through the one
  global compatibility path; recipe or game-type revisions change only if their owned authored
  inputs change.

Code-owned validation rejects non-finite/non-positive base or resolved radii, out-of-range flat or
percentage modifiers, overflow, invalid concealment shapes, and aggregate concealment placements
above the selected ceiling. Exact initial radii and M/N durations are proposed through Balance Lab
evidence during implementation and remain subject to user playtest.

### Shared and server-owned model

The smallest expected shared facts are:

```text
RevealProximityRadius                 resolved immutable fighter attribute
ConcealmentCapabilityId               stable definition identity when more than one rule exists
PublicParticipantState                stable player/team/name/readiness/defeat facts
ConcealmentPresentationState          only facts a permitted client needs for its own/allied HUD
```

The smallest expected server-only facts are:

```text
TerrainConcealmentMembership          current placement/source generation for one fighter
AttackRevealDeadline                  latest accepted-attack reveal deadline
DamageRevealDeadline                  latest positive-damage reveal deadline
ObserverVisibilityCache               last applied outcome per stable observer/subject pair
VisibilityTransition                  bounded reason/tick fact for filtering and telemetry
```

Exact types may combine when they have one owner and lifecycle. Per-connection cache keys use stable
player/network identity plus the server's connection entity internally; process-local `Entity`
never enters shared serialization or wire messages.

### Public participant and spatial fighter split

M01 creates one always-replicated participant projection per admitted roster member, keyed by
stable `PlayerId` and the spatial fighter's `NetworkEntityId`. It owns only facts that remain public
while concealed: display name, team, connected/ready/restart-ready state, defeat/respawn category,
and any mode-approved score fact already public today.

The spatial fighter remains the authority entity for pose, facing, collision, health/combat runtime,
abilities, control, and gameplay. Its replication is cullable per observer. The Dashboard/in-match
roster and scoreboard migrate to the public projection instead of caching data by observing the
spatial fighter. Owner input/control continues to target the authority fighter and is not transferred
to the public projection.

Do not duplicate mutable gameplay authority onto the projection. Server systems may update its
public lifecycle view after authoritative transitions; clients cannot author it.

### Terrain membership

The V8 resolver derives a bounded concealment shape for every `TALL_GRASS` placement from its
validated footprint. A server system evaluates living fighter centers against current active grass
shapes after authoritative movement and map mutation are current. Equal-boundary behavior is
inclusive and covered by exact-distance tests.

Membership stores stable map instance/generation and placement identity. Placement removal or
replacement invalidates membership before visibility derivation. Respawn, restart, recovery, map
replacement, and teardown reconstruct or clear membership from current state; no historical enter/
exit stream is required for recovery.

### Observer decision

For M01, a subject is concealed from an enemy observer exactly when:

```text
observer is a living active combatant
and subject is alive and inside active TALL_GRASS
and no unexpired attack/damage reveal deadline exists
and planar_distance(observer, subject) > observer.reveal_proximity_radius
```

Self and allies are always permitted. Objective carriers are ineligible. Defeated observers receive
no special permission. A defeated subject has no live concealed spatial state and follows ordinary
defeat/respawn presentation policy.

Distance equality reveals: `distance <= radius` is visible. Computation uses finite authoritative
Avian `Position` values and squared distance where appropriate. Stable ordering is by observer
`PlayerId`, subject `NetworkEntityId`, then source placement ID so test traces and transition caps
are deterministic.

### Attack and damage reveal

The accepted attack path emits one internal source fact after it has committed ammo/cooldown and
created the accepted attack. The concealment owner extends `AttackRevealDeadline` to
`tick + attack_reveal_ticks`. Rejections do nothing.

The damage transaction emits one internal target fact only when applied damage is positive. The
concealment owner extends `DamageRevealDeadline` to `tick + damage_reveal_ticks`. Multiple facts keep
the latest deadline. Deadline semantics must state whether the end tick is inclusive and use the
same rule in HUD text, pure tests, and fixed-schedule tests.

Although M01 has no self cloak, both locks suppress the whole concealment decision so M02 can add
sources without changing their meaning.

### Fixed schedule and deferred boundaries

M01 introduces an explicit concealment set chain spanning the current fixed schedules:

```text
FixedUpdate
  GameplaySet::Lifecycle
  GameplaySet::Input
  GameplaySet::Simulation
  GameplaySet::Fire
    accepted attacks record reveal facts

FixedPostUpdate
  CombatSet::ProjectileSweep
  CombatSet::Damage
    positive damage records reveal facts
  ConcealmentSet::Membership
  ConcealmentSet::Decide
  ConcealmentSet::FilterAndApply
    per-link cues filtered
    per-link gain/lose visibility queued
  ApplyDeferred
  CombatSet::TelemetryAndCues / Finalize as reconciled by implementation

PostUpdate
  before lightyear::ReplicationSystems::Send
    assert/apply any connection changes that cannot safely complete in FixedPostUpdate
```

Research established semantic order, not permission to guess exact set labels. Implementation must
inspect Lightyear/Avian ordering after V8 closes, keep combat cue event order, and add a schedule
trace proving visibility and filtering become current before network send. Cue sending cannot remain
an unconditional loop over every link.

### Per-connection cue and target policy

M01 replaces `CombatOutbox(Vec<CombatCue>)` fan-out with one bounded policy step that classifies
each cue's subject-derived stable identities and decides per connection after final visibility.
Process evidence may retain the absolute authoritative event stream, but an ordinary client receives
only public cues and cues permitted for its observer/team.

At minimum:

- an accepted attack reveals the source before its attack/muzzle/projectile facts are sent;
- positive damage reveals the target before target-position/health/hit feedback is sent;
- a concealed subject does not produce remote overhead UI, status, aim, movement, or audio facts;
- sentry target selection excludes concealed hostiles for the sentry owner's team;
- a public projectile may be sent after its hidden source has legally revealed; no projectile path
  may disclose a still-hidden source;
- owner/allies retain their permitted fighter and feedback;
- process-only diagnostics remain server-side and do not become a client leak.

The milestone records a cue-type audit table during implementation. Unknown future cue variants
fail closed for hidden-subject delivery until classified.

### Lightyear visibility application

Fighters use `Replicate::to_clients(NetworkTarget::All)` as the broad target plus Lightyear's
per-link immediate visibility filter. For every changed observer/subject outcome:

- call ordinary `lose_visibility(subject, observer_link)` when concealment begins;
- call `gain_visibility(subject, observer_link)` when reveal begins;
- never call retained or always-present loss for the spatial fighter;
- preserve owner control and ally visibility through explicit decisions rather than assuming broad
  target configuration is enough;
- use one explicit deferred boundary before `ReplicationSystems::Send`;
- clear cached link decisions on disconnect and initialize every new/reconnected link from current
  authoritative state before first replication.

Hierarchy propagation is verified rather than assumed. Any child excluded from replicate hierarchy
receives an explicit policy or is rejected from the concealed fighter composition.

### Client presentation

The client removes/despawns an unauthorized remote fighter through replication. Presentation cleanup
must also remove interpolation remnants, imported/primitive model hierarchy, projected health/name
UI, aim indicators, shadows, trails, status visuals, selection/target markers, and spatial audio.

The local owner and allies see a readable concealment treatment and the grass boundary. An enemy
crossing into reveal distance sees the current fighter state reappear without replaying hidden
motion. Leaving reveal distance removes it without an exact last-position marker. Attack/damage
reveal receives a bounded public transition cue only after permitted state is current.

`TALL_GRASS` must communicate its newly implemented concealing role in imported and
`BRAWLER_FORCE_PRIMITIVE_WORLD=1` modes and under reduced effects. The public boundary and permitted
ally/owner state cannot depend on color alone.

### Recovery and lifecycle

- New and reconnected links calculate current visibility before first spatial replication.
- Terrain membership is reconstructed from current map generation/placement outcomes and current
  fighter positions.
- Match restart clears reveal deadlines/cache transitions and recomputes initial membership after
  spawn placement.
- Defeat removes active membership and public projection reports the mode-approved defeat/respawn
  category without retaining secret pose.
- Respawn begins from the new authoritative position and recalculates visibility; spawn protection
  does not imply concealment.
- Map replacement and teardown clear placement membership and pair caches owned by the old map.
- Disconnect removes link-owned cache entries without changing visibility for remaining observers.
- Worker shutdown has no asynchronous concealment owner outside the ECS lifecycle.

### Bounds and telemetry

M01 derives pair capacity from `ResolvedMatchCapacity.maximum_active_fighters`. At current 3v3 the
ordered pair count is small, but validation and memory ceilings use the resolved maximum rather than
literal six. The milestone pins ceilings for concealing-grass placements, transitions per tick,
queued per-link cues, cached pairs, and telemetry records before implementation.

Telemetry records stable subject/source identity, observing team or permitted aggregate, transition
reason, tick, duration, proximity band, attack/damage reveal, and bounded drops without emitting
secret positions to clients. Balance Lab/playtest summaries include time concealed, proximity
reveals, attacks from concealment, damage breaks, and re-conceal delay.

## Implementation checklist

- [x] Re-audit the post-V8 production map, fighter/loadout, cue, replication, roster, HUD, sentry,
  prediction/interpolation, audio, diagnostic, and Balance Lab consumers.
- [x] Add resolved reveal proximity, validation, canonical modifier math/fixtures, fingerprints,
  snapshots, preview, and Balance Lab support.
- [x] Rebind `TALL_GRASS` to a new explicit concealing gameplay profile, preserve the shared inert
  profile for ground/rubble/other assets, revise all required schema/catalog/content identities,
  and revalidate every affected map without adding a parallel bush asset.
- [x] Implement bounded terrain membership and exact planar proximity rules.
- [x] Add accepted-attack and positive-damage reveal facts/deadlines.
- [x] Add the focused server concealment composition, deterministic observer decision, transition
  cache, lifecycle cleanup, and telemetry.
- [x] Split public participant projection from cullable spatial fighter and migrate roster/HUD
  consumers.
- [x] Configure ordinary per-link Lightyear loss/gain with explicit ordering and deferred boundary.
- [x] Replace global cue fan-out with deny-by-default per-connection filtering and complete the cue,
  hierarchy, projectile, sentry, audio, UI, telemetry, and diagnostic audit.
- [x] Implement owner/ally/boundary/reveal presentation and stale-client cleanup in normal,
  reduced-effects, and primitive fallback modes.
- [x] Update protocol/content revisions, schemas, root/current specifications, commands, fixtures,
  evidence parsing, and developer orientation required by changed production behavior.
- [x] Run the verification plan, deliver the native/security playtest, triage feedback, rerun
  affected checks, and complete the learn-from-errors review.

### Implementation progress — 2026-08-23

- Reveal proximity is now immutable resolved fighter data. Initial Balance Lab/profile values are
  160 world units for default, 192 for lightweight, and 128 for reinforced; these are provisional
  playtest inputs, not final balance claims.
- Canonical modifier resolution uses bounded flat milliunits and percentage basis points, applies
  percentage then flat adjustment, clamps to 32..=1024 world units, and rounds once to thousandths.
- Build catalog/fingerprint, routed immutable match snapshot, profile match snapshot, and Balance
  Lab snapshot versions advanced through their existing fail-closed compatibility paths.
- `TALL_GRASS` retains `MapAssetId(8)` and visual profile 33 but now references dedicated gameplay
  profile 8 with `HideOccupants`. Shared inert profile 1 remains non-concealing.
- Map catalog/fingerprint schemas advanced; all embedded maps re-resolve under validation, including
  the 40 existing Tidal Garden grass placements and the concealment-placement ceiling.
- Focused evidence: `cargo check --all-targets`; `cargo test --lib builds::tests` (10 passed);
  `cargo test --lib map::catalog::tests` (13 passed); `cargo test --lib profiles::tests` (4
  passed); `cargo test --no-default-features --features balance-lab --lib
  server::balance_lab` (10 passed); Balance Lab web `npm run typecheck`.
- Authority/replication is implemented in `src/concealment.rs`: fixed-post membership and reveal
  facts precede observer decisions, and PostUpdate gain/loss plus its deferred boundary precedes
  `ReplicationSystems::Send`. Ordinary loss is used; retained/always-present modes are absent.
- Attack reveal is 90 ticks and positive-damage reveal is 120 ticks, both with exclusive end
  deadlines. Zero-damage outcomes and non-accepted attacks produce no lock.
- Spatial fighters are per-link cullable while `PublicParticipantState` remains independently
  replicated. The client roster reads only that public projection.
- Combat cues are classified and filtered per link from final fighter visibility. Sentry acquisition
  and continued fire reject a concealed target outside the owner's reveal radius.
- Permitted concealed fighters receive `ConcealmentPresentationState`. For the local fighter and
  allies, while concealment is active, imported fighter/equipment materials and the primitive
  fallback preserve their colors at 52% source alpha with `AlphaMode::Blend`, allowing the actual
  terrain to show through; ordinary team/local ground markers remain opaque. A proximity-revealed
  enemy remains opaque. Reveal locks and terrain exit restore the exact original material handles.
  Ordinary replicated despawn removes the complete fighter presentation hierarchy.
- Bounded server-only transition telemetry records stable subject identity, observer team, reason,
  tick, and visibility without secret positions.

### Cue/privacy audit

| Family | Subject policy |
|---|---|
| Attack, muzzle, delivery, lob, impact | Source must be visible; accepted attack establishes its reveal lock first |
| Melee, damage, effect, defeat | Every referenced fighter source/target must be visible; positive damage reveals target first |
| Sentry fire | Owner and target must be visible; authority also rejects concealed acquisition |
| Reset | Reset fighter must be visible under current lifecycle state |
| Deployable removal | Owner must be visible; no hidden owner-derived spatial cue is sent |
| Unknown future variant | Rust exhaustiveness requires an explicit policy before compilation succeeds |

### Verification evidence — 2026-08-23

- `just check` passed every client/server/network/Balance Lab feature graph and web build.
- `just lint` passed formatting, web build, all Clippy `-D warnings` roles, server feature isolation,
  V3 renderer isolation, and V8 legacy-map cleanup.
- `just test` passed routing, 347 client tests, 269 server tests, 279 Balance Lab tests, the full
  78-scenario network suite, both 25-match restart soaks, and 11 performance gates.
- Focused two-client Tidal Garden security test passed: distant grass occupant absent from the
  enemy client World, present for its owner, both public projections retained, exact-radius
  reappearance, retreat re-concealment, accepted-attack reveal, zero-damage non-reveal, and
  positive-damage reveal.
- Affected alpha-signifier verification passed: 12 focused combat-presentation tests, including
  exclusive reveal-deadline switching, cached alpha material construction, and exact original-handle
  restoration; canonical `just check` and `just lint` also passed after the feedback change.

## User playtest handoff

Build and start the routed product with `just run`. In the Dashboard choose **Tidal Garden 2v2**;
use additional `just client` terminals for human observers if desired. Compare Default (160),
Lightweight (192), and Reinforced (128) fighter profiles.

Please verify the seven observations in the native/security playtest section below, especially
that tall-grass boundaries read clearly, allies remain visible, distant enemies disappear without
stale nameplates/shadows/audio, proximity feels understandable, and shooting/damage reveal windows
feel readable. Repeat once with reduced effects and once with
`BRAWLER_FORCE_PRIMITIVE_WORLD=1 just client`.

## Verification plan

### Pure and catalog tests

- Fighter bases and positive/negative modifier fixtures resolve finite bounded proximity radii with
  deterministic flat/percentage/clamp/round order.
- Tall grass resolves concealment only from its explicit revised gameplay profile; appearance alone
  never grants the rule.
- Membership boundaries, exact `distance == radius`, stable ordering, latest-deadline refresh,
  ally/self permissions, objective-carrier exclusion, and pair truth tables pass.
- Invalid values, aggregate placements, modifier overflow, schema revisions, and fingerprints fail
  closed.

### ECS and schedule tests

- Movement/map mutation precede membership; attack and damage facts precede decision; decision,
  filtering, and applied deferred visibility precede replication send.
- Rejected attack, miss, zero damage, and duplicate facts do not reveal or extend deadlines.
- Accepted attack and positive damage reveal on the exact tick and concealment resumes only after
  the latest deadline expires while the fighter remains in grass.
- Defeat, respawn, restart, placement removal/replacement, map teardown, and disconnect clear only
  their owned state.
- Sentry targeting and any server observation adapter cannot acquire a concealed hostile.

### Separate-App and network security tests

- Hidden before relevance: unauthorized client World never contains the spatial fighter.
- Visible to hidden: remote fighter and private hierarchy despawn while public participant remains.
- Two observers: near enemy has current fighter; far enemy does not; allies and owner do.
- Reappearance delivers current authoritative state without hidden-path replay or stale interpolation.
- Attack/damage transition orders fighter visibility and allowed cues correctly under the same tick.
- Late join, reconnect, loss/duplication/jitter, defeat/respawn, restart, and map replacement converge
  without unauthorized components or messages.
- A cue/hierarchy/projectile/UI/audio/diagnostic audit asserts absence rather than relying on a
  screenshot.

### Canonical, performance, and routed gates

After implementation, use the canonical `justfile` commands identified by the post-V8 audit rather
than inventing substitutes. At minimum the evidence set must include:

- role-specific client/server checks, tests, formatting, and Clippy;
- focused network tests plus the canonical routed 1v1/2v2/3v3 product matrix;
- Tidal Garden routed 2v2 with near/far hostile observers and mixed reveal radii;
- delay/loss/duplication/jitter, late join, reconnect, restart, completion, and requeue;
- repeated conceal/reveal churn with bounded entities, pair cache, cues/messages, CPU, recovery
  bytes, and worker/client memory;
- server feature-graph proof that client rendering/assets did not enter headless authority.

### Native and security playtest

The handoff must provide one command path and ask the user to verify:

1. tall grass clearly communicates its newly implemented concealing role and public boundary;
2. allies remain visible while a distant enemy disappears;
3. approaching reveals only the approaching enemy and retreating conceals again;
4. the selected fighter's reveal-proximity value produces an understandable distance difference;
5. shooting and taking positive damage reveal for readable bounded windows;
6. normal, reduced-effects, and primitive-fallback boundaries communicate the same rule;
7. no stale model, nameplate, shadow, trail, sound, or marker remains after concealment.

The automated handoff separately reports client-World or decoded-packet evidence that hidden live
state was absent. The user is not asked to infer security from visuals.

## Exit criteria

- V8 is complete and the user approves this specification before implementation;
- the existing `TALL_GRASS` identity and accepted Tidal Garden layout exercise real server-owned
  concealment through an explicit revised gameplay profile and content identity;
- reveal proximity is a resolved observer fighter attribute with bounded positive/negative modifier
  support and maintained Balance Lab coverage;
- public participant and cullable spatial state are correctly separated;
- observer-specific proximity, accepted-attack reveal, and positive-damage reveal pass exact-tick
  behavior and security tests;
- every audited private component/message/hierarchy/cue is absent for unauthorized clients;
- recovery, lifecycle, impairment, routed, performance, role-isolation, and native checks pass;
- feedback is triaged, affected verification reruns, and the learn-from-errors review completes;
- the user accepts the terrain concealment slice before M02 is created.

## Feedback review

- Accepted on 2026-08-23: the core terrain-concealment behavior works in the native playtest.
- Implemented on 2026-08-23: while a fighter is actively concealed from distant enemies, keep its
  ordinary team/local ground marker and render its locally controlled/allied visible body with alpha
  blending so the actual terrain shows through. A proximity-revealed enemy stays opaque. Apply the
  treatment to imported character materials, attached equipment, and the primitive fallback without
  changing replication visibility or exposing a fighter to an unauthorized client. The
  implementation uses 52% source alpha and restores original handles exactly on reveal or terrain
  exit.
- Accepted on 2026-08-23: the 52% alpha treatment provides the requested readable hidden-fighter
  signifier. M01 needs no further presentation adjustment.

## Learn-from-errors review

Completed on 2026-08-23.

- Mistake: the initial specification treated V8 acceptance of non-concealing tall grass as a durable
  design choice and proposed a parallel dense-bush asset. Cause: it inferred intent from behavior
  that existed only because concealment had not been implemented. Prevention: when promoting a
  backlog capability, distinguish an explicit negative product decision from a placeholder behavior
  caused by the capability's absence; preserve stable content identity when the user confirms the
  intended semantic evolution.
- Mistake: the first local concealment cue reused the slow-status ground-marker material. Cause: it
  optimized for an already available visual handle instead of preserving one meaning per visual
  language. Prevention: do not overload status colors for unrelated gameplay state; use a dedicated
  material transition and retain the team/orientation marker's established meaning.
- Correction made before handoff: alpha treatment must be relative to the controlled fighter's team.
  A hostile fighter visible through proximity is revealed to that client and therefore remains
  opaque, while the local fighter and allies receive the concealment signifier. Prevention: include
  observer relation in tests for every observer-specific presentation state, not only in authority
  tests.
- Reusable success: separating always-public participant projection from cullable spatial fighter
  state made ordinary Lightyear visibility loss usable without breaking the roster. Per-link cue
  filtering and sentry targeting then consumed the same authoritative observer decision instead of
  creating presentation-only secrecy paths.
- Reusable success: cached cloned `StandardMaterial` variants cover imported descendants, attached
  equipment, and primitive fallbacks without mutating shared assets, while retaining exact original
  handles makes reveal and terrain-exit restoration deterministic.
- No new Codex skill was created: the lessons are currently specific to Brawler's accepted content
  semantics and concealment presentation. They are recorded here and in the durable concealment
  specification; recurrence across milestones would justify extracting a reusable project skill.
