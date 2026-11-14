# Technical specification

## Outcome and scope

Extract only the projected damageable-object health UI concern from `src/client/presentation_3d/mod.rs` into a private `src/client/presentation_3d/object_health.rs` module. The module owns the screen-space health-bar key/index/components, pure health/projection policies, spawn styling, and reconciliation/projection system shared by replicated damageable map objects and Heist safes.

This is an organization-only phase. It must not change rendering, copy, health rules, map/Heist identity, camera behavior, UI layout/style, visibility, schedule order, protocol, authority, or player-visible behavior.

## Exact ownership boundary

Move into `object_health.rs`:

- `OBJECT_HEALTH_BAR_WIDTH`, `OBJECT_HEALTH_BAR_HEIGHT`, and `OBJECT_HEALTH_WORLD_HEIGHT`;
- `DamageableObjectHealthKey`;
- `DamageableObjectHealthUi`, `DamageableObjectHealthFillUi`, and `DamageableObjectHealthUiIndex`;
- `damageable_object_health_fraction`;
- `spawn_damageable_object_health_ui`;
- `projected_object_health_top_left`;
- `project_damageable_object_health_ui`;
- the two existing pure tests for partial-health policy and viewport projection.

Keep in `presentation_3d/mod.rs`:

- `WorldPresentationPlugin`, resource initialization, and all schedule registration;
- `DynamicMapVisual`, dynamic map reconciliation/material updates, and diagnostics use;
- Heist safe mesh/status reconciliation and assets;
- `ArenaCamera`, `ground_position`, foundation assets/cameras, and shared presentation sets;
- all combat, fighter, static map, pickup, and imported-asset presentation.

Expose only `DamageableObjectHealthUiIndex` and `project_damageable_object_health_ui` to the parent with `pub(super)`. All other module items stay private. Child access to parent-private `DynamicMapVisual`, `ArenaCamera`, and `ground_position` must not broaden those types' visibility. Do not add a sub-plugin, new set, messages, events, traits, generic UI framework, or public API.

## Preserved schedule and deferred behavior

- Initialize `DamageableObjectHealthUiIndex` at the existing `WorldPresentationPlugin` composition point.
- Register `project_damageable_object_health_ui` in the same PostUpdate tuple beside fighter overhead projection.
- Preserve exact `.after(TransformSystems::Propagate)`, `.after(CameraUpdateSystems)`, and `.before(UiSystems::Prepare)` relations.
- Do not add `.chain()`, `ApplyDeferred`, or a new schedule boundary.
- Preserve deferred Commands behavior: a missing/repaired bar is spawned hidden and indexed during one pass, then becomes query-visible and projected on a later pass under Bevy's existing deferred-command semantics.

## Preserved map and Heist policies

- Map-object bars exist only when `maximum > 0`, `current > 0`, and `current < maximum`.
- Heist-safe bars are desired whenever `ClientHeistReadiness::Ready` and `maximum > 0`, including full and zero health fractions. Do not reuse the map partial-health predicate for safes.
- Map keys remain `(map_instance_id, generation, placement_id)` and require an exact `DynamicMapVisual` plus `MapPresentationMember` match.
- Heist keys remain `(match_id, anchor_id)`.
- Reconciliation remains deterministic through BTreeMap ordering.
- Stale entries retain `try_despawn` plus index removal; missing indexed entities retain remove-and-respawn repair.
- Mutation remains by stable semantic key, never process-local dynamic-object Entity identity.

## Preserved projection and styling

- Use the single ArenaCamera query and retain missing/multiple-camera behavior.
- Preserve `logical_viewport_size`, `world_to_viewport`, the existing `ground_position` for safes, and world anchor height 52.
- Preserve partial-overlap viewport acceptance without screen clamping.
- Preserve width 76.8, height 11, padding/border radius/overflow, background/fill colors, z-index 119, exact Names, hidden-at-spawn, and Hidden/Inherited transitions.
- Missing projection/camera or offscreen anchors hide existing bars.
- Preserve fill width updates, debug key assertion, independent fill-query failure behavior, and current unclamped fraction arithmetic.

## Tests and verification

Retain the moved pure tests:

1. map-object health bars exist only between full and terminal health;
2. centered projection yields the exact top-left and offscreen projection is rejected.

Existing `presentation_phases_preserve_sync_and_semantic_order`, dynamic-map/Heist presentation tests, and client composition tests must remain green. Source review must confirm unchanged PostUpdate transform/camera/UI relations and deferred-command behavior.

Run:

- `cargo fmt --all -- --check`
- `git diff --check`
- focused `client::presentation_3d` tests
- `cargo check --locked --no-default-features --features client --all-targets`
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings`
- `just check`
- `just lint`
- `just test`

Native evidence is not required if the extraction is literal and all values, UI nodes, ordering, and outcomes remain unchanged.

## Exclusions

Dynamic map visual extraction, Heist safe mesh/status extraction, restoration pickups, HUD/ClientHeistReadiness ownership changes, camera changes, health-bar redesign, new health policies, mode registration, diagnostics changes, schedule redesign, protocol changes, and native polish are excluded.

## Visibility clarification

Keep direct Bevy function-item registration in the composition root. If Rust's `private_interfaces` lint requires the query marker components named in the schedule-facing system signature to share `pub(super)` visibility, expose only those marker structs in addition to the index resource and system; keep their fields, semantic key, policies, helpers, and all other implementation private. Do not introduce an opaque `ScheduleSystem` factory solely to hide marker types.

## Implementation evidence — 2026-08-31

Extracted projected damageable-object and Heist-safe health UI ownership into the private `client::presentation_3d::object_health` module. The parent composition root still initializes the index and directly registers the system in the same PostUpdate tuple with the exact transform-propagation, camera-update, and UI-prepare ordering. Stable semantic keys, map partial-health policy, Heist full/zero-health policy, deterministic reconciliation, deferred Commands behavior, projection arithmetic, styling, copy, and visibility outcomes are unchanged. The two focused policy/projection tests moved with their owner.

An intermediate attempt to hide schedule-facing marker types behind an opaque zero-argument system factory was rejected during review. The final design keeps direct Bevy function-item registration and exposes only the schedule-facing system, index resource, and two query marker types to the parent; all fields, keys, policies, and helpers remain private. This is the reusable lesson for future extractions: preserve composition readability and use the narrowest Rust visibility required by the real Bevy system signature rather than adding indirection solely for lint appeasement.

Independent review found no P0, P1, or P2 issues. Verification passed:

- `cargo fmt --all -- --check`
- `git diff --check`
- 71 focused `client::presentation_3d` tests
- client all-target check and strict all-target Clippy
- `just check`
- `just lint`
- `just test`: 545 client, 493 server, 522 Balance Lab, the exact cross-feature network smoke, all 97 routed network tests, and all 12 performance gates

No native rerun was required because this was a literal organization-only extraction with unchanged values, nodes, ordering, and outcomes. No feedback item remains open.
