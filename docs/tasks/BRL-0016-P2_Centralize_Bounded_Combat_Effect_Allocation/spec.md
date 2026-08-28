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

# As built

The four cue-family systems now validate their own facts and emit one private
`PendingCombatEffect` descriptor containing the already-decided lifetime, authoritative expiry,
mesh, material, transform, and label. They retain all family-specific behavior:

- combat cues own actor feedback, Reveal Scan geometry, authoritative duration, and palette;
- world-object cues own generation validation and damage/explosion visuals;
- pickup cues own identity validation and spawned/collected/expired presentation;
- Heist cues own readiness/target validation and damaged/critical/destroyed presentation.

One `materialize_combat_effects` system is the only capacity and order owner. It sorts the live
bounded population by `(order, entity)`, advances one global local sequence past the largest live
order, repairs any pre-existing over-cap population, evicts the oldest reservation before every new
spawn at capacity, and creates the common timed mesh/material/no-shadow bundle. Newly reserved
entities participate in the same-frame queue, so a cue burst cannot bypass `MAX_EFFECTS` through
deferred commands. Eviction and cleanup use `try_despawn` because both decisions may legally target
the same stale entity in one frame.

The presentation composition explicitly chains combat, world-object, pickup, Heist, and final
materialization. This makes the existing family precedence visible and deterministic without
merging their validation or visual policy into an effect graph.

# Research

- The original implementations in `src/client/presentation_3d/combat.rs` independently counted the
  same query, selected an oldest effect, and maintained four unrelated `Local` sequences.
- The installed Bevy 0.19.1 `bevy_ecs/src/system/commands/mod.rs` confirms `try_despawn` as the
  appropriate deferred command when another same-frame owner may already have removed an entity.
- Existing Brawler message readers/writers and explicit `.chain()` composition provided the
  Bevy-native producer-to-materializer boundary; no new dependency or general effect framework was
  necessary.

The exact dependency and project patterns were available locally, so internet research was not
needed.

# Verification

- Focused effect tests pass for a pre-existing `MAX_EFFECTS + 2` overflow plus two same-frame
  reservations, exact oldest eviction, a settled count of 96, monotonic orders, deterministic
  combat/world-object/pickup/Heist order, reduced-effect lifetime preservation, and authoritative
  cleanup after materialization.
- All 20 focused combat-presentation tests pass.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings`
  passes.
- `cargo test --locked --no-default-features --features client --all-targets` passes: 422 library
  tests and the client binary target, with no failures.
- The canonical routed release-client Practice gate passes in
  `target/brl-0016-render-evidence.txt`: 1,260 gameplay samples, p50 `16.678ms`, p95 `16.972ms`,
  p99 `17.400ms`, maximum `41.520ms`, three frames over `25ms`, effect high-water `3`, terminal
  effect count `0`, `result=pass`, and `first_failure=none`. The match worker later emitted a
  `WorkerExitMismatch` classification despite exiting with code zero; this report is therefore
  visual/performance evidence, not a clean routed-lifecycle claim.
