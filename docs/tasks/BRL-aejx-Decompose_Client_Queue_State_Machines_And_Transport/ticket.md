---
id: BRL-aejx
was: BRL-0028

title: Decompose client queue state machines and transport orchestration

status: done
closed: 2026-08-28T10:31:50Z
status_changed: 2026-08-28T10:31:50Z

theme: quality

release:

created: 2026-08-28T07:52:25Z

blocks: []

related: [BRL-cb3x, BRL-v6yb]

parent:

rank:
---

# Description

Refactor `src/client/queue.rs` so Practice start, matchmaking queue, match-loading cancellation/readiness, transport observation, and headless automation have focused state-machine owners. Preserve request correlation, freshness, retry/timeout, product-flow observations, public paths, and wire behavior.
