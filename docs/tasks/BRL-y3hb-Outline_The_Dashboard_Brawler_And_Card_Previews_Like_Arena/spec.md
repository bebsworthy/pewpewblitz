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


# Implementation (2026-11-18)
- Marker, not entity: `fighter_outline::PreviewOutline` (`pub(crate)`, re-exported from `presentation_3d`) sits on each preview Riff scene root: the Dashboard `DashboardImportedScene::Character` root (`dashboard.rs` `spawn_imported_character`) and every card family's Riff `CardScene` root (`dashboard/card_preview.rs` `ensure`). The card target serves the brawler list cards, the details portrait, and the creation fighter choice card, so one marker covers all three.
- `attach_preview_outlines` (fighter_outline.rs) walks descendants of marked roots and gives each `MeshMaterial3d<ToonMaterial>` mesh without `FighterOutlined` one hull via the existing `hull_for` cache. The hull spawn is now one shared helper, `spawn_hull`, used by both the arena and the preview pass. Preview hulls insert the root's `RenderLayers` explicitly and copy the body's fixed preview `MeshTag`. `FighterOutlined` prevents duplicates. A held-weapon swap despawns the old weapon together with its hull children, and the new weapon's meshes are outlined on the next pass. The pass is scheduled in `WorldPresentationSet::Animate` (`presentation_3d/mod.rs`) beside the arena chain. That set has no flow run condition, so it runs on the Dashboard and brawler screens, and with no marked root it visits nothing.
- Decision: held weapon, yes. It is a descendant of the Riff root (socket child), so it is outlined like the arena fighter's weapon. Standalone weapon close-ups (`dashboard/weapon_preview.rs`: weapon equipment, creation weapon card, details weapon card), no. They are part illustrations, not a fighter silhouette, and they carry no marker.
- Width: the shader width is world units at the fighter's depth, and the preview Riff is scale 32 against about 17.4 in the arena, framed close. The catalog gains `preview_width_world` (schema 1 → 2, same 0.3..=6.0 bounds), set to 0.8. `FighterOutlineAssets` holds a second `preview_material`. At 0.8 the Dashboard Riff outline is about 2.5 logical px at 1280x720, the details portrait about the same, and list cards about 1 to 1.5 px. That is bold enough to read as the arena silhouette, not a heavy border.
- Found during capture: at preview scale, the old fixed `HULL_DEPTH_PUSH = 2.0` let the face decals (mouth, blush, nose), pockets, and buckle poke their own dark rings through (the mouth read as a moustache). The push is now a material uniform (`width.y`, shader const removed). The arena keeps 2.0 (`HULL_DEPTH_PUSH_WORLD`), and previews use 6.0 (`PREVIEW_HULL_DEPTH_PUSH_WORLD`, a code geometry constant, not tuning). The face is then clean, and only the silhouette and real overlaps (ears, goggles, arms, held weapon) are outlined. The arena look is unchanged.
- Docs: `docs/11-art-and-presentation-direction.md` preview section and outline bullet updated.

# Verification
- ECS test `presentation_3d::combat::fighter_outline::tests::a_preview_root_outlines_its_toon_meshes_once_on_the_preview_layer`: a `PreviewOutline` root on layer 29 with a toon body mesh gets exactly one hull child carrying `RenderLayers::layer(29)` and the preview material across 3 updates (no duplicates). An unmarked weapon-close-up-like root on layer 28 gets none. Catalog tests now also cover `preview_width_world` bounds and the schema bump. Passes.
- `just lint`: pass. `just test`: pass (exit 0, all suites ok, client lib 855 tests).
- Native captures, release client, macOS Retina (PNGs are 2x), tier HIGH. Before = HEAD binary (BRL-1gkv capture client, same source plus the scratch flow driver). After = working tree plus the same scratch-only driver (`flow/evidence_drive.rs`, patched source copy only, never in the real tree). Isolated supervisor on 127.0.0.1:5391/5393, data under `target/y3hb/`. Each run has 1-create-brawler (creation choice card), 2-brawler-details (details portrait), 3-weapon-equipment (close-up, unchanged by design), 4-brawler-list (card), and 5-dashboard:
  - before: `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brly3hb/before/{1280x720,914x411}/`
  - after: `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brly3hb/after/{1280x720,914x411}/`
  - Judged by eye: all four Riff previews show the navy silhouette, held weapon included. The face matches before (thin "w" mouth, no ringed blush). The weapon close-ups are unchanged. At 914x411 the list-card outline is thin (about 1 px) but present.
- Not done here: the Pixel 8a Medium check of Dashboard and brawler list look and frame pacing (one extra hull draw per Riff mesh per preview camera). The owner will do it.

# Mistakes / learning
- The first capture at width 0.8 with the arena depth push outlined the face decals. Lesson: inverted-hull depth push is in world units and must scale with the model's on-screen scale, so check close-up framings of anything that reuses the arena hull.
