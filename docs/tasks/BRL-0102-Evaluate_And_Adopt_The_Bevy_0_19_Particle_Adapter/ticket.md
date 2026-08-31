---
id: BRL-0102
title: Evaluate and adopt the Bevy 0.19 particle adapter
status: todo
theme:
release:
created: 2026-08-31T08:21:40Z
modified: 2026-08-31T11:01:53Z
closed:
revision: 1bee0204228e8d8d
blocks: [BRL-0103]
related: [BRL-0094]
---

# Description

Evaluate a Bevy 0.19-compatible GPU particle adapter against Brawler's production constraints, then either adopt it behind the recipe layer for decorative chaos or record a measured no-adoption decision. Particles must never own critical geometry, gameplay timing, authority, or required identity.
