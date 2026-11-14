# PewPew Blitz original audio

BRL-0158, 2026-11-13. Original sound effects and the two music loops for combat, objectives,
match flow and the shell. No third-party, purchased, sampled, AI-generated or hand-edited audio is
used; every clip is code-authored synthesis (oscillators, envelopes, filtered noise and note
events) in `build_audio.py`.

Build, from the repository root (Python 3 standard library plus an `ffmpeg` executable):

    python3 asset_src/audio/build_audio.py                  # whole set
    python3 asset_src/audio/build_audio.py ui-click dash    # selected clips
    python3 asset_src/audio/build_audio.py --wav /tmp/wav   # also keep the WAV masters
    python3 asset_src/audio/build_audio.py --list           # inventory

The script renders deterministic stereo 16-bit 44.1 kHz WAV masters (seeded xorshift noise, no
wall-clock or platform randomness) and encodes each one to `assets/brawler/audio/original/<name>.ogg`
with FFmpeg's built-in `vorbis` encoder (`-q:a 4`, bit-exact, no metadata). The set was built with
FFmpeg 9.0.2; the same FFmpeg version reproduces byte-identical files. A full build takes about
10 seconds and removes `.ogg` files in that directory that are no longer in the inventory.

Ogg Vorbis is used because the client decodes audio only through `bevy/vorbis` (lewton); the
built-in FFmpeg encoder only writes stereo, so the masters are stereo with identical channels.

## Sound contract

- Short and readable: one-shots are 0.05–2.2 s, start on their transient and end in silence
  (trailing silence trimmed, 10 ms fade).
- Loudness: every clip is normalized to a per-clip RMS target (UI −22 to −24 dBFS, gameplay −15 to
  −20 dBFS, big moments −13 dBFS) with a −1 dBFS peak ceiling. `assets/catalogs/audio_profiles.ron`
  fine-tunes per-cue volume; the player's Master/Effects/Music settings scale everything.
- Families are recognizable by timbre: fire sounds per weapon delivery family (sticky "thwup",
  lobbed "thoonk", splash slosh, melee whoosh, spray hiss), impacts by delivery, a bright two-tone
  tick for hits dealt and a dull low thump for hits taken, square-lead stings for match flow,
  triangle blips for UI.
- Music loops: `music-menu` is 100 BPM, 8 bars (exactly 19.2 s) in A minor (Am–F–C–G) with a
  pad, echoing arpeggio, soft kick and hats; `music-match` is 128 BPM, 16 bars (exactly 30 s) in D
  minor (Dm–B♭–C–Am) with four-on-the-floor drums, octave bass, stabs, and a lead from bar 9. Every
  note event wraps around the loop buffer and filters are pre-warmed across the seam, so the end
  joins the start without a click. Playback trims to `loop_seconds` to drop encoder padding.

## Inventory

Weapon fire: fire-sticky, fire-lobbed, fire-splash, fire-melee, fire-spray (fire-straight uses
the Kenney `fire.ogg`). Impacts: impact-lob, impact-melee, impact-spray (impact-projectile uses
the Kenney `impact.ogg`). Hits: hit-dealt, hit-taken. Fighters and sentries: respawn, sentry-fire,
sentry-removed (defeat uses the Kenney `defeat.ogg`). Ultimates: ultimate-cloak,
ultimate-uncloak, ultimate-scan, ultimate-revealed, ultimate-field, ultimate-strike. Abilities:
dash, charge-ready, passive, sentry-spawn, concealment-field-spawn, reload. Objectives:
objective-hit, objective-critical, objective-destroyed, zone-captured, zone-lost, zone-progress.
Match: match-start, match-score, match-victory, match-defeat, match-draw. Pickups:
pickup-spawned, pickup-collected. UI: ui-click, ui-confirm, ui-back, ui-move. Music: music-menu,
music-match. (`playable` and `error` use the Kenney `ready.ogg` and `error.ogg`.)

Add a clip by defining a render function and a `CLIPS` row in the script, rebuilding it, adding an
`audio.original.<name>` row to `assets/manifest.ron`, and mapping a cue key or music key to it in
`assets/catalogs/audio_profiles.ron`. Provenance: `assets/licenses/pewpew-blitz-original-audio.txt`.
