---
id: BRL-3p70
was: BRL-0194

title: Finish excluding inert automation data and legacy evidence observers from the player client

status: done
closed: 2026-11-14T22:06:38Z
status_changed: 2026-11-14T22:06:38Z

theme: quality

release:

created: 2026-11-13T14:08:49Z

blocks: []

related: [BRL-5fk5, BRL-rhw3, BRL-zv4x]

parent: BRL-q5c4

rank:
---

# Description

The player client build contains no automation-only data fields, validation messages, or legacy evidence observers; BRL-zv4x gated the automation drivers, and this ticket removes the inert remainder.
