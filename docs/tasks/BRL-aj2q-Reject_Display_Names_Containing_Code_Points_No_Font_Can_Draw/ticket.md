---
id: BRL-aj2q
was: BRL-0214

title: Reject display names containing code points no font can draw

status: doing
closed:
status_changed: 2026-11-18T03:22:57Z

theme: quality

release:

created: 2026-11-14T05:52:38Z

blocks: []

related: [BRL-5e2s]

parent: BRL-q5c4

rank: 11337728
---

# Description

Display-name validation rejects characters that no shipped or system font can draw, with clear feedback, instead of letting them render as boxes.
