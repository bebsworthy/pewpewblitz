---
id: BRL-0011
title: 'P2: Restore signal in network-test error logging'
status: done
theme: quality
release:
priority: none
created: 2026-08-27T18:15:29Z
modified: 2026-08-27T22:01:49Z
closed: 2026-08-27T22:01:49Z
revision: 0d806d60070e81a8
blocks: []
related: [BRL-0002]
---

# Description

Audit finding OBS-01. Successful impairment and soak tests emit thousands of expected Lightyear ERROR lines plus repeated logger-installation errors, obscuring unexpected ECS failures. Source: audit/bevy-rust-code-audit-20260827.md.
