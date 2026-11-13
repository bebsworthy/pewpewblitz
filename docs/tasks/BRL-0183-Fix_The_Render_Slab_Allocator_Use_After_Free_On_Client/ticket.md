---
id: BRL-0183
title: Fix the render slab allocator use-after-free on client start and match exit
status: todo
theme: quality
release:
created: 2026-11-13T06:34:38Z
modified: 2026-11-13T06:34:41Z
closed:
status_changed: 2026-11-13T06:34:38Z
history:
- todo@2026-11-13T06:34:38Z
revision: 2e17da4220de2fa6
blocks: []
related: [BRL-0149, BRL-0151, BRL-0152]
parent: BRL-0171
---

# Description

Stop the client from logging `bevy_render::slab_allocator: Use-after-free: attempted to copy element data for an unallocated key` by freeing or replacing render meshes only after nothing references them.
