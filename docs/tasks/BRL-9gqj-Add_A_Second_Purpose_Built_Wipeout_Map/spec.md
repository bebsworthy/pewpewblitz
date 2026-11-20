# Outcome

Players queuing or practicing 3v3 Wipeout can get a second accepted layout whose combat rhythm and viable builds differ materially from **Verdant Crossfire** (`verdant-crossfire`, MapPresetId 10), today the only advertised 3v3 Wipeout map.

# Baseline to differ from

Verdant Crossfire is a padded 25×37 portrait sparse recipe on theme 3. It is built almost entirely from green symbol walls (52 cells) and tall grass (46 cells), plus 8 yellow striped cover cells, with three spawns per team at opposite short edges. Its decisions are lane-and-bush: long sightlines along the portrait axis, broken by wall blocks and concealment groups.

The new map must produce visibly different decisions. Examples: wider or landscape proportions, a contested centre with destructible cover, water or barrels that shape routes, shorter sightlines that favour close range, or fewer concealment groups. The final recipe rationale records which differences were chosen and why.

# Scope

- Author one **new** sparse-grid Wipeout recipe with stable identity (next free MapPresetId), three spawns per team, safe re-entry routes, readable bounds, and no mode anchors.
- Do not promote the unadvertised legacy Wipeout recipes (Ashen Court, Barrel Yard, Crossroads Facility, Tidal Garden). They are conversion and proof layouts kept for focused regressions (`docs/16-grid-map-asset-system.md`), at a different scale.
- Use established ground, walls, destructible cover, concealment, barrels, chests, pickups, water, and decoration assets only where they create deliberate decisions.
- Design for at least two viable approaches, meaningful close, mid, and long-range opportunities, limited dead space, and no indefinite disengagement route.
- Advertise it by adding its key to the existing `wipeout-3v3` game type's `maps` list in `config/server/game-types.ron` and bumping that entry's revision. Do not add a new game type: game types already hold several maps, and Practice lets players choose one (BRL-3c10). A new game type would split the 3v3 Wipeout queue.
- Rename the `wipeout-3v3` display name if it still names only Verdant Crossfire.
- Reuse a current presentation theme. A theme different from Verdant's is preferred so the two maps read differently. New gameplay mechanics, assets, and rendering systems are out of scope.
- Update `docs/04-maps-and-game-modes.md`, which lists the advertised 3v3 maps.

# Acceptance criteria

- The recipe resolves and validates. `wipeout-3v3` advertises both maps, and matches on the new map admit, restart, recover, and render through production paths.
- Spawn safety, fighter clearance, projectile readability, Practice bot navigation, destructible state (if used), and the maximum 3v3 performance gate all pass on the new map.
- Matchmade and Practice selection can both reach the new map.
- The recipe rationale (what differs from Verdant Crossfire and why), the evidence, and the durable map documentation are recorded.

# Verification

- `just lint` and `just test`, including the map catalog, validation, and admission tests that cover advertised maps.
- `just practice-e2e wipeout-3v3`, run so that it actually lands on the new map. Extend the script or its selection only if it cannot already target a specific map.
- `just v3-render-evidence`, with `BRAWLER_RENDER_PLAYERS_PER_TEAM=3` and `BRAWLER_RENDER_GAME_TYPE=wipeout-3v3`, on the new map. Attach a top-down capture and a live-match capture.
- A bot-navigation check that every spawn reaches the contested area and both flanks.

# Playtest notes (owner, after done)

This is not a gate for done. The agent fills in the run path and controls when the work lands. The owner plays 3v3 Practice on each map back to back with keyboard and controller, and observes:

- whether range and flank choices feel different from Verdant Crossfire;
- whether either side has a spawn or route advantage;
- whether any spot allows indefinite disengagement.

Feedback goes to this ticket or a new linked ticket.
