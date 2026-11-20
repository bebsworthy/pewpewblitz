# Outcome

Players can queue or practice a complete team objective match that converts combat advantage into collecting, carrying, protecting, dropping, and recovering a contested resource, then holding a target count through a visible countdown.

# Naming

"Gem Grab" is the name of an existing commercial game's mode and must not ship as a player-facing name. The gameplay family (collect, carry, hold a threshold) is genre-generic and stays.

The working name is **Shard Rush**, with shards as the resource. This is pending the owner question on this ticket. Use the chosen name for the mode key, the `GameMode` variant, the game-type IDs, the HUD, and the docs. The internal identity (`GameMode::GemGrab`, `gem-*` keys) must not use "Gem Grab".

# Scope

- Add a focused mode plugin under `src/modes/` and `src/matchplay/` with stable rules, replicated public state, HUD, cues, results, restart, recovery, and routed admission compatibility.
- The new `GameMode` variant in `src/config.rs` is a closed protocol enum change and needs the global compatibility bump described in `docs/08-network-architecture.md`.
- Add typed map anchors for the resource source, plus one dedicated 3v3 map.
- Advertise a `<mode>-3v3` game type in `config/server/game-types.ron`. Other team sizes are out of scope unless the map and playtests show they work.
- The source spawns resources on deterministic bounded deadlines, up to an authored live-count ceiling.
- Live fighters collect resources through server-owned overlap, and the carried count is public and bounded.
- Defeat drops carried resources at validated positions. Disconnect, restart, and teardown each have an explicit outcome.
- When a team's combined carried count reaches the target, a visible hold countdown begins. Falling below the target applies the owner-chosen rule (see the questions).
- Timeout and simultaneous terminal conditions resolve deterministically.
- Practice bots get focused collect, protect-carrier, intercept-carrier, and recover-drop goals, acting only through ordinary input.

# Reuse: Zombie pickups

Zombie (`src/matchplay/zombie.rs`, `BoostPickup` and related types) already implements server-owned pickup entities, bounded carried counts, and drop-on-death as permanent pickups. Study it before writing new pickup code.

Extract a shared helper only for behaviour that is truly identical in ownership and lifecycle (AGENTS.md, `docs/04-maps-and-game-modes.md` "shared match machinery is extracted only where…"). Mode-specific scoring, spawning, and HUD stay mode-local. Record in this spec what was shared and what was not.

# Authored rule values

These are proposed defaults. They live in `game-types.ron` and mode policy, and all are converted with `src/timing.rs`:

- target team count: 8
- hold countdown: 12 s
- source spawn interval: 6 s, with a live-count ceiling of 12 on the ground
- maximum carried per fighter: the target count
- dropped resources scatter to validated free cells around the defeat position, never inside walls or out of bounds
- match duration, countdown, and respawn: the same as the other 3v3 modes

Tune these from playtest feedback.

# Concealment

Carrier visibility depends on the owner's answer to the concealment question. Whatever is chosen is enforced server-side through the existing concealment and interest path (`docs/17-concealment.md`), never only on the client.

# Build order (one ticket, reviewable steps)

1. Rules core: the mode variant, config, the source, collection, drops, threshold and countdown, results, and focused `App` tests.
2. Map: anchors, the dedicated 3v3 map, validation, and admission.
3. Presentation: HUD counts, the countdown, dropped-resource and source cues, and results.
4. Practice bots: collect, protect, intercept, and recover goals.
5. Routed and network tests, the performance gate, and the docs.

# Boundaries

- No generic inventory, currency, progression, loot rarity, random drops, executable map scripting, or additional mode framework.
- Resources exist only inside the active match and never enter player profiles.

# Acceptance criteria

- Collection, drop, pickup, threshold, countdown, cancellation or reset, timeout, draw, results, restart, late join, disconnect, and recovery rules pass focused tests and `tests/network/` scenarios.
- The dedicated map validates and gives fair approach, retreat, carrier-protection, and comeback paths.
- HUD and world feedback make source cadence, dropped resources, carrier counts, the threshold, and the countdown understandable in 3v3.
- The maximum 3v3 capacity and performance gate passes. Practice bots visibly pursue all four goals.
- `docs/04-maps-and-game-modes.md` moves the mode from "Envisioned mode families" to a supported mode section. The learning notes are recorded here.

# Verification

- `just lint` and `just test`
- `just practice-e2e <mode>-3v3`, `just rejoin-e2e <mode>-3v3`, and `just network-change-e2e <mode>-3v3`
- `just v3-render-evidence` with `BRAWLER_RENDER_GAME_TYPE=<mode>-3v3` and `BRAWLER_RENDER_PLAYERS_PER_TEAM=3`, with captures attached

# Playtest notes (owner, after done)

This is not a gate for done. The agent fills in the run path and controls when the work lands. The owner plays 3v3 Practice and observes:

- whether a leading team can be caught;
- whether carriers are worth protecting and hunting;
- whether the countdown reads clearly;
- whether retreating to stall feels degenerate.
