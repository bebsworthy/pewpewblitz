---
id: BRL-0004
title: Apply accepted Balance Lab defaults
status: done
theme:
release:
created: 2026-08-27T17:57:06Z
modified: 2026-08-27T18:12:52Z
closed: 2026-08-27T18:12:52Z
revision: 31f6da3ef0f80c3a
blocks: []
related: []
---

# Description

Promote the exact user-approved Balance Lab comparison values to the embedded server defaults, preserving server authority and keeping canonical balance documentation synchronized.
Balance Lab changes from server defaults

- Fighters / Default / Core stats / Maximum health (/fighterProfiles/default/maximum_health): 100 -> 1000 health
- Fighters / Default / Core stats / Movement speed (/fighterProfiles/default/movement_speed): 100 -> 70 world units/s
- Fighters / Default / Recovery / Health recovery rate (/fighterProfiles/default/health_recovery_rate): 10 -> 100 health/s
- Weapons / Pulse Sidearm / Economy / Magazine capacity (/weapons/0/recipe/economy/Magazine/capacity): 6 -> 4 shots
- Weapons / Pulse Sidearm / Economy / Ammo recovery per round (/weapons/0/recipe/economy/Magazine/refill_ticks): 1.3 -> 1 s
- Weapons / Pulse Sidearm / Delivery / Projectile speed (/weapons/0/recipe/delivery/Straight/speed): 900 -> 500 world units/s
- Weapons / Pulse Sidearm / Delivery / Projectile radius (/weapons/0/recipe/delivery/Straight/radius): 6 -> 2 world units
- Weapons / Pulse Sidearm / Delivery / Maximum range (/weapons/0/recipe/delivery/Straight/range): 900 -> 320 world units
- Weapons / Pulse Sidearm / Payload 1 / Damage (/weapons/0/recipe/payload_bundles/0/effects/0/Damage/amount): 25 -> 200 health
- Weapons / Scatter Cannon / Economy / Magazine capacity (/weapons/1/recipe/economy/Magazine/capacity): 4 -> 3 shots
- Weapons / Scatter Cannon / Economy / Ammo recovery per round (/weapons/1/recipe/economy/Magazine/refill_ticks): 1.3 -> 1.2 s
- Weapons / Scatter Cannon / Firing / Projectile count (/weapons/1/recipe/firing/Spread/delivery_count): 7 -> 5 projectiles
- Weapons / Scatter Cannon / Delivery / Projectile speed (/weapons/1/recipe/delivery/Straight/speed): 850 -> 600 world units/s
- Weapons / Scatter Cannon / Delivery / Projectile radius (/weapons/1/recipe/delivery/Straight/radius): 4 -> 2 world units
- Weapons / Scatter Cannon / Delivery / Maximum range (/weapons/1/recipe/delivery/Straight/range): 720 -> 320 world units
- Weapons / Scatter Cannon / Payload 1 / Damage (/weapons/1/recipe/payload_bundles/0/effects/0/Damage/amount): 12 -> 120 health
- Weapons / Arc Launcher / Economy / Ammo recovery per round (/weapons/2/recipe/economy/Magazine/refill_ticks): 1.3 -> 1.6 s
- Weapons / Impact Blade / Economy / Ammo recovery per charge (/weapons/3/recipe/economy/Charges/recharge_ticks): 1.3 -> 1 s
