# Scope

Give one presentation owner responsibility for bounded transient effect capacity and deterministic order allocation while keeping cue-family visuals local.

# Acceptance

- Capacity reservation/oldest eviction and sequence allocation have one implementation.
- Cross-family order remains deterministic and MAX_EFFECTS is respected after deferred commands settle.
- A small descriptor may share the timed effect bundle.
- Each cue family retains validation, geometry, duration, material, and labels.
- Tests cover overflow, eviction, family order, reduced-effects mode, and cleanup.
- Client visual and Clippy checks pass.

# Constraints

Do not turn cue presentation into an arbitrary effect graph.
