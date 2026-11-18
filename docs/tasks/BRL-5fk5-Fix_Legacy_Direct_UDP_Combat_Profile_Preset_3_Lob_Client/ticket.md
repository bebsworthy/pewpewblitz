---
id: BRL-5fk5

title: Fix legacy direct-UDP combat-profile preset 3 (lob) client checkpoint timeout

status: backlog
closed:
status_changed: 2026-11-14T21:47:00Z

theme: quality

release:

created: 2026-11-14T21:47:00Z

blocks: []

related: [BRL-3p70]

parent: BRL-q5c4

rank: 2398886
---

# Description

`./scripts/network-combat-profiles.sh` passes for weapon preset 3 (Arc lob) again: both direct-UDP headless clients reproduce the `active_lob_flight`, `active_slow`, and `active_knockback` checkpoints and write their ready files inside the run timeout.
