---
id: BRL-0166
title: Replace file-wide Clippy allows with item-level exceptions
status: backlog
theme: quality
release:
created: 2026-10-23T13:19:03Z
modified: 2026-10-23T13:19:05Z
closed:
revision: 7dc7a69e1e8a4c55
blocks: []
related: [BRL-0134]
---

# Description

Remove module-wide `#![allow(...)]` attributes that AGENTS.md forbids, replacing them with narrow item-level exceptions or decomposition.
