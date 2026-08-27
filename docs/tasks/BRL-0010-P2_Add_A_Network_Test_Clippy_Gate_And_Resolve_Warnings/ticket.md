---
id: BRL-0010
title: 'P2: Add a network-test Clippy gate and resolve warnings'
status: backlog
theme: quality
release:
created: 2026-08-27T18:15:29Z
modified: 2026-08-27T18:15:31Z
closed:
revision: c58fb0e7bb2afd38
blocks: []
related: [BRL-0002]
---

# Description

Audit finding TOOL-01 and backlog item MAINT-NETWORK-TEST-LINT. network-test is checked and tested in CI but omitted from strict Clippy; the audit command found 29 errors. Source: audit/bevy-rust-code-audit-20260827.md.
