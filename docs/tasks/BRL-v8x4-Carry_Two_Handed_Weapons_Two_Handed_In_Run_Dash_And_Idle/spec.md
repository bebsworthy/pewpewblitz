# Context
BRL-q29d baked two-handed holds only into the combat_idle_<family> and attack_<family> clips. run, dash, idle (Dashboard and brawler previews), hit_front, victory, and ultimate are shared across families and carry every weapon one-handed by the grip.

# Scope
- Per-family (or two-handed vs one-handed) run/idle variants, or an additive support-arm layer, built with the BRL-q29d per-frame IK bake in build_riff.py.
- Keep the clip channel contract and the 700 KB GLB budget; measure the size impact of extra clips.

# Acceptance
- Two-handed weapons keep both hands on them while running and in the Dashboard preview.
- Native captures and owner visual acceptance.
