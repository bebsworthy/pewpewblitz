# Outcome

The game sounds finished, and hits feel confirmed by ear.

# Context

`assets/brawler/audio/` contains five clips (defeat, error, fire, impact, ready). `assets/catalogs/audio_profiles.ron` maps about 16 cues onto them. There is no music and there are no UI sounds.

# Scope

- Per-delivery-family fire sounds.
- Hit dealt vs. hit taken confirmation.
- Ultimate ready and activate, pickups, objective events.
- UI click/confirm/back.
- Match start, victory, and defeat stings.
- Menu and match music loops with volume settings.
- Mapping through the existing audio catalog and request boundary.
- Licenses recorded.

# Out of scope

Voice lines and adaptive music systems.

# Acceptance criteria

- Every gameplay and UI cue in the audio catalog has a distinct authored clip.
- Music loops seamlessly and respects the volume settings.
- Provenance is recorded.
- Owner playtest acceptance.

# Verification

- Audio catalog validation tests.
- A native playtest.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.

# Sourcing decision (q-000018, answered 2026-11-13)

CC0 packs only. No downloads are made by the agent. Inventory of audio already in the repository:

- `assets/brawler/audio/` ships five Kenney CC0 clips (fire, impact, defeat, ready, error) with
  manifest rows and license texts. They are retained, each now owning exactly one cue.
- `references/bevy/assets/sounds/` is read-only upstream material (one track is CC BY 4.0, the
  others are unsuitable ambient/acoustic samples), so nothing is promoted from it.

Every other clip, including both music loops, is original audio synthesized by a checked-in
Python 3 standard-library generator (`asset_src/audio/build_audio.py`, like the BRL-nzre icon
generator). It writes deterministic stereo 16-bit WAV masters and encodes them to Ogg Vorbis with
FFmpeg's built-in `vorbis` encoder (the only Vorbis decoder in the client graph is lewton via
`bevy/vorbis`; adding `bevy/wav` would download `hound`, so it is not used). Original audio is
recorded in `assets/licenses/pewpew-blitz-original-audio.txt`, the generator README, and manifest
rows with license `Original` that point at the generator instead of a URL. If the owner rejects
the generated music in the playtest, a follow-up Ticket question lists recommended CC0 tracks.

# Design

## Cue vocabulary (one distinct clip per cue key)

| Producer (rank) | Keys | Source fact |
|---|---|---|
| combat (10) | `fire-straight`, `fire-sticky`, `fire-lobbed`, `fire-splash`, `fire-melee`, `fire-spray` | local `LocalAttackPresented` primary fire / remote `AttackAccepted`, family from the source fighter's replicated `ResolvedMatchLoadout` delivery method (fallback `fire-straight`) |
| combat (10) | `impact-projectile`, `impact-lob`, `impact-melee`, `impact-spray` | `DeliveryImpact`, `LobLanded`, `MeleeContact`, `ConeSprayPulse` |
| combat (10) | `hit-dealt`, `hit-taken` | `DamageApplied` relative to the controlled fighter (initiating fighter / target), one per attack id plus a short tick spacing so sprays stay bounded |
| combat (10) | `defeat`, `respawn`, `sentry-fire`, `sentry-removed` | `FighterDefeated`, `FighterReset`, `SentryFired`, `DeployableRemoved` |
| combat (10) | `ultimate-cloak`, `ultimate-uncloak`, `ultimate-scan`, `ultimate-revealed`, `ultimate-field`, `ultimate-strike` | authoritative ultimate activation cues (local input-frame activation stays BRL-1w27) |
| heist (20) | `objective-hit`, `objective-critical`, `objective-destroyed` | unchanged |
| ability (30) | `dash`, `charge-ready`, `passive`, `sentry-spawn`, `concealment-field-spawn` | unchanged |
| reload (40) | `reload` | unchanged |
| session (50) | `playable`, `error` | playable-gate edge (was `ready`), error edge |
| match (60) | `match-start`, `match-score`, `match-victory`, `match-defeat`, `match-draw` | countdown/active entry, Wipeout score change, completion result relative to the controlled fighter's team |
| hot-zone (70) | `zone-captured`, `zone-lost`, `zone-progress` | control gained, control lost/contested, 50%/90% thresholds |
| pickup (80) | `pickup-spawned`, `pickup-collected` | `PickupCue` Spawned/Collected |
| ui (90) | `ui-click`, `ui-confirm`, `ui-back` | BRL-drny `ReleaseActivation` (pointer/touch undragged release) on enabled controls; `Surface::Primary` confirms, `Surface::Back` backs, everything else clicks |

The shared `fire`, `impact`, `ready` keys are retired so every key maps to its own clip.
`ReceivedPickupCues` becomes a per-frame inbox filled in a new `ClientPickupCueSet` and read (not
drained) by both the pickup VFX and audio producers after that set.

Keyboard/gamepad activation sounds are out of scope here (activation is screen-owned, there is no
shared semantic activation fact yet); deferred to a backlog ticket.

## Music

- Catalog schema 4 adds `music: [(key, asset, volume, loop_seconds)]` for keys `menu` and `match`.
- A small music selector maps `ClientFlow::Match` (and `MatchLoading` keeps whatever is playing)
  to `match` and every other flow to `menu`. The adapter owns one looping entity, a short
  fade-out/fade-in on change, and `PlaybackSettings::duration = loop_seconds` so encoder padding
  cannot open a gap at the loop seam. Missing/unloaded assets degrade to silence and retry when
  loaded; nothing waits on music.

## Volume settings

- `ClientShellSettings` gains `effects_volume` and `music_volume` (0–100, steps of 10; defaults
  100 and 70). The Audio page lists Master, Effects, Music, Mute when unfocused.
- The two new persisted fields use `#[serde(default)]` so an existing settings file (which has no
  schema version) still loads instead of being rejected and losing bindings. Unknown fields stay
  rejected.
- Every client audio entity carries an `AudioMix { category, base }`; one mix system (moved from
  `shell.rs` into `client/audio`) sets sink volume = master × category × base (× music fade) and
  applies focus mute. This also fixes the existing override where the master volume replaced each
  profile's authored volume after the first frame.

## Boundaries

All new code stays under `src/client/` (client feature). Catalog stays client-only, outside the
gameplay fingerprint; `scripts/check-server-features.sh` keeps guarding bevy_audio/rodio/lewton out
of the server graph.

## BRL-zy9d coordination

`hit-dealt`/`hit-taken` are in this ticket's scope ("hit dealt vs hit taken confirmation"), so the
audio half of BRL-zy9d lands here; BRL-zy9d keeps the healing-number half. Noted on BRL-zy9d.

# Implementation (2026-11-13)

- Catalog `assets/catalogs/audio_profiles.ron` schema 4: 46 cue keys, one profile and one distinct
  clip each, plus `music` (`menu` 19.2 s loop, `match` 30 s loop). Producers: combat (per-family
  fire from the shooter's replicated `ResolvedMatchLoadout`, impacts per delivery cue,
  `hit-dealt`/`hit-taken` with 4-tick spacing and one per attack id, ultimates), heist, ability,
  reload, session (`playable`), match (start/score/victory/defeat/draw relative to the controlled
  team), hot zone (captured/lost/progress), new `pickup` (rank 80) and `ui` (rank 90) producers.
- `src/client/audio/music.rs` (selector from `ClientFlow`, 0.8 s crossfade, ≤2 fading tracks,
  `PlaybackSettings::duration = loop_seconds`), `src/client/audio/mix.rs` (`AudioMix` category ×
  base, replaces `shell.rs::apply_audio_preferences`, which also used to overwrite authored profile
  volumes with the master volume).
- Settings: `effects_volume` (default 100) and `music_volume` (default 70), Audio page rows,
  keyboard/gamepad/pointer slider actions, scoped reset; new persisted fields `#[serde(default)]`.
- `ClientPickupCueSet`: `ReceivedPickupCues` is now a per-frame inbox read (not drained) by pickup
  VFX and audio.
- Manifest validator accepts license `Original` only with `asset_src/` generator/source paths and
  an `assets/licenses/pewpew-blitz-original-*` notice.
- Docs: `docs/11` audio section, `docs/13` settings sentence, `docs/backlog.md` GAP-AUDIO-SETTINGS.

# Assets and provenance

- Retained Kenney CC0 (unchanged files): `fire.ogg` → `fire-straight`, `impact.ogg` →
  `impact-projectile`, `defeat.ogg` → `defeat`, `ready.ogg` → `playable`, `error.ogg` → `error`.
- New original, generated by `asset_src/audio/build_audio.py` (stdlib Python, FFmpeg 9.0.2
  built-in vorbis, bit-exact; rebuild is byte-identical), in `assets/brawler/audio/original/`,
  43 files, ~1.0 MB total: 41 one-shots of 4.3–38.8 KB (largest `match-victory` 38.8 KB) plus
  `music-menu.ogg` 180.8 KB and `music-match.ogg` 303.2 KB. Notice:
  `assets/licenses/pewpew-blitz-original-audio.txt`; README `asset_src/audio/README.md`; one
  manifest row per file (license `Original`). No downloads were made.

# Verification

- Focused tests (all pass): distinct clip per cue and per music track; every delivery family has a
  fire/impact cue and bounded hit/UI caps; music keys exactly covered; every manifest audio file
  ships and decodes with the client's lewton decoder at 44.1 kHz; music loops decode to their
  authored length (lewton yields exactly 19.2 s / 30.0 s); manifest `Original` provenance and
  forged-entry rejection; per-family fire selection (local + remote via replicated loadout); hit
  dealt/taken relative to the controlled fighter with spacing; completion stings per team/result;
  hot-zone captured/lost/progress; pickup spawned/collected/expired/stale-generation; UI
  release-activation by surface role with disabled controls silent (ECS test); music selection,
  fades, asset-wait silence and bounded crossfade (ECS test); category mix math; settings
  round-trip, pre-category settings file loads with defaults, invalid music volume rejected; Audio
  page rows, slider steps and scoped reset.
- `just lint` (real ripgrep first on PATH) — pass, including `check-server-features.sh` (no
  bevy_audio/rodio/lewton in the server graph).
- `just test` — pass (lib 884, server 579/529, network 107, performance, routing).
- `just e2e 2` — pass (1 exact 1v1 roster reached Active).
- Native (scratch supervisor/client data dirs; `data/` and `config/` unchanged): windowed client
  with Practice Wipeout 1v1 automation + bounded render report (shell → loading → match, 25 s), and
  a plain windowed Dashboard client for 25 s. Client logs: `client assets ready`, no audio, decoder,
  asset-degraded, or settings-load warnings/errors (the owner's existing settings file, lacking the
  new fields, loaded cleanly). Audibility was not observable by the agent.

# Playtest request

Run `just run 1` (or `just balance-lab` for Practice) and listen:
- Menu music on Dashboard/Settings; crossfade to match music on entering a match and back to menu
  at Results. Listen through at least one full loop (19.2 s menu, 30 s match) for any click or gap
  at the seam. Is the generated music acceptable, or should CC0 tracks be sourced instead?
- Settings → Audio: Master, Effects, Music sliders each scale only their part; Music 0 silences
  music only; focus-loss mute still works.
- Fire each weapon family (straight, sticky, lobbed, splash, melee, spray): distinct launch sounds;
  impacts differ by delivery; a bright tick when you hit someone, a low thump when you are hit;
  sprays should not buzz.
- Ultimates (cloak/uncloak, scan, revealed, field, strike, sentry spawn/fire/removed, dash),
  charge-ready, reload, respawn, defeat.
- Objectives: Heist safe hit/critical/destroyed, Hot Zone captured/lost/progress, Wipeout score;
  match start, victory vs defeat vs draw stings; pickup spawn/collect.
- Pointer/touch clicks on shell buttons: confirm on primary (Play), back on Back, click elsewhere.
- Overall balance: anything too loud, harsh, or annoying on repetition.

Known limitations: keyboard/gamepad activation has no UI sound (BRL-ek8p); local ultimate
activation still sounds on the authoritative cue (BRL-1w27); hit sounds are not reduced by the
reduced-effects setting; one Effects category covers UI and combat.

# Feedback disposition

Awaiting owner playtest.

# Learn from errors

- Mistake: assumed `ffmpeg`'s built-in vorbis encoder accepted mono; it is stereo-only and the
  first mono→stereo upmix also lowered gain by 3 dB and made a quick SNR check look broken. Prevention:
  write stereo masters directly and measure SNR on identical-channel material before judging a codec.
- Mistake: square waves with non-50% duty carried DC offsets up to −0.12, which click at clip
  start/end; caught by a per-clip analysis pass (RMS/peak/DC/seam) before wiring. Reusable lesson:
  always run a numeric analysis of generated audio, since the agent cannot listen.
- Mistake: an f32→f64 loop-length comparison failed on representation (19.2f32 > 19.2f64).
  Prevention: compare converted floats with a tolerance.
- Observation: the old focus/volume system overwrote each sink with the master volume, silently
  discarding authored profile volumes; centralizing mixing in one component-driven system fixed it.
- Deferred: BRL-ek8p (keyboard/gamepad UI sounds). BRL-zy9d annotated: its audio half landed here.


# Owner feedback (2026-11-14)

UI sounds accepted: the owner said "all UI sounds are ok" after a `just client` playtest. Per-weapon combat sounds and music still need owner feedback before this ticket closes.


Owner closed the ticket on 2026-11-14 ("mark it done"), accepting the per-weapon combat sounds and music as shipped.
