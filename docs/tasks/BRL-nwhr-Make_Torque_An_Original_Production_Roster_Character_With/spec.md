# Outcome

A production model for **Torque** — Damage / Heavy, cheerful alien beetle: rounded shell shoulders, tiny head, four chunky forearms — matching the look in `asset_src/04-character_roaster.png` and the visual-language table in `asset_src/01-characters.md`. It follows the BRL-qfaj (Bumper) process: `asset_src/02-characters_design_process.md`, `asset_src/03-characters_modeling_spec.md`, and the shared clip contract.

# Notes

- The kit (weapon and options) is in the BRL-7sqn roster table. Poses and grip should suit Torque's signature weapon.
- Wiring: change only Torque's presentation entry in the client character catalog from BRL-egn6 (it replaces the placeholder model and tint).
- Evidence: gameplay-camera and front renders, a lineup beside Riff and Bumper, an animation sheet, and a native Dashboard capture. Check asset memory on Pixel.

# Implementation record (2026-11-20)

## Design: Torque

A cheerful alien beetle (Damage / Heavy) from `asset_src/01-characters.md` and the Torque cell of `asset_src/04-character_roaster.png`. Brief: `asset_src/characters/torque/spec.md` (concept, palette, silhouette, rig, animation personality).

- Silhouette: widest at the shoulders, low centre of gravity. An emerald shell dome over a tiny face, a back plate with an elytra seam, two huge round shell shoulders, darker octagonal spots, two chunky curved yellow antennae. From above: a three-lobed shell with antennae, unlike Riff's ears or Bumper's wool cloud between horn curls.
- Tiny head: a small round purple face under the dome's front with happy closed `^ ^` eyes, lavender cheeks, a small smile and two amber chin nubs (a gentle beetle cue). Face is 23% of the shell height (Riff/Bumper ~44% head share).
- Four chunky forearms: the weapon pair (navy arms, big yellow gauntlets, purple fists) on the standard chain, and a lower purple pair with yellow bracers on new bones `arm2_upper_<side>` / `arm2_fore_<side>` that rests its fists on the hips and never holds a weapon. They drum the belly in idle, pump in the run, tuck back in two-handed holds and the dash, flex in victory and pound the ground in the ultimate.
- Outfit and palette (from the concept sheet): navy body, brown harness and belt with gold buckles, big flat navy boots with yellow toe caps and cuffs. Shell #2FAE84 / spots #1A6E55, yellow #F4B92E, purple #7A5AD6, navy #2B3266, brown #7A4A2B. Greener than Bumper's teal jersey; no orange, coral or wool.
- Original: no rhinoceros horn, insect jaws, compound eyes or metallic power armour.
- Motion: low stomping waddle with shell sway, shell-first shove dash, belly-drum idle, upturned-beetle KO (tips back onto the shell, four arms and both legs waving), four-arm flex victory, four-arm wind-up and ground-pound ultimate.
- Signature weapon: Arc Launcher is the `throwable` family (two-handed lobber). Its hold is Torque's own: lower and further to the weapon side than Bumper's (`(0.10, -0.33, 0.55, 10, -36, 0)`, support clavicle 44 degrees forward), so the muzzle stays below the tiny face from the front, the gameplay camera and the native Dashboard. The face sits high (centre Z 1.13) for the same reason.

## Decisions

- Skeleton core is Bumper's (Riff's names and lengths, shoulders 0.03 wider), so the weapon socket, holds and per-frame IK carry over; accessory bones are `antenna_l_01`, `antenna_r_01` and the four `arm2_*` bones (34 bones). The shell rides `chest`, so the face turns under it and the clavicles stay free for two-handed holds; `socket_head` sits on the dome top.
- Draw scale stays 1.08 (the Heavy scale BRL-egn6 gave the placeholder). Only Torque's entry changed: `model: "torque"`, `tint: None`, plus the `torque` model row (`source_ground: 0.02`). At 1.08 he stands about 43 world units (1.33 cells) to the antenna tips: wider and taller than Bumper, below Riff's ear tips. His widest bind-pose reach (0.6307 source, the shoulder-dome spots) is 15.0 world units at that scale, the same as the Bumper placeholder it replaces and 1.0 past the 14-unit body radius; the builder fails above 0.636 source (14 units at scale 1). If the owner wants him inside 14 units, setting `scale: 1.0` in the catalog is a data-only change.
- Native shell captures: the `brawlers-picked` / `dashboard-picked` routes pick Bumper. No permanent Torque route was added; the captures used a temporary, uncommitted swap of the picked IDs in `packages/brawler-client/src/flow/automation.rs` (CharacterId 2, Fire Field 8, Quick Cycle 5), reverted afterwards (same approach as BRL-qfaj).

## Delivered

- `asset_src/characters/torque/build_torque.py` (loads `riff/build_riff.py` as the shared pipeline like Bumper's builder), `torque.blend`, `README.md`, `spec.md`, `orthographic_sheet.png`, `previews/`.
- Runtime GLB `assets/brawler/models/original/characters/torque/chr_torque.glb`.
- `assets/catalogs/fighter_characters.ron`: Torque's entry and model row only.
- Commit `6755031f` (worktree branch `worktree-agent-a5f70e0d4f0fa8b44`). `just torque-asset`; license note in `assets/licenses/pewpew-blitz-original-characters.txt`; docs/11 Characters bullet and provenance line.
- Client tests (`packages/brawler-client/src/presentation_3d/fighter_character/tests.rs`): the shipped-GLB contract test now loops Riff, Bumper and Torque and also checks each `socket_weapon_r` rests in Riff's grip frame (the weapon grip rotation); new `torque_is_drawn_with_his_own_model_untinted`. The catalog test (`the_presentation_catalog_loads_exactly_the_contract_checked_models`) covers the third model.

## Budgets (build report)

- 437,620 bytes (budget 700,000; Riff 507,684, Bumper 476,888); 7,542 triangles (budget 10,000; Riff 9,688, Bumper 9,600); one mesh, one primitive, one material, `COLOR_0`, no textures/UVs/normals.
- 21 clips (exact `CLIP_NAMES`, Riff's frame counts and loop flags), 35 channels per clip, 67 rest channels stripped per clip, 34 bones.
- Soles Z 0.02, shell dome top 1.56, antenna tips 1.817; reach 0.6307.
- `socket_weapon_r` rest rotation (0, -0.70711, -0.70711, 0), identical to Riff.
- Worst two-handed grip miss 0.0127 (`combat_idle_throwable`, `attack_throwable`; tolerance 0.02).
- Two consecutive builds give a byte-identical GLB (md5 291920605bbfacc697fa9b8794392f0f).

## Evidence

Under `asset_src/characters/torque/previews/` (key ones attached to this ticket):

- `torque-lineup.png`: between Riff and Bumper at relative in-game sizes, gameplay camera and front.
- `torque-gameplay.png`, `torque-front.png`, `torque-dashboard.png` (Arc Launcher ready), `torque-three-quarter.png` (rest), `../orthographic_sheet.png` (front/side/back/top).
- `torque-families-gameplay.png`, `-front.png`, `-side.png`: every family's combat idle, run 1/5/9, dash, attack peak.
- `torque-animation-sheet.png`: idle, run, dash, hit, ko, victory, ultimate, attack peaks.
- Native: `torque-dashboard-native.png` and `torque-brawlers-native.png`, from `BRAWLER_SHELL_CAPTURE_BIND=127.0.0.1:5181/5182 BRAWLER_SHELL_CAPTURE_DIR=target/brl-nwhr/... just shell-capture dashboard-picked|brawlers-picked 1280x720` (both passed). Torque renders with the toon material and outline, the Arc Launcher on `socket_weapon_r`, face above the muzzle, four fists visible.

## Iteration and lessons

- First pass: the shell sat low and the Arc Launcher's fat muzzle hid the tiny face from the front and the gameplay camera. Raised the face and dome (face centre 1.03 to 1.13) and gave Torque his own lower, more sideways lobber hold; the support hand then missed (0.043) until its clavicle pushed 44 degrees forward. Prevention: for a small-headed character, check the signature weapon's hold from the front before tuning anything else.
- Shell spots modelled as raised discs showed as tabs on the silhouette edges; they now sit almost flush.
- The first reach check failed (0.645, spot discs and fists): shoulder domes and fists pulled in. The builder now fails above 0.636 and names the widest parts.
- The KO transition key floated the body above the ground; lowered.

## Verification

- `just torque-asset` (Blender 5.2.3 LTS, ~30 s): pass, `TORQUE_BUILD_REPORT` as above; rebuild byte-identical.
- `cargo test -p brawler-client --lib fighter_character`: 17 passed.
- `just lint` (VS Code ripgrep on PATH): pass.
- `just test`: pass (exit 0; every suite ok, including network and the fixed-tick performance gates).
- Native shell captures: pass (above).
- Pending: Pixel asset-memory check (device not available in this session).

## Playtest

Practice 3v3 as Torque on each theme: check that the shell, antennae and face read from front, side and back at gameplay distance, that the Arc Launcher shows beside the face, the waddle run and footfall squash, the shell-first dash, the upturned-beetle KO, the four-arm victory and the ultimate. Known limits as Riff's and Bumper's (baked support hand, no reload clip); the lower arms are animation only.

## Merge verification (main, 2026-11-20)

Cherry-picked onto main. `just lint` and `just test` pass on the combined tree. Reviewed the lineup, gameplay-camera render and native Brawlers capture: Torque reads as a distinct three-lobed shell silhouette next to Riff and Bumper, with the face visible above the Arc Launcher. Pending: Pixel asset-memory check (no device connected; the phone also needs a protocol-55 APK).
