# Outcome

Players can queue or practice a short solo Showdown match in which each fighter has one life and the last surviving fighter wins as a visible server-owned boundary closes the playable space.

# Scope

- Add one solo topology only, bounded by the currently supported routed participant capacity.
- Add a focused Showdown mode plugin, distributed spawn requirements, elimination ranking, winner/draw/forfeit rules, timeout, results, restart, and recovery.
- Add a deterministic closing-boundary schedule, safe-zone state, outside-zone damage attribution, world/HUD telegraphs, and one compatible map.
- Eliminated human players enter a clear observer/results transition without influencing gameplay; Practice bots continue through their ordinary controller path.
- Add bounded public pickups only if the ticket's later specification proves they are necessary for exploration pressure; do not infer a general loot system.
- Practice bots receive survival, engagement, retreat, boundary, and pickup goals using permitted facts.

# Boundaries

- No team variant, battle pass, persistent loot, random item rarity, join-in-progress, public spectator service, or generic shrinking-zone framework.
- No respawn inside an active Showdown round.

# Acceptance criteria

- Admission, spawn separation, elimination, ranking, last-survivor, simultaneous defeat, boundary phases, damage, timeout, observer transition, results, restart, late join rejection, and recovery pass.
- Boundary and danger feedback remain readable at supported camera framing and reduced effects.
- The map provides viable early exploration, mid-match engagement, and final convergence without deterministic spawn advantage.
- Routed capacity, bots, native playtest, performance, documentation, and feedback gates pass.
