## Outcome

Make application composition—not protocol or session/transport boundaries—own Brawler gameplay and shared authored content, with no gameplay, protocol, schedule, routed-worker, or feature-isolation behavior change.

## Scope and decisions

1. Add a headless-safe `GameplayContentPlugin` in `content.rs`. It initializes the weapon, combat-condition, build, weapon-part, and map catalog resources and publishes `GameplayContentFingerprint` at Startup after catalog initialization.
2. Keep bot catalog validation in the shared fingerprint envelope and bot runtime installation in the server gameplay composition. Keep audio and VFX catalogs in their existing client presentation plugins because they are client-only assets and presentation policy, not shared/headless gameplay resources.
3. Make `ProtocolPlugin` wire-only: messages, channels, replicated components, interpolation, input registration, and `ProtocolFingerprint`. It must not initialize catalog resources, content fingerprints, gameplay systems, or presentation systems.
4. Add `ServerAuthoritativeGameplayPlugin` as the server gameplay composition root for authoritative map, movement, combat, concealment, abilities, match lifecycle, and Practice bot plugins. Keep configured game-mode installation at the server application builder because it reads validated per-process configuration.
5. Add `ClientReplicatedGameplayPlugin` as the client gameplay composition root for replicated combat, map convergence/reconstruction, queue, and profile plugins. These plugins leave `ClientNetworkPlugin`.
6. Keep `RoutedWorkerPlugin` outside `ServerNetworkPlugin` and install it explicitly at the authoritative application/process composition root. The minimum lobby-worker builder explicitly installs `GameplayContentPlugin`, `ProtocolPlugin`, `LobbyPlugin`, and `RoutedWorkerPlugin`, while remaining free of authoritative gameplay plugins.
7. Preserve the existing fixed-tick sets, deferred-command boundaries, startup ordering, stable public wire types, protocol compatibility handshake, and client/server feature gates. This is an ownership move, not a schedule redesign.

## Acceptance criteria

- A protocol-only test proves wire registration and `ProtocolFingerprint` work without any gameplay catalog resource, content fingerprint, gameplay plugin, or presentation plugin.
- A content-only test proves the complete shared catalog set and `GameplayContentFingerprint` are installed without protocol registration.
- Session/transport composition tests prove `ClientNetworkPlugin` and `ServerNetworkPlugin` do not install combat, map, ability, profile, presentation, or routed-worker plugins.
- Full client and authoritative-server builders install their required content/gameplay/session/process plugins exactly once.
- The lobby-worker builder retains protocol/content compatibility identity while remaining free of authoritative map, movement, combat, ability, match, and client presentation plugins.
- Existing direct, match-worker, lobby-worker, routed, network, performance, and headless role behavior remains unchanged.
- Public protocol/wire contracts and gameplay balance are unchanged.
- Server-only compilation remains free of rendering, windowing, audio, device input, and client assets.

## Verification

- Add focused plugin-composition tests in the owning protocol, content, client, server, and admission modules.
- Run `cargo fmt --all -- --check`, `just check`, `just lint`, `just test`, `just ci`, and `git diff --check`.
- Record role-specific check and routed/practice evidence produced by the canonical commands.
- No native or subjective playtest is required because the change only relocates plugin installation and must preserve behavior exactly.

## Closeout

Record the implementation state, verification results, any regression and prevention lesson, update BRL-0061 Stage 3 progress, move BRL-0064 to `done` only when every criterion passes, and run `ticket sync`.

## Implementation and closeout — 2026-08-30

- Added headless-safe `GameplayContentPlugin` as the sole shared owner of weapon, combat-condition, build, weapon-part, and map catalog resources plus `GameplayContentFingerprint`.
- Reduced `ProtocolPlugin` to input/message/channel/component/interpolation registration and `ProtocolFingerprint`; protocol-only composition now proves no gameplay foundation, catalog, content fingerprint, or presentation dependency is installed.
- Added explicit `ClientReplicatedGameplayPlugin` and `ServerAuthoritativeGameplayPlugin` composition roots. Client combat/map/queue/profile selection left `ClientNetworkPlugin`; authoritative map/movement/combat/concealment/ability/match/bot selection left `ServerNetworkPlugin`.
- Moved routed-worker selection to the authoritative application composition beside the network and gameplay roots. The minimum lobby worker explicitly installs shared content, protocol, lobby authority, and routed transport while remaining free of match gameplay.
- Kept audio and VFX catalog ownership in their client-only presentation plugins. Shared content remains headless-safe and server feature isolation remains intact.
- Preserved the former server receive transaction as named `NetworkLifecycle -> GameplayCleanup -> Flush` sets. Pending ability cleanup is installed by the gameplay root, after hello/session mutation and before the same deferred-command flush.
- Updated manual network/performance fixtures to select the explicit roots and documented the resulting dependency direction in `docs/08-network-architecture.md`.
- Public wire shapes, protocol registration order, compatibility versions, gameplay content versions, authored balance, fixed-tick phases, and routed role behavior are unchanged.

## Verification evidence

- Focused client/server/content/protocol/composition, production startup, routing identity, match-worker, lobby-worker, and receive-order tests pass.
- `cargo fmt --all -- --check`: pass.
- `git diff --check`: pass.
- `just check`: pass for routing, client, server, network-test, Balance Lab web, and Balance Lab server roles.
- `just lint`: pass for all Clippy roles, dedicated-server feature isolation, V3 renderer guard, and V8 map cleanup guard.
- `just test`: pass — 491 client, 432 server, 454 Balance Lab, 1 revised-catalog replication, 94 network, 12 performance, plus routing/process suites.
- `just ci`: pass — repeated static/deterministic gates, routed product 1v1/2v2/3v3, and Wipeout/Hot Zone/Heist Practice 1v1/2v2/3v3 all reached Active.
- Native or subjective playtesting was not required because this change only relocates composition ownership and the complete behavior/evidence matrix remained unchanged.

## Learn-from-errors review

- First attempt ordered the extracted cleanup system with `.before(ApplyDeferred)`. Bevy rejected schedule initialization because Update contains multiple `ApplyDeferred` instances and system-type ordering was ambiguous.
- Cause: the old tuple chain hid an instance-specific deferred boundary that could not be referenced safely once the gameplay-owned system moved to another registration call.
- Correction: introduced three narrow named receive sets and a schedule test, retaining the exact network/session mutation, gameplay cleanup, and flush sequence without ordering against an ambiguous system type.
- Prevention: when extracting a system across plugin ownership while preserving a deferred boundary, name and chain the phase sets at the composition point; do not order against `ApplyDeferred` by type in a schedule with multiple instances.
- A protocol composition test also crossed the repository line threshold after adding negative assertions. Extracting one focused assertion helper kept the contract readable without a new Clippy suppression.
