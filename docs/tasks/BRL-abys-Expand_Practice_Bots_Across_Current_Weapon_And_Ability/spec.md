# Outcome

Start Practice creates a deterministic varied opponent roster that exercises the current Pulse, Scatter, Arc, and Blade weapon bases plus the existing ultimate families through the same saved-brawler resolver and ordinary authoritative input path used by players.

# Scope

- Define a bounded code-owned set of legal bot saved-brawler recipes covering all four current weapon bases and the existing Dash, Sentry, Self Cloak, Reveal Scan, and Concealment Field ultimates.
- Assign recipes deterministically by stable bot identity, team composition, mode, and seed without adapting to hidden player information.
- Extend capability execution for spread fire, lobbed landing decisions, melee approach/retreat, Sentry placement, concealment use, reveal targeting, and existing objective behavior.
- Derive range, cadence, ammo, field geometry, charge, and targeting from resolved loadouts rather than branching on display names.
- Preserve delayed perception, concealment fairness, bounded navigation, deterministic traces, objective priorities, and ordinary FighterInput validation.
- Future feature tickets own bot support for their new content; this ticket covers the current pre-elemental catalog.

# Boundaries

- No difficulty selector, matchmaking substitution, learned policy, bot-only authority, bot protocol, or external bot process.
- No perfect projectile knowledge or hidden-state counter selection.
- Each added behavior must remain readable and fail closed to neutral input when unsupported or invalid.

# Acceptance criteria

- Practice rosters demonstrably include every current weapon base and ultimate family across a bounded deterministic scenario set.
- Bots use preferred range, delivery geometry, ammo, charge, concealment, fields, and mode objectives coherently without bypassing gameplay systems.
- Pure policy traces, lifecycle/restart, 1v1/2v2/3v3 routed scenarios, performance ceilings, and native matchup playtests pass.
- Documentation names the supported bot content matrix and explicitly defers future feature support.
