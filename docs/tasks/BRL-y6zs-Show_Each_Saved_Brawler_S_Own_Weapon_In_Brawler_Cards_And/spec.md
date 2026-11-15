Reviewed 2026-11-15 against HEAD 3437c0d8: valid bug, spec refreshed — cards, details portrait and creation fighter card all share one `BrawlerCardTarget` of the selected brawler's Riff.

# Context
BRL-q29d (13f1cedc) makes the Dashboard Riff hold the selected saved brawler's weapon family. The brawler cards and the brawler-details portrait reuse that one Riff scene, so a non-selected brawler appears with the selected brawler's weapon. The per-weapon panels (equipment, details weapon panel, creation picker) already show the correct family via per-family `WeaponPreviewHost` targets.

# Current code (HEAD 3437c0d8)
- One Riff scene on `DASHBOARD_RENDER_LAYER = 29` (`packages/brawler-client/src/dashboard.rs:21`, spawned :481-501); `sync_dashboard_preview_weapon` (:586-613) attaches `selected_weapon_family(membership)` (`presentation_3d/fighter_weapons.rs:195`).
- `BrawlerCardTarget` (dashboard.rs:347-411): one 768x768 image + camera on layer 29; `bind_brawler_card_previews` (:414-442) gives every `BrawlerCardPreview` host an `ImageNode` of that image.
- `BrawlerCardPreview` hosts: list cards `flow/screens/brawlers.rs:434`, details portrait `flow/screens/brawlers/details.rs:364`, creation fighter choice `flow/screens/brawlers/choice_cards.rs:152` (should show the draft weapon, `BrawlerCreationDraft.weapon_base_id`). The host carries no brawler id today.
- `BrawlerDetailsPreviewHost` (brawlers.rs:139) and its rebinding in `bind_dashboard_preview_host` (dashboard.rs:316-341) are no longer used by production screens (only `dashboard.rs:661` test) — remove or leave, but do not build on it.
- Family lookup: `advertised_weapon_family(catalog, weapon_base_id)` (fighter_weapons.rs:183); `WeaponFamily` has 4 variants (`fighter_character.rs:75`), `WeaponFamily::ALL`/`index()`.
- Pattern to copy: `dashboard/weapon_preview.rs` (`Targets([Option<Target>; 4])`, private layers 28/30/32/33, first host live, others `ImageNode` copies).

# Scope
- Per-family (at most four) Riff card targets: each family gets its own Riff scene + held weapon + card camera on a private layer (new layers clear of 18–24, 28–33, e.g. 34–37), created on demand while a host of that family exists and released when none remain.
- `BrawlerCardPreview` becomes `BrawlerCardPreview(WeaponFamily)` (or carries the brawler's weapon base id); list and details pass that brawler's family, creation passes the draft weapon's family.
- Keep the Dashboard Riff (selected brawler) unchanged.
- Coordinate: BRL-1gkv (mark the new cameras `PreviewCamera`), BRL-y3hb (outline the new scenes).

# Plan
1. Add the family to `BrawlerCardPreview` at the three spawn sites.
2. Replace `BrawlerCardTarget` with `BrawlerCardTargets([Option<..>; 4])`: spawn Riff scene (reuse `spawn_imported_character`/`setup_dashboard_imported_scene` logic parametrised by layer and family), light, camera, image per family in use; bind hosts by family; release unused ones and all on exit (`release_brawler_card_target`).
3. Update dashboard.rs tests (`repeated_dashboard_target_cleanup_returns_assets_to_baseline` :686).

# Acceptance
- ECS test: two cards of different families bind different images; same-family cards share one; targets/cameras/images return to baseline after the list closes; never more than four card targets.
- A card or details portrait shows that brawler's family weapon; creation fighter card follows the draft weapon; native captures with brawlers of 2+ families.
- `just lint`, `just test`.
- Pixel 8a device check: brawler list with mixed families scrolls smoothly; correct weapons shown.

# Verified 2026-11-16
- Implementation: `packages/brawler-client/src/dashboard/card_preview.rs` (per-family targets on layers 34–37, camera marker `BrawlerCardCamera`, which BRL-1gkv/BRL-y3hb can target). ECS tests: distinct layers, different families bind different images, same family shares, at most 4 targets with 12 hosts, baseline cleanup on leaving and on Dashboard exit. `just lint` and `just test` pass.
- Pixel 8a, release APK: created "Brawler 2" with Impact Blade. The creation fighter card switched from Sidearm to blade when the draft weapon changed. The details portrait of Brawler 2 shows the blade while Brawler 1 (Sidearm) is selected. The list shows Brawler 1 with Sidearm and Brawler 2 with blade. The Dashboard Riff still holds the selected brawler's Sidearm. Evidence: scratchpad y6/y05crop.png, y06s.png, y07s.png, y08s.png.
- Not covered: scrolling with 3–4 families live (only 2 brawlers on the phone). Native desktop capture was not made because no automation opens the brawler list (BRL-pw9k).
