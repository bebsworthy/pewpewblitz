# Outcome

Every player-visible mode name is original. Today Wipeout, Hot Zone, and Heist (shipped) and Showdown (planned in BRL-5ss9) match another commercial game's mode names. BRL-wrer already dropped "Gem Grab" for the same reason. Rules, balance, maps, and match behaviour do not change.

# Scope

Player-facing text:
- mode selection labels: the `selection_label` values in `src/modes/{wipeout,hot_zone,heist}.rs`
- the Map Maker mode labels (`packages/brawler-client/src/flow/screens/map_maker/library.rs:829-832`)
- game-type display names in `config/server/game-types.ron`
- HUD, results, lobby, and Practice text
- the Balance Lab editor labels (`src/server/balance_lab/editor.rs`) and the web operator app
- the README and the owning `docs/` (`04-maps-and-game-modes.md`, `18-damageable-world-objects-and-heist.md`, `13-player-ux.md`)

Planned work: update the BRL-5ss9 title, description, and spec to the new Showdown name. Keep BRL-wrer's chosen name consistent with the new set.

Internal identifiers (`GameMode` variants, mode keys, game-type IDs such as `wipeout-3v3`, `just` recipe defaults, script arguments, file and module names) follow the owner's answer to the identifiers question.

Historical records under `docs/implementation/` and done tickets are never rewritten.

# Constraints

- If internal IDs change, this is a global protocol compatibility bump and a content-fingerprint change (`docs/08-network-architecture.md`). Advertised game-type IDs change too, so routed admission, stored published-map submissions, `data/` fixtures, and any operator config keyed by game-type ID need an explicit cutover. This project has no compatibility decoders, so the cutover is a one-time rename, not an alias.
- Use one consistent name set. Each name must be checked against well-known game mode names before it is accepted.
- No rule, map, or balance edits ride along.

# Acceptance criteria

- No player-visible surface (client, HUD, results, Balance Lab, web app, README, owning docs) shows Wipeout, Hot Zone, Heist, or Showdown.
- `rg -i 'wipeout|hot.?zone|heist|showdown'` outside `docs/implementation/`, `docs/tasks/`, and `references/` returns only what the identifiers decision deliberately keeps.
- `just lint`, `just test`, and the e2e matrix (`just ci`) pass, and routed admission advertises the renamed game types.
- Screenshots of the mode selection and results screens are attached.

# Playtest notes (owner, after done)

The owner reads the mode picker, HUD, and results in a native client and confirms the names read well.


# Decisions

- 2026-11-20, owner (q-446v98f): Set A. Wipeout → **Takedown**, Hot Zone → **Turf Hold**, Heist → **Vault Break**, Showdown → **Last Stand**.
- 2026-11-20, owner (q-vmzjrnz): rename **everything**. This covers the `GameMode` variants (`Takedown`, `TurfHold`, `VaultBreak`), mode keys and game-type IDs (`takedown-*`, `turf-hold-*`, `vault-break-*`), the matching map keys (for example `feature-yard-wipeout`), modules and files (`src/modes/`, `src/matchplay/`), `just` recipe defaults, scripts, tests, and types such as `HeistSafe`. The rename comes with the global protocol compatibility bump, the content-fingerprint change, and a one-time cutover of game-type-keyed data and published-map submissions. There are no aliases or compatibility decoders. `docs/18-damageable-world-objects-and-heist.md` is renamed in place, with its inbound links updated.

# Implementation decisions (2026-11-21, agent)

- **Mechanical rename** by a one-off script (not committed): every tracked text file outside the excluded paths below, plus `git mv` of every file/dir whose path contains an old name. Casing rules: `Wipeout/wipeout/WIPEOUT` → `Takedown/takedown/TAKEDOWN`; `Hot Zone/HotZone/hot_zone/hot-zone/HOT_ZONE/hotzone` → `Turf Hold/TurfHold/turf_hold/turf-hold/TURF_HOLD/turfhold`; `Heist` → `VaultBreak` in identifiers, `Vault Break` in prose and player strings, `vault_break` (snake), `vault-break` (keys), `VAULT_BREAK`/`VAULT BREAK`. Rust/RON were lexed so comments and string literals got prose forms and code got identifier forms.
- **Kept on purpose (rg allowlist):** `docs/06-research-sources.md` (it cites the other game's mode list as research); `audit/` (dated audit reports), `inspiration/`, and `docs/design/player-shell/archive/` (historical exploration records, like `docs/implementation/`).
- **Wire/storage:** `GameMode` wire indices are unchanged (1..4); renamed type paths change the registry fingerprint; renamed keys/IDs change the content fingerprint. `SUPPORTED_PROTOCOL_VERSION` 55 → 56 (`docs/08`).
- **One-time cutovers (no aliases):** profile DB schema 4 → 5 rewrites `published_maps.game_type_id` (`wipeout-*`/`hot-zone-*`/`heist-*` → new). Balance Lab snapshot 22 → 23, persistence 16 → 17 (load migration moves persisted `heist` tuning to `vaultBreak`), editor manifest 15 → 16 (paths/labels changed). JSON field is `vaultBreak` (serde camelCase) in Rust and the web app.
- `config/server/game-types.ron`: `heist-3v3` name became `Vault Break 3v3` rather than the stuttering `Powderline Vault Vault Break 3v3`.
- `GameMode::parse` keeps its existing spelling variants for the renamed key (`turf-hold|turf_hold|turfhold`) — same parser shape, not an alias for old names.
- Owner docs updated: `docs/02` (profile schema 5), `docs/04` (Last Stand; collect-and-hold candidate replaces the commercial name), `docs/08` (protocol 56), `docs/15` (schema versions), `docs/18` renamed to `18-damageable-world-objects-and-vault-break.md` with inbound links, `docs/backlog.md`, README, AGENTS.md.
- PNG mode icons and map overviews contain no text, so they were renamed, not regenerated.

# Verification (2026-11-21, agent)

- `just lint` — pass (fmt, Clippy all roles, feature isolation, layout checks, e2e config check).
- `just test` — pass (all suites). Updated golden vectors that hash renamed strings: `lobby::tests::canonical_catalog_matches_committed_golden_vector` (167 bytes, new revision), `server::lobby::catalog` golden advertisement revision, `map::compatibility_tests` catalog digest and per-preset `Debug` digests. Recipe fingerprints and serialized map snapshots are unchanged, which confirms no map/rule content moved.
- New tests: `profiles::tests::schema_4_published_maps_move_to_the_renamed_game_type_ids`, `balance_lab::persistence::tests::version_sixteen_heist_tuning_moves_to_vault_break`.
- One `just test` run hit the 2 s routed-process timing flake in `packages/brawler-routing/tests/process.rs` (untouched); it passed alone and in the next full run.
- `just practice-e2e` — pass (`Practice takedown-1v1 reached Active with one human`).
- Shell captures (attached): `scripts/shell-capture.sh game-select|dashboard 1280x720`, and `BRAWLER_SHELL_CAPTURE_GAME_TYPE=vault-break-3v3 … results 1280x720`. Labels fit at the longest name (VAULT BREAK).
- Not run: full `just ci` e2e matrix (rejoin, network-change, published-map); practice-e2e covered the routed admission path with the renamed game types.
- `rg -i 'wipeout|hot.?zone|heist|showdown'` outside the excluded/kept paths returns only `docs/06-research-sources.md`, the protocol-56 changelog in `docs/08`, and the two cutover migrations with their tests.

# Playtest (owner)

`just server`, then `just client`. Check the dashboard mode card, Choose Game (four cards: TAKEDOWN, TURF HOLD, VAULT BREAK, ZOMBIE), the in-match HUD plate in each mode, and Results. An existing local `data/server` profile DB migrates to schema 5 on first start; persisted Balance Lab tuning migrates on load.

# Learnings

- A blind rename of a word that is both a key and prose ("heist") needs context rules: identifier vs string vs comment, plus JSON casing. Tests caught two misses: the Balance Lab JSON path (serde camelCase `vaultBreak`, not the kebab mode key `vault-break`) and an evidence JSON key that broke sibling snake_case. Review also caught a stuttering display name ("Powderline Vault Vault Break 3v3").
- Prevention: after a scripted rename, list every distinct new string literal in the diff and review each one, and read every player-visible catalog name.

# Follow-up from parent review (2026-11-21)

- **Map Maker draft cutover (was missing):** local drafts are RON and store `MapModeAnchorKind` variants by name, so schema-1 drafts with `HotZoneCircle`/`HeistSafe` failed to parse and old `intended_game_type_id`s (`wipeout-1v1`, …) matched no game type. `MAP_DRAFT_SCHEMA_VERSION` 1 → 2. `load_one` renames the two variant identifiers outside RON string literals (an author's map name is never rewritten), parses, and for schema 1 rewrites the `wipeout-`/`hot-zone-`/`heist-` game-type prefixes and bumps to 2; `load_map_drafts` saves the migrated draft back. Test: `map_maker::persistence::tests::schema_1_draft_migrates_to_renamed_modes_and_is_saved_back` (v1 fixture with both anchor kinds, `heist-2v2`, and a "HeistSafe Yard" name). Documented in `docs/08` and `docs/13`.
- `GameMode::parse` now accepts `vault_break`/`vaultbreak` like `turf_hold`/`turfhold`.
- **Verification:** `just ci` — pass (lint; all tests; routed e2e 2/4/6 clients; Practice e2e for all ten game types: takedown-1v1/2v2/3v3/3v3 map 14, turf-hold-1v1/2v2/3v3, vault-break-1v1/2v2/3v3). Also passed: `just published-map-e2e`, `just rejoin-e2e`, `just completed-rejoin-e2e`, `just network-change-e2e`, `just zombie-e2e`.
- Learning: the cutover inventory covered server-side stores (profile DB, Balance Lab) but missed client-local saved data. Prevention: when renaming serialized enum variants or IDs, list every persistence site on both roles (`rg 'ron::from_str|serde_json::from|AtomicWriteFile'`) before declaring the cutover complete.
