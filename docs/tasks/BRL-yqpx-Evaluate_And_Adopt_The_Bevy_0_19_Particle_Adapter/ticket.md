---
id: BRL-yqpx
was: BRL-0102

title: Evaluate and adopt the Bevy 0.19 particle adapter

status: done
closed: 2026-08-31T19:34:31Z
status_changed: 2026-08-31T19:34:31Z

theme:

release:

created: 2026-08-31T08:21:40Z

blocks: [BRL-qxdw]

related: [BRL-hheg]

parent:

rank:
---

# Description

Evaluate a Bevy 0.19-compatible GPU particle adapter against Brawler's production constraints, then either adopt it behind the recipe layer for decorative chaos or record a measured no-adoption decision. Particles must never own critical geometry, gameplay timing, authority, or required identity.
