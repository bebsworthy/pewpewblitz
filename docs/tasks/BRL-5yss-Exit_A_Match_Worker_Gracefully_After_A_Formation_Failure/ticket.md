---
id: BRL-5yss

title: Exit a match worker gracefully after a formation failure instead of logging WorkerExitMismatch

status: doing
closed:
status_changed: 2026-11-19T11:16:24Z

theme:

release:

created: 2026-11-16T22:20:58Z

blocks: []

related: [BRL-jxfy]

parent:

rank: 11534336
---

# Description

A match worker that ends because formation failed exits cleanly and isn't logged as a worker failure.
