---
id: BRL-r8h8
was: BRL-0024

title: Decompose map catalog by owned responsibilities

status: done
closed: 2026-08-29T18:19:50Z
status_changed: 2026-08-29T18:19:50Z

theme: quality

release:

created: 2026-08-28T07:20:42Z

blocks: []

related: [BRL-dymq]

parent:

rank:
---

# Description

Refactor `src/map/catalog.rs` into cohesive map modules so authored catalog loading, shared replicated state, canonical recipe resolution, geometry/topology rules, and tests have clear owners. Preserve all accepted map behavior, public API paths, serialized wire shapes, fingerprints, authority boundaries, and headless/client feature isolation while reducing the current oversized-module and brain-method risks.
