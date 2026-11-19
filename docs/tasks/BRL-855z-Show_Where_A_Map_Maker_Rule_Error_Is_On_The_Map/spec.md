# Context (deferred from BRL-2xpc, 2026-11-19)

Map Maker's Check tab (`map_maker/check.rs`) lists the countable requirements: spawns per team, the capture zone and the Heist safes. Each unmet requirement has an **Add** action that opens Assets on the entry that fixes it.

When every count is met but `validate_authored_recipe` still refuses the map, Check shows the validator's message as text only. Examples: "effect tile violates spawn safety clearance", "placement is incompatible with the effective surface", and the navigation rules. The validator returns a `String` without the cells or placements involved, so the editor cannot point at the problem.

# Scope

- Let the authoritative validation report the offending placement IDs or cells next to its reason. Use a typed error detail, not message parsing, and keep the server admission path and wire contracts unchanged.
- In Check, add **Show** for those issues. It selects the offending placements (Select tool) and frames the camera on them.

# Acceptance

- A spawn-clearance or surface refusal shows **Show**, and Show selects and frames the offending items.
- The validator's accept/refuse decisions are unchanged (existing map validation tests pass).
