---
id: BRL-0166
title: Replace file-wide Clippy allows with item-level exceptions
status: done
theme: quality
release:
created: 2026-10-23T13:19:03Z
modified: 2026-11-14T05:15:18Z
closed: 2026-11-14T05:15:18Z
status_changed: 2026-11-14T05:15:18Z
history:
- backlog@2026-10-23T13:19:05Z
- todo@2026-11-12T20:15:05Z
- doing@2026-11-14T04:51:39Z
- done@2026-11-14T05:15:18Z
revision: f4cc9f3b0bcf0cdf
blocks: []
related: [BRL-0134, BRL-0213]
parent: BRL-0171
---

# Description

Remove module-wide `#![allow(...)]` attributes that AGENTS.md forbids, replacing them with narrow item-level exceptions or decomposition.
