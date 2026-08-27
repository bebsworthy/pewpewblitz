---
id: BRL-0008
title: 'P1: Replace broad system chains with semantic schedule phases'
status: doing
theme: quality
release:
priority: none
created: 2026-08-27T18:15:29Z
modified: 2026-08-27T20:28:47Z
closed:
revision: 2d6ff9e40237a444
blocks: []
related: [BRL-0002]
---

# Description

Audit finding SCHED-01. Client presentation, client session, and server lifecycle plugins use long chains that serialize unrelated work and obscure required causal edges. Source: audit/bevy-rust-code-audit-20260827.md.
