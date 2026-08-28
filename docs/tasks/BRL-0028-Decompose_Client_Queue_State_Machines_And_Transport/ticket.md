---
id: BRL-0028
title: Decompose client queue state machines and transport orchestration
status: done
theme: quality
release:
priority: none
created: 2026-08-28T07:52:25Z
modified: 2026-08-28T10:31:50Z
closed: 2026-08-28T10:31:50Z
revision: 2b94e01a1f9c2a8e
blocks: []
related: [BRL-0022, BRL-0042]
---

# Description

Refactor `src/client/queue.rs` so Practice start, matchmaking queue, match-loading cancellation/readiness, transport observation, and headless automation have focused state-machine owners. Preserve request correlation, freshness, retry/timeout, product-flow observations, public paths, and wire behavior.
