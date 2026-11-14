# Context
BRL-0179 made the shell previews toon-shaded with the arena rim, but `attach_fighter_outlines` only
runs for `V3FighterVisual` roots with a replicated fighter, so the previews have no outline.

# Scope
- Reuse the hull builder (`smoothed_hull_mesh`/`hull_for`) for preview body meshes; give hulls the
  preview `RenderLayers` (not inherited from the body).
- Outline width is in world units at the fighter's depth; check the Dashboard (scale 32) and card
  framing, and decide whether the standalone weapon close-up gets an outline at all.
- Mind the Low tier (BRL-0161): one extra hull draw per body mesh per preview camera.

# Acceptance criteria
- Dashboard, brawler-details, and brawler-card previews show the arena outline; native captures.
