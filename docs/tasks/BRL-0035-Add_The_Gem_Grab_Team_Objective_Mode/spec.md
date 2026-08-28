# Outcome

Players can queue or practice a complete Gem Grab match that converts combat advantage into collecting, carrying, protecting, dropping, and recovering a contested resource.

# Scope

- Add a focused Gem Grab mode plugin, stable rules, replicated public state, HUD, cues, results, restart, recovery, and routed admission compatibility.
- Add typed map anchors for the gem source and any required mode presentation, plus one dedicated 3v3 map.
- Spawn gems on deterministic bounded deadlines up to an authored live-count ceiling.
- Live fighters collect gems through server-owned overlap; carried count is public and bounded.
- Defeat drops carried gems at validated positions; disconnect/restart/teardown have explicit outcomes.
- Reaching the target begins a visible hold countdown. Falling below the target cancels or resets it according to the authored rule.
- Timeout and simultaneous terminal conditions resolve deterministically.
- Practice bots receive focused collect, protect-carrier, intercept-carrier, and recover-drop goals through ordinary input.

# Boundaries

- No generic inventory, currency, progression, loot rarity, random drops, executable map scripting, or additional mode framework.
- Gems exist only inside the active match and never enter player profiles.

# Acceptance criteria

- Collection, drop, pickup, threshold, countdown, cancellation, timeout, draw, results, restart, late join, and recovery rules pass focused and routed tests.
- The dedicated map validates and provides fair approach, retreat, carrier protection, and comeback paths.
- HUD/world feedback makes source cadence, dropped gems, carrier counts, threshold, and countdown understandable in 3v3.
- Capacity, bot behavior, native playtest, documentation, feedback, and learning gates pass.
