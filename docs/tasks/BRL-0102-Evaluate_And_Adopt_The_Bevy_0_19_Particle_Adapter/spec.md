# Decision gate

Use local dependency metadata/examples and current primary documentation as needed to verify exact Bevy 0.19 compatibility, feature graph, platform support, lifecycle semantics, deterministic limits, shader/runtime cost, and maintenance status. Prototype only the smallest adapter needed for sparks/embers/frost fragments/smoke.

# Adoption requirements

- Client-only dependency and adapter; zero server feature-graph impact.
- Recipes reference bounded particle profiles through an optional layer, while CriticalGeometry remains mesh-based.
- Explicit global/per-effect capacity, spawn/lifetime cleanup, reduced/primitive fallback, asset readiness, diagnostics, and evidence metadata.
- Deterministic CPU-side profile/seed selection is required; exact cross-GPU particle positions are not claimed unless measured and proven.

# Acceptance criteria

- A recorded adopt/defer decision includes compatibility evidence, native Metal measurements, visual value, failure behavior, and maintenance cost.
- If adopted, Cold and Fire receive a restrained decorative upgrade with no loss of readability or bounds.
- If deferred, downstream recipes retain a complete mesh-only path and the decision does not block BRL-0103.

# Verification

Role checks, lifecycle/capacity tests, reduced/primitive tests, native Metal performance and paired captures, plus leak/resource high-water evidence.
