---
id: BRL-0106
title: Align native render evidence duration validation with the sample gate
status: backlog
theme:
release:
created: 2026-08-31T11:43:50Z
modified: 2026-08-31T11:44:10Z
closed:
revision: 179ba55a74054063
blocks: []
related: []
---

# Description

Prevent apparently valid native render-evidence configurations from deterministically failing the locked 1,200-sample gate solely because the requested measurement window is too short. Align validation, operator guidance, and test coverage without weakening the performance threshold or changing historical evidence.
