# Outcome

Players can queue or practice a short solo **Last Stand** match. Each fighter has one life, a visible server-owned boundary closes the playable space, and the last surviving fighter wins.

# Naming

BRL-wg28 (owner decision, 2026-11-20) renames Showdown to **Last Stand** everywhere: player text, the `GameMode::LastStand` variant, the `last-stand` mode key, and `last-stand-*` game-type IDs. BRL-wg28 is ordered before this ticket. If this ticket starts first anyway, it is born with the new names. "Showdown" must not appear in new code or UI.

# Scope

- Add one solo topology: N teams of one fighter, with N fixed by the owner's capacity answer (the routed ceiling is `MAX_PARTICIPANTS` = 8, `packages/brawler-routing/src/limits.rs:35`).
- Add a focused mode plugin under `src/modes/` and `src/matchplay/`, with distributed spawn requirements, elimination ranking, winner, draw, and forfeit rules, timeout, results, restart, and recovery.
- Add a deterministic closing-boundary schedule, safe-zone state, outside-zone damage, world and HUD telegraphs, and one compatible map.
- Eliminated humans enter a small observer state that cannot influence gameplay: a camera that follows a surviving fighter, then results. There is no public spectator service.
- Practice bots get survival, engagement, retreat, boundary, and chest/pickup goals, using only permitted facts and acting through their ordinary controller path.
- Adding the `GameMode` variant is the global protocol compatibility bump (`docs/08-network-architecture.md`).

# Research findings (2026-11-20)

The current match code supports only one or two teams:

- `MatchLifecycleRules` rejects team counts outside 1..=2 (`src/matchplay/server.rs:74`), and `server.rs:799` has a two-team special case.
- Team assignment is fixed at `[u8; 2]` (`src/matchplay/spawns.rs:9`). Queue formation assigns `index % team_count` (`src/server/lobby/queue.rs:1015`).
- The client HUD scoreboard, results, kill feed, and name plates assume `TeamId(0)` and `TeamId(1)` (`packages/brawler-client/src/hud/`, `flow/screens/results/`).
- Ally/enemy relationship VFX already work for free-for-all, because every other fighter is simply an enemy.

Generalising team count is therefore core scope, not a side effect. Keep it bounded: supported team counts become 1..=N with N the solo capacity. Wipeout, Hot Zone, Heist, and Zombie keep their exact current topology, and their two-team HUD stays as it is. Solo gets its own compact standings HUD rather than a generic N-team scoreboard framework.

Existing pieces to reuse:

- Outside-zone damage goes through the existing environmental damage path (`src/combat/environment.rs`), so combat remains the only module that commits health and defeats.
- Chest pickups reuse Zombie's chest-to-boost path (see Decisions). No loot system.
- `docs/04-maps-and-game-modes.md:299` mentions twelve solo fighters. That exceeds the routed ceiling. Correct it to the chosen N.

# Build order (one ticket, reviewable steps)

1. Topology: a bounded team count of 1..=N through lifecycle rules, spawn assignment, queue formation, and admission, with regression tests proving the existing modes are unchanged.
2. Rules core: the mode variant, one life, elimination ranking, last survivor, simultaneous defeat, timeout and draw, results, and focused `App` tests.
3. Boundary: the deterministic phase schedule, the safe-zone state, and damage through `combat::environment`.
4. Map and spawns: distributed, validated spawn separation with no deterministic advantage.
5. Presentation: boundary telegraphs that stay readable with reduced effects, the standings HUD, the observer camera, and results.
6. Practice bots, routed and network tests, the N-fighter performance gate, and docs.

# Boundaries

- No team variant, battle pass, persistent loot, random item rarity, join-in-progress, public spectator service, or generic shrinking-zone framework.
- No respawn inside an active round.
- No generic N-team scoreboard for the existing modes.

# Acceptance criteria

- Admission, spawn separation, elimination, ranking, last survivor, simultaneous defeat, boundary phases, outside-zone damage, timeout, the observer transition, results, restart, late-join rejection, and recovery pass focused tests and `tests/network/` scenarios.
- Existing 1v1, 2v2, 3v3, and Zombie topologies pass unchanged.
- Boundary and danger feedback stay readable at the supported camera framing and with reduced effects.
- The map supports early exploration, mid-match engagement, and final convergence without a deterministic spawn advantage.
- The N-fighter routed capacity and performance gates pass, and Practice bots handle the boundary.
- `docs/04-maps-and-game-modes.md` is updated: the mode moves to supported, and the solo capacity line is corrected. Learning notes are recorded here.

# Verification

- `just lint` and `just test`
- `just practice-e2e last-stand-<N>p` and `just rejoin-e2e last-stand-<N>p`
- `just e2e <N>`, or the routed matrix, at the chosen capacity
- `just v3-render-evidence` with `BRAWLER_RENDER_GAME_TYPE=last-stand-<N>p` at full capacity, with captures attached (a mid-match frame and a late-boundary frame)

# Playtest notes (owner, after done)

This is not a gate for done. The agent fills in the run path and controls when the work lands. The owner plays a full Practice round and observes:

- whether the boundary pacing and telegraphs are readable;
- whether any spawn feels advantaged;
- whether the final convergence produces a real fight;
- whether being eliminated and observing feels clear.


# Decisions

- 2026-11-20, owner (q-9xg5t3j): **6 fighters** (six teams of one). The game-type ID is `last-stand-6p`, and the `<N>` in the verification commands is 6. The performance gate runs at the existing 6-active-fighter budget. Raising the count later needs new evidence and a new ticket.
- 2026-11-20, owner (q-1sqcp61): pickups come from **destructible chests**. The map places neutral `treasure-chest` assets (`MapAssetId(26)`). Destroying one drops a public boost pickup that any fighter can collect. Collecting it grants a bounded boost stack for the rest of that life.
  - Reuse Zombie's existing chest-to-boost path: `ProcessedZombieChests`, `BoostPickup`, `BoostPickupSource::Chest`, and `BoostGrantScope::Individual` in `src/matchplay/zombie.rs`. Do not write a parallel implementation. Extract the shared part into a helper, owned beside the map chest terminal path, only where both modes behave identically, and record which part was shared.
  - Agent default, not asked: carried stacks are **not** dropped on defeat. This keeps the round free of snowball drop-scatter rules, and can be revisited from playtest feedback.
  - Stack value, cap, chest count, and chest health are authored per game type, starting from Zombie's +5% health and +5% damage per stack. Chest placements are mirrored or rotationally fair across the six spawns.
  - Bots get a destroy-chest and collect goal (the pickup goal in Scope).
