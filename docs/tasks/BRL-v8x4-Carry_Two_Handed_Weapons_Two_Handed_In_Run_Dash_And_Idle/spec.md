Reviewed 2026-11-15 against HEAD 3437c0d8: valid; spec refreshed with builder/clip refs and current GLB headroom (~74 KB).

# Context (verified)

- BRL-q29d baked two-handed holds only into `combat_idle_<family>` and `attack_<family>` (`asset_src/characters/riff/build_riff.py:362-388`, per-frame IK via `Poser.hold_weapon` l.259 + `sampled` l.319). `run` (l.411-428, one-handed `carry`), `dash`, `idle`, `hit_front`, `victory`, `ultimate` are shared and hold every weapon one-handed by the grip.
- Client: `CLIP_NAMES` (15 clips, `packages/brawler-client/src/presentation_3d/fighter_character.rs:53`), `FighterClips::from_gltf` (l.136) and `node()` (l.171): `Run`, `Dash`, `Idle` are family-independent. `select_motion` (l.267) uses `Hold(family)` when stationary in a match, so in-match standing is already two-handed; the gap is Run, Dash, and the Dashboard/brawler preview which plays plain `idle` with the family weapon attached (`dashboard.rs:569`, `:605`).
- Budget: `GLB_BYTE_BUDGET = 700_000` (`build_riff.py:62`, checked l.674); current `chr_riff.glb` is 625,732 B.

# Plan

1. Builder: add `run_heavy/throwable/melee` (and `dash_*`, `idle_*` if budget allows) by reusing the run/dash/idle leg+body keys merged with the family's `weapon` placement and per-frame IK support hand; 1h keeps the shared clips. Measure GLB size per added clip; if over budget, drop idle variants first (Dashboard can use `combat_idle_<family>` instead).
2. Client: extend `CLIP_NAMES`/`FighterClips` with per-family run (and dash) nodes; `CharacterMotion::Run`/`Dash` carry the family; Dashboard preview picks family idle (or `combat_idle_<family>`) for two-handed families.
3. Update `asset_src/characters/riff/README.md` clip list and docs/11 animation notes.

# Acceptance

- Two-handed weapons keep both hands on them while running, dashing and in the Dashboard preview (builder grip error under the existing threshold for new clips).
- GLB ≤ 700 KB; `just test` (clip-contract tests) and `just lint` green.
- Native captures and owner visual acceptance.


# Implementation (2026-11-18, on HEAD dc24734e)

## Decisions
- Two-handed families confirmed from the builder/catalog: `heavy` (Spray, ConeSpray), `throwable` (Arc Launcher / Splash, Lobbed/Splash), `melee` (Impact Blade, MeleeArc); all three weapon GLBs carry `grip_secondary`. `1h` (Straight, StickyStraight) keeps the shared `run`/`dash`/`idle`.
- Builder (`asset_src/characters/riff/build_riff.py`): new `TWO_HANDED`, `carried(stride, hold, level)`, and six clips `run_<family>` / `dash_<family>` (heavy, throwable, melee). Each takes the shared `run` / `dash` keys (legs, pelvis bob, lean, ears, scarf, tail) with all arm/clavicle/aim keys removed, adds the family hold's yaw turn (spine_02/chest/head z) and clavicles, drops the hold's stance legs, places the family weapon, and takes the extra locomotion lean back out of the weapon pitch so the muzzle keeps the hold's aim. Sampled every frame (per-frame two-bone IK), like the holds. m7dp changes kept: run keys carry `EARS_UP` and head +6 lean; dash keeps `EARS_FLAT`.
- Melee dash: fully levelling the blade against the 36-degree dash lean leaves the support hand 0.054 short (> 0.02), so `CARRY_LEVEL[("dash","melee")] = 0.5` (blade tipped forward into the lunge; miss 0.0).
- No `idle_<family>`: the Dashboard, brawler-details and brawler-card previews play `combat_idle_<family>` for two-handed families (`FighterClips::preview`), `idle` for 1h.
- GLB size: the six clips first exported at 740,968 B (> 700,000). Measured cause: the exporter keeps translation+rotation+scale channels for all 38 bones in every clip; each channel costs ~170 B of glTF JSON per clip (JSON chunk 371 KB of 741 KB), so a clip costs ~19 KB fixed regardless of length. Only the pelvis ever translates (other bones within 3e-7 of rest) and scale is always 1. The builder now post-processes the export (`strip_rest_channels`): drops translation/scale channels that hold the node's rest value in every clip, from every clip alike (the validator still requires one channel set across clips, so nothing leaks), and repacks the buffer. 114 -> 39 channels per clip (`rest_channels_stripped_per_clip: 75`).
- Size per step: HEAD 624,700 B (15 clips); +6 clips unstripped 740,968 B (~19.4 KB/clip); with stripping, all 21 clips 507,684 B (~2-5 KB bin + ~6 KB JSON per added clip). 9,688 triangles unchanged.
- Client: `CLIP_NAMES` 15 -> 21; `FighterClips.run`/`dash` are per-family arrays (index 0 = shared 1h clip), `run_seconds` per family, mapping extracted to `FighterClips::from_nodes` so tests cover the real index mapping; `CharacterMotion::Run(family)` / `Dash(family)`; footfall squash reads the playing family's run clip. Dashboard: new `DashboardPreviewPlayer(Entity)`; `sync_dashboard_preview_weapon` plays `preview(family)` when it swaps the weapon model (so a family change switches the clip with the model); brawler cards play `preview(family)` at scene ready.

## Grip errors (tolerance 0.02)
combat_idle_heavy 0.0179, attack_heavy 0.0179, attack_melee 0.0134, run_heavy 0.0099, every other two-handed clip (run/dash throwable, run/dash melee, dash_heavy, holds/attacks throwable, combat_idle_melee) 0.0.

## Verification
- `just riff-asset`: pass (weapons unchanged, `just weapon-assets` not needed). Build report bytes 507,684, channels_per_clip 39, ear_top_z 2.3656 (RIFF_SOURCE_TOP unchanged), planar reach 0.6088 unchanged.
- Blender: builder sheets `asset_src/characters/riff/previews/riff-two-handed-gameplay.png`, `riff-two-handed-side.png` (rows heavy/throwable/melee; run 1/5/9/13, dash 1/5); close-up review sheets (same cells + hold) `target/v8x4/blender/two-handed-{front34,back34,gameplay}.png`. Both hands on the grips through the stride and the lunge for all three families; from behind the weapon is hidden by the body as in the hold (known m7dp limit).
- Native gameplay (scratch copy of `scripts/v3-render-evidence.sh` with configurable capture first/every/count; Practice `wipeout-2v2`, weapon preset 6 Spray; isolated supervisors 127.0.0.1:5141/5143, data under target/ deleted after the runs; both `result=pass`, no panics):
  - running (BRAWLER_RENDER_COMBAT=0, move right, no fire): `target/v8x4/native/run-wipeout-2v2-w6.txt.captures/` (12 frames), player crop sheet `target/v8x4/native/run-heavy-player.png` — sprayer forward in both hands, legs striding, ears up.
  - dashing (COMBAT=dummy, captures every 6 frames from 1500; three Dash activations, 17 Dashing frames by ability phase in the capture JSON): kept frames in `target/v8x4/native/dash-wipeout-2v2-w6.txt.captures/`, player crop sheet `target/v8x4/native/dash-heavy-player.png` — ears flat, sprayer held low forward in both hands during the lunge.
- Shell (temporary BRL-y3hb-style evidence driver compiled into a scratch client binary, then removed from the tree; it creates a brawler with weapon 1/6/3/4 and screenshots details, list, Dashboard; supervisor 127.0.0.1:5131-5134): `target/v8x4/shell-{1h,heavy,throwable,melee}/`; side by side `target/v8x4/dashboard-families.png` (1h idle; heavy, throwable, melee in their two-handed hold) and `target/v8x4/cards-families.png`.
- `just lint` (ripgrep on PATH): pass. `just test`: RESULT.

## Playtest request (owner visual acceptance)
`just run 2` (or `just server` + `just client`). Create/select brawlers with Spray (heavy), Arc Launcher or Splash (throwable), Impact Blade (melee), and Pulse Sidearm (1h). Dashboard, brawler list and details: two-handed weapons in both hands (ready pose), 1h in the calm idle; switching the selected brawler switches the pose with the weapon. In Practice: run in all directions and Dash (default ultimate) — both hands should stay on the two-handed weapon; check the blade's forward tip in its dash and that the run lean/ears still read from behind. Known limits: hit, KO, victory and ultimate clips still hold every weapon by the grip; from directly behind the two-handed weapon is hidden by the body. Pixel 8a: check the run reads at phone size.

## Mistakes / learning
- Assumed the support-hand reach was invariant to chest pose (weapon and shoulders both in the chest frame); the IK pole is in armature space, so the melee dash lean changed the reach. Check per-clip grip errors before review renders.
- First size estimate used only accessor bytes (~31 KB for six clips); the GLB JSON chunk, not animation data, is the cost driver. Measure the JSON chunk when budgeting glTF clips.


- Correction to the line above: `just test`: pass (exit 0; client lib 863 tests, network 132 incl. performance gates). Logs `target/v8x4/lint.log`, `target/v8x4/test.log`.
