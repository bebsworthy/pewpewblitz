Reviewed 2026-11-15 against HEAD 3437c0d8: valid but duplicates BRL-pw9k — this is pw9k's Dashboard/brawler/weapon subset using the same mechanism. Recommend merging into pw9k and closing this one; if it stays separate, do it as pw9k's first milestone with the plan below.

# Context

BRL-kp7d (done) needed before/after captures of the shell previews. `ui_gallery` (`packages/brawler-client/src/bin/ui_gallery.rs`) shows widgets only, and the layout harness is headless without rendering. `--auto-connect` turns the product shell off (`src/config.rs:376` `presents_product_shell`), so the scheduled `--screenshot-*` path cannot reach these screens through match automation.

The interactive shell does accept `--screenshot-dir` today (`automation/config.rs:273` rejects only headless clients). What is missing is navigation.

BRL-kp7d used a scratch-only patched copy (still present at scratchpad `brl0179-evidence/evidence_drive.rs` and `apply_patch.sh`). On the Dashboard, a frame-counted system wrote `ConfirmCreateBrawler`, `OpenWeaponEquipment(id)`, `CancelWeaponEquipment`, `BackToBrawlerList`, and `CloseBrawlerList` into `PendingFlowActions.ordinary` (`flow/actions.rs:165`), in `ClientFlowSet::CollectFlowInput` after `collect_flow_input` (`flow.rs:168`). It saved a primary-window screenshot after each step, against a scratch supervisor (`--data-directory`, ephemeral port).

Preview hosts: `DashboardPreviewHost` (`flow/screens/dashboard.rs:111`), `BrawlerCardPreview` (`flow/screens/brawlers.rs:116`), and `BrawlerDetailsPreviewHost` (`brawlers.rs:139`), bound in `dashboard.rs`.

# Plan

Implement BRL-pw9k steps 1-2 with routes `dashboard`, `brawler-list` (card), `brawler-details`, and `weapon-equipment`. Use state-driven steps (wait for the flow/overlay) instead of BRL-kp7d's fixed frame numbers, and capture through `request_scheduled_evidence_capture` so the JSON pair is written too. The script and recipe come from pw9k step 4. Depends on BRL-tpd8 (target dir and isolated data) and BRL-hkbm (no toast in later captures).

# Acceptance

- One command writes Dashboard, brawler list/card, details, and weapon-equipment PNG+JSON pairs from a fresh scratch profile, and `data/server/` is untouched.
- The flag is absent from the player build (`just lint` `_client-features` passes); `just test` passes.
