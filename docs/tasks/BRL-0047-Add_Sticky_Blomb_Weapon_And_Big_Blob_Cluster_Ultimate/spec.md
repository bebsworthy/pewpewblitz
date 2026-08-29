# Outcome

Players can equip a new Sticky Blomb primary weapon and a new Big Blob ultimate. The primary fires a straight projectile that becomes a visible delayed explosive when it contacts a fighter, obstacle, or maximum range. The ultimate is a targeted lob whose landing splits deterministically into six smaller straight sticky projectiles. All gameplay is dedicated-server authoritative, bounded, readable, and tunable in Balance Lab.

# Product behavior

## Sticky Blomb primary

- Firing launches one straight projectile along the accepted aim direction. It is not a lob and does not use arc-launcher landing logic.
- The projectile stops at the first authoritative fighter, deployable, damageable world target, Heist objective, or projectile-blocking map contact.
- A fighter contact attaches the armed blob to that fighter so it follows the target's authoritative planar position until detonation. Other contacts create a stationary world anchor at the resolved impact point.
- Reaching authored maximum range without contact creates a stationary armed blob at that endpoint instead of silently despawning or exploding immediately.
- The armed blob detonates when its authored fuse expires. The requested 1.15-second baseline is exactly 69 fixed ticks at 60 Hz.
- Its explosion applies authored hostile damage in a circle. The requested 2.67-cell baseline is 85.44 world units because one Brawler map cell is 32 world units.
- Explosion targeting deliberately ignores map occlusion, so eligible targets behind walls can be damaged. Projectile travel remains blocked by walls and obstacles.
- Multiple blobs may coexist on one obstacle or world anchor, subject to authored per-owner and hard match ceilings.
- A fighter can carry at most one primary Sticky Blomb. If another primary Sticky Blomb hits that fighter while one is attached, the existing primary detonates immediately at the fighter's current authoritative position and the new primary attaches with a fresh fuse.
- The chain rule is source-aware: only a direct hit by another primary Sticky Blomb triggers the existing primary. Secondary blobs from Big Blob neither trigger nor participate in this immediate-detonation rule.
- Unless separately authored later, explosions use the established hostile-recipient policy: no owner or ally damage.

## Big Blob ultimate

- Big Blob is a new targeted ultimate with lobbed travel and gamepad/mouse placement distance control, reusing the established targeted-ultimate input contract.
- The server validates charge, match state, input freshness, caster state, aim, maximum range, playable bounds, landing clearance, and active-blob ceilings before accepting it.
- On its authoritative landing tick, the parent blob is consumed and spawns exactly six secondary Sticky Blombs at 60-degree intervals in a deterministic hexagonal pattern.
- The pattern uses a canonical world-space phase so the six headings are stable across server, clients, replay/evidence, and frame rates. It does not depend on presentation rotation.
- Each secondary is a straight projectile. The requested 6.67-cell travel baseline is 213.44 world units.
- A secondary uses the same attach-or-anchor lifecycle as the primary, but has independently authored speed, collision radius, fuse duration, damage, and explosion radius. The requested secondary explosion-radius baseline is 1.33 cells, or 42.56 world units.
- Secondary blobs may attach or stack and explode through walls, but never trigger the primary-on-primary immediate detonation rule.
- The ultimate remains independently selectable with any legal weapon; it must not introduce a special weapon/ultimate combination restriction.

# Authored model and tuning

- Add a typed delayed-sticky straight delivery/runtime family rather than encoding fuse, attachment, or splitting as presentation behavior.
- Keep trajectory, projectile collision body, armed attachment/anchor state, payload, source generation, and presentation facts separate.
- Primary weapon content authors economy, fire cooldown, straight-flight speed/radius/range/lifetime, fuse ticks, explosion radius, damage, recipient policy, active ceiling, and presentation profile.
- Big Blob content authors maximum lob range, flight ticks/arc presentation, landing clearance, child count constrained to six for this content, fixed phase, child speed/radius/range/lifetime, child fuse, child explosion radius, child damage, active ceiling, and presentation profile.
- Expose these values through the owning Balance Lab Weapon and Ultimate sections with validation and persisted tuning. Do not add hardcoded balance variables.
- Add engine safety bounds for active armed blobs per owner and per match, spawned children per split, fuse/lifetime, range, speed, radius, targets per explosion, and serialized/evidence sizes.
- Do not expand the legacy point-cost model. If BRL-0043 has not removed compatibility fields when this is implemented, use only the minimum temporary catalog compatibility entry and leave removal owned by BRL-0043.

# Authoritative runtime

- Use stable attack, delivery, owner, team, weapon/ultimate, and source-kind identities; never replicate process-local entity identity.
- Model an armed blob as either a target attachment resolved through stable network identity or a world anchor. Replicate the immutable public identity plus bounded state required for presentation; keep damage selection and lifecycle mutation server-only.
- Resolve fighter attachment position from the target each fixed tick. If the carrier is defeated, despawned, or disconnected before detonation, convert the blob once to a stationary anchor at its last valid authoritative position and let its fuse continue.
- Destroyed static/damageable anchors do not cancel the blob; it remains at its resolved world position.
- Detonation gathers eligible targets deterministically, applies the authored unoccluded area payload once, emits one combat outcome/cue, records attribution, and despawns the armed blob.
- Exact tick ordering must make contact, immediate chain detonation, ordinary fuse expiry, damage application, defeat, charge observation, and cleanup deterministic. A blob cannot detonate twice in one tick.
- Owner defeat does not cancel already armed blobs. Match restart, worker teardown, and generation change clear all flight and armed state. Owner disconnect retains already armed blobs through their fuse but prevents new attacks.
- Late join and recovery receive enough replicated state to show current attachment/anchor, source kind, and fuse deadline without replaying prior damage or split events.

# Collision and target policy

- Straight flight uses the existing authoritative projectile body/sweep contract and effective projectile-blocking map geometry.
- The primary stops and arms on first eligible target or blocking contact; it does not apply direct contact damage.
- The Big Blob parent uses lobbed landing resolution and does not collide with fighters during flight.
- Spawned secondary origins must be repaired or rejected deterministically if landing clearance would place them inside blocking geometry; no child may tunnel through a blocker on its first tick.
- Explosions use `map_occlusion: false` by design and still respect recipient eligibility, live/defeated state, target-kind policy, maximum targets, and stable ordering.
- Damageable world objects and mode objectives receive explosion damage only under their existing positive-damage eligibility policies; status effects do not transfer to non-fighters.

# Presentation and controls

- Provide distinct flight and armed visuals for primary, ultimate parent, and secondary blobs.
- Armed blobs show a readable fuse/instability progression without numeric countdown text. Fighter attachments must remain legible near overhead health/status UI without obscuring it.
- Show the primary straight aim corridor and Big Blob lob landing preview using the same authoritative geometry and maximum ranges.
- Show an explosion-radius warning for armed blobs while preserving concealment rules; do not reveal a concealed carrier solely because a blob is attached unless the combat/reveal policy explicitly requires it.
- Emit bounded launch, impact/attach, arm, split, fuse, immediate-chain, and explosion cues with distinct audio/readability for primary and secondary blasts.
- Practice bots understand primary projectile travel/cover and avoid imminent armed-blob blast areas proportionally to their existing delayed perception; they do not gain hidden information.

# Scope boundaries

- No bouncing, homing, steering, piercing, remote manual detonation, arbitrary recursive splitting, terrain destruction, terrain ignition, persistent damage field, or client-side authority.
- No weapon-exclusive ultimate pairing rule.
- No generic user-authored behavior graph or scripting system; implement the smallest typed lifecycle required by this weapon and ultimate.
- BRL-0040 remains the owner of persistent occupancy/pulse areas and is not a dependency for this delayed one-shot blast family.

# Verification

- Pure tests cover tick conversion, six headings at exact 60-degree intervals, deterministic target ordering, active ceilings, source-aware chain eligibility, and authored validation bounds.
- Small ECS schedule tests cover straight contact-to-attachment, obstacle anchor, maximum-range anchor, moving carrier, 69-tick baseline fuse, expiry, primary-on-primary immediate detonation, replacement attachment, secondary non-chain behavior, carrier defeat/disconnect conversion, anchor destruction, and single-detonation guarantees at boundary ticks.
- Area tests prove the primary 85.44 and secondary 42.56 baseline radii, `map_occlusion: false` wall behavior, hostile-only recipients, damageable-world/objective policy, and maximum-target bounds.
- Ultimate tests cover targeted input distance, lob landing, exact six-child split, 213.44 child range, fixed world-space phase, blocked-origin repair, capacity rejection, charge consumption only on acceptance, and no primary/ultimate combination restriction.
- Routed tests cover source attribution, replication, late join/recovery, disconnect, restart cleanup, and independent client/server builds.
- Balance Lab tests cover validation, apply/persist/reload, diff visibility, and isolation from production persistence.
- Native playtest checks straight-shot feel, attachment readability on moving fighters, wall-bypassing blast clarity, chain-detonation comprehension, fixed six-way ultimate pattern, gamepad throw-distance control, sound/readability, and 3v3 performance under the maximum authored blob count.

# Acceptance criteria

- The primary is observably a straight shot and the ultimate parent is observably a lobbed targeted attack.
- Primary blobs attach/anchor and detonate exactly once according to authored fuse and chain rules.
- Big Blob lands and creates exactly six bounded straight secondary blobs separated by 60 degrees.
- Requested baseline conversions are represented as authored tuning: 69 ticks, 85.44 primary radius, 213.44 secondary range, and 42.56 secondary radius.
- Explosions can damage eligible hostiles behind walls while travel collision and all other recipient/target rules remain authoritative.
- All gameplay tuning is authored and editable in Balance Lab; no new hardcoded gameplay values or legacy point-cost dependency are introduced.
- Focused, canonical, routed, and native verification passes, durable weapon/ability documentation is updated, feedback is dispositioned, and the learning review is recorded before closure.


# Resolved design decisions

- Big Blob is **split only**: its parent landing applies no damage or status effect. Landing consumes the parent and creates the six secondary Sticky Blombs; all ultimate damage comes from those readable secondary explosions.
- An attached blob does not create a persistent or pulsing damage zone. While armed, its prospective explosion center follows the carrier's authoritative planar position. At immediate-chain or fuse detonation, the server snapshots that exact position and evaluates one circular area-damage transaction there. Movement before detonation therefore moves the eventual danger area; movement after detonation cannot affect the committed target set.
- Presentation must communicate that the danger is attached to and moving with the carrier, including the final explosion radius, without turning the carrier into a source of damage before detonation.


## Mandatory future-blast telegraph

- Every armed primary or secondary Sticky Blomb displays its **future one-shot explosion area** as a clear world-space circle for all active players and spectators. This is mandatory counterplay, not an optional local targeting aid.
- The circle is centered on the blob's current authoritative attachment or anchor position and uses that blob's authored explosion radius. On a moving carrier it tracks the carrier continuously; on an obstacle or maximum-range anchor it remains stationary.
- The telegraph persists from arming until detonation or cleanup and communicates fuse progression without numbers. It must be visually distinct from persistent/pulsing damage fields so players understand that the interior is safe until the single explosion.
- The future-blast circle remains visible through intervening walls because the explosion ignores map occlusion. If attached to a concealed fighter, it reveals the moving danger location but does not by itself reveal the fighter model, identity, health, or other concealed information.
- Client presentation reconstructs the circle from replicated authoritative blob state and deadline. It may interpolate motion visually, but it must not predict attachment, detonation, radius, or damage authority.
- Native acceptance requires players to identify the moving future blast footprint, distinguish primary and secondary radii, understand remaining fuse progression, and move the danger area by moving an attached carrier.


# Implementation plan

1. Extend the authored weapon/build schemas with one bounded delayed-sticky delivery definition and one Big Blob ultimate definition, preserving stable IDs and validation.
2. Add a cohesive server-authoritative sticky runtime owning flight termination, fighter attachment/world anchoring, source-aware chain detonation, fuse expiry, unoccluded area damage, cleanup, and exact fixed-tick ordering.
3. Reuse targeted-lob input for the ultimate parent and spawn exactly six straight secondary deliveries at its accepted landing tick.
4. Replicate bounded public sticky state and add client world presentation for the moving future-blast circle, fuse progression, projectile/armed blob distinction, and primary/secondary radii.
5. Expose every balance value in the owning Balance Lab Weapon/Ultimate editors and persistence schemas without adding new hardcoded gameplay tuning.
6. Add pure/ECS/routed regression coverage, run role-specific and canonical checks, then provide the native playtest path while keeping the ticket in doing until acceptance.

# Implementation progress (2026-08-29)

The first playable vertical slice is implemented and remains in `doing` pending native feedback.

- Added weapon preset 5, `Sticky Blomb`, as a typed `StickyStraight` delivery with authored economy, straight-flight geometry, 69-tick fuse, 85.44-unit unoccluded blast, hostile damage, and per-owner ceiling.
- Added ultimate 11, `Big Blob`, as an independently selectable targeted lob. The accepted landing is bounds/clearance repaired, deals no parent damage, and emits exactly six straight secondary blobs at fixed 60-degree world headings with authored 213.44-unit travel and 42.56-unit blast radius.
- Added server-owned arm, fighter/deployable attachment, stationary anchoring, maximum-range arming, primary-only immediate-chain, fuse detonation, owner-disconnect retention, deterministic same-tick replacement, and bounded cleanup behavior.
- Replicated stable armed-blob state and rendered a moving full-radius danger boundary plus nonnumeric authoritative fuse-progress fill. The parent lob, primary/secondary straight flights, and targeted previews use the existing projectile/lob presentation paths.
- Added gamepad targeted-ultimate distance sampling and Practice-bot Big Blob targeting through the existing targeted-ultimate contract.
- Added all numeric weapon/ultimate controls to Balance Lab, bumped snapshot/persistence/editor schemas, and migrated older persisted tuning by adding canonical Sticky Blomb and Big Blob entries without replacing existing values.
- Updated the durable weapons/abilities and Balance Lab documentation.

## Automated evidence so far

- `just check` — passed across routing, client, server, network-test, Balance Lab, and Balance Lab web test/build targets.
- Client library suite — 435 passed.
- Server library suite — 357 passed before the final disconnect-policy regression was added; all existing tests passed.
- Focused Sticky tests — attachment movement/defeat anchoring, primary-only chain eligibility, authored fuse/radius/ceiling, and disconnect persistence passed.
- Focused Balance Lab suite — 19 passed, including manifest path coverage and old-snapshot migration.
- Balance Lab web — 10 tests passed and production build succeeded.
- Clippy passed with `-D warnings` for routing, client, server, network-test, and Balance Lab; server feature isolation, V3 renderer, and V8 map cleanup guards passed.

## Native playtest requested

Run `just balance-lab`, equip **Sticky Blomb** and **Big Blob**, then verify: straight primary feel; attachment and moving future-blast circle; 1.15-second nonnumeric fuse progression; behind-wall blast damage; second-primary immediate detonation/replacement; Big Blob gamepad throw-distance control; split-only landing; exact six-way secondary pattern; and clear primary-versus-secondary radii. Keep BRL-0047 in `doing` until this feedback is dispositioned.

# Native approval and closeout (2026-08-29)

The user approved BRL-0047 after the playable Balance Lab handoff. This accepts the native behavior and presentation for the straight Sticky Blomb primary, moving attached future-blast telegraph, delayed unoccluded explosion and replacement-chain rule, gamepad-targeted split-only Big Blob lob, and fixed six-way secondary pattern. No additional corrective feedback was requested; BRL-0048 remains the independent owner of broader gamepad targeting-distance redesign.

## Learn-from-errors review

- The first sticky lifecycle system combined mutable blob `Position` access with Avian `SpatialQuery`, causing a Bevy runtime query conflict in full server composition. Splitting attachment movement and spatial detonation into explicitly ordered systems fixed the ownership conflict. Future gameplay lifecycles that both move entities and perform spatial queries should declare those as separate schedule phases from the outset.
- The first Balance Lab migration revision added the new condition rules but did not separately model the later weapon/ultimate inventory expansion. Sequential schema migrations now add each generation's canonical fields independently while retaining existing tuning. Inventory changes must always include a migration fixture that actually removes the entries absent from the older schema.
- Broad role tests and `-D warnings` caught stale preset bounds, editor field counts, and duplicated match arms that focused feature tests did not. Catalog inventory changes should update exhaustive preset loops and run role-specific checks before native handoff.

All acceptance criteria, automated evidence, durable documentation, native feedback disposition, and learning review are now complete.
