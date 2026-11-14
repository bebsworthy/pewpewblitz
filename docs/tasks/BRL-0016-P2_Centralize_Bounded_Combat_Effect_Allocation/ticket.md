---
id: BRL-0016

title: 'P2: Centralize bounded combat effect allocation'

status: done
closed: 2026-08-27T23:42:00Z
status_changed: 2026-08-27T23:42:00Z

theme: quality

release:

created: 2026-08-27T18:15:31Z

blocks: []

related: [BRL-0002]

parent:

rank:
---

# Description

Audit finding DUP-02. Four cue consumers duplicate effect-capacity eviction, oldest selection, sequence allocation, and nearly identical spawn bundles. Source: audit/bevy-rust-code-audit-20260827.md.
