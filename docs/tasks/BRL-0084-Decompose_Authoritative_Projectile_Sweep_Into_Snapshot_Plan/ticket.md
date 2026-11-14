---
id: BRL-0084

title: Decompose authoritative projectile sweep into snapshot plan commit

status: done
closed: 2026-08-31T04:21:45Z
status_changed: 2026-08-31T04:21:45Z

theme:

release:

created: 2026-08-31T03:20:59Z

blocks: []

related: [BRL-0070]

parent:

rank:
---

# Description

Decompose the authoritative composed-projectile sweep into explicit immutable world/projectile snapshots, deterministic per-projectile plans, and a sequential commit phase. Preserve exact server-owned movement, collision, sticky/splash behavior, payload/outcome ordering, telemetry, replication, and fixed-schedule boundaries while removing the current monolithic multi-responsibility system.
