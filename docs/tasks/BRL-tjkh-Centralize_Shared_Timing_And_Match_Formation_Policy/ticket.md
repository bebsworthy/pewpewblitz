---
id: BRL-tjkh
was: BRL-0074

title: Centralize shared timing and match formation policy

status: done
closed: 2026-08-30T20:58:28Z
status_changed: 2026-08-30T20:58:28Z

theme:

release:

created: 2026-08-30T20:36:36Z

blocks: []

related: [BRL-4ms7]

parent:

rank:
---

# Description

Replace duplicated authoritative tick-rate conversions and lobby formation deadline literals with canonical validated sources. Preserve 60 Hz simulation semantics, the 30-second loading deadline, and the 10-second grant deadline while ensuring Balance Lab, game-selection presentation, Practice/product reservation advertisement, and formation enforcement cannot drift independently.
