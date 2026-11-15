Reviewed 2026-11-15 against HEAD 3437c0d8: valid, spec refreshed — adds the brawler-card camera, and Medium (the Pixel 8a tier) is also MSAA Off.

# Context
Found during BRL-kp7d: `apply_camera_quality` (`packages/brawler-client/src/presentation_3d/render_quality.rs:315-335`) keeps MSAA (and HDR) equal to the tier only on `Or<(With<ArenaCamera>, With<WorldHudCamera>)>`. Every offscreen preview camera spawns with plain `Camera3d::default()`, so it keeps Bevy's required-component default `Msaa::Sample4` on every tier.

`assets/catalogs/render_quality.ron`: high `msaa: Sample4`, medium `msaa: Off` (the Pixel 8a target), low `msaa: Off`. So both Medium and Low pay 4x MSAA on menu previews.

# Preview cameras (HEAD 3437c0d8)
- Dashboard / brawler-details: `packages/brawler-client/src/dashboard.rs:252-272` (`DashboardPreviewCamera`).
- Brawler card target (cards, details portrait, creation choice): `dashboard.rs:387-406` (`BrawlerCardTarget`, 768x768) — not in the original spec.
- Weapon family previews: `packages/brawler-client/src/dashboard/weapon_preview.rs:~137` (up to 4).
- Map Maker palette: `packages/brawler-client/src/presentation_3d/map_palette_preview.rs:85` (up to 6, recreated per rebuild, see BRL-kgba).

# Scope
- Add a `PreviewCamera` marker component (render_quality.rs, `pub(crate)`) on all four spawn sites; extend `apply_camera_quality` to set `Msaa` on marked cameras. No HDR change: previews are LDR (`Bgra8UnormSrgb` targets) — keep HDR sync limited to arena/HUD (split the query or use a second query).
- Newly spawned preview cameras must get the tier MSAA on the first frame (insert the right `Msaa` at spawn from `ActiveRenderQuality` if present, or rely on the Update sync running before render; the latter is fine since sync runs every frame).
- If BRL-y6zs adds per-family card cameras, they carry the same marker.
- Measure Dashboard frame time / power on the Pixel 8a before and after (BRL-m4eh context).

# Plan
1. `#[derive(Component)] pub(crate) struct PreviewCamera;` in render_quality.rs.
2. Insert it at the four spawn sites above.
3. Second query in `apply_camera_quality`: `Query<&mut Msaa, With<PreviewCamera>>` -> `set_if_neq(msaa)`.

# Acceptance criteria
- ECS test in `presentation_3d/render_quality/tests.rs` (pattern of `low_override_applies_live_and_high_restores_the_desktop_look` :159): a `PreviewCamera` gets `Msaa::Off` on Low/Medium and `Msaa::Sample4` on High, live on preference change, and gains no `Hdr`.
- No visual regression on High (native capture of Dashboard + brawler list).
- `just lint`, `just test`.
- Pixel 8a device check: Dashboard and Map Maker palette frame time before/after (logcat or diagnostics overlay), previews still look acceptable without MSAA.
