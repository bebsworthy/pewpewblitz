# Outcome
Lower power use outside matches on Android without making menus feel sluggish.

# Context
Deferred from BRL-zj53 to BRL-ynt5, then split out: with BRL-ynt5's ADPF hint session the
Dashboard already presents at 60 FPS, so a lower menu rate is a power decision, not a frame-rate
fix. Desktop update behaviour must stay unchanged.

# Scope
- Android-only `WinitSettings` per flow: e.g. `UpdateMode::reactive(1/30 s)` in shell flows,
  continuous in matches; input wakes immediately; the 3D Dashboard preview idle motion stays smooth
  enough.

# Acceptance criteria
- Pixel 8a: Dashboard frame rate and `dumpsys batterystats`/power estimate before and after.
- Match frame rate unchanged (60 FPS median).
