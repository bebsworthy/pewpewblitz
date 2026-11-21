# Context

BRL-wrer ships only `shard-rush-3v3` on Prism Quarry. The rules are team-size independent (team totals, hold countdown), but target count, spawn cadence, ceiling, and map size were tuned for 3v3.

# Scope

- After owner playtests of 3v3, decide which other sizes are worth offering.
- For each chosen size: a game-type row with tuned `shards_to_win`, a compatible map (Prism Quarry or a smaller recipe), admission and lobby catalog tests, and a Practice e2e run.

# Acceptance

- Owner decision recorded; each added game type passes `just practice-e2e <type>` and the catalog tests.
