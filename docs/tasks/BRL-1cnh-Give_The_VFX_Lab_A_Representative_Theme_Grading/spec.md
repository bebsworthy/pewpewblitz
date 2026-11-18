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


# Result (2026-11-18)

## Implemented

- `presentation_3d/mod.rs`: extracted `apply_theme_look` (theme ambient, directional colour/lux, `world_look::apply_theme_grading`), now used by `reconcile_3d_map`, and exported one `pub(crate) apply_map_theme_look(world, theme_id)` (automation-gated) that loads the embedded `MapVisualCatalog` and applies that same look to a scene without an accepted map. `MapThemeProfile` re-exported inside `presentation_3d` only.
- `vfx_lab.rs`: `VfxLabConfig::theme: Option<u16>` (default 1), validated against the embedded `MapContentCatalog` presentation themes (the client theme catalog is test-covered to match it exactly). Startup `apply_lab_theme` runs after `setup_3d_foundation`; the hard-coded lab `GlobalAmbientLight` is removed (Bevy's default resource is overwritten by the theme). Technical diagnostics show `Theme N lighting + grading`; capture JSON has `lab.theme`.
- `bin/vfx_lab.rs`: `--theme N`; `parse_next` / `parse_window_size` helpers extracted to keep `parse_args` under the line limit.
- README VFX-lab section documents `--theme 1|2|3`.

## Verification

- Unit tests: `presentation_3d::tests::map_theme_look_applies_the_production_theme_grading_and_lights`, `vfx_lab::tests::configuration_accepts_only_embedded_map_themes`.
- `just lint` pass (target/1cnh/lint.log); `just test` pass (exit 0, no failures) (target/1cnh/test.log).
- `brawler-vfx-lab --theme 9` exits 2 with "VfxLab theme 9 is not an embedded map theme".
- Lab captures (paused, `--capture-after 20 --exit-after-capture`): before the change `target/1cnh/before-field/`, `target/1cnh/before-explosion/`; after with `--theme 1` `target/1cnh/after-field/`, `target/1cnh/after-explosion/`, `target/1cnh/after-lob/` (`--effect combat.impact --lobbed-impact --radius 150 --sample-ticks 4`). Capture JSON records `"theme": 1`.
- Practice on feature-yard-wipeout (theme 1): `BRAWLER_RENDER_BIND=127.0.0.1:5036 BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_COMBAT=1 BRAWLER_RENDER_WEAPON=3`. The script's fixed capture schedule (updates 300/480/660) lands before or at match start, so `target/1cnh/practice/` shows no landed effect (its report also fails `sample_count` because I shortened the window to 4+8 s; not a regression). For the effect I ran a scratch copy of the script differing only in `--screenshot-first 840 --screenshot-every 20` and 16 captures, with `BRAWLER_RENDER_AIM=left` so the Arc Launcher lob lands on screen: `target/1cnh/practice-left-w3/` (update 1100 shows the lob explosion body).
- Comparison sheet `target/1cnh/compare.png` (3x2, no text labels; ffmpeg here lacks drawtext): row 1 fire field before | after; row 2 world-object.explosion before | after; row 3 lab lobbed impact `--theme 1` | Practice Arc Launcher lob (theme 1).
- My read: with theme 1 the lab picks up the warm sun, +1 EV exposure and 1.15 saturation; the explosion core shifts from orange-red to the same salmon-orange as in Practice and the fire-field interior reads deeper red. The lab's dark neutral floor now reads dark brown, so the effect still sits on a much darker background than the bright Feature Yard floor.

## Owner visual check

Open `target/1cnh/compare.png`. Compare the right column of rows 1-2 (after) with the left (before), and row 3 left (lab) with row 3 right (Practice): the effect hues and brightness should look alike; the background floor intentionally differs (the lab floor is not a map floor). To check live: `just vfx-lab '--theme 1 --effect combat.impact --lobbed-impact --radius 150'`, press I to see the Theme line; try `--theme 2` / `--theme 3`.

## Not done / limits

- No pre-change lab capture of the lobbed impact (the old binary was rebuilt before I added that scenario); before/after is shown for fire field and world-object.explosion.
- The fire field (ultimate) was not captured in Practice: render automation cannot pick an ultimate, so Practice evidence is the Arc Launcher lob only.
- Diagnostics text `Theme` line not visually captured (the capture shows the guide, not technical details).
