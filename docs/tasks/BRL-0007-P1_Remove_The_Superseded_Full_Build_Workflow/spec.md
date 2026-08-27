# Scope

Remove the superseded named full-build selection workflow in one focused compatibility-floor change. Remove the unreachable Build Editor and standalone persistence, obsolete direct-session selection path, named full-build presets, and preset-only telemetry/configuration. Convert fixtures to saved brawlers or explicit canonical recipes.

# Acceptance

- Saved brawlers are the only product selection and persistence workflow.
- Active weapon bases/presets, definitions, resolved loadouts, profiles, routed admission, and server authority remain.
- Advance the global compatibility schema/fingerprint and fail stale peers closed.
- Product copy, automation, tests, and docs no longer refer to the removed workflow.
- Full role, routed, recovery, Balance Lab, network, and native flow verification passes.

# Constraints

Promote through milestone specification review before implementation. Do not retain compatibility decoders.

## Implementation progress (2026-08-27)

- Removed the unreachable Build Editor, its standalone persistence file, client overlay/action/rendering paths, and obsolete tests.
- Removed the direct-session build-selection messages, client state/UI, server transaction, session replay state, replicated selection gate, and selection telemetry.
- Saved-brawler snapshots now install every authoritative fighter loadout at admission; the direct diagnostic topology resolves explicit server-owned saved-brawler recipes.
- Removed named full-build catalog presets and IDs, advanced build catalog/fingerprint schema 6 -> 7, protocol compatibility 30 -> 31, and closeout schema 3 -> 4.
- Renamed automation to active weapon-preset terminology, converted network fixtures to admitted loadouts or explicit saved-brawler recipes, and removed legacy report fields/copy.
- Verification in progress: combined client/server check passed before the catalog cutover; full test compilation is running after fixture conversion.

## Verification update (2026-08-27)

- No removed workflow vocabulary or symbols remain in active source, tests, scripts, config, README, or content.
- Combined client/server test compilation and all 577 library tests pass.
- The network-test graph compiles; direct-admission loadout, authority-tamper, and three-match restart scenarios pass.
- Balance Lab all-target check and Clippy pass.

Full routed/recovery/native product evidence remains before closeout.

## Verification update (2026-08-27)

- Full combined client/server library suite now passes: 581 tests.
- Converted network loadout/authority/restart scenarios pass again after subsequent schedule and presentation changes.


## Complete deterministic and routed verification (2026-08-27)

- The canonical gate exposed two incomplete compatibility-floor fixtures: the combined `network-test,balance-lab` graph lacked a qualified `ResolvedMatchLoadout`, and eight scenarios still depended on the removed full-build selection side effect.
- Added a saved-brawler recipe fixture that accepts an explicit fighter profile. Launcher, concealment, recovery, pulse, projectile, and movement scenarios now install their exact weapon base, ultimate, passives, and profile rather than mutating obsolete client selection configuration.
- Combined Balance Lab/network loadout regression passes.
- Full serialized network suite: 88 passed.
- Routed product `just e2e 2`: one exact 1v1 roster reached Active and shut down cleanly.
- Routed Practice `just practice-e2e wipeout-1v1`: one human match reached Active and shut down cleanly.
- Canonical routing, client, server, and Balance Lab suites passed (83+4+5+5+3 routing tests, 416 client tests, 332 server tests, 349 Balance Lab tests).
- All 12 performance gates passed.
- Bounded native two-client gameplay render rerun passed. The saved-brawler/dashboard product UI still needs its explicit native smoke before this ticket closes.

## Native product-flow closeout (2026-08-27)

- The fresh-profile screenshot `target/brl-dashboard-native-clean/brawler-000540.png` verifies that first-run loadout creation now enters the saved-brawler workflow directly.
- The seeded-profile screenshot `target/brl-dashboard-seeded-screens/brawler-000540.png` verifies that Automation Brawler is surfaced as the dashboard selection and that no superseded Build Editor entry point remains.
- The native connection smoke exposed a duplicate deferred despawn during rejected/disconnected link cleanup. The server now inserts `Disconnected` and leaves teardown to Lightyear's lifecycle observer, eliminating the competing despawn command; the focused server lifecycle tests, server Clippy, and a repeated native connection smoke pass without the warning.
- All acceptance evidence is complete: canonical role suites, 88 network scenarios, routed product and Practice E2E, 12 performance gates, native two-client rendering, and native saved-brawler/dashboard UI.
