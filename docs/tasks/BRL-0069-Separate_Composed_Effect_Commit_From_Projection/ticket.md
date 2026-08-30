---
id: BRL-0069
title: Separate Composed Effect Commit From Projection
status: doing
theme:
release:
priority: none
created: 2026-08-30T14:40:42Z
modified: 2026-08-30T14:45:06Z
closed:
revision: 1363df6afc0ee53e
blocks: [BRL-0061]
related: []
---

# Description

Refactor the authoritative composed-payload application loop into explicit per-effect plan, ordered commit, and committed-outcome projection helpers while preserving batch atomicity, deterministic event/cue/telemetry order, and all current gameplay semantics.
