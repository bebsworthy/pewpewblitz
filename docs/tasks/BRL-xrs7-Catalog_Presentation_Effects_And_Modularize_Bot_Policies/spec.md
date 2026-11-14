# Outcome

New presentation variants within supported renderer/audio families are catalog additions, and new Practice-bot tactics can be installed as deterministic scored behaviors rather than edits to one priority cascade.

# Scope

- Define client-only validated VFX/audio catalog entries keyed by stable presentation/cue families.
- Data-drive material/audio keys, renderer family, scale, lifetime, concurrency cap, and degradation fallback for supported families.
- Retain code-owned renderer implementations for genuinely new rendering mechanics.
- Replace closed bot tactic selection with bounded scored intent candidates and one deterministic arbiter using stable behavior IDs and tie-breaking.
- Let focused bot behavior modules contribute candidates for combat, objectives, pickups, retreat, and future mode/entity policies.
- Continue committing only ordinary FighterInput; bot policy must not become a second authority path.
- Preserve reduced-effects, audio caps, asset provenance, concealment fairness, delayed perception, and bounded navigation behavior.

# Acceptance criteria

- A new supported VFX/audio variant can be added through catalog data without editing the central cue-effect/audio match.
- Invalid asset/profile references and unsafe lifetimes/caps are rejected with deterministic fallback behavior.
- Existing visual/audio output remains acceptably equivalent for built-in content.
- Existing bot decisions remain deterministic for fixed observations/seeds.
- A test bot behavior can be added without editing the arbiter or existing behavior algorithms.
- Bot observation limits and server-authoritative FighterInput path are unchanged.

# Verification

- Catalog validation, fallback, cue mapping, and audio-cap tests.
- Native visual/audio smoke checks for representative cues.
- Pure bot scorer/arbiter determinism and tie-break tests.
- Existing Practice bot concealment, navigation, objective, and routed authority suites.


# Implementation evidence

- Added client-only validated VFX and audio catalogs with stable cue-family mappings, supported renderer/audio profiles, asset keys, scale/speed/volume, lifetime, per-profile caps, fallback policies, and reduced-effects variants.
- Routed every existing transient combat-effect and one-shot combat-audio consumer through catalog resolution while retaining code ownership of renderer mechanics and silence as the terminal audio fallback.
- Replaced the closed Practice-bot priority cascade with seven bounded candidate producers (healing, pressure, object, fallback, objective, pickup, retreat), unique stable behavior IDs, a maximum-eight candidate buffer, and deterministic score-then-ID arbitration.
- Preserved delayed observations, concealment filtering, navigation, and the ordinary server-owned FighterInput commit path.

# Verification evidence

- just check: passed for client, server, and Balance Lab feature graphs.
- just lint: passed with warnings denied for all canonical role graphs.
- just test: passed; client 475, server 414, Balance Lab 436, catalog/loadout replication 1, routed network 90, and performance 12 tests.
- Focused catalog tests cover built-in parity, additive supported variants, invalid references/values/caps, deterministic fallback, and silence termination.
- Focused bot tests cover registry bounds, duplicate IDs, additive behavior registration, bounded candidates, deterministic score ordering, and stable tie-breaking; existing Practice authority, concealment, navigation, and objective tests remain green.
- Native Practice wipeout-1v1 render evidence passed with a 22-second measurement window: target/brl-0059-render-evidence-pass.txt. The client loaded the presentation catalogs and entered routed gameplay without runtime failure. Subjective audible quality was not independently assessed; exact audio mapping/cap behavior is covered by automated tests.
- cargo fmt --all -- --check and git diff --check: passed.

# Learn-from-errors review

- The first shortened native measurement used five seconds and failed only the locked 1,200-sample threshold (301 samples); the report otherwise passed. Reading the threshold and rerunning the warmed binary for 22 seconds produced valid evidence. Future shortened evidence runs must preserve minimum sample-count contracts.
- Parallel slices briefly exposed temporary missing symbols and build-lock contention during integration; strict file ownership plus final whole-tree checks kept the merged result safe.
- Legacy VFX behavior included non-obvious reduced-scale and lifetime values, so built-in parity is encoded in catalog tests instead of inferred from friendly defaults.
- Early bot contributors shared a combat identity; unique stable IDs and duplicate/overcapacity validation now make arbitration extension-safe.
