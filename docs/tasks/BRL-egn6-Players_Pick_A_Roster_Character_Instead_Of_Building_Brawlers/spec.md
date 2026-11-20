# Outcome

The Brawlers flow becomes a character select. A player picks a roster character and that character's ultimate and passive options; the server resolves that pick into the existing `ResolvedMatchLoadout`; and every client renders the correct character (production model, or a tinted placeholder) in gameplay and on the Dashboard. Direction and roster: BRL-7sqn (agreed 2026-11-19).

# Scope

- **Character catalog** (headless gameplay content; register fingerprint material through its content plugin): stable `CharacterId`, key, display name, role and body labels, per-character fighter stats (seeded from today's default/lightweight/reinforced profiles), signature weapon recipe (a weapon base plus optional authored parts), and exactly 2 ultimate options and 2 passive options. The 9 launch characters and their kits are in the BRL-7sqn spec table. Bumper's kit follows the answer to BRL-7sqn q-qxsh7p7, or that question's default.
- **Resolution:** character plus chosen options → the existing validator → `ResolvedMatchLoadout`. Combat, bots and replicated loadout consumers do not change.
- **Profile and protocol:** the profile stores the selected `(CharacterId, ultimate choice, passive choice)`, remembered per character. Remove `SavedBrawler`, the 16-brawler arsenal, the Create/Edit/EquipWeaponParts/Delete commands, and the player part inventory along with its starter seeding. The advertised catalog becomes a character catalog. Use a fresh SQLite schema with no migration (local data is dev/test only). Follow the one global compatibility handshake in `docs/08`.
- **Identity on the wire:** the replicated fighter identity carries the stable `CharacterId`, never asset paths. The client maps the ID to a presentation entry: model (Riff for Light, Bumper for Normal/Heavy, until an art ticket ships the real model), per-character tint, scale and socket. An unknown ID falls back to Riff.
- **Client:** replace the Brawlers screen (`flow/screens/brawlers.rs`) with a roster grid plus an ultimate/passive option picker, keeping touch and keyboard focus parity. The Dashboard preview shows the selected character.
- **Practice bots:** pick characters and options deterministically (the full matchup work stays in BRL-abys).
- **Diagnostics:** fallback diagnostic loadouts in `content/catalogs/builds.ron` become character picks.
- **Docs:** rewrite `docs/00` to the roster pitch (composition stays the authoring model, pillar 5); update `docs/02` and `docs/03` (remove the player arsenal and part inventory; add the character catalog and selection lifecycle); update `docs/13` for player flow; retire the "weapons are interchangeable" rule in `asset_src/01-characters.md`.

# Boundaries

- No unlocks, progression or acquisition: every character is available.
- Duplicate characters are allowed on both teams and within a team.
- Weapon bases, parts, ultimates and passives stay as authoring primitives: no combat rewrite.
- Balance Lab character editing is BRL follow-up work (the "Tune roster characters in Balance Lab" ticket). This ticket only keeps Balance Lab compiling and showing the Practice roster.
- Real character models are separate art tickets.

# Acceptance

- All 9 characters are selectable, with their 2×2 options, on desktop and touch. The pick persists across reconnects.
- A match spawns the resolved kit. Remote fighters render the owner's character and tint.
- No asset paths or process-local IDs go on the wire. An invalid character or option is rejected by the server.
- `just lint`, `just test`, `just practice-e2e` and `just rejoin-e2e` pass.
- Owner visual check of the select screen and in-match placeholders. Pixel check of asset memory.

# Bumper kit (decided 2026-11-19, BRL-7sqn q-qxsh7p7)

Bumper: Impact Blade · Dash or Restoration Field · Tenacity or Adrenal Response.

# Design (2026-11-20, implementation agent)

Research: `src/builds/definitions.rs` (saved-brawler resolver takes `[PassiveDefinitionId; 2]`, rejects the two frame passives, and reads one of three `fighter_profiles`), `src/profiles/*` (SQLite schema 3: `brawlers`, `profile_selection`, `weapon_part_instances`, `brawler_part_slots`, plus `published_maps`), `src/protocol.rs` (protocol 53), `src/server/lobby/*`, `src/server/balance_lab/*`, client `flow/screens/brawlers*`, `flow/screens/equipment.rs`, `dashboard/*`, `presentation_3d/fighter_character.rs` (only Riff is loaded; `chr_bumper.glb` ships on Riff's clip/socket contract but nothing loads it).

## Findings that shape the design

- **One passive.** The roster gives 1 passive from 2 options, but `ResolvedMatchLoadout.passives` is `[ResolvedPassive; 2]` and the resolver requires two distinct passives. Combat reads passives only through `ResolvedPassives::find` and one loop, so the slot count becomes a code constant `PASSIVE_SLOT_COUNT = 1` (`[ResolvedPassive; PASSIVE_SLOT_COUNT]`); combat code is unchanged.
- **Frame passives.** Cobble, Plunk, Pollen and Prism offer Lightweight Frame / Reinforced Frame. Today those passives have no runtime effect: the legacy resolver used them only to pick the lightweight/reinforced stat profile, and saved brawlers could not select them. With per-character stats they get bounded parameters that scale the character's stats: `LightweightFrame { maximum_health_basis_points: 8500, movement_speed_basis_points: 11000 }`, `ReinforcedFrame { 12000, 9000 }` (the original 85/110 and 120/90 ratios to the 100/100 default). See question.
- **Stale stat seeds.** `lightweight` (85 hp, 110 speed) and `reinforced` (120 hp, 90 speed) were never rescaled when `default` became 1000 hp / 70 speed (V12 closeout, commit 3eadd61b); seeding Light characters with 85 hp would be broken. Seeds use the original ratios per body: Light 850 hp / 77 speed / reveal 192, Normal 1000 / 70 / 160, Heavy 1200 / 63 / 128; recovery 100, idle delay 180, cold capacity 1000, resistances 0 for all. See question.

## Catalog: `content/catalogs/characters.ron` (headless, in the gameplay fingerprint)

New domain module `src/characters/` (definitions, validation, resolution, `CharacterContentPlugin` registering canonical fingerprint material).

```text
CharacterCatalog { schema_version: 1, characters: [CharacterDefinition] }   // ≤ 32, ascending ids
CharacterDefinition {
  id: CharacterId(u16), key, display_name, role: Damage|Tank|Helper|Controller, body: Light|Normal|Heavy,
  stats: ResolvedFighterStats,
  weapon: (base: WeaponPresetId, parts: [WeaponPartDefinitionId] /* ≤ 4, authored */),
  ultimate_options: [UltimateDefinitionId; 2],   // distinct
  passive_options: [PassiveDefinitionId; 2],     // distinct
}
```

Validation (at plugin build and in tests): unique ascending IDs and keys, bounded names, stats in the existing fighter bounds, weapon base and parts exist and resolve, options exist and differ. The 9 launch characters follow the BRL-7sqn table with the decided Bumper kit, all with no authored parts at launch.

**CharacterId stability:** IDs are permanent wire and storage identities, never reused, independent of display order: 1 Riff, 2 Torque, 3 Cobble, 4 Plunk, 5 Pollen, 6 Orbit, 7 Sumi, 8 Prism, 9 Bumper. Display order is catalog order. The default pick (new profile, or a stored pick the catalog no longer accepts) is the first character with its first options.

## Resolution

`builds::resolve_loadout(builds, weapons, LoadoutRecipe { character_id, fighter_stats, weapon_base, weapon_modifiers, ultimate, passive })` becomes the single authority path. `CharacterCatalog::resolve(choice, builds, weapons, parts)` validates the choice against the character's options, aggregates the authored parts into `CanonicalWeaponModifiers`, applies a frame passive to the stats, and calls it. `ResolvedMatchLoadout` gains `character_id`; `SelectedBuild` hashes the recipe. Removed because nothing reaches them once characters own stats: `BuildCatalog.fighter_profiles`, the legacy budget resolver `resolve_build_recipe` with its frame-passive stat inference, `BrawlerBuildRecipe`, `build_point_total`, `BuildResolutionError::OverBudget`, `CustomPulse` weapon choices if they become unreachable. Point-cost fields, `total_points` and the remaining budget vocabulary stay for BRL-01c8 (noted there).

## Profile schema (SQLite `user_version` 4, fresh)

- `profiles(account_id, revision, selected_character_id NULL)` and `character_choices(account_id, character_id, ultimate_id, passive_id, PK(account_id, character_id))`. Retired: `brawlers`, `profile_selection`, `weapon_part_instances`, `brawler_part_slots`, the starter part seeding, `next_brawler_ordinal`.
- Opening a schema-3 database drops the four retired tables and the two retired `profiles` columns and creates the new tables; account rows and `published_maps` are kept (no player data is migrated). Schemas 1–2 predate published maps and fail closed.
- `ProfileSnapshot { account_id, revision, selection: CharacterChoice, remembered: Vec<CharacterChoice> }` where `CharacterChoice { character_id, ultimate_id, passive_id }`. The authority normalizes stored rows against the catalog on load (drops unknown characters or options; falls back to the default pick), so every snapshot on the wire is valid. A pick is remembered per character.

## Protocol 54 (one global handshake, no per-message versions)

- `ProfileCommand` becomes `SelectCharacter { request_id, expected_profile_revision, choice }`; Create/Edit/Select/Delete/EquipWeaponParts are removed. `ProfileDecision` loses `MissingBrawler`, `CapacityReached`, `MissingPart`, `PartAlreadyEquipped`, `IncompatibleWeapon`, `IncompatibleBuild`; an unknown character or option is `InvalidRequest`.
- `LobbyJoinOutcome::Accepted.brawler_catalog` becomes `character_catalog: AdvertisedCharacterCatalog` (characters with stats, weapon base and option IDs, plus weapon/ultimate/passive display metadata; format version 6).
- `QueueJoinCommand`, `PracticeStartRequest`, `QueueMembership`: `brawler_id`/`brawler_revision` become `profile_revision`. `AcceptedBuildSummary` becomes `{ character: CharacterChoice, identity: SelectedBuild }`.
- `MatchBuildSnapshot` (routing) schema 8 carries `CharacterChoice` + accepted identity; admission and Balance Lab re-resolve it against the catalogs.
- Replicated identity: `ResolvedMatchLoadout.character_id` (stable `CharacterId`, already replicated with the loadout). No asset paths or entities on the wire.

## Client

- One overlay `ClientOverlay::Brawlers` replaces BrawlerList, BrawlerDetails, BrawlerCreation, BrawlerEditor, WeaponEquipment and DeleteBrawlerConfirmation; the Equipment screen and part UI are deleted.
- Screen (`flow/screens/brawlers.rs`, same D-direction shell, `BACK` + `BRAWLERS`): a 3×3 roster grid (name, role · weapon); a selected-character panel with a 3D preview, name, role/body and weapon; an ULTIMATE row and a PASSIVE row of two option buttons each. Pressing a tile selects that character immediately with its remembered (or default) options; pressing an option selects it for the current character. Each press is one `SelectCharacter` round trip; the pending state disables controls. Wide (1280×720) puts the preview left, grid centre, options right; compact (914×411) drops the preview and puts grid left, options right. Every control is a `FlowButton` in the shell focus order (keyboard/gamepad parity) and at least 48 dp.
- Dashboard: preview shows the selected character (model and tint); the summary reads `Role · Body` and `Weapon · Ultimate · Passive`; `VIEW BRAWLERS` opens the select screen.
- Shell-capture routes: `brawler-list`, `brawler-details`, `brawler-create`, `brawler-editor`, `ultimate-picker`, `weapon-equipment`, `weapon-parts`, `delete-brawler`, `first-brawler` are replaced by `brawlers` (default pick) and `brawlers-picked` (a non-default character and options selected).
- Automation: `--weapon-preset` becomes `--character <ID>`; the e2e diagnostic rotation reads character IDs.

## Presentation mapping (client-only, outside the fingerprint)

`assets/catalogs/fighter_characters.ron`: `models` (key → GLB path and source ground), `characters` (CharacterId → model key, tint RGB, scale multiplier, weapon socket), with Riff as the fallback for unknown IDs. Riff and Bumper are untinted; Light placeholders use Riff, Normal/Heavy placeholders Bumper, each tinted. The client loads Bumper's GLB alongside Riff's. An art ticket changes only its character's entry (and adds a model entry). Tint is applied at toon-material conversion (cached per source material and tint).

## Practice bots and diagnostics

- Bots pick deterministically: bot ordinal `n` takes catalog character `n mod 9` with its first ultimate and passive. Behavior and matchup variety stay in BRL-abys.
- `builds.ron` `direct_diagnostic` becomes an ordered list of character picks (Riff, Plunk, Torque, Cobble: the old Pulse/Scatter/Arc/Blade rotation).

## Balance Lab

Keeps compiling and shows the Practice roster as characters with their picks. The Fighters section (three stat profiles) is removed because the profiles no longer exist; BRL-5v3c adds per-character editing. See question.

## Commit order

1. Character catalog and content plugin (additive).
2. Character resolution and single passive slot; profile, storage, protocol 54, lobby, queue, admission, bots, diagnostics, Balance Lab and scripts; client flow, Brawlers screen and Dashboard.
3. Client presentation mapping: Bumper loading, per-character model and tint.
4. Docs (00, 02, 03, 13, asset_src/01-characters.md) and verification record.

# Implementation record (2026-11-20, implementation agent)

Commits on the worktree branch `worktree-agent-adeac8782d209c288`:

1. `5237ec52` Add the roster character catalog and its content fingerprint.
2. `83f02c58` Pick a roster character instead of saved brawlers and parts (resolution, profile schema 4, protocol 54, lobby/queue/admission, bots, diagnostics, Balance Lab, scripts, client Brawlers screen and Dashboard).
3. `a8a68c10` Draw each fighter as its roster character (client presentation catalog, Bumper loading, per-character model/tint/scale/socket, Dashboard preview follows the selection).
4. `50ffb7eb` Document the roster character model and selection lifecycle (docs 00, 02, 03, 05, 08, 10, 11, 13, 14, 15, README, backlog, asset_src/01-characters.md).

## Decisions made during implementation

- **Identity excludes tuned values.** `SelectedBuild` hashes format, recipe schema, balance revision, character, weapon base and modifiers, ultimate and passive, not stats or weapon recipe values. Including stats broke the bot test that mutates live stats, and tuning is already covered by the gameplay content fingerprint. Consequence: a Balance Lab weapon edit keeps the pick identity and re-resolves the weapon recipe (test `revised_weapon_tuning_keeps_the_pick_identity_and_re_resolves_the_weapon`).
- **Arena fighters wait for the replicated loadout** before spawning a model (no Riff-then-swap flash). An unknown ID draws Riff untinted.
- **Tint** is applied when the scene converts to the toon material, cached per (source material, tint bits).
- **Balance Lab**: the Fighters section and `fighterProfiles` are removed (persistence 15 / snapshot 21 / editor 14; a 14/20 file drops stored profiles and takes canonical frame parameters). The editor gains the four frame-passive scale fields (193 fields total, 16 passive leaves). Per-character tuning stays BRL-5v3c (q-52ghnnx default).
- **Network tests** that measured the old lightweight profile (85 hp, 110 speed, 10 hp/s recovery) now install that exact fixture through `install_compact_test_loadout`, built with `LoadoutRecipe::test_default` + `resolve_loadout`, instead of depending on a roster character's tuning.

## BRL-01c8 boundary

Removed here because character resolution made them unreachable: `resolve_build_recipe` and its frame-passive stat inference, `BrawlerBuildRecipe`, `WeaponChoice`/custom Pulse specification, `build_point_total`, `BuildResolutionError::OverBudget`, `MAX_BUILD_CANDIDATE_BYTES`, `BuildCatalog.fighter_profiles`. Left for BRL-01c8: `BUILD_POINT_BUDGET`, every `point_cost` field, `ResolvedMatchLoadout.total_points` (always 0 now), and `QueueRejection::OverBudget` (protocol variant, unreachable).

## Verification (2026-11-20)

- `just lint` (ripgrep on PATH): pass.
- `just test`: pass (routing 112; client 916 + 257; server 543; balance-lab 592 + revised-catalog network test; network 136; performance 12).
- `BRAWLER_ROUTED_BIND=127.0.0.1:5165 BRAWLER_SERVER_DATA_DIR=$PWD/target/egn6/server-practice just practice-e2e`: pass (Practice wipeout-1v1 reached Active).
- `just rejoin-e2e` on 5166: pass (rejoined the same running match after an outage).
- `just published-map-e2e` on 5167: pass.
- New unit/App tests: character catalog validation and fingerprint; resolution of every option combination; profile normalization; schema-3 to 4 upgrade; protocol 54 picks; presentation catalog (every character has a look on a shipped GLB, placeholders follow body, unknown falls back to Riff); arena fighter waits for its loadout and wears model/tint/scale; tinted toon conversion is cached per tint; the catalog loads exactly the contract-checked GLBs.
- Shell captures (`BRAWLER_SHELL_CAPTURE_BIND=127.0.0.1:5161/5162 just shell-capture all <size>`), every route captured at 1280x720 and 914x411, copies in the worktree: `target/egn6-capture-1280x720/`, `target/egn6-capture-914x411/`. Viewed: `brawlers`, `brawlers-picked`, `dashboard`, `dashboard-picked` at both sizes. The `all` run stops at `results-published`, which needs the 1v1 catalog per the script header; `BRAWLER_SHELL_CAPTURE_GAME_TYPE=wipeout-1v1 ... just shell-capture results-published all` passes (`target/egn6-capture-published/`).
- In-match placeholder: `BRAWLER_RENDER_BIND=127.0.0.1:5164 BRAWLER_RENDER_CHARACTER=4 scripts/v3-render-evidence.sh target/egn6-render-plunk.txt` shows Plunk (1200 health) as a mint-tinted Bumper (`target/egn6-render-plunk.txt.captures/`).

## Known limits and owner checks

- Owner visual check: Brawlers screen at both sizes; tinted placeholders in a match (Plunk read as a mint Bumper; tints may want tuning in `assets/catalogs/fighter_characters.ron`); remote fighters render the owner's character.
- On the Brawlers screen, Bumper's Impact Blade reaches past the preview viewport's right edge at 1280x720 and is clipped (Dashboard is fine). Small framing polish; not fixed in this slice.
- 914x411 keeps the preview (that size is the Wide layout at the small-window fit), so the "compact drops the preview" path applies only below the Wide threshold.
- Pixel memory check: the client now loads Bumper's GLB (about 0.7 MB budget) alongside Riff at startup; check asset memory on the Pixel 8a.
- Practice bots now play every roster weapon with their existing Pulse/Dash-oriented behavior; matchup quality is BRL-abys.

## Playtest

Run `just server` and `just client`. Dashboard → `VIEW BRAWLERS`: arrows or touch to move across the 3×3 grid and option rows, Enter/tap to pick; the pick persists after reconnect. Start Practice and look at your fighter and the bots (Riff, Torque, Cobble…): each should draw its character's model and tint and the HUD should show its ultimate.

## Merge verification (main, 2026-11-20)

Cherry-picked the four commits onto main (79a62df0 and parents). On the combined tree: `just lint`, `just test`, `just practice-e2e`, `just rejoin-e2e`, `just published-map-e2e` all pass. Reviewed captures: Brawlers (default, picked) at 1280x720 and 914x411, Dashboard picked at 914x411, in-match Plunk render (tinted Bumper placeholder, 1200 health).

Found: at 1280x720 Bumper's blade is cut at the right edge of the Brawlers preview; filed BRL-qfv2. Open questions q-65ch0k0 and q-52ghnnx were implemented with their stated defaults and are relayed to the owner.

Not verified: owner visual check of the select screen and in-match placeholders; remote fighters rendering their owner's character in a live two-client match (covered only by an automated test); Pixel asset memory (Bumper now loads with Riff). Protocol is 54: the port-5000 server and the phone APK need a rebuild.
