---
id: BRL-0016
title: 'P2: Centralize bounded combat effect allocation'
status: todo
theme: quality
release:
priority: none
created: 2026-08-27T18:15:31Z
modified: 2026-08-27T20:54:52Z
closed:
revision: 18e55ff75820d1fa
blocks: []
related: [BRL-0002]
---

# Description

Audit finding DUP-02. Four cue consumers duplicate effect-capacity eviction, oldest selection, sequence allocation, and nearly identical spawn bundles. Source: audit/bevy-rust-code-audit-20260827.md.
