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


# Implementation (2026-11-18, against HEAD d54298b9, after BRL-crdt)

## Decisions
- New predicate `local_attack::plays_cast_sound_locally(kind)` (true only for DemolitionStrike, BigBlob). `presents_activation_sound_locally` is unchanged (TRAP honored), so the ledger never records an activation echo for a Strike; `own_activation_cue` also already passes `None` as the Strike echo. The authoritative `ultimate-strike` explosion therefore plays exactly once from `CombatCue::DemolitionStrikeActivated`.
- Combat producer `local_ultimate_activation_cue` maps DemolitionStrike -> `ultimate-strike-cast`, BigBlob -> `ultimate-blob-cast` on the local `LocalAttackPresented` (same local occurrence range as other local casts). Keys added to `COMBAT_KEYS`, `cue_keys` + `BUILTIN` (49), cue/profile rows in `assets/catalogs/audio_profiles.ron` (volume 1.0, cap 24), manifest rows in `assets/manifest.ron`, README inventory.
- Remote casters: no cast sound (Big Blob has no signal; the Strike explosion announces a remote Strike). Recorded in the `plays_cast_sound_locally` doc and in `docs/08-network-architecture.md` (Local attack presentation).
- A rejected local cast still leaves its short cast sound (same as other launch sounds).

## Sounds (asset_src/audio/build_audio.py, deterministic; rebuild reproduces byte-identical .ogg)
- `ultimate-strike-cast` (0.325 s, RMS -18.0 dBFS, peak -7.0 dBFS): radio "call-in": a band-passed static key-up click with a quick triangle chirp, then a rising two-note square/sine beacon (A5 then E6, 110 ms apart), low-passed at 4.2 kHz. Distinct from `ultimate-scan` (single sine ping with echoes) and from the low, long `ultimate-strike` explosion.
- `ultimate-blob-cast` (0.455 s, RMS -16.0 dBFS, peak -7.1 dBFS): wet heavy "thwomp": a 150->42 Hz sine body drop, a gloopy 22 Hz-wobbled sine sliding down from ~230 Hz, and a muffled closing low-pass noise splat, lightly driven. Lower, longer, rounder than `fire-sticky` (square 420->110 Hz "thwup").
- Both within the README contract: gameplay RMS band -15 to -20 dBFS, peak under -1 dBFS, 0.05-2.2 s, start on the transient, end in silence.

## Verification
- New test `audio::producers::tests::strike_and_blob_casts_sound_on_the_input_frame_and_the_strike_explosion_plays_once`: local Strike + Blob casts write exactly one `ultimate-strike-cast` and one `ultimate-blob-cast`, no `ultimate-strike` on the press; the later own `DemolitionStrikeActivated` (with the reconciler stand-in even offering a Cue echo) writes exactly one `ultimate-strike` and no further cast request.
- `local_attack::tests::only_launch_type_ultimates_present_their_activation_sound_locally` asserts `plays_cast_sound_locally` is true only for Strike/Blob.
- `assets::tests::every_manifest_audio_clip_decodes_with_the_client_decoder` and catalog mapping tests cover the new rows.
- Clip numbers measured from WAV masters and `ffmpeg volumedetect` on the .ogg files.
- `just lint`: pass. `just test`: pass (exit 0; 24 suites ok, 0 failed; brawler-client lib 862 passed, network 132 passed).

## Playtest request (owner, native, ~2 min)
- Run: `just server` + `just client`, enter Practice with a build whose ultimate is Demolition Strike, then one with Big Blob.
- Controls: charge the ultimate (or use Practice charge), press ULTIMATE.
- Observe: a short radio call-in beep on the press for Strike, then the explosion exactly once on impact (no double boom); a wet low thwomp on the press for Blob. Judge loudness against fire/dash, and whether the call-in reads as "strike incoming" and the thwomp is distinct from the sticky gun.
- Known limits: remote casters hear no cast sound by design; a rejected cast still leaves its cast sound.
