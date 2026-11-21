# Outcome

Balance Lab gains a Characters category over the character catalog from BRL-egn6. It edits per-character stats, the signature weapon base and authored parts, and the 2 ultimate options and 2 passive options. It reuses the existing schema-driven validation and applied/default state, and the primitives (weapons, ultimates, passives, parts) stay editable as they are today.

# Notes

- Blocked by BRL-egn6.
- Characters are the natural columns for BRL-ytx3's comparison matrices. Coordinate with that ticket rather than building a second layout.
- Verify with `just lint`, `just test` and the Balance Lab web build. The owner then tunes the roster on a native client.

# Research (2026-11-21)

- BRL-egn6 is done. Balance Lab today treats the character and weapon-part catalogs as untuned
  (`UntunedRosterContent`) and only re-resolves admitted picks against tuned build and weapon
  catalogs. The removed Fighters section (dc799071) shows the numeric stat fields and bounds to
  restore per character.
- The editor manifest is numeric-only: every editable leaf is a number at a JSON path. Weapon base,
  parts, and option IDs serialize as plain numbers (`WeaponPresetId(3)` → `3`), so they fit the
  same draft/diff/reset pipeline if the manifest can declare a choice control.
- The lobby advertises the canonical catalog (Brawlers screen, Practice bot picks). Balance Lab
  tuning lives only in the Practice match worker, like weapon/ultimate tuning today.
- All nine characters author `parts: []`; `WEAPON_PART_SLOT_COUNT` is 4.
- BRL-ytx3 (backlog) describes the comparison matrix that already shipped in 7985de0d/b9496d6a.
  Characters reuse that matrix as columns; no second layout.

# Decisions

1. **Snapshot.** Schema 22 adds `characters: [CharacterTuning]` in catalog order: locked `id`,
   `key`, `displayName`, `role`, `body`; tunable `stats` (the nine `ResolvedFighterStats` leaves),
   `weaponBase`, `weaponParts: [u16; 4]` (0 = empty slot, so the topology stays fixed), and
   `ultimateOptions` / `passiveOptions` (`[u16; 2]`). Persistence 16 migrates 15 by filling
   canonical characters. Editor manifest 15 adds a `characters` section.
2. **Choice control.** A field descriptor may carry `control: "select"` plus `options:
   [{value, label}]`. Value storage stays an integer at the path, so draft, Edited/Default Δ
   markers, Copy differences, reset and server error association need no second pipeline.
3. **Validation.** Stats: `validate_fighter_stats` plus the restored manifest bounds. Weapon base,
   parts, ultimates and passives must name existing definitions; the two ultimate options differ
   and the two passive options differ (the catalog's existing rule — a duplicate makes the pick
   meaningless). Every character's four option combinations must resolve against the revised
   build/weapon catalogs. Duplicate parts are allowed if aggregation succeeds (no invariant needs
   them unique). Direct-diagnostic picks (`builds.ron`) are not revalidated against tuned options:
   they never run in a Practice worker.
4. **Admitted picks follow the option slot.** The human and bots were admitted with canonical
   options. On load and apply, an admitted ultimate or passive that the tuned character no longer
   offers maps to the tuned option in the same slot as in the canonical character. This keeps every
   admitted roster resolvable, so option edits never strand a join or reject apply. The Brawlers
   screen still shows canonical options — known limitation, same as all Lab tuning.
5. **Install.** The tuned character catalog joins builds/weapons/maps in the staged transaction and
   is installed into `CharacterCatalogResource` (bots, joins, test dummy read it). Only weapon parts
   stay untuned. The roster panel is re-projected after each apply/restore so it shows the tuned
   kit.

# Implementation notes

- Snapshot `characters` omits role and body entirely (they are descriptive labels; nothing to
  tune), so they stay locked by construction.
- Choice cells reuse the numeric pipeline: `control: "select"` with `options`; web
  `validateDisplayNumber` accepts only an offered value and `formatFieldValue` shows the option
  label in the detail strip and Copy differences.
- `CharacterCatalog::validate_option_combinations` was split from `validate_references`; Balance Lab
  uses it so direct-diagnostic picks (never run in Practice) don't constrain option tuning.
- Slot carry-over lives in `CharacterChoice::carried_by_slot` plus the balance-lab-gated
  `CharacterCatalog::carry_balance_lab_choice` (canonical catalog cached in a `OnceLock`), called
  from `MatchCharacterSnapshot::resolve_revised_balance_lab_catalogs`, so joins, bot install,
  apply, persisted-load and the roster panel all agree.
- A combination that fails to resolve (e.g. Healing Module on Pollen's Splash) rejects Apply with a
  toast naming the character, not a single field: the cause can be the base, a part, or an option.

# Playtest (owner, native client)

1. `just balance-lab`, enter Practice; the browser opens http://127.0.0.1:5123.
2. **Characters** tab: nine columns (Riff … Bumper); groups Core stats, Recovery, Concealment,
   Elemental baselines, Signature weapon, Options.
3. Raise your character's Maximum health and Movement speed, Apply & reset match: the HUD health
   and movement change after the clean restart.
4. Change your character's Weapon base (e.g. Riff → Arc Launcher) and set Authored part 1 to Heavy
   Payload; Apply: you fire the new base with the part's damage/interval change; the Players panel
   shows the new base and modifiers.
5. Replace the ultimate option you picked (slot 1 or 2) with another ultimate; Apply: you now have
   the new ultimate in that slot; the Players panel shows it. Same for a passive.
6. Set both ultimate options to the same value: Apply is rejected and the cell shows the error.
7. Restore canonical defaults: everything returns to authored values.
- Known limit: the Brawlers screen and lobby-picked bot options stay canonical; tuning applies
  only inside the Practice worker.

# Verification (2026-11-21)

- `just lint` — pass (fmt, Clippy for every role incl. balance-lab and network+balance-lab,
  feature isolation, layout checks, Balance Lab web `npm ci`/`test`/`build`).
- `just test` — pass: 24 suites, 2,616 tests, 0 failures, including `_test-balance-lab` and the
  network scenario `builds::revised_catalog_loadout_keeps_build_identity_and_replicates_authoritative_values`
  (revised character stats replicate authoritatively).
- New coverage: `balance_lab::characters` (round trip, install, path-qualified rejections, locked
  identity/topology/bounds), editor manifest Characters columns and select options, persistence
  15→16 migration and tuned round trip, fixed-tick apply carrying Riff's ultimate and Plunk's
  passive by slot with stats/roster-panel/`CharacterCatalogResource` updated,
  `CharacterChoice::carried_by_slot`, web select validation/label formatting/Copy differences.
- Not run: real-process e2e (`just practice-e2e` etc.) — they do not start Balance Lab, so they
  cannot exercise this change; the native browser workflow is the owner playtest above.

# Learnings

- Mistake: a fixture grew past Clippy's `too_many_lines` once character resources were added;
  caught by `just lint`, fixed by extracting `install_catalogs` rather than an allow.
- Making a previously "untuned" catalog tunable has a hidden coupling: picks admitted elsewhere
  (lobby) are validated against canonical content. Any tunable option list needs a defined
  carry-over rule (here: by slot) at the single re-resolution seam, or joins break.
