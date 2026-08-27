---
id: BRL-0008
title: 'P1: Replace broad system chains with semantic schedule phases'
status: done
theme: quality
release:
priority: none
created: 2026-08-27T18:15:29Z
modified: 2026-08-27T21:13:48Z
closed: 2026-08-27T21:13:48Z
revision: f22809dc4f43408d
blocks: []
related: [BRL-0002]
---

# Description

Audit finding SCHED-01. Client presentation, client session, and server lifecycle plugins use long chains that serialize unrelated work and obscure required causal edges. Source: audit/bevy-rust-code-audit-20260827.md.
