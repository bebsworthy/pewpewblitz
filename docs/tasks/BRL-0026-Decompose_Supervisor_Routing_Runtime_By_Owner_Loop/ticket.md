---
id: BRL-0026
title: Decompose supervisor routing runtime by owner-loop responsibility
status: done
theme: quality
release:
priority: none
created: 2026-08-28T07:52:25Z
modified: 2026-08-28T09:35:10Z
closed: 2026-08-28T09:35:10Z
revision: b7d79829f978a636
blocks: []
related: [BRL-0031]
---

# Description

Refactor `packages/brawler-routing/src/runtime.rs` so the Mio supervisor loop remains a clear public facade while allocation transactions, worker/process lifecycle, UDP/IPC routing, queue/backpressure handling, and runtime reporting have focused owners. Preserve all public APIs, byte-level contracts, ordering, capacity, isolation, and shutdown behavior.
