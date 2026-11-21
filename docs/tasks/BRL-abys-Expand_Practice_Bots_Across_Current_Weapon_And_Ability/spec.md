# Outcome

Start Practice fills the bot seats with a deterministic, varied set of roster characters. Over a bounded set of consecutive starts, every one of the 9 launch characters, all 7 weapon bases, and all 11 ultimates appear. Each bot plays its resolved weapon delivery and ultimate type coherently, only through the ordinary character resolver and the authoritative `FighterInput` path.

# Scope (rewritten on pickup, 2026-11-21)

- **Roster variety.** `CharacterCatalog::practice_bot_choice(roster_seed, ordinal)` replaces the catalog-order pick. Character = `(seed + ordinal) mod 9`. Ultimate option = `(seed / 9 + ordinal) mod 2`. Passive option = `(seed / 18 + ordinal) mod 2`. Bots in one roster (at most 5) are always distinct characters. Any 18 consecutive seeds in a 1v1 cover every character × ultimate option pair, and therefore every weapon base and every ultimate.
- **Seed source.** The lobby's monotonically increasing Practice `request_id`. It is deterministic per supervisor process, needs no hidden player information, and never reads the human's pick. Mode and team composition change the roster only through the seat count. They are not mixed into the seed: the request counter already varies each start, and a mode mix would add coupling without adding coverage.
- **Capability execution from the resolved loadout**, never from character or preset IDs:
  - *Lobbed / Splash deliveries* fire over cover (the authoritative landing point already arcs over walls). They lead the target by the resolved, distance-scaled lob flight time, and send the intercept distance as the aim distance.
  - *Empty weapon (magazine refill or melee charges spent)*: Pressure backs off to preferred range instead of closing, so melee bots approach, strike, and fall back readably.
  - *Spread, cone and melee* keep the existing range control, derived from the resolved range/reach.
  - *Sentry* deploys facing a visible enemy inside the resolved acquisition range, not as a gap-closer.
  - *Reveal Scan* targets a remembered, currently unseen contact inside its resolved maximum range, using only the bot's own delayed contact memory.
  - *Concealment Field* is centered on the bot (aim distance 0) while it is engaged within weapon range or retreating. It suppresses primary fire on that tick so the lob distance is not zeroed.
  - *Self Cloak and Dash* keep the engage/escape semantics of `intent.dash`.
  - *Targeted fields, Demolition and Big Blob* keep the existing in-range targeting; Restoration targets an injured ally.
- Delayed perception, concealment fairness, bounded navigation, deterministic traces, objective priorities and `decoded_input_is_valid` neutral fallback are unchanged.

# Boundaries

- No difficulty selector, matchmaking substitution, learned policy, bot-only authority, bot protocol, or external bot process.
- No perfect projectile knowledge, hidden-state counter selection, or reading the human's pick.
- Future content tickets (for example BRL-k66v elemental weapon effects) own bot support for their new content.

# Acceptance criteria

- A pure coverage test proves that the bounded deterministic scenario set (18 consecutive seeds × 1v1/2v2/3v3) includes every weapon base and every ultimate. It also proves that rosters never repeat a character.
- Policy tests cover lob-over-cover fire, empty-weapon spacing, Sentry deployment range, Reveal Scan on a remembered contact, and the Concealment Field self-placement.
- `just lint`, `just test` (network lifecycle/restart, performance ceilings) and `just practice-e2e` pass.
- `docs/10-bots.md` names the supported content matrix and defers future content.
- A native matchup playtest remains for the owner; see Playtest.


# Implementation (2026-11-21)

- `src/characters/resolution.rs`: `practice_bot_choice(roster_seed, ordinal)`. `src/server/lobby/mod.rs`: `practice_bot_rows` takes the Practice `request_id` as the seed. The counter is shared with matchmaking allocations, so Practice seeds are varied but not always consecutive.
- `src/bots/controller.rs`: the observation now carries the resolved lob flight (`lob_flight_ticks_at_range: Option<u64>`; `None` = direct delivery). `ultimate_range` covers Sentry acquisition, Reveal Scan and Concealment Field ranges.
- `src/bots/policy.rs`: `plan_ultimate` dispatches by `UltimateKind`, and `flight_seconds` computes the lob lead. Lobbed deliveries skip the line-of-fire check and send the intercept distance. An ultimate that owns the aim suppresses that tick's primary fire.
- `src/bots/behaviors.rs`: Pressure gives ground with an empty weapon and makes no gap-closing Dash.
- `docs/10-bots.md`: new "Supported content matrix" section; roster pick and capability gates updated.

# Verification

- `just lint`: pass.
- `just test`: pass (24 suites, including network lifecycle/restart and performance gates).
- `just practice-e2e`: pass (Practice wipeout-1v1 reached Active).
- New tests: `practice_bot_rosters_cover_every_character_weapon_and_ultimate_without_repeats` (18 seeds × 1/3/5 bot seats), `lobbed_weapons_fire_over_cover_and_lead_by_the_scaled_lob_flight`, `empty_weapon_gives_ground_instead_of_closing_or_dashing`, `sentry_deploys_only_toward_an_enemy_inside_acquisition_range`, `reveal_scan_targets_only_a_remembered_unseen_contact_in_range`, `concealment_field_covers_the_engaged_bot_without_spending_a_shot`. The lobby practice-request test was updated to the seeded roster.
- Not run by the agent: native matchup playtest (below).

# Playtest (owner)

- Run: `just server`, then `just client`. Start Practice several times in a row in Wipeout 1v1, then in Hot Zone 3v3 and Heist 3v3.
- Observe:
  - The bot character and ultimate change between starts.
  - Torque (Arc Launcher) and Pollen (Splash) lob over walls.
  - Cobble and Bumper (Impact Blade) close in, strike, and back off once their charges are spent.
  - Orbit's Sentry is deployed facing you when you are in range.
  - Orbit and Prism's Reveal Scan lands where you broke line of sight.
  - Sumi's Concealment Field drops on the bot during close fights.
- Known limits: the content matrix stops at the pre-elemental catalog. There is no projectile dodging. Self Cloak still shares Dash's engage/escape trigger.

# Learnings

- Clippy's `struct_excessive_bools` caught a fourth bool on `BotObservation`. An `Option<u64>` carried the lob flight more precisely anyway. Prefer an option-carrying capability over a flag plus a value.
