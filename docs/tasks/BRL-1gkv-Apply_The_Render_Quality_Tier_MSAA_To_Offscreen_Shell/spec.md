# Context
Found during BRL-kp7d: `render_quality.rs` keeps MSAA equal to the tier only on `ArenaCamera` and
`WorldHudCamera`. Preview cameras (`dashboard.rs`, `dashboard/weapon_preview.rs`,
`map_palette_preview.rs`) keep Bevy's default `Msaa::Sample4` on every tier, including Low.

# Scope
- Mark preview cameras and include them in the tier MSAA sync (no HDR change: previews are LDR).
- Measure Dashboard frame time / power on the Pixel 8a Low tier before and after (BRL-m4eh context).

# Acceptance criteria
- Preview cameras use the tier MSAA; ECS test; no visual regression on High.
