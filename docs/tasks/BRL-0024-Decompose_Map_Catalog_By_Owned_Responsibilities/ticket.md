---
id: BRL-0024
title: Decompose map catalog by owned responsibilities
status: todo
theme: quality
release:
priority: none
created: 2026-08-28T07:20:42Z
modified: 2026-08-28T08:37:49Z
closed:
revision: bdee53c2b44bc7b3
blocks: []
related: [BRL-0027]
---

# Description

Refactor `src/map/catalog.rs` into cohesive map modules so authored catalog loading, shared replicated state, canonical recipe resolution, geometry/topology rules, and tests have clear owners. Preserve all accepted map behavior, public API paths, serialized wire shapes, fingerprints, authority boundaries, and headless/client feature isolation while reducing the current oversized-module and brain-method risks.
