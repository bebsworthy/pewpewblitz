---
id: BRL-dq06

title: Generate mipmaps for the floor decal atlas and other minified client textures

status: doing
closed:
status_changed: 2026-11-18T08:08:40Z

theme: core

release:

created: 2026-11-14T14:02:55Z

blocks: []

related: [BRL-23q3]

parent: BRL-q5c4

rank: 11337728
---

# Description

Minified client textures (starting with the BRL-23q3 floor decal atlas) sample a mip chain, so they stay stable on small or zoomed-out views.
