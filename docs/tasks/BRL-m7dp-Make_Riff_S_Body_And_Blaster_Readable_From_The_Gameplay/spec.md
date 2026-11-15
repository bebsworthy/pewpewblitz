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
