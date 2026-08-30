# BRL-0075 specification

## Outcome

World-object terminal behavior resolves to a stable reaction identity owned by shared map content, while server-only reaction plugins register handlers through a bounded App extension. Startup finalization validates deterministic, capacity-bounded coverage of every authored terminal reaction. Runtime handlers receive an explicit terminal transaction context instead of arbitrary `&mut World`; existing explosion, chain damage, pickup spawn, facts, cues, telemetry, transitions, and despawn behavior remain unchanged.

## Scope

1. Introduce a stable nonzero `TerminalReactionId` in the shared map catalog API with current explosion and restoration-pickup identities. `MapObjectTerminalBehavior` provides canonical reaction ID and outcome projections; remove the runtime-local variant-to-ID function.
2. Add a focused server runtime registry module with bounded registration metadata, builder, sealed deterministic registry resource, App extension, and finalization plugin.
3. Reject zero IDs, duplicate registrations, capacity overflow, and any authored terminal reaction lacking a handler. Extra registered handlers are allowed so a plugin may be installed before authored content adopts it.
4. Make explosion and restoration-pickup plugins own their registrations. Keep the runtime composition root responsible for visibly installing the registry and built-in reaction plugins.
5. Replace the handler signature’s unrestricted `&mut World` with a bounded `TerminalReactionContext`. Keep raw World access private to transaction-context methods and the exclusive batch coordinator.
6. Preserve the current authoritative transaction ordering: validate/capacity check, reserve identities, commit health/facts/cues/life state/transition, execute the resolved terminal reaction, then publish consolidated map transitions.
7. Add tests for zero/duplicate/capacity/missing coverage, deterministic ordering, plugin build-order independence, and a synthetic reaction registration that executes without modifying central dispatch.
8. Retain focused barrel/chest chain, pickup, cue, telemetry, and capacity tests as regression evidence.
9. Update durable map/object architecture documentation where needed.

## Constraints

- No map catalog wire/schema/fingerprint change is needed for stable projections over the existing serialized terminal enum.
- Do not add dynamic executable loading, typeless payload maps, trait-object service locators, or one plugin per authored object instance.
- New semantic reaction families may still require an intentional authored schema/type addition; this ticket removes unrelated runtime registration fanout.
- Preserve fixed-tick sets, deferred-command boundaries, stable combat/map identities, server authority, and client feature isolation.
- Do not move environment combatant damage ownership in this phase; BRL-0070 Stage 4 owns that separate authority change.
- Preserve unrelated worktree changes.

## Verification

- Focused terminal registry unit/App tests.
- Existing map runtime barrel/chest terminal reaction tests.
- Server and client role checks.
- `cargo fmt --all`, `git diff --check`, and `just check`.

No native evidence is required because runtime behavior and presentation facts remain unchanged.

## Acceptance criteria

- [x] Shared authored terminal behavior resolves to stable reaction IDs without runtime-local mapping.
- [x] Registry population is additive through plugins/App extension and sealed deterministically.
- [x] Invalid, duplicate, excessive, or missing registrations fail closed.
- [x] A synthetic registered reaction executes without a new central dispatch branch.
- [x] Reaction handlers cannot directly access arbitrary `World` state.
- [x] Explosion/pickup behavior, bounded ordering, facts, cues, telemetry, and transitions remain unchanged.
- [x] Client/server role isolation and fixed schedule composition remain intact.
- [x] Focused tests and `just check` pass.
- [x] Verification and learning are recorded before closeout.


## Implementation and verification record — 2026-08-30

Implemented stable `TerminalReactionId` projections on shared authored terminal behavior without changing serialized catalog material. Server runtime now owns a focused crate-visible registry module: plugins register bounded handler metadata through an App extension, the builder rejects duplicates/capacity overflow, and plugin finalization seals registrations in stable ID order after proving every authored reaction has a handler. Extra registrations remain legal for additive plugin installation.

Explosion and restoration-pickup plugins now own their registrations and remain visibly installed at the map runtime composition root. Registered handlers receive only `WorldObjectTerminalPlan` and `TerminalReactionContext`; raw `World` access is private to context-backed transaction commit functions. Existing preflight, identity reservation, health/fact/cue/life-state/transition ordering and explosion/pickup behavior remain intact. Shared outcome projections also removed repeated client/runtime variant matches.

Verification passed:

- `cargo test --locked --no-default-features --features server --lib terminal_reactions` — 3 passed, covering duplicate/capacity/missing coverage, deterministic order, build-order-independent finalization, and synthetic execution.
- `cargo test --locked --no-default-features --features server --lib map::runtime::tests::` — 8 passed, including barrel chain/restart, chest pickup, occlusion, combatant damage, replacement, recovery, and capacity behavior.
- `cargo test --locked --no-default-features --features server --lib damageable_profiles_reject_invalid_bounds_references_and_incompatible_behavior` — 1 passed, including stable reaction/outcome projection.
- `cargo check --locked --no-default-features --features client --all-targets` and server equivalent — passed warning-free.
- `just check` — routing, client, server, network-test, Balance Lab Rust targets, Balance Lab web tests, and web build passed.
- `just lint` — formatting, all Clippy feature matrices with warnings denied, server feature isolation, V3 renderer, and V8 map cleanup passed.
- `git diff --check` and the raw-World/runtime-local mapping audit passed.

No native evidence was required because facts, cues, values, and presentation behavior are unchanged.

## Learn-from-errors review

- Plugin-built registries do not exist until Bevy plugin finalization. Prevention: registry tests explicitly finalize the App and assert the builder is removed before executing handlers.
- A test-only direct registration helper would have preserved a second composition path. Prevention: runtime fixtures install the same registry and built-in plugins as production, then finalize normally.
- `just check` does not enforce the repository’s Clippy thresholds. The first `just lint` caught a prior embedded-catalog test over 100 lines and an overly wide `u64`-to-`f64` helper. Prevention: run `just lint` for structural phases; split tests by responsibility and keep fractional editor conversion bounded to `u32`, with one narrowly justified exact-frequency cast.
- Stable nonzero IDs are safer when zero cannot be constructed. `TerminalReactionId::new(0)` fails before registration, while the builder owns duplicate and capacity rejection.
