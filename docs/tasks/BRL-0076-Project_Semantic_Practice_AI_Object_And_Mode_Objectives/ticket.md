---
id: BRL-0076
title: Project semantic Practice AI object and mode objectives
status: done
theme:
release:
priority: high
created: 2026-08-30T21:25:08Z
modified: 2026-08-30T21:50:51Z
closed: 2026-08-30T21:50:51Z
revision: 102bccaf7d2dc32b
blocks: []
related: [BRL-0070]
---

# Description

Remove Practice AI dependencies on concrete world-object asset IDs and central mode-specific state inspection. Map/object and match-mode owners publish bounded semantic ECS projections; the bot controller consumes those projections without gaining gameplay authority.
