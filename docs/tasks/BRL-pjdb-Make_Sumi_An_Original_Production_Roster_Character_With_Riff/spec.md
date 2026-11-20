# Outcome

A production model for **Sumi** — Controller / Normal, ink-squid street artist: tentacle hair, asymmetric jacket, ink ribbons — matching the look in `asset_src/04-character_roaster.png` and the visual-language table in `asset_src/01-characters.md`. It follows the BRL-qfaj (Bumper) process: `asset_src/02-characters_design_process.md`, `asset_src/03-characters_modeling_spec.md`, and the shared clip contract.

# Notes

- The kit (weapon and options) is in the BRL-7sqn roster table. Poses and grip should suit Sumi's signature weapon.
- Wiring: change only Sumi's presentation entry in the client character catalog from BRL-egn6 (it replaces the placeholder model and tint).
- Evidence: gameplay-camera and front renders, a lineup beside Riff and Bumper, an animation sheet, and a native Dashboard capture. Check asset memory on Pixel.

# Implementation record (2026-11-20)

## Design: Sumi

An ink-squid street artist (Controller / Normal) from `asset_src/01-characters.md` ("tentacle 'hair,' asymmetrical jacket, animated ink ribbons orbiting slowly around the body"; "tentacles and ink ribbons can spread slightly when using controller abilities and collapse tightly around her while running"), the modelling spec's Sumi idle notes, and the Sumi cell of `asset_src/04-character_roaster.png`. Brief: `asset_src/characters/sumi/spec.md`.

- Silhouette: a tall violet squid mantle tipped back 24 degrees over a big lilac chibi head, a darker rim band, light spots and two dark arrowhead fins near its point; four fat tentacle locks with pink suckers inside each curl hang from the back of the head (big and low curls on her left, a medium one on her right, a long one down her back) and two short locks frame the face. Asymmetric charcoal jacket with mint trim (long slanted left tail, popped left collar with mint lining, flat right collar, left sleeve full with a mint cuff, right sleeve pushed up over a bare forearm and mint sweatband) over a white tee with a violet ink-drop logo; charcoal trousers (right leg rolled into a mint cuff, paint splat on the left thigh), violet high-tops with white soles and mint laces, a magenta spray can at the left hip, a paint splat on the jacket back. Two magenta ink ribbons (brush strokes swelling to a droplet, two beads beside each) orbit her on their own bones.
- Face: dark eyes with glints under slanted dark-violet brows, a lopsided smirk, two mint paint stripes on her left cheek: cool and a little smug.
- Palette: skin lilac #C9A0EE, mantle/tentacles violet #9C4FD8 (dark #6A2DA8, spots #D3A2F4), suckers #F6B8E6, jacket #25222E, mint #3FE3C5, tee #F2EEF6, ink magenta #D24FE0 and violet #7A2CC8, trousers #363143, sneakers #8A44D2. Distinct from the other seven: Torque's purple is a blue-violet face and limbs under an emerald shell, Sumi's violet is redder and is her whole head mass; nobody else is charcoal + white + mint. Lilac, white, mint and the charcoal edge stand out on the shell's blue background (checked in both native captures).
- Original: no squid-kid ink tank, no tentacle ponytail with a cap and goggles, no eye mask.

## Decisions

- **Skeleton:** Riff's production core exactly (positions, names, parenting, head bone to Z 1.64, sockets), as Pollen and Orbit; his family holds, IK reach and socket frame carry over. Bumper's wider clavicles were not needed for the Normal read (the jacket and the mantle give the mass). Accessory bones: `mantle` (upper half and fins), `tentacle_01_01` .. `tentacle_04_02` (the modelling spec's `tentacle_NN_MM` naming, two per lock), `ribbon_l`, `ribbon_r` (on the pelvis, heads on the body axis, so a yaw orbits the ribbon round her). 39 bones. Every part rigidly bound to one bone.
- **Secondary motion within the bone budget:** each lock drifts in pitch and roll on its own phase and integer cycle count per loop (no seam) in idle and every ready loop; the ribbons make one full slow orbit in idle and victory (22.5-degree keys), sway +-28 degrees round her back and sides in the ready loops (clear of the weapon in front), sweep in tight behind her in run (yaw -62/+58) and dash (-80/+76), fling forward in the hit, sag to the ground in the KO, and wrap in then fling out and up for the ultimate. The mantle wobbles a few degrees (trails back in run/dash, nods in idle).
- **Mantle vs the face:** authored upright first, it overhung the brow and hid the face from the gameplay camera and the Dashboard (only the eyes showed). Tipped back 18 degrees, then 24, about the brow (`MANTLE_TILT`), and the ready holds and run hold the chin up (head pitch -6 to -10), so the face shows from the 55-degree camera. The upper half first met the lower half edge to edge and the run wobble opened a visible crease; the upper half now starts wider and sleeves over the lower, and the wobble amplitude was halved (<= 9 degrees).
- **Fins:** first pass were rounded slabs at the mantle's sides and read as mouse ears from the front; replaced with arrowhead fins near the point.
- **Signature weapon:** Spray is `ConeSpray` delivery, so the two-handed `heavy` family (`WeaponFamily::of`). Riff's heavy hold put the sprayer's orange tank over her chin from the Dashboard angle; a grid search over the real Poser IK (scratch script, not committed) picked a lower hip hold further to the weapon side, `(0.08, -0.31, 0.53, 8, -36, 0)` with the support clavicle 60 degrees forward: miss 0.000 in every heavy clip. Pollen's lower lobber hold for `throwable`; Riff's `1h` and `melee` holds. Victory points the weapon at the sky beside the mantle (`aim_r` -75); the ultimate flings both arms wide.
- **Width vs body radius:** the builder fails above 0.636 source units (14 world units at scale 1.0). Widest bind-pose parts: the big left lock's curl 0.6146, the ribbons 0.601 / 0.599, the right hand 0.592: 13.53 world units. Ribbon orbit radius 0.56 with the band's face leaning out 40 degrees from vertical, so it shows from above instead of edge-on (a vertical first-pass band at 0.585 was thin from the gameplay camera). Locks thickened 1.2x after the first renders (the concept's are fat; the first pass looked spindly).
- **Draw scale:** 1.0, the Normal scale (Bumper's) BRL-egn6 gave her placeholder. About 42 world units (1.33 cells) to the mantle point. Only Sumi's catalog entry changed (`model: "sumi"`, `tint: None`, `scale: 1.0`) plus the `sumi` model row (`source_ground: 0.02`).
- **Tests:** the tinted-placeholder test (`arena_fighters_wait_for_their_character_and_wear_its_model_tint_and_scale`) moved from Sumi to Prism (CharacterId 8, Riff placeholder), keeping the tint and scale-transform checks and adding explicit `tint.is_some()` and scale 0.96 asserts. `SHIPPED_CHARACTER_ASSETS` (contract and catalog model-set tests) and `roster_characters_with_their_own_model_are_drawn_with_it_untinted` include Sumi.
- **Native captures:** temporary, uncommitted swap of the picked IDs in `packages/brawler-client/src/flow/automation.rs` (CharacterId 7, UltimateDefinitionId 7 Cryogenic Field, PassiveDefinitionId 7 Cryogenic Insulation), reverted afterwards.

## Delivered

- `asset_src/characters/sumi/build_sumi.py` (loads `riff/build_riff.py`), `sumi.blend`, `README.md`, `spec.md`, `orthographic_sheet.png`, `previews/`.
- Runtime GLB `assets/brawler/models/original/characters/sumi/chr_sumi.glb`.
- `assets/catalogs/fighter_characters.ron`: Sumi's entry and the `sumi` model row only.
- `just sumi-asset`; license note in `assets/licenses/pewpew-blitz-original-characters.txt`; docs/11 Characters bullet, roster-presentation line and provenance line.
- Client tests (`packages/brawler-client/src/presentation_3d/fighter_character/tests.rs`) as above.

## Budgets (build report)

- 564,604 bytes (budget 700,000); 8,714 triangles (budget 10,000; Normal 6,000 to 12,000); one mesh, one primitive, one material, `COLOR_0`, no textures/UVs/normals.
- 21 clips (exact `CLIP_NAMES`, Riff's frame counts and loop flags), 39 bones, 42 channels per clip (75 rest channels stripped per clip; the ribbons keep a small lift translation).
- Soles Z 0.02, mantle point 1.948; head + mantle 51% of the height; reach 0.6146 source = 13.53 world units at 1.0 (limit 0.636).
- `socket_weapon_r` rest rotation (0, -0.70711, -0.70711, 0), identical to Riff's.
- Worst two-handed grip miss 0.0134 (`attack_melee`, Riff's melee hold); every `heavy` and `throwable` clip 0.000 (tolerance 0.02).
- Two consecutive builds give a byte-identical GLB (md5 1c4f1bedc40521f60eb63d1c985a6eac).

## Evidence

Under `asset_src/characters/sumi/previews/` (key ones attached to this ticket):

- `sumi-lineup.png`: beside Riff, Bumper, Torque, Cobble, Plunk, Pollen and Orbit at relative in-game sizes, front (top) and gameplay camera (bottom).
- `sumi-gameplay.png`, `sumi-front.png`, `sumi-back.png`, `sumi-dashboard.png` (Spray ready), `sumi-three-quarter.png` (rest), `../orthographic_sheet.png` (front/side/back/top).
- `sumi-families-gameplay.png`, `-front.png`, `-side.png`: every family's combat idle, run 1/5/9, dash, attack peak.
- `sumi-animation-sheet.png`: idle orbit and weight shift, run, dash, hit, KO, victory, ultimate gather/burst/hold, Spray ready and attack, `1h` and `throwable` attacks.
- Native: `sumi-dashboard-native.png`, `sumi-brawlers-native.png` (1280x720, `BRAWLER_SHELL_CAPTURE_BIND=127.0.0.1:5231|5232 BRAWLER_SHELL_CAPTURE_DIR=target/brl-pjdb-brawlers|-dashboard ./scripts/shell-capture.sh brawlers-picked|dashboard-picked 1280x720`, both passed, with the temporary swap above). Sumi renders with the toon material and outline: the violet mantle, smirking lilac face, curls, charcoal jacket with mint trim and a magenta ribbon read clearly on the blue background; Spray sits at the hip below the face.

## Iteration and lessons

- A tall hat-like mantle overhangs the face from a high camera even when it is flush from the front: tilt it back about the brow and lift the chin in the holds. Prevention: render the gameplay camera before detailing any big headpiece.
- Splitting one shell across two bones needs a sleeve overlap (upper part wider at the joint) and modest rotations, or the wobble opens a crease.
- Rounded side flaps on a pointed cap read as ears; make fins pointed and near the tip.
- Ribbon or sash geometry that is vertical reads edge-on from the gameplay camera; lean its face outward.
- Big-headed chibis plus two-handed weapons: grid-search a lower hold over the real IK (as Pollen and Orbit did for the lobber); the support clavicle pushed forward makes the short arm reach.
- `v1` primitives keep their location on the object, so a mesh-space transform must be wrapped in the object's basis (first tilt put the mantle spots 0.67 out and the reach check caught it).

## Commits (worktree branch `worktree-agent-a33cc43ce2e6e8d66`)

- `f9c81851` model, builder, previews and wiring; `2b83fe44` native captures; `13f64052` rustfmt of the moved test.

## Verification

- `just sumi-asset` (Blender 5.2.3 LTS, ~30 s): pass, `SUMI_BUILD_REPORT` as above; rebuild byte-identical.
- `cargo test -p brawler-client --lib fighter_character`: 17 passed.
- `just lint` (VS Code ripgrep on PATH): pass (first run caught rustfmt on the moved test; fixed).
- `just test`: pass (exit 0; 24 suites, 2,594 tests, 0 failed, including network and the fixed-tick performance gates; no serial rerun needed).
- Native shell captures: pass (above).
- Pending: Pixel asset-memory check (device not available in this session).

## Playtest

Practice 3v3 as Sumi on each theme (`just server` / `just client`, pick Sumi on the Brawlers screen): check that the mantle, side curls and face read from the gameplay camera; Spray stays at the hip below the face in the hold, run, dash and spray; the locks drift and the ribbons sway in the ready loop, sweep in behind her in the run and dash; the flinch, the spin-and-flop KO with ribbons sagging, the hip-pop victory with the ribbon whirl and the ultimate's ink spread. Known limits: baked support hand, no reload clip; locks and ribbons are rigid segments; in the plain `idle` orbit (one-handed weapons only) a ribbon passes in front of the weapon; the concept's tentacle wrapping the support grip is not modelled; legs can pass through the long left jacket tail at stride extremes.

## Merge verification (main, 2026-11-20)

Cherry-picked the three commits onto main. `just lint` and `just test` pass. Reviewed the lineup, gameplay-camera render and native Dashboard capture: Sumi reads as a violet squid mantle with tentacle hair, a smirk and magenta ink ribbons, distinct from the other seven and clear on the blue Dashboard. Pending: Pixel asset-memory check; motion seen only in stills.
