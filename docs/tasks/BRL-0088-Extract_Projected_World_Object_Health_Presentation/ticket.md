---
id: BRL-0088
title: Extract projected world-object health presentation
status: done
theme:
release:
created: 2026-08-31T05:29:04Z
modified: 2026-08-31T05:46:03Z
closed: 2026-08-31T05:46:03Z
revision: a87151568960dd21
blocks: []
related: [BRL-0070]
---

# Description

Extract the screen-space health-bar projection lifecycle for replicated damageable map objects and Heist safes from the large 3D presentation composition root into one private module while preserving exact identity reconciliation, health policies, styling, projection behavior, deferred commands, and PostUpdate ordering.
