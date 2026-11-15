Reviewed 2026-11-15 against HEAD 3437c0d8: valid, spec refreshed — previews still have no outline; note the details portrait now uses the card target.

# Context
BRL-kp7d (c967d37a) made the shell previews toon-shaded with the arena rim, but `attach_fighter_outlines` (`packages/brawler-client/src/presentation_3d/combat/fighter_outline.rs:308`) only walks `V3FighterVisual` roots whose `CombatVisualOwner` is a replicated `Fighter`, so the previews have no outline.

# Current code (HEAD 3437c0d8)
- Hull builder: `smoothed_hull_mesh` (fighter_outline.rs:218), cached by `hull_for` (:281) in `FighterOutlineAssets` (cap `MAX_OUTLINE_HULL_MESHES = 64`, :42); shared `assets.material`; hull spawned as `ChildOf(body)` with `FighterOutlineHull`, `NotShadowCaster/Receiver`, skinned meshes get `SkinnedMesh` + `NoFrustumCulling` (:383-401). Tuning `assets/catalogs/fighter_outline.ron`, shader `assets/shaders/fighter_outline.wgsl`; width is world units at the fighter's depth (bounds 0.3–6.0, :37-38).
- Scheduled in `presentation_3d/mod.rs:497` (Animate set, chained before `tag_fighter_rim_relation`).
- Previews (`packages/brawler-client/src/dashboard.rs`): one Riff scene on `DASHBOARD_RENDER_LAYER = 29` (:21), spawned by `spawn_imported_character` (:481) with `player_preview_toon_scene()` and `DASHBOARD_RIFF_SCALE = 32.0` (`presentation_3d/fighter_character.rs:31`); `setup_dashboard_imported_scene` (:532) copies the layer onto descendants at `WorldInstanceReady`. Rendered by the Dashboard camera (:255-272) and by the shared `BrawlerCardTarget` camera (:356-411).
- The `BrawlerCardPreview` host is used by brawler list cards (`flow/screens/brawlers.rs:434`), the brawler-details portrait (`flow/screens/brawlers/details.rs:364`) and the creation fighter choice card (`flow/screens/brawlers/choice_cards.rs:152`). `BrawlerDetailsPreviewHost` (brawlers.rs:139) is no longer spawned outside a test.
- Weapon close-ups: `dashboard/weapon_preview.rs` (layers 28/30/32/33).

# Scope
- Reuse `hull_for` for preview body meshes (toon meshes under `DashboardImportedScene::Character`); hulls must get the preview `RenderLayers` explicitly (RenderLayers are not inherited from the body).
- Outline width: world units at the fighter's depth; check the Dashboard (scale 32, camera at ~(205,72,125)) and card framing; may need a preview-specific width or a camera-distance factor.
- Decide whether the held weapon (child of the Riff socket) and the standalone weapon close-up get an outline at all (suggest: weapon in hand yes, close-up no).
- Mind Low/Medium tiers (BRL-ynt5): one extra hull draw per body mesh per preview camera.
- Coordinate with BRL-y6zs: if per-family Riff preview scenes land first, outline those too (write the attach so it keys off a preview marker, not a single entity).

# Plan
1. Add a `PreviewOutline` marker (or reuse `DashboardImportedScene::Character`) and a system `attach_preview_outlines` in fighter_outline.rs that walks descendants of marked roots, spawns hulls like :383-401 with the root's `RenderLayers`, and inserts `FighterOutlined`.
2. Expose it `pub(crate)` and schedule it for shell flows (it must run outside matches too).
3. Tune width for preview scale; add catalog field if a separate value is needed.

# Acceptance criteria
- ECS test (extend `presentation_3d/combat/fighter_outline/tests.rs`): a preview root with a toon body mesh on layer 29 gets exactly one hull child carrying `RenderLayers::layer(29)`; re-running does not duplicate.
- Dashboard, brawler-details, brawler-card, and creation choice-card previews show the arena outline; native captures at 1280x720 and 914x411.
- `just lint`, `just test`.
- Pixel 8a device check: Dashboard and brawler list look and frame pacing unchanged on Medium.
