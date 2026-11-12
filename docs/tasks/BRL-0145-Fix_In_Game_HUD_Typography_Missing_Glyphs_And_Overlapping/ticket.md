---
id: BRL-0145
title: 'Fix in-game HUD typography, missing glyphs, and overlapping overhead labels'
status: done
theme: quality
release:
created: 2026-10-23T13:19:03Z
modified: 2026-11-12T20:13:21Z
closed: 2026-11-12T20:13:21Z
status_changed: 2026-11-12T20:13:21Z
history:
- doing@2026-10-23T15:22:38Z
- done@2026-11-12T20:13:21Z
revision: ad11c8d41dd87cc5
blocks: [BRL-0155]
related: [BRL-0149, BRL-0168]
parent: BRL-0171
---

# Description

Render all in-match text in the shipped display font without missing glyphs, keep overhead labels legible when fighters stand close, and stop rewriting HUD text every frame.
