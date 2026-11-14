# Technical specification

## Outcome and scope

Refactor client one-shot audio so feature-owned producer plugins translate their owned gameplay facts and client state transitions into validated stable-key `AudioRequest` messages. A generic audio adapter alone owns authored catalog resolution, occurrence deduplication, loaded-asset fallback, shared concurrency reservations, diagnostics, and Bevy `AudioPlayer` materialization.

This is an organization/extensibility phase. It must not change authoritative gameplay, network/wire types, content fingerprints, asset paths, audible profile values, cue timing, session behavior, or the accepted player experience.

## Current problem

`src/client/audio.rs` centrally composes combat, ability, reload, session, common match, Hot Zone, and Heist interpretation with playback. `AudioCueFamily` plus its static completeness inventory is a closed Rust taxonomy duplicated by the client catalog. Feature systems directly depend on catalog resolution, asset handles, reservations, commands, and playback, so adding a semantic cue requires editing central dispatch and playback-owned code.

## Architecture

### Stable request contract and registry

- Define a client-only bounded `AudioCueKey` and `AudioRequest` message.
- A request contains only a semantic key and optional authoritative occurrence identity. It contains no asset handle, profile ID, speed, volume, cap, fallback, or playback settings.
- Validate nonempty bounded lowercase/digit/dot/hyphen keys.
- Add a bounded startup registry. `AudioRegistryPlugin` creates a private builder during `Plugin::build`; feature producer plugins synchronously register unique producer IDs, deterministic producer ranks, and the exact cue keys they own; `finish` removes the builder, validates exact catalog/registration coverage, sorts deterministically, and inserts an immutable registry.
- Reject registration before plugin installation, after sealing, duplicate producer IDs, duplicate cue keys, invalid keys/ranks, capacity overflow, missing mappings, and orphan mappings. Do not lazily recreate the builder.
- Unknown or malformed runtime requests fail closed without panicking an update.

### Feature-owned producers

Split current interpretation into focused plugins/systems:

- combat producer: exact `CombatCue` mapping and attack/event occurrence identities;
- ability/state producer: ability phase, passive, sentry-spawn, concealment-field, reload, and other replicated-state transitions;
- match producer(s): common completion, Wipeout score, Hot Zone control/contest/threshold, and Heist objective cues including the six-tick damaged suppression window;
- session producer: playable/error transition feedback.

`ClientAudioPlugin` may compose the current built-ins. A new feature using existing playback primitives requires a producer plugin and authored mapping, not central adapter edits.

Producers cannot import `ClientAssetHandles`, `AudioProfileCatalog`, `AudioPlaybackReservations`, `AudioPlayer`, `PlaybackSettings`, or `Commands`.

### Authored catalog and playback adapter

- Migrate `assets/catalogs/audio_profiles.ron` to stable request-key mappings and bump only its client schema.
- Preserve all 16 existing semantic mappings and every profile ID, asset ID, speed, volume, concurrency cap, fallback, and global cap exactly.
- Validate bounded unique keys, mapping/profile/reference coverage, finite positive values, capacities, fallback termination, and exact registry agreement.
- The generic playback system applies `(cue_key, occurrence)` deduplication where occurrence exists, preserving the 128-entry newest-key history.
- Reset shared reservations before producers and play requests after them through explicit audio schedule sets.
- Preserve same-frame immediate reservation behavior, high-priority capacity reservation, per-profile/global caps, missing/late asset terminal silence, and `PlaybackSettings::DESPAWN`.
- Characterize and retain the private suppression counter semantics. Cap rejection is suppression; unavailable assets remain expected silence unless existing behavior proves otherwise.

## Preserved behavior

- Attack acceptance → fire by attack ID.
- Delivery/lob/melee → impact by attack ID; fire and impact remain independent.
- Cone spray, cloak end, and demolition → impact by event ID.
- Damage/defeat/reset, sentry fire/removal, concealment/reveal, elemental, charge/ultimate/passive/deployable transitions remain exact.
- Heist damaged cues retain the six-authoritative-tick suppression; critical/destroyed remain immediate.
- Reload remains controlled-fighter ammunition increase.
- Match completion has one common owner; Wipeout, Hot Zone, session ready/error transitions remain exact.
- Recent occurrence capacity stays 128; authored per-profile/global caps stay exact.
- Audio remains optional and never gates authority, readiness, navigation, saving, cleanup, or session state.
- No server, protocol, routing, gameplay fingerprint, or balance data change.

## Tests and verification

1. Exact 16 built-in key-to-profile parity and byte-equivalent profile values.
2. Registry lifecycle, duplicate/invalid/capacity/missing/orphan rejection, and deterministic order.
3. Synthetic stable request mapping reaches generic playback planning without adapter dispatch changes.
4. Same-frame ResetReservations → ProduceRequests → PlaybackRequests order.
5. `(cue, occurrence)` deduplication, 128-entry eviction, and independent fire/impact occurrence keys.
6. Complete combat mapping including intentionally silent legacy variants.
7. Heist six-tick suppression and exact objective mappings.
8. Wipeout, Hot Zone, common completion, reload, ability/state, and session transition characterization.
9. Loaded asset, unavailable fallback, terminal silence, shared cap, high-priority reservation, and despawn behavior.
10. Producer dependency-boundary checks and client composition.
11. Server-only isolation.
12. Focused tests, `just check`, `just lint`, `just test`, and native audible smoke for representative combat/reload/ability plus objective/session cues.

## Implementation plan

1. Characterize current mappings, occurrence identities, transitions, suppression, priority, and cap behavior.
2. Add stable request/registration types and bounded lifecycle.
3. Migrate the audio catalog to stable keys while preserving exact profile data.
4. Extract feature-owned producer plugins and explicit schedule sets.
5. Convert playback to one generic request consumer.
6. Add extension, ordering, gate, deduplication, fallback, and parity tests.
7. Update durable presentation documentation.
8. Run automated and native verification, independent review, feedback disposition, and learning review.

## Exclusions

- VFX changes, general presentation bus, spatial audio, music, new sounds, volume UX, mixer redesign, asset hot reload, server/shared gameplay changes, or protocol changes.


## Registration sharing refinement

- A semantic audio cue key may be declared by multiple distinct producer registrations; this preserves intentional shared cues such as `ready`, `impact`, and `defeat` across session, combat, and match producers.
- Reject duplicate producer IDs, duplicate keys within one producer registration, missing catalog coverage for the union of registered keys, and unknown catalog keys.
- Request provenance/order must identify the producer independently of the cue key when deterministic cap precedence needs it; cue-key ownership alone is not a valid producer identity.


## Runtime ordering correction

Independent review found that a sealed registration rank which only sorted startup metadata did not make extension-plugin playback/cap precedence deterministic. `AudioRequest` therefore carries renderer-neutral ordering metadata: the registered producer rank plus a producer-local monotonic sequence. The generic adapter validates key/rank ownership and sorts each frame by that order before deduplication and reservation. Asset/profile/playback policy remains absent from the request. Built-in ranks preserve the established combat → Heist → ability → reload → session → common match → Hot Zone precedence; future producers gain deterministic placement by registration and request construction without central adapter edits.


## Diagnostic normalization

The legacy private `suppressed` counter was internally inconsistent: Heist counted unavailable assets, combat/common match/Hot Zone counted only cap rejection, and ability/reload/session ignored cap rejection. The generic adapter intentionally normalizes this non-player-visible diagnostic to count every concurrency-cap rejection and no expected unavailable/terminal-silence result. Audible behavior, authored caps, fallbacks, warning power-of-two cadence per aggregate counter, and authority remain unchanged.


## Implementation result

- Replaced the closed `AudioCueFamily` dispatch with schema-3 stable `AudioCueKey` mappings, semantic `AudioRequest` messages, a bounded sealed producer registry, exact catalog/registration validation, and a single generic playback adapter.
- Split interpretation into combat, Heist, ability, reload, session, common match, and Hot Zone producers with focused transition memory. Producers have no asset, catalog, reservation, command, or playback dependencies.
- Runtime requests carry registered producer rank plus producer-local monotonic sequence. The adapter validates exact rank/key ownership and sorts before occurrence deduplication and reservation, preserving current 10/20/30/40/50/60/70 precedence while making additive producer ordering open to extension.
- Preserved all 16 key/profile mappings and every profile asset, speed, volume, concurrency cap, fallback, the 128 occurrence history, Heist six-tick damage suppression, transition ownership, shared immediate reservations, terminal silence, and `PlaybackSettings::DESPAWN`.
- Normalized the private suppression diagnostic as specified: all cap rejections count; unavailable assets do not. No audible or authoritative behavior changed.
- Updated `docs/11-art-and-presentation-direction.md` with the durable stable-request/registry/adapter contract.

## Verification evidence

- `cargo test --locked --no-default-features --features client --lib client::audio::` — pass, 32/32. Coverage includes exact catalog parity, registry lifecycle/capacities/coverage/sharing, synthetic extension resolution, exact rank/key rejection, scrambled deterministic order, occurrence dedup/eviction, all feature transition selectors, Heist tick suppression, explicit Reset → Produce → Playback order, terminal silence, shared reservation caps, and DESPAWN materialization.
- Producer forbidden-dependency scan for `ClientAssetHandles|AudioProfileCatalog|AudioFrameReservations|AudioPlayer|PlaybackSettings|Commands` — clean.
- `just check` — pass for routing, client, server, network-test, Balance Lab, and Balance Lab web checks/build.
- `just lint` — pass for formatting, all Clippy feature graphs with `-D warnings`, server presentation-feature isolation, V3 renderer contract, and V8 map cleanup.
- `just test` — pass: routing 83 plus process suites; client 543; server and Balance Lab suites; combined Balance Lab/network case 1; network 97; performance 12. Representative performance p95 values remained inside the fixed-tick budget.
- Canonical native routed combat smoke: `BRAWLER_RENDER_COMBAT=1 just v3-render-evidence target/brl-0083-audio-smoke-canonical.txt` — pass on both clients, 1,801 samples each, p95 16.972 ms / 16.960 ms, effect terminal 0, projectile terminal 0, and no audio/backend errors in either client log. Combat-driving inputs exercised attack/ultimate requests plus session and match transitions.
- An intentionally shortened 10-second preliminary run produced healthy timing but failed only the locked 1,800-sample threshold at 601 samples; it was replaced by the canonical-duration passing run and is not accepted evidence.
- Independent architecture review initially found runtime rank ordering and system-level playback evidence gaps. Both were corrected and re-reviewed; final review reported no remaining actionable findings.

## Feedback disposition and learning review

- Accepted: shared semantic keys must remain legal across unique producers; exact rank/key provenance, not cue-key ownership alone, controls runtime ordering.
- Accepted: cap-only suppression normalization is explicit because the legacy private counter treated producers inconsistently.
- Accepted: system-level schedule, terminal-silence, and DESPAWN tests complement pure catalog/reservation tests.
- Mistake: the first registry design sorted ranks only at startup while retaining a hard-coded runtime chain, which left additive producer cap precedence unspecified. Cause: treating deterministic metadata as equivalent to deterministic consumption. Prevention: whenever capacity/eviction depends on cross-producer order, carry validated provenance/order through the request and test scrambled arrival through the production sorter.
- Mistake: the first native command shortened a harness with a locked sample threshold. Cause: optimizing duration without checking the evidence validator. Prevention: use canonical recipe durations for accepted evidence; use shortened runs only as explicitly preliminary diagnostics.
- Reusable lesson: a Bevy presentation extension seam is complete only when producer dependencies, startup coverage, runtime provenance/order, fallback, materialization, and schedule boundaries are independently testable.
