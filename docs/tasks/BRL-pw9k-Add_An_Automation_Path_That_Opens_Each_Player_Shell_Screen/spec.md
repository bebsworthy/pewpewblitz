# Outcome

A development-only automation flag (or gallery-style harness) opens a named shell screen with seeded scratch profile data and captures it through the existing `--screenshot-dir` hook.

# Context

BRL-51zd could capture only Connecting and first-brawler creation natively: the remaining screens need clicks, and agents here have no OS input automation. The Dashboard needs a saved brawler in a scratch server data dir. BRL-pqzt covers the Dashboard/brawler/weapon previews; this ticket covers reaching each shell destination and overlay.

# Scope

- `automation`-feature-only client flag that, after admission, dispatches the canonical flow actions to reach one named destination (never a second navigation path).
- Seeding of a scratch profile with one saved brawler for Dashboard/details/equipment captures.
- Mid-burst captures (results, accepted equipment) at a fixed frame offset.

# Acceptance criteria

- One command captures each listed screen at 1280x720 and at the 914x411 phone viewport.
- No change to player builds; `just lint` client-feature checks pass.
