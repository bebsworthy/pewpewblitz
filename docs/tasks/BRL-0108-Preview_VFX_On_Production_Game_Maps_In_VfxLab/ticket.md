---
id: BRL-0108
title: Preview VFX on production game maps in VfxLab
status: backlog
theme:
release:
created: 2026-08-31T17:42:13Z
modified: 2026-11-12T20:15:05Z
closed:
status_changed: 2026-11-12T20:15:05Z
history:
- todo@2026-08-31T17:45:16Z
- backlog@2026-11-12T20:15:05Z
revision: 13c55efa56940993
blocks: [BRL-0094]
related: [BRL-0094, BRL-0096, BRL-0098]
parent:
---

# Description

## Outcome

Extend the read-only VfxLab so effects can be viewed on selectable production game maps using the same map presentation, camera conventions, authored geometry, and VFX materializers as the game client.

The lab must make visual-context mismatches obvious before playtesting: map surface color and contrast, real effect scale, relationship treatment, nearby fighters and cover, objective clutter, and production camera framing. A neutral gray fixture remains useful for diagnosis, but it must no longer be the only background against which an effect is judged.

This is a deferred follow-up after the BRL-0094 implementation sequence and the integrated ownership-treatment ticket.
