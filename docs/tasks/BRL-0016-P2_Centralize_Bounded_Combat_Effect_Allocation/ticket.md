---
id: BRL-0016
title: 'P2: Centralize bounded combat effect allocation'
status: backlog
theme: quality
release:
created: 2026-08-27T18:15:31Z
modified: 2026-08-27T18:15:33Z
closed:
revision: 21e2cd7d4651bf6d
blocks: []
related: [BRL-0002]
---

# Description

Audit finding DUP-02. Four cue consumers duplicate effect-capacity eviction, oldest selection, sequence allocation, and nearly identical spawn bundles. Source: audit/bevy-rust-code-audit-20260827.md.
