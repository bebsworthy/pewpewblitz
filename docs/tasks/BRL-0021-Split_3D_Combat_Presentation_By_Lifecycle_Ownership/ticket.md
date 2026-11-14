---
id: BRL-0021

title: Split 3D combat presentation by lifecycle ownership

status: done
closed: 2026-08-28T06:06:38Z
status_changed: 2026-08-28T06:06:38Z

theme:

release:

created: 2026-08-28T05:48:12Z

blocks: []

related: [BRL-0094]

parent:

rank:
---

# Description

Refactor the oversized client-only 3D combat presentation module into cohesive lifecycle-owned modules and focused Bevy systems. Preserve every accepted visual, customization point, authority boundary, and schedule dependency while separating durable entity visuals, fighter UI/feedback, aim previews, and transient world effects. Remove avoidable owner scans and per-frame join allocations where the existing process-local entity relationships already permit direct lookup.
