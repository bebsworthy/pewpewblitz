# Outcome

Practice AI observes semantic, server-owned object capabilities and a bounded mode-neutral objective view. New assets that reuse an existing semantic capability and new mode plugins that publish an existing objective shape require no edits to the bot observation adapter.

# Scope and decisions

1. Add orthogonal server-side map-object semantic components for hazardous and valuable damageable targets. Resolve those semantics from the authored terminal-reaction identity during map installation; never compare concrete map asset IDs in bot code.
2. Add a server-only `BotObjectiveView` component on the match root with bounded generic projections for elimination scores, a control area, and attack/defend objectives.
3. Each installed Wipeout, Hot Zone, or Heist mode plugin owns initialization and refresh of its objective projection. Bot controller code may query only the shared projection, not `WipeoutState`, `HotZoneState`, `HeistState`, mode descriptors, or concrete mode-definition IDs.
4. Heist safe entities publish a semantic defending-team objective component. Bot object observations use capability flags/team ownership rather than `OilBarrel`, `TreasureChest`, or `HeistSafe` variants.
5. Preserve delayed observation, concealment, deterministic ordering, role assignment, objective behavior, server authority, stable wire identities, and fixed-tick ordering. These projections are process-local and are not registered for replication.
6. Keep this ticket organization-only: no balance, protocol, routing, content-schema, or player-visible behavior change.

# Acceptance criteria

- `src/bots` contains no references to `OIL_BARREL_ASSET`, `TREASURE_CHEST_ASSET`, `DamageableObjectAsset`, `BotModeProjection`, or concrete Wipeout/Hot Zone/Heist ECS state types.
- Map installation projects hazardous/valuable capabilities from terminal reaction identity and Heist safe installation projects the defending-team objective capability.
- Every production mode plugin publishes a valid `BotObjectiveView`; focused tests cover projection refresh and semantic object observation/behavior.
- Existing Practice bot policy, navigation, role, concealment, objective, admission, and authority behavior remains green.
- Server-only and client-only compile checks, canonical checks, formatting, and lint pass.

# Verification

- Focused bot and matchplay tests.
- `cargo check --no-default-features --features server --lib`
- `cargo check --no-default-features --features client --lib`
- `just check`
- `just lint`


# Implemented design

- Terminal-reaction registrations now contribute bounded hazardous/valuable semantics alongside their handler. Authoritative map installation projects those semantics as orthogonal process-local components; direct low-level geometry fixtures that omit the authority runtime omit only these consumer markers.
- Heist safe installation projects `DefendedDamageableObjective { defending_team }`.
- `BotObjectiveView` is a process-local match-root component with generic `Elimination`, `ControlArea`, and `AttackAndDefend` shapes. Each production mode plugin inserts its own view during its existing startup transaction; the view is immutable for a match generation, while live scores/progress/health remain in their owning mode/object state.
- Practice observation and role/behavior policy consume only semantic components and `BotObjectiveView`. The retired central `BotModeProjection` descriptor and concrete asset recognition were removed.
- The performance fixture now explicitly finalizes plugin-populated registries before directly driving `App::update`, matching the production runner lifecycle.

# Verification evidence

- `cargo check --no-default-features --features server --lib` — pass.
- `cargo check --no-default-features --features client --lib` — pass.
- `cargo test --lib bots:: --no-default-features --features server` — 31 passed.
- `cargo test --lib matchplay:: --no-default-features --features server` — 30 passed.
- `cargo test --lib map::runtime:: --no-default-features --features server` — 14 passed.
- `cargo test --lib --no-default-features --features server match_workers_control_manifest_bots_in_every_practice_mode -- --nocapture` — pass.
- `cargo test --lib --no-default-features --features server objective_bots_emit_hot_zone_and_heist_directed_input_in_real_schedules -- --nocapture` — pass.
- `just check` — pass across routing, client, server, network-test, web, and Balance Lab graphs.
- `just lint` — pass after final fixture correction across formatting, web, every Clippy graph, server isolation, V3 renderer, and V8 map cleanup.
- `just test` — routing, 500 client, 455 server, 477 Balance Lab, Balance Lab/network compatibility, and all 95 network scenarios passed. Its final performance leg initially exposed the unfinalized synthetic App fixture; after the fixture correction, `cargo test --locked --no-default-features --features network-test --test performance -- --nocapture` passed all 12 gates.
- Static acceptance scan: `src/bots` has no references to concrete barrel/chest asset IDs, `DamageableObjectAsset`, `BotModeProjection`, or concrete mode ECS state types.

# Learn-from-errors review

- Mistake: the first implementation assumed every direct `World`/`App` fixture had run plugin finalization because production runners do. One performance fixture drove `App::update` directly and therefore lacked the sealed registry.
- Cause: the new registry is deliberately sealed in `Plugin::finish`, but the fixture lifecycle contract was implicit.
- Prevention: fixtures that exercise plugin-populated registries must call `App::finish` and `App::cleanup` before Startup/update, or use the repository finalization helper where available. Low-level collider-only fixtures remain allowed to omit the full authority runtime and consequently omit consumer-only semantic markers.
