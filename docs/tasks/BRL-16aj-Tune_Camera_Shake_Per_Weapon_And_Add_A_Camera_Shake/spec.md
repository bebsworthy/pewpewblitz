# Outcome

Camera shake strength fits each weapon's weight and the player's preference.

# Context

BRL-e4xj added a presentation-only camera rig (`src/client/presentation_3d/camera_rig.rs`, tuning in `assets/catalogs/camera_rig.ron`). Its shake channel uses one authored local-fire trauma for every weapon and one local-damage trauma regardless of amount; the only player control is the existing reduced-motion / reduced-combat-effects toggles, which disable shake entirely. Barrel blast damage to the local fighter only shakes through the distance-scaled explosion trigger because environment damage emits legacy cues that `DeduplicatedCombatCue` filters out.

# Scope (decide from BRL-e4xj playtest feedback)

- Optional per-weapon (or per delivery family) fire-trauma scaling from authored presentation data, not gameplay catalogs.
- Optional damage-taken trauma scaled by damage amount relative to max health.
- A camera shake intensity setting (for example Off / Low / Full) using the existing `ClientShellSettings` persistence and settings UI pattern.

# Out of scope

Camera angle/FOV (BRL-xyj7); gameplay changes.

# Acceptance criteria

- Shake stays zero under reduced motion and reduced combat effects.
- Settings persist and migrate through the existing settings validation.
- Focused tests plus a native playtest.

# Decisions (2026-11-14)

- BRL-e4xj playtest feedback accepted the rig with no corrections, so this ticket keeps the rig and adds the three optional scope items.
- Per-weapon fire shake is keyed by the controlled fighter's replicated primary-weapon delivery family (`ResolvedMatchLoadout.primary_weapon.recipe.delivery`), with a multiplier for the Spread firing pattern. It is not keyed by preset ID, because weapons are player-authored from parts. A player-built weapon takes its family's value. This matches how the fire-sound producer chooses sounds. An unknown loadout falls back to the straight-shot value.
- Ultimate casts (`LocalAttackKind::Ultimate`) get their own authored `local_ultimate` trauma (0.14, the old single value) instead of a weapon-family value.
- Damage-taken trauma is `minimum + (maximum - minimum) * clamp((amount / maximum_health) / full_strength_health_fraction, 0, 1)`, using the controlled fighter's replicated `fighter_stats.maximum_health`. If maximum health is unknown, the minimum is used.
- Camera shake setting: `ClientShellSettings.camera_shake` is a percentage from 0 to 100 in steps of 10, default 100. It is persisted as a `#[serde(default)]` field in `SettingsFile`, so older files load at full shake, and it is validated like the volumes. It appears as a slider on the Accessibility page, below Reduced combat effects, and reuses the existing 10%-step slider path for pointer, touch, gamepad and keyboard. It scales only the shake amplitude. Trauma still accumulates and decays the same way, so changing the setting mid-shake has no lag. Reduced motion and reduced combat effects still force shake to zero whatever the slider says.
- `assets/catalogs/camera_rig.ron` is now schema 2 (`local_fire` table, `local_ultimate`, `local_damage` table), with code-owned ceilings: each trauma value is in 0..=1, `spread_scale` is in 1..=4, the damage maximum is at least the minimum, and the health fraction is in 0.01..=1. It is a client presentation catalog only. The gameplay content fingerprint, the protocol and the server feature graph are unchanged.

# Implementation (2026-11-14)

- `packages/brawler-client/src/presentation_3d/camera_rig.rs`: adds `LocalFireTrauma::for_weapon`, `LocalDamageTrauma::for_hit` and `CameraRigFrameInput.shake_scale`, which is clamped to [0, 1] and set to 0 when not finite. `accumulate_camera_trauma` now reads the controlled fighter's optional `ResolvedMatchLoadout`.
- `presentation_3d/camera.rs`: passes `settings.camera_shake_scale()` to the rig.
- `settings/persistence.rs`: adds the `camera_shake` field, its default, validation, `camera_shake_scale()`, and backward-compatible loading.
- `shell.rs`, `shell/settings_screen.rs`: add the `CameraShakeUp` row and `CameraShakeDown`/`CameraShakeUp` actions, the slider value, pointer field, Accessibility reset, and description text.
- Authored values: straight 0.12, sticky 0.16, lobbed 0.22, melee 0.16, cone spray 0.08, splash 0.14, spread ×1.75 (Scatter Cannon 0.21 against Pulse Sidearm 0.12), ultimate 0.14; damage from 0.2 to 0.6, reaching full strength at 30% of maximum health.
- `docs/11-art-and-presentation-direction.md`: the camera rig paragraph now describes per-family fire shake, damage-scaled shake and the intensity setting.

# Verification (2026-11-14)

All runs used a scratch `CARGO_TARGET_DIR` and did not touch the running server.
- Focused: `cargo test --locked -p brawler-client --no-default-features --features brawler/client --lib -- camera settings_screen persistence`: 52 passed. New tests:
  - `heavy_weapons_shake_more_than_light_ones` (embedded presets: Scatter Cannon and Arc Launcher above Pulse Sidearm, Spray below it, every preset within [0, 1])
  - `damage_trauma_scales_with_the_hit_share_of_maximum_health`
  - `shake_intensity_scales_amplitude_and_zero_turns_it_off` (half scale gives half the offset, 0 or NaN gives none, out-of-range values are clamped, trauma decay does not depend on scale, reduced motion and reduced effects override the scale)
  - `ecs_local_fire_follows_the_loadout_and_damage_follows_the_hit`
  - `ecs_camera_shake_setting_scales_the_applied_offset` (100/50/0)
  - `camera_shake_round_trips_validates_and_older_files_load_at_full_shake` (35 and 110 are rejected)
  - `accessibility_page_owns_a_camera_shake_slider_independent_of_reduced_motion`
  - Updated: the catalog ceiling and schema test, plus the expected local-fire trauma in the existing ECS test.
- `just check`: pass. `just lint`: pass, with VS Code's bundled `rg` first on PATH; the V3/V8 cleanup scripts printed their pass lines. `just test`: pass (24 suites, 2336 passed, 0 failed).
- Not run: `just e2e` (no routed or protocol change) and native visual evidence (the agent has no display session). Both are covered by the playtest below.

# Playtest handoff

Run `just run 2` and open Practice on any map; Feature Yard Heist has barrels.
- Swap brawlers between Pulse Sidearm, Scatter Cannon, Arc Launcher and Spray, and fire continuously. Spray should barely shake, the Sidearm lightly, and Scatter Cannon and Arc Launcher should give a clearly heavier kick.
- Take a few light hits (spray or field ticks) and a few heavy ones (sticky or sidearm). Heavy hits should shake more.
- Settings > Accessibility > Camera shake: drag or tap the slider (also with a touch screen and with a gamepad left/right) to 50% and then 0%. Shake should halve and then stop, while look-ahead and follow are unchanged. Restart the client and check that the value persisted. Reset Accessibility should restore 100%.
- With the slider at 100%, Reduced motion or Reduced combat effects should still give no shake at all.

Observations wanted:
- Is the light-to-heavy spread right?
- Is Arc Launcher or Scatter Cannon now too strong?
- Do damage-over-time ticks feel too busy?
- Is 10% the right slider step?

Known limitations: barrel and environment damage to the local fighter is still not damage-scaled (BRL-75dh). Fire shake is per delivery family, not per individual weapon part.

# Learn from errors

- In a test, a loop variable named `camera_shake` shadowed the existing `camera_shake(&mut App)` test helper and caused E0618. My first fix was a pointless wrapper function; the right fix was to rename the binding. Lesson: name loop bindings after their role (`intensity`), not after a field or helper name already in scope.
- The background `cargo test | tail` hid a compile error behind a successful pipeline exit. Lesson: grep the log for `error` and `test result` lines, not just the exit status of the pipeline.


# Closeout (2026-11-14)

Moved to done under the owner's process: implementation and agent verification are complete. Owner playtest happens on done tickets; open decisions above stay recorded for that review, and changes come back as a reopen or a new ticket.
