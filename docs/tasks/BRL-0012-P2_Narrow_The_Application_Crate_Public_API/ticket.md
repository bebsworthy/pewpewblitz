---
id: BRL-0012
title: 'P2: Narrow the application crate public API'
status: todo
theme: quality
release:
priority: none
created: 2026-08-27T18:15:30Z
modified: 2026-08-27T20:54:28Z
closed:
revision: 3c8cd744e77959de
blocks: []
related: [BRL-0002]
---

# Description

Audit finding API-01. Most top-level modules and several implementation wildcard re-exports are public primarily for binaries and integration tests, expanding the compatibility surface. Source: audit/bevy-rust-code-audit-20260827.md.
