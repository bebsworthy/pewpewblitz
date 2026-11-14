# Context (found during BRL-0197, 2026-11-14, Pixel 8a)
- Choosing SCOREBOARD in the match menu still shows the old monospace text panel
  ("T1 YOU Brawler-42DD W1 HP 85/85 B0 ready") from `presentation.rs` `spawn_client_hud`. At 914x411
  it is small, unstyled, and reads as debug output. BRL-0155 left this panel out of scope; the
  results screens (BRL-0132/BRL-0135) do not cover the live in-match scoreboard.
- BRL-0197 made a tap or click anywhere dismiss the latched panel back to the menu; there is still no
  visible close affordance.

# Scope
- Present the live scoreboard with the shared shell/HUD surfaces, team colors, and observer-safe
  facts only; keep the held (Tab/Select) and latched (menu) entry points.
- A visible touch-reachable close/back control in addition to Back and tap-anywhere.

# Acceptance criteria
- Readable at 868x411 and 914x411 without overlap; desktop 1280x720 unchanged in behavior.
- Layout test at the four HUD viewports; Pixel 8a check.
