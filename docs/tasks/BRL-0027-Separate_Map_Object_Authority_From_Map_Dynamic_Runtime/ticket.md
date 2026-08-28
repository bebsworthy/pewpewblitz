---
id: BRL-0027
title: Separate map object authority from map dynamic runtime
status: done
theme: quality
release:
priority: none
created: 2026-08-28T07:52:25Z
modified: 2026-08-28T09:57:29Z
closed: 2026-08-28T09:57:29Z
revision: 90350af07c2edcf6
blocks: []
related: [BRL-0024]
---

# Description

Refactor `src/map/runtime.rs` so damageable world-object combat authority, map installation/collider materialization, and replicated map-dynamic destruction/recovery have explicit owners. Preserve fixed-tick ordering, server authority, map behavior, collision, recovery, telemetry, and public API paths.
