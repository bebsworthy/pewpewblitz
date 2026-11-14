---
id: BRL-dq06

title: Generate mipmaps for the floor decal atlas and other minified client textures

status: backlog
closed:
status_changed: 2026-11-14T14:02:55Z

theme: core

release:

created: 2026-11-14T14:02:55Z

blocks: []

related: [BRL-0178]

parent: BRL-0171

rank: 13303808
---

# Description

Minified client textures (starting with the BRL-0178 floor decal atlas) sample a mip chain, so they stay stable on small or zoomed-out views.
