---
id: BRL-0024
title: Decompose map catalog by owned responsibilities
status: done
theme: quality
release:
priority: none
created: 2026-08-28T07:20:42Z
modified: 2026-08-29T18:19:50Z
closed: 2026-08-29T18:19:50Z
revision: 2ff4417397f41a70
blocks: []
related: [BRL-0027]
---

# Description

Refactor `src/map/catalog.rs` into cohesive map modules so authored catalog loading, shared replicated state, canonical recipe resolution, geometry/topology rules, and tests have clear owners. Preserve all accepted map behavior, public API paths, serialized wire shapes, fingerprints, authority boundaries, and headless/client feature isolation while reducing the current oversized-module and brain-method risks.
