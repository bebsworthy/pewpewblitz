# Context

BRL-0154 made Riff the single default fighter presentation. Choosing a model per brawler needs a
profile/data decision (where the stable character ID lives, how it replicates, and catalog
validation) and at least a second production character to be worth building.

# Scope

- Decide the stable character identity and its owner (profile, build catalog, or presentation-only).
- Replicate or derive it for every observer; resolve it to a client presentation entry.
- Brawler screen choice UI.

# Acceptance criteria

- Two characters selectable and rendered consistently in gameplay and previews.
- No process-local or asset paths on the wire.
