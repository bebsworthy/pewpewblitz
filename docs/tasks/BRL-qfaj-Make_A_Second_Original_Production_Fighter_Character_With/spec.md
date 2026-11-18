# Origin

The owner answered BRL-tkmh question q-wr6gc13 on 2026-11-18 with "Make an original character like Riff (Blender, same clip contract)". This ticket comes before BRL-tkmh.

# Scope

- An original character design that reads at the gameplay camera distance, with a silhouette distinct from Riff's ears and goggles. No likeness to existing game characters.
- Built from a builder script under `asset_src/characters/<name>/`, following `asset_src/characters/riff/build_riff.py` and its README: rig, the `CLIP_NAMES` set (including the BRL-v8x4 per-family run/dash clips), `weapon_socket` and support-hand IK onto `grip_secondary`, and `strip_rest_channels`.
- Same budgets as Riff: GLB ≤ 700 KB and a triangle count close to Riff's (9,688). Outline and preview framing come from the shared configs.
- Exported GLB under `assets/brawler/models/original/characters/<name>/`, plus preview sheets.

# Acceptance

- `just lint` and `just test` pass; the client tests' clip-name checks cover the new GLB.
- Native capture sheets of idle, run, dash and attack for all four weapon families, plus the Dashboard preview.

# Not in scope

Choosing the character per brawler belongs to BRL-tkmh, whose question q-m6n2sj1 is still open.
