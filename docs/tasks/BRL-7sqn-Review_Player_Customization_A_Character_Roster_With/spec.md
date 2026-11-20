# Origin

Owner, answering BRL-tkmh q-m6n2sj1 on 2026-11-19: "Currently there are a lot of option for the player to customize a character. It's nice because it allows customization but i think it's overkill to expose it to the user. I think it would make more sense for the user to have a roaster of characters to choose from, each with their own weapon style and character stats." BRL-tkmh is parked for this.

# Context

- Today: saved brawlers combine a weapon base and parts, passives, an ultimate and a stat profile (docs/02, docs/03); the Brawlers screen edits them; bots, Balance Lab and diagnostics use the same model.
- Characters: Riff and Bumper (BRL-qfaj) exist on one shared clip contract; `asset_src/01-characters.md` sketches a roster with roles (Damage/Tank/Helper/Controller) and bodies (Light/Normal/Heavy), written assuming weapons stay interchangeable.

# Status

Idea for owner reflection and design; no implementation until the owner sets direction.

# Agreed direction (owner, 2026-11-19)

Players pick from a **roster of characters**. Each character has authored stats, a signature weapon and a small per-character choice. The build system stays, but only as the authoring vocabulary.

## Decisions

1. **Player shape: a character plus a small choice (shape B) from the start.** Each character fixes its model, stats and signature weapon. The player picks **1 ultimate from 2 options and 1 passive from 2 options** for that character.
2. **Build engine stays as authoring.** Characters are authored recipes over today's primitives: weapon base (plus optional authored parts), ultimates, passives and stats. Balance Lab, bots and tests keep composing. Combat keeps consuming `ResolvedMatchLoadout` and does not change.
3. **Saved brawlers and the player part inventory are retired.** The profile stores the selected character and that character's chosen ultimate and passive. There are no 16 brawler slots, no part inventory and no part equipping. Local SQLite data is dev/test data, so there is no migration; start fresh.
4. **Weapons can repeat across characters.** The launch set still covers every weapon base.
5. **Stats are per character.** Each character owns its health, speed and resistances in the character catalog. Initial values come from today's default, lightweight and reinforced profiles, and are tuned in Balance Lab.
6. **Placeholders reuse existing models, recolored.** Light characters use Riff and Normal/Heavy characters use Bumper, each with a per-character tint, until the character's own model exists (one art ticket per character).
7. **Duplicates are allowed:** the same character may appear on both teams or twice on one team.
8. **Rewrite the product direction.** `docs/00` changes to a roster-of-characters pitch. Composition (pillar 5) remains the authoring model. `docs/02` and `docs/03` drop the player-facing arsenal and part inventory. The "weapons are interchangeable" rule in `asset_src/01-characters.md` is retired, and each character has a signature weapon.

## Launch roster (agreed starting point; numbers are tuned per character ticket and in Balance Lab)

Source: `asset_src/04-character_roaster.png` (8 characters), plus Bumper as a 9th (owner decision).

| Character | Role / body | Weapon | Ultimate options | Passive options | Model |
|---|---|---|---|---|---|
| Riff | Damage / Light | Pulse Sidearm | Dash · Self Cloak | Adrenal Response · Quick Cycle | production |
| Torque | Damage / Heavy | Arc Launcher | Big Blob · Fire Field | Heat Shielding · Quick Cycle | placeholder (Bumper, tinted) |
| Cobble | Tank / Heavy | Impact Blade | Dash · Demolition Strike | Reinforced Frame · Tenacity | placeholder (Bumper, tinted) |
| Plunk | Tank / Heavy | Scatter Cannon | Dash · Restoration Field | Close Quarters · Reinforced Frame | placeholder (Bumper, tinted) |
| Pollen | Helper / Light | Splash | Restoration Field · Poison Field | Filtered Circulation · Lightweight Frame | placeholder (Riff, tinted) |
| Orbit | Helper / Light | Pulse Sidearm | Sentry · Reveal Scan | Quick Cycle · Tenacity | placeholder (Riff, tinted) |
| Sumi | Controller / Normal | Spray | Concealment Field · Cryogenic Field | Close Quarters · Cryogenic Insulation | placeholder (Bumper, tinted) |
| Prism | Controller / Light | Sticky Blomb | Cryogenic Field · Reveal Scan | Lightweight Frame · Cryogenic Insulation | placeholder (Riff, tinted) |
| Bumper | Tank / Normal | provisional: Impact Blade | provisional: Dash · Restoration Field | provisional: Tenacity · Adrenal Response | production |

The roster covers all 7 weapon bases, all 11 ultimates and all 9 passives. Bumper's kit is provisional; see the open question.

## Impact on the current model (for implementation planning)

- **Persistence and protocol:** `SavedBrawler`, `ProfileCommand` (Create, Edit, EquipWeaponParts, Delete) and the part inventory are replaced by a profile selection of `(CharacterId, ultimate choice, passive choice)`. `AdvertisedBrawlerCatalog` becomes a character catalog. The replicated fighter identity gains a stable `CharacterId` so remote clients render the right model and tint; asset paths never go on the wire.
- **Server resolution:** the character recipe and the chosen options resolve through the existing validator into the same `ResolvedMatchLoadout`.
- **Client:** the Brawlers screen becomes a character select with an option picker. The Dashboard preview shows the selected character.
- **Bots:** they pick characters and options. Their behavior already reads only the resolved loadout.
- **Balance Lab:** it adds a character category (stats, weapon recipe, option lists) on top of the primitives.

## Status

Direction agreed. Next, the work is split into implementation tickets: character catalog and resolution, profile/protocol replacement, character-select UI, placeholder tinting, docs rewrite, and one art ticket per new character. None of these has been filed yet.

# Follow-up tickets (2026-11-19)

- BRL-egn6: players pick a roster character (catalog, resolution, profile/protocol, select screen, placeholders, docs).
- BRL-5v3c: tune roster characters in Balance Lab.
- Art: Torque BRL-nwhr · Cobble BRL-zkb3 · Plunk BRL-qqch · Pollen BRL-2x8j · Orbit BRL-m6an · Sumi BRL-pjdb · Prism BRL-xb9x.
- Reshaped: BRL-abys (bots pick characters; blocked by BRL-egn6), BRL-01c8 (scope note), BRL-ytx3 (characters as columns).
- Canceled: BRL-tkmh. Back to idea: BRL-w07m, BRL-7r6j.
- Open: q-qxsh7p7, Bumper's kit (non-blocking; BRL-egn6 uses the default if it is unanswered).

# Bumper kit decided (owner, 2026-11-19, q-qxsh7p7)

Bumper (Tank / Normal): **Impact Blade** · ultimate Dash or Restoration Field · passive Tenacity or Adrenal Response. It is the charging melee tank, the faster Normal-body counterpart to Cobble. This replaces the "provisional" marks in the roster table.
