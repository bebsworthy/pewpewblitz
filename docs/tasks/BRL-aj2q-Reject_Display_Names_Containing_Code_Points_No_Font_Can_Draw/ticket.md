---
id: BRL-aj2q
was: BRL-0214

title: Reject display names containing code points no font can draw

status: done
closed: 2026-11-18T03:39:55Z
status_changed: 2026-11-18T03:39:55Z

theme: quality

release:

created: 2026-11-14T05:52:38Z

blocks: []

related: [BRL-5e2s]

parent: BRL-q5c4

rank:
---

# Description

Display-name validation rejects characters that no shipped or system font can draw, with clear feedback, instead of letting them render as boxes.
