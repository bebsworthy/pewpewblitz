Reviewed 2026-11-15 against HEAD 3437c0d8: valid; spec refreshed (grading lives in `MapThemeProfile`, helpers are `presentation_3d`-private so an export is needed).

# Outcome

VFX-lab captures read like in-match effects: the lab arena camera uses a real map theme's grading (exposure, saturation, contrast) instead of identity grading.

# Context (verified)

- `packages/brawler-client/src/vfx_lab.rs:605` installs the production world look (`install_world_look`) and toon shading; the camera comes from `setup_3d_foundation` (`presentation_3d/mod.rs:1195`, `ArenaCamera` l.1592) with default `ColorGrading`. Nothing in the lab applies theme grading.
- In a match `reconcile_3d_map` (`presentation_3d/mod.rs:1782`) applies the theme: ambient, directional, and `world_look::apply_theme_grading(grid_theme.grading, ..)` (l.1855).
- Per-theme grading is `MapThemeProfile::grading` (`presentation_3d/environment_assets/catalog.rs:149`), read from `assets/catalogs/map_presentation_themes.ron` (themes 1/2/3 at l.14/34/55; theme 1: exposure 1.0, saturation 1.15, contrast 1.05, ambient 160, directional 6000).
- Visibility: `apply_theme_grading` is `pub(super)` (`world_look.rs:326`) and `MapVisualCatalog::theme` is `pub(in crate::presentation_3d)` (`catalog.rs:544`); `vfx_lab.rs` is outside that module.
- Lab lights: directional 6000 lux from `setup_3d_foundation` (mod.rs:1619); lab ambient `vfx_lab.rs:616` is (0.72,0.78,0.90)@160 vs theme 1 (0.72,0.82,0.94)@160.
- Lab flags parsed in `packages/brawler-client/src/bin/vfx_lab.rs:17-104` into `VfxLabConfig` (`vfx_lab.rs:145`, with `validate`).

# Plan

1. Export a `pub(crate)` helper from `presentation_3d` (e.g. `apply_lab_theme(theme_id, world)` or a Startup system) that loads `MapVisualCatalog::embedded`, takes the theme's grading + ambient + directional, and applies them to the `ArenaCamera` / lights.
2. Add `theme: Option<u16>` to `VfxLabConfig` (default 1), parse `--theme N` in `bin/vfx_lab.rs`, validate against the catalog; apply after `setup_lab_scene`. Drop the hard-coded lab ambient in favour of the theme's.
3. Show the active theme in the lab diagnostics text and capture JSON.
4. Document `--theme 1|2|3` in README's VFX-lab section (README.md ~l.110-122).

# Acceptance

- Lab capture of fire field / explosion with `--theme 1` next to a Practice capture of the same effect on a theme-1 map: comparable brightness/saturation (owner visual check).
- Unit test: lab config rejects an unknown theme; the helper sets the camera `ColorGrading` to the theme's.
- `just lint`, `just test`.
