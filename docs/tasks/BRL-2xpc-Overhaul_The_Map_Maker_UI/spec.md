# Origin

Owner feedback, 2026-11-18: "the map maker UI is very poor and needs a complete overhaul". This comes before the incremental Map Maker improvements (BRL-8q4c phone layout, BRL-tztp advanced tools, BRL-qq21 My Maps style), which should be re-scoped against the new design or folded into it.

# Current state

- The library (`map-library`) and editor (`map-editor`, `map-editor-palette`) can be captured natively with `just shell-capture`; the BRL-pw9k sheets in `target/pw9k/` show the current look.
- The library is plain centered text on a dark background with tiny buttons. The editor is a dense toolbar plus an asset drawer, and Map Maker is exempt from the 48 dp touch-target policy (BRL-q148).
- Shipped scope and behaviour: BRL-6vjz (bounded builder), BRL-cwm9 and BRL-kgba (retained UI nodes), BRL-yaa4 (dead readout removed).

# Approach (design first)

1. Audit the current flows (library, create/rename, editor tools, palette, validation issues, test map, save) and list the problems with captures.
2. Produce 2–3 layout directions as mockups or gallery states using the shell toolkit (`Surface` roles, shell font, `spawn_action`, touch targets). Cover desktop 1280x720 and phone 914x411. Ask the owner to choose with a Ticket question.
3. Implement the chosen design. Keep the authoritative map recipe, validation and undo model unchanged; this is presentation and interaction only.

# Acceptance

- The owner approves the design direction (question) and the final native captures.
- `just shell-capture map-library map-editor` at both sizes shows the new UI; every control meets the touch-target policy on phones.
- Existing Map Maker tests pass and are updated for the new structure; `just lint`, `just test`.
- Pixel 8a playtest: create, edit, validate and test-play a map by touch.
