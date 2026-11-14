---
id: BRL-dymq
was: BRL-0027

title: Separate map object authority from map dynamic runtime

status: done
closed: 2026-08-28T09:57:29Z
status_changed: 2026-08-28T09:57:29Z

theme: quality

release:

created: 2026-08-28T07:52:25Z

blocks: []

related: [BRL-r8h8]

parent:

rank:
---

# Description

Refactor `src/map/runtime.rs` so damageable world-object combat authority, map installation/collider materialization, and replicated map-dynamic destruction/recovery have explicit owners. Preserve fixed-tick ordering, server authority, map behavior, collision, recovery, telemetry, and public API paths.
