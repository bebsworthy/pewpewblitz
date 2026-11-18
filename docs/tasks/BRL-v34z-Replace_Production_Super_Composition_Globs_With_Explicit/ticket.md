---
id: BRL-v34z
was: BRL-0213

title: 'Replace production super::* composition globs with explicit imports'

status: done
closed: 2026-11-18T17:53:29Z
status_changed: 2026-11-18T17:53:29Z

theme: quality

release:

created: 2026-11-14T05:13:27Z

blocks: []

related: [BRL-gjz8]

parent: BRL-q5c4

rank:
---

# Description

Production modules name the parent items they use instead of `use super::*`, so each module's dependencies are visible and the 66 item-level `clippy::wildcard_imports` allows left by BRL-gjz8 can be deleted.
