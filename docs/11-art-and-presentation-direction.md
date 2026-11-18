# Art, presentation, and asset specification

## Purpose

This document defines Brawler's durable visual direction, presentation ownership, readability
language, and runtime-asset policy. It describes the established 3D presentation foundation and the
constraints that future art must preserve; versioned implementation documents record when and how
individual parts were delivered.

Exact redistributed third-party files, licenses, and provenance belong to `assets/manifest.ron`,
not to this document. Every advertised built-in resolves stable map visual identities through
`assets/catalogs/map_asset_visuals.ron` and `assets/catalogs/map_presentation_themes.ron`, paired
with the shared definitions in `content/catalogs/` and authored recipes in `content/maps/`. These
machine-readable sources may evolve without turning this specification into a duplicate inventory.

## Visual direction

PewPew Blitz is a bright, energetic, toy-like arena shooter. Chunky voxel-toy characters fight
across saturated, crafted arenas under stylized lighting. Every hit, charge, and victory gets
immediate visual and audio feedback. Readability of the fight always comes first: style adds
character and energy, but it never hides footprints, relationships, objectives, or threats.

The direction was decided in BRL-0140 (2026-10-23). It replaces the earlier matte/restrained world
stance and the cream/teal/orange shell palette. The values below are starting points. Implementing
tickets tune them through native review but do not change the direction.

### Characters

- Chunky voxel-toy figures in the style of the roster concept
  (`asset_src/04-character_roaster.png`). Riff (`asset_src/characters/riff/`) is the first
  production character.
- Exaggerated, readable proportions: the head is about 44% of the crown height (see below) and
  the weapon about 1.5× oversized. Proportions and poses are judged from the 55° gameplay camera,
  not a close-up: torso and weapon must stay visible beside the head from front, side, and back
  (BRL-m7dp). Each character has one strong silhouette and saturated primary colors on
  clothing.
- Visual height is about 1.6 map cells. Visual scale stays independent of the canonical 14-unit
  authoritative radius and its ground ring (see [Fighter identity](#fighter-identity)).
- Expressive motion: idle, run, per-family hold and shoot with recoil, hit reaction, dash, defeat,
  and victory, plus procedural squash and stretch on footfalls, firing, and landing.
- Riff is built by checked-in procedural Blender scripts (`asset_src/characters/riff/build_riff.py`,
  `just riff-asset`), and his runtime GLB lives under
  `assets/brawler/models/original/characters/riff/`. For a long-eared character the head share is
  measured against the crown height (ears excluded) and the 1.6-cell visual height includes the
  ears. The source README owns the export contract and clip list.
- Each primary-weapon animation family has its own original held weapon (BRL-q29d): a one-handed
  blaster (`1h`), a two-handed heavy sprayer (`heavy`), a two-handed lobber (`throwable`), and a
  two-handed impact blade (`melee`), built by `asset_src/weapons/build_weapons.py`
  (`just weapon-assets`) under `assets/brawler/models/original/weapons/`. The authored client
  catalog `assets/catalogs/fighter_weapons.ron` maps family to model and shell-preview framing;
  nothing branches on built-in weapon IDs. Two-handed holds are baked into Riff's family clips with
  the support hand on each weapon's `grip_secondary` anchor; no IK runs at runtime. Two-handed
  weapons stay in both hands while standing, firing, running, and dashing (per-family run and dash
  clips, BRL-v8x4) and in the Dashboard, brawler-screen, and brawler-card previews, which play the
  family's ready loop instead of the calm one-handed `idle`. Hit, defeat, victory, and ultimate
  clips still hold every weapon by the grip alone.

### World shading

- **Toon ramp.** A shared stylized material with a two-band light ramp. The shadow side is about
  0.65× albedo and hue-shifted toward blue/purple.
- **Rim light.** Strength about 0.35. On fighters it is tinted with the relationship color.
- **Outline.** A dark navy outline (`#0E1638`) scaled to world size, applied to every fighter.
  Outlines follow observer-specific concealment exactly. The client draws it as a depth-tested
  inverted hull on each fighter mesh (smoothed normals, extruded in screen space by a world-unit
  width, flat colour, no shadows), so world geometry in front of a fighter still hides it and it
  never shows through walls. A locally concealed ally shows no outline over its translucent body.
  Width (arena and shell preview), colour, and an optional relationship tint live in
  `assets/catalogs/fighter_outline.ron`.
- **Post-processing.** HDR with a filmic tonemapper and restrained bloom. Only emissive projectiles,
  muzzle flashes, and pickups bloom.
- **Color grading.** Saturation about ×1.15–1.25, applied per theme.
- **Grounding.** Every fighter, projectile, and prop is grounded by a contact shadow or baked
  ambient occlusion. Static walls and indestructible blockers use the floor's baked darkening;
  everything else gets a soft blob contact shadow directly beneath it (no SSAO).

### Floors and environments

- **Floor.** A two-tone tile surface aligned to the authoritative grid. The two tones differ by 8%
  value or less, and the floor stays lighter and less saturated than the props standing on it.
- **One kit per theme.** Each theme uses a single coherent environment kit with a shared bevel
  size, a top-face highlight, and baked ambient occlusion. Third-party kits are not mixed within
  one theme.
- **Foliage.** Concealing foliage uses chunky, readable bush forms with a gentle sway (a slow
  bend about the base; held still under reduced effects).

### Feedback and readability

- Relationship cues stay independent of model, theme, and skin color: green for the controlled
  fighter, blue for allies, red for enemies.
- Hits, healing, and defeats have immediate feedback: a short flash on the target, floating
  numbers, a defeat pop, and matching audio. Reduced-effects mode keeps required information but
  removes decorative excess.
- Objective boundaries stay exact, and allowed or blocked previews stay unmistakable.

### Product shell and HUD

- The shell uses a saturated blue gradient, indigo panels, a single orange primary action, and a
  cyan accent. White Lilita One type with a navy stroke is used everywhere, including the in-match
  HUD.
- Lilita One covers ASCII and Latin-1 only. The client enables Bevy's `system_font_discovery`, so
  any other glyph (extended Latin, Greek, Cyrillic, CJK, emoji) in a player name or other text
  falls back per script to the platform's own font instead of a missing-glyph box. No fallback font
  is bundled, so APK size is unaffected.
- Fallback glyphs match the heavy sans display face (`ui::fonts`): display `TextFont`s request
  bold, which selects each fallback family's heavy face while Lilita One's single face renders
  unchanged (Bevy applies no synthetic bold), and a startup system replaces platform script
  fallbacks that are serif or have no heavy face (macOS: Helvetica Neue for Latin/Greek/Cyrillic,
  Hiragino Sans for Han/kana, Apple SD Gothic Neo for Hangul). Android keeps its sans `fonts.xml`
  fallbacks.
- Surfaces read as pressable: a vertical gradient, a light top band, and a darker bottom lip.
- Motion is energetic and always optional under reduced-motion settings.
- [Player-shell design](design/player-shell/README.md) owns the tokens, compositions, and motion
  contract.

### Reference and mockup rule

A mockup used as an acceptance reference must show real in-engine captures or real models from the
project's pipeline. Generated hero art that the pipeline cannot produce is inspiration only. The
reference board for this direction is attached to BRL-0140.

### Adoption status

The direction is adopted incrementally. Until each owning ticket lands, the shipped client keeps
the previous presentation for that area:

| Area | Owning ticket |
|---|---|
| HDR, tonemapping, bloom, color grading | BRL-0146 (adopted) |
| Tile floor | BRL-0147 (adopted) |
| Wall-adjacent floor decals | BRL-0178 (implemented; owner playtest pending) |
| Camera framing re-evaluation | BRL-0148 (adopted) |
| Overhead plates and aim lane | BRL-0149 (adopted) |
| Toon material and rim | BRL-0150 (adopted) |
| Fighter outlines | BRL-0151 (adopted) |
| Contact shadows / AO | BRL-0152 (adopted) |
| One environment kit per theme | BRL-0153 (theme 1), BRL-0187 (theme 2), BRL-0188 (theme 3) |
| Riff and the character animation set | BRL-0154 (adopted) |
| Touch-first HUD | BRL-0155 (implemented; owner playtest pending) |
| Shell surfaces, palette, motion | BRL-0156 (adopted) |
| Icon set | BRL-0157 (adopted) |
| Combat feedback | BRL-0144 |
| Audio | BRL-0158 |

Curated third-party models, audio, fonts, icons, and source art remain replaceable foundation
content. Original art builds the PewPew Blitz identity while preserving gameplay proportions,
anchors, relationship semantics, authoritative-shape agreement, accessibility, and degraded
fallbacks.

## Presentation model

Brawler presents authoritative planar gameplay through a client-owned 3D scene:

```text
server-owned gameplay                  client-owned presentation
Vec2 position / planar rotation   ->   X/Z ground transform + render-only height
Avian 2D collision                ->   Mesh3d / imported GLB scenes
resolved 2D map shapes            ->   exact-footprint 3D/procedural geometry
replicated combat state/cues      ->   animation, effects, audio, projected UI
```

The gameplay camera is fixed, tilted, and uses restrained perspective. There is no player orbit,
authoritative height, jumping, or walkable wall top. The depth buffer owns visual occlusion; render
height, animation, particles, audio, and UI never feed back into simulation.

The accepted gameplay framing (BRL-0148) targets approximately 12 map cells vertically. At the
reference aspect the camera uses a 30-degree vertical field of view, 55-degree elevation, and
566-unit distance, so a fighter stands about 11% of the screen height and its face and front read.
The default native gameplay content area is 1591×720 (approximately 2.21:1, matching the supplied
mobile reference), while explicit window-size overrides and live resizing remain supported. At the
default aspect the conservative visible ground rectangle is approximately 17.6 by 12 cells: about
4.9 cells behind the look target and 7.1 ahead, and about 21 cells wide on the fighter's row, so
the standard 320-unit Pulse Sidearm range stays on screen to either side. Longer reaches (lobs,
Splash, Sticky Blomb, and ultimates up to 20 cells) and vertical aim beyond about 5–7 cells rely on
the reach-aware aim look-ahead (below) and the aim indicator rather than the full range being on
screen. Map authors must
not widen authoritative bounds merely to compensate for camera presentation.

Visible area is aspect-fair: no window shape sees more of the arena than the reference aspect on
either axis. Narrower viewports keep the reference vertical field of view and see less width.
Wider viewports keep the reference horizontal field of view, so their vertical field of view, and
therefore their visible depth, shrinks. Viewport auto-aim only considers on-screen targets, so the
same rule bounds its reach.

Camera fit and follow are decided independently for each axis from the presented map dimensions and
current logical viewport. Because the tilted perspective ground view is trapezoidal, the client uses
the narrow near-edge width for its conservative fit rectangle rather than claiming the wider far
edge. If a complete map axis fits, that camera axis is locked to the map center; otherwise it follows
the controlled player and clamps at the map edge so the player remains visible. Thus a 25×37
production map at the default aspect scrolls on both axes, while a map narrower than about 17
cells centers its width. Recipe-owned padding remains part of the authoritative bounds, and the camera does not change
map authority or collision.

A presentation-only camera rig (`presentation_3d/camera_rig.rs`, tuned by the client-only
`assets/catalogs/camera_rig.ron`) sits on top of that framing. It follows the controlled fighter's
presented (predicted) pose with a critically damped spring, snapping on respawn-sized jumps and map
changes, and leans toward the local aim direction. Every aim leans at least the authored minimum
(1.5 cells); when the commanded reach (the primary weapon's reach, or an armed targeted ultimate's
placement range) plus a small margin runs past the visible ground in the aim direction, the lean
grows to cover it, up to an authored maximum (5 cells, code ceiling 6) (BRL-vptw). The visible
ground is a trapezoid, so the same reach leans more toward the shallow bottom and top of the screen
than to the wide sides, and because the view's shape follows the window aspect, the lean is
computed per viewport: it only reveals up to the commanded reach, never more for a wider window.
At the reference aspect the standard Pulse reach (354 units) stays on screen to the sides with the
minimum lean, shows to within a tenth of a cell toward the top, and would show about 9.9 of its
11 cells toward the bottom, which the top HUD cap (below) shortens; longer reaches stop at the
maximum. Direction-only stick aim leans fully; mouse and placement aim scale the lean by aim distance, with full strength authored at least 1.5
times the maximum lean so a mouse cursor, which is measured through the leaning view, cannot drag
the view after it. The combined follow lag and look-ahead keeps the fighter at least an authored
safe margin (1 cell, code minimum 1) inside every screen edge, and the result then passes through
the unchanged per-axis framing clamp, so it never reveals space the framing contract forbids and a
fit axis stays locked to the map center. The one exception is a top HUD band (BRL-fwtg): when that
clamp holds the camera south of the rig's framing at the map's north edge, the camera may pass the
edge, over the outer dressing, by just enough to keep the local fighter's overhead (name, health,
and ammunition) below the top-center score plate. The band is the plate's bottom edge plus the
overhead height in UI units times the effective UI scale (phone viewport fit and the player's
setting), so it follows both. The same limit also caps the open-field lean (BRL-htdb): a south
lean or northward follow lag never puts the fighter further ahead of the look target than the band
allows, so the overhead stays below the plate everywhere. The cap applies to the look-ahead
spring's target as well as its output, so the lean settles at the cap and aim swings ease out of it
without a pop. It shortens the south lean: at 1280x720 and on the Pixel 8a landscape viewport the
5-cell south lean stops at about 2.0 and 1.7 cells, so the bottom shows less of a long south reach.
A trauma-based shake fed by local fire, local damage and defeat, and distance-scaled nearby lob
landings, barrel explosions, and defeats adds a ground-plane translation of at most a quarter cell
after framing. Local fire trauma is authored per primary-weapon delivery family with a spread-pattern
multiplier, so heavy weapons shake more than light, fast ones and player-authored weapons inherit
their family's value; local damage trauma scales with the hit's share of maximum health. Aim and
viewport auto-aim project through the unshaken view, so shake never moves aim, targeting, input, or
gameplay. The persisted Camera shake setting (Accessibility, 0–100% in 10% steps) scales only the
shake amplitude. Reduced motion disables shake and look-ahead and shortens follow smoothing; reduced
combat effects also disables shake; both override the intensity setting. The map editor camera
bypasses the rig.

Hit feedback (`presentation_3d/hit_feedback.rs`, tuned by the client-only
`assets/catalogs/hit_feedback.ron`) reads only authoritative replicated cues: fighter and deployable
damage and healing (`DamageApplied`, healing `EffectApplied`), damageable-object damage
(`WorldObjectCue::Damaged`), and Heist idol damage. Each damaging hit gives its target's presentation
root a brief white flash (about 80 ms) derived from whatever material it currently shows, so a
concealed translucent ally flashes translucent; ground team rings never flash. Each hit or heal also
feeds a fixed, bounded pool of world-anchored Lilita One numbers drawn above the overhead plates:
dealt (by the local player or its deployables), taken, healing (`+N`), and muted observed styles. Rapid
hits of one style on one target merge into one re-popping number for a bounded window, so spray and
field ticks stay legible; when the pool is full the oldest number is recycled. A cue whose target is
not replicated to this observer (such as a concealed enemy, which the server already withholds)
produces neither flash nor number. Health recovered outside healing cues also shows `+N`
(BRL-0174): a restoration pickup from the amount in its `PickupCue::Collected` (an anonymized,
concealed collector shows nothing), and passive regeneration or a Zombie boost from increases of
the fighter's replicated `CurrentHealth`. Each increase is first matched against healing already
numbered from a cue for that fighter; only the unexplained rest is held for a 250 ms attribution
window so a late cue can still claim it. Increases while the fighter is defeated, respawning,
spawn-protected, at previous health 0, or while the match is not Active are lifecycle resets and
never show a number, and decreases are never read as healing. Reduced motion removes the number pop and rise; reduced combat
effects uses a dimmer, shorter flash. Defeat feedback stays on the shared request path: the authored
`defeat` recipe adds a smoke puff and a brief ground marker at the defeat point, and every defeat
plays the defeat sound.

The world look (`presentation_3d/world_look.rs`, tuned by the client-only
`assets/catalogs/world_look.ron` and the per-theme `grading` in
`assets/catalogs/map_presentation_themes.ron`) renders the arena camera in HDR with the
TonyMcMapface filmic tonemapper. TonyMcMapface was chosen over AgX because it keeps the saturated
toy palette and team hues stable while rolling bright emissive cores smoothly to white; AgX read
flatter and pushed team blue and red toward pastel. Each theme's grading (exposure offset, white
balance, saturation, contrast) is applied with that theme's lights. Restrained additive bloom
thresholds the HDR image before grading: theme lights keep a head-on white surface below the bloom
knee, and only projectile cores, muzzle and explosion flashes, and pickups are authored above it, so
ordinary geometry, relationship rings, and area markers do not glow. Reduced combat effects removes
bloom but keeps HDR, tonemapping, and grading so colors do not shift. The HUD camera shares the HDR
target without tonemapping, grading, or bloom, so HUD colors are exact. Offscreen previews
(Dashboard brawler and card, weapon, Map Maker palette) use the same tonemapper and a preview
grading without HDR or bloom. Unlit materials ignore `emissive`, so unlit glows carry their HDR
value in `base_color` and the shared unlit palette authors no `emissive` at all. Non-glow effect
shaders (fire, poison, and restoration fields, Splash, Sticky, cone spray, and the Dash wake) keep
every authored color at or below a 2.0 peak, under the 2.25 bloom knee, so blended area, status,
and trail effects never bloom (BRL-0176); the straight projectile trail belongs to the projectile
glow set. Without HDR (the `Low` tier) Bevy sets `TONEMAP_IN_SHADER` for every material, and
the seven custom effect shaders end in the shared `assets/shaders/effect_output.wgsl` step, which
then applies the camera's tonemapper and grading to rgb (alpha kept) like `StandardMaterial` and the
toon material, so effects roll off instead of clipping at 1.0 per channel; with HDR it is a no-op
and the post pass grades them (BRL-3ebg). The fighter outline keeps its flat authored ink colour
untonemapped on non-HDR cameras.

**Quality tiers (BRL-0161).** The world presentation runs at one of three tiers authored in
`assets/catalogs/render_quality.ron` (client-only, outside the gameplay fingerprint): `High` is the
look described above (MSAA 4, HDR, bloom, arena cast shadows, foliage sway, Hanabi particles);
`Medium` turns off MSAA, bloom, and shadow maps; `Low` also drops HDR (tonemapping and grading then
run in the material shaders), foliage sway, floor decals, the player-shell background motifs
(BRL-51zd), and GPU particles. Outlines, the toon
material, and contact shadows stay on in every tier because they carry readability. The player's Settings >
Display > Graphics quality chip cycles Auto/Low/Medium/High; Auto resolves from capability detected
once the render device exists: no compute-capable GPU particles selects `Low`, Android selects
`Medium`, anything else `High`. The selected tier and its reason are logged. Code owns the ceilings
(MSAA at most 4 samples, bloom requires HDR). On Android the entry crate additionally votes 60 Hz for
the game surface, limits Bevy's compute pool to two workers, and reports each frame to an Android
Dynamic Performance Framework hint session (16.7 ms target), which is what lets the Pixel 8a hold
60 FPS; see BRL-0161 for the measurements.

The playable floor (`presentation_3d/floor_tiles.rs`) is one generated mesh with one quad per
authoritative map cell, so tile seams always coincide with the gameplay grid. Each theme's `floor`
block in `assets/catalogs/map_presentation_themes.ron` sets two close tones (validated to differ by
8% value or less) alternating as a checkerboard, a small deterministic per-cell value jitter, and
the strength of a darkening that fades over two cell corners from the arena edge and static
player-blocking walls. Destructible cover does not darken the floor, because the floor is built once
per map generation. Tones are carried in vertex colors on a white matte material, so the floor stays
an ordinary lit surface below the bloom knee and works unchanged in the primitive world.

Sparse floor decals (`presentation_3d/floor_decals.rs`, BRL-0178) add place character near walls:
cracks, scuffs, stains, leaves, sand drifts, and pebbles. Each theme's `floor_decals` block in
`map_presentation_themes.ron` picks a few sprites with a tint, a weight, a density, and a peak
opacity (validated: density at most 0.4, opacity at most 0.6, tints at most 20% brighter than the
floor's brightest tone). Placement is a pure function of the map's recipe ID and each cell, so a map
always gets the same decals in every match and in the Map Maker preview. Only open cells that touch a
static wall or the arena edge are candidates; cells under any placement (effect tiles, water, cover,
foliage, spawns), the Hot Zone circle plus half a cell, Heist safes with their two-cell attack
sectors, and Zombie ingresses are excluded, decals keep at least one empty cell between each other,
and a map has at most 160. Every decal fits inside its own cell with its top edge turned toward the
wall. All of a map's decals are one mesh drawn with one lit, alpha-blended toon material over the
generated atlas (`asset_src/environment/floor-decals/`), 0.15 above the floor and sorted before every
other transparent layer, so fields, shadows, rings, and zones always draw over them and they never
exceed the lit floor's brightness (no bloom). The `Low` quality tier hides them. The atlas ships as
a zstd-supercompressed RGBA8 sRGB KTX2 with a pre-filtered mip chain (BRL-dq06), loaded with a
trilinear sampler (the app default is nearest), so decals stay stable on phones, low resolution
scales, and zoomed-out framing. The script box-filters each 128 px sprite cell on its own,
weighting colour by alpha in linear light, and stops at five levels (8 px cells) so no level ever
averages two sprites. Mips are baked offline rather than generated at load, so the client does no
per-load work and Android reads the file from the APK like any other asset. Other client textures
get the same treatment only when they are shown to shimmer.

Fighters and the world share one toon material (`presentation_3d/toon.rs`,
`assets/shaders/toon.wgsl`, tuned by the client-only `assets/catalogs/toon_shading.ron`). It
extends `StandardMaterial`, so textures, vertex colors, alpha, and emissive keep working, and
replaces PBR direct light with a two-band ramp on the directional light: the shadow band (back
faces and cast shadows) is a hue-shifted fraction of the key light, so shadow sides never crush to
black, and the lit band keeps a little N·L so top and side faces stay distinct. There is no
specular. A lit-side rim on side silhouettes mixes toward the fighter's relationship color (green
local, blue ally, red enemy, chosen per mesh by a `MeshTag` that follows the ground ring); it mixes
rather than adds, so no lit surface exceeds a lit white surface and the bloom knee holds. World
props carry no rim by default, because under the high gameplay camera whole side faces are grazing.
Fighters ignore received shadow maps (they still cast), so faces stay in the lit band and fighters
stay readable inside wall shadows. Imported glTF instances are converted once when their scene is
ready, under a `ToonScene` root (fighters, held weapons, zombies, every environment kit instance, and
the Map Maker palette preview), and the generated theme surfaces, primitive fighters, and the
primitive barrel are built as toon materials. Hit flash and concealment variants work on both
material families. VFX, rings, markers, and status visuals keep their own materials. The Dashboard
brawler, brawler card, and weapon previews convert through the same `ToonScene` path and cached
materials (BRL-0179). The brawler and its held weapon wear the local (green) rim, because they
show the player's own brawler; the standalone weapon close-up uses the world rim, because at that
scale a side rim reads as a coloured fringe.

Grounding (`presentation_3d/contact_shadow.rs`, tuned by the client-only
`assets/catalogs/contact_shadows.ron`) is a hybrid. Static walls and indestructible blockers are
grounded by the floor's baked corner darkening. Every other object that stands on the floor gets
one soft blob contact shadow: fighters and zombies, straight projectiles (including local cosmetic
shots), sentries, Heist idols, destructible and damageable map objects, and static non-blocking
props such as concealing foliage and decorations. Lobbed projectiles keep their height-scaled
ground shadow, and pickups keep their ground glow. Effect tiles, water, markers, and visual profiles
marked `flat` in `map_asset_visuals.ron` (floor details, rubble, debris) get none. The blob is a
shared soft-disc mesh drawn just above the floor, below the relationship rings and effect
overlays, with an unlit dark navy tint per class (fighter, projectile, prop), so it never reaches
the bloom knee. It is a child of the object's visual root, so it follows the object and disappears
with it: destroyed cover loses its shadow, and a concealed enemy, which the server does not
replicate, has none. A locally concealed ally hides its shadow like its outline. Reduced effects
and the primitive world keep contact shadows. A class with opacity 0 spawns no blobs, which is the
seam for a lower device tier. SSAO was rejected because it needs depth and normal prepasses,
disables MSAA, and costs a full-screen pass for little gain under the high gameplay camera. On the
`High` tier the arena directional light also casts real shadows (BRL-0180): one shadow cascade,
derived in `camera.rs` from the gameplay framing, spans the whole visible ground (about 480-700
view units) plus one cell, so walls, props, foliage, and fighters shadow the floor away from the
camera. Effect tiles, the Hot Zone, relationship rings, and the aim lane are unlit and never
receive shadows, so cast shadows add depth without darkening gameplay signals. `Medium` and `Low`
skip the shadow pass and rely on contact shadows alone. The map editor's zoomed-out camera can sit
beyond the cascade, where its view simply loses cast shadows.

Presentation reads authoritative or replicated facts and resolves stable presentation identities.
Gameplay definitions and protocol state never carry source paths, scene-node names, mesh or
material handles, client-local entity identities, render height, or other renderer details.

## Presentation surfaces and ownership

### Gameplay world

`WorldPresentationPlugin` owns the sole gameplay-world renderer:

- the gameplay `Camera3d`, fixed projection, lighting, and selected MSAA;
- the tested simulation-XY to render-XZ conversion;
- map, terrain, fighter, projectile, sentry, objective, preview, status, debris, and cue visuals;
- imported-scene readiness, deterministic primitive fallback, animation, and weapon attachment;
- render-only entity reconciliation, cleanup, and final pose writes after interpolation;
- projection anchors for fighter overhead Bevy UI;
- optional bounded native render diagnostics.

Replicated gameplay entities remain presentation-neutral. Render entities carry explicit owner
links and reconcile idempotently across spawn, replacement, restart, reconnect, and late asset
readiness. Final transforms are written after Lightyear interpolation and Avian writeback, before
transform propagation; projected UI follows propagation.

V8 grid-map presentation derives water shores, vegetation edges, and wall joins from canonical
four-neighbor cell masks; recipes do not author visual variants or adjacency masks. V9 promoted
tall grass to explicit concealing terrain while retaining its public geometry, and whole-placement
barriers use normal or rubble replacement visuals. Imported and primitive fallback paths must
communicate the same walkability, concealment boundary, and destruction state. The accepted 32-unit
destruction granularity must never leave a smaller visual or collision speck.

V10's Feature Yard family reuses one normalized geometry across Wipeout, Hot Zone, and Heist while
presenting only the active mode's objective. Damageable barrels and chests reconcile replicated
health and terminal state; a destroyed chest disappears and its public restoration pickup uses a
distinct potion silhouette and glow. Heist presents each typed safe objective as a team idol and
pedestal fitted inside the authoritative footprint. Imported and primitive paths preserve object
class, collision footprint, team ownership, damaged-only versus persistent health treatment, and
terminal/pickup readability.

### Player Dashboard preview

The Dashboard owns a separate client-only brawler preview, its camera, lighting, entities, and
lifecycle. It may reuse the supported fighter model, attached weapon, idle animation, presentation
profile, and primitive fallback, but it does not instantiate a gameplay fighter, decide build
legality, or share entity ownership with the gameplay-world renderer.

The brawler, brawler-card, and weapon previews render toon-shaded like the arena (BRL-0179). Their
offscreen cameras stay LDR with the world tonemapper and the preview grading, so bloom never
applies. The toon material bands only directional light plus ambient, so the preview rig is one
shadowless key light plus a per-camera ambient, both from `preview_light` in
`assets/catalogs/world_look.ron`. The per-camera ambient keeps the preview independent of the last
presented map theme. There are no point lights or preview shadow maps; the toon rim takes the place
of the former coloured rim light. The Dashboard brawler and the brawler-card Riff (card list,
details portrait, creation choice card) draw the fighter outline too, held weapon included, on the
preview's own render layer (BRL-y3hb). The preview Riff is scaled about 1.8x an arena fighter and
framed close, so it uses `preview_width_world` from `assets/catalogs/fighter_outline.ron` and a
deeper hull depth push in code, which keeps face details and pockets from picking up their own dark
rings. The standalone weapon close-ups (weapon equipment, creation weapon card) stay unoutlined:
they are UI illustrations of a part, not a fighter silhouette.

### Bevy UI

Screen-space product flow, Dashboard, selection surfaces, HUD, results, settings, accessibility,
and overlays remain Bevy UI on UI-owned cameras. Fighter names, health, and local ammunition are
also Bevy UI projected from propagated gameplay-world anchors; they are not world-space text and do
not own the facts they display.

UI branding, fonts, icons, and shaders are optional client assets with bounded fallback behavior.
Navigation, focus, saving, networking, recovery, and shutdown must remain functional when a visual
asset or custom shader is unavailable.

The [player-shell UI toolkit](design/player-shell/ui-toolkit.md) owns the shared native palette,
Lilita One display type, semantic surface roles, cut-corner shader geometry, controls, and visual
interaction states. Screens select those roles and retain ownership of content and flow; they do
not recreate local button, card, list-row, toggle, focus, or scroll styling. Focus and persistent
selection remain distinct, and only the active screen or overlay presents focus feedback.

### Audio

Client audio presents bounded gameplay and product cues. Missing or late audio degrades to silence
and cannot delay authority, navigation, match readiness, or cleanup. Playback limits prevent cue
bursts from becoming an unbounded entity or channel lifecycle.

## Readability language

### Fighter identity

- the controlled fighter uses green ground-ring, facing, and name accents;
- allies use blue;
- enemies use red;
- the ring is a flat, unlit, non-shadow-receiving annulus slightly above the ground;
- a small flat arrowhead extends from the exact ring to no farther than the edge of the fighter's
  one-cell allocation; it communicates facing but is UI, not body geometry;
- imported materials, environment themes, and future skins may not override these relation cues.

The imported character's model-space forward direction is corrected once at its visual root. The
primary weapon family's model attaches to the character's `socket_weapon_r` node inside that
corrected hierarchy at unit scale, and animation-specific transforms remain below it. The model
follows the replicated loadout, so a loadout that arrives after the scene is ready swaps it; the
Dashboard brawler holds the selected saved brawler's family (switching to that family's preview
clip with the model), and each shell weapon preview shows its own weapon's family. The standard fighter has one canonical 14-unit
authoritative radius: its circular hitbox, exact outer ground-ring edge, and fallback sphere agree.
Riff is scaled uniformly from his reviewed source height so he stands 1.6 map cells (51.2 units)
including ears; that scale does not depend on the radius, and a test keeps his bind-pose feet,
hands, and torso (about 12.6 units from the center) inside the 14-unit ring. Attached weapons and
animated limbs may cross the ground circle, but must not make the visible ground occupancy
contradict it. Projected overhead placement and hit numbers derive from that visual height. The
matching sphere remains the deterministic fallback and a geometry-verification aid. The Zombie-mode
NPC keeps the Kenney mini-character with its separate footprint fit.

Character motion is chosen per frame by one priority over replicated state and cues only: defeated,
then victory (the fighter's team won the completed match), dash (ability phase Dashing), ultimate
(a Ready-to-active ability transition), attack (the existing shot trigger, per weapon family),
hit reaction (`DamageApplied` on that fighter), run (moving), and the weapon family's ready loop.
The family comes from the replicated primary weapon delivery and also selects the run and dash
clip, so two-handed weapons keep both hands on them while moving. A presentation-only,
volume-preserving squash on the imported scene root marks run footfalls, firing, and landing from a
dash or respawn; reduced motion disables it, and it never moves the gameplay pose or camera.

### Overhead information

Each live fighter has a camera-projected overhead cluster, sized in logical pixels for the
gameplay framing (a fighter stands about 80 px tall at the 720 px reference height):

- player name (15 px Lilita One) centered above the health bar and colored by relation;
- a bold 96 x 22 px rounded health bar with a 2 px navy outline, and the current health (18 px,
  white with the navy shadow stroke) centered inside it;
- relation-colored health fill matching the ground rings: green for the controlled fighter, blue
  for allies, red for enemies;
- a segmented ammunition row beneath the bar only for the controlled fighter, with one outlined
  segment per authoritative shot capacity;
- overlapping plates stack upward from their projected anchors, so floating hit numbers (spawned
  beside the body, drawn above plates) keep clear of a taller plate;
- no reserved ammunition-row height for allies or enemies;
- no cluster for defeated or off-screen fighters.

The cluster reads replicated state only. It never owns health, ammunition, defeat, names, or team
assignment.

### Combat and objectives

- straight projectiles use a compact solid whose X/Z footprint exactly matches the replicated
  `ProjectileBody`; the current circular body is a low puck rather than an elongated bolt extending
  beyond its collider;
- lobbed projectiles may use a presentation-only vertical arc while retaining authoritative ground
  position;
- a live Splash area reconstructs its replicated circle or oriented rectangle as a low translucent
  fill with a crisp raised boundary; restoration color identifies allied healing while the hostile
  effect selects a distinct Fire, Cold, Poison, or damage boundary treatment;
- Splash geometry remains fixed at its replicated landing center/facing and disappears with the
  authoritative entity. Client animation may communicate remaining lifetime or pulse cadence but
  cannot move the footprint, infer occupancy, or apply effects;
- the aim lane is one translucent ground mesh with crisp outlines, built from the resolved primary
  weapon and drawn only for the controlled fighter while it aims. Straight and sticky deliveries
  sweep the same body shape to the first client-observed contact; the fill is the fighter hit lane
  (the path from the fighter widened by projectile radius plus fighter body radius on each side, so
  a fighter standing in the lane is in the path) and an exact-width stroke (at least 3 units) marks each
  delivery's path and stop. Spread fills one fan and still strokes every emitted delivery. Lobbed
  and Splash deliveries show a ground guide and their authored landing area or Splash shape;
  melee and cone sprays fill their authored sector (cone rays keep their map occlusion). Areas,
  sectors, and cones keep the authored shape so the preview matches the effect that follows.
  Clear is white, blocked is red, targeted ultimates are violet (Demolition orange-red) with a
  thicker rim. Overlapping parts never stack translucency, and all colors stay below the bloom
  knee;
- objective boundaries remain precise and cannot depend on a decorative model or decal;
- Heist idols show persistent public team/objective health, while neutral barrels and chests show
  compact health only when damaged; the idol, chest, barrel, and restoration pickup remain
  unmistakable at gameplay scale;
- destroyed chests disappear, pickups remain visibly available until authoritative collection or
  expiry, and primitive fallbacks preserve the same footprint and state distinctions;
- slow, knockback, dash, deployable, impact, damage, defeat, and reset feedback remain distinct;
- reduced-effects mode decreases redundant effect size, lifetime, and count but never hides required
  objectives, previews, projectiles, statuses, or relationship identity.

### Composable combat VFX contract

Combat VFX is client-only presentation over gameplay facts. Gameplay and protocol types own stable
source identity, exact planar geometry, authoritative activation/deadline ticks, relationship, and
semantic effects. The client resolves those facts into a bounded visual signature and a validated
layered recipe. A renderer adapter materializes the recipe without deciding collision, hits,
damage, status application, area membership, targeting, expiry, or cleanup truth.

The production flow is:

```text
replicated state / ordered cue
            |
            v
feature-owned semantic adapter
            |
            v
bounded visual signature ---- relationship + presentation variant
            |
            v
validated recipe resolver ---- fallback + reduced/primitive policy
            |
            v
lifecycle-owned materializer -- transient / attached / persistent
            |
            v
shared meshes, materials, optional particles, diagnostics, cleanup
```

The non-transient lifecycle presentation paths remain intentionally preserved while recipe work is
sequenced. Catalogued transients use the first recipe/layer vertical slice, and every listed
lifecycle root snapshots the same typed semantic signature that production diagnostics and
VfxLab inspect. Distinctive authored lifecycle material selection is layered onto that signature;
it does not replace these ownership and cleanup paths:

| Path | Input and owner | Current materialization | Lifetime and recovery |
| --- | --- | --- | --- |
| Catalogued transients | Feature-owned `VfxRequest` from combat, world-object, pickup, and Heist cues | One recipe root with up to eight combined children: ordered mesh layers plus zero to two optional decorative GPU-particle layers; every particle layer has an authored bounded profile and a shaped retained mesh | Root lifetime is the maximum mesh or particle lifetime, or the authoritative Reveal Scan deadline; oldest-first total and per-recipe eviction; reduced recipe |
| Straight/lobbed projectile | Replicated projectile, body/flight, `AttackDelivery`, and `ReplicatedAttackSource` | Exact projectile body; Neutral straight shots wrap a dimensional nucleus in one animated comet wake, Cold adds slow crystal fins, Fire adds fast trailing lobes, Poison leaves a bubbly wake, and Restoration carries a rotating vitality spark; lobbed flight also has a projected shadow and fixed landing marker, while Fire may emit lingering world-space smoke; the nucleus carries a restrained ally/enemy relationship channel | Root follows the replicated owner and disappears with it; lob height and all flight aids derive from replicated launch/landing facts and disappear with the root |
| Elemental/concealment field | Replicated field state | Subtle fill plus exact bright boundary; Cold uses slow crystalline perimeter motion. Normal Fire uses a retained ember ground underlay plus a procedural material for a coherent turbulent circumference and layered heat bands. Normal Poison uses animated cellular contamination with perimeter bubbles; Normal Restoration uses curved, offset vitality ripples with petal markers and no cardinal-spoke icon. Pattern and motion identify type; selected native filaments, crystals, bubbles, petals, or ripple arcs carry a restrained ally-blue or enemy-red accent. Neutral leaves the pattern untinted. Reduced/primitive variants preserve the same type-pattern/team-accent separation with bounded mesh cues | Recreated after late join from public state; owner removal is cleanup truth |
| Persistent Splash | Replicated shape, facing, effects, and deadlines | Exact circle or oriented rectangle fill and boundary; circular Cold/Fire areas reuse the elemental area style | Recreated after late join; geometry remains fixed; owner removal is cleanup truth |
| Sticky payload | Replicated position, fuse ticks, radius, role, and attack source | Volatile dimensional body, animated veins and droplets, exact warning ring, and authoritative fuse-progress fill | Recreated at the observed tick and removed with the owner |
| Cone spray | Replicated origin, facing, propagation and deadline | One bounded triangle-fan surface rebuilt from blocker-clipped authoritative rays; normal/reduced use deterministic flowing gas and primitive uses the same exact silhouette | Reconstructed from authoritative tick; owner removal clears the retained mesh and dynamic mesh asset |
| Fighter status/dash | Replicated status, ability, concealment, and observed tick | Distinct attached Slow, Cold, Frozen, Fire, Poison, Reveal, and Knockback motifs; Dash uses a deterministic retained wake | Reconciled from current state and removed on status/owner lifecycle |
| Deployable/sentry | Replicated deployable state plus ordered fire/removal cues | Grounded sentry presentation; compact muzzle discharge; orderly expiry or sharper interrupted collapse | Retained root follows deployable ownership; ordered cues own transient fire/cleanup effects |

Poison and Restoration transient recipes cover muzzle, direct impact, lobbed impact, ultimate
impact, and elemental-field activation. Poison layers use central contamination accents plus
bounded bubble bursts; Restoration layers use a reusable vitality-star mesh plus bounded petal
bursts. Lobbed variants retain a thin authoritative-radius boundary. Decorative particles use
stable presentation seeds, and reduced/primitive variants omit them while preserving the accent's
mesh identity. Shared ground discs and rings use high-resolution retained meshes so large
authoritative radii do not expose a coarse polygonal silhouette.

Neutral transient recipes separately cover straight-shot launch/impact, melee sweep/contact,
damage, defeat, reset, sentry fire/cleanup, concealment activation/interruption, Reveal, and
demolition. World feedback recipes cover local world-object damage, restoration-pickup
spawn/collection/expiry, and escalating Heist safe damage/critical/destruction. These semantic
outcomes do not share one generic flash: normal variants layer compact cores, exact or expressive
ground geometry, timed echoes, and at most the bounded decorative particles required by the event;
reduced and primitive variants preserve a complete mesh-only silhouette. Pickup expiry deliberately
uses no particles so it cannot read as successful collection. Observer-relative ownership remains
a separate channel owned by BRL-0107 rather than an overlay drawn above these identities.

Aim previews remain intent UI rather than weapon VFX recipes. Imported fighter/weapon scenes and
map visuals retain their existing owners. The recipe system may reuse presentation assets, but it
does not become a second gameplay-world renderer.

#### Semantic signature

The first production signature is a typed value, not an unrestricted tag set. It contains:

- carrier: point transient, straight projectile, lobbed projectile, melee, cone, persistent area,
  attached status, sticky payload, deployable, or world/object feedback;
- phase: activation, travel, impact, active, pulse, expiry, or interruption;
- accent: neutral, Cold, Fire, Poison, Restoration, Reveal, Demolition, or another implemented
  semantic family;
- observer relationship: self, ally, enemy, or neutral;
- one optional typed contextual override owned by the source feature, such as an ultimate-specific
  activation treatment;
- typed geometry: point, direction/normal, projectile body, circle, oriented rectangle, or cone;
- stable presentation identity: combat event, attack plus delivery index, replicated field/entity
  identity, or an explicit VfxLab seed;
- authoritative activation, observed, and expiry ticks where the lifecycle is persistent;
- normal, reduced, or primitive presentation variant.

Carrier, phase, accent, and relationship remain independent. Pattern and motion communicate the
semantic type; a restrained color accent inside that native pattern communicates team. Fire heat
filaments, Cold crystals, Poison contamination/bubbles, and Restoration petals/ripples may carry
ally blue or enemy red, but relationship may not add a freestanding icon, overlay, or second
gameplay radius. `SelfOwned` uses the ally effect treatment because the controlled fighter's
selection ring already identifies self; neutral leaves the type pattern untinted. Normal shader
variants and degraded mesh variants preserve the same type pattern and cadence for ally and enemy;
only the selected accent color changes. Contextual overrides are a closed owned enum and may replace
only the documented recipe selection step; they do not create a general priority/tag engine.

The client does not initially need a protocol change for exact weapon identity. Delivery entities
already replicate `ReplicatedAttackSource`, whose `AttackSource` includes the immutable
`WeaponRecipeFingerprint`, source kind, team, owner, attack ID, and ultimate/deployable identity.
Fighters also replicate `ResolvedMatchLoadout`. The resolver maintains a FIFO-bounded cache of
at most 512 attack signatures, learning exact fingerprints from replicated delivery state and
snapshotting the current loadout at `AttackAccepted`. A signature remains valid after a projectile
or source visual disappears. Persistent Splash carries its effect semantics directly; elemental
fields carry their kind; armed sticky entities retain their replicated attack source. Missing
identity fails to the neutral recipe. A wire migration is warranted only if focused lifecycle tests
later demonstrate an accepted cue path that cannot be observed or cached before its source is gone.

#### Recipe and layer schema

Catalog schema 8 uses five bounded tables: semantic request mappings, typed style mappings, recipes,
reusable mesh layers, and decorative particle layers. A style mapping may select on an implemented
accent, carrier, lifecycle phase, and ultimate context over the existing typed signature. Every styled
request retains one key-only fallback, duplicate selectors fail validation, and the most-specific
matching selector wins deterministically. IDs are client-only validated lowercase ASCII names and
never enter gameplay content fingerprints or protocol state.

A recipe owns an ordered list of mesh-layer IDs, zero to two particle-layer IDs, a total concurrency
cap, one fallback recipe, and an optional reduced recipe. Recipes do not recursively include other
recipes. References must exist; reduced-recipe chains and executable authoritative-deadline
fallback chains must be acyclic. Resolution expands to at most eight combined layers.

A layer owns:

- one implemented source: mesh, bounded trail, or optional particle adapter;
- a typed primitive/material key rather than an asset path;
- an anchor and geometry policy derived from supplied authoritative facts;
- a bounded timeline;
- shadow flags and optional orientation policy;
- a readability class: `CriticalGeometry`, `Identity`, or `Decorative`.

The demonstrated mesh vocabulary is sphere/flash body, ground disc/fill, regular or thin
annulus/boundary, unit cuboid or segment, exact projectile body, and lobbed body/shadow. A new primitive is added only with a real
carrier/layer use. The first mesh migration authors delay, fixed or authoritative duration, linear
or smooth-step easing, and transform/scale keyframes. It retains static material alpha. Per-instance
animated alpha is not
authored until a bounded material-uniform or explicitly reclaimed material-pool implementation is
proven; cloning `StandardMaterial` every frame or leaving per-effect assets behind is forbidden.

Initial validated engine bounds are:

- 64 semantic request mappings;
- 128 recipes;
- 256 reusable layers;
- eight expanded layers per recipe;
- 96 active transient recipe roots, preserving the current total cap;
- a maximum fixed transient duration of 10 seconds;
- per-recipe concurrency from 1 through 96;
- 512 cached attack signatures;
- no per-frame mesh or material asset creation;
- persistent-root counts remain bounded by their authoritative gameplay caps rather than receiving
  a second VFX-owned capacity rule.

Normal Cold and Fire impact recipes demonstrate the optional particle adapter with seeded fragment
and ember bursts. The adapter is a client-feature-only Hanabi 0.19 integration; it never enters the
server feature graph. Particle layers have no readability-class field and are therefore always
`Decorative`. Reduced and primitive variants omit them, while retained mesh layers remain a complete
fallback. The materializer waits for a stable compiled GPU pipeline before releasing one-shot
bursts. Fire lobbed projectiles additionally demonstrate the bounded rate-emitter mode: particles
spawn at the moving projectile in global simulation space, remain behind to expand and fade, stop
emitting when the projectile disappears, and retain the emitter only for the authored particle
lifetime. Both modes share the global emitter cap and deterministic CPU seed contract.

Hanabi needs compute shaders, vertex-stage storage buffers, and indirect execution, so the adapter
installs it in `Plugin::finish` only when the detected GPU supports them (never on GL).
Otherwise, or when the quality tier selects the CPU backend, burst layers use a non-compute
fallback behind the same recipe boundary: at most ten seeded mesh particles per burst with the
layer's mesh, a flat profile color, acceleration, lifetime, and size curve, simulated on the CPU and
despawned with their effect root, counted against the same emitter cap. The decorative rate-based
smoke trail is skipped in the fallback. `BRAWLER_FORCE_PARTICLE_FALLBACK=1` exercises the
incapable-GPU path on a desktop for verification.

Lobbed landings retain the largest authored area-target radius with the cached attack style. The
landing cue projects that radius into a circle signature and selects a carrier-specific explosion
recipe. Its thin ring remains the exact radius while ground flash, flash/core bodies, expanding
shockwave, typed sparks, and typed smoke provide staged identity. Missing cached identity falls
back to the ordinary point-impact recipe rather than inventing a radius. World-object explosions
reuse this layer vocabulary.

Particle limits are separate and explicit:

- 64 authored particle layers in the catalog;
- at most two particle layers per recipe and eight combined layers;
- at most 256 particles per emitter;
- at most 96 active particle emitters across transient roots;
- a maximum particle lifetime of 10 seconds;
- normal variants only; reduced and primitive variants materialize zero particle emitters;
- no per-frame particle-effect, mesh, or material asset creation.

#### Determinism, variants, and cleanup

Request merge order remains explicit producer rank then stable event ID. Mesh timelines sample
explicit elapsed time; persistent timelines sample authoritative ticks. Randomized presentation
uses counter-based hash sampling from a versioned tuple containing the stable presentation
identity, recipe ID, layer ID, occurrence index, and property salt. It does not consume a mutable
global random stream, so adding one decorative choice cannot perturb other layers. The same tuple
selects the same bounded values. GPU particle motion may be statistically rather than pixel
identical across hardware, but its seed and parameter bounds remain captured. The seed derives
from the stable presentation identity, recipe ID, layer ID, and occurrence index; it does not
consume a mutable global gameplay RNG.

Every recipe variant obeys these rules:

- `CriticalGeometry` is present in normal, reduced, and primitive variants and uses an available
  primitive fallback;
- `Identity` retains at least one non-color motion, silhouette, or timing channel in reduced and
  primitive variants;
- `Decorative` may be shortened, thinned, or omitted;
- bloom, imported assets, particles, and custom shaders are never required for counterplay;
- exact radius, rectangle, cone, projectile footprint, impact point, and authoritative deadline are
  derived from gameplay facts rather than authored approximations.

The normal Fire-area material is the demonstrated optional-shader case. Its periodic noise field is
sampled from authoritative ticks, so pause, step, and replay do not depend on wall-clock time. It
draws only the decorative turbulent circumference and halo. A low-opacity retained disc beneath it
tints the affected ground without obscuring terrain or fighters, and a separate retained annulus
always draws the exact authoritative radius. Reduced/primitive variants replace the shader with
shared fill, ring, and tapered-tongue meshes. The material handles are shared; animation updates one
retained uniform and does not allocate per area or per frame.

Transient recipe roots retain oldest-first total/per-recipe eviction and timer/deadline cleanup.
Attached and persistent roots retain `CombatVisualOwner` and their source entity as cleanup truth.
Owner removal, interruption, reset, match transition, and failed layer materialization must still
remove the complete root. Native render evidence records the VFX catalog fingerprint, selected
presentation variants, active lifecycle-root counts, transient recipe and authored mesh/particle
layer order, particle profile, capacity, bounded burst/rate spawn mode and seed, emitter high-water/suppression
diagnostics, and retained asset counts alongside the authoritative state and rendered PNG.

#### Read-only VfxLab

The development-only `brawler-vfx-lab` binary is the isolated inspection surface for this contract.
It composes the production client catalog, resolver, retained assets, materializers, lifecycle
roots, degradation settings, cleanup, and evidence capture without starting a server or entering a
match. It is excluded from the server feature graph and production player flow.

The transient fixture enumerates all 17 registered semantic requests. Eleven additional bounded
fixtures cover straight, melee, lobbed, sticky, persistent Splash, elemental field, concealment,
cone, dash, sentry, and attached-status facts, then let the production lifecycle materializers
create and own their visual roots. Fixtures do not contain a second visual implementation. Restart
preserves the explicit presentation identity; choosing a new identity is a separate action. Pause
and single-step use the canonical simulation tick, so persistent deadlines and attached motion can
be inspected predictably. Neutral, Cold, Fire, Poison, and Restoration accent controls plus the
context-sensitive alternate control resolve through the same production signature; fixed-style
world feedback disables irrelevant comparisons in the guide.
Automation may apply an exact bounded initial sample in
ticks before freezing, allowing a warmed native capture to inspect a transient at its activation,
peak, or tail.

Every Lab capture uses the production asynchronous screenshot path and writes a paired PNG and JSON
snapshot. The JSON identifies the build and VFX catalog, fixture, semantic request and resolved
recipe with ordered layers where applicable, presentation identity, playback tick,
normal/reduced/primitive variant,
authoritative geometry/deadline inputs, active lifecycle-root counts, and retained asset counts.
The same snapshot also records every active typed lifecycle signature and the bounded attack-cache
size and eviction count, so a production fact and its isolated fixture can be compared without
inferring style from pixels alone.
Technical diagnostics also report current request/style-mapping, recipe, mesh-layer, and
particle-layer occupancy against the sealed capacities, plus transient root/layer high-water,
eviction, particle-emitter high-water, and suppression counts. Evidence automation must warm the
native render surface sufficiently before taking a paused screenshot; the paired JSON remains the
semantic/resource record rather than requiring facts to be inferred from one animation frame.
The Lab is read-only: future authoring and validation workflow belongs to the separate VFX Workbench
and must publish through validated catalog transactions rather than mutating production resources
inside this viewer.

## Environments, themes, and geometry

Visual footprints must agree with authoritative planar shapes:

- rectangles become cuboids or validated imported models with presentation-only height;
- circles become cylinders or generated circular geometry;
- floors use generated or cached grid-aligned tile surfaces that stay subordinate to the actors
  and objectives on them;
- destructible map assets reconcile whole-placement normal, removed, or replacement visuals from
  replicated terminal outcomes;
- arbitrary or dynamic shapes use primitives/generated meshes rather than an approximate prop;
- imported environment scenes require a validated pivot, orientation, scale, footprint, and
  compatible visual profile.

A map theme provides client-owned ground, edge, lighting, palette, material, and default visual
choices. It is not a baked map image or style lock. An authored placement resolves one stable
visual profile, but neither the theme nor the model decides collision, destructibility,
replacement, or other gameplay behavior.

The client resolves shared stable theme and visual-profile IDs through its map presentation
catalogs. A theme's environment kit (`theme_kits` in `assets/catalogs/map_asset_visuals.ron`)
replaces the default visual with the same profile ID on maps of that theme only, so the shared
content (asset and visual-profile IDs, footprints, gameplay profiles) and its fingerprint are
unchanged. Every presentation theme must have a kit, and every built-in map draws its imported
pieces only from its own theme's kit. Kit visuals keep the default's primitive fallback, import only
from the kit's directory, and carry their colour in the kit palette; there is no per-asset tint.
The defaults serve a visual that a theme's kit does not cover (a Map Maker document placing another
theme's prop): every imported default is the original kit piece of its native theme, and validation
rejects an imported default outside the kit directories, so no third-party environment pack ships
(BRL-0188). Wall, cover, and wall-top colours stay clear of the effect-tile hues (speed cyan, slow
violet, damage red). Every presentation site that knows the
map (static and dynamic map assets, Heist safes, restoration pickups, Map Maker markers and palette
previews) resolves through the theme. Imported scenes load and admit once per GLB path. Every
kit builder shares one kit contract module (`asset_src/environment/kit_common.py`).
Asset paths, transforms, and fallbacks remain client-only and do not contribute renderer
details to the gameplay protocol. Presentation reconciliation keys include the accepted map
instance, recipe fingerprint, and theme so replacement cannot retain stale geometry or materials.

For imported map scenes, the client computes one hierarchy-aware intrinsic bound from every mesh
AABB after the scene and its dependencies load. `Exact` profiles uniformly fill an
aspect-compatible authoritative footprint; `Contained` profiles uniformly fit, recenter, ground,
and optionally under-fill it. Their catalog `scale` is a dimensionless fill factor in `(0, 1]`,
not a model-space correction. Imported `Tiled` content is unsupported until a real placement
requires it; generated water and vegetation retain their tiled presentation. Missing, empty,
non-finite, or shape-incompatible imports select the profile's deterministic fallback without
delaying gameplay readiness.
The placement's quarter turns rotate the visual root, so imported and primitive visuals are fitted
in the asset's unrotated footprint (a rotated 2x1 barrier stays two cells long).

Occlusion should first be solved through camera elevation, wall height, material value, and map
composition. Fighter silhouette outlines are part of the visual direction, not an occlusion fix.
Selective fade, x-ray rendering, occlusion-driven outlines, decals, or other custom gameplay-world
pipelines require evidence that these simpler controls are insufficient.

## Asset forms

Choose the smallest representation that preserves readability and authoritative-shape agreement:

- imported GLB scenes for recognizable props, animated characters, and attached weapons;
- cached Bevy primitives for exact, repeated, or highly dynamic shapes;
- generated meshes for resolved perimeter geometry, terrain chunks, procedural rings, and other
  topology derived from authoritative state;
- Bevy UI for product flow, HUD, overlays, and projected fighter information;
- image, font, audio, and shader assets for bounded client presentation with explicit degradation.

GLB is the preferred shipped model format because it carries meshes, materials, hierarchy, skins,
and animation in one Bevy-supported asset. Each imported family retains its pack namespace and
relative texture layout. Common texture filenames must not be flattened across packs.

Animation clips may omit channels. Leaving defeat restores the imported bind pose before starting
a live loop so root or limb transforms cannot remain in a defeat pose. Grip, forward-axis, scale,
and anchor corrections belong to presentation profiles/constants rather than gameplay rotation.

Primitive meshes and materials are cached and shared. Generation-owned meshes have explicit
removal ownership; terrain chunks mutate owned meshes instead of allocating on every revision. A
unique mesh or material per fighter, projectile, tile, prop, or effect is not an accepted default.

## Asset sources and provenance

The asset boundary has these distinct sources:

| Source | Responsibility |
|---|---|
| `external_assets/` and `inspiration/` | Source material and retained concepts; not scanned or packaged as runtime content |
| `assets/manifest.ron` | Exact shipped third-party file inventory, provenance, license, requiredness, and fallback |
| `assets/licenses/THIRD_PARTY.md` | Human-readable pack index (version, source, license, files used) and source re-acquisition steps; `manifest.ron` stays authoritative |
| `assets/catalogs/` | Client-owned paths, transforms, material values, theme profiles, and degradation mappings |
| `content/` | Stable server/client-neutral gameplay, map-asset, gameplay-profile, and theme identities |

Transient VFX uses a strict client-only schema-5 recipe catalog and a sealed request registry. Feature
plugins translate their owned gameplay facts into validated semantic request keys such as
`combat.muzzle`, `world-object.explosion`, or `heist.critical`; these producers do not select
meshes, materials, renderer families, reduced-effects policy, or resolved lifetimes. During plugin
finalization the registry requires exact agreement between bounded producer registrations and
authored request mappings, including authoritative-radius and deadline capabilities. Unknown,
duplicate, missing, extra, or incompatible requests fail closed. Stable producer ranks plus event
identities preserve deterministic cross-feature effect ordering without a central cue-family enum.

`assets/catalogs/vfx.ron` maps semantic request keys to recipes and ordered reusable mesh layers.
Layers own the supported renderer/material key, fixed-world or authoritative-radius scale and
anchor policy, readability class, shadow policy, and bounded timeline; recipes own concurrency,
fallback, and reduced-effects policy. Adding semantic
VFX with existing renderer primitives requires a feature producer registration and authored
mapping, not renderer dispatch changes. Adding a genuinely new concrete primitive or material
remains an intentional typed renderer extension. Authoritative radii still arrive from replicated
gameplay facts: the catalog may scale or anchor their presentation but never replaces their
gameplay geometry.

One-shot audio uses a strict client-only schema-4 catalog and a sealed producer registry. Feature
plugins translate owned gameplay facts or replicated-state transitions into semantic
`AudioRequest` messages identified by bounded stable keys; they do not select assets, profiles,
playback settings, or concurrency policy. Shared semantic keys such as `ready`, `impact`, and
`defeat` may be registered by multiple producers, while producer identities and ranks keep startup
validation and generic-adapter ordering explicit. Producer-local monotonic sequences preserve each
feature's fact order within its declared rank, so same-frame cap precedence does not depend on Bevy
message merge order.

`assets/catalogs/audio_profiles.ron` maps those request keys to stable IDs from
`assets/manifest.ron`, which remains the one owner of shipped audio paths and provenance. A single
generic adapter owns occurrence deduplication, loaded-asset fallback, shared one-shot reservations,
and Bevy playback materialization. Audio profiles own speed, volume, concurrency, and
unavailable-asset fallback. One-shot playback that despawns when the source completes is the sole
supported one-shot mechanic, so it remains an adapter invariant rather than a misleading authored
choice. Adding a cue with existing playback primitives requires a feature producer registration
and authored mapping, not a central playback dispatch edit. Client presentation catalog revisions
do not enter the gameplay content fingerprint or dedicated-server dependency graph.

Since BRL-0158 every registered cue key maps to its own distinct clip (a catalog test enforces
it): per weapon delivery family fire (`fire-straight`, `fire-sticky`, `fire-lobbed`,
`fire-splash`, `fire-melee`, `fire-spray`) and impact sounds, `hit-dealt`/`hit-taken` relative to
the controlled fighter (spaced at least 4 ticks apart per kind, 12 under reduced combat effects,
which thins but never mutes them), per-ultimate activation, objective, pickup, match start/score and
victory/defeat/draw stings, and `ui-click`/`ui-confirm`/`ui-back` from the shared release-activation
press model by surface role. Keyboard, gamepad and platform-Back input is interpreted by each
screen owner, which publishes a presentation-only `UiFeedback` fact (activated with the control's
surface role, back, focus moved, slider stepped) only where its input actually acted; the same UI
producer maps it to the same cues plus a quiet `ui-move` tick for focus moves and slider steps. A
key press within 50 ms of another UI press is one gesture and sounds once, and ticks are at most
one per 70 ms (none right after a press), so held or swept input never buzzes. The same catalog owns two looping music tracks selected by semantic
key (`menu` everywhere except `match` during a match); a client music adapter crossfades between
them and trims playback to the authored `loop_seconds`. A single mix system scales every audio
entity by Master × its category (Effects or Music, both persisted settings) × its authored base
volume and applies focus-loss mute. The five Kenney CC0 clips remain; every other clip is original
audio generated by `asset_src/audio/build_audio.py` (see its README and
`assets/licenses/pewpew-blitz-original-audio.txt`); manifest rows for original audio carry the
license `Original` and point at the generator instead of a URL.

Only assets owned by a current gameplay, map, product-shell, accessibility, or presentation use are
promoted into `assets/`. Availability in a source pack is not sufficient. Preview renders, source
archives, FBX/OBJ alternatives, overview images, and authoring metadata remain source-only unless a
specific runtime use is accepted.

Every redistributed third-party file must have exact provenance in the manifest and retained
license text where required. Original replacements should also preserve their source masters and
document the export pipeline needed to reproduce runtime derivatives. Original runtime models live
under `assets/brawler/models/original/`; their provenance is the `asset_src/` source directory and
its README (for Riff, `asset_src/characters/riff/README.md`; for the held weapons,
`asset_src/weapons/README.md`; for environment kits, `asset_src/environment/<kit>/README.md`),
not a manifest row. `assets/licenses/pewpew-blitz-original-environment.txt`,
`pewpew-blitz-original-weapons.txt` and `pewpew-blitz-original-characters.txt` point at the
environment kit, weapon and fighter character sources. `pewpew-blitz-original-shell-art.txt`
records per-file provenance for the branding and shell menu artwork (`assets/brawler/ui/branding/`,
`assets/brawler/ui/shell/`). Every Credits original-work entry points at one of these notes.
The shell icon set under `assets/brawler/ui/icons/`, including the hanging mode pennants
(BRL-1wms), is likewise original generated artwork:
`asset_src/player-shell/icons/` holds the generator, editable SVG sources and README, and
`assets/licenses/pewpew-blitz-original-icons.txt` points at them.

## Asset admission checklist

A presentation asset or family is admitted only when:

1. a current gameplay, map, UI, audio, branding, or presentation owner requires it;
2. any third-party source permits redistribution and the manifest records its exact provenance;
3. its runtime format and dependency paths preserve a stable pack or product namespace;
4. orientation, scale, pivot, footprint, material response, animation, and real-size readability
   are verified in the supported client rather than inferred from a preview;
5. optional or shape-critical content has a deterministic, usable fallback;
6. missing and late readiness cannot block authority, navigation, or crash the client;
7. repeated spawn, screen transition, map replacement, restart, and reconnect release owned
   entities and generated assets;
8. loading and handles remain client-owned, while shared definitions reference stable IDs only;
9. the dedicated-server feature graph remains free of rendering, windowing, image, scene,
   animation, audio, device-input, shader, and client-asset dependencies;
10. tests and native checks are proportional to the lifecycle, readability, and performance risk.

## Server, protocol, and degradation boundaries

- dedicated servers and routed workers contain no cameras, meshes, materials, images, models,
  animation, audio, windowing, shaders, device input, or client assets;
- protocol state remains planar and never exposes process-local ECS entity identity;
- gameplay emits stable state and cues; client presentation resolves them to assets and effects;
- optional presentation failure cannot alter authoritative timing, collision, damage, status,
  scoring, admission, or results;
- `BRAWLER_FORCE_PRIMITIVE_WORLD=1` is a degradation and verification path within the sole 3D
  gameplay renderer, not a renderer choice or parallel content mode;
- primitive degradation preserves gameplay footprints, relationship cues, objectives, and distinct
  theme readability even when imported scenes are unavailable.

## Performance and verification

The baseline remains deliberately simple: cached meshes and materials, bounded map-asset visuals,
transients, limited lighting, and Bevy's normal culling. The visual direction adds a shared toon
material, fighter outlines, HDR tonemapping, and restrained bloom. Each of these must pass the
render performance gate and have a mobile quality tier (BRL-0161). Instancing, LOD, deferred
rendering, or other custom gameplay-world pipelines still require measured need.

The authoring contract permits dense coverage up to every cell of a 512×512 map, but the current
renderer may still materialize repeated static tiles individually. Structural validity therefore
does not claim acceptable native performance at the maximum. Chunked or instanced vegetation and
faster concealment lookup become required when measured content approaches that scale.

Automated verification should cover coordinate conversion, exact footprints, projectile origins,
asset/catalog validation, lifecycle cleanup, animation recovery, relationship colors, projected-UI
query safety, reduced-effects bounds, degraded fallbacks, and server feature isolation. Native
checks should exercise supported modes, maps/themes, aspect ratios and UI scales, imported and
fallback paths, Dashboard preview, defeat/respawn, map destruction, objectives, and representative
combat density. The render-evidence harness accepts `BRAWLER_RENDER_REDUCED_EFFECTS=1` to install
the same reduced-combat-effects resource used by the product settings shell in non-interactive
release clients; the generated report records that policy so an accidental default-profile run
cannot be accepted as reduced-effects evidence. `BRAWLER_RENDER_WEAPON=<ID>` (a costed weapon ID
from `content/catalogs/builds.ron`) gives the measured client that primary weapon through the
canonical saved-brawler create/select path, so each weapon family's aim lane, projectiles, and
effects can be captured; unset, the measured client keeps its existing selected brawler.

The frame interval in the native render report is vsync-bound on fast desktops, so the report
(schema 9) also records an informational GPU-time distribution: `gpu_time_source`,
`gpu_sample_count`, and `gpu_p50_ms` / `gpu_p95_ms` / `gpu_p99_ms`. The automation-only probe
registers a `Queue::on_submitted_work_done` callback after each frame's render submission and polls
the device from a thread that runs only during the measurement window. The value is submit-to-
completion time, which approximates the frame's GPU time only when the GPU is idle at submit (as in
vsync-bound runs); a GPU-bound run also counts queueing behind earlier frames. Without samples the
source is `unavailable` and the percentiles read `unavailable`. GPU time never affects `result`.

Imported map-object scale is validated from intrinsic asset bounds after the complete parent/child
transform chain, not from a profile number in isolation. A nearby authored tile provides a native
size reference when footprint readability matters. Static materialization and dynamic replacement
must share one runtime classification so an HP-bearing object cannot leave an obscuring static
duplicate. Imported and primitive-fallback transforms receive separate focused regressions when
their fitting rules differ.

## Envisioned direction

- a coherent original PewPew Blitz environment, fighter, weapon, animation, VFX, UI, icon,
  branding, and audio language that can replace or extend the current licensed foundation;
- additional themes and compatible visual families promoted only for concrete map needs;
- skin selection and replication with entitlement, contrast, silhouette, and relationship-cue
  validation;
- additional presentation profiles for genuinely supported fighter and weapon forms;
- advanced occlusion or rendering treatments justified by playtest and performance evidence.

These extensions must preserve the same authority, stable-identity, fallback, lifecycle,
readability, and server-isolation contracts. They do not justify a second gameplay renderer or a
generic asset framework in advance of a demonstrated use.
