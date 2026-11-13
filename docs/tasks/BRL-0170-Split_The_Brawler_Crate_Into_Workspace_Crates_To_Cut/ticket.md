---
id: BRL-0170
title: Split the brawler crate into workspace crates to cut rebuild cost
status: todo
theme: quality
release:
created: 2026-10-23T17:18:22Z
modified: 2026-11-13T20:09:05Z
closed:
status_changed: 2026-11-12T20:15:05Z
history:
- backlog@2026-10-23T17:18:29Z
- todo@2026-11-12T20:15:05Z
revision: 7be728fd0bbb3fc5
blocks: []
related: [BRL-0012]
parent: BRL-0171
---

# Description

Measure, then split the single brawler package into a small number of workspace crates along its existing execution-role boundaries. An edit to one side (for example client presentation) should no longer recompile the gameplay core under every feature set, and both incremental edit-to-test time and target/ disk use should drop measurably.
