---
id: BRL-dq06

title: Generate mipmaps for the floor decal atlas and other minified client textures

status: done
closed: 2026-11-18T08:24:28Z
status_changed: 2026-11-18T08:24:28Z

theme: core

release:

created: 2026-11-14T14:02:55Z

blocks: []

related: [BRL-23q3]

parent: BRL-q5c4

rank:
---

# Description

Minified client textures (starting with the BRL-23q3 floor decal atlas) sample a mip chain, so they stay stable on small or zoomed-out views.
