---
id: BRL-thwh
was: BRL-0212

title: Make the owner-prediction dash convergence test deterministic in the full network suite

status: done
closed: 2026-11-17T18:56:57Z
status_changed: 2026-11-17T18:56:57Z

theme:

release:

created: 2026-11-14T04:44:38Z

blocks: []

related: [BRL-p246, BRL-rz4s]

parent: BRL-q5c4

rank:
---

# Description

`prediction::dash_converges_without_snapping` passes reliably in the full `just test` network suite instead of occasionally failing on its no-visible-teleport assertion.
