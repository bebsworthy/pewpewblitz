# Origin

The owner answered BRL-tkmh question q-wr6gc13 on 2026-11-18 with "Make an original character like Riff (Blender, same clip contract)". This ticket comes before BRL-tkmh.

# Scope

- An original character design that reads at the gameplay camera distance, with a silhouette distinct from Riff's ears and goggles. No likeness to existing game characters.
- Built from a builder script under `asset_src/characters/<name>/`, following `asset_src/characters/riff/build_riff.py` and its README: rig, the `CLIP_NAMES` set (including the BRL-v8x4 per-family run/dash clips), `weapon_socket` and support-hand IK onto `grip_secondary`, and `strip_rest_channels`.
- Same budgets as Riff: GLB ≤ 700 KB and a triangle count close to Riff's (9,688). Outline and preview framing come from the shared configs.
- Exported GLB under `assets/brawler/models/original/characters/<name>/`, plus preview sheets.

# Acceptance

- `just lint` and `just test` pass; the client tests' clip-name checks cover the new GLB.
- Native capture sheets of idle, run, dash and attack for all four weapon families, plus the Dashboard preview.

# Not in scope

Choosing the character per brawler belongs to BRL-tkmh, whose question q-m6n2sj1 is still open.

# Implementation record (2026-11-19)

## Design: Bumper

A stocky mountain ram (Tank / Normal, from the roster list in `asset_src/01-characters.md`) in padded teal sports gear. Brief: `asset_src/characters/bumper/spec.md` (concept, palette, silhouette, rig, animation personality).

- Silhouette: two big ridged sand-gold curled horns (coil tilted out, forward and up so it reads from front, side and the 55-degree camera) plus a cream wool cap; top-down read is "wool cloud between two C-shapes", wide rather than tall. No ears above the head, no goggles, scarf or jacket, none of Riff's orange/purple.
- Sheep cues so he does not read as a cow or bull: narrow Roman-nose muzzle bridging up between the eyes, slit nostrils, Y mouth with a grin, wool cap, collar, ankle warmers and tail. Round white eyes with pupils (Riff has tall dark bars) under low brows: stubborn but friendly.
- Palette: wool #EFE5CF, cocoa skin #94603F, muzzle #C99670, horn #DDB673, jersey teal #169C98 / #0F6A6B, yellow #F2BE2C, coral #E0533D, navy #253252.
- Original: avoids a black face on white wool, football helmet/livery and armour.
- Motion: heavy planted jog, horns-first charge dash, hoof-scuff idle, felled-log KO, flex-with-weapon-raised victory, horn-slam ultimate.

## Delivered

- `asset_src/characters/bumper/build_bumper.py`: loads `riff/build_riff.py` as the shared pipeline (Poser + per-frame two-bone IK onto `grip_secondary`, `sampled`, `carried`, family weapon catalog, `strip_rest_channels`, review weapon attachment) and owns Bumper's parts, armature, animation, joined mesh, validation and review renders. It fails the build if `socket_weapon_r`'s rest rotation differs from Riff's.
- Rig: humanoid-light core with Riff's names and lengths (shoulders 0.03 wider), accessory bones `ear_l_01`, `ear_r_01`, `tuft_01`, `tail_01`, the seven sockets; 32 bones. Support clavicle pushed 32 degrees forward (Riff 25) in two-handed holds for the wider shoulders.
- Runtime GLB `assets/brawler/models/original/characters/bumper/chr_bumper.glb`; source master `bumper.blend`; `README.md`, `spec.md`, `orthographic_sheet.png`, `previews/`.
- `just bumper-asset`; license note `assets/licenses/pewpew-blitz-original-characters.txt`; docs/11 Characters bullet.
- Client test: `shipped_riff_glb_matches_the_runtime_contract` became `shipped_character_glbs_match_the_runtime_contract`, looping Riff and Bumper (budget, one mesh/primitive/material, COLOR_0/JOINTS_0, exact `CLIP_NAMES`, weapon socket). No runtime selection wiring (BRL-tkmh).

## Budgets (build report)

- 476,888 bytes (budget 700,000; Riff 507,684), 9,600 triangles (Riff 9,688; budget 10,000), 21 clips, 33 channels per clip, 63 rest channels stripped per clip.
- Soles Z 0.03, wool crown 1.70, horn tops 1.7093; head share of crown height 0.437; widest bind-pose reach 0.6318 (Riff 0.6088).
- `socket_weapon_r` rest rotation (0, -0.70711, -0.70711, 0), identical to Riff.
- Worst two-handed grip miss 0.0078 (`dash_melee`; tolerance 0.02).
- Two consecutive builds give a byte-identical GLB (md5 cc600f40140bca426e741495d527c5d0).
- Note for BRL-tkmh: use Riff's source-to-world scale (22.016 world units per source unit). Bumper then stands ~37 units (1.15 cells) to his horn tops and his reach is 13.9 of the 14-unit radius. Normalising to 1.6 cells would put the reach at 19.3 units.

## Sheets

- `asset_src/characters/bumper/previews/bumper-families-gameplay.png`, `-front.png`, `-side.png`: one row per family (1h, heavy, throwable, melee): combat idle, run frames 1/5/9, dash, attack peak.
- `previews/bumper-animation-sheet.png` (idle, hit, ko, victory, ultimate, attacks), `bumper-dashboard.png` (Blender Dashboard-style), `bumper-gameplay.png`, `bumper-three-quarter.png`, `bumper-riff-lineup.png` (beside Riff from the gameplay camera and the front), `asset_src/characters/bumper/orthographic_sheet.png` (front/side/back/top).
- Native Dashboard: off-screen `scripts/shell-capture.sh dashboard 1280x720` (`BRAWLER_SHELL_CAPTURE_BIND=127.0.0.1:5095`, output `target/brl-qfaj/shell-capture/1280x720/dashboard.png`) with a temporary local, uncommitted `RIFF_ASSET_PATH` swap to the Bumper GLB; reverted afterwards. Bumper renders with the toon material, outline and the blaster on `socket_weapon_r`. Native in-match captures were not taken: no selection wiring yet (BRL-tkmh).

## Verification

- `just bumper-asset` (Blender 5.2.3 LTS, ~50 s): pass, `BUMPER_BUILD_REPORT` as above.
- `cargo test -p brawler-client --lib fighter_character`: 11 passed.
- `just lint` (VS Code ripgrep on PATH): pass.
- `just test`: every suite passed except the 11 fixed-tick performance gates in the first run (p95 22-40 ms vs 16.7 ms) at host load average ~20 from parallel work; the change is assets plus a test only. Rerun of `cargo test --locked -p brawler-client --features network-test --test performance`: 12 passed.

## Iteration and lessons

- First pass read bovine (brown face, broad flat snout): narrowed the muzzle into a Roman-nose bridge, added the Y mouth and a full wool cap. Prevention: judge species read on the front render, not the brief.
- Two-handed weapons moved forward for the barrel chest made the support hand miss (0.06): kept Riff's placements and pushed the support clavicle further forward instead.
- Bevelled cylinder face strokes cost 164 triangles each (mouth was 984): face strokes are now plain thin tubes. Prevention: count triangles per part before tuning.
- The victory flex put the blaster across the face; the weapon hand now aims it skyward.
- The glTF importer's bone-shape sphere and an unassigned action slot spoiled the first Riff lineup; the builder hides the shapes, assigns the slot, and removes Riff from the saved blend.

## Playtest (after BRL-tkmh)

Once a brawler can choose Bumper: Practice 3v3 on each theme, all four families. Check horns and wool read at gameplay distance from front, side and back, that the weapon shows beside the head, the run cadence and footfall squash, the charge dash, victory and KO. Known limits as Riff's (baked support hand, no reload clip).
