# Evidence
BRL-3zas native capture at 1280x720 in Zombie Practice: the local fighter near the north wall has its name and health plate under the top-center score plate. The camera clamps to the map bounds (`clamp_camera_center`), so the local fighter can reach the top 15% of the screen.

# Scope
- Choose one: add a top safe band to the camera clamp or the dead-zone framing for the local fighter, or move the local overhead below the fighter whenever it would overlap the HUD plate.
- Check the result at 1280x720 and on the phone viewport (868x411).

# Acceptance criteria
- The local fighter's overhead stays readable at every map edge on both viewports.

# Decision (BRL-fwtg)
Chosen option: a top HUD safe band in the camera rig, built on BRL-vptw's framing. Rejected: flipping the local overhead below the fighter (at the north wall the clamp leaves the fighter about half a cell from the top edge, so its head and body would still sit under the score plate, and the plate would jump around the fighter).
- `camera.rs::top_hud_fighter_limit(viewport, ui_scale)`: closed-form farthest ground offset ahead of the look target at which the local overhead (anchor `OVERHEAD_WORLD_HEIGHT`, height `OVERHEAD_AMMO_HEIGHT`) sits a 6-unit gap below the score plate's bottom edge (`hud::plate::PLATE_BOTTOM` = 82 UI units). All UI terms are multiplied by the effective `UiScale`, so it follows the phone viewport fit and the player's UI-scale setting (0.8 to 1.4). About 2.1 cells at 1280x720 and 1.9 cells at 914x411 at the default scale.
- `camera_rig.rs::top_hud_band_center`: after the unchanged per-axis clamp, `center.y = clamped.max((fighter.y - limit).min(framed.y))`. It applies only when the map clamp holds the camera south of the rig's own framing (the north edge), passes the edge only as far as the band needs, and never goes further north than the unclamped framing. Open-field framing, look-ahead and the x axis are unchanged. Not in `camera_rig.ron`: it is derived from code-owned HUD and overhead geometry, so there is no schema change.
- At the north wall the view now shows up to about 4 cells of outer dressing beyond the map edge.
- Durable doc: `docs/11-art-and-presentation-direction.md` camera rig paragraph.

# Verification (BRL-fwtg)
- Pure tests: `camera::tests::top_hud_limit_puts_the_local_overhead_just_below_the_score_plate` (projects through Bevy's camera pose and `Mat4::perspective_rh`; 1280x720, 914x411, 868x411, 1920x1080, 2560x720 x UI scale 0.8/1.0/1.4); `camera_rig::tests::north_edge_keeps_the_local_overhead_below_the_top_hud_band` (north wall, idle/north/east aim, 4 viewports x 3 scales); `camera_rig::tests::top_hud_band_never_moves_open_field_or_south_edge_framing`; `look_ahead_keeps_the_fighter_visible_at_every_edge_aspect_and_reach` updated for the band; `hud::screen_tests` asserts the laid-out plate bottom equals `PLATE_BOTTOM * UiScale` at 1280x720, 868x411, 914x411, 1560x720.
- `just check` pass; `just lint` pass (VS Code ripgrep first on PATH); `just test` pass after the last edit (2394 passed, 0 failed).
- Native capture: Practice wipeout-1v1, Feature Yard, measured client walking north (`--move-axis 0,1`) to the north wall (fighter y = 625 of bound 640); scratch data dir, port 127.0.0.1:57231, scratch target dir.
  - 1280x720: /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brlfwtg/wall2-1280x720.txt.captures/brawler-002300.png (camera target y 559.4, fighter 2.05 cells ahead; the clamp alone would have held it at 412, 6.6 cells). Name and health plate sit fully below the score plate.
  - 914x411: /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brlfwtg/wall-914x411.txt.captures/brawler-002300.png. Same result at the phone aspect.

# Playtest note
- Walk to the north wall on any map at desktop and phone. Expect the name, health and ammunition plate to stay just below the score plate, with green outer dressing above the wall. Also check east/west corners along the north wall, and that the walk away from the wall feels smooth: the band is rigid with the fighter, like the BRL-vptw safe frame.
- Known limitation: a strong south look-ahead lean in the open field can still carry the overhead under the plate (deferred to BRL-htdb; needs a fixed aim from BRL-wegn to measure). The band covers the full screen width, not only the plate's columns.

# Learn from errors
- The first test draft asserted that the band always leaves the fighter more than one cell ahead of the look target. At 32:9 the wide view loses depth and the limit is about 0.65 cell. Check aspect extremes before writing a numeric invariant.
- The first native capture ran out of frames before the fighter (70 units/s) reached the wall. Compute the walk time from the capture JSON (spawn position, map bounds, speed) before choosing the capture frames. The evidence script now takes the capture frames from the environment.
- Clippy `float_cmp` rejects `assert_eq!` on f32 in tests. Use tolerance asserts from the start.
