---
id: BRL-0013

title: 'P2: Verify ambiguities in Brawler-owned schedules'

status: done
closed: 2026-08-27T22:48:23Z
status_changed: 2026-08-27T22:48:23Z

theme: quality

release:

created: 2026-08-27T18:15:30Z

blocks: []

related: [BRL-0002]

parent:

rank:
---

# Description

Audit finding SCHED-02. The project has extensive explicit scheduling but does not configure Bevy schedule ambiguity detection, whose default is Ignore. Source: audit/bevy-rust-code-audit-20260827.md.
