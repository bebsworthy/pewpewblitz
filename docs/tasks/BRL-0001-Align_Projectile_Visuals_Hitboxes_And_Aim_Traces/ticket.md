---
id: BRL-0001

title: 'Align projectile visuals, hitboxes, and aim traces'

status: done
closed: 2026-08-27T17:46:22Z
status_changed: 2026-08-27T17:46:22Z

theme:

release:

created: 2026-08-27T13:52:41Z

blocks: []

related: [BRL-0094]

parent:

rank:
---

# Description

Make straight-projectile collision geometry a single shared gameplay fact so the authoritative sweep, the visible projectile body, and the local aim trace agree. The first slice covers the current circular projectile shape and straight trajectory while keeping body geometry separate from trajectory for later, evidence-backed additions.
