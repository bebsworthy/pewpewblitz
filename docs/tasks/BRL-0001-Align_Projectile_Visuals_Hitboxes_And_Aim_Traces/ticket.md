---
id: BRL-0001
title: 'Align projectile visuals, hitboxes, and aim traces'
status: done
theme:
release:
created: 2026-08-27T13:52:41Z
modified: 2026-08-27T17:46:22Z
closed: 2026-08-27T17:46:22Z
revision: 2fea5868da374675
blocks: [BRL-0003]
related: []
---

# Description

Make straight-projectile collision geometry a single shared gameplay fact so the authoritative sweep, the visible projectile body, and the local aim trace agree. The first slice covers the current circular projectile shape and straight trajectory while keeping body geometry separate from trajectory for later, evidence-backed additions.
