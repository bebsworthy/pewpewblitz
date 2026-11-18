---
id: BRL-vh61
was: BRL-0106

title: Align native render evidence duration validation with the sample gate

status: backlog
closed:
status_changed: 2026-08-31T11:44:10Z

theme:

release:

created: 2026-08-31T11:43:50Z

blocks: []

related: [BRL-a83q]

parent:

rank: 308954
---

# Description

Prevent apparently valid native render-evidence configurations from deterministically failing the locked 1,200-sample gate solely because the requested measurement window is too short. Align validation, operator guidance, and test coverage without weakening the performance threshold or changing historical evidence.
