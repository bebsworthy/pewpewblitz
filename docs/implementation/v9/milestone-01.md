# V9 Milestone 01 — Observer-specific visibility and terrain concealment

## Status

**Specification review.** Research and this specification were prepared on 2026-08-23 at the
user's request. Production implementation is gated on V8 M04 reaching `Complete` and explicit user
approval of this milestone.

## Player-visible outcome

Tidal Garden receives a deliberately distinct dense-bush terrain identity. A living fighter inside
the bush remains visible to itself and allies but disappears for enemy fighters outside their own
resolved reveal-proximity radius. Moving close reveals the hidden fighter only to that observer;
moving away conceals it again. Accepted attacks and positive applied damage reveal a bush occupant
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

Tidal Garden already demonstrates vegetation, water, shaped boundaries, and 2v2 routing, but its
`TALL_GRASS` was deliberately accepted as non-concealing in V8. M01 does not silently change that
identity. It adds a visually denser `DENSE_BUSH` feature with a distinct map-asset/gameplay/visual
identity and replaces only a reviewed subset of Tidal Garden vegetation cells. The remaining tall
grass stays honestly non-concealing. The map recipe and game-type/content revisions change
normally and the altered routes receive a new native playtest.

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
- **Change current tall grass in place:** rewrites an accepted V8 gameplay identity and makes the
  same asset silently mean different things across content revisions.
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
- `DENSE_BUSH` gains stable map-asset, gameplay-profile, visual-profile, and display identities;
- Tidal Garden replaces a reviewed subset of `TALL_GRASS` placements with `DENSE_BUSH`; existing
  `TALL_GRASS` remains non-concealing;
- relevant schema, recipe, catalog, game-type, content, and protocol fingerprints/revisions change
  through the one global compatibility path.

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

The V8 resolver derives a bounded concealment shape for every `DENSE_BUSH` placement from its
validated footprint. A server system evaluates living fighter centers against current active bush
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
and subject is alive and inside active DENSE_BUSH
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

The local owner and allies see a readable concealment treatment and the bush boundary. An enemy
crossing into reveal distance sees the current fighter state reappear without replaying hidden
motion. Leaving reveal distance removes it without an exact last-position marker. Attack/damage
reveal receives a bounded public transition cue only after permitted state is current.

`DENSE_BUSH` must remain distinct from non-concealing tall grass in imported and
`BRAWLER_FORCE_PRIMITIVE_WORLD=1` modes and under reduced effects. Color alone is insufficient.

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
literal six. The milestone pins ceilings for bush placements, transitions per tick, queued per-link
cues, cached pairs, and telemetry records before implementation.

Telemetry records stable subject/source identity, observing team or permitted aggregate, transition
reason, tick, duration, proximity band, attack/damage reveal, and bounded drops without emitting
secret positions to clients. Balance Lab/playtest summaries include time concealed, proximity
reveals, attacks from concealment, damage breaks, and re-conceal delay.

## Implementation checklist

- [ ] Re-audit the post-V8 production map, fighter/loadout, cue, replication, roster, HUD, sentry,
  prediction/interpolation, audio, diagnostic, and Balance Lab consumers.
- [ ] Add resolved reveal proximity, validation, canonical modifier math/fixtures, fingerprints,
  snapshots, preview, and Balance Lab support.
- [ ] Add `DENSE_BUSH` gameplay/map/visual identities and a reviewed Tidal Garden revision while
  preserving non-concealing `TALL_GRASS`.
- [ ] Implement bounded terrain membership and exact planar proximity rules.
- [ ] Add accepted-attack and positive-damage reveal facts/deadlines.
- [ ] Add the focused server concealment composition, deterministic observer decision, transition
  cache, lifecycle cleanup, and telemetry.
- [ ] Split public participant projection from cullable spatial fighter and migrate roster/HUD
  consumers.
- [ ] Configure ordinary per-link Lightyear loss/gain with explicit ordering and deferred boundary.
- [ ] Replace global cue fan-out with deny-by-default per-connection filtering and complete the cue,
  hierarchy, projectile, sentry, audio, UI, telemetry, and diagnostic audit.
- [ ] Implement owner/ally/boundary/reveal presentation and stale-client cleanup in normal,
  reduced-effects, and primitive fallback modes.
- [ ] Update protocol/content revisions, schemas, root/current specifications, commands, fixtures,
  evidence parsing, and developer orientation required by changed production behavior.
- [ ] Run the verification plan, deliver the native/security playtest, triage feedback, rerun
  affected checks, and complete the learn-from-errors review.

## Verification plan

### Pure and catalog tests

- Fighter bases and positive/negative modifier fixtures resolve finite bounded proximity radii with
  deterministic flat/percentage/clamp/round order.
- Dense bush resolves only from its explicit profile; tall grass remains non-concealing.
- Membership boundaries, exact `distance == radius`, stable ordering, latest-deadline refresh,
  ally/self permissions, objective-carrier exclusion, and pair truth tables pass.
- Invalid values, aggregate placements, modifier overflow, schema revisions, and fingerprints fail
  closed.

### ECS and schedule tests

- Movement/map mutation precede membership; attack and damage facts precede decision; decision,
  filtering, and applied deferred visibility precede replication send.
- Rejected attack, miss, zero damage, and duplicate facts do not reveal or extend deadlines.
- Accepted attack and positive damage reveal on the exact tick and concealment resumes only after
  the latest deadline expires while the fighter remains in bush.
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

1. dense bush and ordinary tall grass are visually distinct;
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
- one dedicated terrain identity and one accepted Tidal Garden revision exercise real server-owned
  concealment without changing non-concealing tall grass in place;
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

Pending specification approval and implementation playtest.

## Learn-from-errors review

Pending implementation, verification, and feedback review.
