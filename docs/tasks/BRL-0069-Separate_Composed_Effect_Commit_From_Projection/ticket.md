---
id: BRL-0069

title: Separate Composed Effect Commit From Projection

status: done
closed: 2026-08-30T18:20:38Z
status_changed: 2026-08-30T18:20:38Z

theme:

release:

created: 2026-08-30T14:40:42Z

blocks: [BRL-0061]

related: []

parent:

rank:
---

# Description

Refactor the authoritative composed-payload application loop into explicit per-effect plan, ordered commit, and committed-outcome projection helpers while preserving batch atomicity, deterministic event/cue/telemetry order, and all current gameplay semantics.
