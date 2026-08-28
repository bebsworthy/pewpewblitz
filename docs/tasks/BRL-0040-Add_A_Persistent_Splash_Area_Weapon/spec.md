# Outcome

A new Splash weapon base launches or places a bounded persistent combat area. Every live fighter entering or remaining in the area is affected according to its authored Cold, Fire, Poison, damage, or slow payload.

# Scope

- Add one stable Splash weapon base and a typed persistent-area delivery/runtime family.
- Support authored Circle and Rectangle shapes with bounded size, placement/range, duration, pulse interval, maximum active areas per owner, and presentation profile.
- Support exactly five effect choices: Cold contribution, Fire, Poison, immediate periodic damage, and Slow.
- Cold, Fire, and Poison reuse BRL-0003's target-owned rules and resistances; Damage and Slow reuse existing combat outcomes.
- The first content uses an Everyone recipient policy: owner, allies, and enemies are affected when eligible. The server evaluates authoritative occupancy; clients never report entry or pulses.
- Define impact/placement validation, initial-overlap behavior, entry/exit, pulse deadlines, overlapping areas, strongest/refresh rules, attribution, owner defeat/disconnect, respawn, restart, late join, and teardown.
- Replicate stable public area facts for presentation and evidence while keeping effect application server-only.
- Add aim preview, area warning, remaining lifetime, effect identity, audio, HUD/status feedback, Balance Lab controls, and Practice-bot awareness.

# Boundaries

- No healing splash, arbitrary effect list, user-authored scripts, unbounded stacking, moving areas, terrain ignition, projectiles modified in flight, or client prediction.
- A continuous channeled spray is owned by a separate ticket.

# Acceptance criteria

- Every supported shape/effect combination follows the same bounded area lifecycle and exact source attribution.
- Entry after creation, remaining inside, leaving, re-entering, overlap, concealment, defeat, restart, late join, and recovery pass focused and routed tests.
- Maximum active areas and 3v3 overlap stay within fixed-tick and presentation budgets.
- Native playtests confirm clear shape boundaries, friendly-fire risk, effect identity, duration, and counterplay.
