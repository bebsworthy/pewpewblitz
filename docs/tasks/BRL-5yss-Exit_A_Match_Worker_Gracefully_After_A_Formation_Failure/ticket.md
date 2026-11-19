---
id: BRL-5yss

title: Exit a match worker gracefully after a formation failure instead of logging WorkerExitMismatch

status: done
closed: 2026-11-19T12:11:34Z
status_changed: 2026-11-19T12:11:34Z

theme:

release:

created: 2026-11-16T22:20:58Z

blocks: []

related: [BRL-jxfy]

parent:

rank:
---

# Description

A match worker that ends because formation failed exits cleanly and isn't logged as a worker failure.
