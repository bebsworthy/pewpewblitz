# Context

BRL-wrer added the `ShardRushSource { center_half_cell }` mode anchor and the built-in Prism Quarry map. Map Maker currently admits private recipes for Takedown, Turf Hold, and Vault Break only; the anchor kind is already handled by the editor's rendering and validation pattern matches but cannot be placed.

# Scope

- Add Shard Rush to the Map Maker mode picker and a tool for placing exactly one source anchor (half-cell precision), with validation messages.
- Lobby admission for Shard Rush authored-recipe Practice, reusing the existing private-recipe path.
- Check the draft file format: adding a mode should be additive; record whether the draft schema needs a bump.

# Acceptance

- A player can create, validate, save, and practice a Shard Rush map; invalid source counts are rejected with a clear message.
- Focused Map Maker model and admission tests; `just lint`, `just test`.
