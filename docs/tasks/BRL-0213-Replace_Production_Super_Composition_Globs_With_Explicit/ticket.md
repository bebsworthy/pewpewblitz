---
id: BRL-0213
title: 'Replace production super::* composition globs with explicit imports'
status: todo
theme: quality
release:
created: 2026-11-14T05:13:27Z
modified: 2026-11-14T05:14:00Z
closed:
status_changed: 2026-11-14T05:13:27Z
history:
- todo@2026-11-14T05:13:27Z
revision: 0e0baa20bcc049a8
blocks: []
related: [BRL-0166]
parent: BRL-0171
---

# Description

Production modules name the parent items they use instead of `use super::*`, so each module's dependencies are visible and the 66 item-level `clippy::wildcard_imports` allows left by BRL-0166 can be deleted.
