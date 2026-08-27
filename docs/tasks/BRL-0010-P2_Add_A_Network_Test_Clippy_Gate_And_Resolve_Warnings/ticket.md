---
id: BRL-0010
title: 'P2: Add a network-test Clippy gate and resolve warnings'
status: todo
theme: quality
release:
priority: none
created: 2026-08-27T18:15:29Z
modified: 2026-08-27T20:54:23Z
closed:
revision: 87a40ff111b9f901
blocks: []
related: [BRL-0002]
---

# Description

Audit finding TOOL-01 and backlog item MAINT-NETWORK-TEST-LINT. network-test is checked and tested in CI but omitted from strict Clippy; the audit command found 29 errors. Source: audit/bevy-rust-code-audit-20260827.md.
