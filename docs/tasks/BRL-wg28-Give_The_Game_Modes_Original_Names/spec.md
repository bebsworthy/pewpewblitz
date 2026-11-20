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
- Internal identifiers: pending q-vmzjrnz.
