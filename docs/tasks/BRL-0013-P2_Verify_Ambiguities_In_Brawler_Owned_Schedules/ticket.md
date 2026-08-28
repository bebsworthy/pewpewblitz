---
id: BRL-0013
title: 'P2: Verify ambiguities in Brawler-owned schedules'
status: done
theme: quality
release:
priority: none
created: 2026-08-27T18:15:30Z
modified: 2026-08-27T22:48:23Z
closed: 2026-08-27T22:48:23Z
revision: d4c02074fa0cd5cd
blocks: []
related: [BRL-0002]
---

# Description

Audit finding SCHED-02. The project has extensive explicit scheduling but does not configure Bevy schedule ambiguity detection, whose default is Ignore. Source: audit/bevy-rust-code-audit-20260827.md.
