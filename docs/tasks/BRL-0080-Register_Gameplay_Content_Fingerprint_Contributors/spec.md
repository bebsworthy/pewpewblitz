# Technical specification

## Scope and outcome

Introduce a plugin-populated gameplay fingerprint registry for the seven existing shared gameplay domains. Each owning content plugin contributes read-only canonical material under a stable domain ID. A single deterministic evaluator serves both pre-Startup routing/worker identity and the Startup-installed `GameplayContentFingerprint` resource.

This is an organization and compatibility-correctness phase. It does not introduce dynamic assets, runtime hot registration, client presentation content, per-message compatibility versions, or a new wire type.

## Architecture

### Registration model

- Add a code-owned `GameplayFingerprintRegistration` containing:
  - a stable domain ID;
  - a domain schema version;
  - a read-only `fn(&World) -> Result<Vec<u8>, String>` material callback.
- Store registrations in a private bounded builder resource populated during `Plugin::build`.
- Expose a crate-scoped `App` extension/helper so owning content plugins register their own contribution without exposing the builder as public gameplay API.
- The evaluator copies registrations, rejects over-capacity and duplicate IDs, evaluates their material against current World resources, sorts contributions by stable ID, and hashes a serialized `(envelope_version, [(id, schema_version, material)])` envelope.
- Stable IDs are part of compatibility identity. Do not derive IDs from `TypeId`, Rust type names, module paths, or insertion order.
- Use these built-in IDs: `bots.practice`, `builds.catalog`, `combat.conditions`, `combat.weapons`, `concealment.rules`, `map.catalog`, and `weapon-parts.catalog`.
- Require exact built-in coverage for `GameplayContentPlugin`; synthetic optional contributors are allowed only through the generic registry seam and remain included deterministically.

### Ownership and composition

- Existing content plugins register their owned material:
  - `BotContentPlugin` → Practice bot catalog;
  - `BuildContentPlugin` → build catalog and cross-validation against the live weapon catalog;
  - `WeaponPartContentPlugin` → weapon-part catalog;
  - `ConcealmentContentPlugin` → concealment rules;
  - `MapContentPlugin` → map catalog.
- Add a small headless-safe combat content plugin (or equivalently focused combat-owned registration plugin) that initializes and registers the weapon catalog and combat-condition rules. `GameplayContentPlugin` composes it rather than owning those two concrete resources directly.
- Catalog validation and `canonical_fingerprint_material` functions remain pure and Bevy-independent.
- Shared gameplay content remains transitively identical in client, direct server, lobby worker, match worker, combined tests, and temporary routing-identity Apps.

### Lifecycle and compatibility

- Provide `gameplay_content_fingerprint_from_world(&World)` for pre-Startup callers.
- Convert `initialize_content_fingerprint` to an exclusive Startup system that calls the same evaluator and immediately inserts `GameplayContentFingerprint`.
- Preserve its system identity/order so Balance Lab startup remains after fingerprint initialization; runtime Balance Lab tuning stays excluded from build compatibility.
- Pre-Startup resource overrides must affect both evaluator and Startup results, including weapon-part and combat-condition resources that the old path incorrectly reloaded from embedded files.
- Route `routing_identity` and runtime worker-manifest validation through the World evaluator; remove the misleading three-catalog aggregation path.
- Bump `GAMEPLAY_CONTENT_ENVELOPE_VERSION` from 26 to 27 because stable IDs/schema versions and sorted contribution framing intentionally change compatibility bytes.
- Keep `GameplayContentFingerprint` and all handshake/wire message shapes unchanged. Existing global mismatch rejection remains the sole compatibility gate.

### Bounds and errors

- Define a small explicit registration capacity (at least the seven built-ins plus focused test/near-term headroom).
- Duplicate stable IDs, missing required built-ins, excessive registrations, missing resources, catalog validation failures, and serialization failures return deterministic configuration errors.
- Do not allow presentation-only catalogs or server-only operator/runtime tuning to register with this shared registry.

## Acceptance criteria

1. Adding a synthetic gameplay domain registration changes the fingerprint without changing the central evaluator.
2. Reversing registration order produces the same fingerprint.
3. Duplicate IDs, missing required built-in coverage, and capacity overflow fail deterministically.
4. Changing either a domain ID, schema version, or canonical bytes changes the fingerprint.
5. All seven built-in domains are registered by their owning shared content composition and are evaluated from live World resources.
6. Pre-Startup overrides of weapon-part and combat-condition resources change the result, closing the embedded-reload correctness gap.
7. Invalid live build-to-weapon references fail aggregation.
8. The fingerprint is absent before Startup and installed immediately after Startup; pre-Startup evaluation equals the installed value.
9. Routing identity and production server graph calculate the same content fingerprint. Matching workers are accepted and mismatches retain existing rejection behavior before fighter spawn.
10. Contributor registration/order and shared content composition remain headless-safe and role-consistent; no rendering, input, audio, client assets, protocol plugin, or server-only tuning enters `GameplayContentPlugin`.
11. Envelope version 27 is explicitly tested and documented; no protocol registration/wire shape changes.
12. Durable network architecture documentation explains contribution ownership, stable IDs, deterministic ordering, exclusions, and the single global compatibility boundary.

## Implementation plan

1. Add registry types, bounds, deterministic evaluator, World evaluation API, and focused failure/order tests in `src/content.rs`.
2. Move weapon/condition initialization into combat-owned headless-safe content composition and add registrations in each owning content plugin.
3. Convert Startup finalization to exclusive World evaluation and retain Balance Lab ordering.
4. Update routing identity and worker validation to use the shared World evaluator; adapt the existing public pure helper only if required by callers, without retaining manual domain enumeration.
5. Add regression tests for live resource overrides, invalid references, routing parity, and content mismatch behavior.
6. Document the compatibility contract in `docs/08-network-architecture.md`.
7. Run focused content/admission/network tests, client/server/combined feature checks, `just check`, `just lint`, and the canonical suite proportional to compatibility risk.

## Verification and evidence

Record exact commands/results in this spec before closeout. Native visual evidence is not required because the change preserves gameplay and presentation output. A disk-capacity cleanup of regenerable target artifacts may be used if compilation exhausts the development volume, but it is not product evidence.

## Scope exclusions

- Dynamic loading or runtime mutation of contributor registrations.
- Per-message compatibility versions or compatibility decoders.
- Protocol/routing enum changes.
- Mode, tile-handler, VFX, or other behavior registries.
- Balance Lab runtime tuning in build compatibility.
- Client-only visual/audio asset fingerprints.

## Implementation record — complete 2026-08-31

- Replaced the seven-domain tuple in `src/content.rs` with a private, bounded registry of stable domain ID, domain-owned schema version, and read-only World material callback registrations.
- Added deterministic stable-ID ordering, duplicate/missing/capacity/domain-ID validation, and envelope 27 framing over `(domain_id, domain_schema_version, canonical_material)`.
- Added a headless-safe combat content plugin and made the bot, build, weapon-part, concealment, and map content plugins register their owned contributions. Build material retains live weapon-reference cross-validation.
- Converted Startup fingerprint installation to an exclusive World finalizer and added a pre-Startup World evaluator. Routing identity and lobby/match worker validation now use that same evaluator.
- Closed the prior correctness gap: live weapon-part and combat-condition resource overrides now affect compatibility instead of being silently replaced with embedded values.
- Preserved the `GameplayContentFingerprint` and handshake wire shapes, the global mismatch gate, Balance Lab ordering/exclusion, and client/server role isolation.
- Documented contributor eligibility, deterministic framing, exclusions, role parity, and envelope 27 in `docs/08-network-architecture.md`.

## Verification evidence

All commands passed on 2026-08-31:

- `cargo test --lib content::tests --no-default-features --features server` — 8 passed.
- `cargo test --lib server::admission::tests --no-default-features --features server` — 17 passed.
- `cargo check --no-default-features --features client --lib` — passed without warnings.
- `cargo check --no-default-features --features server --lib` — passed without warnings.
- `cargo check --no-default-features --features network-test --lib` — passed without warnings.
- `cargo test --test network map_content_mismatch_rejects_before_fighter_spawn --no-default-features --features network-test` — 1 passed; rejection occurred before fighter spawn.
- `just lint` — formatting, web checks/build, all Clippy role graphs, server feature isolation, V3 presentation contract, and V8 map cleanup passed.
- `just check` — routing, client, server, network-test, Balance Lab, and web feature graphs passed.
- `just test` — routing library 83 plus routing process suites passed; client 507 passed; server 472 passed; Balance Lab 494 passed; combined revised-catalog replication passed; all 97 network scenarios passed; all 12 performance gates passed.
- `git diff --check` — passed.

No native evidence was required because gameplay, UI, rendering, audio, controls, and authored balance values are unchanged.

## Learn-from-errors review

- The first implementation draft included domain schema versions in the hash but supplied one central version for every domain. Independent review caught that this contradicted domain ownership and made schema evolution impossible through the supported seam. The fix passes an explicit schema version at every owning plugin registration and tests schema sensitivity through that registration API.
- Cause: the envelope framing requirement was implemented before checking that every compatibility field was owned at the intended extension boundary.
- Prevention: extension-seam tests must exercise all extensibility inputs through the same registration API available to production plugins; private registry mutation is insufficient evidence.
- Canonical lint also identified two deliberately fallible synthetic callbacks as `unnecessary_wraps`; the exception is narrowly attached to those test callbacks because their signature intentionally proves the fallible production contract.
- No player-visible regression, compatibility fallback, protocol-shape change, or deferred correction remains. Mode registration remains separately deferred under BRL-0070 rather than being expanded into this phase.
