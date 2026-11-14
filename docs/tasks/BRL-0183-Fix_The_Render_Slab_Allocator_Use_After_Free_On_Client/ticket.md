---
id: BRL-0183

title: Fix the render slab allocator use-after-free on client start and match exit

status: done
closed: 2026-11-13T10:38:53Z
status_changed: 2026-11-13T10:38:53Z

theme: quality

release:

created: 2026-11-13T06:34:38Z

blocks: []

related: [BRL-0149, BRL-0151, BRL-0152]

parent: BRL-0171

rank:
---

# Description

Stop the client from logging `bevy_render::slab_allocator: Use-after-free: attempted to copy element data for an unallocated key` by freeing or replacing render meshes only after nothing references them.
