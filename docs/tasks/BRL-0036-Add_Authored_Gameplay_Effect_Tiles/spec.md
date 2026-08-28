# Outcome

Maps can place gameplay effect tiles that change route decisions through clearly presented speed boosts, slows, or periodic damage while the server owns occupancy and outcomes.

# First slice

- Add three explicit tile capabilities: Speed, Slow, and Damage.
- Attach validated immutable effect definitions to approved map-asset gameplay profiles; recipes place stable asset IDs rather than arbitrary rules.
- Evaluate fighter occupancy on the authoritative fixed schedule using the existing planar map geometry.
- Define deterministic entry, exit, continuous/pulse timing, overlap precedence, defeat, respawn, restart, map replacement, and teardown behavior.
- Speed and Slow modify fighter movement through one reviewed composition rule; Damage produces attributed environment outcomes on bounded pulse deadlines.
- Replicate only the facts clients need for HUD/world presentation, late join, and recovery.
- Add one focused map or existing-map variant that proves meaningful route choice for all three effects.
- Practice bots observe public tile facts and include route cost/benefit without gaining hidden information.

# Boundaries

- The first slice excludes conveyors, ice momentum, teleporters, healing, shields, silence, projectile modifiers, elemental reactions, arbitrary effect graphs, and player-authored executable parameters.
- Visual materials never grant gameplay behavior.

# Acceptance criteria

- Tile validation, occupancy boundaries, overlap, pulse timing, movement composition, damage attribution, lifecycle, and recovery pass pure/ECS/network tests.
- Clients cannot claim membership or outcomes.
- Tile presentation is distinguishable before entry and while affected, including reduced-effects mode.
- Bot navigation, maximum-density performance, routed play, native readability, and documentation pass.
