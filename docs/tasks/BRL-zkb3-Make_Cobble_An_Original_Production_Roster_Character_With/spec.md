# Outcome

A production model for **Cobble** — Tank / Heavy, little stone guardian: giant block hands, square torso, tiny legs, moss tuft — matching the look in `asset_src/04-character_roaster.png` and the visual-language table in `asset_src/01-characters.md`. It follows the BRL-qfaj (Bumper) process: `asset_src/02-characters_design_process.md`, `asset_src/03-characters_modeling_spec.md`, and the shared clip contract.

# Notes

- The kit (weapon and options) is in the BRL-7sqn roster table. Poses and grip should suit Cobble's signature weapon.
- Wiring: change only Cobble's presentation entry in the client character catalog from BRL-egn6 (it replaces the placeholder model and tint).
- Evidence: gameplay-camera and front renders, a lineup beside Riff and Bumper, an animation sheet, and a native Dashboard capture. Check asset memory on Pixel.

# Implementation record (2026-11-20)

## Design: Cobble

An ancient little stone guardian (Tank / Heavy) from `asset_src/01-characters.md` ("almost comically rectangular, with a tiny happy face set into an enormous stone head") and the Cobble cell of `asset_src/04-character_roaster.png`. Brief: `asset_src/characters/cobble/spec.md` (concept, palette, silhouette, rig, animation personality).

- Silhouette: square everywhere, the only roster fighter made of straight edges. An enormous chiselled block head (0.94 wide, 0.70 deep, 0.68 tall; wider than the body) on a square torso, giant block fists hanging at knee height, tiny legs under a hip block. From above: a green moss square with a two-leaf sprout, unlike Riff's ears, Bumper's wool between horn curls, or Torque's three-lobed shell.
- Tiny happy face, flush on the flat front: dark square eyes with a light glint under a carved brow ledge, a small curved smile. Eyes sit high (Z 1.27) so the weapon never covers them.
- Moss tuft as hair: faceted lumps thickest at the front like a fringe, dripping down the left side and back, plus a leaf sprout on its own bone (`sprout_01`) that bobs, droops and streams.
- Giant block fists (0.37 x 0.34 x 0.36): four lighter knuckle ridges, finger seam, thumb block; the weapon fist carries a carved square-spiral rune (from the concept's carved fist), the other wears moss. Blue wrist and ankle wraps, a blue collar band with a gold octagonal medallion (concept).
- Weathered stone: irregular flush chips of darker and lighter stone and a crack on the head; nothing sticks out of the block outline.
- Palette: stone #9A9487 / light #B8B1A2 / dark #6F695E, moss #6FA83A / #93C850 / #4C7A2A, leaf #9DD452, blue #2F74B8, gold #EDB52F, dark legs #5A5961. The stone is a cool warm-grey so it does not read as Riff's peach fur.
- Original: no glowing cracks or lava seams, boulder-pile body, idol or tiki face, or armour.
- Motion: heavy stomping trot with a statue rock and pendulum fists, head-first battering-ram dash, knuckle-knock idle, struck-statue flinch, KO that wobbles back then topples forward stiff and lands face-down with the tiny feet in the air, weapon-to-sky victory with a fist pump and heavy hop, ultimate that raises both fists beside the head and hammers them down.
- Signature weapon: the Impact Blade (`melee`). Bumper holds it up across his chest in a coiled guard; Cobble's is his own rigid low guard: both block fists clamp the hilt at the hip on the weapon side (`(0.22, -0.30, 0.54, -16, 40, 0)`, chest turned 24 degrees into it, head turned back to the front), the blade jutting forward, out and down with its flat to the sky. It stays below and beside the face from the front, the gameplay camera and the Dashboard, and keeps the weapon fist visible past the head's edge at the gameplay camera (a centred hilt was hidden under the head). The attack heaves the blade up beside the head, then chops it down in front.

## Decisions

- Skeleton: Bumper's core (Riff's names and lengths, shoulders 0.03 wider), but the arm chain hangs steeper with the same bone lengths and hand direction (hands at X 0.37 instead of 0.54), and the weapon sockets keep their offset from the hand. That keeps the giant fists near the body radius; `socket_weapon_r` still rests in Riff's grip frame (checked by the builder and the client test). One accessory bone (`sprout_01`); 29 bones. The head block rides `head`, so it turns back to the front in twisted holds.
- Lobber (`throwable`) sits lower than Bumper's (`(0.10, -0.28, 0.54, 6, -36, 0)`, support clavicle 48 degrees forward) so its muzzle stays under the face.
- Draw scale stays 1.08 (the Heavy scale BRL-egn6 gave the placeholder, as Torque's). Only Cobble's catalog entry changed: `model: "cobble"`, `tint: None`, plus the `cobble` model row (`source_ground: 0.02`). At 1.08 he stands about 37 world units (1.15 cells) to the head top and 43 (1.34 cells) to the sprout tips; his widest bind-pose reach (the fists, 0.6031 source) is 14.3 world units, 0.3 past the 14-unit body radius and less than Torque's 15.0. The builder fails above 0.636 source. `scale: 1.0` would put him inside 14 units if the owner wants that (data-only change).
- Triangles: 3,508, well under Riff's 9,688. Chiselled blocks need few faces and more facets would soften the stone read; the budget headroom is left unused on purpose.
- Native shell captures: a temporary, uncommitted swap of the picked IDs in `packages/brawler-client/src/flow/automation.rs` (CharacterId 3, Demolition Strike 6, Tenacity 6), reverted afterwards (same approach as BRL-qfaj and BRL-nwhr).

## Delivered

- `asset_src/characters/cobble/build_cobble.py` (loads `riff/build_riff.py` as the shared pipeline, like Bumper's and Torque's builders), `cobble.blend`, `README.md`, `spec.md`, `orthographic_sheet.png`, `previews/`.
- Runtime GLB `assets/brawler/models/original/characters/cobble/chr_cobble.glb`.
- `assets/catalogs/fighter_characters.ron`: Cobble's entry and `cobble` model row only (the tinted Bumper placeholder is gone).
- Commit `a82ffbf4` (worktree branch `worktree-agent-a521d020f79660737`). `just cobble-asset`; license note in `assets/licenses/pewpew-blitz-original-characters.txt`; docs/11 Characters bullet, roster-presentation line and provenance line.
- Client tests (`packages/brawler-client/src/presentation_3d/fighter_character/tests.rs`): the shipped-GLB contract test now covers Riff, Bumper, Torque and Cobble (including `socket_weapon_r` in Riff's grip frame); Torque's untinted-model test became `roster_characters_with_their_own_model_are_drawn_with_it_untinted`, covering Torque and Cobble.

## Budgets (build report)

- 290,180 bytes (budget 700,000); 3,508 triangles (budget 10,000; Riff 9,688, Bumper 9,600, Torque 7,542); one mesh, one primitive, one material, `COLOR_0`, no textures/UVs/normals.
- 21 clips (exact `CLIP_NAMES`, Riff's frame counts and loop flags), 29 bones.
- Soles Z 0.02, head top 1.585, sprout tips 1.845; reach 0.6031 (fists; limit 0.636).
- `socket_weapon_r` rest rotation identical to Riff's (build fails otherwise).
- Worst two-handed grip miss 0.0006 (`attack_melee`; tolerance 0.02).
- Two consecutive builds give a byte-identical GLB (md5 5aca739be2578ba624259ab3d621ff66).

## Evidence

Under `asset_src/characters/cobble/previews/` (key ones attached to this ticket):

- `cobble-lineup.png`: beside Riff, Bumper and Torque at relative in-game sizes, gameplay camera and front.
- `cobble-gameplay.png`, `cobble-front.png`, `cobble-dashboard.png` (Impact Blade ready), `cobble-three-quarter.png` (rest), `../orthographic_sheet.png` (front/side/back/top).
- `cobble-families-gameplay.png`, `-front.png`, `-side.png`: every family's combat idle, run 1/5/9, dash, attack peak.
- `cobble-animation-sheet.png`: idle, run, dash, hit, ko, victory, ultimate, attack peaks.
- Native: `cobble-dashboard-native.png` and `cobble-brawlers-native.png` (1280x720, `just shell-capture dashboard-picked|brawlers-picked` with the temporary automation swap above, output under `target/brl-zkb3/`). Cobble renders with the toon material and outline, the Impact Blade on `socket_weapon_r`, face clear above the blade.

## Iteration and lessons

- A centred two-fist hilt in front of the chest disappeared under the head block at the gameplay camera. The low guard moved to the hip on the weapon side, with the chest turned into it and the head turned back to the front. Prevention: for a character whose head is wider than the body, check the signature hold from the gameplay camera first, not just the front.
- Giant fists on Bumper's arm chain pushed the reach well past the body radius; hanging the arm chain steeper with the same bone lengths (and keeping the sockets' offset from the hand) fixed it without touching the grip frame.
- The Lobber hold had to sit lower than Bumper's so its muzzle stays under the tiny face.

## Verification

- `just cobble-asset` (Blender 5.2.3 LTS, ~30 s): pass, `COBBLE_BUILD_REPORT` as above; rebuild byte-identical.
- `just lint` (VS Code ripgrep on PATH): pass.
- `just test`: pass (exit 0; 24 suites, 2,594 tests, 0 failed, including network and the fixed-tick performance gates; no serial rerun needed).
- Native shell captures: pass (above).
- Pending: Pixel asset-memory check (device not available in this session).

## Playtest

Practice 3v3 as Cobble on each theme: check that the block head, tiny face, moss tuft and sprout read from front, side and back at gameplay distance; that the Impact Blade low guard stays below the face and the weapon fist shows; the stomping trot, head-first dash, statue flinch, face-down topple KO, fist-pump victory and the hammer-down ultimate. Check whether the 1.08 Heavy scale (fists 0.3 past the body radius) feels right or should drop to 1.0 (data-only). Known limits as the other production fighters (baked support hand, no reload clip); the two fists overlap on the short hilt.

## Merge verification (main, 2026-11-20)

Cherry-picked onto main. `just lint` and `just test` pass on the combined tree (clean rebuild). Reviewed the lineup, gameplay-camera render and native Dashboard capture: Cobble reads as a distinct square stone block with a visible face, moss and sprout, blade held at the hip. Pending: Pixel asset-memory check.
