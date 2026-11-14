# Outcome

Players can equip and activate one short Speed Burst item as an additional bounded tactical action in matches and Practice.

# Scope

- Add one active-item slot to saved-brawler creation/editing, persistence, advertisements, immutable match loadouts, profile revisions, and routed snapshots.
- Add one abstract rebindable active-item input with keyboard/controller parity.
- Define Speed Burst activation, movement modifier, duration, cooldown, rejection, tradeoff, presentation profile, HUD deadline, audio, and feedback.
- The authoritative server validates activation and owns all runtime state; clients only send intent and present replicated facts.
- Initialize, preserve, reject, reset, and clean up state across queue lock, spawn, defeat, respawn, disconnect, build replacement, match restart, and teardown.
- Add Balance Lab fields and focused Practice-bot use through ordinary input.

# Boundaries

- No second item, item framework, consumable inventory, acquisition, currency, rarity, shield, decoy, partial reload, charges, or monetization.
- Speed Burst does not grant collision bypass, Dash semantics, or vertical movement.

# Acceptance criteria

- Slot legality, persistence, resolution, input, activation, cooldown, movement composition, lifecycle, replication, and recovery pass focused and routed tests.
- Dash, slows, effect tiles, Freeze, and ordinary speed modifiers have an explicit deterministic composition rule.
- HUD and native playtests make readiness, activation, expiry, and rejection readable without crowding existing actions.
- Server role isolation, performance, documentation, and feedback gates pass.
