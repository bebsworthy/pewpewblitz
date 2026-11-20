# Outcome

A production model for **Prism** — Controller / Light, crystalline alien: diamond head, floating crystal fragments, no mouth — matching the look in `asset_src/04-character_roaster.png` and the visual-language table in `asset_src/01-characters.md`. It follows the BRL-qfaj (Bumper) process: `asset_src/02-characters_design_process.md`, `asset_src/03-characters_modeling_spec.md`, and the shared clip contract.

# Notes

- The kit (weapon and options) is in the BRL-7sqn roster table. Poses and grip should suit Prism's signature weapon.
- Wiring: change only Prism's presentation entry in the client character catalog from BRL-egn6 (it replaces the placeholder model and tint).
- Evidence: gameplay-camera and front renders, a lineup beside Riff and Bumper, an animation sheet, and a native Dashboard capture. Check asset memory on Pixel.



# Implementation record (2026-11-20)

## Design: Prism

A curious crystalline alien (Controller / Light) from `asset_src/01-characters.md` ("Diamond-shaped head, floating crystal fragments, no visible mouth, expressive glowing eyes"), the modelling spec's Prism example (section 48: "Prism may have floating crystal hands"), the design-process note on floating meshes parented to bones, and the Prism cell of `asset_src/04-character_roaster.png`. Brief: `asset_src/characters/prism/spec.md`.

- Silhouette: a big faceted aqua diamond head (hexagonal section stretched across, a broad flat front facet for the face, a short chin point and a taller crown point leaning back-right; one smaller crystal grows from its upper left back so it reads as a crystal cluster, not a teardrop). A pearl shoulder cape flaring from a gold collar to a gold trim, a violet gem in a gold setting at its front and a crystal spike off each shoulder; an indigo sash with a small violet gem; a pearl bell skirt with a gold hem, an indigo front panel edged in gold and a fringe of six crystal points under the hem. Floating crystal hands (a palm crystal through the grip, two finger shards and a thumb, a floating gold bracelet) and floating crystal feet (inverted points). Two pairs of crystal fragments orbit the body and a halo of two small shards circles the head.
- Face: no mouth. Dark navy eyes with a big and a small glint on the front facet, dark brow chips (left raised and tilted, right level): curious, a little puzzled. Expression otherwise comes from the head (tilts in idle and victory, cocked in the ready holds, tipped back in the hit) and three white sparkles on the upper facets.
- Palette: crystal #22C3EC / light #6FE3FA / deep #127FC0 / glow #BDF6FF (per facet), eyes #141B3F, pearl #E2DAF4, indigo #3A2F7C, gold #F2B53A, violet gem #8E3FE8 / #C79BFF. Distinct from the other eight: Plunk is a deeper cobalt body, Orbit off-white and graphite with a small cyan jet, Torque emerald and purple, Sumi violet; nobody else is aqua with gold. The light aqua head and pearl robe stand out on the blue Dashboard, with the dark eyes, deep-aqua lower facets, gold and indigo as edges (checked in both native captures).
- Original: no gem set in a humanoid face, no crystal armour suit, no glowing core in a transparent body.

## Decisions

- **Skeleton:** Riff's production core exactly (as Pollen, Orbit and Sumi), so his holds, IK reach and socket frame carry over. Accessory bones: `fragment_l`, `fragment_r` (on the pelvis, heads on the body axis, so a yaw orbits each pair, Sumi's ribbon approach) and `halo` (on the head axis). 31 bones, every part rigid on one bone.
- **Floating hands and feet:** Orbit's approach. The clavicle, upper-arm and forearm bones carry no geometry, so the crystal hands land on the grips through Riff's holds and IK; each hand is authored at Riff's hand size, scaled 1.3 about the palm and pulled 0.04 toward the body. The thigh and shin bones carry no geometry and each foot bone carries a pointed crystal, so Riff's stride reads as light floating steps (rather than Orbit's legless hover jet, which would make two hovering silhouettes).
- **Per-facet palette:** crystals are built with several materials (`gem` and `diamond_head` set a material per face); `build_runtime_mesh` writes each face's colour into `COLOR_0` from its material index, still one mesh, one primitive, one material. The face band is the light tone so the dark eyes contrast.
- **Signature weapon:** Sticky Blomb is `StickyStraight` delivery, so `1h` (`WeaponFamily::of`). Riff's one-handed hold; the free crystal hand floats up and open. Two-handed families reuse Sumi's low hip hold (`heavy`) and Pollen's low lobber hold (`throwable`), both grid-searched on this same arm chain, so the face stays clear above them; Riff's melee hold.
- **Face clearance:** the first idle carried the right hand bent up (Orbit's floating-hand rest), which stood the weapon upright beside the face in idle; the right hand now rests lower (`upperarm_r (-14, -16, 0)`, `forearm_r -16`). Victory raises the weapon out beside the diamond (`upperarm_r (0, -100, 0)`, `aim_r -52`) after `(0, -125, 0)` / `-75` put it over the head's upper right.
- **Width vs body radius:** the builder fails above 0.6625 source units (14 world units at 0.96). Widest bind-pose part: the right crystal hand 0.597 source = 12.6 world units. Fragments orbit at 0.46 to 0.50, halo at 0.47.
- **Draw scale:** 0.96, the Light scale BRL-egn6 gave its placeholder (Pollen's, Orbit's). Feet points at source Z 0.02 are `source_ground`. About 39 world units (1.24 cells) to the crown point. Prism's entry is now `model: "prism"`, scale 0.96, plus the `prism` model row.
- **Triangle count:** 2,004, below the modelling spec's Light guide range (5,000 to 10,000) and well under the 10,000 budget. Faceted crystals need few triangles and the spec puts silhouette quality over density; adding triangles would not improve the read. Recorded here rather than padded.

## Tint decision (removed)

Checked every use of the per-character tint after Prism's entry moved to its own model: the catalog `tint` field had no `Some` entry left; the unknown-ID fallback was already untinted (`tint: None`); the arena (`presentation_3d/mod.rs`) and Dashboard/Brawlers preview (`dashboard.rs`) only inserted `ToonSceneTint` when the look had a tint; nothing else (ui_gallery, vfx_lab, Balance Lab, bots) read it. The other "tints" in the client (weapon team tint, relationship rim tint, floor decal tints, world-look grading, toon shadow tint) are unrelated mechanisms. So, per AGENTS.md (no speculative extension points), this ticket removed the tint support:

- `assets/catalogs/fighter_characters.ron`: no `tint` field (the schema is `deny_unknown_fields`, so a reintroduced `tint` is rejected); header comment updated.
- `fighter_characters.rs`: `tint` removed from the source entry, `CharacterLook`, the fallback and validation. Tests rewritten: `every_launch_character_draws_its_own_shipped_model` (each roster character's model key equals its own key and the GLB ships), `unknown_characters_fall_back_to_riff_at_scale_one`, and `catalog_rejects_unknown_models_bad_scales_and_out_of_tree_paths` (the bad-tint case became a bad-scale case). The placeholder-follows-body test was removed: no placeholders remain.
- `toon.rs`: `ToonSceneTint` removed; the converted-material cache is keyed by the source material only (as before BRL-egn6). Its test `tinted_scenes_convert_to_their_own_multiplied_material` removed.
- `presentation_3d/mod.rs`, `dashboard.rs`: no tint insertion.
- `fighter_character/tests.rs`: the tinted-placeholder arena test (which used Prism) became `arena_fighters_wait_for_their_character_and_wear_its_model_and_scale`, still on Prism (CharacterId 8): it now checks that Prism draws `chr_prism.glb` at scale 0.96 with the expected transform. `every_roster_character_is_drawn_with_its_own_model` covers all nine; `SHIPPED_CHARACTER_ASSETS` (contract and catalog model-set tests) has nine entries.
- Docs: docs/11 roster-presentation bullet and Dashboard preview line, docs/13 Brawlers line, `ResolvedMatchLoadout::character_id` doc comment.

If a future character needs a recolour, it should come with its own model or a typed, demonstrated need (a skin system), not a revived generic multiply tint.

## Delivered

- `asset_src/characters/prism/build_prism.py` (loads `riff/build_riff.py`), `prism.blend`, `README.md`, `spec.md`, `orthographic_sheet.png`, `previews/`.
- Runtime GLB `assets/brawler/models/original/characters/prism/chr_prism.glb`.
- `assets/catalogs/fighter_characters.ron`: Prism's entry, the `prism` model row, no tints.
- `just prism-asset`; license note in `assets/licenses/pewpew-blitz-original-characters.txt`; docs/11 Characters bullet, roster-presentation bullet and provenance line; docs/13.
- Tint removal and tests as above.

## Budgets (build report)

- 303,476 bytes (budget 700,000); 2,004 triangles (budget 10,000); one mesh, one primitive, one material, `COLOR_0`, no textures/UVs/normals.
- 21 clips (exact `CLIP_NAMES`, Riff's frame counts and loop flags), 31 bones, 34 channels per clip (59 rest channels stripped per clip; the fragments keep their lift translation).
- Feet points Z 0.02, crown 1.86; head 50% of the height; reach 0.5972 source = 12.62 world units at 0.96 (limit 0.6625).
- `socket_weapon_r` rest rotation (0, -0.70711, -0.70711, 0), identical to Riff's.
- Worst two-handed grip miss 0.0134 (`attack_melee`, Riff's melee hold); every `heavy` and `throwable` clip 0.000 (tolerance 0.02).
- Two consecutive builds (one with renders, one `--no-renders`) give a byte-identical GLB (md5 f81f59e710bbe50a2757b62299765553).

## Evidence

Under `asset_src/characters/prism/previews/` (key ones attached to this ticket):

- `prism-lineup.png`: all nine fighters, Prism in the middle, at relative in-game sizes, front (top) and gameplay camera (bottom).
- `prism-gameplay.png`, `prism-front.png`, `prism-back.png`, `prism-dashboard.png` (Sticky Blomb ready), `prism-three-quarter.png` (rest), `../orthographic_sheet.png` (front/side/back/top).
- `prism-families-gameplay.png`, `-front.png`, `-side.png`: every family's combat idle, run 1/5/9, dash, attack peak.
- `prism-animation-sheet.png`: idle orbit and head tilts, run, dash, hit, KO (air, spin, landed), victory, ultimate gather/burst/hold, Sticky Blomb ready and attack, `heavy`/`throwable`/`melee` attacks.
- Native: `prism-dashboard-native.png`, `prism-brawlers-native.png` (1280x720, `BRAWLER_SHELL_CAPTURE_BIND=127.0.0.1:5241|5242 BRAWLER_SHELL_CAPTURE_DIR=target/brl-xb9x-brawlers|-dashboard ./scripts/shell-capture.sh brawlers-picked|dashboard-picked 1280x720`, both passed) with a temporary, uncommitted swap of the picked IDs in `packages/brawler-client/src/flow/automation.rs` (CharacterId 8, UltimateDefinitionId 4 Reveal Scan, PassiveDefinitionId 7 Cryogenic Insulation), reverted afterwards. Prism renders with the toon material and outline: the aqua diamond with its dark eyes, the pearl cape with gold trim, the crystal hands, feet and fragments read clearly on the blue background; the Sticky Blomb sits in the right crystal hand below the face.

## Iteration and lessons

- First pass (pale #4FD3EE/#A9F1FF crystal, narrow 0.335 head, small hands and fragments, long white robe) read as a grey-white teardrop in a dress and too thin in the lineup, and the white robe echoed Orbit. Saturated the crystal tones, widened the head to 0.375, scaled the hands 1.3x, enlarged the fragments and halo, shortened the robe to a hem at 0.38 with a crystal fringe under it, made the robe pearl-lilac. Prevention (Orbit's lesson again): check mass and colour in the lineup before detailing.
- A pair of small crystals on the head's upper sides read as ears or horns (Sumi's fin lesson); kept one, on the upper left back, so it reads as a cluster.
- Orbit's floating-hand rest pose lifts the weapon hand, which stands a one-handed weapon upright beside the face in idle; give the weapon hand its own lower rest.
- AgX review renders desaturate light cyan a lot; pick crystal tones from the render, not the hex.

## Commits (worktree branch `worktree-agent-a8dffe303f1224fb6`)

- `c0cd0090` model, builder, previews, wiring and tint removal; `b00d3e7c` native captures.

## Verification

- `just prism-asset` (Blender 5.2.3 LTS, ~30 s): pass, `PRISM_BUILD_REPORT` as above; rebuild byte-identical (also with `--no-renders`).
- `cargo test -p brawler-client --lib -- fighter_character toon`: 29 passed.
- `just lint`: pass.
- `just test`: pass (exit 0; 24 suites, 2,593 tests, 0 failed, including network and the fixed-tick performance gates; no serial rerun needed). One fewer than before: the toon tint test and the placeholder test were removed, the unknown-fallback test added.
- Native shell captures: pass (above); `automation.rs` reverted, nothing of the swap committed.
- Pending: Pixel asset-memory check (device not available in this session).

## Playtest

Practice 3v3 as Prism on each theme (`just server` / `just client`, pick Prism on the Brawlers screen): check that the diamond head, eyes and cape read from the gameplay camera; the Sticky Blomb sits in the right crystal hand without covering the face in the hold, run, dash and shot; the hover and head tilts, the hands circling and the fragment orbit in idle; the floating crystal steps in the run, the glide dash, the jolt hit, the KO (fragments dropping), the bobbing victory (weapon beside the diamond) and the ultimate burst. Equip other families in Balance Lab to check the two-handed holds stay below the face. Also confirm every other character still looks as before (the tint removal touched the shared toon conversion). Known limits: baked support hand, no reload clip, flat-coloured crystals (no transparency or glow), each fragment pair rigid on one bone, the raised victory weapon passes beside the upper right of the diamond.

## Merge verification (main, 2026-11-20)

Cherry-picked both commits onto main. `just lint` and `just test` pass. Native shell captures `dashboard-picked` and `brawlers-picked` (914x411) pass after the tint removal; Bumper renders unchanged. Reviewed the all-nine lineup, gameplay-camera render and native Dashboard capture: Prism reads as an aqua crystal diamond with floating shards; from the gameplay camera its pale robe sits near Orbit's white, the diamond head separates them. All nine roster characters now ship their own model. Pending: Pixel asset-memory check for the whole roster; motion seen only in stills.
