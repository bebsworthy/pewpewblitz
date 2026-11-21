---
id: BRL-shwv

title: Give routed-process tests a fresh profile database per test

status: done
closed: 2026-11-21T04:41:51Z
status_changed: 2026-11-21T04:41:51Z

theme:

release:

created: 2026-11-20T03:16:42Z

blocks: []

related: [BRL-1chc]

parent:

rank:
---

# Description

Each routed-process test opens a profile database that no earlier run's file can replace.
