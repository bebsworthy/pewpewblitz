# Outcome

VFX-lab captures read like in-match effects: the lab arena camera uses a real map theme's grading (exposure, saturation, contrast) instead of identity grading.

# Context (from BRL-farz)

- `packages/brawler-client/src/vfx_lab.rs` installs the production world look (HDR, TonyMcMapface, bloom) but leaves the arena camera's `ColorGrading` at identity; in a match `reconcile_3d_map` applies the theme grading (Feature Yard: exposure +1.0, saturation 1.15, contrast 1.05).
- Bloom is decided before grading, so BRL-farz's bloom evidence is unaffected; only perceived brightness/saturation differs from a match.
- The lab already uses the default 6000 lux directional and 160 ambient, which match theme 1.

# Scope

- Apply one theme's grading in the lab (default: the theme of the default map), optionally selectable by a `--theme` automation flag, using `MapVisualCatalog` and `world_look::apply_theme_grading`.
- Document the flag in README's VFX-lab section.

# Acceptance criteria

- Lab capture with theme grading next to an in-match capture of the same effect looks comparable.
- `just lint`, `just test` pass.
