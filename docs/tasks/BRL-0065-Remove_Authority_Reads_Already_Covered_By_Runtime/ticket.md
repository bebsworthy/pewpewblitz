---
id: BRL-0065

title: Remove Authority Reads Already Covered By Runtime Projections

status: done
closed: 2026-08-30T13:00:28Z
status_changed: 2026-08-30T13:00:28Z

theme:

release:

created: 2026-08-30T12:39:33Z

blocks: [BRL-0061]

related: []

parent:

rank:
---

# Description

Remove authoritative ECS dependencies on the replicated `ResolvedMatchLoadout` aggregate wherever Stage 1 already installs the exact immutable runtime projection, preserving aggregate use only at replication, control-plane, evidence, and reconciliation boundaries.
