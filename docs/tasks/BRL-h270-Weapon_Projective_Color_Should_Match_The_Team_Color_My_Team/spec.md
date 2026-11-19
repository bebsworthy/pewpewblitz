# Research (2026-11-19)

## How projectile and weapon VFX colour is chosen today

- `presentation_3d/combat/entities.rs::spawn_projectile` is the single materialization path for
  replicated projectiles and the local input-frame cosmetic (`local_projectile.rs`). It receives the
  projectile `TeamId` and a `CombatVisualSignature` whose `relationship` is already observer-relative
  (`VfxObserver::relationship`).
- Colour is chosen from the **absolute** team, not the viewer: `team_material(team)` (team 1 →
  `team_red`, otherwise `team_blue`) for the plain/primitive/lobbed body and the elemental fallback;
  `straight_projectile_style.rs` picks `projectile_hot_{blue,red}` and
  `projectile_trail_{blue,red}{,_reduced}` with `team.0 == 1`. A viewer on team 1 therefore sees
  their own shots red and enemy shots blue.
- Elemental projectiles (`elemental_style.rs::spawn_elemental_projectile_style`) already tint a
  "relationship core" from `signature.relationship` (`effect_ally`/`effect_enemy`) but fall back to
  the absolute team material for `Neutral`.
- The neutral straight muzzle flash (`assets/catalogs/vfx.ron` recipe `neutral_straight_muzzle`)
  uses the fixed `NeutralBright` (white-cyan) core and `NeutralSoft` (cyan) shell and echo for
  everyone, so an enemy muzzle flash reads blue. Elemental muzzles use their element colour.
- Sentry bodies (`entities.rs::spawn_sentry`) use the absolute `team_material`.
- Bug found: `VfxObserver::relationship` and both `relationship_for_team` helpers treat
  `TeamId(0)` as `Neutral`. `TeamId(0)` is a real playing team (`matchplay/spawns.rs`); the
  reserved neutral identity is `brawler::combat::NEUTRAL_TEAM` (`TeamId(u8::MAX)`, practice dummy
  and Zombie NPCs). So a team-1 viewer saw every team-0 attack as `Neutral` (absolute fallback),
  and a team-0 viewer's teammates were `Neutral`.
- Existing observer-relative colour source: `assets/catalogs/toon_shading.ron` rim `ally`
  `(0.12, 0.72, 0.96)` and `enemy` `(1.0, 0.18, 0.14)`, exposed as `ToonShading::rim_rgb`. The same
  values are the ground ring (`marker_ally`/`marker_enemy`), nameplate and overhead-health
  colours, and the fighter outline's relationship tint already reads `rim_rgb`.

# Design

- Pure rule in `vfx/signature.rs`: `VfxRelationship::weapon_team_tint(team) -> WeaponTeamTint
  {Ally, Enemy}`. `SelfOwned`/`Ally` → Ally (blue), `Enemy` → Enemy (red). `Neutral` (no local
  team yet, or the reserved `NEUTRAL_TEAM`) falls back to the absolute side convention the client
  used before: team 1 red, every other team blue, so the two sides stay distinguishable without a
  viewer. Unit-tested as a table over (local team, projectile team) through the production
  `VfxObserver::relationship`.
- Fix `VfxObserver::relationship` / `relationship_for_team` to treat only `NEUTRAL_TEAM` (and a
  missing observer) as `Neutral`. This also corrects elemental field, concealment, and cue
  relationships for team-0 fighters, which is the documented intended behaviour.
- Colours come from `ToonShading::rim_rgb(Ally/Enemy)` at `setup_3d_foundation`, not new literals:
  the hot nucleus emissive and trail HDR colour are the rim colour converted to linear and scaled
  to the existing glow peaks (6.0 for the nucleus, 2.6 for the trail, so bloom tuning is
  unchanged); the plain/primitive/lobbed body and sentry body use a matte material of the rim
  colour. Materials are renamed `_ally`/`_enemy` (they are no longer absolute-team materials).
  Enemy shots become true relation red instead of the old orange-red, which also separates them
  from the Fire accent.
- Muzzle flash: new catalog material key `RelationshipSoft`, resolved by the generic transient
  adapter from `request.signature.relationship` (ally soft / enemy soft, `Neutral` keeps
  `NeutralSoft`). The neutral straight muzzle shell and echo use it; the white-hot core stays
  `NeutralBright`. Elemental muzzles keep their element colour (identity), as do impacts.
- Weapon identity: mesh, scale, trail length, elemental fins/lobes/bubbles/sparks unchanged; only
  hue channels change.
- Practice vs bots: bots carry real `TeamId`s, so bot teammates read blue and opposing bots red.
- No local player (before the controlled fighter replicates; the observer is sticky after death):
  absolute fallback above.
- VFX Lab: default relationship `Neutral` now uses `NEUTRAL_TEAM` facts (it relied on the
  team-0-is-neutral bug), so it shows the blue default (viewer-side hue); `O` cycles to ally
  (blue) and enemy (red), which also tints the neutral muzzle shell.
- Out of scope (recorded): impacts and the `NeutralSparks` particle colour stay neutral; absolute
  team colours for fighter primitive bodies, Heist safes, Hot Zone, and concealment fields are
  unchanged.

# Implementation (2026-11-19)

- `vfx/signature.rs`: `WeaponTeamTint`, `VfxRelationship::weapon_team_tint`,
  `VfxObserver::team_relationship` (only `NEUTRAL_TEAM` / no observer is `Neutral`); both
  `relationship_for_team` helpers (`vfx/producers/combat.rs`, `presentation_3d/combat/entities.rs`)
  now delegate to it.
- `presentation_3d/mod.rs`: projectile materials renamed `_ally`/`_enemy` and built from
  `ToonShading::rim_rgb` via `relation_color` / `relation_glow` (peaks `GLOW_PROJECTILE_PEAK` 6.0,
  `PROJECTILE_TRAIL_PEAK` 2.6); new `weapon_body_{ally,enemy}` and `effect_{ally,enemy}_soft`;
  `team_material` removed (its only users were projectiles and sentries), replaced by
  `weapon_body_material(tint)`.
- `straight_projectile_style.rs`, `entities.rs` (projectile + sentry), `effects.rs`
  (`VfxMaterialKey::RelationshipSoft`), `vfx/catalog.rs`, `assets/catalogs/vfx.ron`
  (neutral muzzle shell/echo/shell_reduced), `vfx_lab.rs` (Neutral fixture uses `NEUTRAL_TEAM`),
  `world_look/tests.rs`, `docs/11-art-and-presentation-direction.md` (weapon team tint rule).

# Verification (2026-11-19)

- New tests: `vfx::signature::tests::weapon_team_tint_is_blue_for_the_viewer_side_and_red_for_the_other_on_every_team`
  (pure table over viewer team x projectile team, incl. no viewer and `NEUTRAL_TEAM`),
  `team_zero_is_a_playing_team_and_only_the_reserved_team_is_neutral`,
  `straight_projectile_style::tests::bolt_nucleus_and_wake_use_the_viewer_relative_tint_in_rim_colors`
  (App test: spawned nucleus/wake materials per tint; body colours equal the toon rim colours),
  `effects::tests::neutral_straight_muzzle_shell_follows_the_viewer_relative_weapon_tint`
  (material resolution per relationship + production recipe tints only shell and echo);
  updated `trail_materials_encode_quality_and_relation_color`, `glow_materials_exceed_the_bloom_threshold`,
  `relationship_fixture_facts_match_production_observer_inputs`.
- `just lint`: pass. `just test`: pass (all suites incl. network and performance).
- `BRAWLER_RENDER_BIND=127.0.0.1:5041 BRAWLER_RENDER_COMBAT=1 scripts/v3-render-evidence.sh target/h270/render.txt`:
  the run reported `result=fail first_failure=sample_count` and all three captures were still in the
  routed Lobby (no fighters). The machine was heavily loaded (load avg ~16, frame p50 53 ms, a
  concurrent BRL-vh61 render-evidence/build run), so this is not evidence about this change.
- `BRAWLER_RENDER_BIND=127.0.0.1:5042 BRAWLER_RENDER_COMBAT=1 BRAWLER_RENDER_PRACTICE=1 scripts/v3-render-evidence.sh target/h270/practice.txt`:
  passed. `target/h270/practice.txt.captures/brawler-000480.png` shows the viewer's (team 0) own
  Pulse shot as a blue nucleus and wake; the opposing bot did not fire on camera.
- Off-screen VfxLab captures (`brawler-vfx-lab --fixture straight|lobbed --relationship ally|enemy|neutral --offscreen --capture-after 40 --exit-after-capture`):
  `target/h270/lab-straight-enemy/*.png` shows the enemy bolt nucleus and wake in relation red
  (no longer orange); `target/h270/lab-straight-ally/*.png` blue; `target/h270/lab-lobbed-enemy/*.png`
  red lobbed body; neutral fixtures blue (documented Lab default). The muzzle flash was not captured
  on screen (too short for the fixed capture frame); its resolution is covered by the unit test.
- Not verified on screen: a team-1 viewer (render evidence cannot pick the team). Covered by the
  pure table test, which also exercises the fixed team-0 relationship bug.

# Device playtest (Pixel, owner)

Run: `just server` with `BRAWLER_ROUTED_BIND=0.0.0.0:5000`, then `just android-build <host:5000>` /
`just android-install`; optionally a second client with `just client` so teams vary.
Observe:
1. Practice vs bots: your shots and bot teammates' shots blue, opposing bots' shots red; the red
   matches the enemy ground ring/nameplate red and reads differently from Fire orange.
2. Play a few matches until you are on the second team (right-side scoreboard team in the lobby
   sense; team 1): your own shots must still be blue and enemy shots red.
3. Muzzle flash: the soft shell/echo around a neutral straight shot is blue for you/allies, red for
   enemies; the white-hot core stays white.
4. Elemental weapons (Cold/Fire/Poison/Restoration): element pattern unchanged; the small inner core
   is blue/red by side.
5. Lobbed weapon and sentry ultimate: body blue for your side, red for the other side.
6. Reduced effects (Settings) and low render quality: the same blue/red distinction holds.
Known limits: impacts and spark particles keep neutral/element colours; fighter primitive bodies,
Heist safes, Hot Zone and concealment fields keep absolute team colours.
