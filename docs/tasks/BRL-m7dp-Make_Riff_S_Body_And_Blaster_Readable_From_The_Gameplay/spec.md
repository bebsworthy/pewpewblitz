Reviewed 2026-11-15 against HEAD 3437c0d8: valid; proportions unchanged since BRL-57hs, but BRL-q29d (2026-11-14) replaced the single blaster with per-family weapons, so the evidence must be re-captured first.

# Outcome

From the gameplay camera, Riff reads as a rabbit with a visible body and weapon from every facing.

# Evidence

- BRL-57hs captures (e.g. `target/brl0154/r2-theme2-practice.txt.captures/brawler-000480.png`, still on disk): from behind or the side, head and ears cover most of the torso and the blaster; bots read as an orange block with a purple goggle band. Close-up Blender sheet reads well. These predate BRL-q29d's per-family weapon models (`assets/brawler/models/original/weapons/wpn_{blaster,sprayer,lobber,blade}.glb`).

# Context (verified)

- Proportions: `asset_src/characters/riff/build_riff.py:57-58` `HEAD_SCALE = 1.45` about the neck pivot, `EAR_LENGTH_SCALE = 0.80`; docs/11 l.32 "head about 45% of visual height", l.41 head share measured against crown height.
- Gameplay height `RIFF_VISUAL_HEIGHT_WORLD` = 1.6 cells (`fighter_character.rs:28`); camera elevation 55° (`presentation_3d/camera.rs:16`).
- Weapons: placed per family in `build_riff.py:362-388` (1h held forward at chest; two-handed in front of belly); built by `asset_src/weapons/build_weapons.py` (`just weapon-assets`, then `just riff-asset`).
- Budget: `GLB_BYTE_BUDGET = 700_000` (`build_riff.py:62`); current GLB 625,732 B.

# Plan

1. Re-capture baseline with `just v3-render-evidence` on all three themes, each family, front/side/back facings.
2. Try, in order: reduce `HEAD_SCALE` (e.g. 1.30-1.35) and/or tilt ears back in idle/run/hold poses (`EARS_BACK`); raise the 1h weapon hold toward the shoulder; scale weapons up in `build_weapons.py` (docs/11 says weapons ~1.5x oversized).
3. Rebuild (`just weapon-assets && just riff-asset`), keep the builder's grip-error and budget checks green; update docs/11 head-ratio wording if it changes.

# Acceptance

- Native captures at the BRL-xyj7 framing show torso and weapon from front, side and back on all three themes, for at least 1h and heavy families.
- Outline hull, rim, contact shadow and plates still fit; GLB ≤ 700 KB; `just test` (clip contract tests) green.
- Owner visual acceptance; Pixel 8a check of readability at phone size.


# Implementation (2026-11-18)

## Research and decisions
- Baseline (HEAD 46e9dd14) native captures confirm the evidence with per-family weapons: from behind (the most common facing in Practice: teams advance up-screen), the 1.45 head hides the torso and the held weapon, and the `run` clip's `EARS_BACK` folds the ears toward the camera, so the back view reads as an orange block with a purple goggle band. From the side the swept ears splay over the body.
- The spec's "ears back" option is wrong for this camera: Riff's ear chain already curves about 30 degrees back at rest (`ear_l_01/02/03` rest tilt 9/21/32 degrees), and under a 55-degree camera from behind a backward ear projects at cos(55+tilt), so it lies over the head. Tipping the ears forward to about upright makes them stand above the head from behind and the side; from the front they foreshorten slightly but still read.
- Iterated in Blender first with a scratch gameplay-camera facing sheet (front / 3q-front / side / 3q-back / back x 1h hold, 1h run, heavy hold, heavy run; scratch builder copies, outputs under `target/m7dp/variants/`), then confirmed natively. Variants: base; HEAD_SCALE 1.32; + ears/1h arm (v2); + weapons up (v3); + ear_02 straightened (v4); + run head lean forward (v6, chosen); HEAD_SCALE 1.25 (v7, rejected: only marginally more torso and it changes the character design). Comparison: `target/m7dp/variants/cmp-base-v4-v6-v7.png` (rows base, v4, v6, v7; columns side, 3q-back, back for hold then run).
- Heavy hold placement could not move outward: x 0.08 / 0.11 (and 0.12 with yaw -24) miss the support grip by 0.034-0.072 > 0.02 (chibi arm reach). Left as-is.

## Chosen values (smallest change that clearly fixes the read)
- `build_riff.py`: `HEAD_SCALE` 1.45 -> 1.32 (head share of crown height 0.462 -> 0.439; ears 1.16x -> 1.06x V2 length). New `EARS_UP` (ear_01 +12 deg pitch, ear_02 +10) merged into `run` (replacing `EARS_BACK`) and every `combat_idle_<family>` hold (so `attack_1h` uses the merged hold); run passing ear bounce now 2/3 deg, hold breathe ear_02 13/8. 1h hold and `attack_1h` recoil `upperarm_r` yaw 8 -> 20 and run `carry` `upperarm_r` (-48, 0, 4) -> (-48, -16, 12) so the blaster sits out to the side, clear of the head from behind. Run head pitch -6 -> +6 (leans into the stride; ears more upright from behind, shirt shows). Two-handed placements unchanged (BRL-v8x4 reuses run keys plus family placement; nothing here makes that harder).
- `build_weapons.py` scales: blaster 1.15 -> 1.4, sprayer 1.3 -> 1.5, lobber 1.25 -> 1.45, blade 1.15 -> 1.3. Grip errors: heavy 0.0012 -> 0.0179, melee 0.0134, throwable 0.0 (tolerance 0.02).
- `assets/catalogs/fighter_weapons.ron` shell close-up framing compensated exactly for the model scale (preview_scale / ratio, preview_center * ratio): blaster 0.86 (0,0.02,0.18), sprayer 0.62 (0,0.12,0.3), lobber 0.76 (0,0.12,0.29), blade 0.51 (0,0,0.44).
- Client: `RIFF_SOURCE_TOP` 2.506 -> 2.3656 (build report `source_top_z`). Visual height stays 1.6 cells including ears, so the body and weapon now render about 6% larger in game (scale 20.76 -> 22.02); body planar reach 0.6088 x 22.02 = 13.4 <= 14 radius (test green). `FIGHTER_VISUAL_HEIGHT_WORLD` (plates anchor) unchanged.
- GLB: `chr_riff.glb` 625,732 -> 624,700 B (<= 700,000), 9,688 triangles. Weapon GLBs 6,076 / 9,160 / 11,124 / 14,100 B. Builder confirmed deterministic (HEAD source rebuilt byte-identical before changes).
- Docs: docs/11 head wording now "about 44% of the crown height" plus the gameplay-camera readability rule; Riff README runtime contract updated (ear tip 2.3656, head 44%, ears up in run/holds, 1h arm out).

## Verification
- Captures: scratch copy of `scripts/v3-render-evidence.sh` with 8 captures (frames 420 + 120n, 1280x720 logical = 2560x1440 PNG), `BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_COMBAT=dummy`, weapon 1 (Pulse Sidearm, 1h) and 6 (Spray, heavy), isolated supervisors on 127.0.0.1:5071-5106, data under `target/m7dp/*.server-data`. Themes: Feature Yard (`wipeout-2v2`), Powderline Vault (`heist-3v3`), Verdant Crossfire (`wipeout-3v3`). All 24 runs `result=pass`, no panics.
  - before: `target/m7dp/before-<game-type>-w<1|6>.txt.captures/`
  - intermediate: `target/m7dp/mid-*` (v3: ears only at ear_01, still folded from behind), `target/m7dp/v4-*` (v4)
  - after (final): `target/m7dp/after-<game-type>-w<1|6>.txt.captures/`
  - per-run fighter crop sheets (every on-screen fighter projected from the capture JSON, labelled by facing in the build log): `target/m7dp/sheets/{before,v4,mid,after}-<run>.png`; side by side: `target/m7dp/sheets/compare-before-top-after-bottom.png` (back 1h bot Verdant, back 1h Powderline, back heavy Powderline, side 1h and side heavy Feature Yard, front 1h bot and front player Verdant).
- Judgement: back views now read as a rabbit (two ears up above the head) with the blaster visible beside the head on all three themes; the shirt shows as an orange band below the head (the head still covers most of the back of the torso; a smaller head would change the character). Side views: ears upright instead of splayed, torso and larger weapon visible (1h and heavy). Front views unchanged in quality (face, body, weapon). Known limit: the heavy sprayer in its two-handed hold/attack is hidden from directly behind (in front of the belly, reach-limited); it shows from side/3q-back and in the run.
- Outline hull, rim, contact shadow, and overhead plates checked in the captures: hull and rim follow the new mesh, contact shadow and team ring unchanged, plates sit above the ears with the same gap (visual height unchanged).
- Shell previews (y3hb scratch capture client + driver, assets swapped, supervisor 127.0.0.1:5111, data `target/m7dp/shell-*`): `target/m7dp/shell-after/1280x720/` vs before `scratchpad/brly3hb/after/1280x720/`. Dashboard and brawler-list card: smaller head, ears no longer crowd the top, larger blaster, framing fine. The weapon close-up in that scratch binary used its embedded old catalog (the catalog is `include_str!`), so it showed the model about 1.2x larger; the new catalog values compensate exactly (uniform scale about the grip), not natively re-captured.
- `just lint` (ripgrep on PATH): pass. `just test`: pass (exit 0; client lib 862, network 132 incl. performance gates).

## Playtest request (owner visual acceptance)
Run `just run 2` (or `just server` + `just client`) and Practice 3v3 on Powderline Vault (heist-3v3) and Verdant Crossfire (wipeout-3v3) plus Feature Yard. Try Pulse Sidearm (1h) and Spray (heavy). Observe Riff running and standing away from the camera, sideways, and toward it: ears should stand up as two rabbit ears from behind, the blaster should sit beside the head, a band of shirt should show below the head. Check that the smaller head (1.32) still feels like Riff on the Dashboard and brawler cards, that the larger weapons do not feel oversized, and the shell weapon close-ups keep their old framing. Known limits: heavy sprayer hidden from directly behind while standing/firing; two-handed run/dash/idle is BRL-v8x4. Pixel 8a: check readability at phone size.

## Mistakes / learning
- The spec's suggested "ears back" was the opposite of what the camera needs; measuring the rest ear curve before posing would have shortened the loop. Iterating in a Blender gameplay-camera facing sheet and only then running native captures kept native runs to three batches.
