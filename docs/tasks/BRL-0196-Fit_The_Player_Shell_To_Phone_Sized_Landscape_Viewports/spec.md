# Outcome
Shell screens (Server Select, Dashboard, Brawlers, Create Brawler, errors) fit a landscape phone viewport.

# Evidence (BRL-0159 device smoke, 2026-11-13, Pixel 8a, 2400x1080 at density 420, ~914x411 logical)
- Dashboard "Create your first brawler": the fighter preview is clipped at the top and the layout runs
  off the bottom.
- "Connection lost" error panel: the Retry button is cut off at the bottom edge.
- Server Select: the Server Address and Player Name rows show only their labels; the prefilled values
  were not visible, although Connect used the prefilled address.
- Screens were designed around a 16:9 desktop window; phone aspect ratios are about 20:9 and the logical
  height is small.

# Scope
- Choose a UI scale or layout rule for small logical heights (respecting the existing UI-scale setting).
- Fix the Server Select value rendering and the clipped Dashboard and error layouts.
- Respect display-cutout safe areas together with BRL-0195 if that ticket has not landed yet.

# Acceptance criteria
- Device screenshots of each shell screen with no clipped controls and visible field values; desktop
  layout unchanged at 1280x720 and above.
