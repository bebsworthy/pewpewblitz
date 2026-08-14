# MVP stand-in asset shortlist

This is a visual shortlist for the first Brawler prototype. The goal is to test movement, aiming, combat readability, cover, objectives, and map flow—not to establish the final art direction.

The shortlist prioritizes CC0 assets. CC0 generally avoids attribution obligations, but keep the original download page and license information in the repository's asset manifest. OpenGameArt is a catalogue with pack-specific licenses, so never assume that one asset's license applies to another.

## Recommended first pass

### Provisional visual baseline

For the first draft, use **Sci-Fi Facility + Kenney Shape Characters**. This is a temporary art decision and should be replaced later without changing gameplay systems.

- Sci-Fi Facility supplies the arena's floors, walls, objects, and sci-fi context.
- Shape Characters supply readable fighter silhouettes and team-color variants.
- Colored circles or capsules remain an acceptable fallback for early combat tests.
- Final sprites must remain separate from fighter logic, collision shapes, health bars, team markers, and effects.

For the broader asset shortlist, start with:

1. [Kenney Top-down Shooter](https://kenney.nl/assets/top-down-shooter) for the arena, cover, furniture, placeholder enemies, and general props.
2. [OpenGameArt Sci-Fi Facility](https://opengameart.org/content/sci-fi-facility-asset-pack) for a more arena-appropriate facility theme and interactive objects.
3. [Kenney Shape Characters](https://kenney.nl/assets/shape-characters) or simple colored circles for fighters while combat is being tuned.
4. [Game-icons.net GUI icons](https://game-icons.net/tags/gui.html) for temporary ability, weapon, and HUD icons, with attribution.

Do not combine all packs in the same visible scene. Use one primary visual language per prototype scene; otherwise asset style differences can distract from gameplay evaluation.

## Pack comparison

| Pack | Best use | License | Formats / technical notes | Fit for Brawler |
|---|---|---|---|---|
| [Kenney Top-down Shooter](https://kenney.nl/assets/top-down-shooter) | Complete modern top-down test scene: floors, walls, furniture, characters, enemies, and props | CC0 | 2D; the OpenGameArt mirror lists separate PNGs, tile/sprite sheets, and vector files; 580 files | **Best overall starting point**. Coherent and immediately usable, though its zombie/police theme is only a placeholder. |
| [Sci-Fi Facility Asset Pack](https://opengameart.org/content/sci-fi-facility-asset-pack) | Facility arena, walls, floors, computers, gates, buttons, glowing objects, and doodads | CC0 | Pixel art; 4-direction spy and hazmat sprites; downloadable ZIP | **Best thematic fit** for an original arena shooter. Small pack, so expect to supplement it with primitives. |
| [OpenGameArt Top-Down Tileset](https://opengameart.org/content/top-down-tileset-1) | Sci-fi floor and wall test map | CC0 / public domain | 64×64 tile size; PNG tilesheet | Useful for large readable arena geometry. It is an environment sheet, not a complete kit. |
| [Devolution Topdown Tilesets and Sprites](https://opengameart.org/content/devolution-topdown-tilesets-and-sprites) | Fantasy test map, characters, enemies, effects, objects, and UI | CC0 | 16×16 pixel art; overworld, cave, and indoor sets | Broad coverage for a complete prototype, but its fantasy/Zelda-like look is far from the eventual shooter identity. |
| [Versatile 255-Tile Pixel Art Pack](https://opengameart.org/content/versatile-255-tile-pixel-art-pack) | Minimal tilemap, objects, characters, and experimental markers | CC0 | 16×16; includes PNG and Godot-oriented 3×3 minimal tilesheets | **Best utility pack** for testing map logic. Intentionally generic and visually inconsistent in places. |
| [Yohal’s Top Down Tileset Template](https://yohal.itch.io/yohals-guide) | Temporary walls, paths, stairs, trees, and a basic fighter sprite | CC0 | 16×16 pixel art; ZIP; four-direction sprite sheet | Excellent for drawing over or replacing later. It is a template, not a finished art kit. |
| [Good and Evil](https://chromoxi.itch.io/good-and-evil) | Animated placeholder fighters, weapon/no-weapon states, and two biome tilemaps | CC0 1.0 | PNG sprites, Aseprite source files; 35×35 characters; 32×32 tile grid | Good if we want readable animated characters immediately. Fantasy theme and platformer orientation make it a secondary choice. |
| [Kenney Shape Characters](https://kenney.nl/assets/shape-characters) | Temporary fighter bodies, team-color silhouettes, and hitbox readability | CC0 | 2D; 100 files | **Best pure gameplay placeholder**. Shapes make team color, status, and collision easy to read. |
| [Kenney Top-down Shooter on OpenGameArt](https://opengameart.org/content/topdown-shooter) | Alternate download page and asset inventory | CC0 | Lists 580 files, tiles, players, enemies, objects, separate sheets, and vector source | Useful as a mirror and for checking the contents before importing the official pack. |
| [Game-icons.net GUI icons](https://game-icons.net/tags/gui.html) | Temporary ability, weapon, status, and objective icons | CC BY 3.0 | SVG and PNG downloads | Very useful for HUD prototyping. Keep author attribution in a credits file or accessible credits screen. |

## Selection by gameplay need

### Tilemaps and arena geometry

- **Modern / readable:** Kenney Top-down Shooter.
- **Sci-fi:** Sci-Fi Facility Asset Pack or the listed OpenGameArt 64×64 Top-Down Tileset.
- **Pixel-art sandbox:** Devolution Topdown or Versatile 255-Tile Pack.
- **Pure layout testing:** Yohal’s template, supplemented with colored rectangles.

### Fighter sprites

- **Fastest and clearest:** Kenney Shape Characters.
- **Animated 4-direction characters:** Sci-Fi Facility Asset Pack.
- **Broader animated character set:** Good and Evil.
- **Fantasy character and enemy coverage:** Devolution Topdown.

### Object and interaction sprites

- **Furniture and general props:** Kenney Top-down Shooter.
- **Interactive sci-fi objects:** Sci-Fi Facility Asset Pack.
- **Generic pickups and markers:** Versatile 255-Tile Pack.
- **HUD and ability symbols:** Game-icons.net, with attribution.

## Practical Bevy import notes

- Keep pixel-art packs at native resolution and use nearest-neighbor texture filtering.
- Pick one world pixel scale per scene; do not mix 16×16, 32×32, and 64×64 tiles without an explicit scale plan.
- Load runtime visuals through Bevy's asset system, retain the handles for as long as the assets are needed, and gate entry into playable states until required assets finish loading.
- Keep authoritative map definitions separate from client-only sprite sheets, atlases, textures, and audio handles. The headless server must be able to load map geometry and rules without visual assets.
- Make the first built-in map a preset recipe that references stable presentation-catalog IDs. A
  future user map builder may arrange approved visuals, but arbitrary texture paths or asset handles
  must never become authoritative gameplay data.
- Represent floors and indestructible walls through authored map data plus replaceable client visuals. Keep flexible destruction in separate mask-backed terrain chunks, and keep objectives, pickups, hazards, and props as distinct gameplay entities.
- Keep decorative tiles distinct from the gameplay regions cataloged in [Environment, surface, and tile ideas](./09-environment-and-tile-ideas.md); grass art, speed markings, water, or hazard decals have no authoritative effect by themselves.
- Treat the visual sprite and collision shape as separate concerns. A temporary fighter can be a colored circle or capsule while the final sprite is undecided.
- Add team color, health bar, selection ring, and hit flash independently of the sprite. Those signals matter more than character detail during combat testing.
- Keep source ZIPs outside the runtime asset directory or in a clearly labelled `third_party/` folder, and record pack name, URL, author, license, and import date in an asset manifest.

## License checklist

- **CC0:** attribution is not required, but retaining provenance is still good practice.
- **CC BY 3.0:** attribution is required; Game-icons.net is the main item in this shortlist with that requirement.
- **CC BY-SA / GPL / OGA-BY:** defer for now unless we deliberately want to manage share-alike or copyleft obligations.
- Do not use packs marked only “free,” “royalty-free,” or “commercial use” without reading the actual license.
- Do not use ripped game assets or packs containing third-party game characters.
