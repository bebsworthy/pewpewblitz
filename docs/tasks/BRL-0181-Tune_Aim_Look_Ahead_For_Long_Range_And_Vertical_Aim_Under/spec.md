# Outcome

Aiming a long-range weapon, or aiming up or down the screen, shows enough of the threat area under
the BRL-0148 framing (30° FOV, 55° elevation, ~12 visible cells: about 4.9 cells behind the look
target, 7.1 ahead, ~10.5 to each side at the 2.21:1 reference aspect).

# Context

- BRL-0148 accepted an explicit exception: the standard 320-unit Pulse range fits to either side
  at the reference aspect, but vertical aim shows only ~5-7 cells, and lobs (520), Splash (480),
  Sticky Blomb (640, ~755 with Long Assembly), and ranged ultimates exceed the view.
- The BRL-0143 rig leans 1.5 cells toward aim (`assets/catalogs/camera_rig.ron`
  `look_ahead.distance_cells`), capped with follow lag at `MAX_CAMERA_LEAD_WORLD` = 3 cells, below
  the 4.9-cell near extent.

# Scope

- Evaluate per-direction or range-scaled look-ahead (e.g. lean further when aiming a long-reach
  weapon or along the screen's short axis) within an aspect-fair ceiling; keep it presentation
  only and disabled under reduced motion.
- Keep aim/auto-aim on the unshaken view and the per-axis framing clamp unchanged.

# Out of scope

Changing the BRL-0148 framing constants, weapon ranges, or map sizes.

# Acceptance criteria

- Rig tests still prove the fighter stays visible at every edge and aspect.
- Native captures and a playtest confirm long-range aim reads without motion discomfort.
