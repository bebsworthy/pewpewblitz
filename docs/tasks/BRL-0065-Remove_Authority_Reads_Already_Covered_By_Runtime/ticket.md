---
id: BRL-0065
title: Remove Authority Reads Already Covered By Runtime Projections
status: done
theme:
release:
priority: high
created: 2026-08-30T12:39:33Z
modified: 2026-08-30T13:00:28Z
closed: 2026-08-30T13:00:28Z
revision: 41e87d2258854829
blocks: [BRL-0061]
related: []
---

# Description

Remove authoritative ECS dependencies on the replicated `ResolvedMatchLoadout` aggregate wherever Stage 1 already installs the exact immutable runtime projection, preserving aggregate use only at replication, control-plane, evidence, and reconciliation boundaries.
