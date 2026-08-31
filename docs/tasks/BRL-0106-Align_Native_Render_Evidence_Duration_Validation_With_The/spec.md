# Scope

- Reconcile `RenderMeasurementConfig` duration validation with the canonical report validator's minimum sample count and render cadence assumptions.
- Prefer an early actionable configuration error or a duration-aware canonical launcher default over a late completed-run failure.
- Preserve the locked 1,200-sample performance evidence threshold unless a separate measured architecture decision changes it.
- Add focused configuration and script tests and update the native evidence runbook.

# Acceptance criteria

- Every duration accepted by the canonical `v3-render-evidence` path can satisfy the sample gate under its intended cadence, or validation clearly explains the required duration.
- The default command passes its sample-count gate and explicit invalid short windows fail before launching the routed topology.
- Existing report schema and historical reports remain readable.
