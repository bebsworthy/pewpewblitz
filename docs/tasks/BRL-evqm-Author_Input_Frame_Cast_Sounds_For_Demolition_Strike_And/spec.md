Reviewed 2026-11-15 against HEAD 3437c0d8: valid; spec refreshed with code refs and one trap (do not flip `presents_activation_sound_locally`).

# Outcome

Casting Demolition Strike or Big Blob gives immediate audio feedback on the press, like the other ultimates since BRL-1w27.

# Context (verified)

- `packages/brawler-client/src/local_attack.rs:83` `presents_activation_sound_locally` returns false for `Dash | DemolitionStrike | BigBlob`. The local cast is still announced for both kinds (`LocalAttackPresented` with `LocalAttackKind::Ultimate`), so shoot motion, recoil, launch flash and camera trauma already run on the input frame.
- Combat producer: `audio/producers/combat.rs:68` `local_ultimate_activation_cue` maps DemolitionStrike/BigBlob to `None`; the local cast loop is at `combat.rs:159`. Ability producer: `audio/producers/ability.rs:57` `local_ultimate_spawn_cue` (Sentry/Concealment only).
- Demolition Strike's only sound is `ultimate-strike` from the authoritative `CombatCue::DemolitionStrikeActivated` (`combat.rs:274`), emitted on the cast tick at the target centre (`src/abilities/demolition.rs:156`). Big Blob has no cue and no activation entity (`local_attack.rs:100` `has_activation_signal`).
- TRAP: `LocalAttackLedger::observe_own_activation` (`local_attack.rs:388`) records an activation echo only when `presents_activation_sound_locally(kind)`; `is_local_presentation_echo` (`combat.rs:197`) then skips that cue. Flipping the predicate for DemolitionStrike would silence the authoritative explosion. Use a separate predicate for "local cast sound".
- Audio assets: code-synthesised in `asset_src/audio/build_audio.py` (README `asset_src/audio/README.md`), output `assets/brawler/audio/original/*.ogg`; mappings in `assets/catalogs/audio_profiles.ron` (cue keys ~l.21-30, profiles ~l.71-80); keys in `audio/request.rs:81` `cue_keys` + `BUILTIN`. No cast/launch clip exists for either kind today.

# Plan

1. Add two clips to `build_audio.py` (e.g. `ultimate-strike-cast`: short rising "call-in" ping; `ultimate-blob-cast`: wet heavy "thwomp" launch, distinct from `fire-sticky`), build with `python3 asset_src/audio/build_audio.py ultimate-strike-cast ultimate-blob-cast`, follow the README loudness contract.
2. Register cue keys in `request.rs` (`cue_keys` + `BUILTIN`), add cue/profile rows in `audio_profiles.ron`, include them in `COMBAT_KEYS` (`combat.rs`).
3. Add `local_attack::plays_cast_sound_locally(kind)` (true for DemolitionStrike, BigBlob) and extend `local_ultimate_activation_cue` to return the cast keys for those kinds; leave `presents_activation_sound_locally` and the echo path unchanged so `ultimate-strike` still plays once from authority.
4. Remote casters: default = no cue-driven cast sound (Big Blob has no signal; Strike's explosion already announces it). Note the decision in the module doc.

# Acceptance

- Focused producer test in `audio/producers/mod.rs` tests: a local DemolitionStrike / BigBlob `LocalAttackPresented` writes exactly one cast request; the later matching `DemolitionStrikeActivated` still writes one `ultimate-strike` request; no cast request from the server cue.
- `just lint`, `just test`.
- Owner listening check (native): cast both ultimates in Practice; cast sound on press, explosion once on impact.
