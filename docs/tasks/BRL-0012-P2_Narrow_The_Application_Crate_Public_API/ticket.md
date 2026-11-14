---
id: BRL-0012

title: 'P2: Narrow the application crate public API'

status: done
closed: 2026-08-27T22:38:57Z
status_changed: 2026-08-27T22:38:57Z

theme: quality

release:

created: 2026-08-27T18:15:30Z

blocks: []

related: [BRL-0002, BRL-0170]

parent:

rank:
---

# Description

Audit finding API-01. Most top-level modules and several implementation wildcard re-exports are public primarily for binaries and integration tests, expanding the compatibility surface. Source: audit/bevy-rust-code-audit-20260827.md.
