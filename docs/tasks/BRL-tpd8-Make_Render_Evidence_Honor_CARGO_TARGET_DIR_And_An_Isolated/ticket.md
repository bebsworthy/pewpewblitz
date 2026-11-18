---
id: BRL-tpd8

title: Make render evidence honor CARGO_TARGET_DIR and an isolated server data directory

status: done
closed: 2026-11-18T05:22:10Z
status_changed: 2026-11-18T05:22:10Z

theme:

release:

created: 2026-11-14T13:20:27Z

blocks: []

related: [BRL-farz]

parent: BRL-q5c4

rank:
---

# Description

`just v3-render-evidence` runs the binaries it just built and never writes the tracked `data/server/` directory.
