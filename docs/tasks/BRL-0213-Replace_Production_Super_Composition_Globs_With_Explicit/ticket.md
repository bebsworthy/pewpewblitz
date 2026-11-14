---
id: BRL-0213

title: 'Replace production super::* composition globs with explicit imports'

status: todo
closed:
status_changed: 2026-11-14T05:13:27Z

theme: quality

release:

created: 2026-11-14T05:13:27Z

blocks: []

related: [BRL-0166]

parent: BRL-0171

rank: 14024704
---

# Description

Production modules name the parent items they use instead of `use super::*`, so each module's dependencies are visible and the 66 item-level `clippy::wildcard_imports` allows left by BRL-0166 can be deleted.
