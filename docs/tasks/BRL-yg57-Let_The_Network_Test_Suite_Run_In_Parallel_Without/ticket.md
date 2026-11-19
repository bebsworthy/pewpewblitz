---
id: BRL-yg57

title: Let the network test suite run in parallel without overlapping late-input capture scopes

status: backlog
closed:
status_changed: 2026-11-19T13:51:28Z

theme:

release:

created: 2026-11-19T13:51:21Z

blocks: []

related: [BRL-p246]

parent:

rank: 10092544
---

# Description

Found in BRL-p246: process-global late-input capture scope (`src/logging.rs:136`) makes 4 network tests fail when run in parallel; `just test` serialises them.
