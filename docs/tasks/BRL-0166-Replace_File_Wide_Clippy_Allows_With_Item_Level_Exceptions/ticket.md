---
id: BRL-0166
title: Replace file-wide Clippy allows with item-level exceptions
status: todo
theme: quality
release:
created: 2026-10-23T13:19:03Z
modified: 2026-11-12T21:32:43Z
closed:
status_changed: 2026-11-12T20:15:05Z
history:
- backlog@2026-10-23T13:19:05Z
- todo@2026-11-12T20:15:05Z
revision: dca772ab2fc83e42
blocks: []
related: [BRL-0134]
parent: BRL-0171
---

# Description

Remove module-wide `#![allow(...)]` attributes that AGENTS.md forbids, replacing them with narrow item-level exceptions or decomposition.
