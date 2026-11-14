---
id: BRL-0w7w
was: BRL-0026

title: Decompose supervisor routing runtime by owner-loop responsibility

status: done
closed: 2026-08-28T09:35:10Z
status_changed: 2026-08-28T09:35:10Z

theme: quality

release:

created: 2026-08-28T07:52:25Z

blocks: []

related: [BRL-xtbw]

parent:

rank:
---

# Description

Refactor `packages/brawler-routing/src/runtime.rs` so the Mio supervisor loop remains a clear public facade while allocation transactions, worker/process lifecycle, UDP/IPC routing, queue/backpressure handling, and runtime reporting have focused owners. Preserve all public APIs, byte-level contracts, ordering, capacity, isolation, and shutdown behavior.
