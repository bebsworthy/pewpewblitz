---
id: BRL-gjz8
was: BRL-0166

title: Replace file-wide Clippy allows with item-level exceptions

status: done
closed: 2026-11-14T05:15:18Z
status_changed: 2026-11-14T05:15:18Z

theme: quality

release:

created: 2026-10-23T13:19:03Z

blocks: []

related: [BRL-59qw, BRL-v34z]

parent: BRL-q5c4

rank:
---

# Description

Remove module-wide `#![allow(...)]` attributes that AGENTS.md forbids, replacing them with narrow item-level exceptions or decomposition.
