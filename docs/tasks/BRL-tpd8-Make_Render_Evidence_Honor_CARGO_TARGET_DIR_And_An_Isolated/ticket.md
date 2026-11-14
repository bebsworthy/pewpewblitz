---
id: BRL-tpd8

title: Make render evidence honor CARGO_TARGET_DIR and an isolated server data directory

status: backlog
closed:
status_changed: 2026-11-14T13:20:27Z

theme:

release:

created: 2026-11-14T13:20:27Z

blocks: []

related: [BRL-0176]

parent: BRL-0171

rank: 13238272
---

# Description

`just v3-render-evidence` runs the binaries it just built and never writes the tracked `data/server/` directory.
