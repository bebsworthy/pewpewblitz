---
id: BRL-0013
title: 'P2: Verify ambiguities in Brawler-owned schedules'
status: todo
theme: quality
release:
priority: none
created: 2026-08-27T18:15:30Z
modified: 2026-08-27T20:54:31Z
closed:
revision: 3d5607613df1c724
blocks: []
related: [BRL-0002]
---

# Description

Audit finding SCHED-02. The project has extensive explicit scheduling but does not configure Bevy schedule ambiguity detection, whose default is Ignore. Source: audit/bevy-rust-code-audit-20260827.md.
