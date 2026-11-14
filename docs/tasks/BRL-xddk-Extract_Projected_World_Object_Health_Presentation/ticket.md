---
id: BRL-xddk
was: BRL-0088

title: Extract projected world-object health presentation

status: done
closed: 2026-08-31T05:46:03Z
status_changed: 2026-08-31T05:46:03Z

theme:

release:

created: 2026-08-31T05:29:04Z

blocks: []

related: [BRL-4ms7]

parent:

rank:
---

# Description

Extract the screen-space health-bar projection lifecycle for replicated damageable map objects and Heist safes from the large 3D presentation composition root into one private module while preserving exact identity reconciliation, health policies, styling, projection behavior, deferred commands, and PostUpdate ordering.
