# Milestone 03 — Complete 3D combat, fighter, cue, preview, and world-HUD replacement

## Tracking

| Field | Value |
|---|---|
| Status | Complete |
| Prepared | 2026-08-20 after the user accepted and closed M02 |
| Objective | Replace every remaining gameplay-world sprite/XY presentation path with dedicated 3D visual entities while preserving the planar authoritative simulation and interpolated wire pose |
| Entry dependency | Satisfied — M02 is complete at commit `d084938` with the default 3D map/terrain/camera, corrected projectile placement, and obsolete projectile sprite writer removed |
| Scope authority | Production implementation authorized by the user's explicit direction on 2026-08-20 |

## Player-visible outcome

The normal client presents the complete playable combat scene in 3D. Fighters use the selected
animated Mini Characters model with an attached Blaster Kit weapon; sentries, straight and lobbed
projectiles, weapon previews, dash trails, impacts, status feedback, team identity, and health are
depth-aware 3D presentation. Every dynamic family retains a readable primitive fallback where an
imported dependency can fail.

Movement, aiming, collisions, attacks, effects, scoring, and terrain remain authoritative on the
existing 2D plane. Screen-space menus and product HUD remain Bevy UI. M03 changes what players see,
not the rules the server evaluates or the protocol it replicates.

## Scope boundary

### In scope

- independent render-only visual roots for fighters, sentries, straight projectiles, and lobbed
  projectiles, linked to their gameplay owner by a client-local `Entity` reference;
- final pose conversion after interpolation/writeback, with no XY gameplay-world `Transform`
  writer and no 3D repair pass;
- one selected Mini Characters model, one compatible Blaster Kit attachment profile, and primitive
  character/weapon fallbacks;
- idle, walk, holding, shoot, defeated, and reset animation transitions driven only by replicated
  state, interpolated motion, and deduplicated combat cues;
- 3D attack previews for straight/spread, lobbed path/landing/payload, and melee arc delivery;
- 3D dash trails, sentry presentation, combat impacts, damage/defeat/reset feedback, slow and
  knockback status feedback, and existing bounded transient effects;
- world-space fighter health and team/facing identity that remain readable from the fixed camera;
- separation of renderer-neutral cue ingestion/evidence from the windowed 3D consumer;
- deletion of the replaced 2D combat/fighter presentation systems and dormant old movement
  presentation source once their retained screen-UI helpers are moved to the proper UI owner;
- unchanged server authority, Avian 2D physics, gameplay definitions, content fingerprints, and
  wire schema.

### Outside M03

- new skins, skin selection, weapon cosmetics, entitlement, or replication schema;
- importing all Mini Characters or Blaster Kit variants without an authored selection use;
- changing weapon geometry, ranges, damage, timing, collisions, status rules, or cue facts;
- animation-driven movement, root motion, inverse kinematics, upper-body masking, or a general
  animation state-machine framework;
- custom shaders, GPU particles, a general decal system, billboarding infrastructure, outlines,
  post-processing, LOD, or instancing without measured need;
- final visual tuning, wall-occlusion policy, release-profile rendering measurements, and removal
  of residual dependencies used only by UI (M04);
- Avian 3D, replicated height, jumping, vertical collision, or a perspective/orbit camera.

## Research conclusion

M03 is feasible as a client presentation replacement. The protocol already exposes every durable
fact required by the current visuals: interpolated `Position`/`Rotation`, velocity, health,
defeated state, active effects, knockback feedback, ability state, projectile flight state, team
and loadout identity, plus bounded combat cues. No replicated `Transform`, `Vec3`, animation state,
or new gameplay message is required.

The current source has two conflicting presentation models. The remaining sprite systems create
and update their own XY visual entities, while the M01/M02 3D proof still puts fighter/projectile
presentation components and `Transform` directly on replicated gameplay roots. M03 replaces both
with one rule: gameplay roots own gameplay state; independent client-only visual roots own all
meshes, scenes, materials, animation players, transient catch-up state, and render transforms.

This also fixes the class of projectile bug found in M01/M02. A visual root reads the owner's final
interpolated planar pose and converts it once to X/Z. It is neither parented beneath the gameplay
root nor used to overwrite that root, so an incompatible structural or legacy transform cannot
offset it.

No research finding justifies a new crate, protocol change, render framework, replicated animation
state, particle dependency, or content-definition expansion.

## Current presentation inventory

| Family | Current source and behavior | M03 disposition |
|---|---|---|
| fighters and weapon | broad `src/client/presentation_3d/mod.rs` puts 3D scene/fallback and transform on gameplay root; dormant `src/client/presentation.rs` still contains old fighter sprites/Text2d | move to independent actor visual roots; remove dormant 2D actor path after extracting screen UI helpers |
| straight/lobbed projectile | M01 3D mesh and launch catch-up live on projectile gameplay root | retain geometry and bounded catch-up behavior on a dedicated projectile visual root |
| sentry | `src/combat/client/world.rs` inserts a `Sprite` and XY `Transform` on the sentry gameplay entity | primitive base/body/barrel visual root linked to sentry owner |
| dash trail | bounded sprites follow gameplay target and fade over time | bounded translucent ground-aligned 3D segments; no gameplay child relationship |
| health | two sprite entities per fighter, linked by process-local `Entity` | two shared-material 3D bars above the actor; fill changes scale/offset, not mesh allocation |
| slow/knockback | durable sprite markers keyed by target and status kind | shared ring/symbol meshes close to the ground, derived from durable replicated state |
| attack previews | at most 24 sprite slots generated from pure `preview_segments` geometry | preserve pure geometry; materialize cached thin cuboids/discs/arcs on X/Z |
| cue effects | deduplicated cues spawn one generic bounded sprite effect; reduced effects shorten lifetime | semantic ingestion remains headless-safe; 3D consumer maps cue profiles to a small bounded primitive palette |
| product HUD/menus | Bevy UI through the screen-space UI camera | retain; these are not gameplay-world rendering |

The old cue facts and evidence paths remain useful. `DeduplicatedCombatCue` is already the client
boundary for accepted new cues; legacy Muzzle/Impact/Damage/Defeat/Reset variants continue to be
ignored for duplicate presentation after evidence recording.

## Asset and animation decision

### Selected content

Retain the curated runtime assets already introduced by M01:

- `character-male-a.glb` from Kenney Mini Characters;
- `blaster-a.glb` from Kenney Blaster Kit;
- each pack's relative colormap texture and CC0 license/provenance entry.

Direct inspection of the staged Mini Characters pack found twelve animated character variants,
seven consistently named body/bone nodes, and 32 named clips. The selected model supplies `idle`,
`walk`, `holding-right`, `holding-right-shoot`, and `die`. The selected Blaster Kit model fits the
existing `arm-right` attachment after a reviewed local translation/rotation/scale profile.

The Kenney catalog identifies Mini Characters as an animated 3D CC0 pack and Blaster Kit as a CC0
3D pack with animation and variations. These packs can be combined technically because the weapon
is an independently spawned scene attached beneath the character's named arm node; the two packs
do not provide a shared socket contract. Brawler therefore owns the small attachment profile.

M03 does not promote arbitrary character or blaster selection into gameplay content. One real
profile plus deterministic primitive fallbacks proves the actual use without inventing a cosmetic
schema.

### Scene and animation lifecycle

Bevy 0.19 loads the character as `WorldAssetRoot`. `WorldInstanceReady` scopes descendant lookup to
that visual root, finds its `AnimationPlayer` and `arm-right` node, attaches the weapon scene, and
installs one retained `AnimationGraphHandle` plus `AnimationTransitions`. Every clip is started via
`AnimationTransitions::play`; directly mixing `AnimationPlayer::play` with transitions is forbidden
because the transition component owns main-animation weights.

Failure is local and deterministic:

1. The fighter visual root begins with a team-colored sphere, ground ring, and facing marker.
2. When the character scene and recursive dependencies are ready, it replaces or hides the sphere.
3. Missing required clips, `AnimationPlayer`, or `arm-right` keeps the fallback and logs one bounded
   diagnostic rather than leaving a partial invisible actor.
4. Weapon load or attachment failure keeps the primitive cuboid weapon without affecting fighter,
   map, or session readiness.
5. Despawning the visual root cleans up its imported character and weapon descendants.

## Technical specification

### Ownership and module composition

Keep gameplay/combat presentation with the combat client owner while shrinking the broad M01
foundation:

```text
src/client/
  assets.rs                       retained optional GLB handles/readiness
  hud.rs                          screen-space product HUD and pause/session UI
  presentation_3d/
    mod.rs                        world plugin composition and shared 3D resources
    coordinates.rs               sole planar-to-X/Z conversion API
    camera.rs                     accepted camera, light, and ground ray

src/combat/client/
  mod.rs                          renderer-neutral cue/evidence and client combat composition
  presentation_3d/
    mod.rs                        windowed combat 3D plugin, sets, shared components
    actors.rs                     fighter/sentry roots, scene setup, weapon, animation
    projectiles.rs                straight/lobbed roots and visual launch catch-up
    preview.rs                    pure preview geometry materialization
    effects.rs                    cue effects, dash trails, durable status markers
    world_hud.rs                  health, team, facing, defeated visibility
```

Exact filenames may combine when ownership remains cohesive, but `mod.rs` stays a composition/API
surface and the existing 1,200-line M01 module is not allowed to absorb the whole milestone.

`ClientCombatPlugin` retains renderer-neutral cue ingestion and evidence in headless clients.
`CombatPresentation3dPlugin` is installed only by the windowed client composition and is the only
owner allowed to allocate meshes, materials, scenes, or world-space presentation entities.

### Components and resources

| Item | Ownership and purpose |
|---|---|
| `CombatVisualOwner(Entity)` | process-local link from one independent visual root to its gameplay owner; never registered or replicated |
| `FighterVisual3d` | actor root state, fallback/imported readiness, last rendered ground position, and animation mode |
| `SentryVisual3d` | sentry root and presentation-only barrel/facing state |
| `ProjectileVisual3d` | projectile root, delivery family, and optional straight-launch catch-up position |
| `CombatEffect3d` | bounded cue/status/trail family, target or fixed world point, and client-time expiry |
| `WeaponPreviewVisual3d` | one of the fixed preview slots and current primitive kind |
| `WorldHealthVisual3d` | target link and background/fill child identities |
| `CombatPrimitiveAssets` | cached unit meshes for sphere, cuboid/line, cylinder, disc/ring, arc, and cone/marker |
| `CombatMaterialAssets` | bounded shared palette for teams, previews, health, statuses, projectiles, and cue profiles |
| `ImportedFighterAssets` | retained selected GLB, named clips, animation graph, attachment profile, and readiness |
| `CombatEffectBudget` | explicit maximum concurrent transient roots and deterministic oldest-first eviction |

Visual roots are not children of gameplay entities. Imported scenes and primitive parts may be
children of their visual root because those transforms are all render-space transforms under the
same presentation owner.

Repeated geometry uses unit meshes and per-entity `Transform::scale`; health fill and preview line
length therefore do not allocate meshes each frame. Materials are swapped from a bounded palette
rather than cloned or mutated per target.

### Schedule and deferred-command boundaries

```text
Update
  CombatClientSet::Receive
      receive cues; record evidence/dedup only
  CombatPresentation3dSet::Reconcile (after Receive)
      create/remove independent roots for current fighters, sentries, projectiles, bars
  CombatPresentation3dSet::ConsumeCues (after Receive)
      map deduplicated cues to bounded effects and actor animation intents
  CombatPresentation3dSet::State (after Reconcile, after ConsumeCues)
      update health, status, preview, visibility, timers, and animation selection
  CombatPresentation3dSet::Cleanup (after State)
      expire transients and remove orphaned roots

PostUpdate
  Lightyear/Avian interpolation and writeback
  CombatPresentation3dSet::Pose
      read owner Position/Rotation; write visual-root X/Z Transform once
      after InterpolationSystems::Interpolate
      after PhysicsSystems::Writeback
      before TransformSystems::Propagate
  camera follow
  TransformSystems::Propagate
```

Reconciliation commands may make a new visual appear on the following frame. Do not add an
`ApplyDeferred` solely to save one presentation frame. A newly spawned visual root receives its
initial converted pose at spawn so it cannot flash at the origin.

The final pose system is the only system that follows a gameplay owner's planar pose. Child-local
animation and shape transforms never duplicate coordinate conversion. A targeted schedule test
must prove the three ordering edges above.

### Actor lifecycle and animation policy

1. Reconcile one fighter visual root per replicated fighter that has the durable render inputs.
2. Derive team/loadout colors from stable replicated identity exactly as the current fallback does.
3. Derive ground translation and facing from interpolated `Position`/`Rotation` only in the pose
   phase. Derive motion from interpolated ground displacement with a small dead zone; do not use
   animation or render position as gameplay input.
4. Select a small presentation state: `Defeated`, `Shoot`, `Holding`, `Walk`, or `Idle`, in that
   priority order. A deduplicated accepted-attack cue starts the bounded shoot one-shot for local
   and remote fighters. Durable defeated state selects `die` once; reset returns to locomotion.
5. Transition loops through `AnimationTransitions`; replay one-shots only on a new cue/state edge.
6. If the selected rig cannot blend walk and holding without a masked graph, prefer readable
   facing/weapon pose and record the locomotion limitation for M04 rather than introducing a
   general animation framework.
7. A sentry uses cached primitives for base/body/barrel. Its root follows durable sentry pose and
   its barrel faces the replicated rotation. `SentryFired` produces muzzle feedback but never
   mutates targeting or firing state.
8. Missing owner state, owner despawn, disconnect, match reset, or map replacement removes the
   visual root and descendants without touching gameplay entities.

### Projectile policy

- Straight projectiles use a short horizontal cylinder/capsule aligned with travel. Diameter
  follows authoritative radius; presentation length is clamped for readability.
- Lobbed projectiles use a sphere. Ground X/Z follows the interpolated planar owner pose; Y follows
  the existing client-only parabolic arc from launch tick and duration.
- The accepted launch catch-up remains presentation-only: a straight visual starts at the source
  muzzle/origin and approaches the replicated current pose at bounded 3× projectile speed without
  overshoot. Catch-up state lives on the visual root, not the projectile gameplay entity.
- Source resolution failure skips catch-up and uses current replicated pose. Malformed or stale
  data never produces NaN transforms.
- Projectile gameplay roots receive no mesh, material, scene, visual marker, or presentation
  transform component.

### Preview geometry

Retain `preview_segments` as a pure renderer-neutral geometry function because its existing tests
cover all delivery families and terrain-repaired lob landings. Replace only its materializer:

- straight/spread segments become thin ground cuboids with cached unit geometry;
- lob travel becomes a chain of bounded ground segments, the landing footprint a translucent disc
  plus ring, and payload radius a second ring when different;
- melee arcs use at most the existing 24 slots, approximated by short tangent cuboids and boundary
  markers;
- accepted/rejected/cooldown/terrain-blocked states select shared material handles;
- hidden slots use `Visibility`, not transparent active meshes or per-frame despawn/recreate;
- preview transforms use the coordinate API and add only a small render height to avoid z-fighting.

Preview geometry remains advisory and may not become a collision, hit, or attack-acceptance input.

### Cues, trails, statuses, and world HUD

Split cue processing into semantic ingestion and visual consumption. Evidence, deduplication, and
bounded seen-cue history run without rendering. The windowed consumer maps only
`DeduplicatedCombatCue` into a deliberately small effect palette:

| Fact | 3D presentation |
|---|---|
| accepted attack / sentry fired | muzzle flash cone/sphere and short emissive pulse at resolved source |
| delivery impact / lob landed / melee contact | ground disc/ring plus short vertical burst at event point |
| damage/effect applied | small target-local burst; durable slow/knockback markers still come from state |
| defeated | bounded burst and actor `die`; actor visibility follows durable defeated state |
| reset | short team-colored ring and transition back to locomotion |
| deployable removed | brief collapse/fade at last known visual position |

Transient count is bounded. When full, evict the oldest presentation effect deterministically.
Reduced-effects mode shortens lifetime and suppresses secondary particles while retaining the
single shape needed to communicate the gameplay fact.

Dash trails sample the actor's rendered ground path into a bounded number of translucent cuboids;
they expire by client time and never query or modify dash authority. Slow and knockback use durable
state-derived ground rings/symbols so packet loss of a one-shot cue cannot leave the wrong status.

World health uses a dark background bar and team/health-colored fill above the fighter. The fill's
X scale and local offset encode a clamped health ratio. Bars use fixed-camera-aligned world
geometry, remain independent of actor skeletal animation, and hide for defeated/missing targets.
Team/facing ground markers use exact shared discs/rings and remain visible when the imported model
fails. Screen-space combat numbers are not added.

### Lifecycle and failure invariants

- exactly one durable visual root of each expected family exists per live owner;
- duplicate visual roots are deterministically collapsed; orphan roots are removed;
- no visual root is parented beneath a gameplay root;
- gameplay roots never receive presentation mesh/material/scene components;
- disconnect and session teardown remove all actor, projectile, preview, status, bar, and transient
  roots, including imported descendants;
- visual asset failure cannot close the playable gate or affect authority;
- no presentation system inserts, updates, or derives gameplay `Position`, `Rotation`, health,
  effect, projectile, ability, or scoring state;
- every transient collection and dedup history remains bounded.

## Implementation plan

1. Extract retained screen-space UI helpers from dormant `client/presentation.rs`; remove its old
   fighter sprite/Text2d plugin and tests.
2. Add the windowed `CombatPresentation3dPlugin`, explicit schedule sets, cached primitive/material
   resources, owner links, and lifecycle reconciliation.
3. Move the M01 fighter/weapon import proof to independent actor roots; add fallback promotion,
   scoped scene setup, animation selection, sentry primitives, and lifecycle tests.
4. Move straight/lobbed meshes and catch-up state to dedicated projectile roots; delete any visual
   component or transform writes on projectile gameplay entities.
5. Preserve pure preview geometry and replace sprite slots with cached 3D primitives.
6. Split cue ingestion/evidence from rendering; add bounded 3D cue effects, dash trails, durable
   status markers, reduced-effects behavior, and cleanup.
7. Replace sprite health bars/team identity with fixed-camera-readable 3D geometry.
8. Delete `src/combat/client/world.rs`, sprite materializers, and obsolete tests after equivalent
   ownership is installed; decompose the broad M01 3D module by responsibility.
9. Run targeted source/feature/schedule audits, automated verification, two-client visual matrix,
   and user playtest handoff. Do not begin M04 polish incidentally.

## Verification plan

## Implementation result

Implemented on 2026-08-20:

- gameplay fighters, straight/lobbed projectiles, and sentries no longer receive render meshes,
  scenes, materials, or presentation transforms;
- independent `CombatVisualOwner(Entity)` roots now own imported/fallback fighters, projectile
  catch-up, sentries, world health, durable status rings, dash trails, and scene descendants;
- the final dynamic pose writer performs the only gameplay-position-to-X/Z conversion after
  Lightyear interpolation and Avian writeback and before transform propagation;
- the selected Mini Characters model now uses named idle, walk, holding, shoot, and defeated clips,
  with cue-driven shoot transitions and a retained primitive fallback/weapon path;
- pure preview geometry is retained and materialized through 24 cached 3D cuboid slots;
- renderer-neutral cue ingestion remains in `ClientCombatPlugin`; the windowed 3D consumer owns a
  bounded 96-effect palette and reduced-effects scaling;
- obsolete sprite sentry/trail/status/health/effect materializers and the dormant 2D fighter/XY
  transform writer were deleted.

### Verification evidence

| Gate | Result |
|---|---|
| `just check` | Passed for routing, client, server, and network-test roles |
| `just lint` | Passed formatting, routing/client/server Clippy with warnings denied, and server feature isolation |
| `just test` | Passed: routing 83 plus process suites, client 353, server 301, network 82, performance 14 |
| Source audit | No `Sprite`, `Text2d`, `Mesh2d`, or `ColorMaterial` remains under `src/client` or `src/combat/client`; no fighter/projectile gameplay-root transform writer remains |
| Native two-client smoke | Passed after query repairs; imported fighter, team/facing ring, 3D preview, 3D world health, map, terrain, camera, and HUD rendered in `/tmp/brawler-m03-smoke.02qHAy/brawler-000180.png` |
| `git diff --check` | Passed |

The first native smoke exposed overlapping mutable `Transform` queries between health fills,
trails, and previews. The second exposed the equivalent `Visibility` overlap between health roots
and previews. Explicit marker/filter pairs made the families structurally disjoint; the third
native smoke ran without an ECS panic and produced the recorded screenshot. The reusable lesson is
that a render-only family marker must participate in both sides of every mutable-query disjointness
proof, and a MinimalPlugins schedule test cannot replace one native plugin-composition frame.

### User playtest handoff

Run `just server`, then `just run 2` (or use Practice from one `just client`). Exercise Wipeout and
Hot Zone with presets 1–4, move above and below the simulation Y origin, and inspect launch origin,
remote/local animation, preview agreement, health/team/status readability, dash/sentry cleanup,
defeat/respawn, reconnect, and restart. Also run one client with
`BRAWLER_FORCE_PRIMITIVE_WORLD=1 just client` to verify the fallback path. Report visual defects;
M03 remains open until feedback is classified and affected verification is rerun.

### Focused automated tests

- coordinate conversion at origin and positive/negative X/Y; all actor/projectile/status/bar roots
  have correct X/Z and render height;
- reconcile creates one independent root, repairs duplicates, removes orphans, and recursively
  cleans imported descendants;
- fighter and projectile gameplay roots contain no render mesh/material/scene marker and are not
  parents of visual roots;
- `WorldInstanceReady` searches only the owning character descendants; missing player, arm node,
  clip, weapon, or recursive dependency retains the complete primitive fallback;
- animation transitions cover idle/walk/holding/shoot/defeated/reset edges, remote attack cues, and
  no repeated one-shot restart without a new fact;
- sentry spawn, rotation, fire cue, removal, reset, and disconnect;
- straight launch at muzzle, bounded catch-up, no overshoot, malformed-source fallback, positive
  and negative simulation Y; lob X/Z ground path plus finite parabolic height;
- existing pure preview tests remain and new materializer tests cover slot bound, visibility,
  shared mesh/material handles, terrain-repaired landing, and no per-frame asset growth;
- cue dedup creates at most one effect; legacy duplicate cues create none; effect budget evicts the
  oldest; reduced mode retains primary readability and suppresses secondary work;
- health ratio clamps at 0/1, fill scale/offset is correct, bars hide when defeated, and status
  markers follow durable state expiry;
- schedule trace proves pose after Lightyear interpolation and Avian writeback and before transform
  propagation;
- headless client/server composition contains no render asset resources or systems from the new
  plugin.

### Canonical verification

Run the repository commands rather than substitutes:

```text
just check
just lint
just test
just network
```

Run `git diff --check` and targeted source audits for `Sprite`, `Text2d`, `Mesh2d`,
`ColorMaterial`, direct gameplay-root render bundles, and planar XY `Transform` writes. Remaining
uses must be screen-space UI or explicitly deferred M04 dependencies, never gameplay-world output.

### Windowed/network visual matrix

Use two native clients against the authoritative local topology and record screenshots/notes for:

- Wipeout and Hot Zone;
- each current straight, spread, lobbed, and melee weapon delivery;
- fighters near arena center and at positive/negative simulation Y to catch axis-dependent offsets;
- local and remote idle, walk, hold, shoot, defeat, respawn, health change, slow, and knockback;
- dash and sentry abilities, sentry firing/removal, projectile launch/impact, terrain collision, and
  lob landing/payload boundaries;
- imported-ready and forced-primitive fallback paths;
- normal and reduced-effects settings;
- reconnect, match restart, map replacement, and disconnect cleanup;
- supported window aspects and camera corners, checking readability and occlusion without widening
  scope into M04 polish.

Requested observations are pose alignment, muzzle origin, weapon grip/clipping, animation clarity,
preview-to-authority agreement, health/team readability, status/cue readability, z-fighting,
occlusion, stale visuals, and any difference between local and remote actors.

## Risks and mitigations

| Risk | Evidence/gate | Selected mitigation |
|---|---|---|
| incompatible pose conventions reintroduce offsets | positive/negative Y tests and source audit | independent roots plus one post-interpolation conversion writer |
| actor visual appears one frame at origin | spawn/reconnect capture | initialize converted pose at spawn; tolerate deferred full reconciliation only |
| animation cues differ for remote actors | two-client shoot/defeat/reset matrix | drive from replicated durable state and deduplicated network cues, not local input |
| holding pose conflicts with walking | grip/locomotion matrix | prefer readable aim/weapon pose; defer general masks unless the selected rig proves a narrow need |
| imported hierarchy search attaches to another actor | multi-actor scene-ready test | descend only from `WorldInstanceReady.entity` and store owner-scoped result |
| per-entity asset growth | asset-count tests before/after churn | unit cached meshes, bounded shared materials, slot reuse |
| transparent effects obscure play or sort poorly | both modes and overlap capture | simple ground-separated geometry, short lifetimes, bounded alpha, depth-tested primary shapes |
| world health becomes unreadable at camera angle | aspect/corner visual matrix | fixed-camera alignment and conservative vertical offset; retain screen HUD |
| visual cleanup leaks scene descendants | restart/disconnect entity-count gate | one owned visual root with recursive descendant cleanup and orphan reconciliation |
| presentation changes server/protocol feature graph | role-specific checks and dependency audit | install rendering plugin only in windowed client; no shared wire types |

## Exit criteria

M03 may move to `User playtest` only when:

- every current fighter, sentry, projectile, preview, ability/status, cue, and world-health family has
  a readable 3D result;
- durable gameplay entities own no world-render mesh/material/scene components and no visual roots
  are children of gameplay roots;
- no gameplay-world client system writes simulation `(x, y)` into `Transform.translation.(x, y)`;
- the projectile muzzle/catch-up path is correct at center and positive/negative simulation Y;
- imported character/weapon and forced primitive fallback paths both remain playable;
- cues/effects are deduplicated, bounded, optional, and presentation-only;
- the canonical check/lint/test/network suite, schedule tests, lifecycle tests, role isolation, and
  source audit pass;
- the windowed two-client matrix is recorded with known limitations and clear playtest instructions.

M03 becomes `Complete` only after user feedback is classified, accepted changes are reverified,
and the learn-from-errors review is recorded. M04 then owns final retirement/performance/readability
closeout.

## Research sources

### Exact local sources

- `src/client/presentation_3d/mod.rs` — current M01/M02 actor/projectile proof, coordinate ordering,
  asset promotion, animation, and launch catch-up;
- `src/combat/client/{mod.rs,cues.rs,effects.rs,hud.rs,preview.rs,world.rs,tests.rs}` — remaining
  sprite families, semantic cue boundary, geometry rules, bounds, and tests;
- `src/client/presentation.rs` — dormant legacy fighter/XY path plus screen-UI helpers to extract;
- `src/combat/cues.rs`, `src/combat/model.rs`, `src/abilities/{dash.rs,sentry.rs}` — durable state
  and cue facts available without protocol changes;
- `/Users/boyd/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.19.1/examples/animation/animated_mesh.rs`;
- `/Users/boyd/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.19.1/examples/animation/animated_mesh_control.rs`;
- `/Users/boyd/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_animation-0.19.1/src/transition.rs`;
- `/Users/boyd/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_world_serialization-0.19.1/src/world_asset_spawner.rs`;
- staged `external_assets/kenney_mini-characters` and
  `external_assets/kenney_blaster-kit_2.1`, inspected as upstream source only.

The cargo-registry Bevy 0.19.1 source is authoritative for exact APIs because the checked-in Bevy
reference is 0.20-dev and the current online Bevy examples can advance independently.

### Primary external sources

- [Bevy animated mesh example](https://bevy.org/examples/animation/animated-mesh/) — scene-root
  loading, `WorldInstanceReady`, scoped descendant search, graph installation, and animation player;
- [Bevy animated mesh control example](https://bevy.org/examples-webgpu/animation/animated-mesh-control/) — named glTF animations and transition-owned playback;
- [Kenney Mini Characters](https://kenney.nl/assets/mini-characters) — animated 3D pack and CC0
  license;
- [Kenney Blaster Kit](https://kenney.nl/assets/blaster-kit) — 3D weapon variations, animation, and
  CC0 license.

## Feedback and closeout placeholders

| Feedback | Decision | Verification |
|---|---|---|
| User directed M03 commit and M04 research with no additional visual correction | Accepted as the M03 playtest closeout; no code change required | Canonical verification and native smoke evidence above remained green |

### Learn-from-errors review

The first native visual smoke exposed overlapping mutable `Transform` and `Visibility` queries that
the focused `MinimalPlugins` tests could not schedule against the complete renderer. The cause was
relying on marker intent without proving query disjointness to Bevy. The correction added explicit
role markers and reciprocal `Without` filters. Future presentation families must test root/child
markers separately and run one native smoke before declaring verification complete.

Independent visual roots and the single post-interpolation pose writer prevented recurrence of the
axis-dependent muzzle offset. Scene descendant search remained scoped to each ready world instance,
and bounded effect/preview pools avoided per-frame asset creation. No new reusable project skill is
needed: these rules are now recorded in this milestone and the repository organization guidance.
