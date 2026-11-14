---
id: BRL-0214
title: Reject display names containing code points no font can draw
status: todo
theme: quality
release:
created: 2026-11-14T05:52:38Z
modified: 2026-11-14T05:52:54Z
closed:
status_changed: 2026-11-14T05:52:38Z
history:
- todo@2026-11-14T05:52:38Z
revision: 509f364c64c9c0d9
blocks: []
related: [BRL-0168]
parent: BRL-0171
---

# Description

Display-name validation rejects characters that no shipped or system font can draw, with clear feedback, instead of letting them render as boxes.
