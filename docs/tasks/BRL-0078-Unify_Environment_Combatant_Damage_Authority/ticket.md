---
id: BRL-0078
title: Unify environment combatant damage authority
status: done
theme:
release:
created: 2026-08-30T22:24:38Z
modified: 2026-08-30T23:20:30Z
closed: 2026-08-30T23:20:30Z
revision: 66597ecb0340e1d3
blocks: []
related: [BRL-0070]
---

# Description

Move oil-barrel explosion damage for fighters and deployables behind one combat-owned environment-damage transaction, leaving map authority responsible only for object selection, occlusion, chaining, terminal state, and map telemetry.
