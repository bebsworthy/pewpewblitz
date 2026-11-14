# Outcome

The Pixel 8a frame cost of High-tier arena cast shadows (BRL-0180 Option A: one cascade spanning the
visible ground) is measured, and `render_quality.ron` is adjusted only if the data says so.

# Context

BRL-0180 measured A against shadow maps off on an Apple M6 desktop only (about +0.2 ms GPU per
frame, no frame-interval change at 120 Hz). Android Auto resolves to Medium, which already has
`shadow_maps: false`, so only a player who picks High on a phone pays for the pass. BRL-0161 measured
the Pixel 8a High tier at 33 FPS average before BRL-0180 (4 near-empty default cascades).

# Scope

- Pixel 8a, release APK, High tier picked in Settings, Practice Wipeout 3v3 Verdant Crossfire with
  bots and scripted move + fire (the BRL-0161 scenario), `dumpsys SurfaceFlinger --timestats`.
- Compare High as shipped vs High with `shadow_maps: false` (one temporary build or catalog edit).
- Record median/p95 frame time, GPU post-acquire median, and thermal status.

# Acceptance criteria

- Recorded cost on the device with the method above.
- A decision: keep High as is, or drop shadow maps from High on Android (with rationale).
