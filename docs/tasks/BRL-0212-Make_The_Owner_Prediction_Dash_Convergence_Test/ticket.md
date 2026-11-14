---
id: BRL-0212

title: Make the owner-prediction dash convergence test deterministic in the full network suite

status: todo
closed:
status_changed: 2026-11-14T04:44:38Z

theme:

release:

created: 2026-11-14T04:44:38Z

blocks: []

related: [BRL-0208]

parent: BRL-0171

rank: 13959168
---

# Description

`prediction::dash_converges_without_snapping` passes reliably in the full `just test` network suite instead of occasionally failing on its no-visible-teleport assertion.
