# Context
BRL-0179 needed before/after captures of the shell previews. `ui_gallery` shows widgets only and
the `layout_harness` is headless without rendering; `--auto-connect` disables the product shell, so
the existing `--screenshot-*` schedule cannot reach these screens. BRL-0179 used a scratch-only
patched copy: a system queues `FlowUiAction`s (`ConfirmCreateBrawler`, `OpenWeaponEquipment`,
`BackToBrawlerList`, `CloseBrawlerList`) through `PendingFlowActions.ordinary` and saves a
primary-window screenshot after each, against a scratch supervisor (`--data-directory`, ephemeral
port). Source: /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0179-evidence/evidence_drive.rs
(scratch; may be gone).

# Scope
- An `automation`-feature-only shell capture flag for the interactive product shell, plus a script
  that honours `CARGO_TARGET_DIR` and an isolated data directory (see BRL-tpd8).

# Acceptance criteria
- One command writes Dashboard, details, card, and weapon PNGs; excluded from the player build.
