# Scope

- Implement the frozen typed catalog tables, recipes, reusable layers, semantic mappings, validation bounds, fallback graph, readability classes, deterministic counter-hash sampling, and phase-one timelines.
- Materialize up to eight ordered retained mesh/material layers per recipe under one bounded visual root.
- Migrate all current `VfxRequest`/`VfxProfile` transient effects to recipes with visual-equivalence fixtures; keep gameplay producers presentation-agnostic.
- Apply normal/reduced/primitive degradation, total/per-recipe caps, deterministic oldest-first eviction, authoritative deadline support, and leak-free cleanup.
- Make the same runtime directly consumable by VfxLab.

# Acceptance criteria

- Existing transient cues resolve through recipes with no gameplay/protocol change and no unintended presentation redesign.
- Invalid catalogs fail with precise diagnostics for every frozen bound and reference rule.
- Repeated spawn/evict/cleanup cycles stay within root and retained-resource budgets.
- Deterministic inputs select identical authored variants; layer order is stable.
- Client/server role checks and native equivalence evidence pass.

# Verification

Focused catalog/resolver/timeline/ECS tests, client/server checks and linting, production presentation guards, VfxLab replay evidence, and representative native before/after captures.
