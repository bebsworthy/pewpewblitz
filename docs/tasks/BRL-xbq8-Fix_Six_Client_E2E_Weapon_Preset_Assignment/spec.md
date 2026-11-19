## Scope

Correct scripts/network-product-match.sh preset cycling for six clients without changing gameplay catalogs. Add a focused shell/config regression where practical.

## Acceptance criteria

- All generated --weapon-preset values are within the supported 1-4 range.
- just e2e 6 completes successfully.
- Existing 2-client and 4-client E2E paths remain valid.

## Re-check on current code (2026-11-19)

**Finding: the original defect no longer reproduces.** In August only weapons 1-4 existed. Today
`content/catalogs/builds.ron` has seven costed weapons (`weapon_costs` 1-7), and the client accepts
any costed weapon ID. The script's literal rotation `1 + (index - 1) % 7` therefore gave clients 1-6
the presets 1-6, all valid.

- Baseline, unchanged script: `BRAWLER_ROUTED_BIND=127.0.0.1:5070 BRAWLER_SERVER_DATA_DIR=target/xbq8/e6-before ./scripts/e2e.sh 6`
  exited 0. Clients created saved brawlers with weapons 1, 2, 3, 4, 5, 6, and one exact 3v3 roster reached Active.

**Remaining gap:** the rotation was still a literal (`% 7`) rather than authored data. AGENTS.md
requires diagnostic loadouts to come from `builds.ron`. If the catalog shrinks or is renumbered,
the bug comes back silently.

## Fix

- New sourced helper `scripts/e2e-config.sh`. `e2e_diagnostic_weapon_for_client` rotates through
  the authored `direct_diagnostic.weapon_base_ids` (currently `[1, 2, 3, 4]`), which is the same
  rotation the server's direct diagnostic loadout uses. It fails closed if the list cannot be read.
- `scripts/network-product-match.sh` takes every `--weapon-preset` from that rotation, including the
  former literal `1` on the Practice, rejoin, completed-rejoin and requeue paths (they use the first
  diagnostic weapon). Six clients now get 1, 2, 3, 4, 1, 2, all inside the 1-4 diagnostic range.
- Regression check `scripts/check-e2e-config.sh`, wired into `just lint` as `_e2e-config`. For 2, 4
  and 6 clients it asserts every assigned preset is in the diagnostic rotation and is a costed
  weapon a saved brawler can select. It also fails if either list cannot be parsed from `builds.ron`.
- README: the lint description names the new check.

## Verification

- `./scripts/check-e2e-config.sh`: passes (`diagnostic weapons (1 2 3 4)`).
- `BRAWLER_ROUTED_BIND=127.0.0.1:5072+n ... ./scripts/e2e.sh N` for N = 2, 4, 6: all exited 0.
  - 2 clients: weapons 1, 2; the 1v1 reached Active.
  - 4 clients: weapons 1-4; the 2v2 reached Active.
  - 6 clients: weapons 1, 2, 3, 4, 1, 2; the 3v3 reached Active.
- `just lint` (with ripgrep on PATH): exit 0, including the new `_e2e-config` check. Commit c5a6821f on branch worktree-agent-a484ae02404e5df00 (not pushed).

## Learning

The August defect was fixed indirectly when the catalog grew to seven weapons, not by the script.
The literal that caused it survived. Re-check old tickets against current code, and replace
literals mirroring authored data with reads plus a lint-time check.
