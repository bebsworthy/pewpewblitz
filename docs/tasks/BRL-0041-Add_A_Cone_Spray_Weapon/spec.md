# Outcome

Players can equip a Spray weapon that damages eligible targets inside a short authored cone rather than launching projectile entities.

# First slice

- Add one stable Spray weapon base using existing magazine/ammunition recovery and fire cooldown.
- Each accepted attack resolves one instantaneous authoritative cone pulse with authored reach, angle, damage, falloff, map-occlusion policy, and maximum targets.
- Resolve candidates in stable order and apply existing damage attribution/outcome facts; no projectile entity, flight, sweep, or lifetime is created.
- Show an exact local cone preview over client-observed blockers and targets, plus distinct spray, contact, damage, audio, and controller feedback.
- Preserve server authority under movement, latency, concealment, spawn protection, defeat, reconnect, and restart.
- Add Balance Lab fields, saved-brawler/weapon-base content, evidence, telemetry, and Practice-bot range/use behavior.

# Boundaries

- The first slice is repeated discrete cone pulses, not a continuous channel.
- No lingering area, elemental status, healing, knockback, terrain destruction, piercing through blocking cover, arbitrary cone segmentation, or client hit prediction.
- Persistent splash areas are owned by a separate ticket.

# Acceptance criteria

- Cone geometry, boundary/tangency, map occlusion, stable multi-target selection, falloff, cooldown/ammo, attribution, concealment, and lifecycle pass pure/ECS/network tests.
- No projectile is spawned and no client can claim a cone contact.
- Aim preview and native impacts agree with authoritative observed geometry.
- Maximum 3v3 target density, bots, routed recovery, performance, presentation, documentation, and feedback gates pass.
