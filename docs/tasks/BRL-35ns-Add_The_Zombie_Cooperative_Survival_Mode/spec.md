# Outcome

Add Zombie as a complete player-visible cooperative PvE mode in which two to three human players use their saved brawlers against continuously increasing, server-controlled close-range zombie pressure. The mode uses the normal routed worker, authoritative combat and fighter lifecycle, resolved maps, replication, results, and recovery paths.

# Accepted player direction

- Human players form one cooperative team; there are no hostile human players and no Practice bots.
- A defeated player respawns independently after a relatively long delay, initially about 15 seconds. Players do not wait for a wave boundary.
- The run is lost when every human player is defeated at the same time.
- Waves or difficulty stages are internal spawn-director policy. Do not show wave announcements, wave numbers, or between-wave countdowns.
- New zombies should appear from validated ingress points near the map edge.
- Duration, pressure-stage count, spawn quantities, and cadence remain validated playtest configuration rather than hard-coded product decisions.
- Zombies use close-range combat. Their threat comes from numerical superiority, pursuit pressure, and surrounding players rather than ranged attacks or player-like abilities.
- Boost pickups grant stackable +5% maximum health and +5% outgoing damage per stack. Stacks last until that player dies.
- Boosts may come from chests, deterministic random zombie drops, or separate pickups scattered by defeated players.
- Individual pickups affect only the collector. Team pickups grant one stack to every teammate who is alive when collection commits.
- Death drops the defeated player's current boost stacks and clears them. Respawn starts at zero boosts.
- Every Boost pickup remains on the map until collected, match completion, restart, or teardown; there is no gameplay expiry timer.

# Survival loop and terminal rules

1. Two to three humans enter as team 0 and begin through the ordinary countdown.
2. A server-owned pressure director continuously spawns bounded zombie batches from legal edge ingress anchors. Internal pressure stages may change target population, batch size, and cadence without becoming visible match phases.
3. The director selects ingress anchors deterministically and prefers anchors outside a configurable player-exclusion radius and outside direct player sight. If every preferred anchor is temporarily unsafe, spawning waits or uses a validated bounded fallback; it never spawns on a player or outside playable space.
4. A defeated player receives a mode-authored respawn deadline, initially 15 seconds, and the ordinary authoritative reset/spawn-protection transaction returns that player with their saved build and zero boost stacks.
5. After each defeat and before any due respawn, Zombie rules evaluate the living human count. If it is zero, the server immediately commits Overrun. A simultaneous due respawn cannot rescue a team already declared defeated.
6. When exactly one player remains alive, the HUD presents an urgent Last Brawler Standing warning. When none remain, the authoritative Overrun result and all-player-defeated warning replace gameplay.
7. The first implementation remains a finite, bounded match, but its duration and internal pressure schedule are playtest-owned configuration. When the configured spawn schedule ends, the director stops creating zombies; defeating the remaining live zombies commits Cleared. The hard active deadline fails closed as Overrun.
8. Restart and Play Again reset pressure, zombie identities, drops, boost stacks, respawn state, navigation, and mode telemetry without restarting the worker.

The active match freezes one validated rules snapshot. Balance Lab or server configuration changes made during a playtest apply to the next run, never retroactively to a running match.

# Pressure director and close-range zombie behavior

- Start with one authored Zombie archetype and one melee capability.
- Zombies pursue reachable living players, close to authored melee range, visibly wind up, apply one authoritative contact attack, recover, and resume pursuit.
- Do not give the first archetype projectiles, ranged area attacks, Dash, ultimates, player weapons, or objective planning.
- Prefer increasing active count, replacement rate, and approach directions over hidden health/damage inflation so builds remain comparable.
- Pressure configuration owns an ordered bounded set of internal stages. A stage may define elapsed duration, spawn budget or target-live count, spawn interval, batch size, and drop policy. Stage transitions are not replicated as player-facing announcements.
- Start performance work around a code-owned maximum of 24 live zombies. Exact stage values and match duration are deliberately unset until playtest evidence. Raising the active cap requires fixed-tick, replication, client-render, and readability evidence at three humans.
- Independently bound total spawns per match, active zombies, spawn attempts per tick, pending director work, live boost pickups, and transient facts/cues.
- Target the nearest reachable living player with a stable identity tie-break and bounded commitment duration. Re-evaluate when the target dies, becomes unreachable, or the commitment expires; do not retarget every tick.
- Zombies may consume authoritative player locations. Practice-bot reaction delay, concealment fairness, team roles, weapon selection, and objective utility do not apply.
- Reuse the resolved-map navigation snapshot and extract only navigation primitives proven common by this second consumer. Begin with staggered bounded searches and replans; adopt a flow field or other horde-specific optimization only if measured capacity requires it.

# Zombie crowd occupancy and blocking

Live zombies block one another and may not stack into the same body footprint. This is a dedicated hostile-NPC crowd rule; current player fighters remain free of fighter-vs-fighter hard collision.

- Give zombies a dedicated collision membership that interacts with other live zombies and authoritative map blockers. Player projectiles and target queries intentionally include this hostile-NPC layer.
- Combine approach-slot steering around the target, bounded local neighbor separation, and authoritative move-and-slide/non-penetration resolution. Separation makes the crowd flow; collision is the final guarantee that bodies do not overlap.
- Resolve residual zombie-zombie penetration and yielding in stable identity order. A blocked rear zombie must slide, wait, select another approach slot, or replan rather than phase through the front rank.
- At a chokepoint, front-rank zombies physically occupy the opening while rear ranks spread or queue. Only zombies with an unobstructed valid melee contact may attack; rear zombies cannot damage through the bodies ahead of them.
- Bound neighbor collection and correction iterations, and verify 24-zombie crowd cost. Failure to find movement produces a stable wait/replan state, not teleportation, map penetration, or nondeterministic pushing.
- Zombie-zombie blocking does not automatically introduce hard zombie-player body blocking. Keep player movement collision unchanged for the first playtest and rely on pursuit/melee pressure; add player body trapping only if native evidence shows it is fun and escapable.

# Enemy model and combat authority

Zombies are not roster participants, fake players, Fighters, Sentries, map objects, or Practice bots.

Introduce a focused hostile-NPC family with stable replicated identity, definition, spawn generation, pose, allegiance, health, and lifecycle state. It may reuse mode-neutral combat capability components and Avian planar collision, but it must not carry MatchParticipant, PublicParticipantState, player profile/build ownership, network admission, common player respawn, or PracticeBotController state.

Zombie spawning, target selection, navigation intent, movement, melee timing, damage, defeat cleanup, drop rolls, and pressure membership run only on the server. Clients receive replicated facts and semantic cues and never submit any zombie outcome.

Zombie melee enters combat through a typed hostile-NPC attack source and the ordinary collect/validate/plan/commit/publish damage path. Player-owned primary weapons, ultimates, deployables, damage-over-time, fields, knockback, slow, and Cold must intentionally support an explicit HostileNpc target kind. Mode-neutral defeat facts identify hostile-NPC defeats; Zombie mode consumes those facts exactly once for cleanup, pressure accounting, drops, and telemetry.

Use a dedicated stable network-entity identity namespace for hostile NPCs and validate exhaustion and collision against player, dummy, deployable, pickup, and delivery identities.

# Runtime boost model

Boosts are mode-owned runtime power, not authored builds, saved profiles, passives, or permanent progression.

Add a replicated per-fighter boost-stack component with a validated gameplay-configured stack limit under a stricter code-owned ceiling. Do not mutate ResolvedMatchLoadout or ResolvedFighterStats. Instead, common focused helpers derive:

- effective maximum health = authored maximum health increased by 5% per stack;
- player-owned outgoing damage = ordinary resolved damage increased by 5% per stack.

Use deterministic integer or basis-point arithmetic with one documented rounding rule. Three stacks therefore mean +15% maximum health and +15% outgoing damage.

Damage scaling applies to damage owned by that player, including primary weapons, ultimates, deployables, fields, and damage-over-time. It does not scale neutral environment damage, pickup effects, healing, knockback distance, status duration, or zombie damage. The boost multiplier is applied once in the authoritative damage plan and must not be compounded again by delivery type.

Health recovery, healing payloads, restoration fields/pickups, HUD maximums, health bars, and fighter reset logic consume effective maximum health. Collecting a stack raises current health by the same absolute amount that it adds to maximum health, capped at the new effective maximum; the pickup is therefore immediately useful without acting as a full heal.

Death handling is one ordered transaction:

1. Observe the first authoritative fighter defeat fact for that life.
2. Snapshot the player's current stack count.
3. Deterministically plan one repaired scatter position and one stable pickup identity per stack, using capacity reserved specifically for player-death drops.
4. Commit clearing the player's boost state and spawning every planned pickup together, before later damage, HUD, or respawn work can treat the boosts as active.
5. Respawn later with zero stacks and ordinary base maximum health.

Each lost stack becomes a separate physical pickup entity. The server scatters them in a small deterministic pattern around the legal defeat position so a powered-up character produces a readable burst of collectible boosts. Every death drop is Individual, even when the stack originally came from a Team pickup; otherwise collecting and re-dropping one Team pickup could multiply team-wide grants.

Death drops have no public owner label, visual attribution, collection lock, or reservation. Any living teammate may take them immediately, leave them for the defeated player to recover after respawn, or split the scattered pickups. They remain until collected or the match ends, making that cooperative trust decision part of the loop. Any source-player provenance retained for server diagnostics stays server-private and is never replicated or presented.

# Boost pickup family and sources

Introduce a typed Boost pickup family rather than overloading RestorationPickup.

A replicated boost pickup has stable identity, source kind, grant scope, position, availability deadline, and lifecycle state. Every pickup represents exactly one +5% stack. Supported scopes are:

- Individual: grant one stack only to the deterministic collector.
- Team: grant one stack once to every currently alive, active teammate in the match. Dead, respawning, disconnected, or already completed participants receive nothing, regardless of who collected it.

Collection is server-authoritative and deterministically resolves simultaneous candidates. An Individual pickup remains available when the candidate is already at the stack limit. A Team pickup commits when at least one eligible living teammate can receive it; each eligible recipient independently clamps at the configured limit. Team collection never creates extra world pickups for capped or absent teammates. A Team pickup consumes ambient drop budget equal to the configured cooperative roster size, not merely one pickup, so its expected power remains comparable across two- and three-player matches.

Boost sources:

- Chest loot: an authored chest terminal/drop table may create an Individual or Team boost instead of or alongside an explicitly configured existing restoration outcome.
- Zombie drop: the director accrues a validated ambient drop budget and uses seeded randomness to choose which eligible zombie defeat pays it out and whether it is Individual or Team. Independent per-zombie luck cannot exceed or starve the configured run budget; clients never roll loot.
- Player death: each cleared player stack becomes one separate Individual pickup with a deterministic scatter identity and position. These drops are not randomized and are never converted into Team pickups.

All sources respect per-tick fact/cue caps, legal-position repair, next-tick availability, match generation, teardown, restart, and late-replication rules. Ambient chest/zombie drops stop at a lower live-pickup watermark that permanently reserves enough capacity for the maximum legal simultaneous player-death scatter. The configured stack ceiling and cooperative player ceiling therefore prove that every cleared stack can become one entity without unbounded growth. Capacity exhaustion fails closed with bounded telemetry and never duplicates a pickup or preserves stacks on a defeated player.

# Mode, routing, and topology

- Add a stable Zombie game-mode identity through the one current protocol/route migration; do not add compatibility decoders or per-message versions.
- Add one focused Zombie registration and ZombieModePlugin with validated rules, pressure state, boost/drop policy, schedule sets, terminal rules, restart reset, telemetry, and client projection.
- Add a one-team cooperative topology with two to three human participants. Zombies and boost pickups use their own bounded capacities, not participant capacity or MAX_PARTICIPANTS.
- Make lobby advertisement, queue formation, allocation, manifest validation, admission, worker composition, closeout, and automation mode-aware rather than weakening existing PvP validation globally.
- Add a Zombie rules summary and explicit PvE results: Cleared and Overrun, including survival duration and bounded pressure/defeat statistics. Do not encode Overrun as Draw or as victory for a synthetic zombie team.
- Admit exactly two or three real human participants; Zombie is not a solo mode and never fills its cooperative roster with Practice bots.
- Expose playtest-safe duration, pressure stages, respawn delay, boost stack limit, ambient drop budget, and Individual/Team drop weights through validated operator/Balance Lab configuration. Structural ceilings and identity limits remain code-owned.

# Map

Ship one purpose-built Zombie map recipe through the normal catalog and resolution path.

Add typed Zombie ingress anchors near the playable perimeter, separate from human spawn points. Validation proves one cooperative player spawn family with capacity for two to three players, bounded unique ingress anchors, collider clearance, playable-bounds containment, and reachable paths from every ingress into player space.

The map should support readable kiting loops, multiple approach directions, escape choices, and limited chokepoints without a permanent safe firing position. Spawn presentation may telegraph edge arrivals, but animation never authorizes spawn. Chests are included only where their Boost/restoration loot behavior and terminal outcomes are intentionally configured and tested.

# Client experience and readability

- Game Type Select and Dashboard advertise Zombie as cooperative PvE for two to three players.
- Do not show wave numbers, wave banners, between-wave timers, or hidden pressure-stage transitions.
- Always show the authoritative finite run timer without revealing internal spawn stages. When it reaches zero, replace it with a clear Remaining Horde state while spawning stops and players finish the live zombies.
- A persistent teammate panel shows each teammate's name, alive/defeated/respawning state, authoritative respawn countdown, health, and boost stack count.
- Show Last Brawler Standing when one teammate remains active and an unmistakable all-player-defeated/Overrun warning when the living count reaches zero.
- The local HUD shows boost stacks and the resulting health/damage bonus, e.g. 3 stacks / +15%.
- Individual and Team boosts have distinct shape/icon/audio treatment that is not color-only. Multiple death drops visibly scatter as separate collectible objects.
- Zombies have an unmistakable non-player silhouette, readable health, melee wind-up/contact/recovery, defeat feedback, and reduced-effects behavior.
- Semantic VFX/audio producers emit requests; generic presentation adapters choose assets and materialization.
- Aim assistance, targeting previews, projectiles, fields, Sentry targeting, concealment/relevance, camera framing, evidence capture, and diagnostics intentionally include or exclude zombies and boosts rather than inheriting accidental Fighter/Sentry/restoration behavior.

# Non-goals for the first playable slice

- Visible wave ceremony, forced between-wave downtime, or wave-bound player respawn.
- Solo Zombie admission.
- Endless production sessions, leaderboards, persistence, rewards, unlocks, currencies, or account progression.
- Hostile human players, PvP/PvPvE, matchmaking backfill, late joins, or replacing disconnected players.
- Ranged zombies, multiple zombie archetypes, bosses, elite affixes, procedural perks, random player upgrades, shops, manual revives, barricades, or base-defense objectives.
- Reusing Practice bot policy wholesale, building a public NPC SDK, or introducing a generic behavior tree/GOAP/navmesh framework.
- Mutating immutable resolved builds or raising participant limits to hold zombies.

# Acceptance criteria

- Two to three routed human clients can select Zombie, form a match, load the Zombie map, reach Cleared or Overrun, return to Results/Dashboard, and restart/requeue.
- Zombies continuously enter from valid edge ingress anchors under a bounded, playtest-configurable pressure schedule without any player-visible wave ceremony.
- Zombies use only close-range pursuit/melee in the first slice; numerical pressure increases while all work remains within declared capacities. Live zombies visibly block and separate from one another without stacking, attacking through a front rank, or permanently deadlocking a chokepoint.
- A defeated player drops exactly one separate Individual pickup per current stack in a bounded deterministic scatter, clears to zero exactly once, remains visibly respawning for the configured approximately 15-second delay, and returns independently at base stats.
- Overrun commits exactly when no human is alive, before any later respawn; surviving players continue when at least one human remains.
- Individual, Team, chest, budgeted seeded zombie-drop, and player-death boost sources obey their exact recipient, stacking, cap, permanent-until-collected, identity, and teardown contracts.
- Three stacks produce exactly +15% effective maximum health and +15% eligible outgoing damage under the documented rounding rule. Healing/recovery/HUD use the same effective maximum; ineligible mechanics are unchanged.
- Every supported player damage/effect family and relevant ultimate/deployable can affect hostile NPCs; zombie melee damages humans only through authoritative combat transactions.
- Map resolution rejects missing, unreachable, overlapping, unsafe, or over-capacity ingress topology.
- Reconnect/recovery coherently reconstructs pressure state, respawns, live zombies, boost stacks, and every permanent uncollected pickup for supported reconnect behavior; unsupported late joining remains explicitly rejected.
- Headless server builds acquire no rendering, audio, device-input, or client-asset dependency.
- HUD, results, 3D presentation, audio/VFX, evidence capture, diagnostics, telemetry, and Balance Lab expose the owned Zombie facts.
- Durable map/mode, combat, network, UX, presentation, and bot documentation explains the implemented contracts.

# Verification

- Pure tests: pressure-stage validation and transitions, spawn-anchor selection/fallback, capacity bounds, target tie-break/commitment, melee timing, effective-health/damage rounding, stack caps, controlled drop budgets, per-stack death scatter and reserved capacity, permanent pickup recovery, team recipient selection, and terminal result precedence.
- ECS schedule tests: spawn batching, navigation/movement ordering, zombie-zombie non-overlap, stable yielding, chokepoint flow, bounded crowd work, combat in both directions, hostile-NPC conditions/knockback, zombie defeat/drop cleanup, boost collection, player death clear/drop, 15-second respawn, last-player warning state, all-dead-before-respawn loss, restart, and no post-completion mutation.
- Separate-App network tests: protocol registration, zombie and pickup replication/despawn, boost stack and respawn projection, pressure state, reconnect/recovery, and observer relevance.
- Routed E2E at two and three humans: selection, formation, allocation, admission, boost pickup, partial-team death/respawn, all-player defeat, clear, restart, return, and process closeout.
- Capacity/performance evidence at three humans plus 24 live zombies under projectile/area-effect and pickup pressure, measuring fixed tick, network, and client rendering.
- Native playtest iterates duration, pressure-stage count, target live population, spawn cadence, respawn delay, controlled drop budget, stack cap, permanent-drop trust decisions, map ingress fairness, zombie crowd flow, chokepoint jams, melee telegraph, kiting, build usefulness, teammate-state clarity, warnings, and audio/VFX density.
- Run the repository's focused and canonical justfile verification paths and record exact commands/results before closeout.

# Feedback and closeout

This revision incorporates the player's respawn, invisible-pressure, close-range zombie, boost, chest/drop, team-recipient, and death-drop direction. Keep the ticket at idea until the revised first-slice scope is accepted. Move to doing only when implementation begins. Record later feedback as implemented, deferred to linked backlog work, rejected with rationale, or awaiting evidence. Complete a learn-from-errors review, update durable documentation, run ticket sync, and move to done only after every required criterion and feedback item is closed.

## Implementation evidence — 2026-09-02

Implemented the first complete playable slice across the closed route/protocol migration, one-team lobby/allocation/admission/worker topology, Zombie Yard recipe and ingress validation, authoritative pressure/crowd/melee runtime, hostile-NPC combat target support, independent lifecycle respawns, Cleared/Overrun results, permanent individual/team boost pickups and per-stack death scatter, HUD/results, and bounded 3D Zombie/boost presentation. Added `just zombie-e2e 2|3` for the exact supported cooperative rosters. Durable map/mode, combat, network, UX, and bot documentation now owns the shipped contracts.

Correctness review fixed three runtime hazards before handoff: boost collection now uses a Bevy `ParamSet` instead of conflicting read/write queries; simultaneous melee skips a target already reduced to zero health so defeat facts remain exactly once; and melee contact revalidates static/destructible line of sight after wind-up. Zombie slow/freeze and external motion now affect authoritative crowd movement, and every health display uses effective boosted maximum health.

Verified:

- `cargo check --workspace --all-features --lib --bins` — passed.
- `cargo fmt --all --check` — passed.
- `cargo test -p brawler-routing --lib` — 83 passed.
- `cargo test --lib --all-features` — 989 passed.
- Focused Zombie catalog, hidden-pressure, scaling/capacity, map topology, mode-registry, and compatibility tests — passed and included in the full suite.
- `just zombie-e2e 2` — one exact two-human Zombie roster reached Active and all processes closed cleanly.
- `just zombie-e2e 3` — one exact three-human Zombie roster reached Active and all processes closed cleanly.
- Strict Clippy reaches only unrelated pre-existing failures in `src/diagnostics/process/sampling.rs` and `src/client/prediction.rs`; Zombie-owned findings were corrected.
- `cargo check --workspace --all-targets --all-features` is blocked only by unrelated stale APIs in `tests/network/prediction.rs`: missing `client::prediction::resolve_static_arena` and removed `ResolvedMapSnapshot.geometry`.

The ticket remains doing. Native subjective playtest, full-match Cleared/Overrun/restart observation, three-human 24-zombie capacity evidence, and mode-specific semantic audio/VFX density still require evidence before acceptance and closeout. Balance currently exposes duration, hidden pressure-stage count, and respawn delay in `config/server/game-types.ron`; deeper live spawn/drop/stack tuning remains in the validated frozen `ZombieRules` snapshot and should be promoted to the operator/Balance Lab surface if the first playtest confirms those controls are needed.

Learn-from-errors review: adding a fourth mode exposed assumptions not only in production topology but in compatibility fixtures, E2E roster arithmetic, and test registration order. Future closed-mode migrations should search production code, goldens, and automation together before the first full suite. Any system that both selects and mutates the same component family should start with `ParamSet`, and delayed melee must always revalidate target life, distance, and geometry at contact rather than trusting wind-up state.

## Feedback correction — Balance Lab startup panic

Implemented the reported Bevy B0001 correction in `client/presentation_3d/zombie.rs`. Zombie roots, zombie health fills, and boost roots all mutate `Transform`; their query filters now make the three marker families explicitly pairwise disjoint. Added a system-initialization regression test that fails at schedule construction if those accesses can alias.

Verification: the focused regression test passed; `just balance-lab` started the supervisor and interactive client and remained stable beyond the former presentation initialization panic until manually stopped with Ctrl-C; `cargo check --workspace --all-features --lib --bins` passed; and `cargo test --lib --all-features` passed 990 tests. Feedback disposition: implemented and verified.

## Direction correction — Zombie Practice

The earlier “no Practice bots” interpretation is superseded. “Zombie is not solo” means the match must always have a cooperative roster; it does not prohibit AI teammates. Practice is supported with one human plus friendly server-hosted bots filling the selected two- or three-player Zombie roster. These bots are team 0 allies, fight hostile NPCs through ordinary fighter input/combat authority, participate in alive/all-dead terminal evaluation, respawn and boost rules, and never stand in for hostile zombies. Dashboard/Game Type UI must enable Practice for Zombie, and Balance Lab may launch the normal one-human-plus-friendly-bots Zombie composition.

Acceptance correction: Zombie Practice must form and reach Active for both `zombie-2p` and `zombie-3p`; friendly bots must acquire and attack hostile NPCs rather than requiring an enemy-player team or mode objective. The prior non-goal and verification statements excluding Practice bots are obsolete.

## Zombie Practice implementation evidence — 2026-09-02

- Corrected the earlier misunderstanding: Zombie remains cooperative, and Practice now fills the selected 2- or 3-fighter team-zero roster with one or two friendly server-hosted bots.
- Removed Zombie-specific Practice rejection/disabled UI policy from lobby allocation, routed topology validation, match-worker manifest admission, and Dashboard presentation.
- Zombie now publishes the semantic elimination/pressure bot objective. Live HostileNpc entities are projected as bot enemy contacts; bots retain the ordinary delayed-observation, arbitration, navigation, and FighterInput authority path.
- Extended the canonical Practice E2E allowlist to zombie-2p and zombie-3p.

Verification:
- cargo fmt --all -- --check — passed.
- cargo test -p brawler-routing --lib — 84 passed.
- cargo test --lib --no-default-features --features server zombie_practice_bot_rows_fill_only_the_friendly_team — passed.
- cargo test --lib --no-default-features --features server match_workers_control_manifest_bots_in_every_practice_mode — passed; the Zombie case asserts a bot aims toward a live hostile NPC after the authored reaction delay.
- cargo test --lib --all-features — 991 passed.
- just practice-e2e zombie-2p — routed Practice reached Active with one human and one friendly bot.
- just practice-e2e zombie-3p — routed Practice reached Active with one human and two friendly bots.

Native playtest remains required before closeout.

## Playtest feedback — zombie shots do not register

- Player reports that shots cannot damage zombies; projectile hits do not register.
- Treat as a blocking gameplay defect in the current Zombie slice. Trace authoritative delivery collision selection, hostile-NPC target classification, and damage transaction validation; add a regression proving an ordinary player shot reduces hostile-NPC health before considering the feedback resolved.

## Playtest correction evidence — projectile hits on zombies

- Root cause: the ordinary straight-projectile spatial query mask included fighters, deployables, and map geometry, but omitted the `HOSTILE_NPC_LAYER`; zombies therefore never became sweep candidates even though target resolution and damage application already supported `HostileNpc`.
- Correction: include `HOSTILE_NPC_LAYER` in the authoritative straight-projectile sweep filter.
- Regression: `ordinary_straight_projectile_damages_a_live_zombie` boots a production Zombie Practice worker, lets the director spawn a live zombie, fires an ordinary primary projectile through the normal fixed-post-update pipeline, and verifies authoritative health decreases.
- Feedback disposition: implemented; awaiting native playtest confirmation.

Verification:
- `cargo test --lib --no-default-features --features server ordinary_straight_projectile_damages_a_live_zombie` — 1 passed.
- `git diff --check` — passed.
- `cargo fmt --all -- --check` — passed.
- `cargo test --lib --all-features` — 992 passed, 0 failed.

## Playtest correction — Zombie character model

- Replace the generated primitive zombie body with Kenney Mini Characters `character-male-d.glb` from `external_assets/kenney_mini-characters`.
- Promote the GLB into the shipped client asset tree, register its existing CC0 provenance in `assets/manifest.ron`, and retain it through `ClientAssetHandles`.
- Preserve the current primitive zombie as the deterministic degraded fallback while the optional GLB is loading or unavailable; automatically upgrade fallback visuals once the scene resolves.
- Preserve authoritative zombie collision, health, targeting, and combat behavior; this correction is presentation-only.
- Acceptance: live Zombie NPCs render with `character-male-d`, health bars and wind-up emphasis remain attached, the promoted asset and license are covered by tests, and client/all-feature verification passes.

## Playtest correction evidence — Zombie character-male-d

- Promoted `external_assets/kenney_mini-characters/Models/GLB format/character-male-d.glb` to `assets/brawler/models/kenney/mini-characters/character-male-d.glb`; SHA-256 matches the source (`dd12b2e75ffb1cdb45aaec3916c3d6e732929ccb4a8b89b946f69baaabfa64da`).
- Registered `model.character_male_d` with Kenney Mini Characters CC0 provenance and the existing retained license.
- Zombie presentation now instantiates the model default scene at the established Kenney character orientation and scale. Existing generated geometry remains only as the degraded/asynchronous-load fallback and is replaced automatically once the GLB resolves.
- Health UI and authoritative gameplay ownership remain unchanged.
- Feedback disposition: implemented; awaiting native visual confirmation.

Verification:
- `cargo check --all-features` — passed.
- `cargo test --lib --all-features` — 993 passed, 0 failed.
- `cargo fmt --all -- --check` — passed after formatting.
- `git diff --check` — passed.
- Source/promoted GLB SHA-256 comparison — identical.

## Playtest balance correction — Zombie scale, durability, and count

- Interpret the reported “more XP” as more HP because the current Zombie loop has no XP/reward progression system.
- Make the Zombie gameplay model use the exact same imported-character scale function as player fighters, preventing presentation scale drift.
- Increase default zombie health from 180 to 240 so an unboosted zombie is more durable than the current approximately 211-health Practice brawler.
- Increase pressure targets from 4/7/10/13 to 8/13/18/23 live zombies across the four default stages, and increase each spawn batch from 2 to 4 while retaining the measured code-owned cap of 24 live zombies.
- Preserve match duration, spawn cadence, movement speed, melee damage, collision size, and total spawn cap for an isolated playtest balance correction.
- Acceptance: exact shared player/zombie model scale, bounded 8/13/18/23 pressure targets, 240 zombie HP, updated rule regression, and full verification.

## Playtest balance evidence — scale, HP, and horde density

- “More XP” was implemented as more HP because Zombie mode has no XP progression system.
- Player fighters and `character-male-d` zombies now share `gameplay_character_scale`, using identical X/Z footprint fitting and the same reviewed Kenney world-height scale.
- Default zombie HP is 240 (previously 180).
- Default live targets are 8/13/18/23 across four pressure stages (previously 4/7/10/13); spawn batches are 4 (previously 2).
- The existing `MAX_LIVE_ZOMBIES = 24`, total spawn limit, spawn cadence, collision geometry, and combat behavior remain unchanged.
- Feedback disposition: implemented; awaiting native balance confirmation.

Verification:
- Focused pressure rule regression — passed.
- Zombie presentation system initialization regression — passed.
- `cargo check --all-features` — passed.
- `cargo test --lib --all-features` — 993 passed, 0 failed.
- `cargo fmt --all -- --check` and `git diff --check` — passed.

## Blocking crash feedback — boosted lethal payload reservation

- Native Zombie playtest crashed the authoritative worker in `resolve_composed_payloads` at `transaction.rs` with `payload event reservation matches defeat`.
- Root cause: the reservation dry run used authored base damage, while transaction planning applies the attacker’s permanent Zombie boost stacks before determining lethality. A shot whose base damage is nonlethal but boosted damage is lethal reserves one damage event, then consumes a missing second defeat event and panics.
- Correction scope: centralize immediate payload boost scaling and use it identically in event reservation and transaction application. Do not weaken the invariant or replace the panic with silent event loss.
- Regression: a one-stack primary hit whose base amount is below target health but whose boosted amount exactly defeats must reserve both damage and defeat events; the unboosted control must reserve only damage.
- Acceptance: focused regression, all-feature suite, formatting, and diff checks pass; native confirmation remains required.

## Crash correction evidence — boosted lethal payload reservation

- Confirmed the worker crash was not caused by client rendering or the raised horde cap. Event reservation and application disagreed about lethality because only application scaled immediate player damage by Zombie boost stacks.
- Added the shared `boost_scaled_immediate_damage` rule and now call it from both dry-run reservation and atomic transaction planning. Environment and hostile-NPC damage remain intentionally unboosted.
- Added `boost_scaled_lethal_damage_reserves_damage_and_defeat_events`: 100 base damage against 105 HP reserves one damage event without boosts and two events (damage plus defeat) with one +5% stack.
- Preserved fail-closed complete-batch reservation; no panic/assertion was weakened and no event is silently dropped.
- Feedback disposition: implemented; awaiting native replay confirmation.

Verification:
- Focused boosted-lethal reservation regression — passed.
- `cargo check --all-features` — passed.
- `cargo test --lib --all-features` — 994 passed, 0 failed.
- `cargo fmt --all -- --check` and `git diff --check` — passed.

## Playtest correction — animated Zombie model

- `character-male-d` currently instantiates only its default scene and never installs an animation graph, so its embedded 32-animation set remains in bind pose.
- Prepare a Zombie-owned animation graph from the model’s verified `idle`, `walk`, and `attack-melee-right` named clips.
- On `WorldInstanceReady`, find the scene AnimationPlayer, attach the graph and transitions, and start the phase-appropriate animation.
- Map Pursuing to looping walk, Windup to one-shot melee attack, and Recovery to looping idle. Animation remains client-only and does not alter authoritative melee timing.
- Retain the generated primitive fallback if the scene or required animation clips fail to load.
- Acceptance: phase mapping has focused tests, scene/animation systems compile without ECS conflicts, and all-feature verification passes.

## Animated Zombie evidence

- Verified `character-male-d.glb` embeds the required named clips `idle`, `walk`, and `attack-melee-right` (within its 32-animation set).
- Added Zombie-owned asynchronous scene/graph preparation. Imported zombies are materialized only after the default scene and all required clips resolve; otherwise the primitive fallback remains.
- Added a `WorldInstanceReady` observer that locates the instantiated AnimationPlayer, installs the Zombie graph and transitions, and begins the current phase animation.
- Added render-time phase transitions: Pursuing loops walk, Windup plays one melee attack, and Recovery loops idle. Authoritative contact timing remains unchanged.
- Hardened the observer to ignore unrelated scene-ready events while the optional Zombie animation resource is unavailable.
- Feedback disposition: implemented; awaiting native visual confirmation.

Verification:
- Focused Zombie phase-animation mapping test — passed.
- Existing Zombie mutable-query initialization regression — passed.
- `cargo check --all-features` — passed after final observer hardening.
- `cargo test --lib --all-features` — 995 passed, 0 failed before the final optional-resource signature hardening; affected focused test passed afterward.
- `cargo fmt --all -- --check` and `git diff --check` — passed.

## Playtest correction — unaimed fire does not target zombies

- Controller primary fire without a committed right-stick aim uses `ViewportAutoAim`; its hostile-combatant pass queries only `Fighter`, while the later nonfighter pass contains sentries and damageable world objects but no `HostileNpc`.
- Add a dedicated live hostile-NPC query carrying position, team, stable network ID, health, and defeat state.
- Merge viewport-visible live hostile NPCs with hostile fighters before nearest-distance selection. Use stable `NetworkEntityId` ordering for deterministic equal-distance ties.
- Preserve manual aim priority, viewport visibility limits, recipient/action eligibility, and the existing sentry/world-object fallback.
- Regression: hostile-combatant selection succeeds with only an NPC candidate and chooses the nearer actor across fighter/NPC candidates.
- Acceptance: focused targeting tests and full all-feature verification pass; native controller confirmation remains required.

## Unaimed Zombie targeting evidence

- Root cause confirmed in `ViewportAutoAim`: hostile fighters, sentries, and damageable world objects were queried, but replicated `HostileNpc` entities were absent from every candidate set.
- Added `AssistedHostileNpcQuery` with position, team, network ID, current health, and defeat state.
- Merged live, undefeated, viewport-visible hostile NPC candidates with hostile fighters before nearest-distance selection. Equal-distance ordering uses stable replicated network identity.
- Manual aim remains authoritative over assistance; sentry/world-object fallback and action eligibility are unchanged.
- Added `nearest_hostile_combatant_includes_npcs_and_compares_across_actor_kinds`, covering NPC-only selection and nearer-NPC selection against a fighter candidate.
- Feedback disposition: implemented; awaiting native controller confirmation.

Verification:
- Focused hostile-combatant targeting regression — passed.
- `cargo check --all-features` — passed.
- `cargo test --lib --all-features` — 996 passed, 0 failed.
- `cargo fmt --all -- --check` and `git diff --check` — passed.

## Commit verification — 2026-09-03

- Exact staged snapshot, excluding unrelated local VFX work: `cargo test --lib --all-features` passed 993 tests.
- `git diff --cached --check` passed.
- The staged-only build exposed and removed an accidental dependency on the separate VFX branch's `effect_enemy` material; Zombie presentation now uses the existing `marker_enemy` material.
