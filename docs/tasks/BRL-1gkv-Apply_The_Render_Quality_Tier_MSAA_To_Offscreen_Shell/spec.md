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


# Implementation (2026-11-18)
- Camera sweep: `Camera3d` is spawned in only five places. Four are the previews named above: `dashboard.rs` (`DashboardPreviewCamera`), `dashboard/card_preview.rs` (`BrawlerCardCamera`, which has moved out of `dashboard.rs`), `dashboard/weapon_preview.rs`, and `presentation_3d/map_palette_preview.rs`. The fifth is the arena camera (`presentation_3d/mod.rs`). `evidence_capture.rs` only queries `Camera3d` and spawns nothing. vfx_lab and ui_gallery spawn no `Camera3d`.
- `render_quality::PreviewCamera` (`pub(crate)` marker) is added to all four preview spawn bundles.
- `apply_camera_quality` gets a second query, `Query<&mut Msaa, With<PreviewCamera>>`, and calls `set_if_neq(tier msaa)` on each match. The arena/HUD query adds `Without<PreviewCamera>` so the two `&mut Msaa` queries stay disjoint. HDR sync is unchanged and covers only the arena and HUD cameras, so previews stay LDR.
- Spawn-time decision: use an `On<Add, PreviewCamera>` observer (`apply_preview_camera_msaa_on_add`, registered in `RenderQualityPlugin`) that sets the tier MSAA when `ActiveRenderQuality` exists. Rejected alternative: pass `Option<Res<ActiveRenderQuality>>` into the four spawn systems. The observer covers every spawn point no matter where it falls relative to `PrepareAssets`, including Map Maker palette rebuilds, so a newly spawned preview never renders a first frame at Sample4 on Low or Medium. If the resource is absent, Bevy's default stays in place until the Update sync runs.

# Verification
- ECS test `presentation_3d::render_quality::tests::preview_cameras_follow_the_tier_msaa_live_without_gaining_hdr`. A preview camera spawned under Low gets `Msaa::Off` right away from the observer. Live preference changes then give Medium → Off, High → Sample4, Low → Off, and the camera never gains `Hdr`. Passes.
- `just lint`: pass, after one `cargo fmt` fix to the new test. `just test`: pass (exit 0).
- Native capture on High: before (HEAD e2c65528) and after (working tree), release client, 1280x720, macOS. Both client logs show tier=HIGH (Auto, desktop GPU). Setup was a scratch-only patched copy using the BRL-kp7d frame-driven flow driver (create brawler → details → weapon equipment → brawler list → Dashboard, screenshot at each step), an isolated supervisor on 127.0.0.1:5291/5292, and data under `target/1gkv/`. I compared the Dashboard, brawler list card, and weapon equipment PNGs by eye and found no visual difference: the same antialiased edges on the brawler and weapon. The only differences are the random profile name and the idle animation phase. This is expected because High is Sample4, the same as the old default.
- Not done here: the Pixel 8a frame-time and power check on Dashboard and Map Maker palette, and the look of previews without MSAA on device. The owner will do those.
