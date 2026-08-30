# BRL-0063 technical specification

## Outcome

Complete BRL-0061 Stage 2 by making the remaining audited player-affecting policy values come from validated authored catalogs and by proving runtime consumers observe those values. Preserve accepted balance, server authority, deterministic fixed-tick behavior, stable wire contracts, and client/server feature isolation.

## Scope

1. Inventory the exact current constants, consumers, catalog owners, persistence/fingerprint impacts, and tests for:
   - spawn protection and completed-match input lock;
   - Wipeout recent-hostile credit;
   - Heist critical-health feedback threshold;
   - Hot Zone player-affecting proximity/timing policy, if any remains code-owned;
   - bot behavior arbitration base scores, commitment policy, enablement, and deterministic tie-breaking;
   - lobbed-delivery minimum flight policy;
   - duplicate effect-tile runtime tuning constants;
   - VFX/audio scale, anchor, lifetime, asset mapping, and weapon presentation-profile consumption.
2. Record research findings here before implementation. Implement only demonstrated runtime gaps; reject or defer speculative schema fields with evidence.
3. Extend the smallest existing owning RON/catalog schema. Validate bounds, unique IDs, cross-references, registered behavior/profile coverage, and deterministic numeric representation.
4. Route authoritative and presentation consumers through the validated policy without silent code fallbacks.
5. Advance catalog schema/revision/fingerprint contracts where authored content changes and update durable owning documentation.

## Constraints

- Do not change accepted player-visible values except to make an existing live value explicit in its authored source.
- Capacity ceilings, bounded diagnostic buffers, physics iteration limits, protocol compatibility constants, and other genuine engine/process invariants remain code-owned.
- Keep bounded typed content schemas; do not introduce stringly typed effect payloads or a general configuration framework.
- Preserve fixed-tick system ordering, authority ownership, stable IDs, persistence recovery, and current routed topology.
- Do not introduce Stage 3 plugin ownership work or Stage 5 registries except for the minimum validation seam required by authored behavior coverage.
- Presentation policy remains client-only where appropriate and must not enter the dedicated-server dependency graph.
- Preserve unrelated user changes.

## Acceptance criteria

- Every demonstrated residual player-affecting value in scope has one documented authored source and no competing production literal.
- Match lifecycle and mode-specific policy values are parsed, validated, fingerprinted, and consumed by authoritative runtime systems.
- Bot arbitration policy is authored by stable behavior ID, rejects duplicates/missing/extra registrations and invalid arithmetic bounds, and retains deterministic tie-breaking.
- Lob minimum-flight policy is validated with the owning weapon/delivery definition and observed by attack runtime.
- Effect-tile production systems have no duplicate tuning defaults that can diverge from authored map data.
- VFX/audio configurable fields are manifest-owned and runtime-observed; weapon presentation_profile_id is either validated and consumed end to end or intentionally removed with compatibility/documentation evidence.
- Round-trip, invalid-bound, missing-reference, duplicate-ID, and runtime-observation tests cover every added field.
- Role-specific checks, focused tests, routed scenarios, performance gates, and any required native visual/audio evidence pass.
- Findings not implemented are explicitly rejected or deferred with rationale.
- BRL-0061 records Stage 2 disposition; learning review and `ticket sync` complete without conflict.

## Verification

- Focused catalog/model/runtime tests for each policy family.
- `cargo fmt --all -- --check`
- `just check`
- `just lint`
- `just test`
- `just ci`
- `git diff --check`
- Native evidence for any changed VFX/audio/profile behavior, including reduced-effects behavior and audible cue confirmation when audio loading changes.

## Delivery

BRL-0063 blocks BRL-0061 and remains `doing` until its acceptance criteria, evidence, documentation, feedback disposition, and learning review are complete. No implementation commit is required unless explicitly requested.

## Research disposition — 2026-08-30

The Stage 2 inventory traced each audited literal to its runtime consumers and established these implementation decisions before schema work:

- The operator game-type catalog is the authored owner for common lifecycle and mode policy. Schema 4 adds common spawn-protection and completed-input-lock ticks plus Wipeout recent-hostile credit and Heist critical-health percentage. Those values must cross the routed allocation and match manifest; validating them only in the lobby would leave match authority on code defaults.
- Wipeout's 300-tick credit window becomes part of validated `WipeoutRules`. Heist's 25% threshold becomes part of validated `HeistRules` and replicated `HeistState`, because late-joining client presentation cannot safely infer it from transient cues.
- Hot Zone capture timing is already authored as `capture_seconds` and reaches `HotZoneRules`. Its 240-world-unit near-combat expansion is telemetry classification only, not player-affecting policy, so it remains code-owned. Per-tick evaluation cadence is schedule semantics.
- The six-tick lob floor belongs to `WeaponRecipePolicy`, not `DeliveryMethod`; this avoids changing replicated recipe shape. Validation requires a nonzero floor no greater than every Lobbed/Splash maximum, and authority reads the embedded catalog policy.
- Four public effect-tile balance constants have no production consumers. The map gameplay-profile catalog already supplies the exact runtime values, so the duplicates and their public re-exports are removed while engine bounds remain code-owned.

- Bot arbitration is currently seven stable registrations with code-owned scores and a universal +1000 commitment bonus. `bots.ron` schema 2 adds an arbitration block keyed by stable behavior ID with enablement and base score. Startup validation requires exact registration coverage, unique IDs, an enabled fallback, bounded score arithmetic, and deterministic lower-ID tie-breaking.
- The shared weapon `presentation_profile_id` is dead presentation data: it is copied through catalog, loadout, attack source, cues, evidence, and protocol registration, but VFX and audio ignore it. The only semantic read incorrectly identifies Scatter evidence by a cosmetic number. BRL-0063 therefore removes this field and advances affected weapon/build/profile/content/protocol compatibility versions rather than inventing unsupported per-weapon presentation behavior.
- VFX lifetime is authored already, but base scale and anchor remain caller literals. Client VFX schema 2 owns typed scale and anchor policies, typed material keys, exact cue-family coverage, and reduced-profile behavior while retaining authoritative cue radii where geometry is gameplay-derived.
- Audio speed, volume, cap, and semantic mapping are authored already, but paths are duplicated between the provenance manifest and client loader. Audio schema 2 references stable asset-manifest IDs, removes inert one-variant playback/lifetime fields, and validates exact cue-family coverage. Missing runtime assets may still degrade through an explicit fallback; a missing embedded family mapping is invalid.
- Client VFX/audio catalogs remain outside the headless gameplay fingerprint and server dependency graph. Shared presentation-ID removal is a separate intentional compatibility change because it changes authored and registered shared shapes.

## Implementation completed — 2026-08-30

- Operator game-type schema 4 now owns common spawn protection and completed-input-lock timing plus Wipeout hostile-credit and Heist critical-health policies. Validation occurs at lobby resolution, values cross routing control v5 / manifest v4, and match workers install the exact resolved rules. The public advertised-catalog revision remains client-recomputable; a separate private policy revision fingerprints policy-only changes.
- Bot catalog schema 2 owns the seven stable behavior scores, enablement, and commitment bonus. Startup requires exact behavior-registration coverage, rejects duplicate/missing/extra IDs and unsafe arithmetic, and retains deterministic lower-ID tie-breaking. Behavior registration, candidate collection, and arbitration are now separate responsibilities.
- Weapon catalog schema 11 / fingerprint format 9 owns the six-tick lob minimum in `WeaponRecipePolicy`; attack authority consumes it. Duplicate effect-tile tuning constants and re-exports were removed because the map gameplay-profile catalog was already the runtime owner.
- The unused shared `presentation_profile_id` chain was removed end to end rather than inventing unsupported semantics. Weapon/build/profile/content/protocol compatibility versions were advanced, Scatter evidence now identifies the stable source preset and authored spread pattern, and the exact legacy identifier search is empty.
- Client VFX schema 2 owns typed renderer/material, scale, anchor, lifetime, cap, fallback, and reduced-effects policy with complete cue-family coverage, finite world-scale bounds, checked multiplication, strict unknown-field rejection, and runtime observation tests.
- Client audio schema 2 maps complete semantic cue-family coverage to stable `assets/manifest.ron` IDs. The manifest is the sole path/provenance owner; inert lifetime/playback fields were removed; strict unknown-field, fallback, reference, and runtime-selection tests cover the adapter.
- Native render automation now accepts `--render-reduced-effects` through `BRAWLER_RENDER_REDUCED_EFFECTS=1`. This bounded evidence-only seam installs the same `ClientShellSettings` policy used by the interactive product shell, and the report records the active value so a default run cannot masquerade as reduced-effects evidence.
- Durable contracts were updated in `docs/03-weapons-and-abilities.md`, `docs/04-maps-and-game-modes.md`, `docs/10-bots.md`, `docs/11-art-and-presentation-direction.md`, and `docs/16-grid-map-asset-system.md`.

## Verification evidence — 2026-08-30

Passed after the final implementation state:

- `cargo fmt --all -- --check`
- `just check`
- `just lint`, including all role-specific Clippy targets, server feature isolation, V3 renderer contract, and V8 map cleanup
- `just test`: routing suites; 488 client tests; 428 server tests; 450 Balance Lab tests; 94/94 network scenarios; 12/12 performance gates
- `just ci`: lint/test repeated cleanly; routed product 1v1, 2v2, and 3v3 reached Active; all Wipeout, Hot Zone, and Heist Practice 1v1/2v2/3v3 types reached Active
- `git diff --check`
- Exact `rg 'WeaponPresentationProfileId|presentation_profile_id' src content tests` returned no matches.
- Focused non-default runtime proofs cover seven-tick spawn-protection expiry, seven-tick completed-input lock/restart acceptance, and a replicated 26% Heist threshold with the exact critical crossing.

Native release-client evidence:

- `target/brl-0063-render-evidence.txt` and `.peer`: native profile, `reduced_effects=false`, `result=pass`, `first_failure=none`.
- `target/brl-0063-render-evidence-reduced-verified.txt` and `.peer`: native profile, `reduced_effects=true`, `result=pass`, `first_failure=none`.
- `target/brl-0063-render-evidence-reduced-practice.txt`: routed Practice Wipeout 1v1, `reduced_effects=true`, three transient effects and two projectiles observed, `result=pass`, `first_failure=none`.

Audio catalog loading, manifest resolution, family selection, fallback, concurrency, and real native match readiness have automated evidence. Whether the emitted ready/fire/impact cues are actually audible cannot be established from logs or render reports. Required human audible-cue confirmation remains the only open BRL-0063 acceptance item. Suggested bounded check: `BRAWLER_RENDER_REDUCED_EFFECTS=1 BRAWLER_RENDER_PRACTICE=1 just v3-render-evidence target/brl-0063-audio-playtest.txt`; listen for the match-ready cue and bot fire/impact cues during the approximately 40-second run.

## Feedback disposition

- No gameplay correction was requested during implementation.
- Reduced-effects behavior is implemented and observed natively with live combat effects.
- Audible cue quality/presence is awaiting human confirmation; BRL-0063 remains `doing` until that evidence is supplied.

## Learn-from-errors review

- Adding private lifecycle policy to the advertised catalog hash broke lobby welcome because clients can recompute only advertised fields. Prevention: keep public compatibility digests derived solely from public canonical bytes and fingerprint private policy separately.
- Replacing production-tuned `Default` values with neutral valid sentinels exposed network fixtures that had implicitly depended on production defaults. Prevention: verification harnesses must install explicit authored-equivalent fixtures, while non-default tests prove consumers observe injected policy.
- The first reduced-effects native attempt passed rendering but correctly reported `reduced_effects=false` because automation intentionally omits the interactive settings shell. The result was rejected, the temporary user-settings file was removed, and an explicit evidence-only config seam was added. Prevention: accept native evidence only when the report proves the requested policy was active.
- Independent review found missing VFX world-scale bounds, permissive unknown fields, and an unchanged bot fingerprint-format version. Prevention: every schema migration now checks arithmetic safety, strict decoding, exact coverage, and both schema and canonical fingerprint compatibility versions.

## Native audio acceptance — 2026-08-30

The user completed the bounded reduced-effects Practice audio playtest and confirmed that sound is audible. This satisfies the final native audio acceptance item. No audio correction was requested; ready/fire/impact presentation is accepted for BRL-0063.
