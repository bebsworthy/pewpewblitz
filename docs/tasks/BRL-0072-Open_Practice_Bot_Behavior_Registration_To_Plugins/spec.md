# BRL-0072 specification

## Outcome

Practice bot intent arbitration consumes a server-only `BotBehaviorRegistry` resource populated during plugin construction. Adding a behavior requires a stable-ID handler plugin and one authored `bots.ron` policy entry, not edits to a central complete behavior slice or a duplicate registered-ID inventory.

## Implemented design

1. `BotBehaviorId` remains the stable authored newtype. Shared policy validation and the server registration boundary both reject zero IDs; existing built-in IDs and the required fallback ID remain stable.
2. `BotBehaviorId::REGISTERED` and the static complete `BEHAVIORS` slice are removed.
3. Shared `BotCatalog` validation is handler-independent and rejects empty/excessive policy, zero/duplicate IDs, invalid scores, and a missing or disabled fallback policy.
4. `BotCatalogResource` and `BotContentPlugin` install the validated authored bot catalog in every gameplay-content role. Content fingerprinting consumes that resource, while protocol-only composition still installs no gameplay content.
5. Server-only `BotBehaviorRegistryBuilder` accepts bounded, unique registrations through `BotBehaviorAppExt::try_register_bot_behavior`. Registration returns deterministic errors for invalid, duplicate, or excessive entries.
6. `BuiltInBotBehaviorsPlugin` contributes the seven current handlers through the same registration API available to future bot behavior plugins.
7. `BotBehaviorRegistryPlugin::finish` removes the mutable builder, validates exact authored-policy/handler coverage, sorts registrations by stable ID, and inserts the immutable registry. Bevy invokes plugin finalization only after all plugin `build` methods, so registration is plugin-order independent and mismatch fails before schedules run.
8. The former server-only `BotProfileResource` is removed. Controller systems consume the shared `BotCatalogResource`; executable handlers remain isolated in the server-only registry.
9. `BotDecisionPolicy` carries the registry reference, and `choose_intent` iterates sealed registrations. Candidate contribution uses one bounded `propose` API, while score, commitment, enablement, fallback, and stable-ID tie-breaking semantics remain unchanged.
10. A synthetic plugin registers ID 77 as the eighth behavior. With one matching authored policy entry it contributes and wins arbitration without any arbiter branch or built-in inventory edit.

## Design amendment from the initial plan

Registry sealing occurs in `Plugin::finish`, not in a `Startup` system. This is the stronger lifecycle boundary for this repository: all plugin `build` registrations are complete, the immutable resource exists before any schedule can run, failures happen during app finalization, and Brawler's test helper explicitly calls `App::finish`/`cleanup`. Exact coverage therefore does not depend on deferred commands or startup schedule ordering.

`BotProfileResource` was not delayed until Startup; it was removed. The shared validated `BotCatalogResource` now owns the authored profile and arbitration data, avoiding a second parsed copy and keeping executable handler coverage separate.

## Constraints preserved

- Registry, handler function pointers, behavior context, and candidate buffer are server-only.
- `content/catalogs/bots.ron`, `BOT_CATALOG_SCHEMA_VERSION`, the content-envelope version, protocol schemas, and wire shapes are unchanged.
- The content fingerprint covers authored bot policy only, never function pointers, plugin types, or registration order.
- Stable IDs, bounded buffers, deterministic ordering/tie-breaking, fallback behavior, and fail-closed finalization remain mandatory.
- No trait objects, dynamic loading, command bus, observation fairness, navigation, role-assignment, input-validation, or authority changes were introduced.
- No native evidence is required because player-visible selection behavior is unchanged.

## Tests and acceptance evidence

- Shared policy tests prove missing/extra unique handler identities are valid authored structures, while empty/oversized, zero/duplicate ID, invalid score, and disabled fallback policies fail.
- Builder tests reject zero, duplicate, and ninth registrations.
- Sealing tests reject missing handlers, missing fallback handler, extra handlers, and disabled fallback policy.
- Reverse plugin build-order testing proves finalization order independence and sorted registry output.
- Synthetic eighth-behavior testing proves plugin-plus-policy extension without arbiter changes.
- Existing bot policy, navigation, objective, controller, lobby, and admission tests continue to pass.

## Verification

- `cargo test --locked --no-default-features --features server --lib bots` — 34 passed.
- `cargo test --locked --no-default-features --features client --lib content::tests` — 2 passed.
- `cargo test --locked --no-default-features --features client --lib protocol::tests` — 13 passed.
- `cargo check --locked --no-default-features --features server --lib` — passed with no warnings.
- `cargo check --locked --no-default-features --features client --lib` — passed with no warnings.
- `cargo fmt --all` — passed.
- `git diff --check` — passed.
- `just check` — passed for routing, client, server, network-test, Balance Lab Rust targets, and Balance Lab web tests/build.

## Acceptance criteria

- [x] Production has no static complete behavior slice or duplicate registered-ID array.
- [x] Built-in and synthetic behavior plugins populate one bounded registry resource.
- [x] Shared bot catalog validation remains headless and handler-independent.
- [x] Plugin construction/finalization fails for duplicate, invalid, or excess registrations and for policy/handler coverage mismatch.
- [x] Fallback policy and handler remain present and enabled.
- [x] The arbiter iterates registry entries and retains deterministic stable-ID tie-breaking.
- [x] A synthetic eighth behavior participates with only plugin registration plus policy data and no arbiter branch.
- [x] Current embedded policy, Practice bot tests, role checks, and `just check` pass.
- [x] Verification and learning are recorded before closeout.

## Learning

The initial plan placed exact coverage in Startup because Startup follows every plugin build. Reviewing Brawler's explicit app-finalization helper exposed a cleaner invariant: plugin `finish` can seal the registry after every build registration and before any schedule executes. Keeping the shared catalog resource independent from the server registry also eliminated the former duplicate `BotProfileResource` parse/copy instead of merely moving its initialization later.


## Final organization correction

Registration lifecycle, builder validation, sealing, and their focused tests live in `src/bots/registry.rs`; behavior contributors and deterministic arbitration remain in `src/bots/behaviors.rs`. This preserves the repository rule that different state owners and lifecycle phases receive named module boundaries rather than making an already-large algorithm file absorb plugin lifecycle ownership. The focused bot suite and full canonical gate were rerun after the extraction and remain green.
