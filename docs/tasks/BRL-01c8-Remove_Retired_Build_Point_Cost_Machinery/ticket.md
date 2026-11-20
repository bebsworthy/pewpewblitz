---
id: BRL-01c8
was: BRL-0043

title: Remove retired build point-cost machinery

status: done
closed: 2026-11-20T02:55:38Z
status_changed: 2026-11-20T02:55:38Z

theme: quality

release:

created: 2026-08-28T13:27:16Z

blocks: []

related: [BRL-1t80, BRL-4ms7, BRL-5pxn, BRL-egn6, BRL-k66v]

parent:

rank:
---

# Description

Remove the inert build point-cost machinery left behind after roster character picks (BRL-egn6) became the sole product loadout: the 12-point budget constant, every authored `point_cost` and the weapon cost table, `total_points` on the resolved loadout and match telemetry, the unreachable over-budget queue rejection, and the matching evidence key, copy, tests and docs. Character resolution (`builds::resolve_loadout`), stable IDs, slot/conflict rules, parameter bounds, byte ceilings, server authority and gameplay are unchanged.
