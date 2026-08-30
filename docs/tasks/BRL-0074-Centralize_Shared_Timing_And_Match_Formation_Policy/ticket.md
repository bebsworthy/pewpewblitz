---
id: BRL-0074
title: Centralize shared timing and match formation policy
status: done
theme:
release:
priority: none
created: 2026-08-30T20:36:36Z
modified: 2026-08-30T20:58:28Z
closed: 2026-08-30T20:58:28Z
revision: 1d383908fd7d3b1f
blocks: []
related: [BRL-0070]
---

# Description

Replace duplicated authoritative tick-rate conversions and lobby formation deadline literals with canonical validated sources. Preserve 60 Hz simulation semantics, the 30-second loading deadline, and the 10-second grant deadline while ensuring Balance Lab, game-selection presentation, Practice/product reservation advertisement, and formation enforcement cannot drift independently.
