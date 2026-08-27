---
id: BRL-0013
title: 'P2: Verify ambiguities in Brawler-owned schedules'
status: backlog
theme: quality
release:
created: 2026-08-27T18:15:30Z
modified: 2026-08-27T18:15:32Z
closed:
revision: 8d5e9c44ebe2ecf2
blocks: []
related: [BRL-0002]
---

# Description

Audit finding SCHED-02. The project has extensive explicit scheduling but does not configure Bevy schedule ambiguity detection, whose default is Ignore. Source: audit/bevy-rust-code-audit-20260827.md.
