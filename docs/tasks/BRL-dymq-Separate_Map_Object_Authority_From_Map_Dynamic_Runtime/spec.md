# Context

`src/map/runtime.rs` is 1,943 physical lines (about 1,887 NLOC). Its module comment claims map installation and whole-cell destruction, but the file also owns the complete damageable-world-object transaction: pending damage ordering, health mutation, terminal reactions, chained explosions, combatant damage, cues, pickups, telemetry, and safety caps. `process_world_target_damage` is about 380 NLOC/CCN 27 and `apply_explosion_to_combatants` is about 197 NLOC. Installation/collider materialization and dynamic replication/recovery change for different reasons from combat/object authority.

# Target ownership

Keep a small server-only map runtime composition point that registers resources, sets, systems, and explicit ordering.

Create focused owners, using names consistent with the existing `map::objects` and `map::pickups` boundaries:

- world-object authority: pending target damage ordering, damage application, life-state transitions, terminal outcomes, explosion chaining/occlusion, combatant effects, cues, pickup terminal reactions, and world-object telemetry;
- map installation: resolved map/root/member materialization plus static and dynamic collider spawning;
- map dynamics: whole-cell destruction, placement transitions, restart restoration, dynamic generation, mutation/reset outbox, recovery admission/snapshots, publishing, and map-dynamic telemetry;
- focused tests beside those owners, with schedule/integration tests at the composition boundary.

Shared stable object types remain in `objects.rs`; pickup lifecycle remains in `pickups.rs`. Avoid duplicating those type owners.

# Function-level improvements

- Replace `process_world_target_damage` with an ordered coordinator over named stages: collect/sort bounded requests, validate live target, apply damage, determine terminal behavior, enqueue secondary reactions, commit facts/cues/telemetry, and enforce caps.
- Split explosion handling into target selection/occlusion, combatant effect planning, and authoritative commit helpers. Preserve deterministic order and the environment attack source.
- Keep `install_resolved_map` as an atomic installation transaction; extract member/collider materialization helpers without allowing a partially installed map after validation failure.
- Decompose destruction and restoration into selection/planning/commit helpers where it makes generation, collider replacement, and telemetry invariants explicit.
- Keep recovery admission and response publishing distinct from mutation application.
- Remove broad imports and suppressions only when the owning functions no longer require them; do not move giant functions unchanged.

# Authority and ordering constraints

- The server remains the sole owner of health mutation, terminal outcomes, explosions, map destruction, pickups, colliders, recovery responses, and telemetry.
- Preserve `MapRuntimeSet::ApplyDestruction` and `Publish`, their relationships to ability outcome observation, combat damage, match mode rules, restart/reset, pickups, cue publication, and deferred commands.
- Preserve all bounded caps, deterministic sort keys, no-op counting, one-terminal-reaction behavior, chain-reaction limits, collision layers, occlusion rules, placement identities, generation semantics, and recovery rate limits.
- Preserve protocol types/channels, serialization, public `crate::map` exports, snapshots, and client reconstruction behavior.
- Preserve headless/server feature isolation; no presentation or client asset dependency may enter the authority modules.

# Acceptance criteria

- World-object combat authority, map installation/collider materialization, and map-dynamic destruction/recovery are separate focused modules with one explicit schedule composition point.
- `process_world_target_damage` is a readable phase coordinator rather than a 380-line mutation transaction, and each stage is covered by focused tests.
- Explosion target planning, occlusion, combatant damage, chained terminals, cues, pickups, and telemetry retain deterministic behavior and safety caps.
- Map installation remains atomic and produces the same root/member/collider ECS state for every built-in resolved map.
- Destruction, restart restoration, dynamic generations, mutation/reset publication, and recovery snapshots preserve existing observable results and ordering.
- Existing public paths, protocol/wire shapes, map fingerprints, identities, collision layers, and client convergence remain unchanged.
- Schedule tests demonstrate the required fixed-tick and publication ordering across combat, abilities, map dynamics, pickups, match rules, and restart.
- Existing barrel, barrier, chest, occlusion, combatant-damage, brush destruction, restart, and recovery tests remain green under their focused owners.
- Focused map tests plus `just fmt`, `just check`, `just lint`, and `just test` pass for server and client feature graphs.
- Routed/headless tests confirm authoritative destruction and recovery convergence without presentation dependencies.
- Repowise health is rerun and remaining findings are dispositioned by ownership; no numeric score or line target is required.
- Verification evidence, learn-from-errors review, and conflict-free `ticket sync` are recorded before completion.

# Non-goals

- No gameplay balance, damage, explosion, pickup, map content, collision, recovery, or presentation change.
- No protocol/schema evolution.
- No general combat pipeline or event-bus abstraction.

# Implementation evidence (2026-08-28)

- Replaced `src/map/runtime.rs` with a private `map::runtime` module family: `mod.rs` is the explicit schedule/resource facade, `object_authority.rs` owns world-object damage and terminal reactions, `installation.rs` owns validated root/member/collider materialization, `dynamics.rs` owns destruction/restoration/recovery/publication, and `tests.rs` owns focused composition and behavior coverage.
- Preserved the public `crate::map` exports and the fixed/update schedule relationships around ability outcome observation, combat damage, map publication, match rules, cue publication, restart, recovery, and tick finalization.
- Changed `process_world_target_damage` into admission plus ordered batch coordination. The item transaction remains deliberately atomic because health mutation, terminal commitment, chained requests, event identity allocation, and cue/fact publication must share one bounded order; its target lookup, transition commit, explosion selection/occlusion, and combatant planning/commit are named helpers rather than hidden schedule stages.
- Split explosion combatant selection from authoritative commit while retaining deterministic distance/identity order, occlusion, lineage credit, target caps, environment attribution, defeat cleanup, cues, and outcome facts.
- Added preflight validation for every dynamic asset reference and player-only rectangle before replacing an installed map. `invalid_installation_preserves_the_existing_map_root` proves a validation error cannot leave partial or torn-down map state.

# Verification evidence (2026-08-28)

- `cargo check --no-default-features --features server` passed.
- `cargo clippy --no-default-features --features server --all-targets -- -D warnings` passed.
- Eight focused `map::runtime::tests` passed, covering recovery admission, brush destruction/restart, barrier replacement/restart, chained barrels, occlusion, combatant environmental damage, chest/pickup behavior, and atomic installation failure.
- `just fmt`, `just check`, `just lint`, and `just test` passed. The full run included 428 client tests, 337 server tests, 353 Balance Lab tests, 88 serialized headless network tests, and 12 performance tests.
- Headless network scenarios `map_cover_destruction_converges_for_connected_and_late_joining_clients`, `barrel_partial_health_and_terminal_absence_converge_for_two_clients`, `feature_yard_barrier_replacement_converges_for_connected_and_late_joining_clients`, and `late_join_and_map_root_replacement_converge_from_durable_state` passed, confirming authoritative mutation/recovery convergence without presentation dependencies.
- Repowise health for `src/map/runtime` reports 8.29/10 average, 10.0/10 for the composition facade, and no alert files or static performance findings. Remaining large/complex markers are dispositioned as bounded atomic commit paths (`apply_world_damage_batch`, combatant damage commit, installation, destruction/restoration); their different ownership is now isolated and tested, and further splitting would require a behavior-changing transaction abstraction outside this ticket.
- Scoped `git diff --check` for the runtime and ticket paths passed. The repository-wide check still reports a pre-existing trailing blank line in the user-owned Repowise-generated `AGENTS.md` section, which this ticket does not modify.

# Learn-from-errors review

- Moving a flat module into a directory changes the meaning of `super::`; the first compile exposed references that now resolved to `map::runtime` instead of `map`. Future mechanical extractions should map relative-path depth before moving bodies and then prefer explicit owner imports.
- Extracted sibling tests had implicitly depended on root-module imports and visibility. Giving tests explicit imports and exposing only the necessary owner helpers with `pub(super)` made those dependencies reviewable.
- The ownership review exposed that installation tore down the current root before all fallible catalog lookups. Preflight validation plus a regression test now protects atomic replacement.
- Strict Clippy caught an unnecessary mutable binding introduced during phase extraction; running role-specific lint immediately after each mechanical move keeps cleanup local.
