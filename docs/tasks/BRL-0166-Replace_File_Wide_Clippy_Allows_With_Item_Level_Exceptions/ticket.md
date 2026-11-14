---
id: BRL-0166

title: Replace file-wide Clippy allows with item-level exceptions

status: done
closed: 2026-11-14T05:15:18Z
status_changed: 2026-11-14T05:15:18Z

theme: quality

release:

created: 2026-10-23T13:19:03Z

blocks: []

related: [BRL-0134, BRL-0213]

parent: BRL-0171

rank:
---

# Description

Remove module-wide `#![allow(...)]` attributes that AGENTS.md forbids, replacing them with narrow item-level exceptions or decomposition.
