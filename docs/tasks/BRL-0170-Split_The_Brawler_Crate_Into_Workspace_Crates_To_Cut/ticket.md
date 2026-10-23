---
id: BRL-0170
title: Split the brawler crate into workspace crates to cut rebuild cost
status: backlog
theme: quality
release:
created: 2026-10-23T17:18:22Z
modified: 2026-10-23T17:18:29Z
closed:
revision: 59dd9734361eff97
blocks: []
related: [BRL-0012]
---

# Description

Measure, then split the single brawler package into a small number of workspace crates along its existing execution-role boundaries. An edit to one side (for example client presentation) should no longer recompile the gameplay core under every feature set, and both incremental edit-to-test time and target/ disk use should drop measurably.
