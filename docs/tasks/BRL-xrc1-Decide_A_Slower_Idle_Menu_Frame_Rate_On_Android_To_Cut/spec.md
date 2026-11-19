# Context
BRL-fs5k cut the idle Dashboard on the Pixel 8a by about 20-25% (single-threaded menu executors, no arena pass behind opaque screens, materials written only on change). It still uses roughly one core: the remaining cost is broad per-frame work across many menu systems, not one hotspot.

# Options
- Drop the Android idle menu rate below 30 FPS (e.g. update only on input or at a low reactive rate) — a product call on how menus feel (animations, the Dashboard character).
- Or profile per-system menu costs and trim the largest ones.

# Acceptance
Owner chooses the approach; idle Dashboard CPU measured with the BRL-fs5k method before and after.
