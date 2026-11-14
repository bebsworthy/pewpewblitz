# Outcome

Each weapon family reads at gameplay scale from the weapon in Riff's hands.

# Context

BRL-57hs shipped Riff with `combat_idle_<family>` / `attack_<family>` clips for `1h`, `heavy`,
`throwable`, and `melee`, but every family still attaches the shared Kenney `blaster-a` to
`socket_weapon_r`. Two-handed families only approximate the off hand; there is no left-hand IK,
reload clip, or weapon compatibility check (asset_src/characters/riff/spec.md section 34).

# Scope

- Original procedural weapon GLBs per family (or an accepted curated source), built and validated
  like Riff (`asset_src/`, `just` recipe, `assets/brawler/models/original/`).
- Client selection of the weapon model from the replicated primary weapon delivery family.
- Off-hand placement on two-handed weapons (authored poses or bounded IK).
- Optional reload clips if the economy exposes a reload state.

# Out of scope

Per-brawler character selection, cosmetics.

# Acceptance criteria

- Every shipped weapon preset shows its family weapon in gameplay and the Dashboard preview.
- Primitive fallback unchanged; outline hull cap respected.
- Native captures on all three themes and owner visual acceptance.

# Implementation record (2026-11-14)

## Decisions

- **Original procedural weapons, one per family.** `asset_src/weapons/build_weapons.py` (`just weapon-assets`, Blender 5.2.3 LTS) builds `wpn_blaster` (1h), `wpn_sprayer` (heavy), `wpn_lobber` (throwable), `wpn_blade` (melee) under `assets/brawler/models/original/weapons/`. They follow `asset_src/03-characters_modeling_spec.md` §10-12: `weapon_root` as the identity attachment origin on the primary grip, anchor empties `grip_primary`/`muzzle`/`projectile_spawn`, plus `grip_secondary` on two-handed weapons. One mesh, one material, flat `COLOR_0` palette, no normals/UVs/textures, authored in Riff source units and attached at unit scale. Builds self-validate (anchors, attributes, ≤1,500 triangles, ≤48 KB) and are byte-identical across runs. Sizes: 5.8-14.1 KB each, about 40 KB total.
- **Kenney Blaster Kit retired.** Nothing loads `blaster-a` any more, so its GLB and texture (55 KB), manifest rows, license text, and THIRD_PARTY rows are removed. Provenance for the new weapons is in `assets/licenses/pewpew-blitz-original-weapons.txt` and `asset_src/weapons/README.md`. The net APK asset change is about −15 KB (BRL-5k0g).
- **Selection is in authored client data.** `assets/catalogs/fighter_weapons.ron` (schema 1, validated in `presentation_3d/fighter_weapons.rs`) maps `WeaponFamily` to model plus shell-preview framing. The family still comes from the replicated primary delivery. There are no built-in-ID branches and no protocol or gameplay change.
- **The two-handed grip is baked, not runtime IK.** `build_riff.py` places each two-handed family's weapon in the chest frame and solves a per-frame two-bone IK at build time: the weapon hand goes onto `grip_primary` and the left hand onto the weapon's `grip_secondary`, which it reads from the shipped GLB that the catalog names. Family clips are sampled every frame, so the hands never drift between keys. The build fails if a hand misses by more than 0.02 units. The worst misses are attack_melee 0.013 and heavy 0.001, with the rest 0. Riff's arms are very short (about 0.48 units from shoulder to wrist), so the fore grips sit about 0.2 units from the primary grip and the holds turn the torso about 30° into the weapon, protract the support clavicle, and counter-turn the head. The clip names, channel count (114), sockets, and rest pose are unchanged, so no Rust Riff constants changed. `chr_riff.glb` is 625,732 bytes (budget 700 KB) and byte-identical across rebuilds.
- **Runtime.** A `HeldWeapon` component on each imported Riff scene keeps exactly one weapon instance on `socket_weapon_r` and swaps it when the family changes. `sync_held_weapons` runs in the Animate set, chained before `update_character_animation`, so a loadout that replicates late or changes still updates the model. The weapon is a `ToonScene` child, so the existing per-frame outline (one hull per mesh asset; at most 4 new mesh assets against the 64 cap), concealment, and rim paths apply unchanged. The weapon casts High-tier shadows like the body (visible in the captures). The primitive fallback (`BRAWLER_FORCE_PRIMITIVE_WORLD`) is unchanged.
- **Shell previews.** The Dashboard Riff holds the selected saved brawler's family (`selected_weapon_family`). `WeaponPreviewHost(WeaponFamily)` gives each weapon preview (equipment, brawler details weapon panel, creation overview and picker cards) its own family model. `dashboard/weapon_preview.rs` keeps at most four offscreen cameras on distinct render layers (28, 30, 32, 33), created and released per family on screen. The previews still use the arena toon material without a rim (BRL-0179).

## Verification

- Asset builds: `just weapon-assets` and `just riff-asset` pass their self-validation. Two consecutive builds produced byte-identical weapon and Riff GLBs (an early concave blade outline was nondeterministic; it is now built from explicit convex faces). Review images: `asset_src/weapons/previews/weapon-sheet.png`, `asset_src/characters/riff/previews/riff-weapons-front.png`, `riff-weapons-gameplay.png`, and `riff-animation-sheet.png`.
- New tests: `presentation_3d/fighter_weapons/tests.rs` covers catalog coverage and validation (missing, duplicate, third-party, and escaping paths), the shipped weapon GLB contract (one mesh, `COLOR_0`, identity `weapon_root`, anchors, `grip_secondary` exactly on two-handed families, byte budget), preview centring, and `HeldWeapon` swap/despawn. There is also a preview-layer distinctness test, the renamed Riff grip-facing test, and an asset test that the Kenney Blaster Kit license is retired.
- Focused: `cargo test -p brawler-client --features brawler/client --lib -- fighter_ weapon dashboard assets` passed (106).
- `just check`: pass. `just lint` (VS Code ripgrep first on PATH): pass. `just test`: pass, 2,374 tests, 0 failed (performance gates included).
- Native (patched scratch copy of `scripts/v3-render-evidence.sh`, scratch target/data directories, port 5187, `BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_COMBAT=1`, 1920x1080 logical on a 3024x1834 Retina window), one window per run:
  - Pulse Sidearm (1, `1h`), wipeout-1v1, theme 1 Crossroads Facility: pass. `…/scratchpad/brlq29d/evidence/w1.txt.captures/brawler-000330.png`, `-000570.png`
  - Spray (6, `heavy`), heist-3v3 Powderline Vault, theme 2: pass. `…/evidence/w6.txt.captures/brawler-000330.png`, `-000480.png`, `-000630.png`
  - Arc Launcher (3, `throwable`), wipeout-3v3 Verdant Crossfire, theme 3: captures OK. The report failed only `sample_count`: the window paced at 60 Hz (p50 16.6 ms, p99 19.9 ms, no frame over 25 ms) where the passing runs paced at 120 Hz (p50 8.3 ms). This is display vsync pacing, not render cost. `…/evidence/w3.txt.captures/brawler-000480.png`
  - Impact Blade (4, `melee`), hot-zone-3v3 Switchback Basin, theme 3: same 60 Hz `sample_count` failure, captures OK. `…/evidence/w4.txt.captures/brawler-000480.png`
  - The Practice bots in the 3v3 captures also hold their own families' weapons (blaster, blade).
  - (`…` = `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brlq29d`; scratch, not retained.)
- Not natively captured: the Dashboard and shell weapon previews. The automated Practice path passes the Dashboard before the first capture frame, so these need the playtest below.

## Playtest handoff

- Rebuild and restart the client: `just client` (the routed server needs no rebuild, because nothing server-side, wire-facing, or fingerprinted changed). Controls are unchanged.
- Scenario: on the Dashboard, create or select brawlers with Pulse Sidearm, Spray, Arc Launcher, and Impact Blade (Brawlers → Create → Weapon picker). Check the creation weapon cards, the brawler details weapon panel, the weapon equipment screen, and the Dashboard Riff. Then play a Practice match with each family on any map.
- Please observe: whether each family reads at gameplay scale; whether the two-handed holds and attacks look believable (hands on the fore grip/hilt, no head or ear clipping); whether the blade swing is legible; whether the outline and High-tier shadows look right on the weapons; and whether the shell previews are well framed.
- Known limitations (filed): run, dash, idle, and the Dashboard idle carry two-handed weapons one-handed (BRL-v8x4). Brawler cards and the details portrait reuse the Dashboard Riff, so they show the selected brawler's weapon (BRL-y6zs). There are no reload clips (BRL-3mqg). The `muzzle` and `projectile_spawn` anchors are not yet consumed; projectiles keep the authoritative muzzle offset.

## Learn from errors

- **zsh word splitting.** `set -- $pair` does not split in zsh, so the first batch of evidence runs failed argument validation without opening windows. Prevention: in multi-run shell loops, use parameter expansion (`${pair%%:*}`) or run `bash -c`.
- **Nondeterministic export.** A concave n-gon (the blade's edge strip) exported with varying triangulation, so GLB bytes differed between runs while the bounds and counts matched. Prevention: build procedural slabs from explicit convex faces, and include a two-build md5 comparison in every asset-builder verification. Repeat it at least three times, because one in three runs differed.
- **Chibi reach.** The first two-handed holds placed fore grips 0.25-0.33 units ahead, which is out of reach for a 0.48-unit arm. The build-time miss check caught it before any visual review. Lesson: express grips as anchors, validate the reach in the builder, and design weapon ergonomics around the character's arm length rather than real-world proportions.
- **Evidence order.** Practice evidence never captures the Dashboard. Shell-preview evidence needs either a dedicated automation capture point or the owner playtest; this is recorded above rather than claimed.

## Open for the owner

Visual acceptance of the four weapons and the two-handed holds across three themes, and the Dashboard and weapon previews. The ticket stays in `doing`.
