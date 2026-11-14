---
id: BRL-0214

title: Reject display names containing code points no font can draw

status: todo
closed:
status_changed: 2026-11-14T05:52:38Z

theme: quality

release:

created: 2026-11-14T05:52:38Z

blocks: []

related: [BRL-0168]

parent: BRL-0171

rank: 14090240
---

# Description

Display-name validation rejects characters that no shipped or system font can draw, with clear feedback, instead of letting them render as boxes.
