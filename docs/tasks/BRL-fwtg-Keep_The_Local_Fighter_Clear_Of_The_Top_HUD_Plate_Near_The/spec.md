# Evidence
BRL-3zas native capture at 1280x720 in Zombie Practice: the local fighter near the north wall has its name and health plate under the top-center score plate. The camera clamps to the map bounds (`clamp_camera_center`), so the local fighter can reach the top 15% of the screen.

# Scope
- Choose one: add a top safe band to the camera clamp or the dead-zone framing for the local fighter, or move the local overhead below the fighter whenever it would overlap the HUD plate.
- Check the result at 1280x720 and on the phone viewport (868x411).

# Acceptance criteria
- The local fighter's overhead stays readable at every map edge on both viewports.
