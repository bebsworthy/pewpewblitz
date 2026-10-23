# Outcome

One written art direction, achievable with the project's real asset pipeline, governs world rendering, characters, environments, HUD, and the player shell. `docs/11-art-and-presentation-direction.md` states it, and later visual tickets are judged against it.

# Context

Four directions currently coexist:

- the glossy blue Brawl-like mockups (`inspiration/player_dashboard*.png`);
- the approved cream/teal/orange "poster" shell (`docs/design/player-shell/`, `src/client/ui/theme.rs`, chosen in BRL-0112/BRL-0114);
- the voxel-toy roster concept (`asset_src/04-character_roaster.png`, Riff in `asset_src/characters/riff/`);
- what actually renders: Kenney/KayKit kit pieces from six packs (`assets/catalogs/map_asset_visuals.ron`) and one Kenney mini-character for every fighter.

`docs/11` §Visual direction asks for matte materials, calm floors, and restrained shadows and surface detail, which works against the saturated, outlined, energetic target the owner wants. Approved mockups are built around hero characters the pipeline cannot produce.

# Scope

- Decide the character style, world material treatment (toon/rim/outline), palette (world and shell), floor treatment, typography treatment (stroke/shadow), and motion stance.
- Produce one reference board using real in-engine captures or real models, not unreachable generated heroes.
- Rewrite `docs/11` §Visual direction and the shell palette section of `docs/design/player-shell/README.md` to match.
- Rule: visual mockups used as acceptance references must use assets the pipeline can produce.

# Out of scope

Implementing renderer, asset, or UI changes; those belong to the linked tickets.

# Acceptance criteria

- The owner answers the direction question, and the decision is recorded here.
- `docs/11` and the player-shell README describe one coherent direction, with explicit palette tokens and material rules.
- The reference board exists as ticket assets and uses only achievable art.
- Linked visual tickets reference the decision.

# Verification

Documentation review and owner acceptance.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.

# Decision (2026-10-23)

The owner approved the review recommendation in chat (q-000016, suggestion 1): **voxel-toy characters with toon, rim and outline world shading, and a saturated blue shell.** This replaces the cream/teal/orange shell palette chosen in BRL-0112/BRL-0114 and the "matte, restrained" stance in `docs/11`.

The values below are the starting point. Implementing tickets may tune exact values but must not change the direction.

## Characters (BRL-0154)

- Chunky voxel-toy figures in the style of the roster in `asset_src/04-character_roaster.png`.
- Proportions:
  - head about 45% of height;
  - weapon about 1.5x oversized;
  - visual height about 1.6 cells, with the authoritative collision radius unchanged.
- One strong silhouette per character, and saturated primary colors on clothing.
- Riff is the first production character.

## World shading (BRL-0146, 0150, 0151, 0152)

- **Light ramp:** a two-band toon ramp. The shadow side is about 0.65x albedo, hue-shifted toward blue/purple.
- **Rim light:** strength about 0.35. On fighters it is tinted with the relationship color.
- **Fighter outline:** dark navy `#0E1638`, scaled to world size.
- **Post-processing:** HDR with a filmic tonemapper and restrained bloom. Only emissive projectiles, muzzle flashes and pickups bloom.
- **Color grading:** saturation about x1.15-1.25.
- **Grounding:** contact shadows or baked AO on every object.

## Floor and environment (BRL-0147, BRL-0153)

- **Floor:** a two-tone tile surface aligned to the grid. The two tones differ by 8% value or less, and the floor stays lighter and less saturated than the props.
- **Kits:** one environment kit per theme, with a shared bevel, a top-face highlight and baked AO.
- **Foliage:** chunky, readable bush forms.
- No mixing of third-party kits within one theme.

## Shell and HUD palette (BRL-0155, BRL-0156)

| Token | Value | Use |
|---|---|---|
| `shell-bg-top` / `shell-bg-bottom` | `#2E7BFF` / `#1B3FA8` | Screen background gradient |
| `panel` | `#24308F` | Primary panels |
| `panel-raised` | `#3B5BDB` | Raised cards, selected rows |
| `ink` | `#0E1638` | Text stroke, outlines, bottom lips |
| `play` / `play-highlight` | `#FF8A1F` / `#FFC21A` | The primary action only |
| `accent` | `#38D6FF` | Focus, links, ultimate charge |
| `text` | `#FFFFFF` | Headings, buttons, HUD numbers |
| `self` / `ally` / `enemy` | `#4CD964` / `#2F8BFF` / `#FF3B3B` | Relationship colors (world and HUD) |

- **Typography:** Lilita One everywhere, including the HUD. Headings, buttons and HUD numbers are white with a 2-4 px `ink` stroke and a soft drop shadow.
- **Surfaces:** vertical gradient, a light top band, and a darker 4-6 px bottom lip. Buttons squash on press and activate on release.

## Motion

- The interface is energetic rather than static. That covers:
  - idle loops on the primary action;
  - panel entrances;
  - press squash;
  - hit and reward bursts.
- Reduced-motion and reduced-effects settings disable idle and decorative motion.
- This replaces the 0.4% drift-only rule in `docs/design/player-shell/motion.md`.

## Mockup rule

Mockups used as acceptance references must use real in-engine captures or real models from the project's pipeline. Generated hero art that the pipeline cannot produce is not an acceptance reference.

## Remaining for this ticket

- Rewrite `docs/11-art-and-presentation-direction.md` §Visual direction to match this decision.
- Update the shell palette and motion sections in `docs/design/player-shell/`.
- Create the reference board from the Riff preview, the roster concept and in-engine captures.

# Delivery (2026-10-23)

## Documentation

- `docs/11-art-and-presentation-direction.md`:
  - §Visual direction is rewritten to cover characters, world shading, floors and environments, feedback, the shell and HUD, the mockup rule, and an adoption-status table mapping each area to its owning ticket.
  - The camera framing paragraph notes the BRL-0148 re-evaluation.
  - The floor bullet now says "grid-aligned tile surfaces".
  - The occlusion paragraph separates direction-owned silhouette outlines from evidence-gated occlusion treatments.
  - The performance baseline admits the toon material, outlines, HDR and bloom, each subject to the performance gate and a mobile tier.
- `docs/design/player-shell/README.md`:
  - A new visual-language token table.
  - A legacy-to-new role migration map.
  - A note that the approved references remain composition references only.
- `docs/design/player-shell/ui-toolkit.md`:
  - The token contract and migration status (`theme.rs` keeps legacy values until BRL-0156).
  - Role colors by token.
  - Pressable surface chrome (gradient, top band, bottom lip).
  - Activate on release with drag-cancel.
  - The gradient background.
- `docs/design/player-shell/motion.md` is rewritten as an energetic motion contract (press squash, release overshoot, Play breathing and shine, entrances, reward bursts, reduced-motion rules).
- `docs/design/player-shell/generation-brief.md` uses the new palette and the mockup rule.
- `docs/design/player-shell/delivery-contract.md` now refers to the toon, rim and outline material instead of "matte, restrained".

## Reference board

Ticket assets:

- `brl-0140-reference-board.png`:
  - (1) a world look proof from the real Riff GLB and the shipped Kenney blaster;
  - (2) a shell mockup built from the real Riff render and the decided tokens;
  - (3) the roster concept, labelled inspiration only, beside the live 2026-10-23 "before" capture.
- `brl-0140-world-look-proof.png` and `brl-0140-riff-hero-proof.png` are the raw renders.
- Reproducible sources are in `asset_src/art-direction/` (`render_look.py`, `compose.py`, `README.md`). A clean re-run from the repo reproduced the board.

## Deliberately unchanged

- `docs/13-player-ux.md` still describes the shipped shell with legacy color words (cream, teal, orange) because they are accurate for the current build. BRL-0156 updates that wording when it swaps the palette.
- The camera framing constants stay the accepted contract until BRL-0148.

## Acceptance check

- Owner decision recorded (q-000016 approved and resolved): met.
- `docs/11` and the player-shell docs describe one coherent direction with explicit tokens and material rules: met.
- Reference board attached, using only real renders except the concept panel, which is explicitly labelled: met.
- Linked visual tickets reference the decision: met. BRL-0146, 0147, 0150, 0151, 0153, 0154, 0155, 0156 and 0157 are blocked by this ticket; BRL-0148 is related.

## Verification

- Documentation review.
- Blender 5.2 renders and board composition re-run from `asset_src/art-direction/`.
- No Rust code changed, so no build or test run was required.

## Learning review

- **Mistake:** the first proof renders showed Riff in T-pose and almost entirely in the shadow band.
  - **Causes:** the glTF importer keeps clips as muted NLA tracks, and closed inverted-hull outline shells cast shadows onto the mesh inside them.
  - **Prevention:** recorded in `asset_src/art-direction/README.md` and forwarded to BRL-0150/BRL-0151 as runtime requirements.
- **Mistake:** a high key light let Riff's ears shadow his face.
  - **Prevention:** BRL-0150 must choose the character self-shadow and sun-angle policy deliberately.
- **Reusable lesson:** prove a visual direction with the project's real assets before writing it down. The earlier mockups failed because they relied on unreachable generated heroes.
